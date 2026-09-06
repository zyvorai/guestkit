// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live evidence collection from a running guest (no guestfs).

use super::snapshot::*;
use crate::evidence::collectors::collect_systemd_live;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Collect evidence from the live running system.
pub fn build_evidence_live() -> Result<EvidenceSnapshot> {
    let os = collect_os_live();
    let storage = collect_storage_live();
    let boot = collect_boot_live();
    let network = collect_network_live();
    let packages = collect_packages_live();
    let security = collect_security_live();
    let vm_tools = collect_vm_tools_live();
    let systemd = collect_systemd_live().map(|mut info| {
        info.runtime = crate::collectors::dbus::collect_systemd_runtime_safe();
        info
    });
    let kubevirt = Some(crate::evidence::collectors::collect_kubevirt_live());
    let cloud_init = Some(crate::evidence::collectors::collect_cloud_init_live());
    let network_probes = Some(crate::evidence::collectors::collect_network_probes_live());
    let snapshot_readiness = Some(crate::evidence::collectors::collect_snapshot_readiness_live());
    let windows = crate::evidence::collectors::collect_windows_live();
    let process = Some(crate::collectors::process::collect_process_evidence());
    let hardware = Some(crate::collectors::hardware::collect_hardware_evidence());

    let linux_migration = if cfg!(target_os = "linux") {
        Some(collect_linux_migration_live())
    } else {
        None
    };

    let mut snapshot = EvidenceSnapshot {
        schema_version: SCHEMA_VERSION,
        image_path: "live".to_string(),
        collected_at: Utc::now().to_rfc3339(),
        root: "/".to_string(),
        os,
        storage,
        boot,
        network,
        packages,
        security,
        vm_tools,
        systemd,
        windows,
        kubevirt,
        cloud_init,
        network_probes,
        snapshot_readiness,
        process,
        hardware,
        linux_migration,
        online_cache: None,
    };
    enrich_migration_fields_live(&mut snapshot);
    Ok(snapshot)
}

/// Live collection of the schema-v4 migration fields.
fn enrich_migration_fields_live(snapshot: &mut EvidenceSnapshot) {
    let boot = &mut snapshot.boot;
    boot.firmware = if std::path::Path::new("/sys/firmware/efi").exists() {
        "uefi"
    } else {
        "bios"
    }
    .to_string();
    boot.secure_boot = detect_secure_boot_live();
    boot.serial_console_configured = fs::read_to_string("/proc/cmdline")
        .map(|c| c.contains("console=ttyS"))
        .unwrap_or(false);
    boot.initramfs_modules = list_initramfs_modules_live();

    // Root free space via statvfs-equivalent (sysinfo Disks; agent builds).
    #[cfg(feature = "sysinfo")]
    {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        snapshot.storage.free_space_root_mb = disks
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"))
            .map(|d| d.available_space() / (1024 * 1024));
    }

    // Routing table for drift comparison.
    if let Ok(out) = std::process::Command::new("ip").args(["route"]).output() {
        if out.status.success() {
            snapshot.network.routes = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::to_string)
                .collect();
        }
    }
}

fn detect_secure_boot_live() -> Option<bool> {
    // mokutil is authoritative when present.
    if let Ok(out) = std::process::Command::new("mokutil")
        .arg("--sb-state")
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("enabled") {
                return Some(true);
            }
            if text.contains("disabled") {
                return Some(false);
            }
        }
    }
    // Fallback: SecureBoot efivar's last data byte.
    let efivars = std::path::Path::new("/sys/firmware/efi/efivars");
    if efivars.exists() {
        if let Ok(entries) = fs::read_dir(efivars) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("SecureBoot-")
                {
                    if let Ok(data) = fs::read(entry.path()) {
                        return data.last().map(|b| *b == 1);
                    }
                }
            }
        }
    }
    None
}

/// Virtio-relevant module names bundled in the running kernel's initramfs.
fn list_initramfs_modules_live() -> Vec<String> {
    let release = fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let candidates = [
        format!("/boot/initramfs-{release}.img"),
        format!("/boot/initrd.img-{release}"),
        format!("/boot/initrd-{release}"),
    ];
    let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
        return Vec::new();
    };
    for lister in ["lsinitrd", "lsinitramfs"] {
        if let Ok(out) = std::process::Command::new(lister).arg(path).output() {
            if out.status.success() {
                let mut modules: Vec<String> = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter_map(|line| {
                        let file = line.rsplit([' ', '/']).next()?;
                        file.strip_suffix(".ko")
                            .or_else(|| file.strip_suffix(".ko.xz"))
                            .or_else(|| file.strip_suffix(".ko.zst"))
                            .or_else(|| file.strip_suffix(".ko.gz"))
                            .map(str::to_string)
                    })
                    .filter(|m| m.starts_with("virtio") || m.starts_with("nvme"))
                    .collect();
                modules.sort();
                modules.dedup();
                return modules;
            }
        }
    }
    Vec::new()
}

fn collect_linux_migration_live() -> crate::evidence::snapshot::LinuxMigrationEvidence {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let hypervisor_modules = fs::read_to_string("/proc/modules")
        .map(|content| {
            content
                .lines()
                .filter_map(|l| l.split_whitespace().next())
                .filter(|m| {
                    m.starts_with("vmw_")
                        || m.starts_with("vmxnet")
                        || m.starts_with("hv_")
                        || m.starts_with("xen")
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    // Files that pin static IP configuration (candidates for migration
    // network-preservation planning).
    type ScanDirCheck = (&'static str, fn(&str) -> bool);

    let mut static_ip_configs = Vec::new();
    let scan_dirs: [ScanDirCheck; 3] = [
        ("/etc/sysconfig/network-scripts", |c| {
            c.contains("BOOTPROTO=none") || c.contains("BOOTPROTO=static")
        }),
        ("/etc/netplan", |c| c.contains("addresses:")),
        ("/etc/NetworkManager/system-connections", |c| {
            c.contains("method=manual")
        }),
    ];
    for (dir, is_static) in scan_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if is_static(&content) {
                            static_ip_configs.push(path.display().to_string());
                        }
                    }
                }
            }
        }
    }

    crate::evidence::snapshot::LinuxMigrationEvidence {
        predictable_nic_names: Some(!cmdline.contains("net.ifnames=0")),
        static_ip_configs,
        hypervisor_modules,
    }
}

fn collect_os_live() -> OsEvidence {
    let mut os = OsEvidence::default();
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim_matches('"');
                match k {
                    "ID" => os.os_type = v.to_string(),
                    "NAME" if os.distribution.is_empty() => os.distribution = v.to_string(),
                    "VERSION_ID" => os.version = v.to_string(),
                    _ => {}
                }
            }
        }
    }
    if os.distribution.is_empty() {
        os.distribution = os.os_type.clone();
    }
    if let Ok(out) = Command::new("uname").arg("-m").output() {
        os.architecture = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    if let Ok(out) = Command::new("hostname").output() {
        os.hostname = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    if Path::new("/run/systemd/system").exists() || Path::new("/usr/lib/systemd").exists() {
        os.init_system = "systemd".to_string();
    }
    if Path::new("/usr/bin/dpkg").exists() {
        os.package_manager = "dpkg".to_string();
    } else if Path::new("/usr/bin/rpm").exists() {
        os.package_manager = "rpm".to_string();
    }
    os
}

fn collect_storage_live() -> StorageEvidence {
    let mut storage = StorageEvidence::default();
    if let Ok(content) = fs::read_to_string("/etc/fstab") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                storage.fstab_entries.push(FstabEntry {
                    device: parts[0].to_string(),
                    mountpoint: parts[1].to_string(),
                    fstype: parts[2].to_string(),
                    options: parts.get(3).unwrap_or(&"defaults").to_string(),
                });
                if parts[1] == "/" {
                    storage.root_filesystem = parts[2].to_string();
                }
            }
        }
    }
    if let Ok(content) = fs::read_to_string("/etc/crypttab") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                storage.crypttab_entries.push(CrypttabEntry {
                    name: parts[0].to_string(),
                    device: parts[1].to_string(),
                    keyfile: parts.get(2).unwrap_or(&"-").to_string(),
                });
            }
        }
    }
    if let Ok(out) = Command::new("lsblk")
        .args(["-J", "-o", "NAME,UUID,FSTYPE"])
        .output()
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(devices) = json["blockdevices"].as_array() {
                walk_lsblk(devices, &mut storage.partition_uuids);
            }
        }
    }
    storage
}

fn walk_lsblk(devices: &[serde_json::Value], out: &mut Vec<PartitionUuid>) {
    for dev in devices {
        if let (Some(name), Some(uuid), Some(fstype)) = (
            dev["name"].as_str(),
            dev["uuid"].as_str(),
            dev["fstype"].as_str(),
        ) {
            if !uuid.is_empty() && !fstype.is_empty() {
                out.push(PartitionUuid {
                    device: format!("/dev/{name}"),
                    uuid: uuid.to_string(),
                    fstype: fstype.to_string(),
                });
            }
        }
        if let Some(children) = dev["children"].as_array() {
            walk_lsblk(children, out);
        }
    }
}

fn collect_boot_live() -> BootEvidence {
    let mut boot = BootEvidence::default();
    if let Ok(cmdline) = fs::read_to_string("/proc/cmdline") {
        boot.kernel_cmdline = cmdline.trim().to_string();
    }
    if let Ok(modules) = fs::read_to_string("/proc/modules") {
        boot.loaded_modules = modules
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .map(|s| s.to_string())
            .collect();
    }
    boot.efi_present = Path::new("/sys/firmware/efi").exists();
    boot.pending_relabel = Path::new("/.autorelabel").exists();
    boot.cloud_init_present = Path::new("/etc/cloud").exists();
    for path in [
        "/boot/grub2/grub.cfg",
        "/boot/grub/grub.cfg",
        "/boot/efi/EFI/redhat/grub.cfg",
    ] {
        if Path::new(path).exists() {
            boot.grub_cfg_path = Some(path.to_string());
            boot.bootloader = "grub2".to_string();
            break;
        }
    }
    if let Ok(entries) = fs::read_dir("/boot") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("vmlinuz") {
                boot.kernel_paths.push(format!("/boot/{name}"));
            }
            if name.starts_with("initramfs") || name.starts_with("initrd") {
                boot.initramfs_paths.push(format!("/boot/{name}"));
            }
        }
    }
    boot
}

fn collect_network_live() -> NetworkEvidence {
    let mut network = NetworkEvidence::default();
    if let Ok(out) = Command::new("ip").args(["-j", "link"]).output() {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for iface in json {
                if let Some(name) = iface["ifname"].as_str() {
                    if name != "lo" {
                        network.interfaces.push(name.to_string());
                    }
                }
            }
        }
    }
    if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
        for line in content.lines() {
            if let Some(ip) = line.strip_prefix("nameserver ") {
                network.dns_servers.push(ip.trim().to_string());
            }
        }
    }
    let udev_dir = Path::new("/etc/udev/rules.d");
    if udev_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(udev_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("persistent-net") {
                    network.udev_persistent_net.push(name);
                }
            }
        }
    }
    crate::collectors::network_live::enrich_network_evidence(&mut network);
    network
}

fn collect_packages_live() -> PackageEvidence {
    let mut packages = PackageEvidence::default();
    if Path::new("/usr/bin/dpkg").exists() {
        if let Ok(out) = Command::new("dpkg-query")
            .args(["-W", "-f=${Package}\n"])
            .output()
        {
            let list: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            packages.count = list.len();
            packages.kernels = list
                .iter()
                .filter(|p| p.starts_with("linux-image") || p.starts_with("kernel"))
                .cloned()
                .collect();
            packages.sample_packages = list.into_iter().take(50).collect();
        }
    } else if Path::new("/usr/bin/rpm").exists() {
        if let Ok(out) = Command::new("rpm")
            .args(["-qa", "--qf", "%{NAME}\n"])
            .output()
        {
            let list: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            packages.count = list.len();
            packages.kernels = list
                .iter()
                .filter(|p| p.starts_with("kernel"))
                .cloned()
                .collect();
            packages.sample_packages = list.into_iter().take(50).collect();
        }
    }
    packages
}

fn collect_security_live() -> SecurityEvidence {
    let mut security = SecurityEvidence::default();
    if let Ok(out) = Command::new("getenforce").output() {
        security.selinux = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    security.apparmor = Path::new("/sys/kernel/security/apparmor").exists();
    if let Ok(out) = Command::new("systemctl")
        .args(["is-active", "firewalld"])
        .output()
    {
        security.firewall_enabled = String::from_utf8_lossy(&out.stdout).trim() == "active";
    }
    if let Ok(content) = fs::read_to_string("/etc/ssh/sshd_config") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("PermitRootLogin") {
                let val = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("prohibit-password");
                security.ssh_root_login = Some(!matches!(val, "no" | "prohibit-password"));
            }
        }
    }
    security.auditd = Path::new("/usr/sbin/auditd").exists()
        && Command::new("systemctl")
            .args(["is-active", "auditd"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or(false);
    security.open_ports = collect_listening_ports();
    security.pending_security_updates = detect_pending_security_updates();
    security
}

fn collect_listening_ports() -> Vec<u16> {
    let mut ports = Vec::new();
    if let Ok(out) = Command::new("ss").args(["-lntH"]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(col) = line.split_whitespace().nth(3) {
                if let Some(port) = col.rsplit(':').next().and_then(|p| p.parse().ok()) {
                    if !ports.contains(&port) {
                        ports.push(port);
                    }
                }
            }
        }
    }
    ports.sort_unstable();
    ports.truncate(32);
    ports
}

fn detect_pending_security_updates() -> bool {
    if Path::new("/usr/bin/apt").exists() {
        return Command::new("apt")
            .args(["list", "--upgradable"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .skip(1)
                    .any(|l| l.contains("security") || l.contains("-security"))
            })
            .unwrap_or(false);
    }
    if Path::new("/usr/bin/dnf").exists() {
        return Command::new("dnf")
            .args(["check-update", "--security", "-q"])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
    }
    false
}

fn collect_vm_tools_live() -> VmToolsEvidence {
    let mut detected = Vec::new();
    let probes = [
        ("vmware-tools", "/usr/bin/vmware-toolbox-cmd"),
        ("open-vm-tools", "/usr/bin/vmtoolsd"),
        ("qemu-guest-agent", "/usr/sbin/qemu-ga"),
        ("hyperv-daemons", "/usr/sbin/hv_kvp_daemon"),
        ("virtualbox-guest", "/usr/sbin/VBoxService"),
    ];
    for (name, path) in probes {
        if Path::new(path).exists() {
            detected.push(name.to_string());
        }
    }
    if Command::new("systemctl")
        .args(["is-active", "qemu-guest-agent"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false)
        && !detected.iter().any(|d| d.contains("qemu"))
    {
        detected.push("qemu-guest-agent".to_string());
    }
    VmToolsEvidence { detected }
}

/// Lightweight guest identity payload for Zeus VM Tools heartbeat.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel: String,
    pub architecture: String,
    pub ips: Vec<String>,
    pub boot_mode: String,
    pub cloud_init_present: bool,
    pub qga_ready: bool,
    pub zyvor_agent_ready: bool,
    pub virtio_modules_loaded: bool,
    pub agent_version: String,
    pub uptime_secs: u64,
    pub last_heartbeat: String,
}

pub fn build_agent_status_live() -> Result<AgentStatus> {
    let evidence = build_evidence_live()?;
    let ips = collect_live_ips();
    let kernel = evidence
        .boot
        .kernel_paths
        .first()
        .map(|p| p.rsplit('/').next().unwrap_or("unknown").to_string())
        .unwrap_or_else(|| {
            std::fs::read_to_string("/proc/version")
                .unwrap_or_default()
                .split_whitespace()
                .nth(2)
                .unwrap_or("unknown")
                .to_string()
        });
    let qga_ready = Path::new("/dev/virtio-ports/org.qemu.guest_agent.0").exists()
        && (Path::new("/usr/sbin/qemu-ga").exists()
            || Command::new("systemctl")
                .args(["is-active", "qemu-guest-agent"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false));
    let virtio_modules_loaded = evidence
        .boot
        .loaded_modules
        .iter()
        .any(|m| m.contains("virtio"));
    let boot_mode = if evidence.boot.efi_present {
        "uefi"
    } else {
        "bios"
    };
    let os_name = if evidence.os.distribution.is_empty() {
        evidence.os.os_type.clone()
    } else {
        evidence.os.distribution.clone()
    };
    Ok(AgentStatus {
        hostname: evidence.os.hostname,
        os_name,
        os_version: evidence.os.version,
        kernel,
        architecture: evidence.os.architecture,
        ips,
        boot_mode: boot_mode.into(),
        cloud_init_present: evidence.boot.cloud_init_present,
        qga_ready,
        zyvor_agent_ready: true,
        virtio_modules_loaded,
        agent_version: crate::VERSION.to_string(),
        uptime_secs: read_uptime_secs(),
        last_heartbeat: Utc::now().to_rfc3339(),
    })
}

pub fn collect_live_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(out) = Command::new("ip").args(["-j", "addr"]).output() {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for iface in json {
                if let Some(addrs) = iface["addr_info"].as_array() {
                    for addr in addrs {
                        if addr["family"].as_str() == Some("inet") {
                            if let Some(local) = addr["local"].as_str() {
                                if local != "127.0.0.1" {
                                    ips.push(local.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    ips
}

fn read_uptime_secs() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(String::from))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_evidence_on_host() {
        let evidence = build_evidence_live().expect("live evidence");
        assert_eq!(evidence.image_path, "live");
        assert_eq!(evidence.schema_version, SCHEMA_VERSION);
    }
}
