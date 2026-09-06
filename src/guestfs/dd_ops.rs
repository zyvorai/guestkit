// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! DD and copy operations for disk image manipulation
//!
//! This implementation provides dd-style copy operations.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Copy from source to destination
    ///
    pub fn dd(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: dd {} {}", src, dest);
        }

        let src_path = self.resolve_guest_path(src)?;
        let dest_path = self.resolve_guest_path(dest)?;

        let output = Command::new("dd")
            .arg(format!("if={}", src_path.display()))
            .arg(format!("of={}", dest_path.display()))
            .arg("bs=1M")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "dd failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Copy with count and skip
    ///
    /// Additional functionality for dd operations
    pub fn dd_opts(
        &mut self,
        src: &str,
        dest: &str,
        count: Option<i64>,
        skip: Option<i64>,
        seek: Option<i64>,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: dd_opts {} {} {:?} {:?} {:?}",
                src, dest, count, skip, seek
            );
        }

        let src_path = self.resolve_guest_path(src)?;
        let dest_path = self.resolve_guest_path(dest)?;

        let mut cmd = Command::new("dd");
        cmd.arg(format!("if={}", src_path.display()))
            .arg(format!("of={}", dest_path.display()))
            .arg("bs=512");

        if let Some(c) = count {
            cmd.arg(format!("count={}", c));
        }

        if let Some(s) = skip {
            cmd.arg(format!("skip={}", s));
        }

        if let Some(s) = seek {
            cmd.arg(format!("seek={}", s));
        }

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "dd failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Zero blocks on device
    ///
    pub fn zero_device(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zero_device {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let _output = Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", nbd_device_path.display()))
            .arg("bs=1M")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {}", e)))?;

        // dd may return error when disk is full, which is expected
        Ok(())
    }

    /// Zero N bytes on device
    ///
    pub fn zero(&mut self, device: &str) -> Result<()> {
        self.zero_device(device)
    }

    /// Zero with free space
    ///
    pub fn zero_free_space_extended(&mut self, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zero_free_space_extended {}", directory);
        }

        let host_path = self.resolve_guest_path(directory)?;

        // Create a large file filled with zeros until disk is full
        let temp_file = host_path.join("guestkit_zero_temp");

        let _output = Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", temp_file.display()))
            .arg("bs=1M")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {}", e)))?;

        // Remove temp file
        let _ = std::fs::remove_file(&temp_file);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dd_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
