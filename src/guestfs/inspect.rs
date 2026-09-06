// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! OS inspection APIs for VM disk images
//!
//! Goals:
//! - Prefer correctness over “best effort guessing”
//! - Never mount RW during inspection
//! - Reduce false positives when selecting OS roots
//! - Provide a single high-level `inspect()` API returning `InspectedOS` records
//! - Broaden distro/package-format coverage via os-release parsing
//!
//! NOTE: This file assumes your Guestfs wrapper provides these methods (as in your code):
//! - ensure_ready(), partition_table(), parse_device_name()
//! - mount_ro(dev, mp), umount(mp), exists(path), cat(path)
//! - vgscan(), vg_activate_all(bool), lvs()
//! - and fields: verbose/debug, mounted, mount_root, windows_version_cache
//!
//! If your actual API names differ slightly, adjust accordingly.

use crate::core::{Error, Result};
use crate::disk::FileSystem;
use crate::guestfs::Guestfs;
use std::collections::HashMap;

/// OS inspection information
#[derive(Debug, Clone)]
pub struct InspectedOS {
    pub root: String,
    pub os_type: String,
    pub distro: String,
    pub product_name: String,
    pub major_version: i32,
    pub minor_version: i32,
    pub arch: String,
    pub hostname: String,
    pub package_format: String,
    pub mountpoints: HashMap<String, String>,
}

impl Guestfs {
    /// High-level inspect API: returns structured `InspectedOS` results.
    ///
    /// This mounts each candidate root at `/` read-only (once per root) and
    /// gathers all metadata with minimal remounting.
    pub fn inspect(&mut self) -> Result<Vec<InspectedOS>> {
        self.ensure_ready()?;

        let roots = self.inspect_os()?;
        let mut out = Vec::with_capacity(roots.len());

        for root in roots {
            // Mount RO if not already mounted
            let was_mounted = self.mounted.contains_key(&root);
            if !was_mounted {
                self.mount_ro(&root, "/")?;
            }

            // All inspect_get_* functions should be called while root is mounted
            let os_type = self.inspect_get_type(&root)?;
            let distro = self.inspect_get_distro(&root)?;
            let product_name = self.inspect_get_product_name(&root)?;
            let major_version = self.inspect_get_major_version(&root)?;
            let minor_version = self.inspect_get_minor_version(&root)?;
            let hostname = self.inspect_get_hostname(&root)?;
            let arch = self.inspect_get_arch(&root)?;
            let package_format = self.inspect_get_package_format(&root)?;
            let mountpoints = self.inspect_get_mountpoints(&root)?;

            // Cleanup: unmount if we mounted it
            if !was_mounted {
                let _ = self.umount(&root);
            }

            out.push(InspectedOS {
                root,
                os_type,
                distro,
                product_name,
                major_version,
                minor_version,
                arch,
                hostname,
                package_format,
                mountpoints,
            });
        }

        Ok(out)
    }

    /// Inspect operating systems in the disk image.
    ///
    /// Returns a list of *validated* root devices where operating systems were found.
    /// Validation is done by mounting candidates RO and checking for OS root markers.
    pub fn inspect_os(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        let mut roots = crate::core::mem_optimize::vec_for_partitions();

        // Try to scan and activate LVM volumes (best-effort).
        if self.vgscan().is_ok() {
            if let Err(e) = self.vg_activate_all(true) {
                if self.debug {
                    eprintln!("[DEBUG] Failed to activate LVM volumes: {}", e);
                }
            } else if self.verbose {
                eprintln!("guestfs: LVM volumes activated");
            }
        }

        // Clone partition data to avoid borrow checker issues.
        let partitions = {
            let pt = self.partition_table()?;
            pt.partitions().to_vec()
        };

        // Use standard device path (most common in VMs)
        let disk_dev = "/dev/sda".to_string();

        // 0) Whole-disk fallback: no partition table at all, filesystem
        // directly on the block device (e.g. Firecracker-style rootfs
        // images, which are typically `dd`'d as a bare ext4 filesystem with
        // no partition table so the guest can use root=/dev/vda directly).
        // parse_device_name() resolves a bare "/dev/sda" (no trailing
        // partition number) to the whole NBD/loop device, so mount_ro()
        // already supports this target; inspect_os() just never tried it.
        if partitions.is_empty() && self.validate_root_partition(&disk_dev)? {
            roots.push(disk_dev.clone());
        }

        // 1) Partition candidates
        for p in &partitions {
            let dev = build_partition_path(&disk_dev, p.number);

            // Only consider partitions with plausible FS types, then validate.
            let reader = self
                .reader
                .as_mut()
                .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;

            if let Ok(fs) = FileSystem::detect(reader, p) {
                match fs.fs_type() {
                    crate::disk::FileSystemType::Ext
                    | crate::disk::FileSystemType::Xfs
                    | crate::disk::FileSystemType::Btrfs
                    | crate::disk::FileSystemType::Ntfs
                        if self.validate_root_partition(&dev)? =>
                    {
                        roots.push(dev);
                    }
                    _ => {}
                }
            }
        }

        // 1b) Initrd-contained roots (Cirros and similar boot-partition cloud images)
        for p in &partitions {
            let dev = build_partition_path(&disk_dev, p.number);
            if roots.iter().any(|r| r == &dev) {
                continue;
            }

            let reader = self
                .reader
                .as_mut()
                .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;

            if let Ok(fs) = FileSystem::detect(reader, p) {
                match fs.fs_type() {
                    crate::disk::FileSystemType::Ext
                    | crate::disk::FileSystemType::Xfs
                    | crate::disk::FileSystemType::Btrfs
                        if self.validate_initrd_boot_partition(&dev)? =>
                    {
                        roots.push(dev);
                    }
                    _ => {}
                }
            }
        }

        // 2) LVM logical volume candidates (validated)
        if let Ok(lvs) = self.lvs() {
            // Prefer typical root LV names first (stable priority).
            let mut preferred = Vec::new();
            let mut others = Vec::new();

            for lv_path in lvs {
                let lv = lv_path.trim().to_string();
                if lv.is_empty() {
                    continue;
                }
                let name = lv.to_lowercase();

                // Quick rejects
                if name.contains("swap") {
                    continue;
                }

                // Partition into buckets
                if name.contains("root") || name.contains("system") || name.ends_with("/root") {
                    preferred.push(lv);
                } else if name.contains("home") || name.contains("var") || name.contains("tmp") {
                    // likely data volumes; still could be root in weird installs, but de-prioritize
                    others.push(lv);
                } else {
                    others.push(lv);
                }
            }

            for lv in preferred.into_iter().chain(others) {
                if self.validate_root_partition(&lv)? {
                    roots.push(lv);
                }
            }
        }

        Ok(roots)
    }

    /// Root validation: mount RO and check for strong OS markers.
    ///
    /// This is the key upgrade that reduces false positives (/home, data disks, etc.).
    /// Mount failures are treated as "not a valid root" (returns `Ok(false)`) rather
    /// than propagating errors, because LVM volumes on read-only NBD devices may
    /// fail to mount for benign reasons (e.g. dirty XFS log).
    fn validate_root_partition(&mut self, dev: &str) -> Result<bool> {
        // Check if this device is already mounted (at any mountpoint)
        let was_mounted = self.mounted.contains_key(dev);

        // Temporarily mount at / if not already mounted.
        if !was_mounted {
            if let Err(e) = self.mount_ro(dev, "/") {
                if self.verbose || self.debug {
                    eprintln!(
                        "guestfs: validate_root: mount failed for {} (skipping): {}",
                        dev, e
                    );
                }
                return Ok(false);
            }
        }

        // Validate root markers.
        let is_linux = looks_like_linux_root(self);
        let is_windows = looks_like_windows_root(self);

        // Cleanup: unmount if we mounted it
        if !was_mounted {
            let _ = self.umount(dev);
        }

        Ok(is_linux || is_windows)
    }

    /// Boot partition with root filesystem inside initrd (e.g. Cirros cloud images).
    fn validate_initrd_boot_partition(&mut self, dev: &str) -> Result<bool> {
        use std::process::Command;

        let was_mounted = self.mounted.contains_key(dev);
        if !was_mounted && self.mount_ro(dev, "/").is_err() {
            return Ok(false);
        }

        if looks_like_linux_root(self) || looks_like_windows_root(self) {
            if !was_mounted {
                let _ = self.umount(dev);
            }
            return Ok(false);
        }

        let Some(initrd_rel) = self.find_initrd_path() else {
            if !was_mounted {
                let _ = self.umount(dev);
            }
            return Ok(false);
        };

        let mount_point = match self.mounted.get(dev) {
            Some(p) => p.clone(),
            None => {
                if !was_mounted {
                    let _ = self.umount(dev);
                }
                return Ok(false);
            }
        };

        let initrd_host =
            std::path::PathBuf::from(&mount_point).join(initrd_rel.trim_start_matches('/'));
        if !initrd_host.exists() {
            if !was_mounted {
                let _ = self.umount(dev);
            }
            return Ok(false);
        }

        let script = format!(
            "zcat '{}' | cpio -t 2>/dev/null",
            initrd_host.to_string_lossy()
        );
        let output = Command::new("sh").arg("-c").arg(&script).output();

        let valid = match output {
            Ok(out) if out.status.success() || !out.stdout.is_empty() => {
                let listing = String::from_utf8_lossy(&out.stdout);
                let has_os_release = listing.lines().any(|l| {
                    let l = l.trim();
                    l == "etc/os-release" || l.ends_with("/etc/os-release")
                });
                let has_shell = listing.lines().any(|l| {
                    let l = l.trim();
                    l == "bin/sh"
                        || l.ends_with("/bin/sh")
                        || l == "bin/busybox"
                        || l.ends_with("/bin/busybox")
                });
                has_os_release && has_shell
            }
            _ => false,
        };

        if !was_mounted {
            let _ = self.umount(dev);
        }

        if valid && (self.verbose || self.debug) {
            eprintln!(
                "guestfs: validate_initrd: found Linux root in initrd on {}",
                dev
            );
        }

        Ok(valid)
    }

    fn find_initrd_path(&mut self) -> Option<String> {
        for candidate in ["/initrd.img", "/boot/initrd.img"] {
            if self.exists(candidate).unwrap_or(false) {
                return Some(candidate.to_string());
            }
        }

        if self.exists("/boot").unwrap_or(false) {
            if let Ok(entries) = self.ls("/boot") {
                for name in entries {
                    let lower = name.to_lowercase();
                    if lower.contains("initrd") {
                        return Some(format!("/boot/{name}"));
                    }
                }
            }
        }

        None
    }

    /// Get the type of operating system (linux/windows/unknown).
    pub fn inspect_get_type(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        // If root is an LV, parse_device_name may fail; use marker detection instead.
        // Prefer marker-based detection when mounted.
        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let os_type = if looks_like_windows_root(self) {
            "windows".to_string()
        } else if looks_like_linux_root(self) {
            "linux".to_string()
        } else {
            "unknown".to_string()
        };

        if !was_mounted {
            let _ = self.umount(root);
        }

        Ok(os_type)
    }

    /// Get the distribution name (Linux ID from os-release; Windows edition if available).
    pub fn inspect_get_distro(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        let os_type = self.inspect_get_type(root)?;

        // Windows: return edition where possible.
        if os_type == "windows" {
            if let Ok((_, _, edition)) = self.read_windows_version(root) {
                return Ok(edition);
            }
            return Ok("windows".to_string());
        }

        // Linux: prefer os-release
        if let Ok(os_release) = self.read_os_release(root) {
            if !os_release.id.is_empty() {
                return Ok(os_release.id);
            }
        }

        // Fallback: legacy release files
        if let Ok(distro) = self.detect_from_release_files(root) {
            return Ok(distro);
        }

        // Last ditch: filesystem label hints (weak)
        // NOTE: This can be inaccurate; keep it as a final fallback only.
        let partition_num = self.parse_device_name(root)?;
        let partition = {
            let pt = self.partition_table()?;
            pt.partitions()
                .iter()
                .find(|p| p.number == partition_num)
                .cloned()
                .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partition_num)))?
        };

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;
        let fs = FileSystem::detect(reader, &partition)?;

        if let Some(label) = fs.label() {
            let l = label.to_lowercase();
            if l.contains("fedora") {
                return Ok("fedora".to_string());
            } else if l.contains("ubuntu") {
                return Ok("ubuntu".to_string());
            } else if l.contains("debian") {
                return Ok("debian".to_string());
            } else if l.contains("rhel") || l.contains("redhat") {
                return Ok("rhel".to_string());
            } else if l.contains("centos") {
                return Ok("centos".to_string());
            } else if l.contains("photon") {
                return Ok("photon".to_string());
            } else if l.contains("opensuse") || l.contains("suse") {
                return Ok("opensuse".to_string());
            } else if l.contains("alpine") {
                return Ok("alpine".to_string());
            }
        }

        Ok("unknown".to_string())
    }

    /// Read and parse /etc/os-release (or /usr/lib/os-release).
    fn read_os_release(&mut self, root: &str) -> Result<OsRelease> {
        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let os_release_content = self
            .cat("/etc/os-release")
            .or_else(|_| self.cat("/usr/lib/os-release"))?;

        if !was_mounted {
            let _ = self.umount(root);
        }

        OsRelease::parse(&os_release_content)
    }

    /// Read Windows version from registry hive.
    ///
    /// Uses a cache and searches common hive paths beneath the mount root.
    fn read_windows_version(&mut self, root: &str) -> Result<(String, String, String)> {
        use crate::guestfs::windows_registry::get_windows_version;

        // Cache
        if let Some(cached) = self.windows_version_cache.get(root) {
            if self.verbose || self.debug {
                eprintln!("[DEBUG] Using cached Windows version for {}", root);
            }
            return Ok(cached.clone());
        }

        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let mount_root = self
            .mount_root
            .as_ref()
            .ok_or_else(|| Error::InvalidState("No mount root (mount_root is None)".to_string()))?
            .clone(); // Clone to avoid borrow issues

        // Common Windows registry hive locations (case variations).
        let hive_paths = [
            mount_root.join("Windows/System32/config/SOFTWARE"),
            mount_root.join("Windows/System32/Config/SOFTWARE"),
            mount_root.join("WINDOWS/System32/config/SOFTWARE"),
            mount_root.join("WINDOWS/System32/Config/SOFTWARE"),
            mount_root.join("WinNT/System32/config/SOFTWARE"),
            mount_root.join("WinNT/System32/Config/SOFTWARE"),
        ];

        let mut result = Err(Error::NotFound("SOFTWARE hive not found".to_string()));

        for hive_path in &hive_paths {
            if hive_path.exists() {
                result = get_windows_version(hive_path);
                if result.is_ok() {
                    break;
                }
            }
        }

        if let Ok(ref data) = result {
            self.windows_version_cache
                .insert(root.to_string(), data.clone());
        }

        if !was_mounted {
            let _ = self.umount(root);
        }

        result
    }

    /// Detect distribution from legacy release files (fallback only).
    fn detect_from_release_files(&mut self, root: &str) -> Result<String> {
        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let distro = if self.exists("/etc/arch-release").unwrap_or(false)
            || (self.exists("/usr/bin/pacman").unwrap_or(false)
                && self.exists("/etc/pacman.conf").unwrap_or(false))
        {
            "arch".to_string()
        } else if self.exists("/etc/alpine-release").unwrap_or(false) {
            "alpine".to_string()
        } else if self.exists("/etc/redhat-release").unwrap_or(false) {
            if let Ok(content) = self.cat("/etc/redhat-release") {
                let lc = content.to_lowercase();
                if lc.contains("fedora") {
                    "fedora".to_string()
                } else if lc.contains("centos") {
                    "centos".to_string()
                } else if lc.contains("rocky") {
                    "rocky".to_string()
                } else if lc.contains("alma") {
                    "alma".to_string()
                } else if lc.contains("oracle linux") {
                    "ol".to_string()
                } else {
                    "rhel".to_string() // default family guess (includes Red Hat)
                }
            } else {
                "unknown".to_string()
            }
        } else if self.exists("/etc/SuSE-release").unwrap_or(false)
            || self.exists("/etc/os-release").unwrap_or(false)
        {
            // SuSE-release is legacy; os-release should have ID=sles/opensuse
            if let Ok(osr) = self.read_os_release(root) {
                if osr.id.contains("sles") {
                    "sles".to_string()
                } else if osr.id.contains("opensuse") || osr.id.contains("suse") {
                    "opensuse".to_string()
                } else {
                    osr.id
                }
            } else {
                "unknown".to_string()
            }
        } else if self.exists("/etc/debian_version").unwrap_or(false) {
            // Debian or Ubuntu
            if self.exists("/etc/lsb-release").unwrap_or(false) {
                if let Ok(content) = self.cat("/etc/lsb-release") {
                    if content.contains("Ubuntu") {
                        "ubuntu".to_string()
                    } else {
                        "debian".to_string()
                    }
                } else {
                    "debian".to_string()
                }
            } else {
                "debian".to_string()
            }
        } else {
            if !was_mounted {
                let _ = self.umount(root);
            }
            return Err(Error::NotFound("No release files found".to_string()));
        };

        if !was_mounted {
            let _ = self.umount(root);
        }

        Ok(distro)
    }

    /// Get the product name.
    pub fn inspect_get_product_name(&mut self, root: &str) -> Result<String> {
        let os_type = self.inspect_get_type(root)?;

        if os_type == "windows" {
            if let Ok((product_name, _, _)) = self.read_windows_version(root) {
                return Ok(product_name);
            }
            return Ok("Windows".to_string());
        }

        if let Ok(os_release) = self.read_os_release(root) {
            if !os_release.pretty_name.is_empty() {
                return Ok(os_release.pretty_name);
            }
        }

        let distro = self.inspect_get_distro(root)?;
        if os_type == "linux" {
            match distro.as_str() {
                "fedora" => Ok("Fedora Linux".to_string()),
                "ubuntu" => Ok("Ubuntu".to_string()),
                "debian" => Ok("Debian GNU/Linux".to_string()),
                "rhel" => Ok("Red Hat Enterprise Linux".to_string()),
                "centos" => Ok("CentOS Linux".to_string()),
                "rocky" => Ok("Rocky Linux".to_string()),
                "alma" => Ok("AlmaLinux".to_string()),
                "ol" => Ok("Oracle Linux".to_string()),
                "amzn" | "amazon" => Ok("Amazon Linux".to_string()),
                "photon" => Ok("VMware Photon OS".to_string()),
                "opensuse" => Ok("openSUSE".to_string()),
                "sles" => Ok("SUSE Linux Enterprise Server".to_string()),
                "alpine" => Ok("Alpine Linux".to_string()),
                "arch" => Ok("Arch Linux".to_string()),
                _ => Ok("Linux".to_string()),
            }
        } else {
            Ok("Unknown".to_string())
        }
    }

    /// Get the architecture (heuristic; avoids hardcoding x86_64).
    ///
    /// Without reading ELF/PE headers (binary-safe read), this uses conservative hints.
    /// If uncertain, returns "unknown".
    pub fn inspect_get_arch(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        let os_type = self.inspect_get_type(root)?;
        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let arch = if os_type == "linux" {
            // Heuristics:
            // - /lib64 strongly suggests 64-bit userspace
            // - /lib/ld-linux-aarch64.so.1 suggests aarch64
            // - /lib/ld-linux-armhf.so.3 suggests armhf
            // - /lib/ld-musl-x86_64.so.1 suggests x86_64 (musl)
            // Keep conservative.
            if self.exists("/lib/ld-linux-aarch64.so.1").unwrap_or(false)
                || self.exists("/lib64/ld-linux-aarch64.so.1").unwrap_or(false)
            {
                "aarch64".to_string()
            } else if self.exists("/lib/ld-linux-armhf.so.3").unwrap_or(false) {
                "armhf".to_string()
            } else if self.exists("/lib/ld-musl-x86_64.so.1").unwrap_or(false)
                || self.exists("/lib64/ld-linux-x86-64.so.2").unwrap_or(false)
                || self.exists("/lib/ld-linux-x86-64.so.2").unwrap_or(false)
                || self.exists("/lib64").unwrap_or(false)
            {
                "x86_64".to_string()
            } else {
                "unknown".to_string()
            }
        } else if os_type == "windows" {
            // Heuristics:
            // - Presence of "Program Files (x86)" often indicates 64-bit Windows.
            if self.exists("/Program Files (x86)").unwrap_or(false)
                || self.exists("/program files (x86)").unwrap_or(false)
            {
                "x86_64".to_string()
            } else {
                // Otherwise could be 32-bit.
                "i686".to_string()
            }
        } else {
            "unknown".to_string()
        };

        if !was_mounted {
            let _ = self.umount(root);
        }

        Ok(arch)
    }

    /// Get the major version number.
    pub fn inspect_get_major_version(&mut self, root: &str) -> Result<i32> {
        self.ensure_ready()?;

        let os_type = self.inspect_get_type(root)?;

        if os_type == "windows" {
            if let Ok((_, version, _)) = self.read_windows_version(root) {
                if let Some(major_str) = version.split('.').next() {
                    if let Ok(major) = major_str.parse::<i32>() {
                        return Ok(major);
                    }
                }
            }
            return Ok(0);
        }

        if let Ok(os_release) = self.read_os_release(root) {
            return Ok(os_release.version_major);
        }

        Ok(0)
    }

    /// Get the minor version number.
    pub fn inspect_get_minor_version(&mut self, root: &str) -> Result<i32> {
        self.ensure_ready()?;

        let os_type = self.inspect_get_type(root)?;

        if os_type == "windows" {
            if let Ok((_, version, _)) = self.read_windows_version(root) {
                let parts: Vec<&str> = version.split('.').collect();
                if parts.len() >= 2 {
                    if let Ok(minor) = parts[1].parse::<i32>() {
                        return Ok(minor);
                    }
                }
            }
            return Ok(0);
        }

        if let Ok(os_release) = self.read_os_release(root) {
            return Ok(os_release.version_minor);
        }

        Ok(0)
    }

    /// Get the hostname (read-only; never mounts RW).
    pub fn inspect_get_hostname(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        let was_mounted = self.mounted.contains_key(root);

        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let hostname = {
            // Linux hostname
            if let Ok(content) = self.cat("/etc/hostname") {
                let t = content.trim();
                if !t.is_empty() {
                    t.to_string()
                } else if let Ok(mi) = self.cat("/etc/machine-info") {
                    // systemd machine-info
                    let mut result = "localhost".to_string();
                    for line in mi.lines() {
                        if let Some(v) = line.strip_prefix("PRETTY_HOSTNAME=") {
                            let t = v.trim().trim_matches('"');
                            if !t.is_empty() {
                                result = t.to_string();
                                break;
                            }
                        }
                    }
                    result
                } else {
                    "localhost".to_string()
                }
            } else if let Ok(mi) = self.cat("/etc/machine-info") {
                // systemd machine-info
                let mut result = "localhost".to_string();
                for line in mi.lines() {
                    if let Some(v) = line.strip_prefix("PRETTY_HOSTNAME=") {
                        let t = v.trim().trim_matches('"');
                        if !t.is_empty() {
                            result = t.to_string();
                            break;
                        }
                    }
                }
                result
            } else {
                "localhost".to_string()
            }
        };

        if !was_mounted {
            let _ = self.umount(root);
        }

        Ok(hostname)
    }

    /// Get the package format (rpm/deb/apk/pacman/unknown).
    ///
    /// Uses os-release `ID` + `ID_LIKE` for broader coverage.
    pub fn inspect_get_package_format(&mut self, root: &str) -> Result<String> {
        let os_type = self.inspect_get_type(root)?;
        if os_type == "windows" {
            return Ok("msi".to_string()); // not strictly “package format”, but useful
        }

        if let Ok(osr) = self.read_os_release(root) {
            let mut ids = Vec::new();
            if !osr.id.is_empty() {
                ids.push(osr.id.clone());
            }
            ids.extend(osr.id_like.clone());

            let has = |s: &str| ids.iter().any(|x| x == s);

            if has("alpine") {
                return Ok("apk".to_string());
            }
            if has("debian") || has("ubuntu") {
                return Ok("deb".to_string());
            }
            if has("arch") || has("archlinux") || has("manjaro") {
                return Ok("pacman".to_string());
            }
            if has("rhel")
                || has("fedora")
                || has("centos")
                || has("suse")
                || has("opensuse")
                || has("sles")
                || has("photon")
                || has("amzn")
                || has("rocky")
                || has("alma")
                || has("ol")
            {
                return Ok("rpm".to_string());
            }
        }

        // Fallback map from distro string
        let distro = self.inspect_get_distro(root)?;
        match distro.as_str() {
            "fedora" | "rhel" | "centos" | "photon" | "opensuse" | "sles" | "rocky" | "alma"
            | "ol" => Ok("rpm".to_string()),
            "ubuntu" | "debian" => Ok("deb".to_string()),
            "arch" => Ok("pacman".to_string()),
            "alpine" => Ok("apk".to_string()),
            _ => Ok("unknown".to_string()),
        }
    }

    /// Get mountpoints for the root device.
    ///
    /// Parses /etc/fstab to discover additional mountpoints beyond root.
    /// Mounts `root` read-only first when needed so fstab is readable — without
    /// that, callers only see `/` and miss separate `/boot` / `/boot/efi`
    /// partitions (common on Ubuntu cloud images).
    pub fn inspect_get_mountpoints(&mut self, root: &str) -> Result<HashMap<String, String>> {
        self.ensure_ready()?;

        let mut mountpoints = HashMap::new();
        // Always include root
        mountpoints.insert("/".to_string(), root.to_string());

        // Root must be mounted to read /etc/fstab. Leave it mounted so the
        // caller's subsequent mount loop can skip `/` via already_mounted.
        if !self.mounted.contains_key(root) {
            self.mount_ro(root, "/")?;
        }

        // Try to parse /etc/fstab for additional mountpoints
        if let Ok(fstab_content) = self.cat("/etc/fstab") {
            // Pseudo-filesystems to skip
            let pseudo_fs = [
                "proc",
                "sysfs",
                "tmpfs",
                "devpts",
                "devtmpfs",
                "cgroup",
                "cgroup2",
                "debugfs",
                "securityfs",
                "configfs",
                "fusectl",
                "mqueue",
                "hugetlbfs",
                "pstore",
                "binfmt_misc",
                "autofs",
                "efivarfs",
            ];

            for line in fstab_content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() < 3 {
                    continue;
                }

                let spec = parts[0];
                let mount = parts[1];
                let fstype = parts[2];

                // Skip pseudo-filesystems and swap
                if pseudo_fs.contains(&fstype) || fstype == "swap" || mount == "none" {
                    continue;
                }

                // Skip if we already have this mountpoint (root)
                if mount == "/" {
                    continue;
                }

                mountpoints.insert(mount.to_string(), spec.to_string());
            }
        }

        Ok(mountpoints)
    }

    /// List installed applications.
    pub fn inspect_list_applications(&mut self, root: &str) -> Result<Vec<Application>> {
        self.ensure_ready()?;

        let apps2 = self.inspect_list_applications2(root)?;

        let applications = apps2
            .into_iter()
            .map(|(name, version, release)| Application {
                name: name.clone(),
                display_name: name,
                epoch: 0,
                version,
                release,
                arch: String::new(),
                install_path: String::new(),
                publisher: String::new(),
                url: String::new(),
                description: String::new(),
            })
            .collect();

        Ok(applications)
    }

    /// Check if this is a live CD/USB.
    pub fn inspect_is_live(&mut self, _root: &str) -> Result<bool> {
        self.ensure_ready()?;

        // Check for common live CD/USB indicators
        let live_indicators = ["/run/live", "/lib/live", "/casper", "/run/initramfs/live"];

        for path in &live_indicators {
            if self.exists(path).unwrap_or(false) {
                return Ok(true);
            }
        }

        // Check if root filesystem is squashfs (common for live images)
        if let Ok(content) = self.cat("/proc/mounts") {
            if content.lines().any(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                parts.len() >= 3 && parts[1] == "/" && parts[2] == "squashfs"
            }) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Installed application information
#[derive(Debug, Clone)]
pub struct Application {
    pub name: String,
    pub display_name: String,
    pub epoch: i32,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub install_path: String,
    pub publisher: String,
    pub url: String,
    pub description: String,
}

/// Parsed /etc/os-release information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OsRelease {
    pub id: String,
    pub id_like: Vec<String>,
    pub pretty_name: String,
    pub version_id: String,
    pub version_major: i32,
    pub version_minor: i32,
    pub cpe_name: String,
    pub support_end: String,
    pub home_url: String,
    pub bug_report_url: String,
}

impl OsRelease {
    fn parse(content: &str) -> Result<Self> {
        let mut id = String::new();
        let mut id_like = Vec::<String>::new();
        let mut pretty_name = String::new();
        let mut version_id = String::new();
        let mut cpe_name = String::new();
        let mut support_end = String::new();
        let mut home_url = String::new();
        let mut bug_report_url = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, raw_val)) = line.split_once('=') {
                let value = raw_val.trim().trim_matches('"').trim();

                match key {
                    "ID" => id = value.to_lowercase(),
                    "ID_LIKE" => {
                        // Common format: ID_LIKE="rhel fedora"
                        id_like = value
                            .split_whitespace()
                            .map(|s| s.to_lowercase())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    "PRETTY_NAME" => pretty_name = value.to_string(),
                    "VERSION_ID" => version_id = value.to_string(),
                    "CPE_NAME" => cpe_name = value.to_string(),
                    "SUPPORT_END" => support_end = value.to_string(),
                    "HOME_URL" => home_url = value.to_string(),
                    "BUG_REPORT_URL" => bug_report_url = value.to_string(),
                    _ => {}
                }
            }
        }

        // Parse version into major.minor (default to 0 for non-numeric components)
        let (version_major, version_minor) = if !version_id.is_empty() {
            let parts: Vec<&str> = version_id.split('.').collect();
            let major = parts
                .first()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    log::debug!(
                        "Non-numeric major version in '{}', defaulting to 0",
                        version_id
                    );
                    0
                });
            let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            (major, minor)
        } else {
            (0, 0)
        };

        if id.is_empty() {
            return Err(Error::NotFound("ID not found in os-release".to_string()));
        }

        Ok(OsRelease {
            id,
            id_like,
            pretty_name,
            version_id,
            version_major,
            version_minor,
            cpe_name,
            support_end,
            home_url,
            bug_report_url,
        })
    }
}

/// Build a partition path that handles the "p" separator for nvme/mmcblk devices.
///
/// Examples:
/// - /dev/sda + 1 => /dev/sda1
/// - /dev/vda + 2 => /dev/vda2
/// - /dev/nvme0n1 + 3 => /dev/nvme0n1p3
/// - /dev/mmcblk0 + 1 => /dev/mmcblk0p1
fn build_partition_path(disk: &str, part_num: u32) -> String {
    let needs_p = disk.contains("nvme") || disk.contains("mmcblk");
    if needs_p {
        format!("{}p{}", disk, part_num)
    } else {
        format!("{}{}", disk, part_num)
    }
}

/// Strong Linux root markers.
/// Keep this strict-ish to reduce false positives.
fn looks_like_linux_root(g: &mut Guestfs) -> bool {
    let osr = g.exists("/etc/os-release").unwrap_or(false)
        || g.exists("/usr/lib/os-release").unwrap_or(false);
    let shellish =
        g.exists("/bin/sh").unwrap_or(false) || g.exists("/usr/bin/env").unwrap_or(false);
    osr && shellish
}

/// Strong Windows root markers.
/// Keep strict-ish to avoid NTFS data volumes being misclassified.
fn looks_like_windows_root(g: &mut Guestfs) -> bool {
    // Case-insensitive filesystems make this fairly robust.
    let win = g.exists("/Windows/System32").unwrap_or(false)
        || g.exists("/WINDOWS/System32").unwrap_or(false);
    let hive = g.exists("/Windows/System32/config").unwrap_or(false)
        || g.exists("/Windows/System32/Config").unwrap_or(false)
        || g.exists("/WINDOWS/System32/config").unwrap_or(false)
        || g.exists("/WINDOWS/System32/Config").unwrap_or(false);
    win && hive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_path_builder() {
        assert_eq!(build_partition_path("/dev/sda", 1), "/dev/sda1");
        assert_eq!(build_partition_path("/dev/vda", 2), "/dev/vda2");
        assert_eq!(build_partition_path("/dev/nvme0n1", 3), "/dev/nvme0n1p3");
        assert_eq!(build_partition_path("/dev/mmcblk0", 1), "/dev/mmcblk0p1");
    }

    #[test]
    fn test_os_release_parse_photon() {
        let content = r#"
NAME="VMware Photon OS"
VERSION="5.0"
ID=photon
VERSION_ID=5.0
PRETTY_NAME="VMware Photon OS/Linux"
"#;
        let os_release = OsRelease::parse(content).unwrap();
        assert_eq!(os_release.id, "photon");
        assert_eq!(os_release.version_major, 5);
        assert_eq!(os_release.version_minor, 0);
    }

    #[test]
    fn test_os_release_parse_fedora() {
        let content = r#"
NAME="Fedora Linux"
VERSION="39 (Server Edition)"
ID=fedora
ID_LIKE="rhel fedora"
VERSION_ID=39
PRETTY_NAME="Fedora Linux 39 (Server Edition)"
"#;
        let os_release = OsRelease::parse(content).unwrap();
        assert_eq!(os_release.id, "fedora");
        assert!(os_release.id_like.contains(&"rhel".to_string()));
        assert_eq!(os_release.version_major, 39);
        assert_eq!(os_release.version_minor, 0);
    }
}
