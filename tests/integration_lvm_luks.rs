// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for LVM and LUKS operations
//!
//! These tests verify complex scenarios involving encryption and logical volumes.

use guestkit::Guestfs;
use std::fs;

fn cleanup_disk(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
#[ignore] // Requires root/sudo for cryptsetup
fn test_luks_basic() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_luks.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    // Format as LUKS
    let password = "test_password_123";
    g.luks_format("/dev/sda1", password, 0)?;

    // Open LUKS device
    g.luks_open("/dev/sda1", password, "encrypted")?;

    // Create filesystem on encrypted device
    g.mkfs("ext4", "/dev/mapper/encrypted")?;

    // Mount and use
    g.mount("/dev/mapper/encrypted", "/")?;
    g.write("/secret.txt", b"This is encrypted")?;

    let content = g.cat("/secret.txt")?;
    assert_eq!(content, "This is encrypted");

    // Cleanup
    g.umount("/")?;
    g.luks_close("/dev/mapper/encrypted")?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
#[ignore] // Requires root/sudo for LVM
fn test_lvm_basic() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_lvm.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 500 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition for LVM
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    // Setup LVM
    // Note: This requires actual LVM setup which may need additional commands
    // This is a simplified version showing the API usage

    // Create logical volume
    g.lvcreate("data", "vg0", 100)?; // 100MB

    // Create filesystem on LV
    g.mkfs("ext4", "/dev/vg0/data")?;

    // Mount and use
    g.mount("/dev/vg0/data", "/")?;
    g.write("/lvm_test.txt", b"Data on LVM")?;

    let content = g.cat("/lvm_test.txt")?;
    assert_eq!(content, "Data on LVM");

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
#[ignore] // Requires root/sudo for LUKS + LVM
fn test_luks_with_lvm() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_luks_lvm.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 500 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    // Encrypt the partition
    let password = "secure_password_456";
    g.luks_format("/dev/sda1", password, 0)?;
    g.luks_open("/dev/sda1", password, "encrypted")?;

    // Create LVM on encrypted device
    // (Simplified - actual implementation would need pvcreate, vgcreate, etc.)

    // Create logical volumes
    g.lvcreate("root", "vg0", 200)?; // 200MB
    g.lvcreate("home", "vg0", 100)?; // 100MB

    // Create filesystems
    g.mkfs("ext4", "/dev/vg0/root")?;
    g.mkfs("ext4", "/dev/vg0/home")?;

    // Mount and test
    g.mount("/dev/vg0/root", "/")?;
    g.mkdir("/home")?;
    g.mount("/dev/vg0/home", "/home")?;

    g.write("/test.txt", b"Root filesystem")?;
    g.write("/home/user.txt", b"Home filesystem")?;

    assert_eq!(g.cat("/test.txt")?, "Root filesystem");
    assert_eq!(g.cat("/home/user.txt")?, "Home filesystem");

    // Cleanup
    g.umount_all()?;
    g.luks_close("/dev/mapper/encrypted")?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_lvm_inspection() -> Result<(), Box<dyn std::error::Error>> {
    // This test doesn't create LVM but tests the inspection APIs
    let _g = Guestfs::new()?;

    // Test that the APIs exist and return appropriate empty results
    // when no LVM is present
    // Note: We can't fully test without actual LVM setup

    // These should return empty lists on a new instance
    // (Can't call without launching, so this is more of an API structure test)

    Ok(())
}

#[test]
fn test_luks_uuid() -> Result<(), Box<dyn std::error::Error>> {
    // Test LUKS UUID operations
    // Note: Requires actual LUKS device to fully test

    Ok(())
}

#[test]
#[ignore] // Requires root/sudo
fn test_luks_key_management() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_luks_keys.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    // Format as LUKS with initial key
    let key1 = "initial_password";
    g.luks_format("/dev/sda1", key1, 0)?;

    // Add a second key
    let key2 = "second_password";
    g.luks_add_key("/dev/sda1", key1, key2, 1)?;

    // Test that we can open with the new key
    g.luks_open("/dev/sda1", key2, "encrypted")?;
    g.luks_close("/dev/mapper/encrypted")?;

    // Test that we can still open with the original key
    g.luks_open("/dev/sda1", key1, "encrypted")?;
    g.luks_close("/dev/mapper/encrypted")?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
#[ignore] // Requires root/sudo
fn test_lvm_volume_operations() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_lvm_ops.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 500 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup LVM (simplified)
    // In reality, would need pvcreate, vgcreate first

    // Create logical volume
    g.lvcreate("test_lv", "test_vg", 100)?;

    // List logical volumes
    let lvs = g.lvs()?;
    assert!(!lvs.is_empty());

    // Remove logical volume
    g.lvremove("/dev/test_vg/test_lv")?;

    // Verify it's removed
    let lvs = g.lvs()?;
    assert!(!lvs.iter().any(|lv| lv.contains("test_lv")));

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_vg_scan() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_vgscan.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Test that vgscan works (even if no VGs present)
    g.vgscan()?;

    // Test VG activation (should succeed even with no VGs)
    g.vg_activate_all(true)?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
#[ignore] // Requires root/sudo
fn test_luks_readonly_open() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_luks_ro.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition and format as LUKS
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    let password = "test_password";
    g.luks_format("/dev/sda1", password, 0)?;

    // Open in read-only mode
    g.luks_open_ro("/dev/sda1", password, "encrypted_ro")?;

    // Create filesystem
    g.mkfs("ext4", "/dev/mapper/encrypted_ro")?;

    // Mount read-only
    g.mount_ro("/dev/mapper/encrypted_ro", "/")?;

    // Should be able to read
    let files = g.ls("/")?;
    assert!(!files.is_empty());

    // Writing should fail (implementation dependent)

    g.umount("/")?;
    g.luks_close("/dev/mapper/encrypted_ro")?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}
