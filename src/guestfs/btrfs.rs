// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Btrfs operations for disk image manipulation
//!
//! This implementation provides Btrfs-specific functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create Btrfs subvolume
    ///
    pub fn btrfs_subvolume_create(&mut self, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_create {}", dest);
        }

        let host_path = self.resolve_guest_path(dest)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("create")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs subvolume create failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Delete Btrfs subvolume
    ///
    pub fn btrfs_subvolume_delete(&mut self, subvolume: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_delete {}", subvolume);
        }

        let host_path = self.resolve_guest_path(subvolume)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("delete")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs subvolume delete failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// List Btrfs subvolumes
    ///
    pub fn btrfs_subvolume_list(&mut self, fs: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_list {}", fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("list")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs subvolume list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let subvolumes: Vec<String> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();

        Ok(subvolumes)
    }

    /// Create Btrfs snapshot
    ///
    pub fn btrfs_subvolume_snapshot(&mut self, source: &str, dest: &str, ro: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: btrfs_subvolume_snapshot {} {} {}",
                source, dest, ro
            );
        }

        let host_source = self.resolve_guest_path(source)?;
        let host_dest = self.resolve_guest_path(dest)?;

        let mut cmd = Command::new("btrfs");
        cmd.arg("subvolume").arg("snapshot");

        if ro {
            cmd.arg("-r");
        }

        cmd.arg(&host_source).arg(&host_dest);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs snapshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set Btrfs subvolume as default
    ///
    pub fn btrfs_subvolume_set_default(&mut self, id: i64, fs: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_set_default {} {}", id, fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("set-default")
            .arg(id.to_string())
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs set-default failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get Btrfs default subvolume ID
    ///
    pub fn btrfs_subvolume_get_default(&mut self, fs: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_get_default {}", fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("get-default")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs get-default failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output: "ID 5 (FS_TREE)"
        for line in stdout.lines() {
            if line.starts_with("ID") {
                if let Some(id_str) = line.split_whitespace().nth(1) {
                    if let Ok(id) = id_str.parse::<i64>() {
                        return Ok(id);
                    }
                }
            }
        }

        Err(Error::NotFound(
            "Could not parse default subvolume ID".to_string(),
        ))
    }

    /// Show Btrfs subvolume info
    ///
    pub fn btrfs_subvolume_show(&mut self, subvolume: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_subvolume_show {}", subvolume);
        }

        let host_path = self.resolve_guest_path(subvolume)?;

        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("show")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs subvolume show failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Balance Btrfs filesystem
    ///
    pub fn btrfs_balance(&mut self, fs: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_balance {}", fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("balance")
            .arg("start")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs balance failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Scrub Btrfs filesystem
    ///
    pub fn btrfs_scrub(&mut self, fs: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_scrub {}", fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("scrub")
            .arg("start")
            .arg("-B") // Blocking mode
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs scrub failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check Btrfs filesystem
    ///
    pub fn btrfs_filesystem_show(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_filesystem_show {}", device);
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

        let output = Command::new("btrfs")
            .arg("filesystem")
            .arg("show")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs filesystem show failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Defragment Btrfs filesystem
    ///
    pub fn btrfs_filesystem_defragment(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_filesystem_defragment {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("btrfs")
            .arg("filesystem")
            .arg("defragment")
            .arg("-r") // Recursive
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs defragment failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Sync Btrfs filesystem
    ///
    pub fn btrfs_filesystem_sync(&mut self, fs: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: btrfs_filesystem_sync {}", fs);
        }

        let host_path = self.resolve_guest_path(fs)?;

        let output = Command::new("btrfs")
            .arg("filesystem")
            .arg("sync")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "btrfs sync failed: {}",
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
    fn test_btrfs_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
