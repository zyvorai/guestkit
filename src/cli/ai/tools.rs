// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Tool functions that the AI can call to inspect VMs

use crate::Guestfs;
use anyhow::Result;
use serde_json::json;

/// Context for AI tools with access to guestfs
pub struct DiagnosticTools {
    pub guestfs: Guestfs,
    pub root: String,
}

impl DiagnosticTools {
    pub fn new(guestfs: Guestfs, root: String) -> Self {
        Self { guestfs, root }
    }

    /// Get block device information (like lsblk)
    pub fn get_block_devices(&mut self) -> Result<String> {
        let devices = self.guestfs.list_devices()?;
        let mut result = Vec::new();

        for device in devices {
            let size = self.guestfs.blockdev_getsize64(&device).unwrap_or(0);
            let info = json!({
                "device": device,
                "size_bytes": size,
                "size_mb": size / 1024 / 1024,
            });
            result.push(info);
        }

        Ok(serde_json::to_string_pretty(&result)?)
    }

    /// Get LVM information
    pub fn get_lvm_info(&mut self) -> Result<String> {
        match self.guestfs.inspect_lvm(&self.root) {
            Ok(lvm) => Ok(serde_json::to_string_pretty(&lvm)?),
            Err(_) => Ok("No LVM configuration detected".to_string()),
        }
    }

    /// Get filesystem mounts
    pub fn get_mounts(&mut self) -> Result<String> {
        let mounts = self.guestfs.mounts()?;
        Ok(mounts.join("\n"))
    }

    /// Get fstab contents
    pub fn get_fstab(&mut self) -> Result<String> {
        match self.guestfs.inspect_fstab(&self.root) {
            Ok(fstab) => Ok(serde_json::to_string_pretty(&fstab)?),
            Err(e) => Ok(format!("Error reading fstab: {}", e)),
        }
    }

    /// Get system information
    pub fn get_system_info(&mut self) -> Result<String> {
        let info = json!({
            "os_type": self.guestfs.inspect_get_type(&self.root).ok(),
            "distro": self.guestfs.inspect_get_distro(&self.root).ok(),
            "version": {
                "major": self.guestfs.inspect_get_major_version(&self.root).ok(),
                "minor": self.guestfs.inspect_get_minor_version(&self.root).ok(),
            },
            "hostname": self.guestfs.inspect_get_hostname(&self.root).ok(),
            "architecture": self.guestfs.inspect_get_arch(&self.root).ok(),
            "init_system": self.guestfs.inspect_get_init_system(&self.root).ok(),
        });

        Ok(serde_json::to_string_pretty(&info)?)
    }

    /// Get kernel information
    pub fn get_kernel_info(&mut self) -> Result<String> {
        let modules = self
            .guestfs
            .inspect_kernel_modules(&self.root)
            .unwrap_or_default();
        let info = json!({
            "modules_count": modules.len(),
            "modules": modules.iter().take(20).collect::<Vec<_>>(),
        });

        Ok(serde_json::to_string_pretty(&info)?)
    }

    /// Check boot configuration
    pub fn check_boot_config(&mut self) -> Result<String> {
        let mut issues = Vec::new();

        // Check if /boot is accessible
        if let Err(e) = self.guestfs.is_dir("/boot") {
            issues.push(format!("Boot directory not accessible: {}", e));
        }

        // Check for kernel
        if let Ok(false) = self.guestfs.is_file("/boot/vmlinuz") {
            issues.push("No kernel found at /boot/vmlinuz".to_string());
        }

        // Check for initramfs
        match self.guestfs.ls("/boot") {
            Ok(files) => {
                let has_initramfs = files
                    .iter()
                    .any(|f| f.starts_with("initramfs") || f.starts_with("initrd"));
                if !has_initramfs {
                    issues.push("No initramfs/initrd found in /boot".to_string());
                }
            }
            Err(e) => issues.push(format!("Cannot list /boot: {}", e)),
        }

        if issues.is_empty() {
            Ok("Boot configuration looks OK".to_string())
        } else {
            Ok(format!("Boot issues found:\n{}", issues.join("\n")))
        }
    }

    /// Get security status
    pub fn get_security_status(&mut self) -> Result<String> {
        match self.guestfs.inspect_security(&self.root) {
            Ok(sec) => Ok(serde_json::to_string_pretty(&sec)?),
            Err(e) => Ok(format!("Error reading security info: {}", e)),
        }
    }

    /// Read a specific file (with size limit for safety)
    #[allow(dead_code)]
    pub fn read_file(&mut self, path: &str) -> Result<String> {
        const MAX_SIZE: i64 = 100 * 1024; // 100KB limit

        if let Ok(size) = self.guestfs.filesize(path) {
            if size > MAX_SIZE {
                return Ok(format!(
                    "File too large ({} bytes), showing first 100KB",
                    size
                ));
            }
        }

        match self.guestfs.read_file(path) {
            Ok(contents) => {
                let text = String::from_utf8_lossy(&contents);
                Ok(text.to_string())
            }
            Err(e) => Ok(format!("Error reading {}: {}", path, e)),
        }
    }

    /// List directory contents
    #[allow(dead_code)]
    pub fn list_directory(&mut self, path: &str) -> Result<String> {
        match self.guestfs.ls(path) {
            Ok(entries) => Ok(entries.join("\n")),
            Err(e) => Ok(format!("Error listing {}: {}", path, e)),
        }
    }

    /// Get deterministic bootability + root cause report from evidence engine
    pub fn get_bootability_report(&mut self, image_path: &std::path::Path) -> Result<String> {
        use crate::boot::{analyze_bootability, BootTarget};
        use crate::evidence::build_evidence;
        use crate::inference::infer_root_cause;

        let evidence = build_evidence(&mut self.guestfs, &self.root, image_path)?;
        let boot = analyze_bootability(&evidence, BootTarget::Kvm);
        let root_cause = infer_root_cause(&evidence, &boot);
        let report = json!({
            "bootability": boot,
            "root_cause": root_cause,
        });
        Ok(serde_json::to_string_pretty(&report)?)
    }

    /// Get normalized evidence snapshot (digital twin)
    pub fn get_evidence_snapshot(&mut self, image_path: &std::path::Path) -> Result<String> {
        use crate::evidence::build_evidence;
        let evidence = build_evidence(&mut self.guestfs, &self.root, image_path)?;
        Ok(serde_json::to_string_pretty(&evidence)?)
    }
}
