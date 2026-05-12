//! Microsoft Graph HTTP client.

pub mod error;

use mws_auth::account::Account;
use mws_auth::device_code::Endpoints as TokenEndpoints;
use mws_auth::refresh;

pub use error::GraphError;

#[derive(Debug, Clone)]
pub struct GraphClient {
    http: reqwest::Client,
    base: String,
    token_endpoints: TokenEndpoints,
}

impl GraphClient {
    pub fn new(base: impl Into<String>, token_endpoints: TokenEndpoints) -> Self {
        Self { http: reqwest::Client::new(), base: base.into(), token_endpoints }
    }

    /// Convenience: v1.0 against the common tenant.
    pub fn v1() -> Self {
        Self::new("https://graph.microsoft.com/v1.0", TokenEndpoints::for_tenant("common"))
    }

    pub async fn get_json(&self, account: &mut Account, path: &str) -> Result<serde_json::Value, GraphError> {
        let url = format!("{}{}", self.base, path);
        for attempt in 0..2 {
            let token = account
                .access_token
                .as_deref()
                .ok_or_else(|| GraphError::Api { status: 401, code: "no_token".into(), message: "no access token cached".into() })?;
            let resp = self.http.get(&url).bearer_auth(token).send().await?;
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                refresh::refresh(&self.http, &self.token_endpoints, account).await?;
                continue;
            }
            let status = resp.status();
            if !status.is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                let code = body.pointer("/error/code").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                let message = body.pointer("/error/message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                return Err(GraphError::Api { status: status.as_u16(), code, message });
            }
            return Ok(resp.json().await?);
        }
        unreachable!("loop exits in <=2 iterations")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mws_auth::account::Account;
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
        let endpoints = mws_auth::device_code::Endpoints {
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
        let endpoints = mws_auth::device_code::Endpoints {
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
        let endpoints = mws_auth::device_code::Endpoints {
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
}
