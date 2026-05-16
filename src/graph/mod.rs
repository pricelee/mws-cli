//! Microsoft Graph HTTP client.

pub mod error;
pub mod paging;
pub mod upload;

use crate::auth::account::Account;
use crate::auth::Endpoints as TokenEndpoints;
use crate::auth::refresh;

pub use error::GraphError;

#[derive(Debug, Clone)]
pub struct GraphClient {
    http: reqwest::Client,
    base: String,
    token_endpoints: TokenEndpoints,
    max_retries: u32,
}

impl GraphClient {
    pub fn new(base: impl Into<String>, token_endpoints: TokenEndpoints) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
            token_endpoints,
            max_retries: 5,
        }
    }

    #[cfg(test)]
    pub fn set_max_retries(&mut self, n: u32) {
        self.max_retries = n;
    }

    pub async fn get_json(&self, account: &mut Account, path: &str) -> Result<serde_json::Value, GraphError> {
        let url = format!("{}{}", self.base, path);
        let mut refreshed = false;
        let mut throttle_attempts = 0u32;
        loop {
            let token = account
                .access_token
                .as_deref()
                .ok_or_else(|| GraphError::Api {
                    status: 401,
                    code: "no_token".into(),
                    message: "no access token cached".into(),
                })?;
            let resp = self.http.get(&url).bearer_auth(token).send().await?;
            let status = resp.status();

            if status == reqwest::StatusCode::UNAUTHORIZED && !refreshed {
                refresh::refresh(&self.http, &self.token_endpoints, account).await?;
                refreshed = true;
                continue;
            }

            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
                && throttle_attempts < self.max_retries
            {
                let wait = parse_retry_after(&resp).unwrap_or_else(|| {
                    backoff_delay(throttle_attempts)
                });
                tokio::time::sleep(wait).await;
                throttle_attempts += 1;
                continue;
            }

            if !status.is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                let code = body
                    .pointer("/error/code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let message = body
                    .pointer("/error/message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                return Err(GraphError::Api {
                    status: status.as_u16(),
                    code,
                    message,
                });
            }
            return Ok(resp.json().await?);
        }
    }

    /// Send a request with arbitrary method, body, and extra headers.
    /// Returns (status, response_body_bytes, content_type). 401-refresh and 429-retry are honored.
    pub async fn send_request(
        &self,
        account: &mut Account,
        method: reqwest::Method,
        path: &str,
        body: Option<Vec<u8>>,
        headers: &[(String, String)],
    ) -> Result<(reqwest::StatusCode, bytes::Bytes, Option<String>), GraphError> {
        let url = format!("{}{}", self.base, path);
        let mut refreshed = false;
        let mut throttle_attempts = 0u32;
        loop {
            let token = account
                .access_token
                .as_deref()
                .ok_or_else(|| GraphError::Api {
                    status: 401,
                    code: "no_token".into(),
                    message: "no access token cached".into(),
                })?;
            let mut req = self.http.request(method.clone(), &url).bearer_auth(token);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            if let Some(b) = body.clone() {
                req = req.body(b);
            }
            let resp = req.send().await?;
            let status = resp.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && !refreshed {
                refresh::refresh(&self.http, &self.token_endpoints, account).await?;
                refreshed = true;
                continue;
            }
            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
                && throttle_attempts < self.max_retries
            {
                let wait = parse_retry_after(&resp).unwrap_or_else(|| backoff_delay(throttle_attempts));
                tokio::time::sleep(wait).await;
                throttle_attempts += 1;
                continue;
            }
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let body = resp.bytes().await?;
            return Ok((status, body, content_type));
        }
    }
}

fn parse_retry_after(resp: &reqwest::Response) -> Option<std::time::Duration> {
    let header = resp.headers().get(reqwest::header::RETRY_AFTER)?;
    let s = header.to_str().ok()?;
    if let Ok(secs) = s.parse::<u64>() {
        return Some(std::time::Duration::from_secs(secs));
    }
    None
}

fn backoff_delay(attempt: u32) -> std::time::Duration {
    use rand::Rng;
    let base_ms = 1000u64;
    let exp = base_ms.saturating_mul(1u64 << attempt.min(6));
    let jitter = rand::thread_rng().gen_range(0..base_ms);
    std::time::Duration::from_millis(exp.saturating_add(jitter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::account::Account;
    use wiremock::matchers::{header, method, path as wpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn account() -> Account {
        let mut a = Account::new("x", "common", "CID", vec!["User.Read".into()]);
        a.access_token = Some("AT".into());
        a.access_token_expires_at = Some(u64::MAX);
        a.refresh_token = Some("RT".into());
        a
    }

    #[tokio::test]
    async fn get_me_returns_json() {
        let graph = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .and(header("authorization", "Bearer AT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "u1",
                "displayName": "Alice",
                "userPrincipalName": "alice@contoso.com"
            })))
            .mount(&graph)
            .await;
        let token = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account();
        let v = client.get_json(&mut a, "/me").await.unwrap();
        assert_eq!(v["userPrincipalName"], "alice@contoso.com");
    }

    #[tokio::test]
    async fn refreshes_on_401_then_succeeds() {
        let graph = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .and(header("authorization", "Bearer AT"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&graph)
            .await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .and(header("authorization", "Bearer AT2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "u1"})))
            .mount(&graph)
            .await;
        let token = MockServer::start().await;
        Mock::given(method("POST"))
            .and(wpath("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT2", "refresh_token": "RT2", "expires_in": 3600
            })))
            .mount(&token)
            .await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account();
        let v = client.get_json(&mut a, "/me").await.unwrap();
        assert_eq!(v["id"], "u1");
        assert_eq!(a.access_token.as_deref(), Some("AT2"));
    }

    #[tokio::test]
    async fn graph_error_is_surfaced() {
        let graph = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": {"code": "Authorization_RequestDenied", "message": "Insufficient privileges"}
            })))
            .mount(&graph)
            .await;
        let token = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account();
        let err = client.get_json(&mut a, "/me").await.unwrap_err();
        match err {
            GraphError::Api { status, code, .. } => {
                assert_eq!(status, 403);
                assert_eq!(code, "Authorization_RequestDenied");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn retries_on_429_with_retry_after_seconds() {
        let graph = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "0")
            )
            .up_to_n_times(1)
            .mount(&graph).await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "u1"})))
            .mount(&graph).await;
        let token = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account();
        let v = client.get_json(&mut a, "/me").await.unwrap();
        assert_eq!(v["id"], "u1");
    }

    #[tokio::test]
    async fn gives_up_after_max_retries() {
        let graph = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wpath("/me"))
            .respond_with(
                ResponseTemplate::new(429).insert_header("Retry-After", "0")
            )
            .mount(&graph).await;
        let token = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let mut client = GraphClient::new(graph.uri(), endpoints);
        client.set_max_retries(2);
        let mut a = account();
        let err = client.get_json(&mut a, "/me").await.unwrap_err();
        match err {
            GraphError::Api { status, .. } => assert_eq!(status, 429),
            other => panic!("expected 429 Api error, got {other:?}"),
        }
    }
}
