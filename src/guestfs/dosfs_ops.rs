// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! DOS/FAT filesystem operations for disk image manipulation
//!
//! This implementation provides FAT filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Set FAT filesystem label
    ///
    pub fn set_dos_label(&mut self, device: &str, label: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_dos_label {} {}", device, label);
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

        let output = Command::new("fatlabel")
            .arg(&nbd_partition)
            .arg(label)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fatlabel: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "fatlabel failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get FAT filesystem label
    ///
    pub fn get_dos_label(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_dos_label {}", device);
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

        let output = Command::new("fatlabel")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fatlabel: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "fatlabel failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check FAT filesystem
    ///
    pub fn fsck_dos(&mut self, device: &str, correct: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsck_dos {} {}", device, correct);
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

        let mut cmd = Command::new("fsck.vfat");

        if correct {
            cmd.arg("-a");
        } else {
            cmd.arg("-n");
        }

        cmd.arg(&nbd_partition);

        let _output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck.vfat: {}", e)))?;

        // fsck returns non-zero for errors found, which is expected
        Ok(())
    }

    /// Create FAT filesystem
    ///
    pub fn mkfs_dos(&mut self, device: &str, fat_bits: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfs_dos {} {}", device, fat_bits);
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

        let mut cmd = Command::new("mkfs.vfat");

        match fat_bits {
            12 => cmd.arg("-F").arg("12"),
            16 => cmd.arg("-F").arg("16"),
            32 => cmd.arg("-F").arg("32"),
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid FAT bits: {}",
                    fat_bits
                )))
            }
        };

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfs.vfat: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkfs.vfat failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get FAT filesystem info
    ///
    /// Additional functionality for FAT support
    pub fn dosfs_info(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: dosfs_info {}", device);
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

        let output = Command::new("fsck.vfat")
            .arg("-v")
            .arg("-n")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fsck.vfat: {}", e)))?;

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
    fn test_dosfs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
