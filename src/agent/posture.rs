// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Security posture assessment (spec §18, Phase 5 scope).
//!
//! Evidence-first: each finding is a concrete observation with an id,
//! severity, and pass/fail, grouped into scored categories. This is a
//! posture report, not an EDR — no event streaming, no enforcement.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    fn weight(self) -> f64 {
        match self {
            Self::High => 10.0,
            Self::Medium => 5.0,
            Self::Low => 2.0,
            Self::Info => 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostureFinding {
    pub id: String,
    pub severity: Severity,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostureCategory {
    pub name: String,
    pub score: f64,
    pub findings: Vec<PostureFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostureReport {
    pub collected_at: String,
    pub platform: String,
    pub overall_score: f64,
    pub categories: Vec<PostureCategory>,
}

fn finding(
    id: &str,
    severity: Severity,
    passed: bool,
    message: impl Into<String>,
) -> PostureFinding {
    PostureFinding {
        id: id.to_string(),
        severity,
        passed,
        message: message.into(),
    }
}

fn score_category(findings: &[PostureFinding]) -> f64 {
    let penalty: f64 = findings
        .iter()
        .filter(|f| !f.passed)
        .map(|f| f.severity.weight())
        .sum();
    (100.0 - penalty * 2.0).max(0.0)
}

fn run_quick(cmd: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

pub fn collect() -> PostureReport {
    #[cfg(target_os = "windows")]
    let categories = windows_categories();
    #[cfg(not(target_os = "windows"))]
    let categories = linux_categories();
    let overall = if categories.is_empty() {
        0.0
    } else {
        categories.iter().map(|c| c.score).sum::<f64>() / categories.len() as f64
    };
    PostureReport {
        collected_at: chrono::Utc::now().to_rfc3339(),
        platform: std::env::consts::OS.to_string(),
        overall_score: overall.round(),
        categories,
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_categories() -> Vec<PostureCategory> {
    let mut cats = Vec::new();

    // --- Access control (MAC + SSH) ---
    let mut access = Vec::new();
    let selinux = run_quick("getenforce", &[]).unwrap_or_default();
    let apparmor = std::path::Path::new("/sys/kernel/security/apparmor").exists();
    access.push(match selinux.as_str() {
        "Enforcing" => finding("POS-L-001", Severity::High, true, "SELinux enforcing"),
        "Permissive" => finding(
            "POS-L-001",
            Severity::Medium,
            false,
            "SELinux permissive — policies logged but not enforced",
        ),
        "Disabled" => finding("POS-L-001", Severity::High, false, "SELinux disabled"),
        _ if apparmor => finding("POS-L-001", Severity::High, true, "AppArmor active"),
        _ => finding(
            "POS-L-001",
            Severity::Medium,
            false,
            "no mandatory access control (SELinux/AppArmor) detected",
        ),
    });

    if let Ok(sshd) = std::fs::read_to_string("/etc/ssh/sshd_config") {
        let effective = |key: &str| -> Option<String> {
            sshd.lines()
                .map(str::trim)
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.strip_prefix(key))
                .map(|v| v.trim().to_lowercase())
                .next()
        };
        let root_login = effective("PermitRootLogin").unwrap_or_else(|| "default".into());
        access.push(finding(
            "POS-L-002",
            Severity::High,
            root_login == "no" || root_login == "prohibit-password",
            format!("sshd PermitRootLogin = {root_login}"),
        ));
        let password_auth = effective("PasswordAuthentication").unwrap_or_else(|| "yes".into());
        access.push(finding(
            "POS-L-003",
            Severity::Medium,
            password_auth == "no",
            format!("sshd PasswordAuthentication = {password_auth}"),
        ));
    }
    cats.push(PostureCategory {
        name: "access_control".into(),
        score: score_category(&access),
        findings: access,
    });

    // --- Network exposure ---
    let mut network = Vec::new();
    let firewalld = run_quick("systemctl", &["is-active", "firewalld"])
        .map(|s| s == "active")
        .unwrap_or(false);
    let ufw = run_quick("systemctl", &["is-active", "ufw"])
        .map(|s| s == "active")
        .unwrap_or(false);
    let nft_rules = run_quick("nft", &["list", "ruleset"])
        .map(|s| s.lines().count() > 5)
        .unwrap_or(false);
    network.push(finding(
        "POS-L-010",
        Severity::High,
        firewalld || ufw || nft_rules,
        if firewalld {
            "firewalld active"
        } else if ufw {
            "ufw active"
        } else if nft_rules {
            "nftables ruleset present"
        } else {
            "no host firewall detected"
        },
    ));
    let intel = crate::agent::netintel::collect();
    let wide_open: Vec<u16> = intel
        .listeners
        .iter()
        .filter(|l| l.local_addr == "0.0.0.0" || l.local_addr == "::")
        .map(|l| l.local_port)
        .collect();
    network.push(finding(
        "POS-L-011",
        Severity::Info,
        true,
        format!(
            "{} listener(s), {} bound to all interfaces: {:?}",
            intel.total_listening,
            wide_open.len(),
            wide_open.iter().take(12).collect::<Vec<_>>()
        ),
    ));
    cats.push(PostureCategory {
        name: "network".into(),
        score: score_category(&network),
        findings: network,
    });

    // --- Auditing & updates ---
    let mut hygiene = Vec::new();
    let auditd = run_quick("systemctl", &["is-active", "auditd"])
        .map(|s| s == "active")
        .unwrap_or(false);
    hygiene.push(finding(
        "POS-L-020",
        Severity::Medium,
        auditd,
        if auditd {
            "auditd active"
        } else {
            "auditd not active"
        },
    ));
    let reboot_required = std::path::Path::new("/var/run/reboot-required").exists();
    hygiene.push(finding(
        "POS-L-021",
        Severity::Low,
        !reboot_required,
        if reboot_required {
            "reboot required (pending updates not fully applied)"
        } else {
            "no pending reboot marker"
        },
    ));
    if let Some(out) = run_quick("lastb", &["-n", "50", "--time-format", "iso"]) {
        let failures = out.lines().filter(|l| !l.trim().is_empty()).count();
        hygiene.push(finding(
            "POS-L-022",
            if failures > 25 {
                Severity::Medium
            } else {
                Severity::Info
            },
            failures <= 25,
            format!("{failures} recent failed login attempt(s) in lastb sample"),
        ));
    }
    cats.push(PostureCategory {
        name: "hygiene".into(),
        score: score_category(&hygiene),
        findings: hygiene,
    });

    cats
}

#[cfg(target_os = "windows")]
fn windows_categories() -> Vec<PostureCategory> {
    let mut cats = Vec::new();
    let ps = |script: &str| -> Option<String> {
        run_quick(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ],
        )
    };
    let ps_bool = |script: &str| -> Option<bool> {
        ps(script).map(|s| s.eq_ignore_ascii_case("true") || s == "1")
    };

    // --- Endpoint protection ---
    let mut endpoint = Vec::new();
    if let Some(rt) =
        ps_bool("(Get-MpComputerStatus -ErrorAction SilentlyContinue).RealTimeProtectionEnabled")
    {
        endpoint.push(finding(
            "POS-W-001",
            Severity::High,
            rt,
            format!("Defender real-time protection enabled: {rt}"),
        ));
    }
    if let Some(tp) =
        ps_bool("(Get-MpComputerStatus -ErrorAction SilentlyContinue).IsTamperProtected")
    {
        endpoint.push(finding(
            "POS-W-002",
            Severity::Medium,
            tp,
            format!("Defender tamper protection: {tp}"),
        ));
    }
    cats.push(PostureCategory {
        name: "endpoint_protection".into(),
        score: score_category(&endpoint),
        findings: endpoint,
    });

    // --- Network exposure ---
    let mut network = Vec::new();
    if let Some(profiles) = ps(
        "(Get-NetFirewallProfile -ErrorAction SilentlyContinue | Where-Object Enabled -eq $false | Measure-Object).Count",
    ) {
        let disabled: usize = profiles.parse().unwrap_or(0);
        network.push(finding(
            "POS-W-010",
            Severity::High,
            disabled == 0,
            format!("{disabled} firewall profile(s) disabled"),
        ));
    }
    if let Some(smb1) =
        ps_bool("(Get-SmbServerConfiguration -ErrorAction SilentlyContinue).EnableSMB1Protocol")
    {
        network.push(finding(
            "POS-W-011",
            Severity::High,
            !smb1,
            format!("SMBv1 enabled: {smb1}"),
        ));
    }
    if let Some(nla) = ps_bool(
        "[bool](Get-ItemProperty 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Terminal Server\\WinStations\\RDP-Tcp' -Name UserAuthentication -ErrorAction SilentlyContinue).UserAuthentication",
    ) {
        network.push(finding(
            "POS-W-012",
            Severity::Medium,
            nla,
            format!("RDP Network Level Authentication: {nla}"),
        ));
    }
    cats.push(PostureCategory {
        name: "network".into(),
        score: score_category(&network),
        findings: network,
    });

    // --- Data protection ---
    let mut data = Vec::new();
    if let Some(bl) = crate::collectors::windows_live::collect_bitlocker_state() {
        data.push(finding(
            "POS-W-020",
            Severity::Medium,
            bl.any_protected,
            if bl.any_protected {
                "BitLocker protection active"
            } else {
                "no BitLocker-protected volumes"
            },
        ));
    }
    cats.push(PostureCategory {
        name: "data_protection".into(),
        score: score_category(&data),
        findings: data,
    });

    cats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_penalizes_failures() {
        let findings = vec![
            finding("A", Severity::High, false, "bad"),
            finding("B", Severity::Low, true, "ok"),
        ];
        assert_eq!(score_category(&findings), 80.0);
        let all_pass = vec![finding("A", Severity::High, true, "ok")];
        assert_eq!(score_category(&all_pass), 100.0);
    }

    #[test]
    fn collect_is_well_formed() {
        let report = collect();
        assert!(!report.collected_at.is_empty());
        assert!(report.overall_score >= 0.0 && report.overall_score <= 100.0);
        assert!(!report.categories.is_empty());
    }
}
