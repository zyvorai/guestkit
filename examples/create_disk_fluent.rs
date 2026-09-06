// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Create a disk image using the ergonomic fluent API
//!
//! This example demonstrates the new type-safe, fluent API for creating
//! and configuring disk images. Compare with older string-based approaches.
//!
//! ⚠️  REQUIRES: sudo/root permissions for NBD mounting
//!
//! Usage:
//!   sudo cargo run --example create_disk_fluent

use guestkit::guestfs::{Guestfs, PartitionTableType};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Creating Disk Image with Fluent API ===\n");

    let disk_path = "/tmp/fluent-example.img";
    let disk_size_mb = 512;

    // Cleanup old image
    let _ = fs::remove_file(disk_path);

    // Create guest using builder pattern - fluent and type-safe!
    println!("[1/8] Creating guest with builder pattern...");
    let mut guest = Guestfs::builder()
        .verbose(false) // Control output
        .autosync(true) // Auto-sync on close
        .identifier("fluent-example") // Give it a name
        .build()?;

    // Create sparse disk image
    println!("[2/8] Creating {} MB disk image...", disk_size_mb);
    let disk_size_bytes = (disk_size_mb * 1024 * 1024) as i64;
    guest.disk_create(disk_path, "raw", disk_size_bytes)?;

    // Add drive and launch
    println!("[3/8] Adding drive and launching...");
    guest.add_drive(disk_path)?;
    guest.launch()?;

    // Initialize partition table using type-safe enum
    println!("[4/8] Creating GPT partition table...");
    guest.part_init("/dev/sda", PartitionTableType::Gpt.as_str())?;

    // Create EFI and root partitions
    println!("[5/8] Creating partitions...");

    // EFI System Partition: 100MB
    let esp_start = 2048;
    let esp_end = esp_start + ((100 * 1024 * 1024) / 512) - 1;
    guest.part_add("/dev/sda", "p", esp_start, esp_end)?;
    guest.part_set_gpt_type("/dev/sda", 1, "c12a7328-f81f-11d2-ba4b-00a0c93ec93b")?;
    guest.part_set_name("/dev/sda", 1, "EFI System Partition")?;

    // Root partition: rest of disk
    let root_start = esp_end + 1;
    guest.part_add("/dev/sda", "p", root_start, -34)?;
    guest.part_set_name("/dev/sda", 2, "Linux Root")?;

    // Create filesystems using fluent API - type-safe and self-documenting!
    println!("[6/8] Creating filesystems...");

    // VFAT for EFI
    guest.mkfs("vfat", "/dev/sda1")?;
    guest.set_label("/dev/sda1", "EFI")?;

    // BTRFS for root
    guest.mkfs("btrfs", "/dev/sda2")?;
    guest.set_label("/dev/sda2", "rootfs")?;

    // Mount filesystems
    println!("[7/8] Mounting filesystems...");
    guest.mount("/dev/sda2", "/")?;
    guest.mkdir("/boot/efi")?;
    guest.mount("/dev/sda1", "/boot/efi")?;

    // Create directory structure
    println!("[8/8] Setting up directory structure...");
    guest.mkdir_p("/etc")?;
    guest.mkdir_p("/home")?;
    guest.mkdir_p("/var/log")?;
    guest.mkdir_p("/usr/bin")?;

    // Write configuration files
    guest.write("/etc/hostname", b"fluent-example\n")?;
    guest.write(
        "/etc/fstab",
        b"/dev/sda2 / btrfs defaults 0 0\n\
          /dev/sda1 /boot/efi vfat umask=0077 0 1\n",
    )?;

    guest.write(
        "/etc/os-release",
        b"NAME=\"Example Linux\"\n\
          VERSION=\"1.0\"\n\
          ID=example\n\
          PRETTY_NAME=\"Example Linux (Fluent API Demo)\"\n",
    )?;

    // Create some test files
    guest.write(
        "/home/README.txt",
        b"This disk was created using GuestKit's fluent API!\n\n\
          Key features demonstrated:\n\
          - Builder pattern for guest creation\n\
          - Type-safe filesystem enums (VFAT, BTRFS, etc.)\n\
          - Self-documenting fluent methods\n\
          - Compile-time safety\n",
    )?;

    // Verify what we created
    println!("\n--- Verification ---");
    println!(
        "Hostname: {}",
        String::from_utf8_lossy(&guest.read_file("/etc/hostname")?).trim()
    );
    println!("Root filesystem: {}", guest.vfs_type("/dev/sda2")?);
    println!("EFI filesystem: {}", guest.vfs_type("/dev/sda1")?);

    let files = guest.ls("/etc")?;
    println!("Files in /etc: {}", files.join(", "));

    // Cleanup
    println!("\n--- Cleanup ---");
    guest.sync()?;
    guest.umount_all()?;
    guest.shutdown()?;

    println!("\n✓ Disk image created successfully!");
    println!("  Location: {}", disk_path);
    println!("  Size: {} MB", disk_size_mb);
    println!("  Partitions: GPT with EFI + Root");
    println!("  Filesystems: VFAT (EFI) + BTRFS (root)");
    println!("\nYou can inspect it with:");
    println!("  guestkit inspect {}", disk_path);

    Ok(())
}
