#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn mail_send_inline_no_attachments() {
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
    Mock::given(method("POST"))
        .and(path("/me/sendMail"))
        .and(body_string_contains("\"subject\":\"hello\""))
        .and(body_string_contains("\"address\":\"alice@example.com\""))
        .respond_with(ResponseTemplate::new(202))
        .mount(&graph).await;

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
            "mail", "send",
            "--to", "alice@example.com",
            "--subject", "hello",
            "--body", "test body",
        ])
        .assert()
        .success()
        .stdout(contains("Sent."));
}

#[tokio::test(flavor = "multi_thread")]
async fn mail_send_inline_small_attachment() {
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
    Mock::given(method("POST"))
        .and(path("/me/sendMail"))
        .and(body_string_contains("#microsoft.graph.fileAttachment"))
        .and(body_string_contains("\"name\":\"hello.txt\""))
        .respond_with(ResponseTemplate::new(202))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    let attach = tmp.path().join("hello.txt");
    std::fs::write(&attach, b"small attachment payload").unwrap();

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
            "mail", "send",
            "--to", "alice@example.com",
            "--subject", "with attachment",
            "--body", "see attached",
            "--attachment", attach.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("Sent."));
}
