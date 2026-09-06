// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Utility operations for disk image manipulation
//!
//! This implementation provides miscellaneous utility functions.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::fs;
use std::process::Command;

impl Guestfs {
    /// Get file type
    ///
    pub fn file(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: file {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("file")
            .arg("-b") // Brief mode
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute file: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "File command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get file architecture
    ///
    pub fn file_architecture(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: file_architecture {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("file")
            .arg("-b")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute file: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "File command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let file_output = String::from_utf8_lossy(&output.stdout);

        // Parse architecture from file output
        let arch = if file_output.contains("ELF 64-bit") {
            if file_output.contains("x86-64") || file_output.contains("x86_64") {
                "x86_64"
            } else if file_output.contains("aarch64") || file_output.contains("ARM aarch64") {
                "aarch64"
            } else if file_output.contains("PowerPC") {
                "ppc64"
            } else {
                "unknown"
            }
        } else if file_output.contains("ELF 32-bit") {
            if file_output.contains("Intel 80386") || file_output.contains("i386") {
                "i386"
            } else if file_output.contains("ARM") {
                "arm"
            } else if file_output.contains("PowerPC") {
                "ppc"
            } else {
                "unknown"
            }
        } else if file_output.contains("PE32+") {
            "x86_64" // Windows 64-bit
        } else if file_output.contains("PE32") {
            "i386" // Windows 32-bit
        } else {
            "unknown"
        };

        Ok(arch.to_string())
    }

    /// Read symbolic link
    ///
    pub fn readlink(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: readlink {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let link_target = fs::read_link(&host_path)
            .map_err(|e| Error::NotFound(format!("Failed to read symlink {}: {}", path, e)))?;

        Ok(link_target.to_string_lossy().to_string())
    }

    /// Create symbolic link
    ///
    pub fn ln_s(&mut self, target: &str, linkname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ln_s {} {}", target, linkname);
        }

        let linkname_path = self.resolve_guest_path(linkname)?;

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &linkname_path)
                .map_err(|e| Error::CommandFailed(format!("Failed to create symlink: {}", e)))?;
        }

        #[cfg(not(unix))]
        {
            return Err(Error::Unsupported(
                "Symbolic links are only supported on Unix systems".to_string(),
            ));
        }

        Ok(())
    }

    /// Create hard link
    ///
    pub fn ln(&mut self, target: &str, linkname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ln {} {}", target, linkname);
        }

        let target_path = self.resolve_guest_path(target)?;
        let linkname_path = self.resolve_guest_path(linkname)?;

        fs::hard_link(&target_path, &linkname_path)
            .map_err(|e| Error::CommandFailed(format!("Failed to create hard link: {}", e)))
    }

    /// Create hard link (forced)
    ///
    pub fn ln_f(&mut self, target: &str, linkname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ln_f {} {}", target, linkname);
        }

        // Remove existing link if present
        let linkname_path = self.resolve_guest_path(linkname)?;
        let _ = fs::remove_file(&linkname_path); // Ignore errors

        self.ln(target, linkname)
    }

    /// Create symbolic link (forced)
    ///
    pub fn ln_sf(&mut self, target: &str, linkname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ln_sf {} {}", target, linkname);
        }

        // Remove existing link if present
        let linkname_path = self.resolve_guest_path(linkname)?;
        let _ = fs::remove_file(&linkname_path); // Ignore errors

        self.ln_s(target, linkname)
    }

    /// Get extended attribute
    ///
    pub fn getxattr(&mut self, path: &str, name: &str) -> Result<Vec<u8>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: getxattr {} {}", path, name);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getfattr")
            .arg("--only-values")
            .arg("-n")
            .arg(name)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Failed to get extended attribute: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(output.stdout)
    }

    /// List extended attributes
    ///
    pub fn lgetxattrs(&mut self, path: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lgetxattrs {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getfattr")
            .arg("-d")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Failed to list extended attributes: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let attrs: Vec<String> = stdout
            .lines()
            .filter(|line| line.contains('='))
            .map(|line| line.split('=').next().unwrap_or("").trim().to_string())
            .collect();

        Ok(attrs)
    }

    /// Get file flags
    ///
    pub fn get_e2attrs(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_e2attrs {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("lsattr")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute lsattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Failed to get e2 attributes: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse lsattr output (format: "flags filename")
        let attrs = stdout.split_whitespace().next().unwrap_or("").to_string();

        Ok(attrs)
    }

    /// Set file flags
    ///
    pub fn set_e2attrs(&mut self, path: &str, attrs: &str, clear: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_e2attrs {} {}", path, attrs);
        }

        let host_path = self.resolve_guest_path(path)?;

        let flag = if clear { "-" } else { "+" };
        let attr_arg = format!("{}{}", flag, attrs);

        let output = Command::new("chattr")
            .arg(&attr_arg)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute chattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Failed to set e2 attributes: {}",
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
    fn test_utils_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
