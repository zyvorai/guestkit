// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: VM cloning and preparation
//!
//! This example demonstrates how to prepare a VM for cloning:
//! - Remove unique identifiers (machine-id, SSH keys)
//! - Clean up temporary files and logs
//! - Generalize network configuration
//! - Prepare for sysprep

use guestkit::Guestfs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VM Clone Preparation Example ===\n");

    let source_disk = std::env::args()
        .nth(1)
        .expect("Usage: vm_clone_prep <source.img> <dest.img>");

    let dest_disk = std::env::args()
        .nth(2)
        .expect("Usage: vm_clone_prep <source.img> <dest.img>");

    if !Path::new(&source_disk).exists() {
        eprintln!("Error: Source disk not found: {}", source_disk);
        std::process::exit(1);
    }

    println!("Source: {}", source_disk);
    println!("Destination: {}", dest_disk);
    println!();

    // Step 1: Copy the disk image
    println!("[1/5] Copying disk image...");
    std::fs::copy(&source_disk, &dest_disk)?;
    println!("✓ Disk copied");

    // Step 2: Open and prepare
    let mut g = Guestfs::new()?;
    g.set_verbose(false);

    println!("\n[2/5] Opening disk and detecting OS...");
    g.add_drive_ro(&dest_disk)?;
    g.launch()?;

    let roots = g.inspect_os()?;
    if roots.is_empty() {
        eprintln!("Error: No operating system found");
        std::process::exit(1);
    }

    let root = &roots[0];
    let ostype = g.inspect_get_type(root)?;
    let distro = g.inspect_get_distro(root)?;

    println!("  OS Type: {}", ostype);
    println!("  Distribution: {}", distro);

    // Mount filesystems
    let mountpoints = g.inspect_get_mountpoints(root)?;
    for (mp, device) in mountpoints {
        g.mount(&device, &mp)?;
    }

    // Step 3: Remove unique identifiers
    println!("\n[3/5] Removing unique identifiers...");
    remove_unique_ids(&mut g, &ostype)?;

    // Step 4: Clean up temporary files
    println!("\n[4/5] Cleaning up temporary files...");
    cleanup_temp_files(&mut g)?;

    // Step 5: Generalize configuration
    println!("\n[5/5] Generalizing configuration...");
    generalize_config(&mut g, &ostype)?;

    g.umount_all()?;
    g.shutdown()?;

    println!("\n{}", "=".repeat(70));
    println!("✓ VM preparation complete!");
    println!("{}", "=".repeat(70));
    println!("\nThe cloned disk is ready at: {}", dest_disk);
    println!("\nNext steps:");
    println!("  1. Boot the cloned VM");
    println!("  2. Set new hostname");
    println!("  3. Configure network settings");
    println!("  4. Regenerate SSH host keys (will happen automatically on first boot)");
    println!();

    Ok(())
}

fn remove_unique_ids(g: &mut Guestfs, ostype: &str) -> Result<(), Box<dyn std::error::Error>> {
    match ostype {
        "linux" => {
            // Remove machine-id
            if g.exists("/etc/machine-id")? {
                println!("  Removing /etc/machine-id");
                g.rm("/etc/machine-id")?;
                // Create empty file so systemd regenerates it
                g.touch("/etc/machine-id")?;
            }

            if g.exists("/var/lib/dbus/machine-id")? {
                println!("  Removing /var/lib/dbus/machine-id");
                g.rm("/var/lib/dbus/machine-id")?;
            }

            // Remove SSH host keys (will be regenerated on first boot)
            println!("  Removing SSH host keys");
            let ssh_key_patterns = vec!["/etc/ssh/ssh_host_*_key", "/etc/ssh/ssh_host_*_key.pub"];

            for pattern in ssh_key_patterns {
                if let Ok(keys) = g.glob_expand(pattern) {
                    for key in keys {
                        if g.exists(&key)? {
                            println!("    Removing {}", key);
                            g.rm(&key)?;
                        }
                    }
                }
            }

            // Remove unique network identifiers
            let udev_rules = "/etc/udev/rules.d/70-persistent-net.rules";
            if g.exists(udev_rules)? {
                println!("  Removing persistent network rules");
                g.rm(udev_rules)?;
            }

            // Clear DHCP leases
            let dhcp_lease_files = vec![
                "/var/lib/dhcp/dhclient.leases",
                "/var/lib/dhclient/*",
                "/var/lib/NetworkManager/*.lease",
            ];

            for pattern in dhcp_lease_files {
                if let Ok(files) = g.glob_expand(pattern) {
                    for file in files {
                        if g.is_file(&file).unwrap_or(false) {
                            println!("    Removing DHCP lease: {}", file);
                            g.rm(&file)?;
                        }
                    }
                }
            }
        }
        "windows" => {
            println!("  Windows preparation not fully implemented");
            // Windows would need different sysprep process
        }
        _ => {
            println!("  Unknown OS type: {}", ostype);
        }
    }

    Ok(())
}

fn cleanup_temp_files(g: &mut Guestfs) -> Result<(), Box<dyn std::error::Error>> {
    let temp_locations = vec![
        "/tmp/*",
        "/var/tmp/*",
        "/var/cache/yum/*",
        "/var/cache/dnf/*",
        "/var/cache/apt/*",
    ];

    for pattern in temp_locations {
        if let Ok(files) = g.glob_expand(pattern) {
            for file in files {
                // Skip directories, only remove files
                if g.is_file(&file).unwrap_or(false) {
                    println!("  Removing {}", file);
                    g.rm(&file)?;
                }
            }
        }
    }

    // Clean log files (truncate rather than delete)
    let log_files = vec![
        "/var/log/messages",
        "/var/log/syslog",
        "/var/log/boot.log",
        "/var/log/wtmp",
        "/var/log/btmp",
        "/var/log/lastlog",
    ];

    for log in log_files {
        if g.exists(log)? && g.is_file(log)? {
            println!("  Truncating {}", log);
            g.truncate(log)?;
        }
    }

    // Remove bash history
    if let Ok(homes) = g.glob_expand("/home/*") {
        for home in homes {
            let bash_history = format!("{}/.bash_history", home);
            if g.exists(&bash_history)? {
                println!("  Removing {}", bash_history);
                g.rm(&bash_history)?;
            }

            let zsh_history = format!("{}/.zsh_history", home);
            if g.exists(&zsh_history)? {
                println!("  Removing {}", zsh_history);
                g.rm(&zsh_history)?;
            }
        }
    }

    if g.exists("/root/.bash_history")? {
        println!("  Removing /root/.bash_history");
        g.rm("/root/.bash_history")?;
    }

    Ok(())
}

fn generalize_config(g: &mut Guestfs, ostype: &str) -> Result<(), Box<dyn std::error::Error>> {
    if ostype == "linux" {
        // Comment out specific hostname configuration
        if g.exists("/etc/hostname")? {
            println!("  Clearing hostname");
            g.write("/etc/hostname", b"localhost.localdomain\n")?;
        }

        // Remove specific network configurations
        let network_configs = vec![
            "/etc/sysconfig/network-scripts/ifcfg-*",
            "/etc/netplan/*.yaml",
        ];

        for pattern in network_configs {
            if let Ok(files) = g.glob_expand(pattern) {
                for file in files {
                    // Don't remove ifcfg-lo
                    if !file.contains("ifcfg-lo") {
                        if let Ok(content) = g.cat(&file) {
                            // Remove hardware-specific lines
                            let mut new_content = String::new();
                            for line in content.lines() {
                                if !line.contains("HWADDR")
                                    && !line.contains("UUID")
                                    && !line.contains("IPADDR")
                                {
                                    new_content.push_str(line);
                                    new_content.push('\n');
                                }
                            }
                            println!("  Generalizing {}", file);
                            g.write(&file, new_content.as_bytes())?;
                        }
                    }
                }
            }
        }

        // Disable cloud-init if present (to prevent it from applying old config)
        if g.is_dir("/etc/cloud").unwrap_or(false) {
            println!("  Disabling cloud-init");
            if g.exists("/etc/cloud/cloud.cfg.d/99-disable-network-config.cfg")? {
                g.write(
                    "/etc/cloud/cloud.cfg.d/99-manual-cache.cfg",
                    b"manual_cache_clean: True\n",
                )?;
            }
        }
    }

    Ok(())
}
