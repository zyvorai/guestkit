// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestKit evidence -> QEMU runtime planning.

use super::config::*;
use crate::boot::BootabilityReport;
use crate::evidence::EvidenceSnapshot;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GuestKitQemuOptions {
    pub name: Option<String>,
    pub memory_mb: u64,
    pub vcpus: u16,
    pub acceleration: Acceleration,
    pub disk_format: Option<DiskFormat>,
    pub disk_interface: DiskInterface,
    pub cache: CacheMode,
    pub discard: bool,
    pub network_backend: NetworkBackend,
    pub network_model: NetworkModel,
    pub mac: Option<String>,
    pub firmware: Option<Firmware>,
    pub console: Console,
    pub qmp_socket: Option<PathBuf>,
    pub vsock_cid: Option<u32>,
    pub daemonize: bool,
    pub pidfile: Option<PathBuf>,
    pub binary_override: Option<PathBuf>,
}

impl Default for GuestKitQemuOptions {
    fn default() -> Self {
        Self {
            name: None,
            memory_mb: 4096,
            vcpus: 2,
            acceleration: Acceleration::Kvm,
            disk_format: None,
            disk_interface: DiskInterface::VirtioBlk,
            cache: CacheMode::None,
            discard: true,
            network_backend: NetworkBackend::default(),
            network_model: NetworkModel::VirtioNet,
            mac: None,
            firmware: None,
            console: Console::Serial,
            qmp_socket: None,
            vsock_cid: None,
            daemonize: false,
            pidfile: None,
            binary_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestKitQemuPlan {
    pub evidence_schema: u32,
    pub image: PathBuf,
    pub guest_os: String,
    pub boot_score: f64,
    pub boot_confidence: f64,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub requires_uefi_firmware: bool,
    pub vm: QemuVm,
}

impl GuestKitQemuPlan {
    pub fn from_assurance(
        image: &Path,
        evidence: &EvidenceSnapshot,
        boot: &BootabilityReport,
        options: GuestKitQemuOptions,
    ) -> Result<Self> {
        if options.vcpus == 0 {
            return Err(QemuError::InvalidConfig(
                "vcpus must be greater than zero".into(),
            ));
        }
        let architecture = Architecture::from_guestkit(&evidence.os.architecture)?;
        let disk_format = match options.disk_format {
            Some(format) => format,
            None => DiskFormat::infer(image)?,
        };
        let name = options
            .name
            .unwrap_or_else(|| default_name(image, evidence));

        let mut vm = QemuVm::new(name, architecture);
        vm.acceleration = options.acceleration;
        vm.cpu = CpuConfig {
            sockets: 1,
            cores: options.vcpus,
            threads: 1,
            model: match options.acceleration {
                Acceleration::Kvm => CpuModel::Host,
                Acceleration::Tcg => CpuModel::Max,
            },
        };
        vm.memory.size_mb = options.memory_mb;
        vm.disks.push(Disk {
            id: "root".into(),
            path: image.to_path_buf(),
            format: disk_format,
            interface: options.disk_interface,
            readonly: false,
            cache: options.cache,
            discard: options.discard,
        });
        vm.networks.push(NetworkInterface {
            id: "net0".into(),
            backend: options.network_backend,
            model: options.network_model,
            mac: options.mac,
        });
        vm.devices.push(VirtioDevice::Balloon {
            id: "balloon0".into(),
        });
        vm.devices.push(VirtioDevice::Rng {
            id: "rng0".into(),
            source: PathBuf::from("/dev/urandom"),
        });
        if let Some(cid) = options.vsock_cid {
            vm.devices.push(VirtioDevice::Vsock {
                id: "vsock0".into(),
                cid,
            });
        }
        vm.firmware = options.firmware;
        vm.console = options.console;
        vm.qmp = options.qmp_socket.map(|socket| QmpEndpoint {
            socket,
            server: true,
            wait: false,
        });
        vm.daemonize = options.daemonize;
        vm.pidfile = options.pidfile;
        vm.binary_override = options.binary_override;
        vm.validate()?;

        let requires_uefi_firmware =
            evidence.boot.efi_present || evidence.boot.firmware.eq_ignore_ascii_case("uefi");
        let mut warnings = boot
            .warnings
            .iter()
            .map(|finding| format!("{}: {}", finding.title, finding.message))
            .collect::<Vec<_>>();

        if requires_uefi_firmware && vm.firmware.is_none() {
            warnings.push(
                "GuestKit detected a UEFI guest but no QEMU pflash firmware was configured; provide OVMF/AAVMF code and, when needed, a writable vars image"
                    .into(),
            );
        }
        if matches!(&vm.console, Console::Serial) && !evidence.boot.serial_console_configured {
            warnings.push(
                "GuestKit did not detect a configured serial console; -nographic may boot successfully but show no login/output"
                    .into(),
            );
        }
        if uses_virtio_storage(&vm)
            && evidence.os.os_type.eq_ignore_ascii_case("linux")
            && !evidence.boot.initramfs_modules.is_empty()
            && !has_virtio_storage_module(&evidence.boot.initramfs_modules)
        {
            warnings.push(
                "VirtIO storage is selected but GuestKit evidence does not show virtio_blk/virtio_scsi in the initramfs"
                    .into(),
            );
        }

        Ok(Self {
            evidence_schema: evidence.schema_version,
            image: image.to_path_buf(),
            guest_os: guest_os_label(evidence),
            boot_score: boot.score,
            boot_confidence: boot.confidence,
            blockers: boot
                .blockers
                .iter()
                .map(|finding| format!("{}: {}", finding.title, finding.message))
                .collect(),
            warnings,
            requires_uefi_firmware,
            vm,
        })
    }

    pub fn is_ready(&self, min_boot_score: f64) -> bool {
        self.blockers.is_empty()
            && self.boot_score >= min_boot_score
            && (!self.requires_uefi_firmware || self.vm.firmware.is_some())
    }

    pub fn enforce_ready(&self, min_boot_score: f64) -> Result<()> {
        if !self.blockers.is_empty() {
            return Err(QemuError::InvalidConfig(format!(
                "GuestKit found {} boot blocker(s); refusing QEMU start",
                self.blockers.len()
            )));
        }
        if self.boot_score < min_boot_score {
            return Err(QemuError::InvalidConfig(format!(
                "GuestKit boot assurance score {:.0} is below the required {:.0}",
                self.boot_score, min_boot_score
            )));
        }
        if self.requires_uefi_firmware && self.vm.firmware.is_none() {
            return Err(QemuError::InvalidConfig(
                "UEFI guest requires QEMU firmware configuration before start".into(),
            ));
        }
        Ok(())
    }
}

fn default_name(image: &Path, evidence: &EvidenceSnapshot) -> String {
    if !evidence.os.hostname.trim().is_empty() {
        return evidence.os.hostname.trim().to_string();
    }
    image
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("guestkit-vm")
        .to_string()
}

fn guest_os_label(evidence: &EvidenceSnapshot) -> String {
    let distro = evidence.os.distribution.trim();
    let version = evidence.os.version.trim();
    if distro.is_empty() {
        evidence.os.os_type.clone()
    } else if version.is_empty() {
        distro.to_string()
    } else {
        format!("{distro} {version}")
    }
}

fn uses_virtio_storage(vm: &QemuVm) -> bool {
    vm.disks.iter().any(|disk| {
        matches!(
            disk.interface,
            DiskInterface::VirtioBlk | DiskInterface::VirtioScsi
        )
    })
}

fn has_virtio_storage_module(modules: &[String]) -> bool {
    modules.iter().any(|module| {
        matches!(
            module.trim().replace('-', "_").as_str(),
            "virtio_blk" | "virtio_scsi" | "virtio_pci" | "virtio"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::report::Finding;

    fn evidence(architecture: &str, firmware: &str, efi: bool) -> EvidenceSnapshot {
        serde_json::from_value(serde_json::json!({
            "schema_version": 5,
            "image_path": "/tmp/test.qcow2",
            "collected_at": "2026-08-31T00:00:00Z",
            "root": "/dev/sda1",
            "os": {
                "os_type": "linux",
                "distribution": "Ubuntu",
                "version": "24.04",
                "architecture": architecture,
                "hostname": "guestkit-test",
                "init_system": "systemd",
                "package_manager": "apt"
            },
            "storage": {
                "fstab_entries": [],
                "crypttab_entries": [],
                "swap_devices": [],
                "root_filesystem": "ext4",
                "partition_uuids": []
            },
            "boot": {
                "bootloader": "grub",
                "default_entry": "",
                "kernel_cmdline": "",
                "kernel_paths": [],
                "initramfs_paths": [],
                "efi_present": efi,
                "grub_cfg_path": null,
                "loaded_modules": [],
                "pending_relabel": false,
                "cloud_init_present": false,
                "initramfs_modules": ["virtio_blk"],
                "firmware": firmware,
                "serial_console_configured": true
            },
            "network": {
                "interfaces": [],
                "dns_servers": [],
                "udev_persistent_net": []
            },
            "packages": { "count": 0, "kernels": [], "sample_packages": [] },
            "security": {
                "selinux": "disabled",
                "apparmor": false,
                "firewall_enabled": false,
                "ssh_root_login": null,
                "auditd": false
            },
            "vm_tools": { "detected": [] }
        }))
        .unwrap()
    }

    fn boot(score: f64) -> BootabilityReport {
        BootabilityReport {
            score,
            confidence: 0.95,
            target: "kvm".into(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: String::new(),
        }
    }

    #[test]
    fn aarch64_evidence_selects_virt_machine() {
        let plan = GuestKitQemuPlan::from_assurance(
            Path::new("/tmp/vm.qcow2"),
            &evidence("aarch64", "bios", false),
            &boot(92.0),
            GuestKitQemuOptions::default(),
        )
        .unwrap();
        assert_eq!(plan.vm.architecture, Architecture::Aarch64);
        assert_eq!(plan.vm.machine, MachineType::Virt);
    }

    #[test]
    fn uefi_without_firmware_is_not_ready() {
        let plan = GuestKitQemuPlan::from_assurance(
            Path::new("/tmp/vm.qcow2"),
            &evidence("x86_64", "uefi", true),
            &boot(95.0),
            GuestKitQemuOptions::default(),
        )
        .unwrap();
        assert!(plan.requires_uefi_firmware);
        assert!(!plan.is_ready(70.0));
        assert!(plan.enforce_ready(70.0).is_err());
    }

    #[test]
    fn blockers_gate_execution() {
        let mut report = boot(99.0);
        report.blockers.push(Finding {
            check_id: "BOOT-TEST".into(),
            title: "Missing boot disk".into(),
            message: "root device is unavailable".into(),
            remediation: None,
        });
        let plan = GuestKitQemuPlan::from_assurance(
            Path::new("/tmp/vm.qcow2"),
            &evidence("x86_64", "bios", false),
            &report,
            GuestKitQemuOptions::default(),
        )
        .unwrap();
        assert!(!plan.is_ready(70.0));
    }
}
