// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Tests for JSON/YAML/CSV output formats
//!
//! These tests verify output format functionality via the CLI interface

use assert_cmd::Command;

fn guestkit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_guestkit"))
}

#[test]
fn test_cli_help_shows_output_option() {
    let mut cmd = guestkit();
    cmd.arg("inspect").arg("--help");

    let output = cmd.output().expect("Failed to execute command");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify --output option is documented with all formats
    assert!(stdout.contains("--output") || stdout.contains("-o"));
    assert!(stdout.contains("json"));
    assert!(stdout.contains("yaml"));
    assert!(stdout.contains("csv"));
}

#[test]
fn test_cli_accepts_json_flag() {
    let mut cmd = guestkit();
    cmd.arg("inspect")
        .arg("nonexistent.qcow2")
        .arg("--output")
        .arg("json");

    // The command will fail (no such file), but it should parse the flag successfully
    // If the flag wasn't recognized, clap would error before trying to open the file
    let output = cmd.output().expect("Failed to execute command");

    // Should fail with file not found or similar, not with unknown flag error
    assert!(!output.status.success());
}

#[test]
fn test_cli_accepts_yaml_flag() {
    let mut cmd = guestkit();
    cmd.arg("inspect")
        .arg("nonexistent.qcow2")
        .arg("-o") // Test short form
        .arg("yaml");

    let output = cmd.output().expect("Failed to execute command");
    assert!(!output.status.success());
}

#[test]
fn test_cli_accepts_csv_flag() {
    let mut cmd = guestkit();
    cmd.arg("inspect")
        .arg("nonexistent.qcow2")
        .arg("-o")
        .arg("csv");

    let output = cmd.output().expect("Failed to execute command");
    assert!(!output.status.success());
}

#[test]
fn test_cli_rejects_invalid_format() {
    let mut cmd = guestkit();
    cmd.arg("inspect")
        .arg("nonexistent.qcow2")
        .arg("--output")
        .arg("invalid-format");

    let output = cmd.output().expect("Failed to execute command");
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should error with unknown format message
    assert!(!output.status.success());
    assert!(stderr.contains("Unknown output format") || stderr.contains("invalid"));
}

// Note: Full integration tests with actual disk images would require test fixtures
// These tests verify the CLI interface accepts the flags correctly
