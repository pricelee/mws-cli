use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn whoami_prints_user_after_login() {
    // 1) Spin up token endpoints for the login step.
    let idp = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC", "user_code": "X", "verification_uri": "x",
            "expires_in": 60, "interval": 0
        })))
        .mount(&idp)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 86400
        })))
        .mount(&idp)
        .await;

    // 2) Spin up Graph.
    let graph = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer AT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "u1", "displayName": "Alice Tester", "userPrincipalName": "alice@example.com",
            "mail": "alice@example.com"
        })))
        .mount(&graph)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();

    // 3) Login.
    // Use --config-dir so both subprocesses share an isolated config directory.
    // directories::ProjectDirs uses SHGetKnownFolderPath on Windows and ignores
    // the APPDATA env var, so we pass the path explicitly to get proper isolation.
    // The vault service name is derived via a deterministic hash of this path so
    // each tempdir gets its own Windows Credential Manager entry.
    Command::cargo_bin("mws").unwrap()
        .args(["--config-dir", cfg,
               "auth", "login", "--device",
               "--device-endpoint", &format!("{}/devicecode", idp.uri()),
               "--token-endpoint", &format!("{}/token", idp.uri()),
        ])
        .assert().success();

    // 4) Whoami.
    Command::cargo_bin("mws").unwrap()
        .args(["--config-dir", cfg,
               "--output", "json",
               "--graph-base", &graph.uri(),
               "whoami"])
        .assert()
        .success()
        .stdout(contains("Alice Tester"))
        .stdout(contains("alice@example.com"));
}
