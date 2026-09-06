// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit profile handler - Security and compliance profiling

use async_trait::async_trait;
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{OperationHandler, HandlerContext, HandlerResult};

/// Profile types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ProfileType {
    Security,
    Compliance,
    Hardening,
    Performance,
    Migration,
}

/// Profile operation payload
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProfilePayload {
    image: ImageSpec,
    profiles: Vec<ProfileType>,
    #[serde(default)]
    options: ProfileOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<OutputSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ImageSpec {
    path: String,
    format: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ProfileOptions {
    #[serde(default)]
    severity_threshold: String, // "low", "medium", "high", "critical"
    #[serde(default)]
    fail_on_critical: bool,
    #[serde(default)]
    include_remediation: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OutputSpec {
    format: String,
    destination: String,
    #[serde(default)]
    include_remediation: bool,
}

/// Finding severity
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Profile finding
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Finding {
    severity: Severity,
    title: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remediation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    references: Option<Vec<String>>,
}

fn convert_profile_report(report: &guestkit::cli::profiles::ProfileReport) -> Vec<Finding> {
    use guestkit::cli::profiles::{FindingStatus, RiskLevel};

    let mut out = Vec::new();
    for section in &report.sections {
        for f in &section.findings {
            if matches!(f.status, FindingStatus::Pass) {
                continue;
            }
            let severity = match (&f.status, f.risk_level) {
                (FindingStatus::Fail, Some(RiskLevel::Critical)) => Severity::Critical,
                (FindingStatus::Fail, Some(RiskLevel::High)) => Severity::High,
                (FindingStatus::Fail, Some(RiskLevel::Medium)) => Severity::Medium,
                (FindingStatus::Fail, _) => Severity::High,
                (FindingStatus::Warning, Some(RiskLevel::High) | Some(RiskLevel::Critical)) => {
                    Severity::High
                }
                (FindingStatus::Warning, _) => Severity::Medium,
                _ => Severity::Info,
            };
            out.push(Finding {
                severity,
                title: format!("{}: {}", section.title, f.item),
                description: f.message.clone(),
                remediation: None,
                references: Some(vec![report.profile_name.clone()]),
            });
        }
    }
    out
}

/// Guestkit profile handler
pub struct ProfileHandler {
    temp_dir: PathBuf,
}

impl ProfileHandler {
    /// Create a new profile handler
    pub fn new() -> Self {
        Self {
            temp_dir: std::env::temp_dir().join("guestkit-profile"),
        }
    }

    /// Run security profile using guestkit
    async fn run_security_profile(
        &self,
        context: &HandlerContext,
        image_path: String,
    ) -> WorkerResult<Vec<Finding>> {
        context.report_progress("security", Some(25), "Running security profile").await?;

        let findings = tokio::task::spawn_blocking(move || -> WorkerResult<Vec<Finding>> {
            use guestkit::Guestfs;

            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs: {}", e)))?;

            g.add_drive_ro(&image_path)
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {}", e)))?;

            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {}", e)))?;

            // Inspect OS and mount root
            let inspected = g.inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect: {}", e)))?;

            if inspected.is_empty() {
                return Ok(Vec::new());
            }

            let os_info = &inspected[0];
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount: {}", e)))?;

            let mut findings = Vec::new();

            // Check SSH root login
            if g.exists("/etc/ssh/sshd_config").unwrap_or(false) {
                if let Ok(config) = g.cat("/etc/ssh/sshd_config") {
                    if config.lines().any(|line| {
                        let line = line.trim();
                        line.starts_with("PermitRootLogin") && line.contains("yes")
                    }) {
                        findings.push(Finding {
                            severity: Severity::High,
                            title: "SSH root login enabled".to_string(),
                            description: "Root user can login via SSH".to_string(),
                            remediation: Some("Set PermitRootLogin no in /etc/ssh/sshd_config".to_string()),
                            references: Some(vec!["CIS-SSH-001".to_string()]),
                        });
                    }
                }
            }

            // Check password authentication
            if g.exists("/etc/ssh/sshd_config").unwrap_or(false) {
                if let Ok(config) = g.cat("/etc/ssh/sshd_config") {
                    if config.lines().any(|line| {
                        let line = line.trim();
                        line.starts_with("PasswordAuthentication") && line.contains("yes")
                    }) {
                        findings.push(Finding {
                            severity: Severity::Medium,
                            title: "SSH password authentication enabled".to_string(),
                            description: "Password authentication is less secure than key-based auth".to_string(),
                            remediation: Some("Set PasswordAuthentication no in /etc/ssh/sshd_config".to_string()),
                            references: Some(vec!["CIS-SSH-002".to_string()]),
                        });
                    }
                }
            }

            // Check firewall status
            let has_firewall = g.exists("/etc/firewalld").unwrap_or(false)
                || g.exists("/etc/ufw").unwrap_or(false)
                || g.exists("/etc/iptables").unwrap_or(false);

            if !has_firewall {
                findings.push(Finding {
                    severity: Severity::Medium,
                    title: "Firewall not configured".to_string(),
                    description: "No firewall configuration detected".to_string(),
                    remediation: Some("Install and configure firewalld or ufw".to_string()),
                    references: Some(vec!["CIS-FW-001".to_string()]),
                });
            }

            let _ = g.umount_all();
            let _ = g.shutdown();

            Ok(findings)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {}", e)))??;

        Ok(findings)
    }

    /// Run compliance profile using guestkit
    async fn run_compliance_profile(
        &self,
        context: &HandlerContext,
        image_path: String,
    ) -> WorkerResult<Vec<Finding>> {
        context.report_progress("compliance", Some(50), "Running compliance profile").await?;

        let findings = tokio::task::spawn_blocking(move || -> WorkerResult<Vec<Finding>> {
            use guestkit::Guestfs;

            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs: {}", e)))?;

            g.add_drive_ro(&image_path)
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {}", e)))?;

            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {}", e)))?;

            let inspected = g.inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect: {}", e)))?;

            if inspected.is_empty() {
                return Ok(Vec::new());
            }

            let os_info = &inspected[0];
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount: {}", e)))?;

            let mut findings = Vec::new();

            // Check SELinux status
            if let Ok(selinux_status) = g.getcon() {
                match selinux_status.to_lowercase().as_str() {
                    "disabled" => {
                        findings.push(Finding {
                            severity: Severity::High,
                            title: "SELinux disabled".to_string(),
                            description: "SELinux is completely disabled".to_string(),
                            remediation: Some("Enable SELinux in /etc/selinux/config and reboot".to_string()),
                            references: Some(vec!["PCI-DSS-2.2.4".to_string(), "CIS-1.6.1.1".to_string()]),
                        });
                    }
                    "permissive" => {
                        findings.push(Finding {
                            severity: Severity::Medium,
                            title: "SELinux in permissive mode".to_string(),
                            description: "SELinux is not enforcing policies".to_string(),
                            remediation: Some("Set SELINUX=enforcing in /etc/selinux/config".to_string()),
                            references: Some(vec!["PCI-DSS-2.2.4".to_string()]),
                        });
                    }
                    _ => {}
                }
            }

            // Check password policy
            if g.exists("/etc/login.defs").unwrap_or(false) {
                if let Ok(login_defs) = g.cat("/etc/login.defs") {
                    let pass_max_days = login_defs
                        .lines()
                        .find(|l| l.trim().starts_with("PASS_MAX_DAYS"))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .and_then(|v| v.parse::<u32>().ok());

                    if let Some(days) = pass_max_days {
                        if days > 90 {
                            findings.push(Finding {
                                severity: Severity::Medium,
                                title: "Password expiration policy too long".to_string(),
                                description: format!("Password max age is {} days (should be <= 90)", days),
                                remediation: Some("Set PASS_MAX_DAYS to 90 or less in /etc/login.defs".to_string()),
                                references: Some(vec!["CIS-5.4.1.1".to_string()]),
                            });
                        }
                    }
                }
            }

            let _ = g.umount_all();
            let _ = g.shutdown();

            Ok(findings)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {}", e)))??;

        Ok(findings)
    }

    /// Run hardening profile using guestkit
    async fn run_hardening_profile(
        &self,
        context: &HandlerContext,
        image_path: String,
    ) -> WorkerResult<Vec<Finding>> {
        context.report_progress("hardening", Some(75), "Running hardening profile").await?;

        let findings = tokio::task::spawn_blocking(move || -> WorkerResult<Vec<Finding>> {
            use guestkit::Guestfs;

            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs: {}", e)))?;

            g.add_drive_ro(&image_path)
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {}", e)))?;

            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {}", e)))?;

            let inspected = g.inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect: {}", e)))?;

            if inspected.is_empty() {
                return Ok(Vec::new());
            }

            let os_info = &inspected[0];
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount: {}", e)))?;

            let mut findings = Vec::new();

            // Check for unnecessary services
            if let Ok(services) = g.list_enabled_services() {
                let unnecessary = ["telnet", "rsh", "rlogin", "tftp", "vsftpd"];
                for svc in &unnecessary {
                    if services.iter().any(|s| s.contains(svc)) {
                        findings.push(Finding {
                            severity: Severity::Medium,
                            title: format!("Unnecessary service {} enabled", svc),
                            description: format!("Service {} should be disabled", svc),
                            remediation: Some(format!("Disable service: systemctl disable {}", svc)),
                            references: Some(vec!["CIS-2.2.1".to_string()]),
                        });
                    }
                }
            }

            // Check for world-writable files
            if g.exists("/tmp").unwrap_or(false) {
                findings.push(Finding {
                    severity: Severity::Info,
                    title: "Review world-writable directories".to_string(),
                    description: "Ensure /tmp has proper sticky bit set".to_string(),
                    remediation: Some("Verify /tmp permissions: chmod 1777 /tmp".to_string()),
                    references: Some(vec!["CIS-1.1.3".to_string()]),
                });
            }

            let _ = g.umount_all();
            let _ = g.shutdown();

            Ok(findings)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {}", e)))??;

        Ok(findings)
    }

    /// Run performance profile using guestkit CLI inspection profiles
    async fn run_performance_profile(
        &self,
        context: &HandlerContext,
        image_path: String,
    ) -> WorkerResult<Vec<Finding>> {
        context
            .report_progress("performance", Some(60), "Running performance profile")
            .await?;
        self.run_cli_inspection_profile(image_path, "performance")
            .await
    }

    /// Run migration profile using guestkit CLI inspection profiles
    async fn run_migration_profile(
        &self,
        context: &HandlerContext,
        image_path: String,
    ) -> WorkerResult<Vec<Finding>> {
        context
            .report_progress("migration", Some(80), "Running migration profile")
            .await?;
        self.run_cli_inspection_profile(image_path, "migration")
            .await
    }

    /// Mount image and run a named `guestkit::cli::profiles` inspection profile.
    async fn run_cli_inspection_profile(
        &self,
        image_path: String,
        profile_name: &'static str,
    ) -> WorkerResult<Vec<Finding>> {
        let findings = tokio::task::spawn_blocking(move || -> WorkerResult<Vec<Finding>> {
            use guestkit::cli::profiles::get_profile;
            use guestkit::Guestfs;

            let profile = get_profile(profile_name).ok_or_else(|| {
                WorkerError::ExecutionError(format!("unknown profile '{profile_name}'"))
            })?;

            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs: {e}")))?;

            g.add_drive_ro(&image_path)
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {e}")))?;

            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {e}")))?;

            let inspected = g
                .inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect: {e}")))?;

            if inspected.is_empty() {
                return Ok(Vec::new());
            }

            let os_info = &inspected[0];
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount: {e}")))?;

            let report = profile
                .inspect(&mut g, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Profile inspect failed: {e}")))?;

            let _ = g.umount_all();
            let _ = g.shutdown();

            Ok(convert_profile_report(&report))
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {e}")))??;

        Ok(findings)
    }

    /// Generate profile report
    async fn generate_report(
        &self,
        payload: &ProfilePayload,
        all_findings: Vec<Finding>,
    ) -> WorkerResult<serde_json::Value> {
        let critical_count = all_findings.iter()
            .filter(|f| matches!(f.severity, Severity::Critical))
            .count();
        let high_count = all_findings.iter()
            .filter(|f| matches!(f.severity, Severity::High))
            .count();
        let medium_count = all_findings.iter()
            .filter(|f| matches!(f.severity, Severity::Medium))
            .count();
        let low_count = all_findings.iter()
            .filter(|f| matches!(f.severity, Severity::Low))
            .count();

        Ok(serde_json::json!({
            "version": "1.0",
            "image": {
                "path": payload.image.path,
                "format": payload.image.format,
            },
            "profiles": payload.profiles,
            "summary": {
                "total_findings": all_findings.len(),
                "by_severity": {
                    "critical": critical_count,
                    "high": high_count,
                    "medium": medium_count,
                    "low": low_count,
                }
            },
            "findings": all_findings,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }

    /// Write report to file
    async fn write_report(
        &self,
        report: &serde_json::Value,
        output: &OutputSpec,
    ) -> WorkerResult<String> {
        let content = match output.format.as_str() {
            "json" => serde_json::to_string_pretty(report)?,
            "yaml" => serde_yaml::to_string(report)
                .map_err(|e| WorkerError::ExecutionError(format!("YAML error: {}", e)))?,
            _ => return Err(WorkerError::ExecutionError(
                format!("Unsupported format: {}", output.format)
            )),
        };

        let path = std::path::Path::new(&output.destination);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&output.destination, content).await?;

        Ok(output.destination.clone())
    }
}

impl Default for ProfileHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OperationHandler for ProfileHandler {
    fn name(&self) -> &str {
        "guestkit-profile"
    }

    fn operations(&self) -> Vec<String> {
        vec!["guestkit.profile".to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let profile_payload: ProfilePayload = serde_json::from_value(payload.data.clone())
            .map_err(|e| WorkerError::ExecutionError(
                format!("Invalid profile payload: {}", e)
            ))?;

        if profile_payload.profiles.is_empty() {
            return Err(WorkerError::ExecutionError(
                "At least one profile must be specified".to_string()
            ));
        }

        Ok(())
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        log::info!("Starting profile analysis for job {}", context.job_id);

        let profile_payload: ProfilePayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(
                format!("Failed to parse profile payload: {}", e)
            ))?;

        context.report_progress("start", Some(0), "Starting profile analysis").await?;

        // Run requested profiles
        let mut all_findings = Vec::new();
        let image_path = profile_payload.image.path.clone();

        for profile_type in &profile_payload.profiles {
            let findings = match profile_type {
                ProfileType::Security => self.run_security_profile(&context, image_path.clone()).await?,
                ProfileType::Compliance => self.run_compliance_profile(&context, image_path.clone()).await?,
                ProfileType::Hardening => self.run_hardening_profile(&context, image_path.clone()).await?,
                ProfileType::Performance => {
                    self.run_performance_profile(&context, image_path.clone()).await?
                }
                ProfileType::Migration => {
                    self.run_migration_profile(&context, image_path.clone()).await?
                }
            };
            all_findings.extend(findings);
        }

        // Generate report
        let report = self.generate_report(&profile_payload, all_findings).await?;

        // Write output
        let output_file = if let Some(ref output) = profile_payload.output {
            self.write_report(&report, output).await?
        } else {
            let temp_file = context.work_dir.join(format!("{}-profile.json", context.job_id));
            tokio::fs::write(
                &temp_file,
                serde_json::to_string_pretty(&report)?
            ).await?;
            temp_file.to_string_lossy().to_string()
        };

        context.report_progress("complete", Some(100), "Profile analysis complete").await?;

        Ok(HandlerResult::new()
            .with_output(output_file)
            .with_data(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressTracker;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_profile_handler() {
        let handler = ProfileHandler::new();
        assert_eq!(handler.operations(), vec!["guestkit.profile"]);

        let payload = Payload {
            payload_type: "guestkit.profile.v1".to_string(),
            data: serde_json::json!({
                "image": {
                    "path": "/vms/test.qcow2",
                    "format": "qcow2"
                },
                "profiles": ["security", "compliance"]
            }),
        };

        assert!(handler.validate(&payload).await.is_ok());
    }
}
