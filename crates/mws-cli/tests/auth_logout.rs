#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn idp_with_mocks() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC", "user_code": "X", "verification_uri": "x",
            "expires_in": 60, "interval": 0
        })))
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 3600
        })))
        .mount(&server).await;
    server
}

fn login(tmp: &std::path::Path, idp: &MockServer) {
    Command::cargo_bin("mws").unwrap()
        .args([
            "--config-dir", tmp.to_str().unwrap(),
            "auth", "login", "--device",
            "--device-endpoint", &format!("{}/devicecode", idp.uri()),
            "--token-endpoint", &format!("{}/token", idp.uri()),
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn logout_removes_account_file() {
    let idp = idp_with_mocks().await;
    let tmp = tempfile::tempdir().unwrap();
    login(tmp.path(), &idp);

    let accounts_dir = tmp.path().join("accounts");
    assert!(accounts_dir.join("default.bin").exists(), "login should write default.bin");

    Command::cargo_bin("mws").unwrap()
        .args(["--config-dir", tmp.path().to_str().unwrap(), "auth", "logout"])
        .assert()
        .success()
        .stdout(contains("Removed account 'default'"));

    assert!(!accounts_dir.join("default.bin").exists(), "logout should remove default.bin");
}

#[tokio::test(flavor = "multi_thread")]
async fn logout_all_clears_directory() {
    let idp = idp_with_mocks().await;
    let tmp = tempfile::tempdir().unwrap();
    login(tmp.path(), &idp);

    Command::cargo_bin("mws").unwrap()
        .args([
            "--account", "work",
            "--config-dir", tmp.path().to_str().unwrap(),
            "auth", "login", "--device",
            "--device-endpoint", &format!("{}/devicecode", idp.uri()),
            "--token-endpoint", &format!("{}/token", idp.uri()),
        ])
        .assert()
        .success();

    let accounts_dir = tmp.path().join("accounts");
    assert!(accounts_dir.join("default.bin").exists());
    assert!(accounts_dir.join("work.bin").exists());

    Command::cargo_bin("mws").unwrap()
        .args(["--config-dir", tmp.path().to_str().unwrap(), "auth", "logout", "--all"])
        .assert()
        .success();

    assert!(!accounts_dir.join("default.bin").exists());
    assert!(!accounts_dir.join("work.bin").exists());
}
