// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Optional offline repair operations (QGA inject, cloud-init network, fstab).

use crate::boot::BootabilityReport;
use crate::cli::plan::types::{
    CommandExec, FixPlan, Operation, OperationType, PackageInstall, Priority, ServiceOperation,
};

use super::RepairOptions;

pub fn append_repair_extras(plan: &mut FixPlan, options: &RepairOptions, boot: &BootabilityReport) {
    let mut op_counter = plan.operations.len();

    if options.inject_qga {
        op_counter += 1;
        plan.operations.push(Operation {
            id: format!("repair-{op_counter:03}"),
            op_type: OperationType::PackageInstall(PackageInstall {
                packages: vec!["qemu-guest-agent".into()],
                estimated_size: Some("~2MB".into()),
                host_cache: None,
            }),
            priority: Priority::High,
            description: "Install qemu-guest-agent for KubeVirt QGA channel".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });
        if options.enable_systemd {
            op_counter += 1;
            plan.operations.push(Operation {
                id: format!("repair-{op_counter:03}"),
                op_type: OperationType::ServiceOperation(ServiceOperation {
                    service: "qemu-guest-agent".into(),
                    state: Some("enabled".into()),
                    start: true,
                    restart: false,
                }),
                priority: Priority::Medium,
                description: "Enable qemu-guest-agent on boot".into(),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec![format!("repair-{:03}", op_counter - 1)],
                validation: None,
                undo: None,
            });
        }
    }

    if options.fix_cloud_init_network {
        op_counter += 1;
        plan.operations.push(Operation {
            id: format!("repair-{op_counter:03}"),
            op_type: OperationType::PackageInstall(PackageInstall {
                packages: vec!["cloud-init".into()],
                estimated_size: Some("~5MB".into()),
                host_cache: None,
            }),
            priority: Priority::Medium,
            description: "Ensure cloud-init is installed for guest network bootstrap".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });
        op_counter += 1;
        plan.operations.push(Operation {
            id: format!("repair-{op_counter:03}"),
            op_type: OperationType::CommandExec(CommandExec {
                interpreter: None,
                command: "grep -q '^network:' /etc/cloud/cloud.cfg 2>/dev/null || echo 'network: {config: disabled}' >> /etc/cloud/cloud.cfg".into(),
                expected_exit: 0,
                timeout: Some(60),
            }),
            priority: Priority::Medium,
            description: "Ensure cloud-init network configuration section exists".into(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![format!("repair-{:03}", op_counter - 1)],
            validation: None,
            undo: None,
        });
    }

    if options.validate_fstab {
        let has_fstab_finding = boot.blockers.iter().chain(boot.warnings.iter()).any(|f| {
            f.title.to_lowercase().contains("fstab") || f.message.to_lowercase().contains("fstab")
        });
        if !has_fstab_finding {
            op_counter += 1;
            plan.operations.push(Operation {
                id: format!("repair-{op_counter:03}"),
                op_type: OperationType::CommandExec(CommandExec {
                    interpreter: None,
                    command: "grep -v '^#' /etc/fstab | awk '{print $1,$2,$3}' | head -20".into(),
                    expected_exit: 0,
                    timeout: Some(60),
                }),
                priority: Priority::Medium,
                description: "Validate /etc/fstab entries against disk layout".into(),
                risk: Priority::Low,
                reversible: false,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }
    }

    if !options.inject_qga && options.enable_systemd && options.inject_agent {
        op_counter += 1;
        plan.operations.push(Operation {
            id: format!("repair-{op_counter:03}"),
            op_type: OperationType::CommandExec(CommandExec {
                interpreter: None,
                command: "systemctl daemon-reload".into(),
                expected_exit: 0,
                timeout: Some(30),
            }),
            priority: Priority::Low,
            description: "Reload systemd after agent unit install".into(),
            risk: Priority::Low,
            reversible: false,
            depends_on: vec![],
            validation: None,
            undo: None,
        });
    }
}
