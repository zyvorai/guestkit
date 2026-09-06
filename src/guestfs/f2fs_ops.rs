// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! F2FS (Flash-Friendly File System) operations for disk image manipulation
//!
//! This implementation provides F2FS filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create F2FS filesystem
    ///
    /// Additional functionality for F2FS support
    pub fn mkfs_f2fs(&mut self, device: &str, label: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_f2fs {} {:?}", device, label);
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

        let mut cmd = Command::new("mkfs.f2fs");

        if let Some(l) = label {
            cmd.arg("-l").arg(l);
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfs.f2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkfs.f2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check F2FS filesystem
    ///
    /// Additional functionality for F2FS support
    pub fn fsck_f2fs(&mut self, device: &str, fix: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck_f2fs {} {}", device, fix);
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

        let mut cmd = Command::new("fsck.f2fs");

        if fix {
            cmd.arg("-a");
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck.f2fs: {}", e)))?;

        // fsck returns non-zero for errors found, which is expected
        Ok(())
    }

    /// Resize F2FS filesystem
    ///
    /// Additional functionality for F2FS support
    pub fn resize_f2fs(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: resize_f2fs {}", device);
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

        let output = Command::new("resize.f2fs")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute resize.f2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "resize.f2fs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get F2FS filesystem info
    ///
    /// Additional functionality for F2FS support
    pub fn f2fs_info(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: f2fs_info {}", device);
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

        let output = Command::new("dump.f2fs")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dump.f2fs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "dump.f2fs failed: {}",
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f2fs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
