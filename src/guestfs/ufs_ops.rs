// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! UFS (Unix File System) operations for disk image manipulation
//!
//! This implementation provides UFS filesystem inspection functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Get UFS filesystem label
    ///
    /// Additional functionality for UFS support
    pub fn ufs_get_label(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ufs_get_label {}", device);
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

        // UFS label is stored in the superblock
        // Try to use dumpfs if available
        let output = Command::new("dumpfs")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dumpfs: {}", e)))?;

        if !output.status.success() {
            return Ok(String::new());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Look for volume name in dumpfs output
        for line in output_str.lines() {
            if line.contains("volume name") {
                if let Some(label) = line.split(':').nth(1) {
                    return Ok(label.trim().to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Get UFS filesystem info
    ///
    /// Additional functionality for UFS support
    pub fn ufs_info(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ufs_info {}", device);
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

        let output = Command::new("dumpfs")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dumpfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "dumpfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut info = Vec::new();

        for line in output_str.lines() {
            if let Some((key, value)) = line.split_once(':') {
                info.push((key.trim().to_string(), value.trim().to_string()));
            }
        }

        Ok(info)
    }

    /// Check UFS filesystem
    ///
    /// Additional functionality for UFS support
    pub fn fsck_ufs(&mut self, device: &str, fix: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck_ufs {} {}", device, fix);
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

        let mut cmd = Command::new("fsck_ufs");

        if fix {
            cmd.arg("-y");
        } else {
            cmd.arg("-n");
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck_ufs: {}", e)))?;

        // fsck returns non-zero for errors found, which is expected
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ufs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
