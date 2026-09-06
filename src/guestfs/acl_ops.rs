// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! ACL (Access Control List) operations for disk image manipulation
//!
//! This implementation provides POSIX ACL management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Get POSIX ACL
    ///
    pub fn acl_get_file(&mut self, path: &str, acltype: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_get_file {} {}", path, acltype);
        }

        let host_path = self.resolve_guest_path(path)?;

        let mut cmd = Command::new("getfacl");

        match acltype {
            "access" => {
                cmd.arg("--access");
            }
            "default" => {
                cmd.arg("--default");
            }
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid ACL type: {}",
                    acltype
                )));
            }
        }

        cmd.arg(&host_path);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "getfacl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Set POSIX ACL
    ///
    pub fn acl_set_file(&mut self, path: &str, acltype: &str, acl: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_set_file {} {}", path, acltype);
        }

        let host_path = self.resolve_guest_path(path)?;

        // Write ACL to temporary file
        let temp_acl = format!(
            "/tmp/guestfs-acl-{}-{}.txt",
            std::process::id(),
            uuid::Uuid::new_v4()
        );
        std::fs::write(&temp_acl, acl).map_err(Error::Io)?;

        let mut cmd = Command::new("setfacl");

        match acltype {
            "access" => {
                cmd.arg("-M").arg(&temp_acl);
            }
            "default" => {
                cmd.arg("-d").arg("-M").arg(&temp_acl);
            }
            _ => {
                std::fs::remove_file(&temp_acl).ok();
                return Err(Error::InvalidFormat(format!(
                    "Invalid ACL type: {}",
                    acltype
                )));
            }
        }

        cmd.arg(&host_path);

        let output = cmd.output().map_err(|e| {
            std::fs::remove_file(&temp_acl).ok();
            Error::CommandFailed(format!("Failed to execute setfacl: {}", e))
        })?;

        std::fs::remove_file(&temp_acl).ok();

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Delete POSIX ACL
    ///
    pub fn acl_delete_def_file(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_delete_def_file {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfacl")
            .arg("-k")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl -k failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Remove all ACLs
    ///
    pub fn acl_remove_all(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_remove_all {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfacl")
            .arg("-b")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl -b failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set ACL entry
    ///
    pub fn acl_set_entry(&mut self, path: &str, entry: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_set_entry {} {}", path, entry);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfacl")
            .arg("-m")
            .arg(entry)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl -m failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Remove ACL entry
    ///
    pub fn acl_remove_entry(&mut self, path: &str, entry: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_remove_entry {} {}", path, entry);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfacl")
            .arg("-x")
            .arg(entry)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl -x failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Copy ACL from one file to another
    ///
    pub fn acl_copy(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: acl_copy {} {}", src, dest);
        }

        let host_src = self.resolve_guest_path(src)?;
        let host_dest = self.resolve_guest_path(dest)?;

        // Get ACL from source
        let output = Command::new("getfacl")
            .arg(&host_src)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "getfacl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Write to temporary file
        let temp_acl = format!(
            "/tmp/guestfs-acl-copy-{}-{}.txt",
            std::process::id(),
            uuid::Uuid::new_v4()
        );
        std::fs::write(&temp_acl, &output.stdout).map_err(Error::Io)?;

        // Apply to destination
        let output = Command::new("setfacl")
            .arg("--set-file")
            .arg(&temp_acl)
            .arg(&host_dest)
            .output()
            .map_err(|e| {
                std::fs::remove_file(&temp_acl).ok();
                Error::CommandFailed(format!("Failed to execute setfacl: {}", e))
            })?;

        std::fs::remove_file(&temp_acl).ok();

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "setfacl --set-file failed: {}",
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
    fn test_acl_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
