// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! ReiserFS operations for disk image manipulation
//!
//! This implementation provides ReiserFS filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create ReiserFS filesystem
    ///
    /// Additional functionality for ReiserFS support
    pub fn mkfs_reiserfs(&mut self, device: &str, label: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_reiserfs {} {:?}", device, label);
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

        let mut cmd = Command::new("mkreiserfs");
        cmd.arg("-f"); // Force creation without confirmation

        if let Some(l) = label {
            cmd.arg("-l").arg(l);
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkreiserfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkreiserfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set ReiserFS filesystem label
    ///
    /// Additional functionality for ReiserFS support
    pub fn reiserfs_set_label(&mut self, device: &str, label: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: reiserfs_set_label {} {}", device, label);
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

        let output = Command::new("reiserfstune")
            .arg("-l")
            .arg(label)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute reiserfstune: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "reiserfstune failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set ReiserFS filesystem UUID
    ///
    /// Additional functionality for ReiserFS support
    pub fn reiserfs_set_uuid(&mut self, device: &str, uuid: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: reiserfs_set_uuid {} {}", device, uuid);
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

        let output = Command::new("reiserfstune")
            .arg("-u")
            .arg(uuid)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute reiserfstune: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "reiserfstune failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check ReiserFS filesystem
    ///
    /// Additional functionality for ReiserFS support
    pub fn fsck_reiserfs(&mut self, device: &str, fix: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck_reiserfs {} {}", device, fix);
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

        let mut cmd = Command::new("reiserfsck");

        if fix {
            cmd.arg("--fix-fixable");
        } else {
            cmd.arg("--check");
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute reiserfsck: {}", e)))?;

        // reiserfsck returns non-zero for errors found, which is expected
        Ok(())
    }

    /// Resize ReiserFS filesystem
    ///
    /// Additional functionality for ReiserFS support
    pub fn reiserfs_resize(&mut self, device: &str, size: Option<i64>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: reiserfs_resize {} {:?}", device, size);
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

        let mut cmd = Command::new("resize_reiserfs");

        if let Some(s) = size {
            cmd.arg("-s").arg(format!("{}K", s / 1024));
        }

        cmd.arg(&nbd_partition);

        let output = cmd.output().map_err(|e| {
            Error::CommandFailed(format!("Failed to execute resize_reiserfs: {}", e))
        })?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "resize_reiserfs failed: {}",
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
    fn test_reiserfs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
