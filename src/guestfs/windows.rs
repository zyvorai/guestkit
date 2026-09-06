// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows-specific operations for disk image manipulation
//!
//! This implementation provides Windows registry and Windows-specific functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::collections::HashMap;

impl Guestfs {
    /// Get Windows systemroot
    ///
    pub fn inspect_get_windows_systemroot(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_windows_systemroot {}", root);
        }

        // Check common Windows systemroot locations
        let common_paths = vec!["/Windows", "/WINDOWS", "/windows", "/WinNT"];

        for path in common_paths {
            if self.exists(path)? {
                return Ok(path.to_string());
            }
        }

        // Try to read from registry if available
        Err(Error::NotFound("Windows systemroot not found".to_string()))
    }

    /// Get Windows current control set
    ///
    pub fn inspect_get_windows_current_control_set(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_windows_current_control_set {}", root);
        }

        // Default control set
        // In a full implementation, this would read from the registry
        Ok("ControlSet001".to_string())
    }

    /// List Windows drivers
    ///
    pub fn inspect_list_windows_drivers(&mut self, root: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_list_windows_drivers {}", root);
        }

        let mut drivers = Vec::new();

        // Check Windows\System32\drivers directory
        let systemroot = self.inspect_get_windows_systemroot(root)?;
        let drivers_path = format!("{}/System32/drivers", systemroot);

        if self.exists(&drivers_path)? {
            let entries = self.ls(&drivers_path)?;
            for entry in entries {
                if entry.ends_with(".sys") {
                    drivers.push(entry);
                }
            }
        }

        Ok(drivers)
    }

    /// Get Windows software hive path
    ///
    pub fn inspect_get_windows_software_hive(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_windows_software_hive {}", root);
        }

        let systemroot = self.inspect_get_windows_systemroot(root)?;
        let software_hive = format!("{}/System32/config/SOFTWARE", systemroot);

        if self.exists(&software_hive)? {
            Ok(software_hive)
        } else {
            Err(Error::NotFound("SOFTWARE hive not found".to_string()))
        }
    }

    /// Get Windows system hive path
    ///
    pub fn inspect_get_windows_system_hive(&mut self, root: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_windows_system_hive {}", root);
        }

        let systemroot = self.inspect_get_windows_systemroot(root)?;
        let system_hive = format!("{}/System32/config/SYSTEM", systemroot);

        if self.exists(&system_hive)? {
            Ok(system_hive)
        } else {
            Err(Error::NotFound("SYSTEM hive not found".to_string()))
        }
    }

    /// Check if Windows is hibernated
    ///
    pub fn is_windows_hibernated(&mut self) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_windows_hibernated");
        }

        // Check for hiberfil.sys
        if self.exists("/hiberfil.sys")? {
            // Check file size - if > 0, Windows is hibernated
            if let Ok(size) = self.filesize("/hiberfil.sys") {
                return Ok(size > 0);
            }
        }

        Ok(false)
    }

    /// Map Windows drive letters
    ///
    pub fn inspect_get_drive_mappings(&mut self, root: &str) -> Result<HashMap<String, String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_drive_mappings {}", root);
        }

        let mut mappings = HashMap::new();

        // In a full implementation, this would read from the registry
        // For now, provide common mappings
        mappings.insert("C".to_string(), "/dev/sda1".to_string());

        Ok(mappings)
    }

    /// Get Windows version from product name
    ///
    pub fn inspect_get_windows_version(&mut self, root: &str) -> Result<(i32, i32)> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_windows_version {}", root);
        }

        // Try to get version from product name
        let product_name = self.inspect_get_product_name(root).unwrap_or_default();

        // Parse common Windows versions
        let (major, minor) =
            if product_name.contains("Windows 11") || product_name.contains("Windows 10") {
                (10, 0) // Windows 11 is actually 10.0
            } else if product_name.contains("Windows 8.1") {
                (6, 3)
            } else if product_name.contains("Windows 8") {
                (6, 2)
            } else if product_name.contains("Windows 7") {
                (6, 1)
            } else if product_name.contains("Vista") {
                (6, 0)
            } else if product_name.contains("XP") {
                (5, 1)
            } else if product_name.contains("2000") {
                (5, 0)
            } else {
                (0, 0)
            };

        Ok((major, minor))
    }

    /// Download registry hive
    ///
    pub fn download_hive(&mut self, hive_path: &str, local_path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: download_hive {} {}", hive_path, local_path);
        }

        // Download the registry hive file
        self.download(hive_path, local_path)
    }

    /// Upload registry hive
    ///
    pub fn upload_hive(&mut self, local_path: &str, hive_path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: upload_hive {} {}", local_path, hive_path);
        }

        // Upload the registry hive file
        self.upload(local_path, hive_path)
    }

    /// Get icon from Windows executable
    ///
    pub fn inspect_get_icon(&mut self, root: &str) -> Result<Vec<u8>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: inspect_get_icon {}", root);
        }

        // In a full implementation, this would extract the icon from Windows executables
        // For now, return empty
        Ok(Vec::new())
    }

    /// Detect if system uses UEFI
    ///
    pub fn is_efi_system(&mut self) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_efi_system");
        }

        // Check for EFI system partition
        if self.exists("/EFI")? {
            return Ok(true);
        }

        // Check for UEFI boot files
        if self.exists("/boot/efi")? {
            return Ok(true);
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
