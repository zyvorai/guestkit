// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Orchestrated application-consistent snapshots.
//!
//! Flow (spec §7): discover applications → quiesce plugins → freeze
//! filesystems (Linux) or create a VSS marker shadow (Windows) → host
//! snapshots → complete: thaw/cleanup → resume plugins → consistency
//! report. A watchdog always thaws + resumes if the host never calls
//! complete.

pub mod plugins;
pub mod vss;

use plugins::{builtin_plugins, PluginReport};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

pub const DEFAULT_SNAPSHOT_WATCHDOG_SECS: u64 = 120;
pub const MAX_SNAPSHOT_WATCHDOG_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPrepareReport {
    pub snapshot_id: String,
    pub prepared_at: String,
    /// "frozen" (Linux fsfreeze), "vss" (Windows marker shadow), or
    /// "flush_only" (no freeze mechanism available).
    pub mechanism: String,
    pub fs_frozen: bool,
    pub app_consistent: bool,
    pub plugins: Vec<PluginReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vss: Option<vss::VssSnapshotResult>,
    pub watchdog_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCompleteReport {
    pub snapshot_id: String,
    pub completed_at: String,
    pub thawed: bool,
    pub plugins_resumed: Vec<PluginReport>,
    /// Final verdict for the backup catalog.
    pub consistency: String, // "application" | "filesystem" | "crash"
}

/// Active prepare state so complete/watchdog know what to unwind.
struct ActiveSnapshot {
    snapshot_id: String,
    frozen: bool,
    vss_shadow: Option<String>,
    quiesced_plugins: Vec<&'static str>,
    app_consistent: bool,
}

static ACTIVE: Mutex<Option<ActiveSnapshot>> = Mutex::new(None);

fn state_lock() -> std::sync::MutexGuard<'static, Option<ActiveSnapshot>> {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Quiesce applications and freeze. Idempotent guard: a second prepare
/// while one is active fails cleanly.
pub fn prepare(watchdog_secs: Option<u64>) -> anyhow::Result<SnapshotPrepareReport> {
    {
        let active = state_lock();
        if active.is_some() {
            anyhow::bail!("a snapshot prepare is already active — call snapshot.complete first");
        }
    }
    let watchdog_secs = watchdog_secs
        .unwrap_or(DEFAULT_SNAPSHOT_WATCHDOG_SECS)
        .clamp(10, MAX_SNAPSHOT_WATCHDOG_SECS);
    let snapshot_id = format!("snap-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));

    // 1. Application plugins.
    let mut reports = Vec::new();
    let mut quiesced = Vec::new();
    let mut all_apps_ok = true;
    for plugin in builtin_plugins() {
        let discovered = plugin.discover();
        let mut report = PluginReport {
            plugin: plugin.name().to_string(),
            discovered,
            quiesced: false,
            resumed: false,
            detail: None,
        };
        if discovered {
            match plugin.quiesce() {
                Ok(detail) => {
                    report.quiesced = true;
                    report.detail = Some(detail);
                    quiesced.push(plugin.name());
                }
                Err(e) => {
                    all_apps_ok = false;
                    report.detail = Some(format!("quiesce failed: {e}"));
                    log::warn!("snapshot plugin {} quiesce failed: {e}", plugin.name());
                }
            }
        }
        reports.push(report);
    }

    // 2. Platform freeze.
    let (mechanism, fs_frozen, vss_result) = if cfg!(target_os = "windows") {
        match vss::create_marker_shadow("C:") {
            Ok(result) => {
                let consistent = result.app_consistent;
                ("vss".to_string(), false, Some((result, consistent)))
            }
            Err(e) => {
                log::warn!("VSS marker shadow failed: {e}");
                (
                    "flush_only".to_string(),
                    false,
                    Some((
                        vss::VssSnapshotResult {
                            created: false,
                            detail: Some(e.to_string()),
                            ..Default::default()
                        },
                        false,
                    )),
                )
            }
        }
    } else {
        match crate::agent::snapshot_hooks::freeze_filesystems() {
            Ok(_) => ("frozen".to_string(), true, None),
            Err(e) => {
                log::warn!("fsfreeze failed: {e}; snapshot will be flush-only");
                ("flush_only".to_string(), false, None)
            }
        }
    };
    let (vss_report, vss_consistent) = match vss_result {
        Some((r, c)) => (Some(r), c),
        None => (None, false),
    };

    let app_consistent = all_apps_ok
        && (fs_frozen || vss_consistent)
        // No discovered apps + frozen fs is still app-consistent (nothing
        // to quiesce).
        ;

    {
        let mut active = state_lock();
        *active = Some(ActiveSnapshot {
            snapshot_id: snapshot_id.clone(),
            frozen: fs_frozen,
            vss_shadow: vss_report.as_ref().and_then(|v| v.shadow_id.clone()),
            quiesced_plugins: quiesced,
            app_consistent,
        });
    }

    // 3. Watchdog: unwind everything if the host never completes.
    let watchdog_id = snapshot_id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(watchdog_secs));
        let still_active = {
            let active = state_lock();
            active
                .as_ref()
                .map(|a| a.snapshot_id == watchdog_id)
                .unwrap_or(false)
        };
        if still_active {
            log::warn!("snapshot watchdog fired for {watchdog_id} — auto-completing");
            let _ = complete();
        }
    });

    crate::agent::state::AgentRuntime::global()
        .fs_frozen_hint
        .store(fs_frozen, Ordering::Relaxed);

    Ok(SnapshotPrepareReport {
        snapshot_id,
        prepared_at: chrono::Utc::now().to_rfc3339(),
        mechanism,
        fs_frozen,
        app_consistent,
        plugins: reports,
        vss: vss_report,
        watchdog_secs,
    })
}

/// Thaw, clean up the VSS marker, resume plugins, and report consistency.
pub fn complete() -> anyhow::Result<SnapshotCompleteReport> {
    let active = {
        let mut guard = state_lock();
        guard
            .take()
            .ok_or_else(|| anyhow::anyhow!("no active snapshot prepare"))?
    };

    let mut thawed = false;
    if active.frozen {
        match crate::agent::snapshot_hooks::thaw_filesystems() {
            Ok(_) => thawed = true,
            Err(e) => log::error!("thaw failed: {e}"),
        }
    }
    if let Some(shadow_id) = &active.vss_shadow {
        match vss::delete_marker_shadow(shadow_id) {
            Ok(msg) => log::info!("{msg}"),
            Err(e) => log::warn!("VSS marker cleanup: {e}"),
        }
    }

    let mut resumed = Vec::new();
    for plugin in builtin_plugins() {
        if !active.quiesced_plugins.contains(&plugin.name()) {
            continue;
        }
        let mut report = PluginReport {
            plugin: plugin.name().to_string(),
            discovered: true,
            quiesced: true,
            resumed: false,
            detail: None,
        };
        match plugin.resume() {
            Ok(detail) => {
                report.resumed = true;
                report.detail = Some(detail);
            }
            Err(e) => {
                report.detail = Some(format!("resume failed: {e}"));
                log::error!("snapshot plugin {} resume failed: {e}", plugin.name());
            }
        }
        resumed.push(report);
    }

    crate::agent::state::AgentRuntime::global()
        .fs_frozen_hint
        .store(false, Ordering::Relaxed);

    let consistency = if active.app_consistent {
        "application"
    } else if active.frozen || thawed {
        "filesystem"
    } else {
        "crash"
    };

    Ok(SnapshotCompleteReport {
        snapshot_id: active.snapshot_id,
        completed_at: chrono::Utc::now().to_rfc3339(),
        thawed,
        plugins_resumed: resumed,
        consistency: consistency.to_string(),
    })
}

/// True when a prepare is outstanding (heartbeat surfaces this as quiesced).
pub fn snapshot_active() -> bool {
    state_lock().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_without_prepare_fails() {
        // Ensure clean state even if another test left residue.
        let _ = complete();
        let err = complete().unwrap_err();
        assert!(err.to_string().contains("no active"));
    }

    #[test]
    fn double_prepare_guard() {
        let _ = complete();
        // No root: freeze will fail → flush_only, but state machine still
        // guards double-prepare.
        let first = prepare(Some(10));
        if let Ok(report) = first {
            assert!(!report.snapshot_id.is_empty());
            let second = prepare(Some(10));
            assert!(second.is_err());
            let done = complete().unwrap();
            assert_eq!(done.snapshot_id, report.snapshot_id);
        }
    }
}
