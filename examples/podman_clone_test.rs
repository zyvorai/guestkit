// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

// Integration test: lvm_clone_podman with a real RHEL VM
use guestkit::guestfs::lvm_clone::{lvm_clone_podman, IsolationLevel, LvmCloneConfig};

fn main() {
    let config = LvmCloneConfig {
        source_vg: "rhel".to_string(),
        source_lv: "root".to_string(),
        clone_lv_name: "root-podman-test".to_string(),
        target_vg: None,
        regenerate_uuids: true,
        update_fstab: true,
        update_bootloader: true,
        update_crypttab: true,
        hostname: Some("rhel88-clone".to_string()),
        dry_run: false,
        snapshot_size: Some("2G".to_string()),
        regenerate_initramfs: false,
        isolation_level: IsolationLevel::None,
        verify_security: true,
        regenerate_grub: false,
        verify_boot: true,
        container_image: Some("fedora:latest".to_string()),
    };

    println!("Starting podman-based LVM clone...");
    println!("  Source: /dev/rhel/root");
    println!("  Clone:  /dev/rhel/root-podman-test");
    println!("  Image:  fedora:latest");
    println!();

    match lvm_clone_podman(&config, true) {
        Ok(result) => {
            println!();
            println!("=== Clone completed successfully ===");
            println!("Source: {}", result.source_path);
            println!("Clone:  {}", result.clone_path);
            println!("Timestamp: {}", result.timestamp);
            println!("fstab_updated: {}", result.fstab_updated);
            println!("bootloader_updated: {}", result.bootloader_updated);
            println!("crypttab_updated: {}", result.crypttab_updated);
            println!("namespace_isolated: {}", result.namespace_isolated);
            println!("boot_verified: {}", result.boot_verified);
            println!("UUID mappings: {}", result.uuid_mappings.len());
            for m in &result.uuid_mappings {
                println!(
                    "  {} ({}): {} -> {}",
                    m.device, m.fs_type, m.old_uuid, m.new_uuid
                );
            }
            if !result.security_warnings.is_empty() {
                println!("Security warnings:");
                for w in &result.security_warnings {
                    println!("  [{}] {}", w.category, w.message);
                }
            }
            println!("Backup files: {:?}", result.backup_files);
        }
        Err(e) => {
            eprintln!("Clone failed: {}", e);
            std::process::exit(1);
        }
    }
}
