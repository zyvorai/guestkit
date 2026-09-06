// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Lightweight tamper / integrity monitoring (spec §19).
//!
//! Baseline-and-diff, not a full EDR (per the spec: "provide high-value VM
//! lifecycle and migration security events first; avoid turning GuestKit
//! into a full EDR initially"). `baseline` snapshots the security-sensitive
//! surface; `check` diffs the current state and reports categorized changes
//! with severity. The baseline file is integrity-hashed so tampering with
//! it is itself detected.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Directories scanned for SUID/SGID binaries — the escalation surface.
/// The whole-filesystem scan is deliberately avoided for speed/determinism.
const BIN_DIRS: &[&str] = &[
    "/usr/bin",
    "/usr/sbin",
    "/bin",
    "/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
];

const SENSITIVE_WORLD_WRITABLE: &[&str] = &["/etc", "/etc/cron.d", "/etc/sudoers.d"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrityBaseline {
    pub captured_at: String,
    pub boot_id: String,
    /// path -> sha256 for SUID/SGID binaries.
    pub suid_sgid: BTreeMap<String, String>,
    /// "port/proto" listeners with owning process.
    pub listeners: BTreeMap<String, String>,
    pub kernel_modules: Vec<String>,
    /// user -> sha256 of authorized_keys content.
    pub authorized_keys: BTreeMap<String, String>,
    pub sudoers: Vec<String>,
    pub cron_jobs: Vec<String>,
    pub systemd_timers: Vec<String>,
    pub world_writable: Vec<String>,
    /// Integrity digest over the fields above (set when persisted).
    #[serde(default)]
    pub integrity_sha256: String,
}

pub fn baseline_path() -> PathBuf {
    if let Ok(dir) = std::env::var("GUESTKIT_STATE_DIR") {
        return PathBuf::from(dir).join("integrity-baseline.json");
    }
    if cfg!(windows) {
        PathBuf::from("C:\\ProgramData\\GuestKit\\integrity-baseline.json")
    } else {
        PathBuf::from("/var/lib/guestkit/integrity-baseline.json")
    }
}

fn sha256_file(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Some(hex(&h.finalize()))
}

fn sha256_str(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(target_os = "linux")]
fn scan_suid_sgid() -> BTreeMap<String, String> {
    use std::os::unix::fs::PermissionsExt;
    let mut out = BTreeMap::new();
    for dir in BIN_DIRS {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let mode = meta.permissions().mode();
            // setuid (0o4000) or setgid (0o2000).
            if mode & 0o6000 != 0 {
                if let Some(digest) = sha256_file(&path) {
                    out.insert(path.display().to_string(), digest);
                }
            }
        }
    }
    out
}

#[cfg(not(target_os = "linux"))]
fn scan_suid_sgid() -> BTreeMap<String, String> {
    BTreeMap::new()
}

fn scan_listeners() -> BTreeMap<String, String> {
    let intel = crate::agent::netintel::collect();
    intel
        .listeners
        .into_iter()
        .map(|l| {
            (
                format!("{}/{}", l.local_port, l.proto),
                if l.process.is_empty() {
                    format!("pid-{}", l.pid)
                } else {
                    l.process
                },
            )
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn scan_kernel_modules() -> Vec<String> {
    std::fs::read_to_string("/proc/modules")
        .map(|c| {
            let mut v: Vec<String> = c
                .lines()
                .filter_map(|l| l.split_whitespace().next().map(str::to_string))
                .collect();
            v.sort();
            v
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn scan_kernel_modules() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn scan_authorized_keys() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    for line in passwd.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 {
            continue;
        }
        let uid: u32 = f[2].parse().unwrap_or(0);
        if uid != 0 && uid < 1000 {
            continue; // system accounts
        }
        let ak = format!("{}/.ssh/authorized_keys", f[5]);
        if let Ok(content) = std::fs::read_to_string(&ak) {
            out.insert(f[0].to_string(), sha256_str(&content));
        }
    }
    out
}

#[cfg(not(target_os = "linux"))]
fn scan_authorized_keys() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[cfg(target_os = "linux")]
fn scan_sudoers() -> Vec<String> {
    let group = std::fs::read_to_string("/etc/group").unwrap_or_default();
    let mut members: Vec<String> = group
        .lines()
        .filter(|l| l.starts_with("sudo:") || l.starts_with("wheel:"))
        .filter_map(|l| l.split(':').nth(3))
        .flat_map(|m| m.split(',').filter(|x| !x.is_empty()).map(str::to_string))
        .collect();
    // Also NOPASSWD entries in /etc/sudoers.d.
    if let Ok(entries) = std::fs::read_dir("/etc/sudoers.d") {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for line in content.lines() {
                    let t = line.trim();
                    if !t.starts_with('#') && !t.is_empty() && !t.starts_with("Defaults") {
                        members.push(format!("sudoers.d: {t}"));
                    }
                }
            }
        }
    }
    members.sort();
    members.dedup();
    members
}

#[cfg(not(target_os = "linux"))]
fn scan_sudoers() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn scan_cron() -> Vec<String> {
    let mut jobs = Vec::new();
    for dir in ["/etc/cron.d", "/var/spool/cron/crontabs", "/var/spool/cron"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                jobs.push(entry.path().display().to_string());
            }
        }
    }
    jobs.sort();
    jobs
}

#[cfg(not(target_os = "linux"))]
fn scan_cron() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn scan_timers() -> Vec<String> {
    use std::process::Command;
    Command::new("systemctl")
        .args([
            "list-timers",
            "--all",
            "--no-legend",
            "--no-pager",
            "--plain",
        ])
        .output()
        .ok()
        .map(|o| {
            let mut v: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split_whitespace().last().map(str::to_string))
                .filter(|s| s.ends_with(".timer"))
                .collect();
            v.sort();
            v.dedup();
            v
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn scan_timers() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn scan_world_writable() -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;
    let mut out = Vec::new();
    for dir in SENSITIVE_WORLD_WRITABLE {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                // world-writable and not sticky.
                if meta.permissions().mode() & 0o002 != 0 && meta.permissions().mode() & 0o1000 == 0
                {
                    out.push(entry.path().display().to_string());
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(not(target_os = "linux"))]
fn scan_world_writable() -> Vec<String> {
    Vec::new()
}

/// Snapshot the current security-sensitive surface.
pub fn capture() -> IntegrityBaseline {
    let boot_id = crate::agent::state::AgentRuntime::global()
        .last_heartbeat()
        .map(|h| h.boot_id)
        .unwrap_or_default();
    let mut b = IntegrityBaseline {
        captured_at: chrono::Utc::now().to_rfc3339(),
        boot_id,
        suid_sgid: scan_suid_sgid(),
        listeners: scan_listeners(),
        kernel_modules: scan_kernel_modules(),
        authorized_keys: scan_authorized_keys(),
        sudoers: scan_sudoers(),
        cron_jobs: scan_cron(),
        systemd_timers: scan_timers(),
        world_writable: scan_world_writable(),
        integrity_sha256: String::new(),
    };
    b.integrity_sha256 = digest_baseline(&b);
    b
}

fn digest_baseline(b: &IntegrityBaseline) -> String {
    // Hash everything except the digest field itself.
    let mut clone = b.clone();
    clone.integrity_sha256 = String::new();
    clone.captured_at = String::new(); // time must not change the identity digest
    sha256_str(&serde_json::to_string(&clone).unwrap_or_default())
}

/// Persist the baseline as the reference for future checks.
pub fn write_baseline() -> anyhow::Result<Value> {
    let baseline = capture();
    let path = baseline_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&baseline)?)?;
    std::fs::rename(&tmp, &path)?;
    crate::agent::audit::audit("integrity.baseline", "captured", "");
    Ok(json!({
        "captured_at": baseline.captured_at,
        "suid_sgid": baseline.suid_sgid.len(),
        "listeners": baseline.listeners.len(),
        "kernel_modules": baseline.kernel_modules.len(),
        "path": path.display().to_string(),
    }))
}

fn load_baseline() -> Option<IntegrityBaseline> {
    let bytes = std::fs::read(baseline_path()).ok()?;
    let baseline: IntegrityBaseline = serde_json::from_slice(&bytes).ok()?;
    // Detect tampering with the baseline file itself.
    if digest_baseline(&baseline) != baseline.integrity_sha256 {
        log::warn!("integrity baseline failed self-check — treating as absent");
        return None;
    }
    Some(baseline)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityChange {
    pub category: String,
    pub kind: String, // "added" | "removed" | "modified"
    pub item: String,
    pub severity: String, // "high" | "medium" | "low"
}

/// Diff the current surface against the stored baseline.
pub fn check() -> Value {
    let Some(baseline) = load_baseline() else {
        return json!({
            "has_baseline": false,
            "message": "no integrity baseline — run integrity.baseline first",
        });
    };
    let current = capture();
    let mut changes: Vec<IntegrityChange> = Vec::new();

    // SUID/SGID binaries: additions and hash changes are high severity
    // (new escalation path or trojaned binary).
    diff_map(
        &baseline.suid_sgid,
        &current.suid_sgid,
        "suid_sgid",
        "high",
        &mut changes,
    );
    // Kernel modules: newly loaded modules are high (rootkit surface).
    diff_list(
        &baseline.kernel_modules,
        &current.kernel_modules,
        "kernel_module",
        "high",
        true, // additions only
        &mut changes,
    );
    // authorized_keys changes: high (persistence).
    diff_map(
        &baseline.authorized_keys,
        &current.authorized_keys,
        "authorized_keys",
        "high",
        &mut changes,
    );
    // sudoers: high (privilege).
    diff_list(
        &baseline.sudoers,
        &current.sudoers,
        "sudoers",
        "high",
        false,
        &mut changes,
    );
    // New listeners: medium.
    diff_map(
        &baseline.listeners,
        &current.listeners,
        "listener",
        "medium",
        &mut changes,
    );
    // cron / timers: medium (persistence).
    diff_list(
        &baseline.cron_jobs,
        &current.cron_jobs,
        "cron",
        "medium",
        true,
        &mut changes,
    );
    diff_list(
        &baseline.systemd_timers,
        &current.systemd_timers,
        "systemd_timer",
        "medium",
        true,
        &mut changes,
    );
    // world-writable sensitive paths: medium.
    diff_list(
        &baseline.world_writable,
        &current.world_writable,
        "world_writable",
        "medium",
        true,
        &mut changes,
    );

    let high = changes.iter().filter(|c| c.severity == "high").count();
    json!({
        "has_baseline": true,
        "baseline_captured_at": baseline.captured_at,
        "checked_at": current.captured_at,
        "boot_changed": !baseline.boot_id.is_empty() && baseline.boot_id != current.boot_id,
        "change_count": changes.len(),
        "high_severity": high,
        "tampered": high > 0,
        "changes": changes,
    })
}

fn diff_map(
    base: &BTreeMap<String, String>,
    cur: &BTreeMap<String, String>,
    category: &str,
    severity: &str,
    out: &mut Vec<IntegrityChange>,
) {
    for (k, v) in cur {
        match base.get(k) {
            None => out.push(IntegrityChange {
                category: category.to_string(),
                kind: "added".to_string(),
                item: k.clone(),
                severity: severity.to_string(),
            }),
            Some(bv) if bv != v => out.push(IntegrityChange {
                category: category.to_string(),
                kind: "modified".to_string(),
                item: k.clone(),
                severity: severity.to_string(),
            }),
            _ => {}
        }
    }
    for k in base.keys() {
        if !cur.contains_key(k) {
            out.push(IntegrityChange {
                category: category.to_string(),
                kind: "removed".to_string(),
                item: k.clone(),
                severity: "low".to_string(),
            });
        }
    }
}

fn diff_list(
    base: &[String],
    cur: &[String],
    category: &str,
    severity: &str,
    additions_only: bool,
    out: &mut Vec<IntegrityChange>,
) {
    for item in cur {
        if !base.contains(item) {
            out.push(IntegrityChange {
                category: category.to_string(),
                kind: "added".to_string(),
                item: item.clone(),
                severity: severity.to_string(),
            });
        }
    }
    if !additions_only {
        for item in base {
            if !cur.contains(item) {
                out.push(IntegrityChange {
                    category: category.to_string(),
                    kind: "removed".to_string(),
                    item: item.clone(),
                    severity: "low".to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_self_digest_stable_across_time() {
        let mut b = IntegrityBaseline {
            captured_at: "t1".into(),
            kernel_modules: vec!["a".into(), "b".into()],
            ..Default::default()
        };
        let d1 = digest_baseline(&b);
        b.captured_at = "t2".into(); // time excluded from identity
        assert_eq!(d1, digest_baseline(&b));
        b.kernel_modules.push("evil".into());
        assert_ne!(d1, digest_baseline(&b));
    }

    #[test]
    fn diff_detects_new_suid_and_module() {
        let mut base = IntegrityBaseline::default();
        base.suid_sgid
            .insert("/usr/bin/sudo".into(), "hash1".into());
        base.kernel_modules = vec!["ext4".into()];

        let mut cur = base.clone();
        cur.suid_sgid.insert("/tmp/rootkit".into(), "hash2".into()); // added
        cur.suid_sgid
            .insert("/usr/bin/sudo".into(), "trojaned".into()); // modified
        cur.kernel_modules.push("evil_mod".into()); // added

        let mut changes = Vec::new();
        diff_map(
            &base.suid_sgid,
            &cur.suid_sgid,
            "suid_sgid",
            "high",
            &mut changes,
        );
        diff_list(
            &base.kernel_modules,
            &cur.kernel_modules,
            "kernel_module",
            "high",
            true,
            &mut changes,
        );
        assert!(changes
            .iter()
            .any(|c| c.item == "/tmp/rootkit" && c.kind == "added"));
        assert!(changes
            .iter()
            .any(|c| c.item == "/usr/bin/sudo" && c.kind == "modified"));
        assert!(changes.iter().any(|c| c.item == "evil_mod"));
    }

    #[test]
    fn capture_is_well_formed() {
        let b = capture();
        assert!(!b.integrity_sha256.is_empty());
        // Self-consistent digest.
        assert_eq!(b.integrity_sha256, digest_baseline(&b));
    }
}
