use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn prints_help() {
    Command::cargo_bin("mws-cli").unwrap().arg("--help").assert().success().stdout(contains("Microsoft Workspace CLI"));
}

#[test]
fn version_flag_works() {
    Command::cargo_bin("mws-cli").unwrap().arg("--version").assert().success().stdout(contains("0.0.1"));
}

#[test]
fn unknown_subcommand_fails() {
    Command::cargo_bin("mws-cli").unwrap().arg("nonsense").assert().failure();
}
