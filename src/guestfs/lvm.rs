// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! LVM (Logical Volume Manager) operations for disk image manipulation
//!
//! This implementation uses LVM command-line tools (lvm2 package).
//!
//! **Requires**: lvm2 package and sudo/root permissions

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

/// Logical volume information
#[derive(Debug, Clone)]
pub struct LV {
    pub lv_name: String,
    pub lv_uuid: String,
    pub lv_attr: String,
    pub lv_major: i64,
    pub lv_minor: i64,
    pub lv_kernel_major: i64,
    pub lv_kernel_minor: i64,
    pub lv_size: i64,
    pub seg_count: i64,
    pub origin: String,
    pub snap_percent: f32,
    pub copy_percent: f32,
    pub move_pv: String,
    pub lv_tags: String,
    pub mirror_log: String,
    pub modules: String,
}

impl Guestfs {
    /// Get device filter config for LVM to restrict to NBD/loop devices only
    fn get_lvm_device_filter(&self) -> String {
        let device_path = if let Some(nbd) = &self.nbd_device {
            nbd.device_path().display().to_string()
        } else if let Some(loop_dev) = &self.loop_device {
            loop_dev
                .device_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "/dev/loop".to_string())
        } else {
            "/dev/nbd".to_string()
        };

        // LVM filter to ONLY scan our device and reject all others
        // This prevents accidentally discovering host LVM volumes
        // Escape all regex metacharacters in the device path
        let escaped_path = Self::escape_lvm_regex(&device_path);
        format!(
            r#"devices {{ filter=["a|^{}|","r|.*|"] }} global {{ locking_type=0 }}"#,
            escaped_path
        )
    }

    /// Scan for LVM volume groups (isolated to our block device only)
    ///
    pub fn vgscan(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: vgscan");
        }

        // Ensure NBD device is set up for LVM to detect
        self.setup_nbd_if_needed()?;

        // Create isolated LVM system directory
        let lvm_dir = if let Some(mount_root) = &self.mount_root {
            mount_root.join("lvm")
        } else {
            std::path::PathBuf::from("/run")
                .join(format!("guestkit-{}", std::process::id()))
                .join("lvm")
        };

        std::fs::create_dir_all(&lvm_dir)
            .map_err(|e| Error::CommandFailed(format!("Failed to create LVM directory: {}", e)))?;

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Get device filter to restrict LVM to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Build vgscan command with isolation
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("env");
            // Set env vars via sudo env to ensure they're passed through
            sudo_cmd.arg(format!("LVM_SYSTEM_DIR={}", lvm_dir.display()));
            sudo_cmd.arg("LVM_SUPPRESS_FD_WARNINGS=1");
            sudo_cmd.arg("vgscan");
            sudo_cmd
        } else {
            let mut cmd = Command::new("vgscan");
            cmd.env("LVM_SYSTEM_DIR", &lvm_dir);
            cmd.env("LVM_SUPPRESS_FD_WARNINGS", "1");
            cmd
        };

        // Run vgscan with device filter to only scan our NBD/loop device
        let output = cmd.arg("--config").arg(&lvm_filter).output().map_err(|e| {
            Error::CommandFailed(format!(
                "Failed to run vgscan: {}. Is lvm2 installed? Requires sudo/root.",
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!("vgscan failed: {}", stderr)));
        }

        Ok(())
    }

    /// Activate all LVM volume groups (isolated to our block device only)
    ///
    pub fn vg_activate_all(&mut self, activate: bool) -> Result<()> {
        self.ensure_ready()?;

        let action = if activate { "y" } else { "n" };

        if self.verbose {
            eprintln!("guestfs: vg_activate_all {}", activate);
        }

        // Get isolated LVM directory
        let lvm_dir = if let Some(mount_root) = &self.mount_root {
            mount_root.join("lvm")
        } else {
            std::path::PathBuf::from("/run")
                .join(format!("guestkit-{}", std::process::id()))
                .join("lvm")
        };

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Get device filter to restrict LVM to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Build vgchange command with isolation
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("env");
            sudo_cmd.arg(format!("LVM_SYSTEM_DIR={}", lvm_dir.display()));
            sudo_cmd.arg("LVM_SUPPRESS_FD_WARNINGS=1");
            sudo_cmd.arg("vgchange");
            sudo_cmd
        } else {
            let mut cmd = Command::new("vgchange");
            cmd.env("LVM_SYSTEM_DIR", &lvm_dir);
            cmd.env("LVM_SUPPRESS_FD_WARNINGS", "1");
            cmd
        };

        // Run vgchange to activate/deactivate all VGs
        let output = cmd
            .arg("-a")
            .arg(action)
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run vgchange: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!("vgchange failed: {}", stderr)));
        }

        // After activation, wait for device nodes to be created
        // Device-mapper creates nodes in /dev/mapper/ regardless of LVM_SYSTEM_DIR
        if activate {
            // Run udevadm settle to ensure device nodes are fully created
            let settle_result = Command::new("udevadm").arg("settle").output();

            if let Ok(settle_output) = settle_result {
                if !settle_output.status.success() && self.verbose {
                    eprintln!(
                        "guestfs: udevadm settle failed: {}",
                        String::from_utf8_lossy(&settle_output.stderr)
                    );
                }
            }

            // Brief additional sleep to ensure device nodes are ready
            std::thread::sleep(std::time::Duration::from_millis(200));

            // Record activated VGs for cleanup (after activation succeeded)
            if let Ok(vgs) = self.vgs() {
                for vg in vgs {
                    if !self.activated_vgs.contains(&vg) {
                        self.activated_vgs.push(vg);
                    }
                }
            }
        }

        Ok(())
    }

    /// Activate all LVM volume groups (alias of `vg_activate_all`, kept for
    /// callers migrating off libguestfs-style backends where this operation
    /// is named `vgchange_activate_all`).
    pub fn vgchange_activate_all(&mut self, activate: bool) -> Result<()> {
        self.vg_activate_all(activate)
    }

    /// Activate a specific volume group
    ///
    pub fn vg_activate(&mut self, activate: bool, volgroups: &[&str]) -> Result<()> {
        self.ensure_ready()?;

        let action = if activate { "y" } else { "n" };

        if self.verbose {
            eprintln!("guestfs: vg_activate {} {:?}", activate, volgroups);
        }

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Get device filter to restrict LVM to our device only
        let lvm_filter = self.get_lvm_device_filter();

        for vg in volgroups {
            // Build vgchange command
            let mut cmd = if need_sudo {
                let mut sudo_cmd = Command::new("sudo");
                sudo_cmd.arg("vgchange");
                sudo_cmd
            } else {
                Command::new("vgchange")
            };

            let output = cmd
                .arg("-a")
                .arg(action)
                .arg(vg)
                .arg("--config")
                .arg(&lvm_filter)
                .output()
                .map_err(|e| Error::CommandFailed(format!("Failed to run vgchange: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(Error::CommandFailed(format!(
                    "vgchange {} failed: {}",
                    vg, stderr
                )));
            }

            if self.verbose {
                let stdout = String::from_utf8_lossy(&output.stdout);
                eprintln!("vgchange {} output: {}", vg, stdout);
            }
        }

        Ok(())
    }

    /// Create a logical volume
    ///
    ///
    /// # Arguments
    ///
    /// * `logvol` - Name of logical volume to create
    /// * `volgroup` - Name of volume group
    /// * `mbytes` - Size in megabytes
    pub fn lvcreate(&mut self, logvol: &str, volgroup: &str, mbytes: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lvcreate {} {} {}M", logvol, volgroup, mbytes);
        }

        // Check if drive is readonly
        if let Some(drive) = self.drives.first() {
            if drive.readonly {
                return Err(Error::PermissionDenied(
                    "Cannot create LV on read-only drive".to_string(),
                ));
            }
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Create logical volume
        let output = Command::new("lvcreate")
            .arg("-L")
            .arg(format!("{}M", mbytes))
            .arg("-n")
            .arg(logvol)
            .arg(volgroup)
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run lvcreate: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!("lvcreate failed: {}", stderr)));
        }

        Ok(())
    }

    /// Remove a logical volume
    ///
    ///
    /// # Arguments
    ///
    /// * `device` - Logical volume device path (e.g., "/dev/vg/lv")
    pub fn lvremove(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lvremove {}", device);
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Remove logical volume
        let output = Command::new("lvremove")
            .arg("-f") // Force, don't ask for confirmation
            .arg(device)
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run lvremove: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!("lvremove failed: {}", stderr)));
        }

        Ok(())
    }

    /// List logical volumes with full details
    ///
    pub fn lvs_full(&self) -> Result<Vec<LV>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lvs_full");
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Use lvs with specific output fields
        let output = Command::new("lvs")
            .arg("--noheadings")
            .arg("--separator")
            .arg("|")
            .arg("-o")
            .arg("lv_name,lv_uuid,lv_attr,lv_major,lv_minor,lv_kernel_major,lv_kernel_minor,lv_size,seg_count,origin,snap_percent,copy_percent,move_pv,lv_tags,mirror_log,modules")
            .arg("--units")
            .arg("b")
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute lvs: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LVS command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lvs = Vec::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 16 {
                continue;
            }

            // Parse size (remove 'B' suffix)
            let size_str = parts[7].trim().trim_end_matches('B');
            let lv_size = size_str.parse::<i64>().unwrap_or(0);

            // Device major/minor of -1 means not applicable (e.g., inactive LV)
            let parse_i64 = |s: &str| -> i64 { s.trim().parse().unwrap_or(-1) };
            let parse_f32 = |s: &str| -> f32 { s.trim().parse().unwrap_or(0.0) };

            lvs.push(LV {
                lv_name: parts[0].trim().to_string(),
                lv_uuid: parts[1].trim().to_string(),
                lv_attr: parts[2].trim().to_string(),
                lv_major: parse_i64(parts[3]),
                lv_minor: parse_i64(parts[4]),
                lv_kernel_major: parse_i64(parts[5]),
                lv_kernel_minor: parse_i64(parts[6]),
                lv_size,
                seg_count: parts[8].trim().parse::<i64>().unwrap_or(0),
                origin: parts[9].trim().to_string(),
                snap_percent: parse_f32(parts[10]),
                copy_percent: parse_f32(parts[11]),
                move_pv: parts[12].trim().to_string(),
                lv_tags: parts[13].trim().to_string(),
                mirror_log: parts[14].trim().to_string(),
                modules: parts[15].trim().to_string(),
            });
        }

        Ok(lvs)
    }

    /// List logical volumes (simple)
    ///
    pub fn lvs(&self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lvs");
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Build lvs command
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("lvs");
            sudo_cmd
        } else {
            Command::new("lvs")
        };

        // List logical volumes with device filter
        let output = cmd
            .arg("--noheadings")
            .arg("-o")
            .arg("lv_path")
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run lvs: {}", e)))?;

        if !output.status.success() {
            // Not an error if no LVs found
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lvs: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(lvs)
    }

    /// List volume groups
    ///
    pub fn vgs(&self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: vgs");
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Build vgs command
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("vgs");
            sudo_cmd
        } else {
            Command::new("vgs")
        };

        // List volume groups with device filter
        let output = cmd
            .arg("--noheadings")
            .arg("-o")
            .arg("vg_name")
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run vgs: {}", e)))?;

        if !output.status.success() {
            // Not an error if no VGs found
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let vgs: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(vgs)
    }

    /// List physical volumes
    ///
    pub fn pvs(&self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: pvs");
        }

        // Get device filter to restrict to our device only
        let lvm_filter = self.get_lvm_device_filter();

        // Check if we need sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Build pvs command
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("pvs");
            sudo_cmd
        } else {
            Command::new("pvs")
        };

        // List physical volumes with device filter
        let output = cmd
            .arg("--noheadings")
            .arg("-o")
            .arg("pv_name")
            .arg("--config")
            .arg(&lvm_filter)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run pvs: {}", e)))?;

        if !output.status.success() {
            // Not an error if no PVs found
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pvs: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(pvs)
    }

    /// GuestKit: LVM scan (`vgscan` + optional activate).
    pub fn lvm_scan(&mut self, activate: bool) -> Result<()> {
        self.vgscan()?;
        if activate {
            self.vg_activate_all(true)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lvm_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
