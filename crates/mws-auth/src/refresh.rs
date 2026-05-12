use serde::Deserialize;

use crate::account::{now_secs, Account};
use crate::device_code::{Endpoints, TokenGrant};
use crate::error::AuthError;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn refresh(http: &reqwest::Client, endpoints: &Endpoints, account: &mut Account) -> Result<(), AuthError> {
    let rt = account.refresh_token.as_deref().ok_or(AuthError::NoRefreshToken)?;
    let resp = http
        .post(endpoints.token.clone())
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &account.client_id),
            ("refresh_token", rt),
            ("scope", &account.scopes.join(" ")),
        ])
        .send()
        .await?;
    let body: TokenResponse = resp.json().await?;
    if let Some(err) = body.error {
        return Err(AuthError::OAuth { error: err, description: body.error_description.unwrap_or_default() });
    }
    let grant = TokenGrant {
        access_token: body.access_token.ok_or_else(|| AuthError::State("no access_token in refresh response".into()))?,
        refresh_token: body.refresh_token,
        id_token: body.id_token,
        expires_in: body.expires_in.unwrap_or(0),
    };
    account.access_token = Some(grant.access_token);
    account.access_token_expires_at = Some(now_secs() + grant.expires_in);
    if grant.refresh_token.is_some() {
        account.refresh_token = grant.refresh_token;
    }
    if grant.id_token.is_some() {
        account.id_token = grant.id_token;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::Account;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn eps(base: &str) -> Endpoints {
        Endpoints {
            device_authorization: format!("{base}/devicecode").parse().unwrap(),
            token: format!("{base}/token").parse().unwrap(),
        }
    }

    #[tokio::test]
    async fn refresh_updates_account() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT2",
                "refresh_token": "RT2",
                "expires_in": 1800
            })))
            .mount(&server)
            .await;
        let mut a = Account::new("x", "common", "CID", vec!["User.Read".into()]);
        a.refresh_token = Some("RT".into());
        let http = reqwest::Client::new();
        refresh(&http, &eps(&server.uri()), &mut a).await.unwrap();
        assert_eq!(a.access_token.as_deref(), Some("AT2"));
        assert_eq!(a.refresh_token.as_deref(), Some("RT2"));
    }

    #[tokio::test]
    async fn refresh_without_token_fails() {
        let server = MockServer::start().await;
        let mut a = Account::new("x", "common", "CID", vec![]);
        let http = reqwest::Client::new();
        let err = refresh(&http, &eps(&server.uri()), &mut a).await.unwrap_err();
        assert!(matches!(err, AuthError::NoRefreshToken));
    }
}
