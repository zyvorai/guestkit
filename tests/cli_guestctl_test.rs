// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI tests for guestctl alias and UX entry points.

use assert_cmd::Command;
use predicates::prelude::*;

fn guestctl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_guestctl"))
}

fn guestkit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_guestkit"))
}

#[test]
fn guestctl_version() {
    guestctl()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("guestctl"));
}

#[test]
fn guestctl_welcome_without_subcommand() {
    guestctl()
        .assert()
        .success()
        .stdout(predicate::str::contains("Common workflows"))
        .stdout(predicate::str::contains("guestctl inspect"));
}

#[test]
fn guestkit_welcome_uses_guestkit_name() {
    guestkit()
        .assert()
        .success()
        .stdout(predicate::str::contains("guestkit — Guest VM"))
        .stdout(predicate::str::contains("guestkit inspect"));
}

#[test]
fn guestctl_commands_subcommand() {
    guestctl()
        .arg("commands")
        .assert()
        .success()
        .stdout(predicate::str::contains("Inspect & report"))
        .stdout(predicate::str::contains("inspect"));
}

#[test]
fn guestctl_disk_shorthand_preprocess() {
    use guestkit::cli::invocation;

    let args = vec![
        "guestctl".to_string(),
        "/tmp/test-shorthand.qcow2".to_string(),
    ];
    let out = invocation::preprocess_args(args);
    assert_eq!(
        out,
        vec![
            "guestctl".to_string(),
            "inspect".to_string(),
            "/tmp/test-shorthand.qcow2".to_string(),
        ]
    );
}

#[test]
fn guestctl_completion_supports_all_flag() {
    guestctl()
        .arg("completion")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--all"));
}

#[test]
#[cfg(not(debug_assertions))]
fn guestctl_completion_all_generates_both_names() {
    guestctl()
        .arg("completion")
        .arg("bash")
        .arg("--all")
        .assert()
        .success()
        .stdout(predicate::str::contains("guestkit"))
        .stdout(predicate::str::contains("guestctl"));
}
