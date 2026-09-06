// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Plan preview and diff display

use super::types::*;
use colored::*;

/// Displays fix plans in human-readable format
pub struct PlanPreview;

impl PlanPreview {
    /// Display a plan as formatted text
    pub fn display(plan: &FixPlan) {
        Self::print_header(plan);
        Self::print_operations(plan);
        Self::print_footer(plan);
    }

    /// Display a plan as unified diff
    pub fn display_diff(plan: &FixPlan) {
        println!("{}", "Diff Preview".bold().cyan());
        println!("{}", "═".repeat(60).bright_black());
        println!();

        for op in &plan.operations {
            Self::print_operation_diff(op);
        }
    }

    /// Print plan header
    fn print_header(plan: &FixPlan) {
        println!();
        println!("{}", "📋 Fix Plan Preview".bold().cyan());
        println!("{}", "═".repeat(60).bright_black());
        println!();
        println!("{}: {}", "VM".bold(), plan.vm);
        println!(
            "{}: {} ({} risk)",
            "Profile".bold(),
            plan.profile,
            Self::colorize_risk(&plan.overall_risk)
        );
        println!("{}: {}", "Operations".bold(), plan.operations.len());
        println!(
            "{}: {}",
            "Estimated Duration".bold(),
            plan.estimated_duration
        );
        println!();
        println!("{}", "━".repeat(60).bright_black());
        println!();
    }

    /// Print operations grouped by priority
    fn print_operations(plan: &FixPlan) {
        for priority in &[
            Priority::Critical,
            Priority::High,
            Priority::Medium,
            Priority::Low,
            Priority::Info,
        ] {
            let ops: Vec<&Operation> = plan
                .operations
                .iter()
                .filter(|op| op.priority == *priority)
                .collect();

            if ops.is_empty() {
                continue;
            }

            println!(
                "{} {} Priority ({} operations)",
                priority.emoji(),
                priority.as_str().to_uppercase(),
                ops.len()
            );
            println!();

            for op in ops {
                Self::print_operation(op);
            }
        }
    }

    /// Ops that offline `plan apply` cannot fully execute without extras / first-boot.
    fn is_live_only(op: &OperationType) -> bool {
        match op {
            OperationType::PackageInstall(pi) => {
                !crate::cli::plan::package_stage::can_stage_offline(pi)
            }
            // Service/Command are offline-capable via wants symlink or first-boot staging.
            OperationType::ServiceOperation(_) | OperationType::CommandExec(_) => false,
            _ => false,
        }
    }

    /// Print a single operation
    fn print_operation(op: &Operation) {
        println!("[{}] {}", op.id.yellow(), op.description.bold());
        if let OperationType::PackageInstall(pi) = &op.op_type {
            println!(
                "  {}",
                crate::cli::plan::package_stage::stage_preview_note(pi).yellow()
            );
        } else if let OperationType::ServiceOperation(so) = &op.op_type {
            println!(
                "  {}",
                crate::cli::plan::firstboot_stage::service_preview_note(so).yellow()
            );
        } else if let OperationType::CommandExec(ce) = &op.op_type {
            println!(
                "  {}",
                crate::cli::plan::firstboot_stage::command_preview_note(ce).yellow()
            );
        } else if Self::is_live_only(&op.op_type) {
            println!(
                "  {}",
                "offline apply: skipped (needs running guest / live executor)".yellow()
            );
        }

        match &op.op_type {
            OperationType::FileEdit(fe) => {
                println!("  File: {}", fe.file.bright_blue());
                for change in &fe.changes {
                    if change.line > 0 {
                        println!(
                            "  Line {}: {} → {}",
                            change.line,
                            change.before.red(),
                            change.after.green()
                        );
                    }
                }
            }
            OperationType::FileWrite(fw) => {
                println!("  Write: {}", fw.path.bright_blue());
                println!("  Bytes: {}", fw.content.len());
            }
            OperationType::Symlink(sl) => {
                println!(
                    "  Symlink: {} → {}",
                    sl.link_path.bright_blue(),
                    sl.target.bright_green()
                );
            }
            OperationType::FileDelete(fd) => {
                println!("  Delete: {}", fd.path.bright_red());
                if fd.missing_ok {
                    println!("  {}", "missing_ok".bright_black());
                }
            }
            OperationType::PackageInstall(pi) => {
                println!("  Packages: {}", pi.packages.join(", ").bright_cyan());
                if let Some(size) = &pi.estimated_size {
                    println!("  Size: {}", size);
                }
            }
            OperationType::ServiceOperation(so) => {
                println!("  Service: {}", so.service.bright_cyan());
                if let Some(state) = &so.state {
                    println!("  State: {}", state.green());
                }
                if so.start {
                    println!("  {}", "Start on apply".green());
                }
            }
            OperationType::SelinuxMode(sm) => {
                println!("  File: {}", sm.file.bright_blue());
                println!("  {} → {}", sm.current.red(), sm.target.green());
                if let Some(warning) = &sm.warning {
                    println!("  ⚠️  {}", warning.yellow());
                }
            }
            OperationType::RegistryEdit(re) => {
                println!("  Key: {}", re.key.bright_blue());
                println!("  Value: {}", re.value);
                println!(
                    "  {} → {}",
                    re.current_data.to_string().red(),
                    re.new_data.to_string().green()
                );
            }
            OperationType::DriverInject(di) => {
                println!("  Driver: {}", di.driver_name.bright_blue());
                println!("  INF: {}", di.inf_path);
                if di.boot_critical {
                    println!("  {}", "boot-critical".yellow());
                }
                if di.resolve_host_dir().is_none() {
                    println!(
                        "  {}",
                        "set host_dir or GUESTKIT_VIRTIO_WIN for offline apply".yellow()
                    );
                }
            }
            OperationType::CommandExec(ce) => {
                println!("  Command: {}", ce.command.bright_cyan());
            }
            OperationType::FileCopy(fc) => {
                println!(
                    "  {} → {}",
                    fc.source.bright_blue(),
                    fc.destination.bright_green()
                );
            }
            OperationType::DirectoryCreate(dc) => {
                println!("  Path: {}", dc.path.bright_blue());
                if let Some(mode) = &dc.mode {
                    println!("  Mode: {}", mode);
                }
            }
            OperationType::FilePermissions(fp) => {
                println!("  Path: {}", fp.path.bright_blue());
                println!("  Mode: {}", fp.mode.green());
            }
        }

        println!(
            "  Risk: {} | Reversible: {}",
            Self::colorize_risk(op.risk.as_str()),
            if op.reversible {
                "Yes".green()
            } else {
                "No".red()
            }
        );

        if !op.depends_on.is_empty() {
            println!("  Depends on: {}", op.depends_on.join(", ").bright_black());
        }

        println!();
    }

    /// Print operation as diff
    fn print_operation_diff(op: &Operation) {
        if let OperationType::FileEdit(fe) = &op.op_type {
            println!("diff --git a{} b{}", fe.file, fe.file);
            println!("--- a{}", fe.file);
            println!("+++ b{}", fe.file);

            for change in &fe.changes {
                if let Some(context) = &change.context {
                    for line in context.lines() {
                        println!(" {}", line);
                    }
                }
                println!("{}", format!("-{}", change.before).red());
                println!("{}", format!("+{}", change.after).green());
            }
            println!();
        }
    }

    /// Print plan footer
    fn print_footer(plan: &FixPlan) {
        println!("{}", "━".repeat(60).bright_black());
        println!();

        if !plan.operations.is_empty() {
            // Show dependencies
            let has_deps = plan.operations.iter().any(|op| !op.depends_on.is_empty());
            if has_deps {
                println!("{}", "Dependencies:".bold());
                for op in &plan.operations {
                    if !op.depends_on.is_empty() {
                        println!(
                            "  {} → {}",
                            op.depends_on.join(", ").yellow(),
                            op.id.yellow()
                        );
                    }
                }
                println!();
            }

            // Show post-apply actions
            if !plan.post_apply.is_empty() {
                println!("{}", "Post-Apply Actions:".bold());
                for action in &plan.post_apply {
                    match action {
                        PostApplyAction::ServiceRestart { services } => {
                            println!(
                                "  • Restart services: {}",
                                services.join(", ").bright_cyan()
                            );
                        }
                        PostApplyAction::Validation { command, .. } => {
                            println!("  • Validate: {}", command.bright_blue());
                        }
                        PostApplyAction::Message { message } => {
                            println!("  • {}", message);
                        }
                        PostApplyAction::RebootRequired { reason } => {
                            println!("  {} Reboot required: {}", "⚠️".yellow(), reason.yellow());
                        }
                    }
                }
                println!();
            }
        }

        println!("{}", "Backup: Will create automatic backup".bright_black());
        println!(
            "{}",
            "Rollback: Available for all operations".bright_black()
        );
        println!();
    }

    /// Colorize risk level
    fn colorize_risk(risk: &str) -> ColoredString {
        match risk.to_lowercase().as_str() {
            "critical" => risk.red().bold(),
            "high" => risk.bright_red(),
            "medium" => risk.yellow(),
            "low" => risk.green(),
            _ => risk.normal(),
        }
    }

    /// Print summary statistics
    pub fn print_summary(plan: &FixPlan) {
        println!("{}", "Plan Summary".bold().cyan());
        println!("{}", "─".repeat(40));
        println!("Total Operations: {}", plan.operations.len());
        println!("Critical: {}", plan.count_by_priority(Priority::Critical));
        println!("High: {}", plan.count_by_priority(Priority::High));
        println!("Medium: {}", plan.count_by_priority(Priority::Medium));
        println!("Low: {}", plan.count_by_priority(Priority::Low));
        println!("Info: {}", plan.count_by_priority(Priority::Info));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_plan() -> FixPlan {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        // Add a test operation
        plan.add_operation(Operation {
            id: "op-001".to_string(),
            op_type: OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 15,
                    before: "PermitRootLogin yes".to_string(),
                    after: "PermitRootLogin no".to_string(),
                    context: Some("# Authentication:\n".to_string()),
                }],
            }),
            priority: Priority::High,
            description: "Disable root login".to_string(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        plan
    }

    #[test]
    fn test_preview_creation() {
        let plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());
        // Just ensure it doesn't panic
        PlanPreview::print_summary(&plan);
    }

    #[test]
    fn test_risk_colorization_critical() {
        let colored = PlanPreview::colorize_risk("critical");
        assert!(colored.to_string().contains("critical"));
    }

    #[test]
    fn test_risk_colorization_high() {
        let colored = PlanPreview::colorize_risk("high");
        assert!(colored.to_string().contains("high"));
    }

    #[test]
    fn test_risk_colorization_medium() {
        let colored = PlanPreview::colorize_risk("medium");
        assert!(colored.to_string().contains("medium"));
    }

    #[test]
    fn test_risk_colorization_low() {
        let colored = PlanPreview::colorize_risk("low");
        assert!(colored.to_string().contains("low"));
    }

    #[test]
    fn test_risk_colorization_unknown() {
        let colored = PlanPreview::colorize_risk("unknown");
        assert!(colored.to_string().contains("unknown"));
    }

    #[test]
    fn test_display_doesnt_panic() {
        let plan = create_test_plan();
        // Just ensure these methods don't panic
        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_diff_doesnt_panic() {
        let plan = create_test_plan();
        PlanPreview::display_diff(&plan);
    }

    #[test]
    fn test_print_summary_with_operations() {
        let mut plan = create_test_plan();

        // Add operations of different priorities
        plan.add_operation(Operation {
            id: "op-002".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo test".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Critical,
            description: "Critical operation".to_string(),
            risk: Priority::High,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        PlanPreview::print_summary(&plan);
    }

    #[test]
    fn test_print_summary_empty_plan() {
        let plan = FixPlan::new("empty.qcow2".to_string(), "test".to_string());
        PlanPreview::print_summary(&plan);
    }

    #[test]
    fn test_display_with_dependencies() {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-001".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo first".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::High,
            description: "First operation".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        plan.add_operation(Operation {
            id: "op-002".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo second".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::High,
            description: "Second operation".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec!["op-001".to_string()],
            validation: None,
            undo: None,
        });

        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_with_post_apply_actions() {
        let mut plan = create_test_plan();

        plan.post_apply.push(PostApplyAction::ServiceRestart {
            services: vec!["sshd".to_string()],
        });

        plan.post_apply.push(PostApplyAction::Message {
            message: "Configuration updated".to_string(),
        });

        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_package_install_operation() {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-pkg".to_string(),
            op_type: OperationType::PackageInstall(PackageInstall {
                packages: vec!["fail2ban".to_string(), "aide".to_string()],
                estimated_size: Some("25MB".to_string()),
                host_cache: None,
            }),
            priority: Priority::Medium,
            description: "Install security packages".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_service_operation() {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-svc".to_string(),
            op_type: OperationType::ServiceOperation(ServiceOperation {
                service: "firewalld".to_string(),
                state: Some("enabled".to_string()),
                start: true,
                restart: false,
            }),
            priority: Priority::High,
            description: "Enable firewall".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_selinux_operation() {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-sel".to_string(),
            op_type: OperationType::SelinuxMode(SELinuxMode {
                file: "/etc/selinux/config".to_string(),
                current: "permissive".to_string(),
                target: "enforcing".to_string(),
                warning: Some("Requires reboot".to_string()),
            }),
            priority: Priority::Critical,
            description: "Set SELinux to enforcing".to_string(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        PlanPreview::display(&plan);
    }

    #[test]
    fn test_display_all_operation_types() {
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        // FileEdit
        plan.add_operation(Operation {
            id: "op-001".to_string(),
            op_type: OperationType::FileEdit(FileEdit {
                file: "/etc/config".to_string(),
                backup: true,
                changes: vec![],
            }),
            priority: Priority::High,
            description: "Edit config".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        // FileCopy
        plan.add_operation(Operation {
            id: "op-002".to_string(),
            op_type: OperationType::FileCopy(FileCopy {
                source: "/etc/default/config".to_string(),
                destination: "/etc/config".to_string(),
                backup: true,
            }),
            priority: Priority::Medium,
            description: "Copy config".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        // DirectoryCreate
        plan.add_operation(Operation {
            id: "op-003".to_string(),
            op_type: OperationType::DirectoryCreate(DirectoryCreate {
                path: "/var/log/audit".to_string(),
                mode: Some("0755".to_string()),
            }),
            priority: Priority::Low,
            description: "Create directory".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        // FilePermissions
        plan.add_operation(Operation {
            id: "op-004".to_string(),
            op_type: OperationType::FilePermissions(FilePermissions {
                path: "/etc/shadow".to_string(),
                mode: "0000".to_string(),
                owner: Some("root".to_string()),
                group: Some("root".to_string()),
            }),
            priority: Priority::High,
            description: "Set permissions".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        PlanPreview::display(&plan);
    }
}
