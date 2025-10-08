use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_invalid_profile_error_message() {
    let temp_dir = tempdir().unwrap();
    let devenv_nix_path = temp_dir.path().join("devenv.nix");
    fs::write(
        devenv_nix_path,
        r#"{ pkgs, ... }: { profiles = { test = {}; }; }"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("devenv").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["--profile", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Profile 'nonexistent' not found"));
}

#[test]
fn test_profile_help_shows_global_help() {
    let temp_dir = tempdir().unwrap();
    let devenv_nix_path = temp_dir.path().join("devenv.nix");
    fs::write(
        devenv_nix_path,
        r#"{ pkgs, ... }: { profiles = { ci = {}; }; }"#,
    )
    .unwrap();
    fs::write(temp_dir.path().join("devenv.yaml"), "{}").unwrap();

    let mut cmd = Command::cargo_bin("devenv").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["--profile", "ci", "--help"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Fast, Declarative, Reproducible, and Composable")
                .and(predicates::str::contains("Usage: devenv [OPTIONS] [COMMAND]")),
        );
}

#[test]
fn test_profile_test_command_help() {
    let temp_dir = tempdir().unwrap();
    let devenv_nix_path = temp_dir.path().join("devenv.nix");
    fs::write(
        devenv_nix_path,
        r#"{ pkgs, ... }: { profiles = { ci = {}; }; }"#,
    )
    .unwrap();
    fs::write(temp_dir.path().join("devenv.yaml"), "{}").unwrap();

    let mut cmd = Command::cargo_bin("devenv").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["--profile", "ci", "test", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage: devenv test"));
}

#[test]
fn test_ci_profile_runs_tests() {
    let temp_dir = tempdir().unwrap();
    let devenv_nix_path = temp_dir.path().join("devenv.nix");
    fs::write(
        devenv_nix_path,
        r#"
{ pkgs, ... }: {
  profiles = { ci = {}; };
  test = ''
    echo "Tests passed!"
  '';
}"#,
    )
    .unwrap();
    fs::write(temp_dir.path().join("devenv.yaml"), "{}").unwrap();

    let mut cmd = Command::cargo_bin("devenv").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["--profile", "ci"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Tests passed!"));
}