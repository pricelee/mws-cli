#![cfg(feature = "test-helpers")]
//! Integration coverage for `mws-cli teams ...`. We test:
//!   1. `teams list` against a wiremocked Graph (collection GET path).
//!   2. `teams post --dry-run` — pure local; asserts the prepared request shape.
//!   3. `teams post` against wiremock — asserts URL, body JSON, content type.
//!   4. `teams channels --team "bad/id"` — usage error, exit code != 0.

use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn login_into(tmp: &std::path::Path, idp_uri: &str) {
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.to_str().unwrap(),
            "auth", "login", "--device",
            "--device-endpoint", &format!("{idp_uri}/devicecode"),
            "--token-endpoint", &format!("{idp_uri}/token"),
        ])
        .assert().success();
}

async fn idp_mocks() -> MockServer {
    let idp = MockServer::start().await;
    Mock::given(method("POST")).and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "DC", "user_code": "X", "verification_uri": "x",
            "expires_in": 60, "interval": 0
        })))
        .mount(&idp).await;
    Mock::given(method("POST")).and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "AT", "refresh_token": "RT", "expires_in": 86400
        })))
        .mount(&idp).await;
    idp
}

#[tokio::test(flavor = "multi_thread")]
async fn teams_list_returns_joined_teams() {
    let idp = idp_mocks().await;
    let graph = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me/joinedTeams"))
        .and(header("authorization", "Bearer AT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": [{"id": "T1", "displayName": "Eng"}]
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    login_into(tmp.path(), &idp.uri()).await;
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", &graph.uri(),
            "--output", "json",
            "teams", "list",
        ])
        .assert()
        .success()
        .stdout(contains("\"T1\""))
        .stdout(contains("\"Eng\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn teams_post_dry_run_prints_prepared_request() {
    let tmp = tempfile::tempdir().unwrap();
    // No login needed: dry-run never touches the account store.
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", "https://example.test",
            "--output", "json",
            "teams", "post",
            "--team", "T1", "--channel", "C1", "--message", "hello",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("\"dry_run\""))
        .stdout(contains("/teams/T1/channels/C1/messages"))
        .stdout(contains("\"contentType\""))
        .stdout(contains("\"text\""))
        .stdout(contains("\"hello\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn teams_post_sends_message_body() {
    let idp = idp_mocks().await;
    let graph = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/teams/T1/channels/C1/messages"))
        .and(header("authorization", "Bearer AT"))
        .and(body_json(serde_json::json!({
            "body": {"content": "hi", "contentType": "text"}
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "M1", "createdDateTime": "2026-05-13T00:00:00Z"
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    login_into(tmp.path(), &idp.uri()).await;
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", &graph.uri(),
            "--output", "json",
            "teams", "post",
            "--team", "T1", "--channel", "C1", "--message", "hi",
        ])
        .assert()
        .success()
        .stdout(contains("\"M1\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn teams_channels_rejects_bad_team_id() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "teams", "channels", "--team", "bad/id",
        ])
        .assert()
        .failure()
        .stderr(contains("invalid character"));
}
