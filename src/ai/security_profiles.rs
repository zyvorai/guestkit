// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 4 — CIS-style security profiles from evidence.

use crate::ai::semantic::SemanticAnalysis;
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProfileReport {
    pub profile: String,
    pub score: u8,
    pub passed: Vec<ProfileCheck>,
    pub failed: Vec<ProfileCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCheck {
    pub id: String,
    pub title: String,
    pub detail: String,
}

/// Evaluate a lightweight CIS-inspired profile against collected evidence.
pub fn evaluate_cis_profile(
    evidence: &EvidenceSnapshot,
    semantic: &SemanticAnalysis,
) -> SecurityProfileReport {
    let mut passed = Vec::new();
    let mut failed = Vec::new();

    check(
        &mut passed,
        &mut failed,
        "CIS-SSH-1",
        "SSH root login disabled",
        evidence.security.ssh_root_login == Some(false),
        "PermitRootLogin should be no",
    );
    check(
        &mut passed,
        &mut failed,
        "CIS-SEL-1",
        "SELinux not disabled unexpectedly",
        evidence.security.selinux != "disabled"
            || evidence.os.os_type.to_lowercase().contains("windows"),
        format!("SELinux mode: {}", evidence.security.selinux),
    );
    check(
        &mut passed,
        &mut failed,
        "CIS-FW-1",
        "Host firewall enabled",
        evidence.security.firewall_enabled,
        "Enable host firewall before production migration",
    );
    check(
        &mut passed,
        &mut failed,
        "CIS-AUD-1",
        "Audit daemon present",
        evidence.security.auditd,
        "Install and enable auditd for compliance workloads",
    );

    let weak_root_services = semantic
        .sandbox_scores
        .iter()
        .filter(|s| s.runs_as_root && s.score < 20)
        .count();
    check(
        &mut passed,
        &mut failed,
        "CIS-SVC-1",
        "Root services sandboxed",
        weak_root_services == 0,
        format!("{weak_root_services} enabled root services lack sandbox flags"),
    );

    if let Some(windows) = &evidence.windows {
        check(
            &mut passed,
            &mut failed,
            "CIS-WIN-RDP",
            "RDP disabled or reviewed",
            !windows.rdp_enabled,
            "Remote Desktop is enabled — restrict before migration",
        );
        check(
            &mut passed,
            &mut failed,
            "CIS-WIN-BL",
            "BitLocker state documented",
            !windows.bitlocker_detected || windows.pending_reboot,
            "BitLocker detected — capture recovery keys",
        );
        if let Some(forensic) = &windows.event_logs.forensic {
            check(
                &mut passed,
                &mut failed,
                "CIS-WIN-EVT",
                "No recent failed logon burst",
                forensic.failed_logons < 50,
                format!(
                    "{} failed logons in Security.evtx sample",
                    forensic.failed_logons
                ),
            );
            check(
                &mut passed,
                &mut failed,
                "CIS-WIN-SVC",
                "No service crash storm",
                forensic.service_failures < 20,
                format!(
                    "{} service failure events in System.evtx",
                    forensic.service_failures
                ),
            );
        }
    }

    let total = passed.len() + failed.len();
    let pct = passed
        .len()
        .checked_mul(100)
        .and_then(|v| {
            if total == 0 {
                Some(100)
            } else {
                v.checked_div(total)
            }
        })
        .unwrap_or(100);
    let score = u8::try_from(pct).unwrap_or(100);

    SecurityProfileReport {
        profile: "guestkit-cis-lite-v1".into(),
        score,
        passed,
        failed,
    }
}

fn check(
    passed: &mut Vec<ProfileCheck>,
    failed: &mut Vec<ProfileCheck>,
    id: &str,
    title: &str,
    ok: bool,
    detail: impl Into<String>,
) {
    let item = ProfileCheck {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
    };
    if ok {
        passed.push(item);
    } else {
        failed.push(item);
    }
}
