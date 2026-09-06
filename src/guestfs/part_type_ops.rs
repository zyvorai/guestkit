// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extended partition type operations for disk image manipulation
//!
//! This implementation provides partition type identification and management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Get partition type GUID (GPT)
    ///
    pub fn part_get_gpt_type(&mut self, device: &str, partnum: i32) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_get_gpt_type {} {}", device, partnum);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("sgdisk")
            .arg("-i")
            .arg(partnum.to_string())
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Partition GUID code:") {
                if let Some(guid) = line.split(':').nth(1) {
                    return Ok(guid.trim().to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Set partition type GUID (GPT)
    ///
    pub fn part_set_gpt_type(&mut self, device: &str, partnum: i32, guid: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_set_gpt_type {} {} {}", device, partnum, guid);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("sgdisk")
            .arg("-t")
            .arg(format!("{}:{}", partnum, guid))
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get partition attributes (GPT)
    ///
    pub fn part_get_gpt_attributes(&mut self, device: &str, partnum: i32) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_get_gpt_attributes {} {}", device, partnum);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("sgdisk")
            .arg("-i")
            .arg(partnum.to_string())
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Attribute flags:") {
                if let Some(attrs) = line.split(':').nth(1) {
                    let attrs_str = attrs.trim();
                    if let Ok(value) = i64::from_str_radix(attrs_str.trim_start_matches("0x"), 16) {
                        return Ok(value);
                    }
                }
            }
        }

        Ok(0)
    }

    /// Set partition attributes (GPT)
    ///
    pub fn part_set_gpt_attributes(
        &mut self,
        device: &str,
        partnum: i32,
        attributes: i64,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: part_set_gpt_attributes {} {} {}",
                device, partnum, attributes
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("sgdisk")
            .arg("-A")
            .arg(format!("{}:set:{}", partnum, attributes))
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Expand GPT to fill disk
    ///
    pub fn part_expand_gpt(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_expand_gpt {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("sgdisk")
            .arg("-e")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get MBR partition ID
    ///
    /// Already exists as part_get_mbr_id, adding extended version
    pub fn part_get_mbr_part_type(&mut self, device: &str, partnum: i32) -> Result<i32> {
        self.part_get_mbr_id(device, partnum)
    }

    /// Set MBR partition ID
    ///
    /// Already exists as part_set_mbr_id, adding extended version
    pub fn part_set_mbr_part_type(
        &mut self,
        device: &str,
        partnum: i32,
        idbyte: i32,
    ) -> Result<()> {
        self.part_set_mbr_id(device, partnum, idbyte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_part_type_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
