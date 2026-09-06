// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! File attribute operations for disk image manipulation
//!
//! This implementation provides extended file attribute functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Set extended attribute
    ///
    pub fn setxattr(&mut self, xattr: &str, val: &str, vallen: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: setxattr {} {} {} {}", xattr, val, vallen, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfattr")
            .arg("-n")
            .arg(xattr)
            .arg("-v")
            .arg(val)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Remove extended attribute
    ///
    pub fn removexattr(&mut self, xattr: &str, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: removexattr {} {}", xattr, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfattr")
            .arg("-x")
            .arg(xattr)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// List all extended attributes
    ///
    /// Additional functionality for xattr listing
    pub fn listxattrs(&mut self, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: listxattrs {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getfattr")
            .arg("-d")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "getfattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut attrs = Vec::new();

        for line in output_str.lines() {
            if line.starts_with("user.")
                || line.starts_with("security.")
                || line.starts_with("system.")
                || line.starts_with("trusted.")
            {
                if let Some(attr_name) = line.split('=').next() {
                    attrs.push(attr_name.to_string());
                }
            }
        }

        Ok(attrs)
    }

    /// Copy extended attributes
    ///
    /// Additional functionality for xattr copying
    pub fn copy_xattrs(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_xattrs {} {}", src, dest);
        }

        let _src_path = self.resolve_guest_path(src)?;
        let _dest_path = self.resolve_guest_path(dest)?;

        // Get all xattrs from source
        let xattrs = self.listxattrs(src)?;

        // Copy each xattr to destination
        for xattr in xattrs {
            if let Ok(value) = self.getxattr(src, &xattr) {
                if let Ok(value_str) = String::from_utf8(value.clone()) {
                    let _ = self.setxattr(&xattr, &value_str, value.len() as i32, dest);
                }
            }
        }

        Ok(())
    }

    /// Set file attributes (immutable, append-only, etc.)
    ///
    /// Additional functionality for file flags
    pub fn set_file_attrs(&mut self, path: &str, attrs: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_file_attrs {} {}", path, attrs);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("chattr")
            .arg(attrs)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute chattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "chattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get file attributes
    ///
    /// Additional functionality for file flags
    pub fn get_file_attrs(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_file_attrs {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("lsattr")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute lsattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "lsattr failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        Ok(output_str.lines().next().unwrap_or("").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attr_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
