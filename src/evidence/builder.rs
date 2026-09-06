// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Build evidence snapshots from Guestfs inspection.

use super::snapshot::*;
use crate::evidence::collectors::{collect_systemd_guest, collect_windows_details};
use crate::Guestfs;
use anyhow::Result;
use chrono::Utc;
use std::path::Path;

pub struct EvidenceBuilder;

/// Collect a full evidence snapshot from a mounted guestfs instance.
pub fn build_evidence(g: &mut Guestfs, root: &str, image_path: &Path) -> Result<EvidenceSnapshot> {
    EvidenceBuilder::build(g, root, image_path)
}

impl EvidenceBuilder {
    pub fn build(g: &mut Guestfs, root: &str, image_path: &Path) -> Result<EvidenceSnapshot> {
        let os = Self::collect_os(g, root);
        let storage = Self::collect_storage(g, root);
        let boot = Self::collect_boot(g, root);
        let network = Self::collect_network(g, root);
        let packages = Self::collect_packages(g, root);
        let security = Self::collect_security(g, root);
        let vm_tools = Self::collect_vm_tools(g, root);
        let systemd = collect_systemd_guest(g, &os.init_system);
        let windows = if os.os_type.to_lowercase().contains("windows") {
            Some(Self::collect_windows(g, root))
        } else {
            None
        };

        let mut snapshot = EvidenceSnapshot {
            schema_version: SCHEMA_VERSION,
            image_path: image_path.display().to_string(),
            collected_at: Utc::now().to_rfc3339(),
            root: root.to_string(),
            os,
            storage,
            boot,
            network,
            packages,
            security,
            vm_tools,
            systemd,
            windows,
            kubevirt: None,
            cloud_init: None,
            network_probes: None,
            snapshot_readiness: None,
            process: None,
            hardware: None,
            linux_migration: None,
            online_cache: Self::read_online_cache(g),
        };
        Self::derive_migration_fields(&mut snapshot);
        Ok(snapshot)
    }

    /// Offline read of the live agent's inventory cache (spec §31): if the
    /// powered-off image contains a cache written by a previously-running
    /// agent, surface its last-known running state after integrity check.
    fn read_online_cache(g: &mut Guestfs) -> Option<serde_json::Value> {
        for path in [
            "/var/lib/guestkit/inventory.snapshot",
            "/ProgramData/Zyvor/GuestKit/inventory.snapshot",
        ] {
            let Ok(content) = g.read_file(path) else {
                continue;
            };
            let Ok(cache) = serde_json::from_slice::<serde_json::Value>(content.as_ref()) else {
                continue;
            };
            // Verify integrity_sha256 over payload before trusting it.
            let (Some(payload), Some(claimed)) = (
                cache.get("payload"),
                cache.get("integrity_sha256").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            use sha2::{Digest, Sha256};
            let canonical = serde_json::to_vec(payload).unwrap_or_default();
            let mut hasher = Sha256::new();
            hasher.update(&canonical);
            let computed: String = hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if computed == claimed {
                return Some(cache);
            }
            log::warn!("offline inventory cache at {path} failed integrity check");
        }
        None
    }

    /// Fill schema-v4 migration fields derivable from already-collected
    /// evidence (no extra guestfs round trips).
    fn derive_migration_fields(snapshot: &mut EvidenceSnapshot) {
        let boot = &mut snapshot.boot;
        if boot.firmware.is_empty() {
            boot.firmware = if boot.efi_present { "uefi" } else { "bios" }.to_string();
        }
        boot.serial_console_configured = boot.kernel_cmdline.contains("console=ttyS");

        if !snapshot.os.os_type.to_lowercase().contains("windows") {
            let hypervisor_modules: Vec<String> = boot
                .loaded_modules
                .iter()
                .filter(|m| {
                    m.starts_with("vmw_")
                        || m.starts_with("vmxnet")
                        || m.starts_with("hv_")
                        || m.starts_with("xen")
                })
                .cloned()
                .collect();
            let predictable = Some(!boot.kernel_cmdline.contains("net.ifnames=0"));
            snapshot.linux_migration = Some(crate::evidence::snapshot::LinuxMigrationEvidence {
                predictable_nic_names: predictable,
                static_ip_configs: Vec::new(),
                hypervisor_modules,
            });
        }
    }

    fn collect_os(g: &mut Guestfs, root: &str) -> OsEvidence {
        let version = match (
            g.inspect_get_major_version(root),
            g.inspect_get_minor_version(root),
        ) {
            (Ok(major), Ok(minor)) => format!("{}.{}", major, minor),
            _ => String::new(),
        };

        OsEvidence {
            os_type: g.inspect_get_type(root).unwrap_or_default(),
            distribution: g.inspect_get_distro(root).unwrap_or_default(),
            version,
            architecture: g.inspect_get_arch(root).unwrap_or_default(),
            hostname: g.inspect_get_hostname(root).unwrap_or_default(),
            init_system: g.inspect_get_init_system(root).unwrap_or_default(),
            package_manager: g.inspect_get_package_management(root).unwrap_or_default(),
        }
    }

    fn collect_storage(g: &mut Guestfs, root: &str) -> StorageEvidence {
        let fstab_content = g
            .read_file("/etc/fstab")
            .ok()
            .map(|c| String::from_utf8_lossy(&c).into_owned())
            .unwrap_or_default();

        let fstab_entries: Vec<FstabEntry> = g
            .inspect_fstab(root)
            .unwrap_or_default()
            .into_iter()
            .map(|(device, mountpoint, fstype)| {
                let options = fstab_content
                    .lines()
                    .find(|l| l.contains(&device) && l.contains(&mountpoint))
                    .and_then(|l| l.split_whitespace().nth(3))
                    .unwrap_or("")
                    .to_string();
                FstabEntry {
                    device,
                    mountpoint,
                    fstype,
                    options,
                }
            })
            .collect();

        let crypttab_entries = Self::parse_crypttab(g);
        let swap_devices = g.inspect_swap(root).unwrap_or_default();
        let root_filesystem = fstab_entries
            .iter()
            .find(|e| e.mountpoint == "/")
            .map(|e| e.fstype.clone())
            .unwrap_or_default();

        let partition_uuids = Self::collect_partition_uuids(g, root);

        StorageEvidence {
            fstab_entries,
            crypttab_entries,
            swap_devices,
            root_filesystem,
            partition_uuids,
            free_space_root_mb: None,
            boot_disk: None,
            disk_controller: None,
        }
    }

    fn parse_crypttab(g: &mut Guestfs) -> Vec<CrypttabEntry> {
        let mut entries = Vec::new();
        if let Ok(content) = g.read_file("/etc/crypttab") {
            let text = String::from_utf8_lossy(&content);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    entries.push(CrypttabEntry {
                        name: parts[0].to_string(),
                        device: parts[1].to_string(),
                        keyfile: parts.get(2).unwrap_or(&"none").to_string(),
                    });
                }
            }
        }
        entries
    }

    fn collect_partition_uuids(g: &mut Guestfs, _root: &str) -> Vec<PartitionUuid> {
        let mut uuids = Vec::new();
        if let Ok(blockdevs) = g.list_filesystems() {
            for (device, fstype) in blockdevs {
                if let Ok(output) = g.command(&["blkid", "-o", "export", &device]) {
                    let mut uuid = String::new();
                    for line in output.lines() {
                        if let Some(val) = line.strip_prefix("UUID=") {
                            uuid = val.to_string();
                            break;
                        }
                    }
                    if !uuid.is_empty() {
                        uuids.push(PartitionUuid {
                            device,
                            uuid,
                            fstype,
                        });
                    }
                }
            }
        }
        uuids
    }

    fn collect_boot(g: &mut Guestfs, root: &str) -> BootEvidence {
        let boot_config = g.inspect_boot_config(root).unwrap_or_default();
        let kernel_paths = Self::list_boot_files(g, "/boot", "vmlinuz");
        let initramfs_paths = Self::list_boot_files(g, "/boot", "initrd")
            .into_iter()
            .chain(Self::list_boot_files(g, "/boot", "initramfs"))
            .collect();

        let efi_present = g.exists("/boot/efi").unwrap_or(false)
            || g.exists("/sys/firmware/efi").unwrap_or(false);

        let grub_cfg_path = ["/boot/grub/grub.cfg", "/boot/grub2/grub.cfg"]
            .iter()
            .find(|p| g.exists(p).unwrap_or(false))
            .map(|s| s.to_string());

        let loaded_modules = g
            .read_file("/proc/modules")
            .ok()
            .map(|content| {
                String::from_utf8_lossy(&content)
                    .lines()
                    .filter_map(|l| l.split_whitespace().next())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_else(|| Self::list_module_names(g));

        let pending_relabel = g.exists("/.autorelabel").unwrap_or(false);
        let cloud_init_present = g.exists("/etc/cloud").unwrap_or(false);

        BootEvidence {
            bootloader: boot_config.bootloader,
            default_entry: boot_config.default_entry,
            kernel_cmdline: boot_config.kernel_cmdline,
            kernel_paths,
            initramfs_paths,
            efi_present,
            grub_cfg_path,
            loaded_modules,
            pending_relabel,
            cloud_init_present,
            initramfs_modules: Vec::new(),
            secure_boot: None,
            firmware: String::new(),
            serial_console_configured: false,
        }
    }

    fn list_boot_files(g: &mut Guestfs, dir: &str, prefix: &str) -> Vec<String> {
        let mut paths = Vec::new();
        if let Ok(entries) = g.ls(dir) {
            for entry in entries {
                if entry.contains(prefix) {
                    paths.push(format!("{}/{}", dir, entry));
                }
            }
        }
        paths
    }

    fn list_module_names(g: &mut Guestfs) -> Vec<String> {
        let mut modules = Vec::new();
        if let Ok(entries) = g.ls("/lib/modules") {
            if let Some(latest) = entries.iter().max() {
                let mod_dir = format!("/lib/modules/{}/kernel", latest);
                Self::walk_modules(g, &mod_dir, &mut modules);
            }
        }
        modules
    }

    fn walk_modules(g: &mut Guestfs, dir: &str, out: &mut Vec<String>) {
        if let Ok(entries) = g.ls(dir) {
            for entry in entries {
                let path = format!("{}/{}", dir, entry);
                if entry.ends_with(".ko") || entry.ends_with(".ko.xz") {
                    out.push(
                        entry
                            .trim_end_matches(".ko.xz")
                            .trim_end_matches(".ko")
                            .to_string(),
                    );
                } else if g.is_dir(&path).unwrap_or(false) {
                    Self::walk_modules(g, &path, out);
                }
            }
        }
    }

    fn collect_network(g: &mut Guestfs, root: &str) -> NetworkEvidence {
        let interfaces = g
            .inspect_network(root)
            .unwrap_or_default()
            .into_iter()
            .map(|i| format!("{}:{}", i.name, i.ip_address.join(",")))
            .collect();
        let dns_servers = g.inspect_dns(root).unwrap_or_default();

        let mut udev_persistent_net = Vec::new();
        if let Ok(content) = g.read_file("/etc/udev/rules.d/70-persistent-net.rules") {
            let text = String::from_utf8_lossy(&content);
            for line in text.lines() {
                if line.contains("NAME=") {
                    udev_persistent_net.push(line.to_string());
                }
            }
        }

        NetworkEvidence {
            interfaces,
            dns_servers,
            udev_persistent_net,
            ..Default::default()
        }
    }

    fn collect_packages(g: &mut Guestfs, root: &str) -> PackageEvidence {
        let pkg_info = g.inspect_packages(root).unwrap_or_default();
        let sample: Vec<String> = pkg_info
            .packages
            .iter()
            .take(50)
            .map(|p| format!("{}={}", p.name, p.version))
            .collect();
        let kernels: Vec<String> = pkg_info
            .packages
            .iter()
            .filter(|p| p.name.starts_with("linux-image") || p.name.starts_with("kernel"))
            .map(|p| format!("{}={}", p.name, p.version))
            .collect();

        PackageEvidence {
            count: pkg_info.packages.len(),
            kernels,
            sample_packages: sample,
        }
    }

    fn collect_security(g: &mut Guestfs, root: &str) -> SecurityEvidence {
        let sec = g.inspect_security(root).unwrap_or_default();
        let firewall_enabled = g.inspect_firewall(root).map(|f| f.enabled).unwrap_or(false);

        let ssh_root_login = g.inspect_ssh_config(root).ok().and_then(|cfg| {
            match cfg.get("PermitRootLogin").map(String::as_str) {
                Some("yes") | Some("without-password") | Some("prohibit-password") => Some(true),
                Some("no") => Some(false),
                _ => None,
            }
        });

        SecurityEvidence {
            selinux: sec.selinux,
            apparmor: sec.apparmor,
            firewall_enabled,
            ssh_root_login,
            auditd: sec.auditd,
            open_ports: Vec::new(),
            pending_security_updates: false,
        }
    }

    fn collect_vm_tools(g: &mut Guestfs, root: &str) -> VmToolsEvidence {
        VmToolsEvidence {
            detected: g.inspect_vm_tools(root).unwrap_or_default(),
        }
    }

    fn collect_windows(g: &mut Guestfs, root: &str) -> WindowsEvidence {
        use crate::guestfs::windows_registry;
        use std::path::PathBuf;

        let systemroot = g
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());

        let software_hive_guest = format!("{}/System32/config/SOFTWARE", systemroot);
        let system_hive_guest = format!("{}/System32/config/SYSTEM", systemroot);
        let software_hive = g
            .resolve_guest_path(&software_hive_guest)
            .unwrap_or_else(|_| PathBuf::from(&software_hive_guest));
        let system_hive = g
            .resolve_guest_path(&system_hive_guest)
            .unwrap_or_else(|_| PathBuf::from(&system_hive_guest));

        let installed_apps_count =
            windows_registry::parse_installed_software(software_hive.as_path())
                .map(|a| a.len())
                .unwrap_or(0);

        let services_count = windows_registry::parse_windows_services(system_hive.as_path())
            .map(|s| s.len())
            .unwrap_or(0);

        let drivers_path = format!("{}/System32/drivers", systemroot);
        let drivers_count = g.ls(&drivers_path).map(|d| d.len()).unwrap_or(0);

        let (product_name, version) =
            windows_registry::get_windows_version(software_hive.as_path())
                .map(|(n, v, _)| (n, v))
                .unwrap_or_else(|_| (String::new(), String::new()));

        let domain_info = windows_registry::parse_domain_info(system_hive.as_path());
        let rdp_enabled = windows_registry::parse_rdp_enabled(system_hive.as_path());
        let pending_reboot = windows_registry::parse_pending_reboot(system_hive.as_path());
        let dollar_bitlocker = g.exists("/$BitLocker").unwrap_or(false);
        let fvevol_sys = g
            .exists(&format!("{}/System32/drivers/fvevol.sys", systemroot))
            .unwrap_or(false);
        let bitlocker = windows_registry::collect_bitlocker_offline(
            software_hive.as_path(),
            system_hive.as_path(),
            dollar_bitlocker,
            fvevol_sys,
        );
        let bitlocker_detected = bitlocker
            .as_ref()
            .map(|b| b.any_protected || b.offline_uncertain || !b.volumes.is_empty())
            .unwrap_or(false)
            || dollar_bitlocker
            || fvevol_sys;

        let svi_present = g.exists("/System Volume Information").unwrap_or(false)
            || g.exists(&format!("{}/System Volume Information", systemroot))
                .unwrap_or(false);
        let vss = windows_registry::collect_vss_offline(system_hive.as_path(), svi_present);
        let hypervisor_remnants =
            windows_registry::detect_hypervisor_remnants(system_hive.as_path(), &drivers_path, g);
        let av_edr = windows_registry::detect_av_edr(software_hive.as_path(), g, &systemroot);
        let minidump_path = format!("{}/Minidump", systemroot);
        let minidump_count = g.ls(&minidump_path).map(|d| d.len()).unwrap_or(0);

        let bcd_candidates = [
            // UEFI: BCD on the EFI System Partition
            "/EFI/Microsoft/Boot/BCD".to_string(),
            format!("{}/Boot/EFI/BCD", systemroot),
            "/Windows/Boot/EFI/BCD".to_string(),
            // Legacy BIOS: BCD lives at \Boot\BCD on the system/boot volume
            // (a dedicated "System Reserved" partition, or the root of the
            // Windows volume on single-partition installs).
            "/Boot/BCD".to_string(),
        ];
        let mut bcd_store_found = bcd_candidates
            .iter()
            .any(|path| g.exists(path).unwrap_or(false));
        let bootmgr_candidates = [
            // UEFI boot manager
            "/EFI/Microsoft/Boot/bootmgfw.efi".to_string(),
            format!("{}/Boot/EFI/bootmgfw.efi", systemroot),
            // Legacy BIOS boot manager at the volume root
            "/bootmgr".to_string(),
        ];
        let mut bootmgr_found = bootmgr_candidates
            .iter()
            .any(|path| g.exists(path).unwrap_or(false));

        let system_reserved = Self::detect_system_reserved_partition(g, root);
        if let Some(ref sr) = system_reserved {
            if sr.has_bcd {
                bcd_store_found = true;
            }
            if sr.has_bootmgr {
                bootmgr_found = true;
            }
        }

        let esp_present = system_reserved
            .as_ref()
            .map(|s| s.role == "esp")
            .or_else(|| {
                Some(
                    g.exists("/EFI/Microsoft/Boot").unwrap_or(false)
                        || g.exists("/EFI/BOOT").unwrap_or(false),
                )
            });

        let details = collect_windows_details(g, root);

        let mut virtio_drivers = Self::derive_virtio_drivers(&details.services);
        Self::enrich_virtio_sys_presence(g, &systemroot, &mut virtio_drivers);

        let (hotfixes, hotfix_count, hf_mig_present) =
            Self::collect_windows_hotfixes(g, &systemroot, software_hive.as_path());

        let driver_signature_enforcement =
            Self::probe_driver_signature_enforcement(g, &bcd_candidates);

        WindowsEvidence {
            systemroot: systemroot.clone(),
            product_name,
            version,
            domain_joined: domain_info.0,
            domain_name: domain_info.1,
            rdp_enabled,
            pending_reboot,
            bitlocker_detected,
            installed_apps_count,
            services_count,
            drivers_count,
            hypervisor_remnants,
            av_edr,
            minidump_count,
            bcd_store_found,
            bootmgr_found,
            system_reserved,
            services: details.services,
            installed_apps: details.installed_apps,
            persistence: details.persistence,
            event_logs: details.event_logs,
            virtio_drivers,
            hotfixes,
            hotfix_count,
            hf_mig_present,
            bitlocker,
            vss,
            ghost_nics: windows_registry::detect_ghost_nics(system_hive.as_path()),
            static_nic_configs: windows_registry::parse_static_nic_configs(system_hive.as_path()),
            driver_signature_enforcement,
            esp_present,
            activation: {
                let mut activation =
                    windows_registry::parse_activation_info(software_hive.as_path());
                let oeminfo = g
                    .exists(&format!("{}/System32/oeminfo.ini", systemroot))
                    .unwrap_or(false);
                if oeminfo {
                    match &mut activation {
                        Some(a)
                            if a.channel.is_empty() || a.channel.eq_ignore_ascii_case("Retail") =>
                        {
                            a.channel = "OEM".into();
                        }
                        None => {
                            activation = Some(crate::evidence::snapshot::ActivationInfo {
                                licensed: false,
                                channel: "OEM".into(),
                                product_id: None,
                                edition_id: None,
                            });
                        }
                        _ => {}
                    }
                }
                activation
            },
        }
    }

    fn enrich_virtio_sys_presence(
        g: &mut Guestfs,
        systemroot: &str,
        drivers: &mut [crate::evidence::snapshot::WindowsDriverEntry],
    ) {
        for d in drivers.iter_mut() {
            let sys = format!("{}/System32/drivers/{}.sys", systemroot, d.name);
            d.sys_present = Some(g.exists(&sys).unwrap_or(false));
            // Service hive + on-disk .sys both count as "present" for diagnostics.
            if d.sys_present == Some(true) {
                d.present = true;
            }
        }
    }

    fn collect_windows_hotfixes(
        g: &mut Guestfs,
        systemroot: &str,
        software_hive: &std::path::Path,
    ) -> (
        Vec<crate::evidence::snapshot::WindowsHotfixEntry>,
        usize,
        bool,
    ) {
        use crate::evidence::snapshot::WindowsHotfixEntry;
        use crate::guestfs::windows_registry;
        use std::collections::BTreeMap;

        let mut by_kb: BTreeMap<String, WindowsHotfixEntry> = BTreeMap::new();

        if let Ok(reg) = windows_registry::parse_installed_updates(software_hive) {
            for u in reg {
                let kb = u.kb_number.to_ascii_uppercase();
                by_kb.entry(kb.clone()).or_insert(WindowsHotfixEntry {
                    kb,
                    source: "registry".into(),
                    title: Some(u.title),
                });
            }
        }

        if let Ok(host_root) = g.resolve_guest_path(systemroot) {
            if let Ok(fs) = windows_registry::detect_hotfixes_from_filesystem(host_root.as_path()) {
                for u in fs {
                    let kb = u.kb_number.to_ascii_uppercase();
                    by_kb.entry(kb.clone()).or_insert(WindowsHotfixEntry {
                        kb,
                        source: if u.title.contains("migration") {
                            "hf_mig".into()
                        } else {
                            "filesystem".into()
                        },
                        title: Some(u.title),
                    });
                }
            }
        }

        let cbs_guest = format!("{}/Logs/CBS/CBS.log", systemroot);
        if let Ok(cbs_host) = g.resolve_guest_path(&cbs_guest) {
            for u in windows_registry::parse_cbs_log_tail(cbs_host.as_path(), 512 * 1024) {
                let kb = u.kb_number.to_ascii_uppercase();
                by_kb.entry(kb.clone()).or_insert(WindowsHotfixEntry {
                    kb,
                    source: "cbs".into(),
                    title: Some(u.title),
                });
            }
        }

        let hf_mig_present = g
            .exists(&format!("{}/$hf_mig$", systemroot))
            .unwrap_or(false);

        let hotfix_count = by_kb.len();
        let hotfixes: Vec<_> = by_kb.into_values().take(40).collect();
        (hotfixes, hotfix_count, hf_mig_present)
    }

    fn probe_driver_signature_enforcement(
        g: &mut Guestfs,
        bcd_candidates: &[String],
    ) -> Option<bool> {
        use crate::guestfs::windows_registry;
        for path in bcd_candidates {
            if !g.exists(path).unwrap_or(false) {
                continue;
            }
            let Ok(host) = g.resolve_guest_path(path) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&host) else {
                continue;
            };
            // Cap read already done by fs::read — skip huge unexpected files.
            if bytes.len() > 8 * 1024 * 1024 {
                continue;
            }
            if let Some(enforced) = windows_registry::probe_bcd_signature_enforcement(&bytes) {
                return Some(enforced);
            }
        }
        None
    }

    /// Probe non-OS NTFS/FAT volumes for legacy System Reserved or UEFI ESP.
    ///
    /// Multi-partition Windows installs keep `bootmgr` + `Boot\BCD` on a small
    /// volume separate from `%SystemRoot%`. Without this probe, doctor falsely
    /// reports missing BCD when only C: is mounted as `/`.
    fn detect_system_reserved_partition(
        g: &mut Guestfs,
        windows_root: &str,
    ) -> Option<crate::evidence::snapshot::SystemReservedPartition> {
        use crate::evidence::snapshot::SystemReservedPartition;

        let filesystems = g.list_filesystems().ok()?;
        let probe = "/__gk_sysreserved";
        let _ = g.mkdir_p(probe);

        let mut best: Option<SystemReservedPartition> = None;

        for (device, fstype) in filesystems {
            if device == windows_root {
                continue;
            }
            let ft = fstype.to_ascii_lowercase();
            if !matches!(ft.as_str(), "ntfs" | "vfat" | "fat" | "fat32" | "msdos") {
                continue;
            }

            if g.mount_ro(&device, probe).is_err() {
                continue;
            }

            let has_windows = g
                .exists(&format!("{probe}/Windows/System32"))
                .unwrap_or(false)
                || g.exists(&format!("{probe}/windows/system32"))
                    .unwrap_or(false);
            let has_bootmgr = g.exists(&format!("{probe}/bootmgr")).unwrap_or(false)
                || g.exists(&format!("{probe}/Boot/bootmgr")).unwrap_or(false)
                || g.exists(&format!("{probe}/EFI/Microsoft/Boot/bootmgfw.efi"))
                    .unwrap_or(false);
            let has_bcd = g.exists(&format!("{probe}/Boot/BCD")).unwrap_or(false)
                || g.exists(&format!("{probe}/boot/BCD")).unwrap_or(false)
                || g.exists(&format!("{probe}/EFI/Microsoft/Boot/BCD"))
                    .unwrap_or(false);
            let has_efi_ms = g
                .exists(&format!("{probe}/EFI/Microsoft/Boot"))
                .unwrap_or(false);
            let size_bytes = g.statvfs(probe).ok().map(|m| {
                let blocks = *m.get("blocks").unwrap_or(&0);
                let bsize = *m.get("bsize").unwrap_or(&4096);
                (blocks.saturating_mul(bsize)).max(0) as u64
            });

            // Umount by device — Guestfs tracks host paths, not guest mountpoints.
            let _ = g.umount(&device);

            if has_windows {
                continue;
            }
            if !has_bootmgr && !has_bcd && !has_efi_ms {
                continue;
            }

            let role = if has_efi_ms || ft.contains("fat") {
                "esp"
            } else {
                "system_reserved"
            };

            let candidate = SystemReservedPartition {
                device: device.clone(),
                fstype,
                role: role.into(),
                has_bootmgr,
                has_bcd,
                size_bytes,
            };

            // Prefer volumes that carry both bootmgr and BCD; else first match.
            let replace = match &best {
                None => true,
                Some(prev) => {
                    let prev_score = i32::from(prev.has_bootmgr) + i32::from(prev.has_bcd);
                    let new_score = i32::from(candidate.has_bootmgr) + i32::from(candidate.has_bcd);
                    new_score > prev_score
                }
            };
            if replace {
                best = Some(candidate);
            }
        }

        best
    }

    /// VirtIO driver install state derived from the parsed SYSTEM hive
    /// service entries — the migration-critical set plus their boot flags.
    fn derive_virtio_drivers(
        services: &[crate::evidence::snapshot::WindowsServiceEntry],
    ) -> Vec<crate::evidence::snapshot::WindowsDriverEntry> {
        use crate::evidence::snapshot::{WindowsDriverEntry, WindowsStartType};
        const VIRTIO: &[&str] = &[
            "viostor", "vioscsi", "netkvm", "vioser", "balloon", "viorng",
        ];
        VIRTIO
            .iter()
            .map(|name| {
                let entry = services
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(name) && s.kernel_driver);
                match entry {
                    Some(s) => WindowsDriverEntry {
                        name: name.to_string(),
                        version: None,
                        start_type: format!("{:?}", s.start_type).to_lowercase(),
                        boot_critical: s.start_type == WindowsStartType::Boot,
                        present: true,
                        sys_present: None,
                    },
                    None => WindowsDriverEntry {
                        name: name.to_string(),
                        version: None,
                        start_type: String::new(),
                        boot_critical: false,
                        present: false,
                        sys_present: None,
                    },
                }
            })
            .collect()
    }
}

// Default impls for guestfs types used above
impl Default for crate::guestfs::BootConfig {
    fn default() -> Self {
        Self {
            bootloader: "unknown".to_string(),
            default_entry: "unknown".to_string(),
            timeout: "unknown".to_string(),
            kernel_cmdline: String::new(),
        }
    }
}

impl Default for crate::guestfs::SecurityInfo {
    fn default() -> Self {
        Self {
            selinux: "unknown".to_string(),
            apparmor: false,
            fail2ban: false,
            aide: false,
            auditd: false,
            ssh_keys: Vec::new(),
        }
    }
}

impl Default for crate::guestfs::PackageInfo {
    fn default() -> Self {
        Self {
            manager: "unknown".to_string(),
            package_count: 0,
            packages: Vec::new(),
        }
    }
}
