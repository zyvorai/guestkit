// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Work with LVM and LUKS encrypted volumes
//!
//! This demonstrates LVM detection and LUKS encryption support.
//!
//! ⚠️  REQUIRES: sudo/root permissions, cryptsetup, lvm2 packages
//!
//! Usage:
//!   sudo cargo run --example lvm_luks_demo /path/to/disk.qcow2 [passphrase]

use guestkit::guestfs::Guestfs;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image> [luks-passphrase]", args[0]);
        eprintln!(
            "Example: sudo {} /path/to/encrypted-vm.qcow2 mypassword",
            args[0]
        );
        eprintln!("\n⚠️  Requires sudo/root, cryptsetup, and lvm2 packages");
        std::process::exit(1);
    }

    let disk_path = &args[1];
    let passphrase = args.get(2).map(|s| s.as_str());

    println!("=== GuestKit LVM & LUKS Demo ===");
    println!("Image: {}\n", disk_path);

    // Create GuestFS handle
    let mut g = Guestfs::new()?;
    g.set_verbose(true);

    // Add disk and launch
    println!("Adding disk and launching...");
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Check for LUKS encrypted partitions
    println!("\n--- LUKS Encryption Detection ---");
    let filesystems = g.list_filesystems()?;
    let mut luks_devices = Vec::new();

    for (device, fstype) in &filesystems {
        if fstype == "crypto_LUKS" {
            println!("Found LUKS device: {}", device);
            luks_devices.push(device.clone());

            // Get LUKS UUID
            if let Ok(uuid) = g.luks_uuid(device) {
                println!("  UUID: {}", uuid);
            }
        }
    }

    // Try to open LUKS device if passphrase provided
    if let (Some(device), Some(pass)) = (luks_devices.first(), passphrase) {
        println!("\nAttempting to open LUKS device...");
        match g.luks_open(device, pass, "cryptroot") {
            Ok(_) => {
                println!("✓ LUKS device opened as /dev/mapper/cryptroot");

                // Now we can work with the decrypted device
                println!("Decrypted device is now available for mounting");

                // Close it when done
                println!("Closing LUKS device...");
                g.luks_close("cryptroot")?;
                println!("✓ LUKS device closed");
            }
            Err(e) => {
                println!("✗ Failed to open LUKS device: {}", e);
                println!("  (This is normal if the passphrase is incorrect)");
            }
        }
    } else if !luks_devices.is_empty() {
        println!("\nTo open LUKS devices, provide passphrase as 2nd argument");
    } else {
        println!("No LUKS encrypted devices found");
    }

    // Scan for LVM
    println!("\n--- LVM Detection ---");
    println!("Scanning for volume groups...");

    match g.vgscan() {
        Ok(_) => {
            println!("✓ VG scan complete");

            // List physical volumes
            let pvs = g.pvs()?;
            if !pvs.is_empty() {
                println!("\nPhysical Volumes:");
                for pv in &pvs {
                    println!("  {}", pv);
                }
            } else {
                println!("\nNo LVM physical volumes found");
            }

            // List volume groups
            let vgs = g.vgs()?;
            if !vgs.is_empty() {
                println!("\nVolume Groups:");
                for vg in &vgs {
                    println!("  {}", vg);
                }

                // Activate volume groups
                println!("\nActivating volume groups...");
                match g.vg_activate_all(true) {
                    Ok(_) => {
                        println!("✓ Volume groups activated");

                        // List logical volumes
                        let lvs = g.lvs()?;
                        if !lvs.is_empty() {
                            println!("\nLogical Volumes:");
                            for lv in &lvs {
                                println!("  {}", lv);
                            }
                            println!("\n✓ These LVs are now available for mounting");
                        } else {
                            println!("\nNo logical volumes found");
                        }

                        // Deactivate when done
                        println!("\nDeactivating volume groups...");
                        g.vg_activate_all(false)?;
                        println!("✓ Volume groups deactivated");
                    }
                    Err(e) => {
                        println!("✗ Failed to activate VGs: {}", e);
                        println!("  (This is normal if no VGs exist)");
                    }
                }
            } else {
                println!("\nNo LVM volume groups found");
            }
        }
        Err(e) => {
            println!("✗ VG scan failed: {}", e);
            println!("  (This is normal if no LVM is present)");
        }
    }

    // Cleanup
    println!("\n--- Cleaning Up ---");
    g.shutdown()?;

    println!("\n✓ Complete!");
    println!("\nSummary:");
    println!("  LUKS devices found: {}", luks_devices.len());
    println!("  LVM support: Available");

    Ok(())
}
