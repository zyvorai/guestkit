// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Bcache operations for disk image manipulation
//!
//! This implementation provides bcache (block cache) management functionality.

use crate::core::{Error, Result};
use crate::guestfs::security_utils::PathValidator;
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create bcache backing device
    ///
    /// Additional functionality for bcache support
    pub fn bcache_make_backing(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: bcache_make_backing {}", device);
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

        let output = Command::new("make-bcache")
            .arg("-B")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute make-bcache: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "make-bcache failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create bcache cache device
    ///
    /// Additional functionality for bcache support
    pub fn bcache_make_cache(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: bcache_make_cache {}", device);
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

        let output = Command::new("make-bcache")
            .arg("-C")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute make-bcache: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "make-bcache failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Register bcache device
    ///
    /// Additional functionality for bcache support
    pub fn bcache_register(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: bcache_register {}", device);
        }

        // Validate device path to prevent command injection
        PathValidator::validate_device_path(device)?;

        // Write directly to sysfs instead of using shell command
        let register_path = "/sys/fs/bcache/register";
        std::fs::write(register_path, device).map_err(|e| {
            Error::CommandFailed(format!(
                "Failed to register bcache device {}: {}",
                device, e
            ))
        })?;

        Ok(())
    }

    /// Stop bcache device
    ///
    /// Additional functionality for bcache support
    pub fn bcache_stop(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: bcache_stop {}", device);
        }

        // Validate device path to prevent command injection
        PathValidator::validate_device_path(device)?;

        let bcache_name = device.trim_start_matches("/dev/");

        // Validate bcache name doesn't contain path traversal
        PathValidator::validate_path_component(bcache_name)?;

        // Write directly to sysfs instead of using shell command
        let stop_path = format!("/sys/block/{}/bcache/stop", bcache_name);
        std::fs::write(&stop_path, "1").map_err(|e| {
            Error::CommandFailed(format!("Failed to stop bcache device {}: {}", device, e))
        })?;

        Ok(())
    }

    /// Get bcache statistics
    ///
    /// Additional functionality for bcache support
    pub fn bcache_stats(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: bcache_stats {}", device);
        }

        let bcache_name = device.trim_start_matches("/dev/");
        let stats_dir = format!("/sys/block/{}/bcache", bcache_name);

        let mut stats = Vec::new();

        // Try to read common bcache stats
        let stat_files = vec![
            "cache_mode",
            "dirty_data",
            "writeback_percent",
            "sequential_cutoff",
        ];

        for stat_file in stat_files {
            let path = format!("{}/{}", stats_dir, stat_file);
            if let Ok(value) = std::fs::read_to_string(&path) {
                stats.push((stat_file.to_string(), value.trim().to_string()));
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcache_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
