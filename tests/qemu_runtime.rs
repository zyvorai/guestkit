// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use guestkit::qemu::{
    Architecture, CacheMode, Disk, DiskFormat, DiskInterface, NetworkBackend, NetworkInterface,
    NetworkModel, QemuVm, VirtioDevice,
};
use std::path::PathBuf;

#[test]
fn qemu_vm_json_round_trip_preserves_runtime_definition() {
    let mut vm = QemuVm::new("roundtrip", Architecture::X86_64);
    vm.disks.push(Disk {
        id: "root".into(),
        path: PathBuf::from("/var/lib/guestkit/root.qcow2"),
        format: DiskFormat::Qcow2,
        interface: DiskInterface::VirtioBlk,
        readonly: false,
        cache: CacheMode::None,
        discard: true,
    });
    vm.networks.push(NetworkInterface {
        id: "net0".into(),
        backend: NetworkBackend::default(),
        model: NetworkModel::VirtioNet,
        mac: None,
    });
    vm.devices.push(VirtioDevice::Balloon {
        id: "balloon0".into(),
    });

    let encoded = serde_json::to_string(&vm).unwrap();
    let decoded: QemuVm = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, vm);
}

#[test]
fn qemu_command_uses_argument_vector_not_shell_tokenization() {
    let mut vm = QemuVm::new("argv-test", Architecture::X86_64);
    vm.disks.push(Disk {
        id: "root".into(),
        path: PathBuf::from("/var/lib/guestkit/a disk with spaces.qcow2"),
        format: DiskFormat::Qcow2,
        interface: DiskInterface::VirtioBlk,
        readonly: false,
        cache: CacheMode::None,
        discard: false,
    });

    let spec = vm.command_spec().unwrap();
    assert!(spec.args.iter().any(|arg| {
        arg.to_string_lossy()
            .contains("file=/var/lib/guestkit/a disk with spaces.qcow2")
    }));
}
