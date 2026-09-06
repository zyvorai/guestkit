// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline initramfs virtio drop-in. Rebuild happens at first boot.

use super::types::*;

pub fn virtio_initramfs_plan(vm: &str, dracut: bool) -> FixPlan {
    let mut plan = FixPlan::new(vm.to_string(), "virtio-initramfs".into());
    plan.version = "1".into();
    plan.overall_risk = "medium".into();
    plan.estimated_duration = "seconds".into();
    plan.metadata.author = "guestkit".into();
    plan.metadata.review_required = true;
    plan.metadata.reversible = true;
    plan.metadata.description = Some("Offline virtio modules for next initramfs rebuild".into());
    plan.metadata.tags = vec!["linux".into(), "initramfs".into(), "virtio".into()];

    if dracut {
        plan.add_operation(Operation {
            id: "dracut-virtio".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: "/etc/dracut.conf.d/99-guestkit-virtio.conf".into(),
                content: "add_drivers+=\" virtio_blk virtio_scsi virtio_net virtio_pci \"\n".into(),
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: "Persist virtio drivers in dracut config".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: Some(UndoInfo::Command {
                command: "rm -f /etc/dracut.conf.d/99-guestkit-virtio.conf".into(),
            }),
        });
    } else {
        plan.add_operation(Operation {
            id: "initramfs-tools-virtio".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: "/etc/initramfs-tools/modules.d/guestkit-virtio".into(),
                content: "virtio_blk\nvirtio_scsi\nvirtio_net\nvirtio_pci\n".into(),
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: "Persist virtio modules for initramfs-tools".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: Some(UndoInfo::Command {
                command: "rm -f /etc/initramfs-tools/modules.d/guestkit-virtio".into(),
            }),
        });
    }
    plan.add_operation(Operation {
        id: "rebuild-flag".into(),
        op_type: OperationType::FileWrite(FileWrite {
            path: "/GuestKit/rebuild-initramfs.flag".into(),
            content: "1\n".into(),
            mode: Some("0644".into()),
        }),
        priority: Priority::Medium,
        description: "Arm first-boot initramfs rebuild (dracut -f or update-initramfs -u)".into(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![],
        validation: None,
        undo: Some(UndoInfo::Command {
            command: "rm -f /GuestKit/rebuild-initramfs.flag".into(),
        }),
    });
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dracut_plan_writes_conf() {
        let p = virtio_initramfs_plan("d.qcow2", true);
        assert!(p.operations.iter().any(|o| o.id == "dracut-virtio"));
        assert!(p.operations.iter().any(|o| o.id == "rebuild-flag"));
    }
}
