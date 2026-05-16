#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn raw_all_follows_next_link() {
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
    let base = graph.uri();
    Mock::given(method("GET"))
        .and(path("/me/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": [{"id": "m1"}, {"id": "m2"}],
            "@odata.nextLink": format!("{base}/me/messages?$skiptoken=PG")
        }))).up_to_n_times(1).mount(&graph).await;
    Mock::given(method("GET"))
        .and(path("/me/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": [{"id": "m3"}]
        }))).mount(&graph).await;

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
        .args([
            "--config-dir", tmp.path().to_str().unwrap(),
            "--graph-base", &graph.uri(),
            "--all",
            "--output", "json",
            "raw", "GET", "/me/messages",
        ])
        .assert()
        .success()
        .stdout(contains("m1"))
        .stdout(contains("m2"))
        .stdout(contains("m3"));
}
