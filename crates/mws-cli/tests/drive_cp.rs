#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn cp_small_file_uses_single_put() {
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
    Mock::given(method("PUT"))
        .and(path("/me/drive/root:/Documents/hello.txt:/content"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "F1", "name": "hello.txt"
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("hello.txt");
    std::fs::write(&local, b"small content").unwrap();

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
            "drive", "cp",
            local.to_str().unwrap(),
            "mws:/Documents/hello.txt",
        ])
        .assert()
        .success()
        .stdout(contains("Uploaded"));
}

#[tokio::test(flavor = "multi_thread")]
async fn cp_large_file_uses_upload_session() {
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
    Mock::given(method("POST"))
        .and(path("/me/drive/root:/Documents/big.bin:/createUploadSession"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uploadUrl": format!("{base}/upload-session/big")
        })))
        .mount(&graph).await;
    Mock::given(method("PUT"))
        .and(path("/upload-session/big"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "F2", "name": "big.bin"
        })))
        .mount(&graph).await;

    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("big.bin");
    // 5 MiB payload — above the 4 MiB upload-session threshold but below the chunk size.
    std::fs::write(&local, vec![0u8; 5 * 1024 * 1024 + 1]).unwrap();

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
            "drive", "cp",
            local.to_str().unwrap(),
            "mws:/Documents/big.bin",
        ])
        .assert()
        .success()
        .stdout(contains("Uploaded"));
}
