// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! ISO and CD-ROM operations for disk image manipulation
//!
//! This implementation provides ISO image handling.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create ISO image from directory
    ///
    pub fn mkisofs(&mut self, iso_file: &str, source_dir: &str, volid: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkisofs {} {}", iso_file, source_dir);
        }

        let host_source = self.resolve_guest_path(source_dir)?;

        let mut cmd = Command::new("genisoimage");
        cmd.arg("-o").arg(iso_file);
        cmd.arg("-r"); // Rock Ridge extensions
        cmd.arg("-J"); // Joliet extensions

        if let Some(vol) = volid {
            cmd.arg("-V").arg(vol);
        }

        cmd.arg(&host_source);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute genisoimage: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "genisoimage failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// List files in ISO image
    ///
    pub fn isoinfo(&mut self, iso_file: &str) -> Result<Vec<String>> {
        if self.verbose {
            eprintln!("guestfs: isoinfo {}", iso_file);
        }

        let output = Command::new("isoinfo")
            .arg("-l")
            .arg("-i")
            .arg(iso_file)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute isoinfo: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "isoinfo failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();

        Ok(files)
    }

    /// Get ISO volume identifier
    ///
    pub fn isoinfo_device(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: isoinfo_device {}", device);
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

        let output = Command::new("isoinfo")
            .arg("-d")
            .arg("-i")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute isoinfo: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "isoinfo failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse volume ID from output
        for line in stdout.lines() {
            if line.starts_with("Volume id:") {
                if let Some(volid) = line.split(':').nth(1) {
                    return Ok(volid.trim().to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Mount ISO file as loop device
    ///
    pub fn mount_loop(&mut self, file: &str, mountpoint: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mount_loop {} {}", file, mountpoint);
        }

        let host_file = self.resolve_guest_path(file)?;
        let host_mountpoint = self.resolve_guest_path(mountpoint)?;

        // Create mountpoint if it doesn't exist
        if !host_mountpoint.exists() {
            std::fs::create_dir_all(&host_mountpoint).map_err(Error::Io)?;
        }

        let output = Command::new("mount")
            .arg("-o")
            .arg("loop,ro")
            .arg(&host_file)
            .arg(&host_mountpoint)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mount: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mount failed: {}",
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
    fn test_iso_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
