#![cfg(feature = "test-helpers")]
//! End-to-end seam: an `auth login` that fails because a requested scope needs
//! admin consent is turned into an admin-consent remediation with exit code 3.

use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Device-code flow where the token endpoint rejects with an admin-consent error.
async fn mount_admin_consent_required(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://microsoft.com/devicelogin",
            "expires_in": 60,
            "interval": 0
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "consent_required",
            "error_description": "AADSTS90094: The grant requires admin permission. Needs admin approval."
        })))
        .mount(server)
        .await;
}

fn login_args<'a>(cfg: &'a str, server_uri: &'a str, output: &'a str) -> Vec<String> {
    vec![
        "--config-dir".into(), cfg.into(),
        "--output".into(), output.into(),
        "auth".into(), "login".into(), "--device".into(),
        "--scope".into(), "Sites.Read.All".into(),
        "--device-endpoint".into(), format!("{server_uri}/devicecode"),
        "--token-endpoint".into(), format!("{server_uri}/token"),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn login_admin_consent_required_renders_text_exit_3() {
    let server = MockServer::start().await;
    mount_admin_consent_required(&server).await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args(login_args(cfg, &server.uri(), "table"))
        .assert()
        .failure()
        .code(3)
        .stderr(contains("admin consent"))
        .stderr(contains("/organizations/v2.0/adminconsent"))
        .stderr(contains("scope=Sites.Read.All"))
        .stderr(contains("mws-cli auth login --scope Sites.Read.All"));
}

#[tokio::test(flavor = "multi_thread")]
async fn login_admin_consent_required_emits_json_exit_3() {
    let server = MockServer::start().await;
    mount_admin_consent_required(&server).await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args(login_args(cfg, &server.uri(), "json"))
        .assert()
        .failure()
        .code(3)
        .stderr(contains("\"type\": \"admin_consent\""))
        .stderr(contains("\"consent_url\""))
        .stderr(contains("Sites.Read.All"));
}
