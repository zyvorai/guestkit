// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Policy-based validation module

pub mod benchmarks;
pub mod cloud_profiles;
pub mod expr;
pub mod policy;
pub mod rego;
pub mod rules;

use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use benchmarks::Benchmark;
pub use policy::{Policy, PolicyRule, RuleType};

/// Validation result for a single rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub rule_id: String,
    pub rule_name: String,
    pub status: ValidationStatus,
    pub message: String,
    pub severity: String,
    pub remediation: Option<String>,
}

/// Validation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Pass,
    Fail,
    Warning,
    Skip,
    Error,
}

impl ValidationStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Warning => "WARN",
            Self::Skip => "SKIP",
            Self::Error => "ERROR",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            Self::Pass => "✅",
            Self::Fail => "❌",
            Self::Warning => "⚠️",
            Self::Skip => "⏭️",
            Self::Error => "🔥",
        }
    }
}

/// Complete validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub image_path: String,
    pub policy_name: String,
    pub timestamp: String,
    pub results: Vec<ValidationResult>,
    pub summary: ValidationSummary,
}

/// Validation summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_rules: usize,
    pub passed: usize,
    pub failed: usize,
    pub warnings: usize,
    pub skipped: usize,
    pub errors: usize,
    pub compliance_score: f64,
}

impl ValidationSummary {
    pub fn new(results: &[ValidationResult]) -> Self {
        let total = results.len();
        let passed = results
            .iter()
            .filter(|r| r.status == ValidationStatus::Pass)
            .count();
        let failed = results
            .iter()
            .filter(|r| r.status == ValidationStatus::Fail)
            .count();
        let warnings = results
            .iter()
            .filter(|r| r.status == ValidationStatus::Warning)
            .count();
        let skipped = results
            .iter()
            .filter(|r| r.status == ValidationStatus::Skip)
            .count();
        let errors = results
            .iter()
            .filter(|r| r.status == ValidationStatus::Error)
            .count();

        let compliance_score = if total > skipped {
            (passed as f64 / (total - skipped) as f64) * 100.0
        } else {
            0.0
        };

        Self {
            total_rules: total,
            passed,
            failed,
            warnings,
            skipped,
            errors,
            compliance_score,
        }
    }
}

/// Validate disk image against policy
pub fn validate_image<P: AsRef<Path>>(
    image_path: P,
    policy: &Policy,
    verbose: bool,
) -> Result<ValidationReport> {
    let image_path_str = image_path.as_ref().display().to_string();

    if verbose {
        println!("🔍 Validating: {}", image_path_str);
        println!("📋 Policy: {}", policy.name);
    }

    // Initialize guestfs
    let mut g = Guestfs::new()?;
    g.add_drive_opts(&image_path, true, None)?;
    g.launch()?;

    // Inspect OS
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        anyhow::bail!("No operating systems found in disk image");
    }

    let root = &roots[0];

    // Mount filesystems
    let mountpoints = g.inspect_get_mountpoints(root)?;
    for (mp, dev) in mountpoints {
        let _ = g.mount(&dev, &mp);
    }

    // Build evidence snapshot for expression rules
    let evidence = crate::evidence::build_evidence(&mut g, root, image_path.as_ref())?;
    let boot_report = crate::boot::analyze_bootability(&evidence, crate::boot::BootTarget::Generic);

    // Run validation rules
    let mut results = Vec::new();

    for rule in &policy.rules {
        if verbose {
            println!("  Checking: {}", rule.name);
        }

        let result = validate_rule(&mut g, root, rule, &evidence, boot_report.score)?;
        results.push(result);
    }

    // Shutdown guestfs
    g.shutdown()?;

    // Calculate summary
    let summary = ValidationSummary::new(&results);

    Ok(ValidationReport {
        image_path: image_path_str,
        policy_name: policy.name.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        results,
        summary,
    })
}

/// Validate a single rule
fn validate_rule(
    g: &mut Guestfs,
    root: &str,
    rule: &PolicyRule,
    evidence: &crate::evidence::EvidenceSnapshot,
    boot_score: f64,
) -> Result<ValidationResult> {
    // Expression rules take precedence
    if let Some(expr) = &rule.expr {
        let passed = expr::evaluate_expr(expr, evidence, Some(boot_score))?;
        return Ok(ValidationResult {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            status: if passed {
                ValidationStatus::Pass
            } else {
                ValidationStatus::Fail
            },
            message: format!("Expression `{}` => {}", expr, passed),
            severity: rule.severity.clone(),
            remediation: rule.remediation.clone(),
        });
    }

    let status = match &rule.rule_type {
        RuleType::PackageInstalled { package } => check_package_installed(g, root, package)?,
        RuleType::PackageForbidden { package } => check_package_forbidden(g, root, package)?,
        RuleType::FileExists { path } => check_file_exists(g, path)?,
        RuleType::FileNotExists { path } => check_file_not_exists(g, path)?,
        RuleType::FileContains { path, pattern } => check_file_contains(g, path, pattern)?,
        RuleType::FilePermissions { path, mode } => check_file_permissions(g, path, mode)?,
        RuleType::ServiceEnabled { service } => check_service_enabled(g, service)?,
        RuleType::ServiceDisabled { service } => check_service_disabled(g, service)?,
        RuleType::UserExists { username } => check_user_exists(g, username)?,
        RuleType::UserNotExists { username } => check_user_not_exists(g, username)?,
        RuleType::PortClosed { port: _ } => {
            // Port checking requires more complex parsing
            ValidationStatus::Skip
        }
        RuleType::Expression { expr } => {
            if expr::evaluate_expr(expr, evidence, Some(boot_score))? {
                ValidationStatus::Pass
            } else {
                ValidationStatus::Fail
            }
        }
        RuleType::Custom { check: _ } => {
            // Custom checks would be implemented here
            ValidationStatus::Skip
        }
    };

    let message = if status == ValidationStatus::Pass {
        format!("{} - Check passed", rule.name)
    } else {
        format!("{} - Check failed", rule.name)
    };

    Ok(ValidationResult {
        rule_id: rule.id.clone(),
        rule_name: rule.name.clone(),
        status,
        message,
        severity: rule.severity.clone(),
        remediation: rule.remediation.clone(),
    })
}

// Rule check implementations

fn check_package_installed(g: &mut Guestfs, root: &str, package: &str) -> Result<ValidationStatus> {
    let apps = g.inspect_list_applications2(root)?;
    let installed = apps.iter().any(|(name, _, _)| name == package);
    Ok(if installed {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_package_forbidden(g: &mut Guestfs, root: &str, package: &str) -> Result<ValidationStatus> {
    let apps = g.inspect_list_applications2(root)?;
    let installed = apps.iter().any(|(name, _, _)| name == package);
    Ok(if installed {
        ValidationStatus::Fail
    } else {
        ValidationStatus::Pass
    })
}

fn check_file_exists(g: &mut Guestfs, path: &str) -> Result<ValidationStatus> {
    let exists = g.exists(path)?;
    Ok(if exists {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_file_not_exists(g: &mut Guestfs, path: &str) -> Result<ValidationStatus> {
    let exists = g.exists(path)?;
    Ok(if exists {
        ValidationStatus::Fail
    } else {
        ValidationStatus::Pass
    })
}

fn check_file_contains(g: &mut Guestfs, path: &str, pattern: &str) -> Result<ValidationStatus> {
    if !g.exists(path)? {
        return Ok(ValidationStatus::Fail);
    }

    let content = g.read_file(path)?;
    let content_str = String::from_utf8_lossy(&content);
    Ok(if content_str.contains(pattern) {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_file_permissions(
    g: &mut Guestfs,
    path: &str,
    expected_mode: &str,
) -> Result<ValidationStatus> {
    if !g.exists(path)? {
        return Ok(ValidationStatus::Fail);
    }

    let stat = g.stat(path)?;
    let actual_mode = format!("{:o}", stat.mode & 0o777);

    Ok(if actual_mode == expected_mode {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_service_enabled(g: &mut Guestfs, service: &str) -> Result<ValidationStatus> {
    // Check if systemd unit is enabled
    let service_path = format!(
        "/etc/systemd/system/multi-user.target.wants/{}.service",
        service
    );
    let enabled = g.exists(&service_path)?;

    Ok(if enabled {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_service_disabled(g: &mut Guestfs, service: &str) -> Result<ValidationStatus> {
    let service_path = format!(
        "/etc/systemd/system/multi-user.target.wants/{}.service",
        service
    );
    let enabled = g.exists(&service_path)?;

    Ok(if enabled {
        ValidationStatus::Fail
    } else {
        ValidationStatus::Pass
    })
}

fn check_user_exists(g: &mut Guestfs, username: &str) -> Result<ValidationStatus> {
    if !g.exists("/etc/passwd")? {
        return Ok(ValidationStatus::Error);
    }

    let passwd = g.read_file("/etc/passwd")?;
    let passwd_str = String::from_utf8_lossy(&passwd);

    let exists = passwd_str.lines().any(|line| {
        line.split(':')
            .next()
            .map(|u| u == username)
            .unwrap_or(false)
    });

    Ok(if exists {
        ValidationStatus::Pass
    } else {
        ValidationStatus::Fail
    })
}

fn check_user_not_exists(g: &mut Guestfs, username: &str) -> Result<ValidationStatus> {
    if !g.exists("/etc/passwd")? {
        return Ok(ValidationStatus::Error);
    }

    let passwd = g.read_file("/etc/passwd")?;
    let passwd_str = String::from_utf8_lossy(&passwd);

    let exists = passwd_str.lines().any(|line| {
        line.split(':')
            .next()
            .map(|u| u == username)
            .unwrap_or(false)
    });

    Ok(if exists {
        ValidationStatus::Fail
    } else {
        ValidationStatus::Pass
    })
}

/// Format validation report as text
pub fn format_report(report: &ValidationReport) -> String {
    let mut output = String::new();

    output.push_str("🔍 Policy Validation Report\n");
    output.push_str("==========================\n\n");
    output.push_str(&format!("Image: {}\n", report.image_path));
    output.push_str(&format!("Policy: {}\n", report.policy_name));
    output.push_str(&format!("Time: {}\n\n", report.timestamp));

    output.push_str("📊 Summary\n");
    output.push_str("----------\n");
    output.push_str(&format!("Total Rules: {}\n", report.summary.total_rules));
    output.push_str(&format!("✅ Passed: {}\n", report.summary.passed));
    output.push_str(&format!("❌ Failed: {}\n", report.summary.failed));
    output.push_str(&format!("⚠️  Warnings: {}\n", report.summary.warnings));
    output.push_str(&format!("⏭️  Skipped: {}\n", report.summary.skipped));
    output.push_str(&format!(
        "\n📈 Compliance Score: {:.1}%\n\n",
        report.summary.compliance_score
    ));

    if report.summary.failed > 0 {
        output.push_str("❌ Failed Checks\n");
        output.push_str("---------------\n");
        for result in &report.results {
            if result.status == ValidationStatus::Fail {
                output.push_str(&format!(
                    "  {} [{}] {}\n",
                    result.status.emoji(),
                    result.severity,
                    result.rule_name
                ));
                if let Some(remediation) = &result.remediation {
                    output.push_str(&format!("    💡 {}\n", remediation));
                }
            }
        }
        output.push('\n');
    }

    if report.summary.warnings > 0 {
        output.push_str("⚠️  Warnings\n");
        output.push_str("-----------\n");
        for result in &report.results {
            if result.status == ValidationStatus::Warning {
                output.push_str(&format!(
                    "  {} [{}] {}\n",
                    result.status.emoji(),
                    result.severity,
                    result.rule_name
                ));
            }
        }
        output.push('\n');
    }

    if report.summary.compliance_score >= 90.0 {
        output.push_str("✅ Excellent compliance!\n");
    } else if report.summary.compliance_score >= 75.0 {
        output.push_str("⚠️  Good compliance, but improvements needed\n");
    } else if report.summary.compliance_score >= 50.0 {
        output.push_str("❌ Poor compliance - significant issues found\n");
    } else {
        output.push_str("🔥 Critical compliance failure!\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_status_as_str() {
        assert_eq!(ValidationStatus::Pass.as_str(), "PASS");
        assert_eq!(ValidationStatus::Fail.as_str(), "FAIL");
        assert_eq!(ValidationStatus::Warning.as_str(), "WARN");
        assert_eq!(ValidationStatus::Skip.as_str(), "SKIP");
        assert_eq!(ValidationStatus::Error.as_str(), "ERROR");
    }

    #[test]
    fn test_validation_status_emoji() {
        assert_eq!(ValidationStatus::Pass.emoji(), "✅");
        assert_eq!(ValidationStatus::Fail.emoji(), "❌");
        assert_eq!(ValidationStatus::Warning.emoji(), "⚠️");
        assert_eq!(ValidationStatus::Skip.emoji(), "⏭️");
        assert_eq!(ValidationStatus::Error.emoji(), "🔥");
    }

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult {
            rule_id: "SEC-001".to_string(),
            rule_name: "SSH root login disabled".to_string(),
            status: ValidationStatus::Pass,
            message: "SSH configuration is secure".to_string(),
            severity: "HIGH".to_string(),
            remediation: Some("No action needed".to_string()),
        };

        assert_eq!(result.rule_id, "SEC-001");
        assert_eq!(result.status, ValidationStatus::Pass);
        assert_eq!(result.severity, "HIGH");
        assert!(result.remediation.is_some());
    }

    #[test]
    fn test_validation_summary_all_passed() {
        let results = vec![
            ValidationResult {
                rule_id: "R1".to_string(),
                rule_name: "Rule 1".to_string(),
                status: ValidationStatus::Pass,
                message: "OK".to_string(),
                severity: "HIGH".to_string(),
                remediation: None,
            },
            ValidationResult {
                rule_id: "R2".to_string(),
                rule_name: "Rule 2".to_string(),
                status: ValidationStatus::Pass,
                message: "OK".to_string(),
                severity: "MEDIUM".to_string(),
                remediation: None,
            },
        ];

        let summary = ValidationSummary::new(&results);

        assert_eq!(summary.total_rules, 2);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.warnings, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.errors, 0);
        assert_eq!(summary.compliance_score, 100.0);
    }

    #[test]
    fn test_validation_summary_mixed_results() {
        let results = vec![
            ValidationResult {
                rule_id: "R1".to_string(),
                rule_name: "Rule 1".to_string(),
                status: ValidationStatus::Pass,
                message: "OK".to_string(),
                severity: "HIGH".to_string(),
                remediation: None,
            },
            ValidationResult {
                rule_id: "R2".to_string(),
                rule_name: "Rule 2".to_string(),
                status: ValidationStatus::Fail,
                message: "Failed".to_string(),
                severity: "HIGH".to_string(),
                remediation: Some("Fix this".to_string()),
            },
            ValidationResult {
                rule_id: "R3".to_string(),
                rule_name: "Rule 3".to_string(),
                status: ValidationStatus::Warning,
                message: "Warning".to_string(),
                severity: "LOW".to_string(),
                remediation: None,
            },
            ValidationResult {
                rule_id: "R4".to_string(),
                rule_name: "Rule 4".to_string(),
                status: ValidationStatus::Skip,
                message: "Skipped".to_string(),
                severity: "MEDIUM".to_string(),
                remediation: None,
            },
        ];

        let summary = ValidationSummary::new(&results);

        assert_eq!(summary.total_rules, 4);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.warnings, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.errors, 0);
        // Score = 1 passed / (4 total - 1 skipped) = 1/3 = 33.33%
        assert!((summary.compliance_score - 33.33).abs() < 0.1);
    }

    #[test]
    fn test_validation_summary_empty() {
        let results = vec![];
        let summary = ValidationSummary::new(&results);

        assert_eq!(summary.total_rules, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.compliance_score, 0.0);
    }

    #[test]
    fn test_validation_report_creation() {
        let report = ValidationReport {
            image_path: "/path/to/image.qcow2".to_string(),
            policy_name: "CIS Benchmark".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 0.0,
            },
        };

        assert_eq!(report.policy_name, "CIS Benchmark");
        assert_eq!(report.results.len(), 0);
    }

    #[test]
    fn test_format_report_contains_header() {
        let report = ValidationReport {
            image_path: "/test/image.qcow2".to_string(),
            policy_name: "Test Policy".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 0.0,
            },
        };

        let output = format_report(&report);

        assert!(output.contains("Policy Validation Report"));
        assert!(output.contains("/test/image.qcow2"));
        assert!(output.contains("Test Policy"));
        assert!(output.contains("Summary"));
    }

    #[test]
    fn test_format_report_compliance_messages() {
        // Excellent compliance
        let report_excellent = ValidationReport {
            image_path: "/test.qcow2".to_string(),
            policy_name: "Test".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 95.0,
            },
        };
        let output = format_report(&report_excellent);
        assert!(output.contains("Excellent compliance"));

        // Good compliance
        let report_good = ValidationReport {
            image_path: "/test.qcow2".to_string(),
            policy_name: "Test".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 80.0,
            },
        };
        let output = format_report(&report_good);
        assert!(output.contains("Good compliance"));

        // Poor compliance
        let report_poor = ValidationReport {
            image_path: "/test.qcow2".to_string(),
            policy_name: "Test".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 60.0,
            },
        };
        let output = format_report(&report_poor);
        assert!(output.contains("Poor compliance"));

        // Critical failure
        let report_critical = ValidationReport {
            image_path: "/test.qcow2".to_string(),
            policy_name: "Test".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            results: vec![],
            summary: ValidationSummary {
                total_rules: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
                errors: 0,
                compliance_score: 30.0,
            },
        };
        let output = format_report(&report_critical);
        assert!(output.contains("Critical compliance failure"));
    }
}
