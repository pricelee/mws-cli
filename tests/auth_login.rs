#![cfg(feature = "test-helpers")]

use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn device_code_login_saves_account() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://microsoft.com/devicelogin",
            "expires_in": 60,
            "interval": 0
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();
    // Use --config-dir so the subprocess writes to the tempdir.
    // directories::ProjectDirs ignores APPDATA on Windows (uses SHGetKnownFolderPath),
    // so env var overrides alone do not isolate test state.
    Command::cargo_bin("mws-cli").unwrap()
        .args(["--config-dir", cfg,
               "auth", "login", "--device",
               "--device-endpoint", &format!("{}/devicecode", server.uri()),
               "--token-endpoint", &format!("{}/token", server.uri()),
        ])
        .assert().success().stdout(contains("ABCD-EFGH")).stdout(contains("Saved account"));
}
