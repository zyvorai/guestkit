// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SELinux operations for disk image manipulation
//!
//! This implementation provides SELinux context management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    // Note: getcon, setcon, selinux_relabel are in security.rs
    // Note: get_selinux, set_selinux are in misc.rs

    /// Check if SELinux is enabled in guest
    ///
    pub fn inspect_get_selinux_enabled(&mut self, root: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_selinux_enabled {}", root);
        }

        // Check for SELinux config file
        let selinux_config = "/etc/selinux/config";
        if !self.exists(selinux_config)? {
            return Ok(false);
        }

        let content = self.cat(selinux_config)?;

        // Look for SELINUX=enforcing or SELINUX=permissive
        for line in content.lines() {
            if line.starts_with("SELINUX=") {
                let value = line.split('=').nth(1).unwrap_or("").trim();
                return Ok(value == "enforcing" || value == "permissive");
            }
        }

        Ok(false)
    }

    /// Get SELinux policy type
    ///
    pub fn inspect_get_selinux_policy(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_selinux_policy {}", root);
        }

        let selinux_config = "/etc/selinux/config";
        if !self.exists(selinux_config)? {
            return Err(Error::NotFound("SELinux not configured".to_string()));
        }

        let content = self.cat(selinux_config)?;

        for line in content.lines() {
            if line.starts_with("SELINUXTYPE=") {
                let value = line.split('=').nth(1).unwrap_or("").trim();
                return Ok(value.to_string());
            }
        }

        Err(Error::NotFound("SELINUXTYPE not found".to_string()))
    }

    /// Restore SELinux contexts recursively
    ///
    pub fn restorecon(&mut self, path: &str, recursive: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: restorecon {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let mut cmd = Command::new("restorecon");

        if recursive {
            cmd.arg("-R");
        }

        cmd.arg(&host_path);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute restorecon: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "restorecon failed: {}",
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
    fn test_selinux_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
