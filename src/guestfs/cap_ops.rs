// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Linux capabilities operations for disk image manipulation
//!
//! This implementation provides file capabilities management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Get file capabilities
    ///
    pub fn cap_get_file(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cap_get_file {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getcap")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getcap: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "getcap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse output like "/path/to/file = cap_net_raw+eip"
        if let Some(caps) = output_str.split('=').nth(1) {
            Ok(caps.trim().to_string())
        } else {
            Ok(String::new())
        }
    }

    /// Set file capabilities
    ///
    pub fn cap_set_file(&mut self, path: &str, cap: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cap_set_file {} {}", path, cap);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setcap")
            .arg(cap)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setcap: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setcap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// List all files with capabilities
    ///
    pub fn cap_list_files(&mut self, directory: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cap_list_files {}", directory);
        }

        let host_path = self.resolve_guest_path(directory)?;

        let output = Command::new("getcap")
            .arg("-r")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getcap: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in output_str.lines() {
            if let Some((file, caps)) = line.split_once('=') {
                files.push((file.trim().to_string(), caps.trim().to_string()));
            }
        }

        Ok(files)
    }

    /// Remove file capabilities
    ///
    pub fn cap_remove_file(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cap_remove_file {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setcap")
            .arg("-r")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setcap: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setcap -r failed: {}",
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
    fn test_cap_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
