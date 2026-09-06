// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! LDM (Windows Logical Disk Manager) operations for disk image manipulation
//!
//! This implementation provides Windows dynamic disk support.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// List LDM volumes
    ///
    pub fn ldmtool_diskgroup_volumes(&mut self, diskgroup: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_diskgroup_volumes {}", diskgroup);
        }

        let output = Command::new("ldmtool")
            .arg("show")
            .arg("volumes")
            .arg(diskgroup)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let volumes: Vec<String> = output_str
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with("Volume"))
            .map(|s| s.trim().to_string())
            .collect();

        Ok(volumes)
    }

    /// List LDM disk groups
    ///
    pub fn ldmtool_diskgroup_name(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_diskgroup_name {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("ldmtool")
            .arg("show")
            .arg("diskgroup")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let diskgroup = output_str.lines().next().unwrap_or("").trim().to_string();

        Ok(diskgroup)
    }

    /// List LDM disks
    ///
    pub fn ldmtool_diskgroup_disks(&mut self, diskgroup: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_diskgroup_disks {}", diskgroup);
        }

        let output = Command::new("ldmtool")
            .arg("show")
            .arg("disks")
            .arg(diskgroup)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let disks: Vec<String> = output_str
            .lines()
            .filter(|line| !line.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Ok(disks)
    }

    /// Scan for LDM volumes
    ///
    pub fn ldmtool_scan(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_scan");
        }

        let output = Command::new("ldmtool")
            .arg("scan")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool scan failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let diskgroups: Vec<String> = output_str
            .lines()
            .filter(|line| !line.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Ok(diskgroups)
    }

    /// Remove all LDM volumes
    ///
    pub fn ldmtool_remove_all(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_remove_all");
        }

        let output = Command::new("ldmtool")
            .arg("remove")
            .arg("all")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool remove all failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create LDM device nodes
    ///
    pub fn ldmtool_create_all(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_create_all");
        }

        let output = Command::new("ldmtool")
            .arg("create")
            .arg("all")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool create all failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get LDM volume type
    ///
    pub fn ldmtool_volume_type(&mut self, diskgroup: &str, volume: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_volume_type {} {}", diskgroup, volume);
        }

        let output = Command::new("ldmtool")
            .arg("show")
            .arg("volume")
            .arg(diskgroup)
            .arg(volume)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse volume type from output
        for line in output_str.lines() {
            if line.contains("Type:") {
                if let Some(vol_type) = line.split(':').nth(1) {
                    return Ok(vol_type.trim().to_string());
                }
            }
        }

        Ok("unknown".to_string())
    }

    /// Get LDM volume hint
    ///
    pub fn ldmtool_volume_hint(&mut self, diskgroup: &str, volume: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ldmtool_volume_hint {} {}", diskgroup, volume);
        }

        let output = Command::new("ldmtool")
            .arg("show")
            .arg("volume")
            .arg(diskgroup)
            .arg(volume)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ldmtool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ldmtool failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse volume hint (mountpoint/drive letter) from output
        for line in output_str.lines() {
            if line.contains("Hint:") {
                if let Some(hint) = line.split(':').nth(1) {
                    return Ok(hint.trim().to_string());
                }
            }
        }

        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldm_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
