// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Security and SELinux operations for disk image manipulation
//!
//! This implementation provides security context access.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Get SELinux context
    ///
    pub fn getcon(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: getcon");
        }

        // Check if SELinux is enabled
        if !self.exists("/etc/selinux/config")? {
            return Ok("disabled".to_string());
        }

        // Try to read SELinux config
        let config = self.cat("/etc/selinux/config")?;
        for line in config.lines() {
            if line.starts_with("SELINUX=") {
                let status = line.trim_start_matches("SELINUX=").trim();
                return Ok(status.to_string());
            }
        }

        Ok("unknown".to_string())
    }

    /// Set SELinux context
    ///
    pub fn setcon(&mut self, context: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: setcon {}", context);
        }

        // Update SELinux config
        let config_path = "/etc/selinux/config";
        if self.exists(config_path)? {
            let config = self.cat(config_path)?;
            let mut new_config = String::new();

            for line in config.lines() {
                if line.starts_with("SELINUX=") {
                    new_config.push_str(&format!("SELINUX={}\n", context));
                } else {
                    new_config.push_str(line);
                    new_config.push('\n');
                }
            }

            self.write(config_path, new_config.as_bytes())?;
        }

        Ok(())
    }

    /// Get SELinux context of a file
    ///
    pub fn getxattr_selinux(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: getxattr_selinux {}", path);
        }

        match self.getxattr(path, "security.selinux") {
            Ok(data) => Ok(String::from_utf8_lossy(&data)
                .trim_matches('\0')
                .to_string()),
            Err(_) => Ok(String::new()),
        }
    }

    /// Set SELinux context of a file
    ///
    pub fn setxattr_selinux(&mut self, path: &str, context: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: setxattr_selinux {} {}", path, context);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfattr")
            .arg("-n")
            .arg("security.selinux")
            .arg("-v")
            .arg(context)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfattr: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Failed to set SELinux context: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Restore SELinux contexts
    ///
    pub fn selinux_relabel(&mut self, specfile: &str, path: &str, force: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: selinux_relabel {} {}", specfile, path);
        }

        let _host_path = self.resolve_guest_path(path)?;

        let mut cmd = Command::new("chroot");
        let root_mountpoint = self
            .mounted
            .values()
            .next()
            .ok_or_else(|| Error::InvalidState("No filesystem mounted".to_string()))?;

        cmd.arg(root_mountpoint);
        cmd.arg("restorecon");

        if force {
            cmd.arg("-F");
        }

        cmd.arg("-R");
        cmd.arg(path);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute restorecon: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Restorecon failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get AppArmor profile
    ///
    pub fn get_apparmor_profile(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_apparmor_profile {}", path);
        }

        // Check if AppArmor is available
        if !self.exists("/etc/apparmor.d")? {
            return Ok("disabled".to_string());
        }

        // Try to find profile for path
        let profile_name = path.trim_start_matches('/').replace('/', ".");
        let profile_path = format!("/etc/apparmor.d/{}", profile_name);

        if self.exists(&profile_path)? {
            self.cat(&profile_path)
        } else {
            Ok("unconfined".to_string())
        }
    }

    /// Get file capabilities
    ///
    pub fn getcap(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: getcap {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getcap")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getcap: {}", e)))?;

        if !output.status.success() {
            return Ok(String::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse output (format: "path = capabilities")
        if let Some(caps) = stdout.split('=').nth(1) {
            Ok(caps.trim().to_string())
        } else {
            Ok(String::new())
        }
    }

    /// Set file capabilities
    ///
    pub fn setcap(&mut self, cap: &str, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: setcap {} {}", cap, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setcap")
            .arg(cap)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setcap: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Failed to set capabilities: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get file ACL
    ///
    pub fn getfacl(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: getfacl {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("getfacl")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute getfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::NotFound(format!(
                "Failed to get ACL: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Set file ACL
    ///
    pub fn setfacl(&mut self, mode: &str, path: &str, acl: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: setfacl {} {}", mode, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("setfacl")
            .arg(mode)
            .arg(acl)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute setfacl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Failed to set ACL: {}",
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
    fn test_security_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
