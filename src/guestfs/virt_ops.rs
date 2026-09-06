// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Virt operations for disk image manipulation virt-* tools
//!
//! This implementation provides equivalents to virt-* command-line tools.

use crate::core::Result;
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get disk format (like virt-inspector)
    ///
    /// Additional functionality for disk inspection
    pub fn virt_inspector(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: virt_inspector");
        }

        // Gather comprehensive system information
        let mut output = String::new();

        output.push_str("=== Disk Inspector ===\n");

        // OS Information
        if let Ok(roots) = self.inspect_os() {
            for root in roots {
                output.push_str(&format!("\nOS Root: {}\n", root));

                if let Ok(os_type) = self.inspect_get_type(&root) {
                    output.push_str(&format!("  Type: {}\n", os_type));
                }

                if let Ok(distro) = self.inspect_get_distro(&root) {
                    output.push_str(&format!("  Distribution: {}\n", distro));
                }

                if let Ok(version) = self.inspect_get_product_name(&root) {
                    output.push_str(&format!("  Version: {}\n", version));
                }

                if let Ok(arch) = self.inspect_get_arch(&root) {
                    output.push_str(&format!("  Architecture: {}\n", arch));
                }

                if let Ok(hostname) = self.inspect_get_hostname(&root) {
                    output.push_str(&format!("  Hostname: {}\n", hostname));
                }
            }
        }

        // Devices
        if let Ok(devices) = self.list_devices() {
            output.push_str("\nDevices:\n");
            for device in devices {
                output.push_str(&format!("  {}\n", device));
            }
        }

        // Partitions
        if let Ok(partitions) = self.list_partitions() {
            output.push_str("\nPartitions:\n");
            for partition in partitions {
                output.push_str(&format!("  {}\n", partition));
            }
        }

        // Filesystems
        if let Ok(filesystems) = self.list_filesystems() {
            output.push_str("\nFilesystems:\n");
            for (fs, fstype) in filesystems {
                output.push_str(&format!("  {}: {}\n", fs, fstype));
            }
        }

        Ok(output)
    }

    /// Disk format conversion helper
    ///
    /// Additional functionality for disk operations
    pub fn virt_convert_info(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: virt_convert_info");
        }

        let mut output = String::new();
        output.push_str("=== Disk Conversion Info ===\n");

        if let Some(drive) = self.drives.first() {
            output.push_str(&format!("Source: {}\n", drive.path.display()));
            if let Some(ref format) = drive.format {
                output.push_str(&format!("Format: {}\n", format));
            }
        }

        Ok(output)
    }

    /// Filesystem resize helper
    ///
    /// Additional functionality for resize operations
    pub fn virt_resize_info(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: virt_resize_info");
        }

        let mut output = String::new();
        output.push_str("=== Filesystem Resize Info ===\n");

        if let Ok(partitions) = self.list_partitions() {
            for partition in partitions {
                if let Ok(fstype) = self.vfs_type(&partition) {
                    output.push_str(&format!("\nPartition: {}\n", partition));
                    output.push_str(&format!("  Filesystem: {}\n", fstype));

                    // Get size if possible
                    if let Ok(size) = self.blockdev_getsize64(&partition) {
                        output.push_str(&format!("  Size: {} bytes\n", size));
                    }
                }
            }
        }

        Ok(output)
    }

    /// Sparsify helper
    ///
    /// Additional functionality for sparsify operations
    pub fn virt_sparsify_info(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: virt_sparsify_info");
        }

        let mut output = String::new();
        output.push_str("=== Sparsify Info ===\n");

        if let Some(drive) = self.drives.first() {
            output.push_str(&format!("Disk: {}\n", drive.path.display()));
            if let Some(ref format) = drive.format {
                output.push_str(&format!("Format: {}\n", format));
            }

            // Check for filesystems that can be sparsified
            if let Ok(filesystems) = self.list_filesystems() {
                output.push_str("\nFilesystems that can be sparsified:\n");
                for (fs, fstype) in filesystems {
                    if fstype == "ext2" || fstype == "ext3" || fstype == "ext4" || fstype == "xfs" {
                        output.push_str(&format!("  {} ({})\n", fs, fstype));
                    }
                }
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virt_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
