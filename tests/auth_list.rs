#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn list_empty_prints_no_accounts() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args(["--config-dir", tmp.path().to_str().unwrap(), "auth", "list"])
        .assert()
        .success()
        .stdout(contains("No accounts"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_shows_logged_in_account() {
    let idp = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC", "user_code": "X", "verification_uri": "x",
            "expires_in": 60, "interval": 0
        })))
        .mount(&idp).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 3600
        })))
        .mount(&idp).await;

    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "auth", "login", "--device",
            "--device-endpoint", &format!("{}/devicecode", idp.uri()),
            "--token-endpoint", &format!("{}/token", idp.uri()),
        ])
        .assert().success();

    Command::cargo_bin("mws-cli").unwrap()
        .args(["--config-dir", tmp.path().to_str().unwrap(), "--output", "json", "auth", "list"])
        .assert()
        .success()
        .stdout(contains("default"))
        .stdout(contains("common"));
}
