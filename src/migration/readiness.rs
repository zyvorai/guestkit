// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration-readiness check framework.
//!
//! Checks are written once against [`EvidenceSnapshot`], so the same engine
//! serves offline images (`guestkit migrate-assess`) and the live agent
//! (`guestkit.migration.assess`). Boot-level probes are not duplicated:
//! wrapper checks translate `BootabilityReport` results into categorized
//! migration checks via [`AssessContext::boot_check`].

use crate::boot::report::{BootabilityReport, CheckSeverity};
use crate::boot::BootTarget;
use crate::evidence::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessCategory {
    Boot,
    Storage,
    Network,
    Driver,
    Application,
    Security,
}

impl ReadinessCategory {
    pub const ALL: [ReadinessCategory; 6] = [
        Self::Boot,
        Self::Storage,
        Self::Network,
        Self::Driver,
        Self::Application,
        Self::Security,
    ];
}

/// Machine-usable pointer from a failed check to a repair strategy;
/// consumed by `MigrationRepairPlanner`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationHint {
    AddVirtioToInitramfs,
    ConvertFstabToUuid,
    RepairBootloader,
    EnableSerialConsole,
    RemoveHypervisorTools { packages: Vec<String> },
    ScheduleSelinuxRelabel,
    InjectWindowsDriver { driver: String },
    RegisterBootCritical { driver: String },
    SuspendBitLocker,
    RemoveGhostNics { instance_ids: Vec<String> },
    PreserveStaticIp,
    Manual { instructions: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationCheckResult {
    /// "MIG-G-001" (general), "MIG-L-002" (Linux), "MIG-W-003" (Windows).
    pub id: String,
    pub name: String,
    pub category: ReadinessCategory,
    pub passed: bool,
    pub severity: CheckSeverity,
    pub message: String,
    pub weight: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<RemediationHint>,
}

impl MigrationCheckResult {
    pub fn pass(
        id: &str,
        name: &str,
        category: ReadinessCategory,
        weight: f64,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category,
            passed: true,
            severity: CheckSeverity::Info,
            message: message.into(),
            weight,
            remediation: None,
        }
    }

    pub fn fail(
        id: &str,
        name: &str,
        category: ReadinessCategory,
        weight: f64,
        severity: CheckSeverity,
        message: impl Into<String>,
        remediation: Option<RemediationHint>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category,
            passed: false,
            severity,
            message: message.into(),
            weight,
            remediation,
        }
    }
}

pub struct AssessContext<'a> {
    pub target: BootTarget,
    pub target_name: String,
    pub live: bool,
    pub boot_report: &'a BootabilityReport,
}

impl AssessContext<'_> {
    /// Wrap an already-computed boot check (by BOOT-nnn id) as a migration
    /// check, so probes run once. Returns None when the boot engine didn't
    /// run that check for this target.
    pub fn boot_check(
        &self,
        boot_id: &str,
        mig_id: &str,
        category: ReadinessCategory,
        weight: f64,
        remediation: Option<RemediationHint>,
    ) -> Option<MigrationCheckResult> {
        let src = self.boot_report.checks.iter().find(|c| c.id == boot_id)?;
        Some(MigrationCheckResult {
            id: mig_id.to_string(),
            name: src.name.clone(),
            category,
            passed: src.passed,
            severity: if src.passed {
                CheckSeverity::Info
            } else {
                src.severity
            },
            message: src.message.clone(),
            weight,
            remediation: if src.passed { None } else { remediation },
        })
    }
}

/// A single migration-readiness check.
pub trait MigrationCheck {
    fn run(&self, ev: &EvidenceSnapshot, ctx: &AssessContext) -> Option<MigrationCheckResult>;
}
