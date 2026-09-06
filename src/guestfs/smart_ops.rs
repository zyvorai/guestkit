// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SMART disk monitoring operations for disk image manipulation
//!
//! This implementation provides SMART disk health monitoring functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Check if SMART is available
    ///
    /// Additional functionality for SMART support
    pub fn smart_available(&mut self, device: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: smart_available {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("smartctl")
            .arg("-i")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute smartctl: {}", e)))?;

        Ok(output.status.success())
    }

    /// Get SMART health status
    ///
    /// Additional functionality for SMART support
    pub fn smart_health(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: smart_health {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("smartctl")
            .arg("-H")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute smartctl: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Look for health status in output
        for line in output_str.lines() {
            if line.contains("SMART overall-health") {
                if let Some(status) = line.split(':').nth(1) {
                    return Ok(status.trim().to_string());
                }
            }
        }

        Ok("UNKNOWN".to_string())
    }

    /// Get SMART attributes
    ///
    /// Additional functionality for SMART support
    pub fn smart_attributes(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: smart_attributes {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("smartctl")
            .arg("-A")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute smartctl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "smartctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut attributes = Vec::new();

        // Parse SMART attributes table
        let mut in_table = false;
        for line in output_str.lines() {
            if line.contains("ID#") && line.contains("ATTRIBUTE_NAME") {
                in_table = true;
                continue;
            }

            if in_table && !line.is_empty() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 10 {
                    let attr_name = parts[1];
                    let value = parts[3];
                    attributes.push((attr_name.to_string(), value.to_string()));
                }
            }
        }

        Ok(attributes)
    }

    /// Get SMART information
    ///
    /// Additional functionality for SMART support
    pub fn smart_info(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: smart_info {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("smartctl")
            .arg("-i")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute smartctl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "smartctl failed: {}",
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

    /// Run SMART self-test
    ///
    /// Additional functionality for SMART support
    pub fn smart_selftest(&mut self, device: &str, test_type: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: smart_selftest {} {}", device, test_type);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let test_arg = match test_type {
            "short" => "short",
            "long" => "long",
            "conveyance" => "conveyance",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid test type: {}",
                    test_type
                )))
            }
        };

        let output = Command::new("smartctl")
            .arg("-t")
            .arg(test_arg)
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute smartctl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "smartctl self-test failed: {}",
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
    fn test_smart_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
