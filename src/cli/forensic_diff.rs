// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Forensic diff with security drift scoring.

use crate::cli::diff::{Change, InspectionDiff};
use crate::cli::formatters::InspectionReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicDiffReport {
    pub inspection_diff: InspectionDiff,
    pub security_drift_score: f64,
    pub config_drift_count: usize,
    pub suspicious_persistence: Vec<String>,
    pub ransomware_indicators: Vec<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbom: Option<crate::cli::sbom_diff::SbomDiffReport>,
}

/// Compute forensic diff between two inspection reports with security drift scoring.
pub fn compute_forensic_diff(
    before: &InspectionReport,
    after: &InspectionReport,
) -> ForensicDiffReport {
    let inspection_diff = InspectionDiff::compute(before, after);

    let mut drift_points = 0u32;
    let mut max_points = 0u32;

    // User changes
    max_points += 10;
    if !inspection_diff.user_changes.added.is_empty()
        || !inspection_diff.user_changes.removed.is_empty()
    {
        drift_points += 10;
    }

    // Service changes
    max_points += 10;
    if !inspection_diff.service_changes.enabled.is_empty()
        || !inspection_diff.service_changes.disabled.is_empty()
    {
        drift_points += 8;
    }

    // Package changes
    max_points += 15;
    let pkg_changes = inspection_diff.package_changes.added.len()
        + inspection_diff.package_changes.removed.len()
        + inspection_diff.package_changes.updated.len();
    if pkg_changes > 0 {
        drift_points += (pkg_changes.min(15)) as u32;
    }

    // Network changes
    max_points += 10;
    if !inspection_diff.network_changes.is_empty() {
        drift_points += 10;
    }

    let security_drift_score = if max_points > 0 {
        (drift_points as f64 / max_points as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let mut suspicious_persistence = Vec::new();
    for pkg in &inspection_diff.package_changes.added {
        let lower = pkg.to_lowercase();
        if lower.contains("cron") || lower.contains("systemd") || lower.contains("backdoor") {
            suspicious_persistence.push(format!("New package: {}", pkg));
        }
    }

    let mut ransomware_indicators = Vec::new();
    for change in &inspection_diff.config_changes {
        if change.new_value.contains(".encrypted")
            || change.new_value.contains("ransom")
            || change.field.contains("entropy")
        {
            ransomware_indicators.push(change.field.clone());
        }
    }
    for pkg in &inspection_diff.package_changes.added {
        if pkg.to_lowercase().contains("encrypt") {
            ransomware_indicators.push(format!("Suspicious package: {}", pkg));
        }
    }

    let config_drift_count =
        inspection_diff.config_changes.len() + inspection_diff.os_changes.len();

    let summary = if security_drift_score > 70.0 {
        format!(
            "High security drift ({:.0}%) — investigate before/after outage window",
            security_drift_score
        )
    } else if security_drift_score > 40.0 {
        format!("Moderate security drift ({:.0}%)", security_drift_score)
    } else {
        format!("Low security drift ({:.0}%)", security_drift_score)
    };

    ForensicDiffReport {
        inspection_diff,
        security_drift_score,
        config_drift_count,
        suspicious_persistence,
        ransomware_indicators,
        summary,
        sbom: None,
    }
}

/// Attach an SBOM package diff (SPDX / CycloneDX / inventory JSON).
pub fn attach_sbom(report: &mut ForensicDiffReport, sbom: crate::cli::sbom_diff::SbomDiffReport) {
    if sbom.dirty() {
        report.config_drift_count += sbom.added.len() + sbom.removed.len() + sbom.updated.len();
        for p in &sbom.added {
            let lower = p.name.to_lowercase();
            if lower.contains("cron") || lower.contains("backdoor") {
                report
                    .suspicious_persistence
                    .push(format!("SBOM added: {} {}", p.name, p.version));
            }
        }
    }
    report.sbom = Some(sbom);
}

/// Add config file checksum drift entries between reports.
pub fn enrich_config_drift(report: &mut ForensicDiffReport, extra_changes: Vec<Change>) {
    report.config_drift_count += extra_changes.len();
    report.inspection_diff.config_changes.extend(extra_changes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::formatters::{InspectionReport, OsInfo, PackagesInfo};

    fn empty_report() -> InspectionReport {
        InspectionReport {
            image_path: None,
            os: OsInfo {
                root: "/".to_string(),
                os_type: Some("linux".to_string()),
                distribution: None,
                product_name: None,
                architecture: None,
                version: None,
                hostname: None,
                package_format: None,
                init_system: None,
                package_manager: None,
                format: None,
            },
            system_config: None,
            network: None,
            users: None,
            ssh: None,
            services: None,
            runtimes: None,
            storage: None,
            boot: None,
            scheduled_tasks: None,
            security: None,
            packages: Some(PackagesInfo {
                format: "deb".to_string(),
                count: 0,
                kernels: vec![],
            }),
            disk_usage: None,
            windows: None,
        }
    }

    #[test]
    fn identical_reports_low_drift() {
        let before = empty_report();
        let after = empty_report();
        let report = compute_forensic_diff(&before, &after);
        assert!(report.security_drift_score < 10.0);
    }

    #[test]
    fn kernel_changes_raise_drift() {
        let before = empty_report();
        let mut after = empty_report();
        after.packages = Some(PackagesInfo {
            format: "deb".to_string(),
            count: 1,
            kernels: vec!["6.1.0-cron-helper".to_string()],
        });
        let report = compute_forensic_diff(&before, &after);
        assert!(report.security_drift_score > 0.0);
        assert!(!report.suspicious_persistence.is_empty());
    }
}
