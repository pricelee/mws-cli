#![cfg(feature = "test-helpers")]
//! End-to-end seam: a runtime Graph 403 "Missing scope permissions" is turned
//! into an actionable consent remediation with the documented exit code.

use assert_cmd::Command;
use predicates::boolean::PredicateBooleanExt;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Seed a signed-in account in `cfg` by running device-code login against `server`.
async fn seed_account(server: &MockServer, cfg: &str) {
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
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 3600
        })))
        .mount(server)
        .await;
    Command::cargo_bin("mws-cli")
        .unwrap()
        .args([
            "--config-dir", cfg,
            "auth", "login", "--device",
            "--device-endpoint", &format!("{}/devicecode", server.uri()),
            "--token-endpoint", &format!("{}/token", server.uri()),
        ])
        .assert()
        .success();
}

/// A Graph 403 whose one-of list includes a self-consentable candidate
/// (`Team.ReadBasic.All` is a DEFAULT scope ⇒ user-consentable despite `.All`).
async fn mount_self_consentable_403(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/me/joinedTeams"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": {
                "code": "Forbidden",
                "message": "Missing scope permissions on the request. API requires one of 'Team.ReadBasic.All, Directory.Read.All'. Scopes on the request 'openid, profile, User.Read'",
                "innerError": {}
            }
        })))
        .mount(server)
        .await;
}

/// A Graph 403 whose required scopes are ALL admin-consent scopes.
async fn mount_all_admin_403(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/me/joinedTeams"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": {
                "code": "Forbidden",
                "message": "Missing scope permissions on the request. API requires one of 'Sites.Read.All, Directory.Read.All'. Scopes on the request 'openid, profile, User.Read'",
                "innerError": {}
            }
        })))
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_403_all_admin_renders_admin_consent_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();
    seed_account(&server, cfg).await;
    mount_all_admin_403(&server).await;

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args([
            "--config-dir", cfg,
            "--graph-base", &server.uri(),
            "--output", "table", // force the human path (non-TTY default is json)
            "raw", "GET", "/me/joinedTeams",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("admin consent"))
        .stderr(contains("/organizations/v2.0/adminconsent"))
        .stderr(contains("scope=Sites.Read.All"))
        .stderr(contains("mws-cli auth login --scope Sites.Read.All"))
        .stderr(contains("After your admin clicks Accept"));
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_403_self_consentable_routes_to_user_consent_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();
    seed_account(&server, cfg).await;
    mount_self_consentable_403(&server).await;

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args([
            "--config-dir", cfg,
            "--graph-base", &server.uri(),
            "--output", "table",
            "raw", "GET", "/me/joinedTeams",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("you can grant yourself"))
        .stderr(contains("mws-cli auth login --scope Team.ReadBasic.All"))
        .stderr(contains("adminconsent").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_403_self_consentable_emits_user_consent_json() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();
    seed_account(&server, cfg).await;
    mount_self_consentable_403(&server).await;

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args([
            "--config-dir", cfg,
            "--graph-base", &server.uri(),
            "--output", "json",
            "raw", "GET", "/me/joinedTeams",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("\"type\": \"user_consent\""))
        .stderr(contains("Team.ReadBasic.All"))
        .stderr(contains("consent_url").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_403_all_admin_emits_remediation_json() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().to_str().unwrap();
    seed_account(&server, cfg).await;
    mount_all_admin_403(&server).await;

    Command::cargo_bin("mws-cli")
        .unwrap()
        .args([
            "--config-dir", cfg,
            "--graph-base", &server.uri(),
            "--output", "json",
            "raw", "GET", "/me/joinedTeams",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("\"type\": \"admin_consent\""))
        .stderr(contains("\"consent_url\""))
        .stderr(contains("\"scopes\""))
        .stderr(contains("Sites.Read.All"))
        .stderr(contains("\"next_steps\""));
}
