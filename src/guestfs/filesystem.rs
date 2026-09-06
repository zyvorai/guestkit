// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Filesystem operations for disk image manipulation
//!
//! This implementation provides filesystem creation and management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create a filesystem
    ///
    pub fn mkfs(&mut self, fstype: &str, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs {} {}", fstype, device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Map filesystem type to mkfs command
        let cmd = match fstype {
            "ext2" | "ext3" | "ext4" => format!("mkfs.{}", fstype),
            "xfs" => "mkfs.xfs".to_string(),
            "btrfs" => "mkfs.btrfs".to_string(),
            "vfat" | "fat" => "mkfs.vfat".to_string(),
            "ntfs" => "mkfs.ntfs".to_string(),
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unsupported filesystem type: {}",
                    fstype
                )))
            }
        };

        let output = Command::new(&cmd)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute {}: {}", cmd, e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Mkfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create a filesystem with options
    ///
    pub fn mkfs_opts(
        &mut self,
        fstype: &str,
        device: &str,
        blocksize: Option<i32>,
        features: Option<&str>,
        label: Option<&str>,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_opts {} {}", fstype, device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Build mkfs command with options
        let mut cmd = match fstype {
            "ext2" | "ext3" | "ext4" => Command::new(format!("mkfs.{}", fstype)),
            "xfs" => Command::new("mkfs.xfs"),
            "btrfs" => Command::new("mkfs.btrfs"),
            "vfat" | "fat" => Command::new("mkfs.vfat"),
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unsupported filesystem type: {}",
                    fstype
                )))
            }
        };

        // Add options
        if let Some(bs) = blocksize {
            if fstype.starts_with("ext") {
                cmd.arg("-b").arg(bs.to_string());
            }
        }

        if let Some(feat) = features {
            if fstype.starts_with("ext") {
                cmd.arg("-O").arg(feat);
            }
        }

        if let Some(lbl) = label {
            if fstype.starts_with("ext") || fstype == "xfs" {
                cmd.arg("-L").arg(lbl);
            } else if fstype == "vfat" || fstype == "fat" {
                cmd.arg("-n").arg(lbl);
            }
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Mkfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set or get ext2/3/4 filesystem parameters
    ///
    pub fn tune2fs(
        &mut self,
        device: &str,
        force: bool,
        maxmountcount: Option<i32>,
        label: Option<&str>,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: tune2fs {}", device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        let mut cmd = Command::new("tune2fs");

        if force {
            cmd.arg("-f");
        }

        if let Some(count) = maxmountcount {
            cmd.arg("-c").arg(count.to_string());
        }

        if let Some(lbl) = label {
            cmd.arg("-L").arg(lbl);
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute tune2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Tune2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Run filesystem check and repair
    ///
    pub fn fsck(&mut self, fstype: &str, device: &str) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck {} {}", fstype, device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Run fsck
        let output = Command::new("fsck")
            .arg("-t")
            .arg(fstype)
            .arg("-a") // Automatic repair
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck: {}", e)))?;

        // fsck return codes:
        // 0 - No errors
        // 1 - Filesystem errors corrected
        // 2 - System should be rebooted
        // 4 - Filesystem errors left uncorrected
        // 8 - Operational error
        // 16 - Usage or syntax error
        // 32 - Fsck canceled by user request
        // 128 - Shared-library error
        Ok(output.status.code().unwrap_or(-1))
    }

    /// Zero free space on filesystem
    ///
    pub fn zerofree(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zerofree {}", device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        let output = Command::new("zerofree")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zerofree: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Zerofree failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Trim free space on filesystem
    ///
    pub fn fstrim(&mut self, mountpoint: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fstrim {}", mountpoint);
        }

        // Get actual mountpoint
        let actual_mountpoint = if mountpoint == "/" {
            self.mount_root
                .as_ref()
                .ok_or_else(|| Error::InvalidState("No filesystem mounted".to_string()))?
                .clone()
        } else {
            let mount_root = self
                .mount_root
                .as_ref()
                .ok_or_else(|| Error::InvalidState("No filesystem mounted".to_string()))?;
            mount_root.join(mountpoint.trim_start_matches('/'))
        };

        let output = Command::new("fstrim")
            .arg(&actual_mountpoint)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fstrim: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Fstrim failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get filesystem statistics
    ///
    pub fn df(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: df");
        }

        // Run df on mount root
        let mount_root = self
            .mount_root
            .as_ref()
            .ok_or_else(|| Error::InvalidState("No filesystem mounted".to_string()))?;

        let output = Command::new("df")
            .arg("-h")
            .arg(mount_root)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute df: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Df failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get filesystem statistics in human-readable format
    ///
    pub fn df_h(&mut self) -> Result<String> {
        self.df()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
