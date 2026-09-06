// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Checksum operations for disk image manipulation
//!
//! This implementation provides file checksumming capabilities.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Calculate MD5 checksum of a file
    ///
    pub fn checksum(&mut self, csumtype: &str, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: checksum {} {}", csumtype, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        // Map checksum type to command
        let cmd = match csumtype {
            "md5" => "md5sum",
            "sha1" => "sha1sum",
            "sha224" => "sha224sum",
            "sha256" => "sha256sum",
            "sha384" => "sha384sum",
            "sha512" => "sha512sum",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unsupported checksum type: {}",
                    csumtype
                )))
            }
        };

        let mut command = Command::new(cmd);
        command.arg(&host_path);

        let output = command
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute {}: {}", cmd, e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Checksum failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Parse output (format: "checksum  filename")
        let stdout = String::from_utf8_lossy(&output.stdout);
        let checksum = stdout
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::InvalidFormat("Invalid checksum output".to_string()))?;

        Ok(checksum.to_string())
    }

    /// Calculate checksum of a device
    ///
    pub fn checksum_device(&mut self, csumtype: &str, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: checksum_device {} {}", csumtype, device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Parse device name to get partition number
        let partition_num = self.parse_device_name(device)?;

        // Get NBD partition device path
        let nbd = self.nbd_device.as_ref().ok_or_else(|| {
            Error::InvalidState(
                "NBD device not initialized. Call setup_nbd_if_needed() first.".to_string(),
            )
        })?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Map checksum type to command
        let cmd = match csumtype {
            "md5" => "md5sum",
            "sha1" => "sha1sum",
            "sha224" => "sha224sum",
            "sha256" => "sha256sum",
            "sha384" => "sha384sum",
            "sha512" => "sha512sum",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unsupported checksum type: {}",
                    csumtype
                )))
            }
        };

        let output = Command::new(cmd)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute {}: {}", cmd, e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Checksum failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Parse output (format: "checksum  filename")
        let stdout = String::from_utf8_lossy(&output.stdout);
        let checksum = stdout
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::InvalidFormat("Invalid checksum output".to_string()))?;

        Ok(checksum.to_string())
    }

    /// Read first N bytes of a file
    ///
    pub fn head(&mut self, path: &str) -> Result<Vec<String>> {
        self.head_n(10, path)
    }

    /// Read first N lines of a file
    ///
    pub fn head_n(&mut self, nrlines: i32, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: head_n {} {}", nrlines, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("head")
            .arg("-n")
            .arg(nrlines.to_string())
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute head: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Head failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Read last 10 lines of a file
    ///
    pub fn tail(&mut self, path: &str) -> Result<Vec<String>> {
        self.tail_n(10, path)
    }

    /// Read last N lines of a file
    ///
    pub fn tail_n(&mut self, nrlines: i32, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: tail_n {} {}", nrlines, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("tail")
            .arg("-n")
            .arg(nrlines.to_string())
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute tail: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Tail failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Search compressed file for pattern
    ///
    pub fn zgrep(&mut self, regex: &str, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zgrep {} {}", regex, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("zgrep")
            .arg(regex)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zgrep: {}", e)))?;

        // zgrep returns exit code 1 if no matches found, which is not an error
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Search compressed file for pattern (extended regex)
    ///
    pub fn zegrep(&mut self, regex: &str, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zegrep {} {}", regex, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("zgrep")
            .arg("-E")
            .arg(regex)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zegrep: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Search compressed file for fixed strings
    ///
    pub fn zfgrep(&mut self, pattern: &str, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfgrep {} {}", pattern, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("zgrep")
            .arg("-F")
            .arg(pattern)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfgrep: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
