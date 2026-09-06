// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Execute commands and work with archives
//!
//! This demonstrates command execution and archive operations.
//!
//! ⚠️  REQUIRES: sudo/root permissions for mounting and chroot
//!
//! Usage:
//!   sudo cargo run --example command_and_archive /path/to/disk.qcow2

use guestkit::guestfs::Guestfs;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image>", args[0]);
        eprintln!("Example: sudo {} /path/to/vm.qcow2", args[0]);
        eprintln!("\n⚠️  Requires sudo/root for mounting and chroot");
        std::process::exit(1);
    }

    let disk_path = &args[1];

    println!("=== GuestKit Command & Archive Demo ===");
    println!("Image: {}\n", disk_path);

    // Create GuestFS handle
    let mut g = Guestfs::new()?;
    g.set_verbose(true);

    // Add disk and launch
    println!("Adding disk and launching...");
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Find and mount OS root
    println!("\nFinding OS root...");
    let roots = g.inspect_os()?;

    if roots.is_empty() {
        eprintln!("Error: No OS detected");
        return Ok(());
    }

    let root = &roots[0];
    let mountpoints = g.inspect_get_mountpoints(root)?;

    // Mount root
    if let Some((_, device)) = mountpoints.iter().find(|(mp, _)| mp.as_str() == "/") {
        println!("Mounting {} as /...", device);
        g.mount_ro(device, "/")?;
    } else {
        eprintln!("Error: No root filesystem");
        return Ok(());
    }

    // Execute commands in guest
    println!("\n--- Command Execution ---");

    println!("\n1. Get kernel version:");
    match g.command(&["/bin/uname", "-r"]) {
        Ok(output) => println!("   {}", output.trim()),
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n2. Check disk usage:");
    match g.sh("df -h | head -5") {
        Ok(output) => {
            for line in output.lines() {
                println!("   {}", line);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n3. List processes (from ps snapshot):");
    match g.sh("ps aux | head -10") {
        Ok(output) => {
            for line in output.lines() {
                println!("   {}", line);
            }
        }
        Err(e) => println!("   Error: {} (normal for offline VM)", e),
    }

    println!("\n4. Get environment variables:");
    match g.command_lines(&["/usr/bin/env"]) {
        Ok(lines) => {
            println!("   Found {} environment variables", lines.len());
            for line in lines.iter().take(5) {
                println!("   {}", line);
            }
            if lines.len() > 5 {
                println!("   ... and {} more", lines.len() - 5);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Archive operations
    println!("\n--- Archive Operations ---");

    // Create a temporary directory for demo
    let temp_dir = std::env::temp_dir().join("guestkit_demo");
    std::fs::create_dir_all(&temp_dir)?;

    let archive_path = temp_dir.join("etc_backup.tar");
    let compressed_path = temp_dir.join("etc_backup.tar.gz");

    println!("\n1. Creating tar archive from /etc...");
    match g.tar_out("/etc", &archive_path) {
        Ok(_) => {
            if archive_path.exists() {
                let size = std::fs::metadata(&archive_path)?.len();
                println!(
                    "   ✓ Created: {} ({:.2} MB)",
                    archive_path.display(),
                    size as f64 / 1e6
                );
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!("\n2. Creating compressed tar archive...");
    match g.tgz_out("/etc", &compressed_path) {
        Ok(_) => {
            if compressed_path.exists() {
                let size = std::fs::metadata(&compressed_path)?.len();
                println!(
                    "   ✓ Created: {} ({:.2} MB)",
                    compressed_path.display(),
                    size as f64 / 1e6
                );

                // Compare sizes
                if archive_path.exists() {
                    let orig_size = std::fs::metadata(&archive_path)?.len();
                    let ratio = (size as f64 / orig_size as f64) * 100.0;
                    println!("   Compression ratio: {:.1}%", ratio);
                }
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Note: tar_in and tgz_in would require write access
    println!("\n3. Archive extraction (tar_in, tgz_in):");
    println!("   Available but requires read-write mode");
    println!("   Use add_drive_opts() with readonly=false to enable");

    // File operations demo
    println!("\n--- File Operations ---");

    println!("\n1. Create directory (requires write mode):");
    match g.mkdir_p("/tmp/guestkit_test") {
        Ok(_) => println!("   ✓ Created /tmp/guestkit_test"),
        Err(e) => println!("   ✗ Error: {} (expected in read-only mode)", e),
    }

    println!("\n2. Write file (requires write mode):");
    match g.write("/tmp/test.txt", b"Hello from guestkit!") {
        Ok(_) => println!("   ✓ Wrote to /tmp/test.txt"),
        Err(e) => println!("   ✗ Error: {} (expected in read-only mode)", e),
    }

    // Cleanup
    println!("\n--- Cleaning Up ---");

    // Remove temp files
    let _ = std::fs::remove_file(&archive_path);
    let _ = std::fs::remove_file(&compressed_path);
    let _ = std::fs::remove_dir(&temp_dir);

    g.umount_all()?;
    g.shutdown()?;

    println!("\n✓ Complete!");
    println!("\nNote: Many operations require read-write mode.");
    println!("Use add_drive_opts(path, false, None) instead of add_drive_ro()");

    Ok(())
}
