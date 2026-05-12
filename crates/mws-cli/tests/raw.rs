#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn raw_get_me_returns_json() {
    let idp = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC", "user_code": "X", "verification_uri": "x",
            "expires_in": 60, "interval": 0
        }))).mount(&idp).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 86400
        }))).mount(&idp).await;

    let graph = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer AT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "u1", "userPrincipalName": "alice@example.com"
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();

    Command::cargo_bin("mws").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "auth", "login", "--device",
            "--device-endpoint", &format!("{}/devicecode", idp.uri()),
            "--token-endpoint", &format!("{}/token", idp.uri()),
        ])
        .assert().success();

    Command::cargo_bin("mws").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", &graph.uri(),
            "--output", "json",
            "raw", "GET", "/me",
        ])
        .assert()
        .success()
        .stdout(contains("alice@example.com"));
}
