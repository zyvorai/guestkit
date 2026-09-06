// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Boot and bootloader operations for disk image manipulation
//!
//! This implementation provides boot configuration access.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get bootloader type
    ///
    pub fn get_bootloader(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_bootloader");
        }

        // Check for GRUB2
        if self.exists("/boot/grub2/grub.cfg")? || self.exists("/boot/grub/grub.cfg")? {
            return Ok("grub2".to_string());
        }

        // Check for legacy GRUB
        if self.exists("/boot/grub/menu.lst")? {
            return Ok("grub".to_string());
        }

        // Check for systemd-boot
        if self.exists("/boot/loader/loader.conf")? {
            return Ok("systemd-boot".to_string());
        }

        // Check for syslinux
        if self.exists("/boot/syslinux/syslinux.cfg")? {
            return Ok("syslinux".to_string());
        }

        Ok("unknown".to_string())
    }

    /// Get default kernel
    ///
    pub fn get_default_kernel(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_default_kernel");
        }

        // Check GRUB2 configuration
        if self.exists("/boot/grub2/grubenv")? {
            let grubenv = self.cat("/boot/grub2/grubenv")?;
            for line in grubenv.lines() {
                if line.starts_with("saved_entry=") {
                    return Ok(line.trim_start_matches("saved_entry=").to_string());
                }
            }
        }

        // Fall back to listing /boot and finding the newest kernel
        if self.exists("/boot")? {
            let entries = self.ls("/boot")?;
            let mut kernels: Vec<String> = entries
                .into_iter()
                .filter(|e| e.starts_with("vmlinuz-") || e.starts_with("vmlinux-"))
                .collect();

            kernels.sort();
            if let Some(kernel) = kernels.last() {
                return Ok(kernel.clone());
            }
        }

        Err(Error::NotFound("No kernel found".to_string()))
    }

    /// List kernels
    ///
    pub fn list_kernels(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_kernels");
        }

        let mut kernels = Vec::new();

        if self.exists("/boot")? {
            let entries = self.ls("/boot")?;
            for entry in entries {
                if entry.starts_with("vmlinuz-") || entry.starts_with("vmlinux-") {
                    // Extract version from filename
                    let version = entry
                        .trim_start_matches("vmlinuz-")
                        .trim_start_matches("vmlinux-");
                    kernels.push(version.to_string());
                }
            }
        }

        kernels.sort();
        Ok(kernels)
    }

    /// Get GRUB configuration
    ///
    pub fn get_grub_config(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_grub_config");
        }

        // Try GRUB2 locations
        for path in &[
            "/boot/grub2/grub.cfg",
            "/boot/grub/grub.cfg",
            "/boot/grub/menu.lst",
        ] {
            if self.exists(path)? {
                return self.cat(path);
            }
        }

        Err(Error::NotFound("GRUB configuration not found".to_string()))
    }

    /// Get initrd/initramfs for kernel
    ///
    pub fn get_initrd(&mut self, kernel: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_initrd {}", kernel);
        }

        // Common initrd naming patterns
        let patterns = vec![
            format!("initramfs-{}.img", kernel),
            format!("initrd.img-{}", kernel),
            format!("initrd-{}", kernel),
            format!("initrd-{}.img", kernel),
        ];

        if self.exists("/boot")? {
            for pattern in patterns {
                let initrd_path = format!("/boot/{}", pattern);
                if self.exists(&initrd_path)? {
                    return Ok(initrd_path);
                }
            }
        }

        Err(Error::NotFound(format!(
            "Initrd not found for kernel {}",
            kernel
        )))
    }

    /// Get kernel command line
    ///
    pub fn get_cmdline(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_cmdline");
        }

        // For offline VM, get default from GRUB config
        let grub_config = self.get_grub_config()?;

        // Parse GRUB config to find kernel command line
        for line in grub_config.lines() {
            let line = line.trim();
            if line.starts_with("linux") || line.starts_with("kernel") {
                // Extract everything after the kernel path
                if let Some(cmdline_start) = line.find("root=") {
                    return Ok(line[cmdline_start..].to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Check if running in UEFI mode
    ///
    pub fn is_uefi(&mut self) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_uefi");
        }

        // Check for EFI system partition
        if self.exists("/boot/efi")? || self.exists("/sys/firmware/efi")? {
            return Ok(true);
        }

        // Check partition table type
        let devices = self.list_devices()?;
        if let Some(device) = devices.first() {
            let part_scheme = self.part_get_parttype(device)?;
            if part_scheme == "gpt" {
                // Check for EFI system partition
                let partitions = self.list_partitions()?;
                for part in partitions {
                    if let Ok(fs) = self.vfs_type(&part) {
                        if fs == "vfat" {
                            // Could be ESP
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get EFI boot entries
    ///
    pub fn list_efi_boot_entries(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_efi_boot_entries");
        }

        let mut entries = Vec::new();

        // Check EFI boot directory
        if self.exists("/boot/efi/EFI")? {
            let efi_entries = self.ls("/boot/efi/EFI")?;
            for entry in efi_entries {
                if entry != "BOOT" && entry != "." && entry != ".." {
                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }

    /// Get fstab entries
    ///
    pub fn read_fstab(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: read_fstab");
        }

        self.cat("/etc/fstab")
    }

    /// Get list of filesystems to mount at boot
    ///
    pub fn list_fstab(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_fstab");
        }

        let fstab = self.read_fstab()?;
        let mut filesystems = Vec::new();

        for line in fstab.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                filesystems.push(format!("{} -> {}", parts[0], parts[1]));
            }
        }

        Ok(filesystems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
