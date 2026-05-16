use std::time::Duration;

use serde::Deserialize;
use tokio::time::sleep;

use super::account::{now_secs, Account};
use super::error::AuthError;
use super::token::{Endpoints, TokenGrant};

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn start(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    client_id: &str,
    scopes: &[String],
) -> Result<DeviceAuthorization, AuthError> {
    let resp = http
        .post(endpoints.device_authorization.clone())
        .form(&[("client_id", client_id), ("scope", &scopes.join(" "))])
        .send()
        .await?
        .error_for_status()?
        .json::<DeviceAuthorization>()
        .await?;
    Ok(resp)
}

pub async fn poll(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    client_id: &str,
    auth: &DeviceAuthorization,
) -> Result<TokenGrant, AuthError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(auth.expires_in);
    let mut interval = Duration::from_secs(auth.interval.max(1));
    loop {
        if std::time::Instant::now() >= deadline {
            return Err(AuthError::Timeout);
        }
        sleep(interval).await;
        let resp = http
            .post(endpoints.token.clone())
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", auth.device_code.as_str()),
            ])
            .send()
            .await?;
        let status = resp.status();
        let body: TokenResponse = resp.json().await?;
        if let Some(err) = body.error.as_deref() {
            match err {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += Duration::from_secs(5);
                    continue;
                }
                "expired_token" => return Err(AuthError::Timeout),
                "authorization_declined" | "bad_verification_code" => {
                    return Err(AuthError::Cancelled);
                }
                _ => {
                    return Err(AuthError::OAuth {
                        error: err.to_string(),
                        description: body.error_description.unwrap_or_default(),
                    });
                }
            }
        }
        if !status.is_success() {
            return Err(AuthError::State(format!("token endpoint returned {status}")));
        }
        return Ok(TokenGrant {
            access_token: body.access_token.ok_or_else(|| AuthError::State("no access_token in response".into()))?,
            refresh_token: body.refresh_token,
            id_token: body.id_token,
            expires_in: body.expires_in.unwrap_or(0),
        });
    }
}

pub fn apply_grant(account: &mut Account, grant: TokenGrant) {
    account.access_token = Some(grant.access_token.into());
    account.access_token_expires_at = Some(now_secs() + grant.expires_in);
    if let Some(rt) = grant.refresh_token {
        account.refresh_token = Some(rt.into());
    }
    if let Some(it) = grant.id_token {
        account.id_token = Some(it.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn endpoints(base: &str) -> Endpoints {
        Endpoints {
            device_authorization: format!("{base}/devicecode").parse().unwrap(),
            token: format!("{base}/token").parse().unwrap(),
        }
    }

    #[tokio::test]
    async fn start_returns_device_authorization() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/devicecode"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "DC",
                "user_code": "ABCD-EFGH",
                "verification_uri": "https://microsoft.com/devicelogin",
                "expires_in": 900,
                "interval": 5,
                "message": "go enter ABCD-EFGH"
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let eps = endpoints(&server.uri());
        let auth = start(&http, &eps, "client", &["User.Read".to_string()]).await.unwrap();
        assert_eq!(auth.user_code, "ABCD-EFGH");
        assert_eq!(auth.interval, 5);
    }

    #[tokio::test]
    async fn poll_succeeds_after_pending() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "authorization_pending",
                "error_description": "still waiting"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT",
                "refresh_token": "RT",
                "id_token": "IT",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let eps = endpoints(&server.uri());
        let auth = DeviceAuthorization {
            device_code: "DC".into(),
            user_code: "x".into(),
            verification_uri: "x".into(),
            expires_in: 60,
            interval: 0, // poll fast in tests
            message: None,
        };
        let grant = poll(&http, &eps, "client", &auth).await.unwrap();
        assert_eq!(grant.access_token, "AT");
        assert_eq!(grant.refresh_token.as_deref(), Some("RT"));
    }

    #[tokio::test]
    async fn poll_returns_cancelled_on_decline() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("device_code=DC"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "authorization_declined"
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let eps = endpoints(&server.uri());
        let auth = DeviceAuthorization {
            device_code: "DC".into(),
            user_code: "x".into(),
            verification_uri: "x".into(),
            expires_in: 60,
            interval: 0,
            message: None,
        };
        let err = poll(&http, &eps, "client", &auth).await.unwrap_err();
        assert!(matches!(err, AuthError::Cancelled));
    }
}
