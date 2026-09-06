// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! System configuration operations for disk image manipulation
//!
//! This implementation provides system-level configuration access.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::fs;

impl Guestfs {
    /// Get timezone
    ///
    pub fn get_timezone(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_timezone");
        }

        // Check /etc/timezone (Debian/Ubuntu)
        if self.exists("/etc/timezone")? {
            let tz = self.cat("/etc/timezone")?;
            return Ok(tz.trim().to_string());
        }

        // Check /etc/localtime symlink (modern systemd)
        if self.exists("/etc/localtime")? {
            let host_path = self.resolve_guest_path("/etc/localtime")?;
            if let Ok(link_target) = fs::read_link(&host_path) {
                let target_str = link_target.to_string_lossy();
                if let Some(tz) = target_str.strip_prefix("/usr/share/zoneinfo/") {
                    return Ok(tz.to_string());
                }
                // Handle absolute path
                if let Some(tz) = target_str.strip_prefix("../usr/share/zoneinfo/") {
                    return Ok(tz.to_string());
                }
            }
        }

        Err(Error::NotFound("Could not determine timezone".to_string()))
    }

    /// Set timezone
    ///
    pub fn set_timezone(&mut self, timezone: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_timezone {}", timezone);
        }

        // Write /etc/timezone (Debian/Ubuntu)
        self.write("/etc/timezone", format!("{}\n", timezone).as_bytes())?;

        // Create symlink for /etc/localtime
        let zoneinfo_path = format!("/usr/share/zoneinfo/{}", timezone);
        if self.exists(&zoneinfo_path)? {
            // Remove old localtime
            if self.exists("/etc/localtime")? {
                let _ = self.rm("/etc/localtime");
            }

            // Create symlink
            self.ln_s(&zoneinfo_path, "/etc/localtime")?;
        }

        Ok(())
    }

    /// Get system locale
    ///
    pub fn get_locale(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_locale");
        }

        // Check /etc/default/locale (Debian/Ubuntu)
        if self.exists("/etc/default/locale")? {
            let content = self.cat("/etc/default/locale")?;
            for line in content.lines() {
                if line.starts_with("LANG=") {
                    let locale = line.trim_start_matches("LANG=").trim();
                    return Ok(locale.trim_matches('"').to_string());
                }
            }
        }

        // Check /etc/locale.conf (RHEL/Fedora/systemd)
        if self.exists("/etc/locale.conf")? {
            let content = self.cat("/etc/locale.conf")?;
            for line in content.lines() {
                if line.starts_with("LANG=") {
                    let locale = line.trim_start_matches("LANG=").trim();
                    return Ok(locale.trim_matches('"').to_string());
                }
            }
        }

        Ok("C".to_string()) // Default locale
    }

    /// Set system locale
    ///
    pub fn set_locale(&mut self, locale: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_locale {}", locale);
        }

        // Update /etc/default/locale (Debian/Ubuntu)
        let locale_content = format!("LANG={}\n", locale);
        self.write("/etc/default/locale", locale_content.as_bytes())?;

        // Update /etc/locale.conf (RHEL/Fedora/systemd)
        self.write("/etc/locale.conf", locale_content.as_bytes())?;

        Ok(())
    }

    /// Get OS version
    ///
    pub fn get_osinfo(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_osinfo");
        }

        // Try /etc/os-release first (modern systems)
        if self.exists("/etc/os-release")? {
            return self.cat("/etc/os-release");
        }

        // Try /etc/lsb-release (Ubuntu/Debian)
        if self.exists("/etc/lsb-release")? {
            return self.cat("/etc/lsb-release");
        }

        // Try legacy release files
        for release_file in &[
            "/etc/redhat-release",
            "/etc/debian_version",
            "/etc/fedora-release",
            "/etc/centos-release",
        ] {
            if self.exists(release_file)? {
                return self.cat(release_file);
            }
        }

        Err(Error::NotFound("Could not determine OS info".to_string()))
    }

    /// Get kernel version
    ///
    pub fn get_kernel_version(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_kernel_version");
        }

        // Try running uname in chroot
        match self.command(&["uname", "-r"]) {
            Ok(version) => Ok(version.trim().to_string()),
            Err(_) => {
                // Fallback: check /proc/version if mounted
                if self.exists("/proc/version")? {
                    let version = self.cat("/proc/version")?;
                    // Parse version from /proc/version
                    if let Some(start) = version.find("Linux version ") {
                        let version_part = &version[start + 14..];
                        if let Some(space) = version_part.find(' ') {
                            return Ok(version_part[..space].to_string());
                        }
                    }
                }
                Err(Error::NotFound(
                    "Could not determine kernel version".to_string(),
                ))
            }
        }
    }

    /// Get system uptime
    ///
    pub fn get_uptime(&mut self) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_uptime");
        }

        // For offline VMs, uptime doesn't make sense
        // Return 0 to indicate system is not running
        Ok(0)
    }

    /// Get machine ID
    ///
    pub fn get_machine_id(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_machine_id");
        }

        // Check /etc/machine-id (systemd)
        if self.exists("/etc/machine-id")? {
            let machine_id = self.cat("/etc/machine-id")?;
            return Ok(machine_id.trim().to_string());
        }

        // Check /var/lib/dbus/machine-id (older systems)
        if self.exists("/var/lib/dbus/machine-id")? {
            let machine_id = self.cat("/var/lib/dbus/machine-id")?;
            return Ok(machine_id.trim().to_string());
        }

        Err(Error::NotFound("Machine ID not found".to_string()))
    }

    /// Get systemd units
    ///
    pub fn list_systemd_units(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_systemd_units");
        }

        let mut units = Vec::new();

        // List /etc/systemd/system
        if self.exists("/etc/systemd/system")? {
            if let Ok(entries) = self.ls("/etc/systemd/system") {
                for entry in entries {
                    if entry.ends_with(".service") || entry.ends_with(".target") {
                        units.push(entry);
                    }
                }
            }
        }

        // List /lib/systemd/system
        if self.exists("/lib/systemd/system")? {
            if let Ok(entries) = self.ls("/lib/systemd/system") {
                for entry in entries {
                    if (entry.ends_with(".service") || entry.ends_with(".target"))
                        && !units.contains(&entry)
                    {
                        units.push(entry);
                    }
                }
            }
        }

        units.sort();
        Ok(units)
    }

    /// Get environment variables
    ///
    pub fn get_environment(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_environment");
        }

        // Try to get environment from /etc/environment
        let mut env_vars = Vec::new();

        if self.exists("/etc/environment")? {
            let content = self.cat("/etc/environment")?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    env_vars.push(line.to_string());
                }
            }
        }

        Ok(env_vars)
    }

    /// Get system users
    ///
    pub fn list_users(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_users");
        }

        let mut users = Vec::new();

        if self.exists("/etc/passwd")? {
            let passwd = self.cat("/etc/passwd")?;
            for line in passwd.lines() {
                if let Some(username) = line.split(':').next() {
                    users.push(username.to_string());
                }
            }
        }

        Ok(users)
    }

    /// Get system groups
    ///
    pub fn list_groups(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_groups");
        }

        let mut groups = Vec::new();

        if self.exists("/etc/group")? {
            let group_file = self.cat("/etc/group")?;
            for line in group_file.lines() {
                if let Some(groupname) = line.split(':').next() {
                    groups.push(groupname.to_string());
                }
            }
        }

        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
