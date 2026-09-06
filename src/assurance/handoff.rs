// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Passport → hyper2kvm / h2kvmctl job document.
//!
//! GuestKit certifies. h2kvmctl converts. This file is the contract between
//! the two so a conversion job cannot start on a hard-blocked passport.

use super::{verify_passport, CutoverPassport, PassportVerifyOptions};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HANDOFF_API_VERSION: &str = "guestkit.zyvor.dev/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H2kvmHandoff {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub passport: String,
    pub image: String,
    pub target: String,
    pub scores: HandoffScores,
    pub hard_blocked: bool,
    pub allowed: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    pub h2kvmctl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffScores {
    pub boot: f64,
    pub migration: f64,
}

pub fn build_handoff(
    passport_path: &Path,
    passport: &CutoverPassport,
    allowed: bool,
    extra_blockers: &[String],
) -> H2kvmHandoff {
    let mut blockers: Vec<String> = passport
        .critical_blockers
        .iter()
        .map(|f| format!("{}: {}", f.id, f.title))
        .collect();
    blockers.extend(extra_blockers.iter().cloned());
    H2kvmHandoff {
        api_version: HANDOFF_API_VERSION.into(),
        kind: "H2kvmHandoff".into(),
        passport: passport_path.display().to_string(),
        image: passport.image.path.clone(),
        target: passport.target.clone(),
        scores: HandoffScores {
            boot: passport.scores.boot,
            migration: passport.scores.migration,
        },
        hard_blocked: passport.hard_blocked,
        allowed,
        blockers,
        h2kvmctl: format!(
            "h2kvmctl local --to-output out.qcow2 --backend guestkit --passport {}",
            passport_path.display()
        ),
    }
}

pub fn load_and_gate(
    passport_path: &Path,
    opts: &PassportVerifyOptions,
) -> Result<(CutoverPassport, bool, Vec<String>)> {
    let raw = std::fs::read_to_string(passport_path)
        .with_context(|| format!("read passport {}", passport_path.display()))?;
    let passport: CutoverPassport =
        serde_json::from_str(&raw).context("parse Cutover Passport JSON")?;
    match verify_passport(&passport, opts) {
        Ok(()) => Ok((passport, true, Vec::new())),
        Err(e) => Ok((passport, false, vec![e.to_string()])),
    }
}

pub fn write_handoff(doc: &H2kvmHandoff, output: &Path) -> Result<()> {
    if output
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        std::fs::write(output, serde_json::to_string_pretty(doc)?)?;
    } else {
        std::fs::write(output, serde_yaml::to_string(doc)?)?;
    }
    Ok(())
}

pub fn default_output(passport_path: &Path) -> PathBuf {
    let mut out = passport_path.to_path_buf();
    out.set_extension("handoff.yaml");
    out
}

#[cfg(test)]
mod tests {
    use super::super::passport::{
        ImageFingerprint, PassportScores, PlanDigest, SuiteHandoff, WindowsPassportFlags,
    };
    use super::*;
    use crate::assurance::EvidenceDigest;
    use crate::assurance::PASSPORT_SCHEMA_VERSION;
    use crate::migration::ReadinessLevel;
    use chrono::Utc;

    fn stub(hard_blocked: bool) -> CutoverPassport {
        CutoverPassport {
            schema_version: PASSPORT_SCHEMA_VERSION.into(),
            kind: "guestkit.cutover_passport".into(),
            generated_at: Utc::now().to_rfc3339(),
            tool_version: "0.0.0".into(),
            target: "kvm".into(),
            image: ImageFingerprint {
                path: "/disks/web.qcow2".into(),
                size_bytes: 1,
                content_sha256: None,
            },
            evidence_schema: "1".into(),
            evidence_digest: EvidenceDigest {
                os: "linux".into(),
                architecture: "x86_64".into(),
                bootloader: "grub".into(),
                root_filesystem: "ext4".into(),
                kernel_count: 1,
                fstab_entries: 1,
                virtio_modules_loaded: true,
                vm_tools: vec![],
                selinux: "disabled".into(),
            },
            scores: PassportScores {
                boot: 91.0,
                migration: 88.0,
                readiness: ReadinessLevel::Ready,
            },
            critical_blockers: vec![],
            recommended_actions: vec![],
            fix_plan: PlanDigest {
                profile: "migration-repair".into(),
                operation_count: 0,
                sha256: "abc".into(),
                operation_ids: vec![],
            },
            policy: None,
            windows: WindowsPassportFlags::default(),
            live_attestation: None,
            suite: SuiteHandoff::default(),
            hard_blocked,
            issuer: None,
            expires_at: None,
            signature: None,
        }
    }

    #[test]
    fn allowed_handoff_points_at_h2kvmctl() {
        let p = stub(false);
        let doc = build_handoff(Path::new("p.json"), &p, true, &[]);
        assert!(doc.allowed);
        assert!(doc.h2kvmctl.contains("--passport p.json"));
        assert_eq!(doc.image, "/disks/web.qcow2");
    }

    #[test]
    fn blocked_passport_stays_disallowed() {
        let p = stub(true);
        let doc = build_handoff(Path::new("p.json"), &p, false, &["hard-blocked".into()]);
        assert!(!doc.allowed);
        assert!(doc.hard_blocked);
        assert!(!doc.blockers.is_empty());
    }
}
