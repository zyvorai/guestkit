// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for offline Windows registry writes via libhivex FFI.
//!
//! Requires a seed hive in libhivex's writable format. Point `GK_TEST_HIVE`
//! at one (e.g. libhivex's `images/minimal`); the test copies it, mutates the
//! copy, and reads the values back with the `hivexget` CLI. Skips when unset,
//! so it is a no-op in environments without a fixture.
#![cfg(feature = "registry-write")]

use std::path::PathBuf;
use std::process::Command;

use guestkit::guestfs::hivex_ffi::set_registry_value;
use serde_json::json;

fn seed_hive() -> Option<PathBuf> {
    std::env::var("GK_TEST_HIVE").ok().map(PathBuf::from)
}

fn hivexget(hive: &std::path::Path, key: &str, value: &str) -> String {
    let out = Command::new("hivexget")
        .arg(hive)
        .arg(key)
        .arg(value)
        .output()
        .expect("hivexget must be installed (libhivex-bin)");
    assert!(
        out.status.success(),
        "hivexget {key} {value} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn writes_values_into_a_new_subkey_and_reads_them_back() {
    let Some(seed) = seed_hive() else {
        eprintln!("skipping registry_write: GK_TEST_HIVE not set");
        return;
    };

    // Never mutate the fixture itself.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::copy(&seed, tmp.path()).unwrap();
    let hive = tmp.path();

    // Creating "GuestkitTest" exercises hivex_node_add_child (key does not exist
    // in the minimal fixture), plus set_value + commit across several types.
    set_registry_value(
        hive,
        &["GuestkitTest".into()],
        "Mode",
        "REG_SZ",
        &json!("Enabled"),
    )
    .unwrap();
    set_registry_value(
        hive,
        &["GuestkitTest".into()],
        "Level",
        "REG_DWORD",
        &json!(7),
    )
    .unwrap();
    set_registry_value(
        hive,
        &["GuestkitTest".into(), "Sub".into()],
        "Path",
        "REG_EXPAND_SZ",
        &json!("%SystemRoot%\\System32"),
    )
    .unwrap();

    // Read the persisted bytes back with an independent tool.
    assert_eq!(hivexget(hive, "\\GuestkitTest", "Mode"), "Enabled");

    let level = hivexget(hive, "\\GuestkitTest", "Level");
    assert!(
        level.contains('7'),
        "expected DWORD 7 in read-back, got {level:?}"
    );

    let path = hivexget(hive, "\\GuestkitTest\\Sub", "Path");
    assert!(
        path.contains("SystemRoot"),
        "expected expand-string round-trip, got {path:?}"
    );
}
