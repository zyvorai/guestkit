// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! NILFS2 (log-structured filesystem) operations for disk image manipulation
//!
//! This implementation provides NILFS2 filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create NILFS2 filesystem
    ///
    /// Additional functionality for NILFS2 support
    pub fn mkfs_nilfs2(&mut self, device: &str, label: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_nilfs2 {} {:?}", device, label);
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

        let mut cmd = Command::new("mkfs.nilfs2");

        if let Some(l) = label {
            cmd.arg("-L").arg(l);
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfs.nilfs2: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkfs.nilfs2 failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Resize NILFS2 filesystem
    ///
    /// Additional functionality for NILFS2 support
    pub fn nilfs_resize(&mut self, device: &str, size: Option<i64>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: nilfs_resize {} {:?}", device, size);
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

        let mut cmd = Command::new("nilfs-resize");

        if let Some(s) = size {
            cmd.arg("-y").arg(s.to_string());
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute nilfs-resize: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "nilfs-resize failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Clean NILFS2 filesystem
    ///
    /// Additional functionality for NILFS2 support
    pub fn nilfs_clean(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: nilfs_clean {}", device);
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

        let output = Command::new("nilfs-clean")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute nilfs-clean: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "nilfs-clean failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Tune NILFS2 filesystem
    ///
    /// Additional functionality for NILFS2 support
    pub fn nilfs_tune(
        &mut self,
        device: &str,
        label: Option<&str>,
        uuid: Option<&str>,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: nilfs_tune {} {:?} {:?}", device, label, uuid);
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

        let mut cmd = Command::new("tunefs.nilfs2");

        if let Some(l) = label {
            cmd.arg("-L").arg(l);
        }

        if let Some(u) = uuid {
            cmd.arg("-U").arg(u);
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute tunefs.nilfs2: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "tunefs.nilfs2 failed: {}",
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
    fn test_nilfs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
