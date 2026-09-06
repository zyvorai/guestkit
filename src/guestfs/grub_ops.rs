// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GRUB bootloader operations for disk image manipulation
//!
//! This implementation provides GRUB bootloader installation and configuration.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Install GRUB bootloader
    ///
    pub fn grub_install(&mut self, root: &str, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_install {} {}", root, device);
        }

        let host_root = self.resolve_guest_path(root)?;

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("grub-install")
            .arg("--boot-directory")
            .arg(format!("{}/boot", host_root.display()))
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute grub-install: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "grub-install failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Read GRUB configuration
    ///
    /// Additional functionality for GRUB support
    pub fn grub_read_config(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_read_config {}", path);
        }

        let grub_cfg = if path.is_empty() {
            "/boot/grub/grub.cfg"
        } else {
            path
        };

        self.cat(grub_cfg)
    }

    /// List GRUB menu entries
    ///
    /// Additional functionality for GRUB support
    pub fn grub_list_entries(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_list_entries");
        }

        // Try multiple common GRUB config locations
        let config_paths = vec![
            "/boot/grub/grub.cfg",
            "/boot/grub2/grub.cfg",
            "/boot/grub/menu.lst",
        ];

        for config_path in config_paths {
            if self.exists(config_path).unwrap_or(false) {
                let content = self.cat(config_path)?;
                let mut entries = Vec::new();

                for line in content.lines() {
                    if line.trim().starts_with("menuentry") {
                        // Extract menu entry title
                        if let Some(start) = line.find('\'') {
                            if let Some(end) = line[start + 1..].find('\'') {
                                entries.push(line[start + 1..start + 1 + end].to_string());
                            }
                        } else if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                entries.push(line[start + 1..start + 1 + end].to_string());
                            }
                        }
                    }
                }

                if !entries.is_empty() {
                    return Ok(entries);
                }
            }
        }

        Ok(Vec::new())
    }

    /// Get default GRUB entry
    ///
    /// Additional functionality for GRUB support
    pub fn grub_get_default(&mut self) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_get_default");
        }

        // Check /etc/default/grub
        if self.exists("/etc/default/grub").unwrap_or(false) {
            let content = self.cat("/etc/default/grub")?;

            for line in content.lines() {
                if line.starts_with("GRUB_DEFAULT=") {
                    if let Some(value) = line.split('=').nth(1) {
                        let value = value.trim().trim_matches('"');
                        if let Ok(default) = value.parse::<i32>() {
                            return Ok(default);
                        }
                    }
                }
            }
        }

        Ok(0)
    }

    /// Set default GRUB entry
    ///
    /// Additional functionality for GRUB support
    pub fn grub_set_default(&mut self, entry: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_set_default {}", entry);
        }

        let grub_default = "/etc/default/grub";

        if !self.exists(grub_default).unwrap_or(false) {
            return Err(Error::NotFound("GRUB default file not found".to_string()));
        }

        let mut content = self.cat(grub_default)?;
        let mut updated = false;

        // Update GRUB_DEFAULT line
        let lines: Vec<String> = content
            .lines()
            .map(|line| {
                if line.starts_with("GRUB_DEFAULT=") {
                    updated = true;
                    format!("GRUB_DEFAULT={}", entry)
                } else {
                    line.to_string()
                }
            })
            .collect();

        content = lines.join("\n");

        // Add GRUB_DEFAULT if it wasn't found
        if !updated {
            content.push_str(&format!("\nGRUB_DEFAULT={}\n", entry));
        }

        self.write(grub_default, content.as_bytes())?;

        Ok(())
    }

    /// Update GRUB configuration
    ///
    /// Additional functionality for GRUB support
    pub fn grub_update(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: grub_update");
        }

        let host_root = self
            .mounted
            .values()
            .next()
            .ok_or_else(|| Error::InvalidState("No filesystem mounted".to_string()))?
            .clone();

        // Try update-grub or grub2-mkconfig
        let commands = vec!["update-grub", "grub2-mkconfig"];

        for cmd_name in commands {
            let output = Command::new("chroot")
                .arg(&host_root)
                .arg(cmd_name)
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    return Ok(());
                }
            }
        }

        Err(Error::CommandFailed(
            "Failed to update GRUB configuration".to_string(),
        ))
    }

    /// Offline GRUB repair: chroot `grub-mkconfig`/`update-grub` with bind mounts,
    /// optional `grub-install` onto the NBD/block device when `install` is true,
    /// and first-boot oneshot fallback when chroot mkconfig is unavailable.
    pub fn repair_grub(
        &mut self,
        install: bool,
    ) -> Result<crate::guestfs::grub_repair::GrubRepairReport> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: repair_grub install={}", install);
        }

        let root = std::path::PathBuf::from(self.find_root_mountpoint()?);
        let install_dev = if install {
            let _ = self.setup_nbd_if_needed();
            self.nbd_device
                .as_ref()
                .map(|nbd| nbd.device_path().to_path_buf())
                .or_else(|| {
                    self.drives
                        .first()
                        .and_then(|d| crate::guestfs::grub_repair::infer_install_device(&d.path))
                })
        } else {
            None
        };

        crate::guestfs::grub_repair::repair_grub(&root, install_dev.as_deref(), self.verbose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grub_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
