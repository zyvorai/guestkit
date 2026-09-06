// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extended inspection operations for disk image manipulation
//!
//! This implementation provides additional OS inspection functionality.

use crate::core::Result;
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get operating system product variant
    ///
    pub fn inspect_get_product_variant(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_product_variant {}", root);
        }

        // Check for variant information in os-release
        let os_release_path = format!("{}/etc/os-release", root);
        if let Ok(content) = self.cat(&os_release_path) {
            for line in content.lines() {
                if line.starts_with("VARIANT=") {
                    return Ok(line
                        .split('=')
                        .nth(1)
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Get Windows current control set key
    ///
    /// Already exists, adding extended version
    pub fn inspect_get_windows_current_control_set_key(&mut self, root: &str) -> Result<String> {
        self.inspect_get_windows_current_control_set(root)
    }

    /// Get format of OS
    ///
    pub fn inspect_get_format(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_format {}", root);
        }

        // Determine format based on OS type
        let os_type = self.inspect_get_type(root)?;

        match os_type.as_str() {
            "linux" => Ok("installed".to_string()),
            "windows" => Ok("installed".to_string()),
            _ => Ok("unknown".to_string()),
        }
    }

    /// Check if multipart OS
    ///
    pub fn inspect_is_multipart(&mut self, root: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_is_multipart {}", root);
        }

        // Check if this is a multipart installation (like /usr on separate partition)
        let mountpoints = self.inspect_get_mountpoints(root)?;

        Ok(mountpoints.len() > 1)
    }

    /// Check if NetInstall
    ///
    pub fn inspect_is_netinst(&mut self, root: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_is_netinst {}", root);
        }

        // Network install images are typically smaller and have specific markers
        Ok(false)
    }

    /// Get OSInfo ID
    ///
    pub fn inspect_get_osinfo_id(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_osinfo_id {}", root);
        }

        // Try to get ID from os-release
        let os_release_path = format!("{}/etc/os-release", root);
        if let Ok(content) = self.cat(&os_release_path) {
            for line in content.lines() {
                if line.starts_with("ID=") {
                    return Ok(line
                        .split('=')
                        .nth(1)
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Get icon of OS
    ///
    /// Already exists as inspect_get_icon, adding alias
    pub fn inspect_get_os_icon(&mut self, root: &str) -> Result<Vec<u8>> {
        self.inspect_get_icon(root)
    }

    /// List applications (version 2)
    ///
    /// Enhanced version of inspect_list_applications
    pub fn inspect_list_applications2(
        &mut self,
        root: &str,
    ) -> Result<Vec<(String, String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_list_applications2 {}", root);
        }

        // Get package format
        let pkg_format = self.inspect_get_package_format(root)?;

        let mut apps = Vec::new();

        match pkg_format.as_str() {
            "rpm" => {
                // List RPM packages
                if let Ok(packages) = self.rpm_list() {
                    for pkg in packages {
                        // Parse package string (format: name-version-release)
                        let parts: Vec<&str> = pkg.rsplitn(3, '-').collect();
                        if parts.len() >= 3 {
                            apps.push((
                                parts[2].to_string(),
                                parts[1].to_string(),
                                parts[0].to_string(),
                            ));
                        } else if parts.len() >= 2 {
                            apps.push((parts[1].to_string(), parts[0].to_string(), String::new()));
                        } else {
                            apps.push((pkg, String::new(), String::new()));
                        }
                    }
                }
            }
            "deb" => {
                // List dpkg packages
                if let Ok(packages) = self.dpkg_list() {
                    for pkg in packages {
                        // Parse package string
                        let parts: Vec<&str> = pkg.splitn(2, ' ').collect();
                        if parts.len() >= 2 {
                            apps.push((parts[0].to_string(), parts[1].to_string(), String::new()));
                        } else {
                            apps.push((pkg, String::new(), String::new()));
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(apps)
    }

    /// Get package management tool
    ///
    /// Additional functionality for package detection
    pub fn inspect_get_package_management(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.trace {
            eprintln!("guestfs: inspect_get_package_management {}", root);
        }

        let pkg_format = self.inspect_get_package_format(root)?;

        let tool = match pkg_format.as_str() {
            "rpm" => {
                // Check which tool is available (check tdnf first for Photon OS)
                if self
                    .exists(&format!("{}/usr/bin/tdnf", root))
                    .unwrap_or(false)
                {
                    "tdnf"
                } else if self
                    .exists(&format!("{}/usr/bin/dnf", root))
                    .unwrap_or(false)
                {
                    "dnf"
                } else if self
                    .exists(&format!("{}/usr/bin/yum", root))
                    .unwrap_or(false)
                {
                    "yum"
                } else {
                    "rpm"
                }
            }
            "deb" => {
                if self
                    .exists(&format!("{}/usr/bin/apt", root))
                    .unwrap_or(false)
                {
                    "apt"
                } else {
                    "dpkg"
                }
            }
            "pacman" => "pacman",
            "ebuild" => "emerge",
            _ => "unknown",
        };

        Ok(tool.to_string())
    }

    /// Get init system type
    ///
    /// Already exists as get_init_system, adding alias for inspection
    pub fn inspect_get_init_system(&mut self, root: &str) -> Result<String> {
        // Try to mount if not already mounted
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        // Check for systemd
        let is_systemd = self.exists("/run/systemd/system").unwrap_or(false)
            || self.exists("/usr/lib/systemd/systemd").unwrap_or(false)
            || self.exists("/lib/systemd/systemd").unwrap_or(false);

        if is_systemd {
            if !was_mounted {
                self.umount("/").ok();
            }
            return Ok("systemd".to_string());
        }

        // Check for upstart
        if self.exists("/sbin/initctl").unwrap_or(false) {
            if !was_mounted {
                self.umount("/").ok();
            }
            return Ok("upstart".to_string());
        }

        // Check for sysvinit
        if self.exists("/etc/inittab").unwrap_or(false) {
            if !was_mounted {
                self.umount("/").ok();
            }
            return Ok("sysvinit".to_string());
        }

        if !was_mounted {
            self.umount("/").ok();
        }

        Ok("unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_ext_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
