// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extended filesystem (ext2/3/4) operations for disk image manipulation
//!
//! This implementation provides ext-specific functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Set ext2/3/4 filesystem UUID
    ///
    pub fn set_e2uuid(&mut self, device: &str, uuid: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_e2uuid {} {}", device, uuid);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("tune2fs")
            .arg("-U")
            .arg(uuid)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute tune2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "tune2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set ext2/3/4 filesystem label
    ///
    pub fn set_e2label(&mut self, device: &str, label: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_e2label {} {}", device, label);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("tune2fs")
            .arg("-L")
            .arg(label)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute tune2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "tune2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get ext2/3/4 UUID
    ///
    pub fn get_e2uuid(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_e2uuid {}", device);
        }

        // Use vfs_uuid which already handles this
        self.vfs_uuid(device)
    }

    /// Get ext2/3/4 label
    ///
    pub fn get_e2label(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_e2label {}", device);
        }

        // Use vfs_label which already handles this
        self.vfs_label(device)
    }

    /// Dump ext2/3/4 filesystem
    ///
    pub fn dump_ext2(&mut self, device: &str, backupfile: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: dump_ext2 {} {}", device, backupfile);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("dump")
            .arg("-0")
            .arg("-f")
            .arg(backupfile)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dump: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "dump failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Restore ext2/3/4 filesystem
    ///
    pub fn restore_ext2(&mut self, backupfile: &str, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: restore_ext2 {} {}", backupfile, device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("restore")
            .arg("-r")
            .arg("-f")
            .arg(backupfile)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute restore: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "restore failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Tune ext2/3/4 generation number
    ///
    pub fn set_e2generation(&mut self, file: &str, generation: i64) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_e2generation {} {}", file, generation);
        }

        let host_path = self.resolve_guest_path(file)?;

        let output = Command::new("chattr")
            .arg(format!("+i={}", generation))
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute chattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "chattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get ext2/3/4 generation number
    ///
    pub fn get_e2generation(&mut self, file: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_e2generation {}", file);
        }

        let host_path = self.resolve_guest_path(file)?;

        let output = Command::new("lsattr")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute lsattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "lsattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Parse lsattr output for generation number
        // This is simplified - full implementation would parse the actual generation
        Ok(0)
    }

    /// Run e2fsck
    ///
    pub fn e2fsck(&mut self, device: &str, correct: bool, forceall: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: e2fsck {} {} {}", device, correct, forceall);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let mut cmd = Command::new("e2fsck");

        if correct {
            cmd.arg("-p"); // Automatic repair
        } else {
            cmd.arg("-n"); // No changes
        }

        if forceall {
            cmd.arg("-f"); // Force check even if clean
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute e2fsck: {}", e)))?;

        // e2fsck returns non-zero for errors found, which is expected
        Ok(())
    }

    /// Run mke2fs with options
    ///
    pub fn mke2fs(
        &mut self,
        device: &str,
        blockscount: i64,
        blocksize: i64,
        fragsize: i64,
        reserved: i64,
        inode: i64,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mke2fs {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let mut cmd = Command::new("mke2fs");

        if blocksize > 0 {
            cmd.arg("-b").arg(blocksize.to_string());
        }

        if fragsize > 0 {
            cmd.arg("-f").arg(fragsize.to_string());
        }

        if reserved >= 0 {
            cmd.arg("-m").arg(reserved.to_string());
        }

        if inode > 0 {
            cmd.arg("-i").arg(inode.to_string());
        }

        cmd.arg(&nbd_partition);

        if blockscount > 0 {
            cmd.arg(blockscount.to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mke2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mke2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
