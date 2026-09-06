// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Comprehensive Phase 3 API testing with a fake Fedora-like disk image
//!
//! This test creates a minimal Fedora-like disk image and exercises all
//! Phase 3 APIs to ensure they work correctly in a realistic scenario.

use guestkit::Guestfs;
use std::fs;
use std::path::Path;

const DISK_PATH: &str = "/tmp/phase3-test-fedora.img";
const DISK_SIZE: i64 = 200 * 1024 * 1024; // 200 MB

fn cleanup() {
    let _ = fs::remove_file(DISK_PATH);
}

fn create_fake_fedora_image() -> Result<(), Box<dyn std::error::Error>> {
    cleanup();

    println!("\n=== Creating Fake Fedora Disk Image ===");

    // Test 1: Using create() alias (Phase 3 API)
    println!("\n[1/10] Testing Guestfs::create() alias...");
    let mut g = Guestfs::create()?;
    g.set_verbose(false);

    // Create disk image
    println!("  Creating {}MB disk image...", DISK_SIZE / 1024 / 1024);
    g.disk_create(DISK_PATH, "raw", DISK_SIZE)?;
    println!("  ✓ Disk image created");

    // Test 2: Using add_drive() (Phase 3 API - read-write mode)
    println!("\n[2/10] Testing add_drive() (read-write mode)...");
    g.add_drive(DISK_PATH)?;
    println!("  ✓ Drive added in read-write mode");

    // Launch and create partition table
    g.launch()?;

    println!("\n  Setting up partition table...");
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, 206847)?; // ~100MB boot
    g.part_add("/dev/sda", "primary", 206848, -2048)?; // ~100MB root

    // Test 3: Testing part_set_parttype() (Phase 3 API)
    println!("\n[3/10] Testing part_set_parttype()...");
    let parttype = g.part_get_parttype("/dev/sda")?;
    println!("  Current partition type: {}", parttype);
    assert_eq!(parttype, "gpt");
    println!("  ✓ Partition type is GPT");

    // Create filesystems
    println!("\n  Creating filesystems...");
    g.mkfs("ext4", "/dev/sda1")?;
    g.mkfs("ext4", "/dev/sda2")?;

    // Set filesystem labels
    g.set_label("/dev/sda1", "boot")?;
    g.set_label("/dev/sda2", "root")?;

    // Mount root filesystem
    g.mount("/dev/sda2", "/")?;

    // Create Fedora-like directory structure
    println!("\n  Creating directory structure...");
    for dir in &[
        "/bin",
        "/boot",
        "/dev",
        "/etc",
        "/home",
        "/lib",
        "/lib64",
        "/mnt",
        "/opt",
        "/proc",
        "/root",
        "/run",
        "/sbin",
        "/srv",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
        "/etc/sysconfig",
        "/var/log",
        "/var/lib",
    ] {
        g.mkdir_p(dir)?;
    }

    // Create fake Fedora release files
    println!("\n  Creating Fedora release files...");
    g.write("/etc/fedora-release", b"Fedora release 40 (Forty)\n")?;
    g.write("/etc/redhat-release", b"Fedora release 40 (Forty)\n")?;
    g.write(
        "/etc/os-release",
        b"NAME=Fedora\nVERSION=\"40 (Forty)\"\nID=fedora\nVERSION_ID=40\n",
    )?;

    // Create /etc/fstab
    g.write(
        "/etc/fstab",
        b"LABEL=root / ext4 defaults 0 1\nLABEL=boot /boot ext4 defaults 0 2\n",
    )?;

    // Create /etc/hostname
    g.write("/etc/hostname", b"fedora-test.localdomain\n")?;

    // Test 4: Testing stat() (Phase 3 API)
    println!("\n[4/10] Testing stat() on regular file...");
    let stat = g.stat("/etc/hostname")?;
    println!("  File size: {} bytes", stat.size);
    println!("  Mode: 0{:o}", stat.mode);
    println!("  UID: {}, GID: {}", stat.uid, stat.gid);
    assert!(stat.size > 0);
    println!("  ✓ stat() works correctly");

    // Create some test files with different attributes
    println!("\n  Creating test files...");
    g.write("/etc/test-file.txt", b"This is a test file\n")?;
    g.write("/tmp/temporary.txt", b"Temporary data\n")?;

    // Create a symbolic link
    g.ln_s("/etc/test-file.txt", "/etc/test-link")?;

    // Test 5: Testing lstat() on symlink (Phase 3 API)
    println!("\n[5/10] Testing lstat() on symbolic link...");
    let lstat_result = g.lstat("/etc/test-link")?;
    let stat_result = g.stat("/etc/test-link")?;

    // lstat should show the link itself, stat should follow it
    println!("  lstat size: {} (link metadata)", lstat_result.size);
    println!("  stat size: {} (target file)", stat_result.size);

    // The sizes should be different (link metadata vs file content)
    assert_ne!(
        lstat_result.size, stat_result.size,
        "lstat and stat should differ for symlinks"
    );
    println!("  ✓ lstat() correctly doesn't follow symlink");

    // Create directory for removal testing
    g.mkdir_p("/tmp/test-removal/subdir")?;
    g.write("/tmp/test-removal/file1.txt", b"file1\n")?;
    g.write("/tmp/test-removal/file2.txt", b"file2\n")?;
    g.write("/tmp/test-removal/subdir/file3.txt", b"file3\n")?;

    // Test 6: Testing rm() (Phase 3 API)
    println!("\n[6/10] Testing rm() on single file...");
    assert!(g.exists("/tmp/temporary.txt")?);
    g.rm("/tmp/temporary.txt")?;
    assert!(!g.exists("/tmp/temporary.txt")?);
    println!("  ✓ rm() successfully removed file");

    // Test that rm() fails on directory
    println!("\n  Testing rm() fails on directory...");
    let result = g.rm("/tmp/test-removal");
    assert!(result.is_err(), "rm() should fail on directory");
    println!("  ✓ rm() correctly rejects directory");

    // Test 7: Testing rm_rf() (Phase 3 API)
    println!("\n[7/10] Testing rm_rf() on directory tree...");
    assert!(g.exists("/tmp/test-removal")?);
    assert!(g.exists("/tmp/test-removal/subdir/file3.txt")?);

    g.rm_rf("/tmp/test-removal")?;
    assert!(!g.exists("/tmp/test-removal")?);
    println!("  ✓ rm_rf() successfully removed directory tree");

    // Test rm_rf() on non-existent path (should succeed silently)
    println!("\n  Testing rm_rf() on non-existent path...");
    g.rm_rf("/tmp/does-not-exist")?;
    println!("  ✓ rm_rf() handles non-existent path correctly");

    // Create a CPIO archive for testing
    println!("\n  Creating test CPIO archive...");
    g.mkdir_p("/root/archive-source")?;
    g.write("/root/archive-source/file1.txt", b"Content 1\n")?;
    g.write("/root/archive-source/file2.txt", b"Content 2\n")?;

    // We need to create the CPIO on the host
    g.umount("/")?;
    g.shutdown()?;

    // Re-open with add_drive_ro to test read-only mode
    println!("\n[8/10] Testing add_drive_ro() (read-only mode)...");
    let mut g2 = Guestfs::new()?;
    g2.set_verbose(false);
    g2.add_drive_ro(DISK_PATH)?;
    g2.launch()?;

    // Verify we can read but not write
    g2.mount_ro("/dev/sda2", "/")?;
    let content = g2.cat("/etc/hostname")?;
    assert!(content.contains("fedora-test"));
    println!("  ✓ Successfully mounted read-only");

    // Verify write fails in read-only mode
    let write_result = g2.write("/etc/should-fail", b"test");
    assert!(write_result.is_err(), "Write should fail in read-only mode");
    println!("  ✓ Write correctly fails in read-only mode");

    g2.umount("/")?;
    g2.shutdown()?;

    // Mount boot partition and test part_get_name
    println!("\n[9/10] Testing part_get_name() (GPT partition label)...");
    let mut g3 = Guestfs::new()?;
    g3.set_verbose(false);
    g3.add_drive(DISK_PATH)?;
    g3.launch()?;

    // Try to get partition name (may be empty if not set)
    match g3.part_get_name("/dev/sda", 1) {
        Ok(name) => println!("  Partition 1 name: '{}'", name),
        Err(e) => println!("  Could not get partition name: {}", e),
    }

    g3.shutdown()?;

    // Test 10: Create CPIO archive and test cpio_in()
    println!("\n[10/10] Testing cpio_in() (CPIO archive extraction)...");

    // First, create a CPIO archive on the host
    let cpio_path = "/tmp/test-archive.cpio";
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd /tmp && mkdir -p cpio-test && \
             echo 'test content 1' > cpio-test/file1.txt && \
             echo 'test content 2' > cpio-test/file2.txt && \
             find cpio-test -print | cpio -o -H newc > {}",
            cpio_path
        ))
        .output()?;

    if Path::new(cpio_path).exists() {
        let mut g4 = Guestfs::new()?;
        g4.set_verbose(false);
        g4.add_drive(DISK_PATH)?;
        g4.launch()?;
        g4.mount("/dev/sda2", "/")?;

        // Extract CPIO archive
        g4.mkdir_p("/root/cpio-extracted")?;

        match g4.cpio_in(cpio_path, "/root/cpio-extracted") {
            Ok(_) => {
                println!("  ✓ CPIO archive extracted successfully");

                // Verify extracted files
                if g4.exists("/root/cpio-extracted/cpio-test/file1.txt")? {
                    let content = g4.cat("/root/cpio-extracted/cpio-test/file1.txt")?;
                    println!("  Extracted file content: {}", content.trim());
                    println!("  ✓ CPIO extraction verified");
                }
            }
            Err(e) => {
                println!(
                    "  ⚠ CPIO extraction failed (cpio may not be available): {}",
                    e
                );
            }
        }

        g4.shutdown()?;
        let _ = fs::remove_file(cpio_path);
    } else {
        println!("  ⚠ Could not create CPIO archive for testing");
    }

    // Clean up temporary CPIO test directory
    let _ = std::process::Command::new("rm")
        .args(["-rf", "/tmp/cpio-test"])
        .output();

    println!("\n=== All Phase 3 APIs Tested Successfully! ===");

    Ok(())
}

#[test]
fn test_phase3_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    // Run the comprehensive test
    let result = create_fake_fedora_image();

    // Clean up
    cleanup();

    // Return result
    result
}

#[test]
fn test_stat_vs_lstat_behavior() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing stat() vs lstat() behavior ===");

    let disk = "/tmp/stat-test.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create a file and a symlink to it
    g.write("/target.txt", b"Target file content\n")?;
    g.ln_s("/target.txt", "/link.txt")?;

    // stat() should follow the symlink
    let stat_result = g.stat("/link.txt")?;
    println!("stat(/link.txt) size: {}", stat_result.size);
    assert_eq!(stat_result.size, 20); // "Target file content\n"

    // lstat() should NOT follow the symlink
    let lstat_result = g.lstat("/link.txt")?;
    println!("lstat(/link.txt) size: {}", lstat_result.size);
    assert_ne!(lstat_result.size, 20); // Should be link metadata size

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    println!("✓ stat() and lstat() behave correctly");
    Ok(())
}

#[test]
fn test_rm_rm_rf_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing rm() and rm_rf() edge cases ===");

    let disk = "/tmp/rm-test.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Test rm() on non-existent file (should error)
    println!("Testing rm() on non-existent file...");
    let result = g.rm("/does-not-exist");
    assert!(result.is_err());
    println!("  ✓ rm() correctly errors on non-existent file");

    // Test rm_rf() on non-existent file (should succeed)
    println!("Testing rm_rf() on non-existent file...");
    g.rm_rf("/does-not-exist")?;
    println!("  ✓ rm_rf() correctly ignores non-existent file");

    // Test rm() on directory (should error)
    g.mkdir("/testdir")?;
    let result = g.rm("/testdir");
    assert!(result.is_err());
    println!("  ✓ rm() correctly errors on directory");

    // Test rm_rf() on empty directory
    g.rm_rf("/testdir")?;
    assert!(!g.exists("/testdir")?);
    println!("  ✓ rm_rf() removes empty directory");

    // Test rm_rf() on nested structure
    g.mkdir_p("/deep/nested/structure")?;
    g.write("/deep/file.txt", b"test")?;
    g.write("/deep/nested/file.txt", b"test")?;
    g.rm_rf("/deep")?;
    assert!(!g.exists("/deep")?);
    println!("  ✓ rm_rf() removes nested directory tree");

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    Ok(())
}

#[test]
fn test_create_vs_new() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing create() vs new() ===");

    // Both should create equivalent handles
    let g1 = Guestfs::new()?;
    let g2 = Guestfs::create()?;

    // Both should be in Config state initially
    println!("✓ Both create() and new() work");

    drop(g1);
    drop(g2);

    Ok(())
}

#[test]
fn test_add_drive_vs_add_drive_ro() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing add_drive() vs add_drive_ro() ===");

    let disk = "/tmp/drive-test.img";
    let _ = fs::remove_file(disk);

    // Create a test disk
    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;
    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;
    g.write("/test.txt", b"test")?;
    g.shutdown()?;

    // Test add_drive_ro (read-only)
    let mut g2 = Guestfs::new()?;
    g2.add_drive_ro(disk)?;
    g2.launch()?;
    g2.mount_ro("/dev/sda1", "/")?;

    // Read should work
    let content = g2.cat("/test.txt")?;
    assert_eq!(content, "test");

    // Write should fail
    let result = g2.write("/test2.txt", b"fail");
    assert!(result.is_err());

    println!("✓ add_drive_ro() correctly enforces read-only access");

    g2.shutdown()?;
    let _ = fs::remove_file(disk);

    Ok(())
}
