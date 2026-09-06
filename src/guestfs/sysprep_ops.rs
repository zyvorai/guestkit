// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SysPrep operations for disk image manipulation
//!
//! This implementation provides Windows VM preparation functionality (removing unique data).

use crate::core::Result;
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Remove bash history
    ///
    pub fn sysprep_bash_history(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_bash_history");
        }

        // Remove .bash_history for all users
        let users = vec!["/root", "/home/*"];

        for user_dir in users {
            let history_path = format!("{}/.bash_history", user_dir);
            if self.exists(&history_path).unwrap_or(false) {
                self.rm(&history_path)?;
            }
        }

        Ok(())
    }

    /// Remove SSH host keys
    ///
    pub fn sysprep_ssh_hostkeys(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_ssh_hostkeys");
        }

        // Remove SSH host keys
        let key_patterns = vec!["/etc/ssh/ssh_host_*_key", "/etc/ssh/ssh_host_*_key.pub"];

        for pattern in key_patterns {
            if let Ok(files) = self.glob_expand(pattern) {
                for file in files {
                    if self.exists(&file).unwrap_or(false) {
                        self.rm(&file)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove network configuration
    ///
    pub fn sysprep_net_hwaddr(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_net_hwaddr");
        }

        // Remove udev network rules (which contain MAC addresses)
        let udev_rules = "/etc/udev/rules.d/70-persistent-net.rules";
        if self.exists(udev_rules).unwrap_or(false) {
            self.rm(udev_rules)?;
        }

        // Remove NetworkManager connection UUIDs
        let nm_dir = "/etc/NetworkManager/system-connections";
        if self.exists(nm_dir).unwrap_or(false) {
            if let Ok(files) = self.ls(nm_dir) {
                for file in files {
                    let path = format!("{}/{}", nm_dir, file);
                    if self.exists(&path).unwrap_or(false) {
                        self.rm(&path)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove machine ID
    ///
    pub fn sysprep_machine_id(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_machine_id");
        }

        // Remove systemd machine ID
        let machine_id_paths = vec!["/etc/machine-id", "/var/lib/dbus/machine-id"];

        for path in machine_id_paths {
            if self.exists(path).unwrap_or(false) {
                // Truncate the file instead of removing it
                self.truncate(path)?;
            }
        }

        Ok(())
    }

    /// Remove log files
    ///
    pub fn sysprep_logfiles(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_logfiles");
        }

        // Remove log files
        let log_patterns = vec!["/var/log/*.log", "/var/log/*/*.log"];

        for pattern in log_patterns {
            if let Ok(files) = self.glob_expand(pattern) {
                for file in files {
                    if self.exists(&file).unwrap_or(false) {
                        self.rm(&file)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove temporary files
    ///
    pub fn sysprep_tmp_files(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_tmp_files");
        }

        // Remove temporary files
        let tmp_dirs = vec!["/tmp", "/var/tmp"];

        for dir in tmp_dirs {
            if self.exists(dir).unwrap_or(false) {
                if let Ok(files) = self.ls(dir) {
                    for file in files {
                        let path = format!("{}/{}", dir, file);
                        if self.is_file(&path).unwrap_or(false) {
                            self.rm(&path)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove package manager cache
    ///
    pub fn sysprep_package_cache(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_package_cache");
        }

        // Remove package manager caches
        let cache_dirs = vec!["/var/cache/yum", "/var/cache/dnf", "/var/cache/apt"];

        for dir in cache_dirs {
            if self.exists(dir).unwrap_or(false) {
                if let Ok(files) = self.find(dir) {
                    for file in files {
                        if self.is_file(&file).unwrap_or(false) {
                            self.rm(&file)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Run all sysprep operations
    ///
    pub fn sysprep_all(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sysprep_all");
        }

        // Run all sysprep operations
        self.sysprep_bash_history()?;
        self.sysprep_ssh_hostkeys()?;
        self.sysprep_net_hwaddr()?;
        self.sysprep_machine_id()?;
        self.sysprep_logfiles()?;
        self.sysprep_tmp_files()?;
        self.sysprep_package_cache()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysprep_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
