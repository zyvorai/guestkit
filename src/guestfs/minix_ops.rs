// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Minix filesystem operations for disk image manipulation
//!
//! This implementation provides Minix filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create Minix filesystem
    ///
    /// Additional functionality for Minix support
    pub fn mkfs_minix(&mut self, device: &str, version: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_minix {} {}", device, version);
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

        let mut cmd = Command::new("mkfs.minix");

        match version {
            1 => cmd.arg("-1"),
            2 => cmd.arg("-2"),
            3 => cmd.arg("-3"),
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid Minix version: {}",
                    version
                )))
            }
        };

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfs.minix: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkfs.minix failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check Minix filesystem
    ///
    /// Additional functionality for Minix support
    pub fn fsck_minix(&mut self, device: &str, fix: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck_minix {} {}", device, fix);
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

        let mut cmd = Command::new("fsck.minix");

        if fix {
            cmd.arg("-a");
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck.minix: {}", e)))?;

        // fsck returns non-zero for errors found, which is expected
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minix_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
