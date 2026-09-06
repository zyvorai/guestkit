// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rich heartbeat: state machine, payload builder, and push task.
//!
//! Pull: `guestkit.getAgentHealth` returns a fresh [`Heartbeat`].
//! Push: after `guestkit.subscribeEvents`, `run_heartbeat_task` emits
//! `guestkit.event.heartbeat` notifications on subscribed channels.

use crate::agent::state::AgentRuntime;
use guestkit_agent_protocol::capabilities::NOTIFICATION_HEARTBEAT;
use guestkit_agent_protocol::heartbeat::{AgentState, PressureSummary};
use guestkit_agent_protocol::{Heartbeat, JsonRpcNotification, PROTOCOL_VERSION};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use sysinfo::{Disks, System};

pub const DEFAULT_INTERVAL_SECS: u64 = 30;
pub const MIN_INTERVAL_SECS: u64 = 1;

/// Inputs gathered once per tick, shared by payload build + state derivation.
struct Probe {
    cpu_usage_percent: f32,
    memory_usage_percent: f32,
    root_disk_usage_percent: u8,
    pressure: Option<PressureSummary>,
    pending_reboot: bool,
    pending_reboot_reasons: Vec<String>,
    failed_units: Vec<String>,
    system_state: Option<String>,
    virtio_ready: bool,
}

fn shared_system() -> &'static Mutex<System> {
    static SYS: OnceLock<Mutex<System>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(System::new()))
}

fn probe() -> Probe {
    let (cpu, mem) = {
        let mut sys = shared_system().lock().unwrap_or_else(|e| e.into_inner());
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let mem_pct = if sys.total_memory() > 0 {
            (sys.used_memory() as f64 / sys.total_memory() as f64 * 100.0) as f32
        } else {
            0.0
        };
        (sys.global_cpu_usage(), mem_pct)
    };

    let root_mount = if cfg!(windows) { "C:\\" } else { "/" };
    let disks = Disks::new_with_refreshed_list();
    let root_disk = disks
        .iter()
        .find(|d| d.mount_point().to_string_lossy() == root_mount)
        .map(|d| {
            if d.total_space() > 0 {
                (((d.total_space() - d.available_space()) as f64 / d.total_space() as f64) * 100.0)
                    as u8
            } else {
                0
            }
        })
        .unwrap_or(0);

    let (pending_reboot, pending_reboot_reasons) = pending_reboot();
    let (system_state, failed_units) = service_state();

    Probe {
        cpu_usage_percent: cpu,
        memory_usage_percent: mem,
        root_disk_usage_percent: root_disk,
        pressure: read_psi(),
        pending_reboot,
        pending_reboot_reasons,
        failed_units,
        system_state,
        virtio_ready: virtio_ready(),
    }
}

/// PSI avg10 values from /proc/pressure. Linux only; None elsewhere or when
/// the kernel lacks PSI support.
fn read_psi() -> Option<PressureSummary> {
    fn avg10(path: &str) -> Option<f32> {
        let text = std::fs::read_to_string(path).ok()?;
        let line = text.lines().find(|l| l.starts_with("some"))?;
        let field = line.split_whitespace().find(|f| f.starts_with("avg10="))?;
        field.trim_start_matches("avg10=").parse().ok()
    }
    if !cfg!(target_os = "linux") {
        return None;
    }
    Some(PressureSummary {
        cpu: avg10("/proc/pressure/cpu")?,
        memory: avg10("/proc/pressure/memory").unwrap_or(0.0),
        io: avg10("/proc/pressure/io").unwrap_or(0.0),
    })
}

/// PSI accessor shared with the telemetry sampler.
pub fn psi_for_telemetry() -> Option<PressureSummary> {
    read_psi()
}

#[cfg(target_os = "linux")]
fn pending_reboot() -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if std::path::Path::new("/var/run/reboot-required").exists() {
        reasons.push("reboot-required marker present".to_string());
        if let Ok(pkgs) = std::fs::read_to_string("/var/run/reboot-required.pkgs") {
            reasons.extend(
                pkgs.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| format!("package: {}", l.trim())),
            );
        }
    }
    (!reasons.is_empty(), reasons)
}

#[cfg(target_os = "windows")]
fn pending_reboot() -> (bool, Vec<String>) {
    crate::collectors::windows_live::collect_pending_reboot()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn pending_reboot() -> (bool, Vec<String>) {
    (false, Vec::new())
}

/// (systemd system state, failed unit names). Cheap: two short-lived
/// `systemctl` invocations per tick; both absent → (None, []).
#[cfg(target_os = "linux")]
fn service_state() -> (Option<String>, Vec<String>) {
    use std::process::Command;
    let state = Command::new("systemctl")
        .arg("is-system-running")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let failed = Command::new("systemctl")
        .args(["--failed", "--plain", "--no-legend", "--no-pager"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split_whitespace().next().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (state, failed)
}

#[cfg(target_os = "windows")]
fn service_state() -> (Option<String>, Vec<String>) {
    (
        None,
        crate::collectors::windows_live::failed_auto_services(),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn service_state() -> (Option<String>, Vec<String>) {
    (None, Vec::new())
}

#[cfg(target_os = "linux")]
fn virtio_ready() -> bool {
    std::path::Path::new("/sys/bus/virtio").exists()
}

#[cfg(not(target_os = "linux"))]
fn virtio_ready() -> bool {
    // Windows: boot-critical virtio state comes from migration assessment;
    // for the cheap heartbeat predicate assume the running system is fine.
    true
}

fn boot_id() -> String {
    #[cfg(target_os = "linux")]
    if let Ok(id) = std::fs::read_to_string("/proc/sys/kernel/random/boot_id") {
        return id.trim().to_string();
    }
    // Synthetic stable per-boot id: hash of boot time + hostname.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(System::boot_time().to_le_bytes());
    hasher.update(System::host_name().unwrap_or_default());
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

fn compute_state(rt: &AgentRuntime, p: &Probe) -> AgentState {
    if rt.updating.load(Ordering::Relaxed) {
        return AgentState::Updating;
    }
    if rt.fs_frozen() {
        return AgentState::Quiesced;
    }
    if matches!(
        p.system_state.as_deref(),
        Some("maintenance") | Some("emergency") | Some("rescue")
    ) {
        return AgentState::RecoveryMode;
    }
    let degraded = !p.failed_units.is_empty()
        || p.root_disk_usage_percent > 95
        || p.pressure.map(|ps| ps.memory > 40.0).unwrap_or(false);
    if degraded {
        return AgentState::Degraded;
    }
    AgentState::Healthy
}

/// Build a fresh heartbeat and record it on the runtime.
pub fn build_heartbeat(rt: &AgentRuntime) -> Heartbeat {
    let p = probe();
    let state = compute_state(rt, &p);
    rt.set_state(state);
    let fs_frozen = rt.fs_frozen();
    let hb = Heartbeat {
        seq: rt.next_heartbeat_seq(),
        agent_state: state,
        boot_id: boot_id(),
        os_uptime_secs: System::uptime(),
        agent_uptime_secs: rt.started_at.elapsed().as_secs(),
        agent_version: crate::VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION.to_string(),
        cpu_usage_percent: p.cpu_usage_percent,
        memory_usage_percent: p.memory_usage_percent,
        root_disk_usage_percent: p.root_disk_usage_percent,
        pressure: p.pressure,
        pending_reboot: p.pending_reboot,
        pending_reboot_reasons: p.pending_reboot_reasons,
        critical_services_failed: p.failed_units,
        migration_ready: !fs_frozen && p.virtio_ready,
        fs_frozen,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    rt.store_heartbeat(hb.clone());
    hb
}

/// Periodic push loop. Emits heartbeat notifications on every registered,
/// subscribed channel; a write failure unsubscribes that channel.
pub async fn run_heartbeat_task(rt: Arc<AgentRuntime>, interval: Duration) {
    let interval = interval.max(Duration::from_secs(MIN_INTERVAL_SECS));
    loop {
        tokio::time::sleep(interval).await;
        let subscribed: Vec<_> = rt
            .channels()
            .into_iter()
            .filter(|c| c.subscribed.load(Ordering::Relaxed))
            .collect();
        // Build even with no subscribers: keeps the cached heartbeat warm
        // for getAgentHealth and the Zeus HTTPS push.
        let rt2 = Arc::clone(&rt);
        let hb = tokio::task::spawn_blocking(move || build_heartbeat(&rt2))
            .await
            .ok();
        let Some(hb) = hb else { continue };
        if subscribed.is_empty() {
            continue;
        }
        let params = match serde_json::to_value(&hb) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("heartbeat serialize: {e}");
                continue;
            }
        };
        let note = JsonRpcNotification::new(NOTIFICATION_HEARTBEAT, params);
        for chan in subscribed {
            if let Err(e) = chan.writer.send_notification(&note) {
                log::warn!("heartbeat push to {} failed: {e}; unsubscribing", chan.name);
                chan.subscribed.store(false, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_probe() -> Probe {
        Probe {
            cpu_usage_percent: 0.0,
            memory_usage_percent: 0.0,
            root_disk_usage_percent: 0,
            pressure: None,
            pending_reboot: false,
            pending_reboot_reasons: vec![],
            failed_units: vec![],
            system_state: Some("running".into()),
            virtio_ready: true,
        }
    }

    #[test]
    fn healthy_by_default() {
        let rt = AgentRuntime::default();
        assert_eq!(compute_state(&rt, &empty_probe()), AgentState::Healthy);
    }

    #[test]
    fn failed_units_degrade() {
        let rt = AgentRuntime::default();
        let mut p = empty_probe();
        p.failed_units = vec!["nginx.service".into()];
        assert_eq!(compute_state(&rt, &p), AgentState::Degraded);
    }

    #[test]
    fn full_disk_degrades() {
        let rt = AgentRuntime::default();
        let mut p = empty_probe();
        p.root_disk_usage_percent = 97;
        assert_eq!(compute_state(&rt, &p), AgentState::Degraded);
    }

    #[test]
    fn updating_wins_over_degraded() {
        let rt = AgentRuntime::default();
        rt.updating.store(true, Ordering::Relaxed);
        let mut p = empty_probe();
        p.failed_units = vec!["x.service".into()];
        assert_eq!(compute_state(&rt, &p), AgentState::Updating);
    }

    #[test]
    fn frozen_is_quiesced() {
        let rt = AgentRuntime::default();
        rt.fs_frozen_hint.store(true, Ordering::Relaxed);
        assert_eq!(compute_state(&rt, &empty_probe()), AgentState::Quiesced);
    }

    #[test]
    fn emergency_is_recovery_mode() {
        let rt = AgentRuntime::default();
        let mut p = empty_probe();
        p.system_state = Some("emergency".into());
        assert_eq!(compute_state(&rt, &p), AgentState::RecoveryMode);
    }

    #[test]
    fn build_heartbeat_populates_and_caches() {
        let rt = AgentRuntime::default();
        let hb = build_heartbeat(&rt);
        assert!(!hb.boot_id.is_empty());
        assert_eq!(hb.protocol_version, PROTOCOL_VERSION);
        assert_eq!(rt.last_heartbeat().unwrap().seq, hb.seq);
        let hb2 = build_heartbeat(&rt);
        assert_eq!(hb2.seq, hb.seq + 1);
    }
}
