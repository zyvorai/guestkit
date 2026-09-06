// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestfs API for fstab/crypttab rewriting

use crate::core::Result;
use crate::guestfs::device_inventory::{build_inventory, Inventory};
use crate::guestfs::fstab::{rewrite_crypttab, rewrite_fstab, BtrfsSubvolMap};
use crate::guestfs::Guestfs;
use std::collections::HashMap;

impl Guestfs {
    /// Build device inventory from all available block devices
    ///
    /// This queries all partitions, LVM volumes, and other block devices
    /// to build a complete inventory with UUIDs, PARTUUIDs, labels, and types.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new()?;
    /// g.add_drive_ro("/path/to/disk.qcow2")?;
    /// g.launch()?;
    ///
    /// let inventory = g.build_device_inventory()?;
    /// println!("Found {} devices", inventory.by_dev.len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_device_inventory(&mut self) -> Result<Inventory> {
        self.ensure_ready()?;

        let mut devices = Vec::new();

        // Get all partitions
        let partition_table = self.partition_table()?;
        for partition in partition_table.partitions() {
            let device_name = format!("/dev/sda{}", partition.number);
            devices.push(device_name);
        }

        // Get all LVM logical volumes
        if let Ok(lvs) = self.lvs() {
            devices.extend(lvs);
        }

        // Build inventory
        build_inventory(&devices)
    }

    /// Rewrite /etc/fstab with proper UUID/PARTUUID specifications
    ///
    /// This rewrites fstab entries to use:
    /// - PARTUUID= for boot-critical partitions (/, /boot, /boot/efi)
    /// - UUID= for everything else (LVM, mdraid, non-boot partitions)
    /// - Proper subvol= options for btrfs
    ///
    /// Never keeps: /dev/sdX, /dev/vdX, /dev/nbdX, /dev/disk/by-path/*
    ///
    /// # Arguments
    ///
    /// * `root` - Root device (e.g., "/dev/sda1")
    /// * `btrfs_subvols` - Optional btrfs subvolume mapping (mountpoint -> subvol name)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    /// use std::collections::HashMap;
    ///
    /// let mut g = Guestfs::new()?;
    /// g.add_drive_ro("/path/to/disk.qcow2")?;
    /// g.launch()?;
    ///
    /// let roots = g.inspect_os()?;
    /// if let Some(root) = roots.first() {
    ///     g.mount_ro(root, "/")?;
    ///
    ///     // For btrfs, specify subvolumes
    ///     let mut subvols = HashMap::new();
    ///     subvols.insert("/".to_string(), "@".to_string());
    ///     subvols.insert("/home".to_string(), "@home".to_string());
    ///
    ///     g.rewrite_fstab(root, Some(&subvols))?;
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn rewrite_fstab(
        &mut self,
        root: &str,
        btrfs_subvols: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        self.ensure_ready()?;

        // Mount root if not already mounted
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        // Build device inventory
        let inventory = self.build_device_inventory()?;

        // Get fstab path (in mounted filesystem)
        let mount_root = self
            .mount_root
            .as_ref()
            .ok_or_else(|| crate::core::Error::InvalidState("No mount root".to_string()))?;
        let fstab_path = mount_root.join("etc/fstab");

        // Prepare btrfs subvol map
        let btrfs_map: BtrfsSubvolMap = btrfs_subvols.cloned().unwrap_or_default();

        // Rewrite fstab
        rewrite_fstab(&fstab_path, &inventory, &btrfs_map)?;

        if self.verbose {
            eprintln!("guestfs: rewrote /etc/fstab with proper UUID/PARTUUID specs");
        }

        // Unmount if we mounted it
        if !was_mounted {
            self.umount("/")?;
        }

        Ok(())
    }

    /// Rewrite /etc/crypttab with proper LUKS UUIDs
    ///
    /// This rewrites crypttab entries to use UUID= specifications for
    /// LUKS encrypted devices instead of device paths.
    ///
    /// # Arguments
    ///
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new()?;
    /// g.add_drive_ro("/path/to/disk.qcow2")?;
    /// g.launch()?;
    ///
    /// let roots = g.inspect_os()?;
    /// if let Some(root) = roots.first() {
    ///     g.mount_ro(root, "/")?;
    ///     g.rewrite_crypttab(root)?;
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn rewrite_crypttab(&mut self, root: &str) -> Result<()> {
        self.ensure_ready()?;

        // Mount root if not already mounted
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        // Build device inventory
        let inventory = self.build_device_inventory()?;

        // Get crypttab path (in mounted filesystem)
        let mount_root = self
            .mount_root
            .as_ref()
            .ok_or_else(|| crate::core::Error::InvalidState("No mount root".to_string()))?;
        let crypttab_path = mount_root.join("etc/crypttab");

        // Rewrite crypttab (if it exists)
        rewrite_crypttab(&crypttab_path, &inventory)?;

        if self.verbose {
            eprintln!("guestfs: rewrote /etc/crypttab with proper LUKS UUIDs");
        }

        // Unmount if we mounted it
        if !was_mounted {
            self.umount("/")?;
        }

        Ok(())
    }

    /// Rewrite both fstab and crypttab for VM migration
    ///
    /// This is a convenience method that calls both rewrite_fstab and
    /// rewrite_crypttab in the correct order.
    ///
    /// # Arguments
    ///
    /// * `root` - Root device (e.g., "/dev/sda1")
    /// * `btrfs_subvols` - Optional btrfs subvolume mapping
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new()?;
    /// g.add_drive_ro("/path/to/disk.qcow2")?;
    /// g.launch()?;
    ///
    /// let roots = g.inspect_os()?;
    /// if let Some(root) = roots.first() {
    ///     g.mount_ro(root, "/")?;
    ///     g.rewrite_filesystem_configs(root, None)?;
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn rewrite_filesystem_configs(
        &mut self,
        root: &str,
        btrfs_subvols: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        // Rewrite fstab first
        self.rewrite_fstab(root, btrfs_subvols)?;

        // Then crypttab
        self.rewrite_crypttab(root)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_exists() {
        let g = Guestfs::new().unwrap();
        // Just verify the API exists
        let _ = g;
    }
}
