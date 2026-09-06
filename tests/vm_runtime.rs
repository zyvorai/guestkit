// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

#![cfg(not(target_os = "windows"))]

use guestkit::vm::{
    build_qemu_args, Architecture, DiskBus, ImageFormat, NetworkConfig, QmpClient, UefiConfig,
    VmDefinition, VM_DEFINITION_SCHEMA,
};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use tempfile::tempdir;

fn base_definition(image: PathBuf) -> VmDefinition {
    VmDefinition {
        schema_version: VM_DEFINITION_SCHEMA,
        name: "demo".into(),
        image,
        image_format: ImageFormat::Qcow2,
        architecture: Architecture::X86_64,
        memory_mb: 4096,
        vcpus: 2,
        disk_bus: DiskBus::VirtioBlk,
        network: NetworkConfig::User {
            ssh_port: Some(2222),
        },
        readonly: false,
        vsock_cid: Some(100),
        uefi_required: false,
        uefi: None,
        min_boot_score: 70.0,
    }
}

#[test]
fn image_format_inference() {
    assert_eq!(
        ImageFormat::infer(PathBuf::from("x.qcow2").as_path()).unwrap(),
        ImageFormat::Qcow2
    );
    assert_eq!(
        ImageFormat::infer(PathBuf::from("x.vmdk").as_path()).unwrap(),
        ImageFormat::Vmdk
    );
}

#[test]
fn qemu_args_keep_space_in_disk_path_inside_one_argv() {
    let tmp = tempdir().unwrap();
    let image = tmp.path().join("disk with spaces.qcow2");
    fs::write(&image, b"x").unwrap();
    let def = base_definition(image);
    let args = build_qemu_args(
        &def,
        &tmp.path().join("qmp.sock"),
        &tmp.path().join("vm.pid"),
        true,
    )
    .unwrap();
    assert!(args.iter().any(|v| v.contains("disk with spaces.qcow2")));
}

#[test]
fn user_ssh_forward_is_loopback_only() {
    let tmp = tempdir().unwrap();
    let image = tmp.path().join("disk.qcow2");
    fs::write(&image, b"x").unwrap();
    let def = base_definition(image);
    let args = build_qemu_args(
        &def,
        &tmp.path().join("qmp.sock"),
        &tmp.path().join("vm.pid"),
        true,
    )
    .unwrap();
    assert!(args
        .iter()
        .any(|v| v.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
}

#[test]
fn aarch64_uses_virt_machine_and_device_transport() {
    let tmp = tempdir().unwrap();
    let image = tmp.path().join("disk.qcow2");
    fs::write(&image, b"x").unwrap();
    let mut def = base_definition(image);
    def.architecture = Architecture::Aarch64;
    let args = build_qemu_args(
        &def,
        &tmp.path().join("qmp.sock"),
        &tmp.path().join("vm.pid"),
        true,
    )
    .unwrap();
    assert!(args.iter().any(|v| v == "virt,accel=kvm"));
    assert!(args.iter().any(|v| v.starts_with("virtio-blk-device")));
}

#[test]
fn virtio_scsi_uses_one_controller() {
    let tmp = tempdir().unwrap();
    let image = tmp.path().join("disk.qcow2");
    fs::write(&image, b"x").unwrap();
    let mut def = base_definition(image);
    def.disk_bus = DiskBus::VirtioScsi;
    let args = build_qemu_args(
        &def,
        &tmp.path().join("qmp.sock"),
        &tmp.path().join("vm.pid"),
        true,
    )
    .unwrap();
    assert_eq!(args.iter().filter(|v| v.contains("id=gk_scsi0")).count(), 1);
    assert!(args.iter().any(|v| v.contains("scsi-hd")));
}

#[test]
fn uefi_requires_real_files() {
    let tmp = tempdir().unwrap();
    let image = tmp.path().join("disk.qcow2");
    fs::write(&image, b"x").unwrap();
    let mut def = base_definition(image);
    def.uefi_required = true;
    def.uefi = Some(UefiConfig {
        code: tmp.path().join("missing-code.fd"),
        vars: tmp.path().join("missing-vars.fd"),
    });
    assert!(build_qemu_args(
        &def,
        &tmp.path().join("qmp.sock"),
        &tmp.path().join("vm.pid"),
        true
    )
    .is_err());
}

#[test]
fn qmp_ignores_async_events() {
    let tmp = tempdir().unwrap();
    let socket = tmp.path().join("qmp.sock");
    let listener = UnixListener::bind(&socket).unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        writeln!(stream, "{}", json!({"QMP":{"version":{"qemu":{"major":9,"minor":0,"micro":0},"package":""},"capabilities":[]}})).unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(line.trim()).unwrap()["execute"],
            "qmp_capabilities"
        );
        writeln!(stream, "{}", json!({"return":{}})).unwrap();

        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(line.trim()).unwrap()["execute"],
            "query-status"
        );
        writeln!(
            stream,
            "{}",
            json!({"event":"STOP","data":{},"timestamp":{"seconds":1,"microseconds":1}})
        )
        .unwrap();
        writeln!(
            stream,
            "{}",
            json!({"return":{"running":true,"status":"running"}})
        )
        .unwrap();
    });

    let mut qmp = QmpClient::connect(&socket).unwrap();
    let status = qmp.query_status().unwrap();
    assert!(status.running);
    assert_eq!(status.status, "running");
    server.join().unwrap();
}
