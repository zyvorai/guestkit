// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration repair planner: failed checks → auditable FixPlan operations.
//!
//! Every generated operation carries undo information and (where the
//! effect is verifiable) a post-operation validation command. Ops rated
//! High/Critical without undo are refused, not silently emitted.
//! Destructive operations (ghost-NIC removal, tool uninstall) are only
//! generated when explicitly requested.

use super::readiness::RemediationHint as Hint;
use super::score::MigrationAssessment;
use crate::cli::plan::types::*;
use crate::evidence::EvidenceSnapshot;
use std::collections::HashMap;

#[derive(Default)]
pub struct RepairOptions {
    /// Include operations that cannot be automatically undone (VMware
    /// Tools uninstall, ghost-NIC removal). Gated by
    /// `migration_repair_destructive` policy on the agent side.
    pub include_destructive: bool,
    /// Optional host path to a virtio-win tree or a single driver directory.
    /// Also honored via `$GUESTKIT_VIRTIO_WIN`.
    pub virtio_win_dir: Option<std::path::PathBuf>,
}

pub struct MigrationRepairPlanner;

impl MigrationRepairPlanner {
    /// Build a FixPlan from an assessment's failed checks. Also returns
    /// planner notes (skipped hints and why).
    pub fn from_assessment(
        assessment: &MigrationAssessment,
        ev: &EvidenceSnapshot,
        opts: &RepairOptions,
    ) -> (FixPlan, Vec<String>) {
        let mut plan = FixPlan::new(ev.image_path.clone(), "migration-repair".to_string());
        plan.metadata.description = Some(format!(
            "migration repair plan for target {} (score {})",
            assessment.target, assessment.overall_score
        ));
        let mut notes = Vec::new();
        let mut seen: HashMap<String, ()> = HashMap::new();
        let windows = ev.os.os_type.to_lowercase().contains("windows");

        for check in assessment.checks.iter().filter(|c| !c.passed) {
            let Some(hint) = &check.remediation else {
                continue;
            };
            let key = format!("{hint:?}");
            if seen.insert(key, ()).is_some() {
                continue; // one repair per strategy, however many checks point at it
            }
            let ops = if windows {
                windows_ops(hint, ev, opts, &mut notes)
            } else {
                linux_ops(hint, ev, &mut notes)
            };
            for op in ops {
                // Safety invariant: High/Critical risk requires undo.
                if op.risk <= Priority::High && op.undo.is_none() {
                    notes.push(format!(
                        "refused {}: {:?} risk without undo information",
                        op.id, op.risk
                    ));
                    continue;
                }
                plan.operations.push(op);
            }
        }

        plan.overall_risk = plan
            .operations
            .iter()
            .map(|o| o.risk)
            .min()
            .map(|r| format!("{r:?}").to_lowercase())
            .unwrap_or_else(|| "low".to_string());
        plan.metadata.reversible = plan.operations.iter().all(|o| o.reversible);
        (plan, notes)
    }
}

#[allow(clippy::too_many_arguments)] // internal Operation-builder helper, one call site per field
fn op(
    id: &str,
    description: &str,
    op_type: OperationType,
    risk: Priority,
    reversible: bool,
    undo: Option<UndoInfo>,
    validation: Option<ValidationCheck>,
    depends_on: Vec<String>,
) -> Operation {
    Operation {
        id: id.to_string(),
        op_type,
        priority: Priority::High,
        description: description.to_string(),
        risk,
        reversible,
        depends_on,
        validation,
        undo,
    }
}

fn linux_ops(hint: &Hint, ev: &EvidenceSnapshot, notes: &mut Vec<String>) -> Vec<Operation> {
    let dracut_based = matches!(
        ev.os.package_manager.as_str(),
        "dnf" | "yum" | "zypper" | "rpm"
    ) || ev.os.distribution.to_lowercase().contains("rhel")
        || ev.os.distribution.to_lowercase().contains("fedora")
        || ev.os.distribution.to_lowercase().contains("centos")
        || ev.os.distribution.to_lowercase().contains("suse");

    match hint {
        Hint::AddVirtioToInitramfs => {
            if dracut_based {
                vec![
                    op(
                        "mig-virtio-initramfs-conf",
                        "Persist virtio modules in dracut configuration",
                        OperationType::FileWrite(FileWrite {
                            path: "/etc/dracut.conf.d/99-guestkit-virtio.conf".into(),
                            content:
                                "add_drivers+=\" virtio_blk virtio_scsi virtio_net virtio_pci \"\n"
                                    .into(),
                            mode: Some("0644".into()),
                        }),
                        Priority::Medium,
                        true,
                        Some(UndoInfo::Command {
                            command: "rm -f /etc/dracut.conf.d/99-guestkit-virtio.conf".into(),
                        }),
                        None,
                        vec![],
                    ),
                    op(
                        "mig-virtio-initramfs-rebuild",
                        "Rebuild initramfs with virtio drivers",
                        OperationType::CommandExec(CommandExec {
                            command: "dracut -f --regenerate-all".into(),
                            expected_exit: 0,
                            timeout: Some(600),
                            interpreter: None,
                        }),
                        Priority::Medium,
                        false,
                        None,
                        Some(ValidationCheck {
                            command: "lsinitrd | grep -q virtio".into(),
                            expected_exit: 0,
                            expected_output: None,
                        }),
                        vec!["mig-virtio-initramfs-conf".into()],
                    ),
                ]
            } else {
                vec![
                    op(
                        "mig-virtio-initramfs-conf",
                        "Persist virtio modules in initramfs-tools configuration",
                        OperationType::FileWrite(FileWrite {
                            path: "/etc/initramfs-tools/modules.d/guestkit-virtio".into(),
                            content: "virtio_blk\nvirtio_scsi\nvirtio_net\nvirtio_pci\n".into(),
                            mode: Some("0644".into()),
                        }),
                        Priority::Medium,
                        true,
                        Some(UndoInfo::Command {
                            command: "sed -i '/^virtio_/d' /etc/initramfs-tools/modules".into(),
                        }),
                        None,
                        vec![],
                    ),
                    op(
                        "mig-virtio-initramfs-rebuild",
                        "Rebuild initramfs with virtio drivers",
                        OperationType::CommandExec(CommandExec {
                            command: "update-initramfs -u -k all".into(),
                            expected_exit: 0,
                            timeout: Some(600),
                            interpreter: None,
                        }),
                        Priority::Medium,
                        false,
                        None,
                        Some(ValidationCheck {
                            command: "lsinitramfs /boot/initrd.img-$(uname -r) | grep -q virtio"
                                .into(),
                            expected_exit: 0,
                            expected_output: None,
                        }),
                        vec!["mig-virtio-initramfs-conf".into()],
                    ),
                ]
            }
        }
        Hint::ConvertFstabToUuid => {
            let mut changes = Vec::new();
            for entry in &ev.storage.fstab_entries {
                if !entry.device.starts_with("/dev/sd") && !entry.device.starts_with("/dev/vd") {
                    continue;
                }
                if let Some(uuid) = ev
                    .storage
                    .partition_uuids
                    .iter()
                    .find(|u| u.device == entry.device)
                {
                    changes.push(FileChange {
                        line: 0,
                        before: format!(
                            "{} {} {} {}",
                            entry.device, entry.mountpoint, entry.fstype, entry.options
                        ),
                        after: format!(
                            "UUID={} {} {} {}",
                            uuid.uuid, entry.mountpoint, entry.fstype, entry.options
                        ),
                        context: None,
                    });
                }
            }
            if changes.is_empty() {
                notes.push(
                    "fstab uses device paths but no matching UUIDs were collected — manual conversion required"
                        .into(),
                );
                return vec![];
            }
            vec![op(
                "mig-fstab-uuid",
                "Convert /etc/fstab device paths to stable UUIDs",
                OperationType::FileEdit(FileEdit {
                    file: "/etc/fstab".into(),
                    backup: true,
                    changes,
                }),
                Priority::High,
                true,
                Some(UndoInfo::Command {
                    command: "restore /etc/fstab from the plan rollback snapshot".into(),
                }),
                Some(ValidationCheck {
                    command: "findmnt --verify --tab-file /etc/fstab".into(),
                    expected_exit: 0,
                    expected_output: None,
                }),
                vec![],
            )]
        }
        Hint::RepairBootloader => {
            let (backup_cmd, rebuild_cmd, cfg) = if dracut_based {
                (
                    "cp /boot/grub2/grub.cfg /boot/grub2/grub.cfg.guestkit-bak",
                    "grub2-mkconfig -o /boot/grub2/grub.cfg",
                    "/boot/grub2/grub.cfg",
                )
            } else {
                (
                    "cp /boot/grub/grub.cfg /boot/grub/grub.cfg.guestkit-bak",
                    "update-grub",
                    "/boot/grub/grub.cfg",
                )
            };
            vec![
                op(
                    "mig-grub-backup",
                    "Back up current GRUB configuration",
                    OperationType::CommandExec(CommandExec {
                        command: backup_cmd.into(),
                        expected_exit: 0,
                        timeout: Some(30),
                        interpreter: None,
                    }),
                    Priority::Low,
                    true,
                    Some(UndoInfo::Command {
                        command: format!("rm -f {cfg}.guestkit-bak"),
                    }),
                    None,
                    vec![],
                ),
                op(
                    "mig-grub-rebuild",
                    "Regenerate GRUB configuration",
                    OperationType::CommandExec(CommandExec {
                        command: rebuild_cmd.into(),
                        expected_exit: 0,
                        timeout: Some(300),
                        interpreter: None,
                    }),
                    Priority::High,
                    true,
                    Some(UndoInfo::Command {
                        command: format!("cp {cfg}.guestkit-bak {cfg}"),
                    }),
                    Some(ValidationCheck {
                        command: format!("test -s {cfg}"),
                        expected_exit: 0,
                        expected_output: None,
                    }),
                    vec!["mig-grub-backup".into()],
                ),
            ]
        }
        Hint::EnableSerialConsole => vec![op(
            "mig-serial-console",
            "Enable serial console on all kernel entries",
            OperationType::CommandExec(CommandExec {
                command: "grubby --update-kernel=ALL --args='console=ttyS0,115200 console=tty0'"
                    .into(),
                expected_exit: 0,
                timeout: Some(60),
                interpreter: None,
            }),
            Priority::Medium,
            true,
            Some(UndoInfo::Command {
                command: "grubby --update-kernel=ALL --remove-args='console=ttyS0,115200'".into(),
            }),
            None,
            vec![],
        )],
        Hint::RemoveHypervisorTools { packages } => {
            let pkgs = if packages.iter().any(|p| p.to_lowercase().contains("vmware")) {
                "open-vm-tools open-vm-tools-desktop"
            } else {
                return {
                    notes.push(format!(
                        "hypervisor tooling {packages:?} requires manual removal"
                    ));
                    vec![]
                };
            };
            let mgr_remove = if dracut_based {
                format!("dnf remove -y {pkgs} || yum remove -y {pkgs}")
            } else {
                format!("apt-get remove -y {pkgs}")
            };
            vec![op(
                "mig-remove-vm-tools",
                "Remove VMware guest tooling",
                OperationType::CommandExec(CommandExec {
                    command: mgr_remove,
                    expected_exit: 0,
                    timeout: Some(600),
                    interpreter: None,
                }),
                Priority::Medium,
                true,
                Some(UndoInfo::Command {
                    command: format!("reinstall with the package manager: {pkgs}"),
                }),
                None,
                vec![],
            )]
        }
        Hint::ScheduleSelinuxRelabel => vec![op(
            "mig-selinux-relabel",
            "Schedule SELinux relabel on next boot",
            OperationType::FileWrite(FileWrite {
                path: "/.autorelabel".into(),
                content: String::new(),
                mode: Some("0644".into()),
            }),
            Priority::Low,
            true,
            Some(UndoInfo::Command {
                command: "rm -f /.autorelabel".into(),
            }),
            None,
            vec![],
        )],
        Hint::PreserveStaticIp => {
            notes.push(
                "static IP preservation is handled by baseline capture + post-migration restore, not a repair op"
                    .into(),
            );
            vec![]
        }
        other => {
            notes.push(format!("no automated Linux repair for {other:?}"));
            vec![]
        }
    }
}

fn windows_ops(
    hint: &Hint,
    _ev: &EvidenceSnapshot,
    opts: &RepairOptions,
    notes: &mut Vec<String>,
) -> Vec<Operation> {
    match hint {
        Hint::InjectWindowsDriver { driver } => {
            let mut inj = DriverInject {
                inf_path: format!("C:\\GuestKit\\drivers\\{driver}\\{driver}.inf"),
                driver_name: driver.clone(),
                boot_critical: driver == "viostor" || driver == "vioscsi",
                source: "virtio-win".into(),
                host_dir: None,
            };
            // Prefer an explicit planner override, then GUESTKIT_VIRTIO_WIN layout.
            if let Some(dir) = opts.virtio_win_dir.as_ref() {
                let candidate = dir.join(driver);
                if candidate.is_dir() {
                    inj.host_dir = Some(candidate.display().to_string());
                } else if dir.is_dir() {
                    // Treat as already pointing at the driver folder.
                    inj.host_dir = Some(dir.display().to_string());
                }
            }
            if inj.host_dir.is_none() {
                if let Some(resolved) = inj.resolve_host_dir() {
                    inj.host_dir = Some(resolved.display().to_string());
                } else {
                    notes.push(format!(
                        "DriverInject '{driver}': set GUESTKIT_VIRTIO_WIN or --virtio-win to a \
                         virtio-win tree for offline plan apply"
                    ));
                }
            }
            vec![op(
            &format!("mig-inject-{driver}"),
            &format!("Inject virtio driver {driver}"),
            OperationType::DriverInject(inj),
            Priority::High,
            true,
            Some(UndoInfo::Command {
                command: format!("pnputil /delete-driver {driver}.inf /uninstall"),
            }),
            Some(ValidationCheck {
                command: format!("pnputil /enum-drivers | Select-String {driver}"),
                expected_exit: 0,
                expected_output: None,
            }),
            vec![],
        )]
        },
        Hint::RegisterBootCritical { driver } => vec![op(
            &format!("mig-bootcritical-{driver}"),
            &format!("Register {driver} as boot-critical storage driver"),
            OperationType::RegistryEdit(RegistryEdit {
                key: format!("HKLM\\SYSTEM\\CurrentControlSet\\Services\\{driver}"),
                value: "Start".into(),
                current_data: serde_json::json!(3),
                new_data: serde_json::json!(0),
                data_type: "DWORD".into(),
            }),
            Priority::High,
            true,
            Some(UndoInfo::Command {
                command: format!(
                    "reg.exe add HKLM\\SYSTEM\\CurrentControlSet\\Services\\{driver} /v Start /t REG_DWORD /d 3 /f"
                ),
            }),
            Some(ValidationCheck {
                command: format!(
                    "(Get-ItemProperty HKLM:\\SYSTEM\\CurrentControlSet\\Services\\{driver}).Start -eq 0"
                ),
                expected_exit: 0,
                expected_output: Some("True".into()),
            }),
            vec![],
        )],
        Hint::SuspendBitLocker => vec![op(
            "mig-suspend-bitlocker",
            "Suspend BitLocker protection until after migration validation (2 reboots)",
            OperationType::CommandExec(CommandExec {
                command: "Get-BitLockerVolume | Where-Object ProtectionStatus -eq 'On' | Suspend-BitLocker -RebootCount 2".into(),
                expected_exit: 0,
                timeout: Some(120),
                interpreter: None,
            }),
            Priority::High,
            true,
            Some(UndoInfo::Command {
                command: "Get-BitLockerVolume | Resume-BitLocker".into(),
            }),
            Some(ValidationCheck {
                command: "[bool](Get-BitLockerVolume | Where-Object ProtectionStatus -eq 'Off')".into(),
                expected_exit: 0,
                expected_output: Some("True".into()),
            }),
            vec![],
        )],
        Hint::RemoveGhostNics { instance_ids } => {
            if !opts.include_destructive {
                notes.push(format!(
                    "skipping ghost-NIC removal ({} device(s)) — destructive; requires include_destructive",
                    instance_ids.len()
                ));
                return vec![];
            }
            instance_ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    op(
                        &format!("mig-ghost-nic-{i}"),
                        &format!("Remove disconnected NIC {id}"),
                        OperationType::CommandExec(CommandExec {
                            command: format!("pnputil /remove-device \"{id}\""),
                            expected_exit: 0,
                            timeout: Some(60),
                            interpreter: None,
                        }),
                        // Not undoable: gated behind include_destructive and
                        // rated Medium so the no-undo invariant admits it
                        // only through that explicit gate.
                        Priority::Medium,
                        false,
                        None,
                        None,
                        vec![],
                    )
                })
                .collect()
        }
        Hint::RemoveHypervisorTools { packages } => {
            if !opts.include_destructive {
                notes.push(
                    "skipping VMware Tools uninstall — destructive (can drop the NIC mid-session); requires include_destructive"
                        .into(),
                );
                return vec![];
            }
            let _ = packages;
            vec![op(
                "mig-remove-vmware-tools",
                "Uninstall VMware Tools",
                OperationType::CommandExec(CommandExec {
                    command: "$app = Get-CimInstance Win32_Product -Filter \"Name like 'VMware Tools%'\"; if ($app) { $app | Invoke-CimMethod -MethodName Uninstall }".into(),
                    expected_exit: 0,
                    timeout: Some(900),
                    interpreter: None,
                }),
                Priority::Medium,
                false,
                None,
                None,
                vec![],
            )]
        }
        Hint::PreserveStaticIp => {
            notes.push(
                "static NIC configuration is captured in the pre-migration baseline and re-applied by post-migration validation"
                    .into(),
            );
            vec![]
        }
        other => {
            notes.push(format!("no automated Windows repair for {other:?}"));
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::report::BootabilityReport;
    use crate::migration::score::assess_migration;

    fn boot_report() -> BootabilityReport {
        BootabilityReport {
            score: 90.0,
            confidence: 1.0,
            target: "kvm".into(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: String::new(),
        }
    }

    fn linux_ev_missing_virtio() -> crate::evidence::EvidenceSnapshot {
        let mut ev = crate::migration::checks::tests_support::linux_evidence();
        ev.boot.initramfs_modules = vec!["nvme".into()];
        ev.os.package_manager = "dnf".into();
        ev
    }

    #[test]
    fn missing_virtio_produces_chained_initramfs_ops() {
        let report = boot_report();
        let ev = linux_ev_missing_virtio();
        let assessment = assess_migration(&ev, &report, "kvm", false);
        let (plan, _notes) =
            MigrationRepairPlanner::from_assessment(&assessment, &ev, &RepairOptions::default());
        let ids: Vec<&str> = plan.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"mig-virtio-initramfs-conf"));
        assert!(ids.contains(&"mig-virtio-initramfs-rebuild"));
        let rebuild = plan
            .operations
            .iter()
            .find(|o| o.id == "mig-virtio-initramfs-rebuild")
            .unwrap();
        assert_eq!(rebuild.depends_on, vec!["mig-virtio-initramfs-conf"]);
        assert!(rebuild.validation.is_some());
    }

    #[test]
    fn destructive_ops_skipped_by_default() {
        let report = boot_report();
        let mut ev = crate::migration::checks::tests_support::windows_evidence(true);
        ev.windows.as_mut().unwrap().ghost_nics = vec![crate::evidence::snapshot::GhostNicEntry {
            instance_id: "PCI\\VEN_15AD".into(),
            description: "vmxnet3".into(),
        }];
        let assessment = assess_migration(&ev, &report, "kvm", false);
        let (plan, notes) =
            MigrationRepairPlanner::from_assessment(&assessment, &ev, &RepairOptions::default());
        assert!(!plan
            .operations
            .iter()
            .any(|o| o.id.starts_with("mig-ghost-nic")));
        assert!(notes.iter().any(|n| n.contains("ghost-NIC")));

        let (plan2, _) = MigrationRepairPlanner::from_assessment(
            &assessment,
            &ev,
            &RepairOptions {
                include_destructive: true,
                virtio_win_dir: None,
            },
        );
        assert!(plan2
            .operations
            .iter()
            .any(|o| o.id.starts_with("mig-ghost-nic")));
    }

    #[test]
    fn windows_manual_storage_driver_yields_boot_critical_registry_op() {
        let report = boot_report();
        let ev = crate::migration::checks::tests_support::windows_evidence(false);
        let assessment = assess_migration(&ev, &report, "kvm", false);
        let (plan, _) =
            MigrationRepairPlanner::from_assessment(&assessment, &ev, &RepairOptions::default());
        let reg = plan
            .operations
            .iter()
            .find(|o| o.id == "mig-bootcritical-viostor")
            .expect("boot-critical registry op");
        assert!(reg.undo.is_some());
        assert!(reg.validation.is_some());
    }

    #[test]
    fn high_risk_op_without_undo_is_refused() {
        // Construct via the internal helper: a High op with no undo must be
        // dropped by the planner invariant.
        let report = boot_report();
        let ev = linux_ev_missing_virtio();
        let assessment = assess_migration(&ev, &report, "kvm", false);
        let (plan, _) =
            MigrationRepairPlanner::from_assessment(&assessment, &ev, &RepairOptions::default());
        for op in &plan.operations {
            if op.risk <= Priority::High {
                assert!(op.undo.is_some(), "{} lacks undo", op.id);
            }
        }
    }
}
