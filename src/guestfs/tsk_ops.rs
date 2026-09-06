// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! TSK (The Sleuth Kit) forensics operations for disk image manipulation
//!
//! This implementation provides forensic analysis functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Download deleted file using TSK
    ///
    pub fn download_inode(&mut self, device: &str, inode: i64, filename: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: download_inode {} {} {}", device, inode, filename);
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

        let output = Command::new("icat")
            .arg(&nbd_partition)
            .arg(inode.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute icat: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "icat failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        std::fs::write(filename, output.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// List filesystem with TSK
    ///
    pub fn filesystem_walk(&mut self, device: &str) -> Result<Vec<TskDirent>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: filesystem_walk {}", device);
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

        let output = Command::new("fls")
            .arg("-r")
            .arg("-p")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute fls: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "fls failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in output_str.lines() {
            // Parse fls output: "type inode: path"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(inode) = parts[1].trim_end_matches(':').parse::<i64>() {
                    let name = parts.get(2).unwrap_or(&"").to_string();
                    entries.push(TskDirent {
                        path: name.clone(),
                        name,
                        inode,
                        allocated: !parts[0].contains("*"),
                        size: 0,
                    });
                }
            }
        }

        Ok(entries)
    }

    /// Find inode by path using TSK
    ///
    /// Additional functionality
    pub fn tsk_find_inode(&mut self, device: &str, path: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: tsk_find_inode {} {}", device, path);
        }

        let entries = self.filesystem_walk(device)?;

        for entry in entries {
            if entry.path == path || entry.name == path {
                return Ok(entry.inode);
            }
        }

        Err(Error::NotFound(format!("Path not found: {}", path)))
    }

    /// Get file info using TSK
    ///
    /// Additional functionality
    pub fn tsk_stat(&mut self, device: &str, inode: i64) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: tsk_stat {} {}", device, inode);
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

        let output = Command::new("istat")
            .arg(&nbd_partition)
            .arg(inode.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute istat: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "istat failed: {}",
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

/// TSK directory entry
#[derive(Debug, Clone)]
pub struct TskDirent {
    pub path: String,
    pub name: String,
    pub inode: i64,
    pub allocated: bool,
    pub size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsk_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
