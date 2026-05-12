#![cfg(feature = "test-helpers")]
use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn raw_delete_refused_without_yes() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws")
        .unwrap()
        .args([
            "--config-dir",
            tmp.path().to_str().unwrap(),
            "raw",
            "DELETE",
            "/me/messages/AAA",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("destructive operation refused"))
        .stderr(contains("--yes"));
}

#[test]
fn raw_post_archive_refused_without_yes() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws")
        .unwrap()
        .args([
            "--config-dir",
            tmp.path().to_str().unwrap(),
            "raw",
            "POST",
            "/teams/X/archive",
        ])
        .assert()
        .failure()
        .code(4)
        .stderr(contains("destructive operation refused"));
}

#[test]
fn raw_delete_dry_run_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws")
        .unwrap()
        .args([
            "--config-dir",
            tmp.path().to_str().unwrap(),
            "--dry-run",
            "--output",
            "json",
            "raw",
            "DELETE",
            "/me/messages/AAA",
        ])
        .assert()
        .success()
        .stdout(contains("\"dry_run\": true"))
        .stdout(contains("\"method\": \"DELETE\""))
        .stdout(contains("\"destructive\": true"));
}

#[test]
fn raw_safe_get_dry_run_is_not_destructive() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("mws")
        .unwrap()
        .args([
            "--config-dir",
            tmp.path().to_str().unwrap(),
            "--dry-run",
            "--output",
            "json",
            "raw",
            "GET",
            "/me",
        ])
        .assert()
        .success()
        .stdout(contains("\"method\": \"GET\""))
        .stdout(contains("\"destructive\": false"));
}
