#![cfg(feature = "test-helpers")]
//! Integration coverage for `mws-cli calendar ...`.

use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

#[tokio::test(flavor = "multi_thread")]
async fn events_dry_run_uses_default_window() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", "https://example.test",
            "--output", "json",
            "calendar", "events", "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("/me/calendarView?startDateTime="))
        .stdout(contains("endDateTime="));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_dry_run_emits_event_body() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", "https://example.test",
            "--output", "json",
            "calendar", "create",
            "--subject", "Sync",
            "--start", "2026-05-17T14:00:00Z",
            "--end",   "2026-05-17T15:00:00Z",
            "--attendee", "alice@x.com",
            "--online",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("\"dry_run\""))
        .stdout(contains("\"Sync\""))
        .stdout(contains("\"isOnlineMeeting\""))
        .stdout(contains("\"alice@x.com\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_sends_event_to_graph() {
    let idp = idp_mocks().await;
    let graph = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/me/events"))
        .and(header("authorization", "Bearer AT"))
        .and(body_json(serde_json::json!({
            "subject": "Sync",
            "start": {"dateTime": "2026-05-17T14:00:00", "timeZone": "UTC"},
            "end":   {"dateTime": "2026-05-17T15:00:00", "timeZone": "UTC"},
            "attendees": [{"emailAddress": {"address": "alice@x.com"}, "type": "required"}]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "E1", "subject": "Sync"
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    login_into(tmp.path(), &idp.uri()).await;
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", &graph.uri(),
            "--output", "json",
            "calendar", "create",
            "--subject", "Sync",
            "--start", "2026-05-17T14:00:00Z",
            "--end",   "2026-05-17T15:00:00Z",
            "--attendee", "alice@x.com",
        ])
        .assert()
        .success()
        .stdout(contains("\"E1\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn rsvp_dry_run_picks_accept_path() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws-cli").unwrap()
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", "https://example.test",
            "--output", "json",
            "calendar", "rsvp",
            "--event", "AAA",
            "--response", "accept",
            "--comment", "running late",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("/me/events/AAA/accept"))
        .stdout(contains("\"running late\""));
}
