// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Multipath device operations for disk image manipulation
//!
//! This implementation provides multipath device management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Canonicalize device name and check for multipath
    ///
    pub fn is_multipath(&mut self, device: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_multipath {}", device);
        }

        // Check if device is a multipath device
        let canonical = self.canonical_device_name(device)?;
        Ok(canonical.starts_with("/dev/mapper/mpath") || canonical.starts_with("/dev/dm-"))
    }

    /// List multipath devices
    ///
    /// Additional functionality for multipath support
    pub fn list_multipath_devices(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_multipath_devices");
        }

        let output = Command::new("multipath")
            .arg("-l")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute multipath: {}", e)))?;

        if !output.status.success() {
            // No multipath devices is not an error
            return Ok(Vec::new());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in output_str.lines() {
            // Parse multipath output to extract device names
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    devices.push(format!("/dev/mapper/{}", parts[0]));
                }
            }
        }

        Ok(devices)
    }

    /// Get multipath device info
    ///
    /// Additional functionality for multipath support
    pub fn multipath_info(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: multipath_info {}", device);
        }

        let output = Command::new("multipath")
            .arg("-ll")
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute multipath: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "multipath info failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut info = Vec::new();

        for (i, line) in output_str.lines().enumerate() {
            info.push((format!("line_{}", i), line.trim().to_string()));
        }

        Ok(info)
    }

    /// Reload multipath devices
    ///
    /// Additional functionality for multipath support
    pub fn multipath_reload(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: multipath_reload");
        }

        let output = Command::new("multipath")
            .arg("-r")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute multipath: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "multipath reload failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Flush multipath device
    ///
    /// Additional functionality for multipath support
    pub fn multipath_flush(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: multipath_flush {}", device);
        }

        let output = Command::new("multipath")
            .arg("-f")
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute multipath: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "multipath flush failed: {}",
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
    fn test_mpath_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
