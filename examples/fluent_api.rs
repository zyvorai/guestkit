// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the ergonomic, ergonomic GuestKit API
//!
//! This example shows how to use the new builder patterns, type-safe enums,
//! and fluent interfaces to work with disk images.

use guestkit::guestfs::{FilesystemType, Guestfs, PartitionTableType};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Fluent GuestKit API Examples ===\n");

    // Example 1: Builder Pattern for Guest Creation
    example_builder_pattern()?;

    // Example 2: Type-Safe Filesystem Operations
    example_typed_filesystem()?;

    // Example 3: Fluent Mount API
    example_fluent_mount()?;

    Ok(())
}

/// Example 1: Using the builder pattern instead of manual configuration
fn example_builder_pattern() -> Result<(), Box<dyn Error>> {
    println!("Example 1: Builder Pattern");
    println!("==========================\n");

    // OLD WAY (still works):
    println!("OLD API:");
    println!("```rust");
    println!("let mut g = Guestfs::new()?;");
    println!("g.add_drive(\"/tmp/disk1.img\")?;");
    println!("g.add_drive_ro(\"/tmp/disk2.img\")?;");
    println!("g.set_verbose(true)?;");
    println!("g.launch()?;");
    println!("```\n");

    // NEW WAY (ergonomic, fluent):
    println!("NEW API (Fluent):");
    println!("```rust");
    println!("let mut guest = Guestfs::builder()");
    println!("    .add_drive(\"/tmp/disk1.img\")");
    println!("    .add_drive_ro(\"/tmp/disk2.img\")");
    println!("    .verbose(true)");
    println!("    .autosync(true)");
    println!("    .build_and_launch()?;");
    println!("```\n");

    // Actually demonstrate it
    let _guest = Guestfs::builder()
        .verbose(false)
        .autosync(true)
        .identifier("example-guest")
        .build()?;

    println!("✓ Guest created with builder pattern\n");

    Ok(())
}

/// Example 2: Type-safe filesystem operations using enums
fn example_typed_filesystem() -> Result<(), Box<dyn Error>> {
    println!("Example 2: Type-Safe Filesystem Operations");
    println!("===========================================\n");

    // OLD WAY:
    println!("OLD API (string-based):");
    println!("```rust");
    println!("g.mkfs(\"ext4\", \"/dev/sda1\", Some(4096), Some(\"rootfs\"), None, None)?;");
    println!("// Easy to make typos: \"ext4\" vs \"ext3\" vs \"etx4\"");
    println!("```\n");

    // NEW WAY:
    println!("NEW API (type-safe with enums):");
    println!("```rust");
    println!("use guestkit::guestfs::FilesystemType;");
    println!();
    println!("g.mkfs(\"/dev/sda1\")");
    println!("    .ext4()              // Type-safe, no typos possible!");
    println!("    .label(\"rootfs\")");
    println!("    .blocksize(4096)");
    println!("    .create()?;");
    println!("```\n");

    // Show all available filesystem types
    println!("Available filesystem types (compile-time checked):");
    println!("  - FilesystemType::Ext2");
    println!("  - FilesystemType::Ext3");
    println!("  - FilesystemType::Ext4");
    println!("  - FilesystemType::Xfs");
    println!("  - FilesystemType::Btrfs");
    println!("  - FilesystemType::Vfat");
    println!("  - FilesystemType::Ntfs");
    println!("  - FilesystemType::F2fs");
    println!("  - ... and more!\n");

    // Demonstrate filesystem type features
    let fs = FilesystemType::Ext4;
    println!("Filesystem: {}", fs);
    println!("  Supports labels: {}", fs.supports_labels());
    println!("  Supports UUID: {}", fs.supports_uuid());
    println!();

    Ok(())
}

/// Example 3: Fluent mount API with BTRFS subvolumes
fn example_fluent_mount() -> Result<(), Box<dyn Error>> {
    println!("Example 3: Fluent Mount API");
    println!("============================\n");

    // OLD WAY:
    println!("OLD API:");
    println!("```rust");
    println!("g.mount(\"/dev/sda1\", \"/\", Some(\"subvol=@,compress=zstd\"))?;");
    println!("// Options are just strings - easy to make mistakes");
    println!("```\n");

    // NEW WAY:
    println!("NEW API (fluent, self-documenting):");
    println!("```rust");
    println!("g.mount_with(\"/dev/sda1\", \"/\")");
    println!("    .subvolume(\"@\")           // Clear intent");
    println!("    .compress(\"zstd\")          // Self-documenting");
    println!("    .readonly()                // Type-safe option");
    println!("    .perform()?;");
    println!("```\n");

    println!("Benefits:");
    println!("  ✓ IDE autocompletion");
    println!("  ✓ Compile-time checking");
    println!("  ✓ Self-documenting code");
    println!("  ✓ Harder to make mistakes\n");

    Ok(())
}

/// Example 4: Full workflow with ergonomic API
#[allow(dead_code)]
fn full_ergonomic_workflow() -> Result<(), Box<dyn Error>> {
    println!("Full Fluent Workflow Example");
    println!("=============================\n");

    // Create and configure guest
    let mut guest = Guestfs::builder()
        .add_drive("/tmp/ergonomic-example.img")
        .verbose(true)
        .build()?;

    // Launch
    guest.launch()?;

    // Initialize partition table
    guest.part_init("/dev/sda", PartitionTableType::Gpt.as_str())?;

    // Add EFI partition
    guest.part_add("/dev/sda", "p", 2048, 411647)?; // 200MB

    // Add root partition
    guest.part_add("/dev/sda", "p", 411648, -34)?;

    // Create VFAT filesystem on EFI partition
    guest.mkfs("vfat", "/dev/sda1")?;
    guest.set_label("/dev/sda1", "EFI")?;

    // Create BTRFS filesystem on root
    guest.mkfs("btrfs", "/dev/sda2")?;
    guest.set_label("/dev/sda2", "rootfs")?;

    // Mount root partition
    guest.mount("/dev/sda2", "/")?;

    // Mount EFI partition
    guest.mkdir("/boot/efi")?;
    guest.mount("/dev/sda1", "/boot/efi")?;

    // Create directories
    guest.mkdir_p("/etc")?;
    guest.mkdir_p("/home")?;

    // Write configuration files
    guest.write("/etc/hostname", b"ergonomic-guest\n")?;
    guest.write(
        "/etc/fstab",
        b"/dev/sda2 / btrfs subvol=@,compress=zstd 0 0\n",
    )?;

    // Sync and cleanup
    guest.sync()?;
    guest.umount_all()?;

    println!("✓ Fluent workflow completed successfully!");

    Ok(())
}
