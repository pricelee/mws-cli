use std::time::Duration;

use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::account::now_secs;
use crate::device_code::{Endpoints, TokenGrant};
use crate::error::AuthError;

/// PKCE pair (verifier + challenge).
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier: String = rand::thread_rng().sample_iter(&Alphanumeric).take(64).map(char::from).collect();
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let digest = hasher.finalize();
        let challenge = base64_url(&digest);
        Self { verifier, challenge }
    }
}

fn base64_url(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub authorize_url: Url,
    pub state: String,
    pub pkce: Pkce,
    pub redirect_uri: String,
}

pub fn build_authorize_request(
    endpoints: &Endpoints,
    tenant: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
) -> AuthorizeRequest {
    let state: String = rand::thread_rng().sample_iter(&Alphanumeric).take(24).map(char::from).collect();
    let pkce = Pkce::generate();
    let authorize_base = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize");
    let mut url = Url::parse(&authorize_base).expect("valid url");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");
    // endpoints is unused for the authorize URL today but kept for symmetry with tests.
    let _ = endpoints;
    AuthorizeRequest { authorize_url: url, state, pkce, redirect_uri: redirect_uri.to_string() }
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

pub async fn exchange_code(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    pkce_verifier: &str,
) -> Result<TokenGrant, AuthError> {
    let resp = http
        .post(endpoints.token.clone())
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", pkce_verifier),
        ])
        .send()
        .await?;
    let body: TokenResponse = resp.json().await?;
    if let Some(err) = body.error {
        return Err(AuthError::OAuth { error: err, description: body.error_description.unwrap_or_default() });
    }
    Ok(TokenGrant {
        access_token: body.access_token.ok_or_else(|| AuthError::State("no access_token".into()))?,
        refresh_token: body.refresh_token,
        id_token: body.id_token,
        expires_in: body.expires_in.unwrap_or(0),
    })
}

/// Bind a loopback listener on 127.0.0.1:0; returns (server, redirect_uri).
pub fn loopback() -> Result<(tiny_http::Server, String), AuthError> {
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| AuthError::State(format!("bind loopback: {e}")))?;
    let addr = server.server_addr();
    #[allow(irrefutable_let_patterns)]
    let port = if let tiny_http::ListenAddr::IP(sa) = addr {
        sa.port()
    } else {
        return Err(AuthError::State("expected IP listen addr".into()));
    };
    let redirect = format!("http://127.0.0.1:{port}/callback");
    Ok((server, redirect))
}

/// Block until the browser hits the loopback with `code` and `state`.
/// Returns (`code`, `state`). Sends a small confirmation HTML response back to the browser.
pub fn await_callback(server: tiny_http::Server, timeout: Duration) -> Result<(String, String), AuthError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(std::time::Instant::now()).ok_or(AuthError::Timeout)?;
        let req = match server.recv_timeout(remaining).map_err(|e| AuthError::State(e.to_string()))? {
            Some(r) => r,
            None => return Err(AuthError::Timeout),
        };
        let url = req.url().to_string();
        let parsed = Url::parse(&format!("http://127.0.0.1{url}")).map_err(|e| AuthError::State(e.to_string()))?;
        let mut code = None;
        let mut state = None;
        let mut err = None;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                "error" => err = Some(v.into_owned()),
                _ => {}
            }
        }
        let response_body = "<html><body><h2>mws: sign-in complete</h2><p>You can close this tab.</p></body></html>";
        let resp = tiny_http::Response::from_string(response_body).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
        );
        let _ = req.respond(resp);
        if let Some(e) = err {
            return Err(AuthError::OAuth { error: e, description: String::new() });
        }
        match (code, state) {
            (Some(c), Some(s)) => return Ok((c, s)),
            _ => continue,
        }
    }
}

pub fn apply_grant(account: &mut crate::account::Account, grant: TokenGrant) {
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

    fn eps(base: &str) -> Endpoints {
        Endpoints {
            device_authorization: format!("{base}/devicecode").parse().unwrap(),
            token: format!("{base}/token").parse().unwrap(),
        }
    }

    #[test]
    fn authorize_url_has_pkce_and_scopes() {
        let eps = eps("http://ignored");
        let req = build_authorize_request(&eps, "common", "CID", &["User.Read".into()], "http://127.0.0.1:1234/callback");
        let q: std::collections::HashMap<String, String> = req.authorize_url.query_pairs().into_owned().collect();
        assert_eq!(q.get("client_id").map(String::as_str), Some("CID"));
        assert_eq!(q.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(q.get("code_challenge_method").map(String::as_str), Some("S256"));
        assert!(!q.get("code_challenge").unwrap().is_empty());
        assert_eq!(q.get("scope").map(String::as_str), Some("User.Read"));
    }

    #[tokio::test]
    async fn exchange_code_returns_grant() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=CODE"))
            .and(body_string_contains("code_verifier=VER"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT",
                "refresh_token": "RT",
                "id_token": "IT",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let grant = exchange_code(&http, &eps(&server.uri()), "CID", "http://127.0.0.1/cb", "CODE", "VER").await.unwrap();
        assert_eq!(grant.access_token, "AT");
        assert_eq!(grant.refresh_token.as_deref(), Some("RT"));
    }

    #[test]
    fn loopback_callback_round_trip() {
        let (server, redirect) = loopback().unwrap();
        let url = format!("{redirect}?code=ABC&state=XYZ");
        // Hit the loopback in a background thread.
        let handle = std::thread::spawn(move || {
            let resp = reqwest::blocking::get(&url).unwrap();
            let _ = resp.text();
        });
        let (code, state) = await_callback(server, Duration::from_secs(5)).unwrap();
        assert_eq!(code, "ABC");
        assert_eq!(state, "XYZ");
        handle.join().unwrap();
    }
}
