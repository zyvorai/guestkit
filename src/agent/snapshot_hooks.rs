// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Snapshot hooks and readiness enrichment.

use crate::evidence::collectors::snapshot_live::collect_snapshot_readiness_live;
use guestkit_agent_protocol::{HookResult, SnapshotReadinessReport};
use std::process::Command;

pub fn build_snapshot_readiness_report() -> SnapshotReadinessReport {
    let base = collect_snapshot_readiness_live();
    let hook_results = run_pre_snapshot_hooks();
    let ready = base.quiesce_supported
        && base.guest_agent_connected
        && hook_results.iter().all(|h| h.success);

    SnapshotReadinessReport {
        ready,
        fs_frozen: base.fs_frozen,
        quiesce_supported: base.quiesce_supported,
        guest_agent_connected: base.guest_agent_connected,
        notes: base.notes,
        hook_results,
    }
}

pub fn freeze_filesystems() -> Result<String, String> {
    let result = if Command::new("fsfreeze").args(["-f", "/"]).status().is_ok() {
        Ok("filesystems frozen".into())
    } else if crate::agent::qga::freeze_fs().is_ok() {
        Ok("filesystems frozen via QGA".into())
    } else {
        Err("freeze failed".into())
    };
    if result.is_ok() {
        crate::agent::state::AgentRuntime::global()
            .fs_frozen_hint
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    result
}

pub fn thaw_filesystems() -> Result<String, String> {
    let result = if Command::new("fsfreeze").args(["-u", "/"]).status().is_ok() {
        Ok("filesystems thawed".into())
    } else if crate::agent::qga::thaw_fs().is_ok() {
        Ok("filesystems thawed via QGA".into())
    } else {
        Err("thaw failed".into())
    };
    if result.is_ok() {
        crate::agent::state::AgentRuntime::global()
            .fs_frozen_hint
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
    result
}

fn run_pre_snapshot_hooks() -> Vec<HookResult> {
    let hook_dir = "/etc/zyvor/hooks/pre-snapshot";
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(hook_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sh") {
                let output = Command::new("sh").arg(&path).output();
                let (success, message) = match output {
                    Ok(o) => (
                        o.status.success(),
                        String::from_utf8_lossy(&o.stdout).trim().to_string(),
                    ),
                    Err(e) => (false, e.to_string()),
                };
                results.push(HookResult {
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    success,
                    message,
                });
            }
        }
    }
    results
}
