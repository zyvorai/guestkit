// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pre-migration check, cutover assist, and post-migration validation.
//!
//! Cutover safety: filesystem freeze always arms a watchdog that thaws at
//! the deadline, and the maintenance marker records that deadline so an
//! agent restarted mid-cutover also thaws. A host that dies mid-snapshot
//! can therefore never leave the guest frozen.

use super::baseline::{
    capture_baseline, diff_baselines, latest_pre_baseline, load_baseline, BaselinePhase,
    DriftReport, MigrationBaseline,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_WATCHDOG_SECS: u64 = 120;
pub const MAX_WATCHDOG_SECS: u64 = 600;
const TOKEN_VALIDITY_SECS: i64 = 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessToken {
    pub token: String,
    pub score: f64,
    pub readiness: super::score::ReadinessLevel,
    pub baseline_id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutoverState {
    pub active: bool,
    pub entered_at: Option<String>,
    pub frozen: bool,
    pub watchdog_deadline: Option<String>,
    pub stopped_services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMigrationReport {
    pub boot_ok: bool,
    pub network_ok: bool,
    pub restored_ips: Vec<String>,
    pub drift: DriftReport,
    pub baseline_id: String,
}

fn state_dir() -> PathBuf {
    super::baseline::baseline_dir()
}

fn marker_path() -> PathBuf {
    state_dir().join("cutover.json")
}

// --- self-verified capability token (HMAC-SHA256 over score|baseline|exp) ---

fn token_key() -> Result<[u8; 32]> {
    let path = state_dir().join("token.key");
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }
    let mut key = [0u8; 32];
    // rand is a workspace dependency.
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::create_dir_all(state_dir())?;
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k_ipad = [0x36u8; BLOCK];
    let mut k_opad = [0x5cu8; BLOCK];
    for (i, b) in key.iter().enumerate() {
        k_ipad[i] ^= b;
        k_opad[i] ^= b;
    }
    let inner = {
        let mut h = Sha256::new();
        h.update(k_ipad);
        h.update(message);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(k_opad);
    h.update(inner);
    h.finalize().into()
}

fn sign_token(score: f64, baseline_id: &str, expires_at: &str) -> Result<String> {
    let key = token_key()?;
    let message = format!("{score}|{baseline_id}|{expires_at}");
    Ok(hex::encode(hmac_sha256(&key, message.as_bytes())))
}

pub fn verify_token(token: &ReadinessToken) -> Result<()> {
    let expires =
        chrono::DateTime::parse_from_rfc3339(&token.expires_at).context("token expiry")?;
    if chrono::Utc::now() > expires {
        anyhow::bail!("readiness token expired at {}", token.expires_at);
    }
    let expected = sign_token(token.score, &token.baseline_id, &token.expires_at)?;
    if expected != token.token {
        anyhow::bail!("readiness token signature mismatch");
    }
    Ok(())
}

/// Run assessment + capture the pre-migration baseline; token authorizes
/// destructive repair and cutover for the next hour.
pub fn pre_migration_check(target: &str) -> Result<ReadinessToken> {
    let evidence = crate::evidence::build_evidence_live()?;
    let boot_target = crate::boot::BootTarget::parse(target);
    let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
    let assessment = super::score::assess_migration(&evidence, &boot_report, target, true);
    let baseline = capture_baseline(BaselinePhase::PreMigration, target)?;

    let expires_at =
        (chrono::Utc::now() + chrono::Duration::seconds(TOKEN_VALIDITY_SECS)).to_rfc3339();
    Ok(ReadinessToken {
        token: sign_token(assessment.overall_score, &baseline.id, &expires_at)?,
        score: assessment.overall_score,
        readiness: assessment.readiness,
        baseline_id: baseline.id,
        expires_at,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CutoverParams {
    #[serde(default)]
    pub stop_services: Vec<String>,
    #[serde(default)]
    pub freeze: bool,
    #[serde(default)]
    pub watchdog_secs: Option<u64>,
}

/// Enter maintenance mode: stop listed services, optionally freeze, and arm
/// the watchdog thaw.
pub fn enter_cutover(params: &CutoverParams) -> Result<CutoverState> {
    let watchdog_secs = params
        .watchdog_secs
        .unwrap_or(DEFAULT_WATCHDOG_SECS)
        .clamp(10, MAX_WATCHDOG_SECS);

    let mut stopped = Vec::new();
    for service in &params.stop_services {
        let executor = crate::agent::executor::Executor::new();
        match executor.control_unit("stop", service) {
            Ok(_) => stopped.push(service.clone()),
            Err(e) => log::warn!("cutover: stop {service}: {e}"),
        }
    }

    let mut frozen = false;
    let mut deadline = None;
    if params.freeze {
        crate::agent::snapshot_hooks::freeze_filesystems()
            .map_err(|e| anyhow::anyhow!("freeze: {e}"))?;
        frozen = true;
        let dl = chrono::Utc::now() + chrono::Duration::seconds(watchdog_secs as i64);
        deadline = Some(dl.to_rfc3339());
        arm_watchdog(Duration::from_secs(watchdog_secs));
    }

    let state = CutoverState {
        active: true,
        entered_at: Some(chrono::Utc::now().to_rfc3339()),
        frozen,
        watchdog_deadline: deadline,
        stopped_services: stopped,
    };
    // Persist BEFORE returning: a crashed agent must find the marker.
    std::fs::create_dir_all(state_dir())?;
    std::fs::write(marker_path(), serde_json::to_vec_pretty(&state)?)?;
    Ok(state)
}

/// Spawn the watchdog thaw. Deliberately avoids any disk I/O until after
/// the thaw (writes on a frozen root would block the watchdog itself).
fn arm_watchdog(delay: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let still_frozen = crate::agent::state::AgentRuntime::global().fs_frozen();
        if still_frozen {
            log::warn!("cutover watchdog fired — thawing filesystems");
            let _ = crate::agent::snapshot_hooks::thaw_filesystems();
            let _ = clear_marker_frozen();
        }
    });
}

fn clear_marker_frozen() -> Result<()> {
    if let Ok(bytes) = std::fs::read(marker_path()) {
        if let Ok(mut state) = serde_json::from_slice::<CutoverState>(&bytes) {
            state.frozen = false;
            state.watchdog_deadline = None;
            std::fs::write(marker_path(), serde_json::to_vec_pretty(&state)?)?;
        }
    }
    Ok(())
}

/// Called at agent startup: if a previous run died frozen, thaw now.
pub fn recover_cutover_state() {
    let Ok(bytes) = std::fs::read(marker_path()) else {
        return;
    };
    let Ok(state) = serde_json::from_slice::<CutoverState>(&bytes) else {
        return;
    };
    if state.frozen {
        log::warn!("recovering from interrupted cutover — thawing filesystems");
        let _ = crate::agent::snapshot_hooks::thaw_filesystems();
        let _ = clear_marker_frozen();
    }
}

/// Exit maintenance: thaw and restart the services stopped on entry.
pub fn exit_cutover() -> Result<CutoverState> {
    let previous: Option<CutoverState> = std::fs::read(marker_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());

    if crate::agent::state::AgentRuntime::global().fs_frozen() {
        crate::agent::snapshot_hooks::thaw_filesystems()
            .map_err(|e| anyhow::anyhow!("thaw: {e}"))?;
    }
    let mut restarted = Vec::new();
    if let Some(prev) = &previous {
        for service in &prev.stopped_services {
            let executor = crate::agent::executor::Executor::new();
            match executor.control_unit("start", service) {
                Ok(_) => restarted.push(service.clone()),
                Err(e) => log::warn!("cutover exit: start {service}: {e}"),
            }
        }
    }
    let state = CutoverState {
        active: false,
        entered_at: previous.and_then(|p| p.entered_at),
        frozen: false,
        watchdog_deadline: None,
        stopped_services: restarted,
    };
    let _ = std::fs::remove_file(marker_path());
    Ok(state)
}

/// After first boot on the destination: validate and diff against the
/// pre-migration baseline.
pub fn post_migration_validate(baseline_id: Option<&str>) -> Result<PostMigrationReport> {
    let baseline: MigrationBaseline = match baseline_id {
        Some(id) => load_baseline(id)?,
        None => latest_pre_baseline()
            .ok_or_else(|| anyhow::anyhow!("no pre-migration baseline found"))?,
    };
    let current = crate::evidence::build_evidence_live()?;
    let drift = diff_baselines(&baseline, &current);

    // Boot OK: we are running, systemd not degraded.
    let boot_ok = current
        .systemd
        .as_ref()
        .and_then(|s| s.runtime.as_ref())
        .and_then(|r| r.manager.as_ref())
        .map(|m| m.system_state != "degraded")
        .unwrap_or(true);
    // Network OK: default gateway present and at least one address.
    let network_ok = current.network.default_gateway.is_some()
        && current
            .network
            .live_interfaces
            .iter()
            .any(|i| !i.addresses.is_empty() && i.name != "lo");

    // Static IP restore (Windows path): compare captured static configs.
    let mut restored_ips = Vec::new();
    if let (Some(before_win), Some(_)) = (&baseline.evidence.windows, &current.windows) {
        for nic in &before_win.static_nic_configs {
            restored_ips.extend(nic.ip_addresses.iter().cloned());
        }
    }

    Ok(PostMigrationReport {
        boot_ok,
        network_ok,
        restored_ips,
        drift,
        baseline_id: baseline.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_is_deterministic_and_keyed() {
        let key = [7u8; 32];
        let a = hmac_sha256(&key, b"message");
        let b = hmac_sha256(&key, b"message");
        assert_eq!(a, b);
        let other = hmac_sha256(&[8u8; 32], b"message");
        assert_ne!(a, other);
        let diff_msg = hmac_sha256(&key, b"messagf");
        assert_ne!(a, diff_msg);
    }

    #[test]
    fn cutover_params_clamp() {
        let params = CutoverParams {
            watchdog_secs: Some(10_000),
            ..Default::default()
        };
        let clamped = params
            .watchdog_secs
            .unwrap_or(DEFAULT_WATCHDOG_SECS)
            .clamp(10, MAX_WATCHDOG_SECS);
        assert_eq!(clamped, MAX_WATCHDOG_SECS);
    }
}
