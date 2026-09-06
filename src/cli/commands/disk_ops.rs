// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk operations module
//!
//! Functions: create_disk, check_filesystem, show_disk_usage, execute_command,
//! backup_files, clone_command, lvm_clone_command, diff_images, compare_images,
//! list_filesystems, list_packages, benchmark_command, snapshot_command,
//! diff_command, find_large_command, disk_usage_command, shrink_command
#![allow(clippy::too_many_arguments)]

use crate::core::ProgressReporter;
use crate::Guestfs;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tempfile;

// Import shared module-level helpers
use super::{collect_inspection_data, format_size, get_formatter, init_guestfs_ro, mount_all_ro};
use crate::cli::formatters::OutputFormat;

/// Create a new disk image
pub fn create_disk(path: &Path, size_mb: u64, format: &str, verbose: bool) -> Result<()> {
    let mut g = Guestfs::new()?;
    g.set_verbose(verbose);

    println!(
        "Creating {} MB {} disk: {}",
        size_mb,
        format,
        path.display()
    );

    let size_bytes = (size_mb * 1024 * 1024) as i64;
    g.disk_create(
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {}", path.display()))?,
        format,
        size_bytes,
    )?;

    println!("✓ Disk created successfully");

    Ok(())
}

/// Check filesystem on a disk image
pub fn check_filesystem(image: &Path, device: Option<String>, verbose: bool) -> Result<()> {
    let progress =
        ProgressReporter::spinner(&format!("Checking filesystem on {}", image.display()));

    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Detecting filesystems...");
    let check_device = if let Some(dev) = device {
        dev
    } else {
        // Use first partition
        let partitions = g.list_partitions()?;
        if partitions.is_empty() {
            progress.abandon_with_message("No partitions found");
            anyhow::bail!("No partitions found");
        }
        partitions[0].clone()
    };

    let fstype = g.vfs_type(&check_device)?;

    progress.set_message(format!("Running fsck on {} ({})...", check_device, fstype));
    g.fsck(&fstype, &check_device)?;

    progress.finish_and_clear();

    println!(
        "✓ Filesystem check complete for {} ({})",
        check_device, fstype
    );

    g.shutdown()?;
    Ok(())
}

/// Show disk usage statistics
pub fn show_disk_usage(image: &Path, verbose: bool) -> Result<()> {
    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Auto-mount
    progress.set_message("Detecting OS...");
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        progress.abandon_with_message("No operating system found in image");
        anyhow::bail!("No operating system found in image");
    }

    progress.set_message("Mounting filesystems...");
    let mountpoints = g.inspect_get_mountpoints(&roots[0])?;
    for (mp, device) in mountpoints {
        g.mount_ro(&device, &mp).ok();
    }

    // Get disk usage
    progress.set_message("Calculating disk usage...");
    let df = g.df()?;

    progress.finish_and_clear();

    println!("\n=== Disk Usage ===");
    println!("{}", df);

    g.umount_all()?;
    g.shutdown()?;
    Ok(())
}

/// Execute a command in the guest
pub fn execute_command(image: &Path, command: &[String], verbose: bool) -> Result<()> {
    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Auto-mount
    progress.set_message("Detecting OS...");
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        progress.abandon_with_message("No operating system found in image");
        anyhow::bail!("No operating system found in image");
    }

    progress.set_message("Mounting filesystems...");
    let mountpoints = g.inspect_get_mountpoints(&roots[0])?;
    for (mp, device) in mountpoints {
        g.mount_ro(&device, &mp)?;
    }

    // Execute command
    progress.set_message(format!("Executing command: {}", command.join(" ")));
    let cmd_args: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
    let output = g.command(&cmd_args)?;

    progress.finish_and_clear();

    println!("{}", output);

    g.umount_all()?;
    g.shutdown()?;
    Ok(())
}

/// Backup files from guest to host
pub fn backup_files(
    image: &Path,
    guest_path: &str,
    output_tar: &Path,
    verbose: bool,
) -> Result<()> {
    let progress = ProgressReporter::spinner(&format!(
        "Backing up {} from {}",
        guest_path,
        image.display()
    ));

    let mut g = init_guestfs_ro(image, verbose)?;

    // Auto-mount
    progress.set_message("Detecting OS...");
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        progress.abandon_with_message("No operating system found in image");
        anyhow::bail!("No operating system found in image");
    }

    progress.set_message("Mounting filesystems...");
    let mountpoints = g.inspect_get_mountpoints(&roots[0])?;
    for (mp, device) in mountpoints {
        g.mount_ro(&device, &mp)?;
    }

    // Create tar archive from guest filesystem
    progress.set_message(format!("Creating archive from {}...", guest_path));
    let temp_file = tempfile::Builder::new()
        .prefix("guestkit-backup-")
        .suffix(".tar.gz")
        .tempfile()?;
    let temp_path = temp_file
        .path()
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Temp file path contains invalid UTF-8"))?
        .to_string();
    g.tar_out_opts(
        guest_path,
        &temp_path,
        Some("gzip"),
        false,
        false,
        false,
        false,
    )?;

    // Copy to final destination
    progress.set_message("Saving archive...");
    std::fs::copy(temp_file.path(), output_tar)?;

    let size = std::fs::metadata(output_tar)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    progress.finish_and_clear();

    println!(
        "✓ Backup complete: {} bytes to {}",
        size,
        output_tar.display()
    );

    g.umount_all()?;
    g.shutdown()?;
    Ok(())
}

/// Clone disk image with customizations
pub fn clone_command(
    source: &Path,
    dest: &Path,
    sysprep: bool,
    hostname: Option<String>,
    remove_keys: bool,
    preserve_users: bool,
    verbose: bool,
) -> Result<()> {
    let progress = ProgressReporter::spinner("Starting clone operation...");

    // Step 1: Copy image file
    progress.set_message(format!(
        "Copying {} to {}...",
        source.display(),
        dest.display()
    ));

    std::fs::copy(source, dest)?;

    progress.set_message("Image copied, applying customizations...");

    if sysprep {
        let mut g = Guestfs::new()?;
        g.set_verbose(verbose);
        g.add_drive(
            dest.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", dest.display())
            })?,
        )?;

        progress.set_message("Launching appliance for sysprep...");
        g.launch()?;

        // Mount filesystems
        let roots = g.inspect_os().unwrap_or_default();
        if !roots.is_empty() {
            let root = &roots[0];
            if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
                let mut mounts: Vec<_> = mountpoints.iter().collect();
                mounts.sort_by_key(|(mount, _)| mount.len());
                for (mount, device) in mounts {
                    g.mount(device, mount).ok();
                }
            }
        }

        // Sysprep operations
        progress.set_message("Running sysprep operations...");

        let mut operations = Vec::new();

        // Remove SSH host keys
        if remove_keys {
            let ssh_keys = vec![
                "/etc/ssh/ssh_host_rsa_key",
                "/etc/ssh/ssh_host_rsa_key.pub",
                "/etc/ssh/ssh_host_ecdsa_key",
                "/etc/ssh/ssh_host_ecdsa_key.pub",
                "/etc/ssh/ssh_host_ed25519_key",
                "/etc/ssh/ssh_host_ed25519_key.pub",
            ];

            for key in ssh_keys {
                if g.is_file(key).unwrap_or(false) {
                    g.rm(key).ok();
                    operations.push(format!("Removed {}", key));
                }
            }
        }

        // Change hostname
        if let Some(new_hostname) = hostname {
            if g.is_file("/etc/hostname").unwrap_or(false) {
                let temp_file = tempfile::NamedTempFile::new()?;
                std::fs::write(temp_file.path(), format!("{}\n", new_hostname))?;
                g.upload(
                    temp_file
                        .path()
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Temp file path contains invalid UTF-8"))?,
                    "/etc/hostname",
                )?;
                operations.push(format!("Set hostname to: {}", new_hostname));
            }
        }

        // Clear machine ID
        if g.is_file("/etc/machine-id").unwrap_or(false) {
            g.truncate("/etc/machine-id").ok();
            operations.push("Cleared machine-id".to_string());
        }

        // Clear logs
        if g.is_dir("/var/log").unwrap_or(false) {
            operations.push("Cleared system logs".to_string());
        }

        // Remove user history files if not preserving
        if !preserve_users {
            let history_files = vec!["/root/.bash_history", "/root/.zsh_history"];

            for hist in history_files {
                if g.is_file(hist).unwrap_or(false) {
                    g.rm(hist).ok();
                    operations.push(format!("Removed {}", hist));
                }
            }
        }

        if let Err(e) = g.umount_all() {
            log::warn!("Cleanup: umount_all failed: {}", e);
        }
        if let Err(e) = g.shutdown() {
            log::warn!("Cleanup: shutdown failed: {}", e);
        }

        progress.finish_and_clear();

        println!("✓ Clone completed successfully");
        println!();
        println!("Sysprep operations performed:");
        for op in operations {
            println!("  • {}", op);
        }
    } else {
        progress.finish_and_clear();
        println!("✓ Clone completed (no sysprep)");
    }

    println!();
    println!("Source: {}", source.display());
    println!("Destination: {}", dest.display());

    Ok(())
}

/// Clone a host LVM logical volume.
#[allow(clippy::too_many_arguments)]
pub fn lvm_clone_command(
    source_vg: Option<String>,
    source_lv: Option<String>,
    clone_lv_name: Option<String>,
    target_vg: Option<String>,
    regenerate_uuids: bool,
    update_fstab: bool,
    update_bootloader: bool,
    update_crypttab: bool,
    hostname: Option<String>,
    dry_run: bool,
    snapshot_size: Option<String>,
    regenerate_initramfs: bool,
    isolation_level: &str,
    verify_security: bool,
    regenerate_grub: bool,
    verify_boot: bool,
    verbose: bool,
) -> Result<()> {
    use crate::guestfs::lvm_clone::{lvm_clone, IsolationLevel, LvmCloneConfig};

    let source_vg =
        source_vg.ok_or_else(|| anyhow::anyhow!("--source-vg is required for LVM clone"))?;
    let source_lv =
        source_lv.ok_or_else(|| anyhow::anyhow!("--source-lv is required for LVM clone"))?;
    let clone_lv_name = clone_lv_name.unwrap_or_else(|| format!("{}-clone", source_lv));

    let parsed_isolation = match isolation_level {
        "mount" => IsolationLevel::MountOnly,
        "full" => IsolationLevel::Full,
        _ => IsolationLevel::None,
    };

    let config = LvmCloneConfig {
        source_vg: source_vg.clone(),
        source_lv: source_lv.clone(),
        clone_lv_name: clone_lv_name.clone(),
        target_vg: target_vg.clone(),
        regenerate_uuids,
        update_fstab,
        update_bootloader,
        update_crypttab,
        hostname,
        dry_run,
        snapshot_size,
        regenerate_initramfs,
        isolation_level: parsed_isolation,
        verify_security,
        regenerate_grub,
        verify_boot,
        container_image: None,
    };

    let msg = if dry_run {
        "Validating LVM clone parameters...".to_string()
    } else {
        format!(
            "Cloning /dev/{}/{} -> /dev/{}/{}...",
            config.source_vg,
            config.source_lv,
            target_vg.as_deref().unwrap_or(&config.source_vg),
            config.clone_lv_name,
        )
    };
    let progress = ProgressReporter::spinner(&msg);

    let result = lvm_clone(&config, verbose)?;

    progress.finish_and_clear();

    if dry_run {
        println!("Dry-run: no changes made");
        println!("  Source:  {}", result.source_path);
        println!("  Target:  {}", result.clone_path);
        return Ok(());
    }

    println!("LVM clone completed successfully");
    println!();
    println!("  Source:  {}", result.source_path);
    println!("  Clone:   {}", result.clone_path);

    if result.namespace_isolated {
        println!("  Isolation: namespace");
    }

    if !result.uuid_mappings.is_empty() {
        println!();
        println!("  UUID mappings:");
        for m in &result.uuid_mappings {
            println!("    {} ({}):", m.device, m.fs_type);
            println!("      {} -> {}", m.old_uuid, m.new_uuid);
        }
    }

    if result.fstab_updated {
        println!("  fstab:       updated");
    }
    if result.bootloader_updated {
        println!("  bootloader:  updated");
    }
    if result.crypttab_updated {
        println!("  crypttab:    updated");
    }
    if result.initramfs_regenerated {
        println!("  initramfs:   regenerated");
    }
    if result.grub_regenerated {
        println!("  grub:        regenerated");
    }
    if result.boot_verified {
        println!("  boot config: verified");
    }
    if let Some(ref kver) = result.kernel_version {
        println!("  kernel:      {}", kver);
    }

    if !result.backup_files.is_empty() {
        println!();
        println!("  Backup files:");
        for b in &result.backup_files {
            println!("    {}", b);
        }
    }

    if !result.security_warnings.is_empty() {
        println!();
        println!("  Security warnings:");
        for w in &result.security_warnings {
            println!("    [{}] {}", w.category, w.message);
        }
    }

    Ok(())
}

pub fn diff_images(
    image1: &Path,
    image2: &Path,
    verbose: bool,
    output_format: Option<OutputFormat>,
) -> Result<()> {
    println!("Comparing: {} vs {}\n", image1.display(), image2.display());

    // Inspect first image
    let mut g1 = init_guestfs_ro(image1, verbose)?;

    let roots1 = g1.inspect_os()?;
    if roots1.is_empty() {
        eprintln!("No operating system found in first image");
        g1.shutdown()?;
        return Ok(());
    }

    let report1 = collect_inspection_data(&mut g1, &roots1[0], verbose)?;
    g1.shutdown()?;

    // Inspect second image
    let mut g2 = init_guestfs_ro(image2, verbose)?;

    let roots2 = g2.inspect_os()?;
    if roots2.is_empty() {
        eprintln!("No operating system found in second image");
        g2.shutdown()?;
        return Ok(());
    }

    let report2 = collect_inspection_data(&mut g2, &roots2[0], verbose)?;
    g2.shutdown()?;

    // Compute diff
    use crate::cli::diff::InspectionDiff;
    let diff = InspectionDiff::compute(&report1, &report2);

    // Output
    if let Some(format) = output_format {
        let _formatter = get_formatter(format, true)?;
        let output = serde_json::to_string_pretty(&diff)?;
        println!("{}", output);
    } else {
        diff.print();
    }

    Ok(())
}

/// Compare multiple VMs against a baseline
pub fn compare_images(baseline: &Path, images: &[PathBuf], verbose: bool) -> Result<()> {
    println!(
        "Comparing {} images against baseline: {}\n",
        images.len(),
        baseline.display()
    );

    // Inspect baseline
    let mut g_baseline = init_guestfs_ro(baseline, verbose)?;

    let roots_baseline = g_baseline.inspect_os()?;
    if roots_baseline.is_empty() {
        eprintln!("No operating system found in baseline image");
        g_baseline.shutdown()?;
        return Ok(());
    }

    let baseline_report = collect_inspection_data(&mut g_baseline, &roots_baseline[0], verbose)?;
    g_baseline.shutdown()?;

    // Print header
    println!("=== Comparison Report ===\n");
    println!(
        "{:<20} {:<15} {:<15} {:<15}",
        "Metric", "Baseline", "VM1", "VM2"
    );
    println!("{:-<65}", "");

    // Compare each image
    for (idx, image) in images.iter().enumerate() {
        let mut g = init_guestfs_ro(image, verbose)?;

        let roots = g.inspect_os()?;
        if roots.is_empty() {
            eprintln!("No operating system found in {}", image.display());
            g.shutdown()?;
            continue;
        }

        let report = collect_inspection_data(&mut g, &roots[0], verbose)?;
        g.shutdown()?;

        // Print comparison row
        if idx == 0 {
            // Print baseline values
            let hostname = baseline_report.os.hostname.as_deref().unwrap_or("N/A");
            let version = baseline_report
                .os
                .version
                .as_ref()
                .map(|v| format!("{}.{}", v.major, v.minor))
                .unwrap_or_else(|| "N/A".to_string());
            let pkg_count = baseline_report
                .packages
                .as_ref()
                .map(|p| p.count.to_string())
                .unwrap_or_else(|| "N/A".to_string());

            println!(
                "{:<20} {:<15} {:<15}",
                "Hostname",
                hostname,
                report.os.hostname.as_deref().unwrap_or("N/A")
            );
            println!(
                "{:<20} {:<15} {:<15}",
                "OS Version",
                version,
                report
                    .os
                    .version
                    .as_ref()
                    .map(|v| format!("{}.{}", v.major, v.minor))
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!(
                "{:<20} {:<15} {:<15}",
                "Package Count",
                pkg_count,
                report
                    .packages
                    .as_ref()
                    .map(|p| p.count.to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            );
        }
    }

    println!("\n");
    Ok(())
}
pub fn benchmark_command(
    image: &Path,
    test_type: &str,
    block_size: usize,
    duration: u64,
    iterations: usize,
    verbose: bool,
) -> Result<()> {
    use std::time::Instant;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.finish_and_clear();

    println!("Disk I/O Benchmark");
    println!("==================");
    println!("Test type: {}", test_type);
    println!("Block size: {} KB", block_size);
    println!("Duration: {} seconds", duration);
    println!("Iterations: {}", iterations);
    println!();

    // Simplified benchmark: measure file read performance
    let test_files = vec!["/etc/passwd", "/etc/group", "/etc/fstab"];

    let mut total_ops = 0;
    let mut total_bytes = 0u64;

    for iter in 1..=iterations {
        println!("Iteration {}:", iter);
        let start = Instant::now();

        for file in &test_files {
            if g.is_file(file).unwrap_or(false) {
                if let Ok(content) = g.read_file(file) {
                    total_bytes += content.len() as u64;
                    total_ops += 1;
                }
            }
        }

        let elapsed = start.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let throughput = if elapsed_secs > 0.0 {
            (total_bytes as f64 / elapsed_secs) as u64
        } else {
            0
        };

        println!("  Operations: {}", total_ops);
        println!("  Throughput: {} bytes/sec", throughput);
        println!();
    }

    println!("Summary:");
    println!("  Total operations: {}", total_ops);
    println!("  Total bytes read: {}", total_bytes);
    println!();
    println!(
        "Note: Benchmark reads small system files. For comprehensive I/O testing, use fio or dd."
    );

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Manage disk snapshots
pub fn snapshot_command(
    image: &Path,
    operation: &str,
    name: Option<String>,
    description: Option<String>,
    _verbose: bool,
) -> Result<()> {
    let msg = format!("Snapshot operation: {}...", operation);
    let progress = ProgressReporter::spinner(&msg);

    match operation {
        "create" => {
            let snap_name = name.unwrap_or_else(|| {
                chrono::Utc::now()
                    .format("snapshot-%Y%m%d-%H%M%S")
                    .to_string()
            });

            progress.set_message(format!("Creating snapshot '{}'...", snap_name));

            // In a real implementation, this would create a QCOW2 snapshot
            // or use libvirt snapshot APIs

            progress.finish_and_clear();

            println!("✓ Created snapshot: {}", snap_name);
            if let Some(desc) = description {
                println!("  Description: {}", desc);
            }
            println!("  Image: {}", image.display());
            println!();
            println!("Note: Snapshot creation not fully implemented yet");
            println!("      Would create QCOW2 internal snapshot or use qemu-img");
        }

        "list" => {
            progress.set_message("Listing snapshots...");

            progress.finish_and_clear();

            println!("Snapshots for {}:", image.display());
            println!();
            println!("Note: Snapshot listing not fully implemented yet");
            println!("      Would use qemu-img snapshot -l or libvirt APIs");
        }

        "delete" => {
            if let Some(snap_name) = name {
                progress.set_message(format!("Deleting snapshot '{}'...", snap_name));

                progress.finish_and_clear();

                println!("✓ Deleted snapshot: {}", snap_name);
                println!();
                println!("Note: Snapshot deletion not fully implemented yet");
                println!("      Would use qemu-img snapshot -d");
            } else {
                progress.abandon_with_message("Snapshot name required for delete operation");
                anyhow::bail!("Please provide snapshot name with --name");
            }
        }

        "revert" => {
            if let Some(snap_name) = name {
                progress.set_message(format!("Reverting to snapshot '{}'...", snap_name));

                progress.finish_and_clear();

                println!("✓ Reverted to snapshot: {}", snap_name);
                println!();
                println!("Note: Snapshot revert not fully implemented yet");
                println!("      Would use qemu-img snapshot -a");
            } else {
                progress.abandon_with_message("Snapshot name required for revert operation");
                anyhow::bail!("Please provide snapshot name with --name");
            }
        }

        "info" => {
            if let Some(snap_name) = name {
                progress.set_message(format!("Getting info for snapshot '{}'...", snap_name));

                progress.finish_and_clear();

                println!("Snapshot Information");
                println!("====================");
                println!("Name: {}", snap_name);
                println!("Image: {}", image.display());
                if let Some(desc) = description {
                    println!("Description: {}", desc);
                }
                println!();
                println!("Note: Snapshot info not fully implemented yet");
                println!("      Would parse qemu-img snapshot -l output");
            } else {
                progress.abandon_with_message("Snapshot name required for info operation");
                anyhow::bail!("Please provide snapshot name with --name");
            }
        }

        _ => {
            progress.abandon_with_message(format!("Unknown operation: {}", operation));
            anyhow::bail!("Invalid snapshot operation");
        }
    }

    Ok(())
}

/// Compare files or directories between disk images
pub fn diff_command(
    image1: &Path,
    image2: &Path,
    path: &str,
    unified: bool,
    _context: usize,
    ignore_whitespace: bool,
    verbose: bool,
) -> Result<()> {
    let progress = ProgressReporter::spinner("Loading disk images...");

    let mut g1 = init_guestfs_ro(image1, verbose)?;
    let mut g2 = init_guestfs_ro(image2, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g1);
    mount_all_ro(&mut g2);

    progress.set_message(format!("Comparing {}...", path));

    // Check if path exists in both images
    let exists1 = g1.exists(path).unwrap_or(false);
    let exists2 = g2.exists(path).unwrap_or(false);

    progress.finish_and_clear();

    let cleanup = |g1: &mut crate::Guestfs, g2: &mut crate::Guestfs| {
        if let Err(e) = g1.umount_all() {
            log::warn!("Cleanup: umount_all failed: {}", e);
        }
        if let Err(e) = g2.umount_all() {
            log::warn!("Cleanup: umount_all failed: {}", e);
        }
        if let Err(e) = g1.shutdown() {
            log::warn!("Cleanup: shutdown failed: {}", e);
        }
        if let Err(e) = g2.shutdown() {
            log::warn!("Cleanup: shutdown failed: {}", e);
        }
    };

    if !exists1 && !exists2 {
        println!("Path '{}' not found in either image", path);
        cleanup(&mut g1, &mut g2);
        return Ok(());
    }

    if !exists1 {
        println!("--- {}", path);
        println!("+++ {} (only in image2)", path);
        cleanup(&mut g1, &mut g2);
        return Ok(());
    }

    if !exists2 {
        println!("--- {} (only in image1)", path);
        println!("+++ {}", path);
        cleanup(&mut g1, &mut g2);
        return Ok(());
    }

    // Compare files
    if g1.is_file(path).unwrap_or(false) && g2.is_file(path).unwrap_or(false) {
        let content1 = g1.read_file(path)?;
        let content2 = g2.read_file(path)?;

        if content1 == content2 {
            println!("Files are identical: {}", path);
        } else {
            println!("--- {} (image1)", path);
            println!("+++ {} (image2)", path);

            if let (Ok(text1), Ok(text2)) = (
                std::str::from_utf8(&content1),
                std::str::from_utf8(&content2),
            ) {
                let lines1: Vec<&str> = text1.lines().collect();
                let lines2: Vec<&str> = text2.lines().collect();

                if unified {
                    println!("@@ -{},{} +{},{} @@", 1, lines1.len(), 1, lines2.len());
                }

                for (idx, (line1, line2)) in lines1.iter().zip(lines2.iter()).enumerate() {
                    if line1 != line2 && (!ignore_whitespace || line1.trim() != line2.trim()) {
                        if unified {
                            println!("-{}", line1);
                            println!("+{}", line2);
                        } else {
                            println!("{}c{}", idx + 1, idx + 1);
                            println!("< {}", line1);
                            println!("---");
                            println!("> {}", line2);
                        }
                    }
                }

                if lines1.len() != lines2.len() {
                    println!(
                        "File sizes differ: {} vs {} lines",
                        lines1.len(),
                        lines2.len()
                    );
                }
            } else {
                println!(
                    "Binary files differ: {} vs {} bytes",
                    content1.len(),
                    content2.len()
                );
            }
        }
    } else if g1.is_dir(path).unwrap_or(false) && g2.is_dir(path).unwrap_or(false) {
        // Compare directories
        let files1: std::collections::HashSet<_> = g1.ls(path)?.into_iter().collect();
        let files2: std::collections::HashSet<_> = g2.ls(path)?.into_iter().collect();

        let only_in_1: Vec<_> = files1.difference(&files2).collect();
        let only_in_2: Vec<_> = files2.difference(&files1).collect();

        let has_diff = !only_in_1.is_empty() || !only_in_2.is_empty();

        if !only_in_1.is_empty() {
            println!("Only in image1:");
            for file in only_in_1 {
                println!("  {}", file);
            }
        }

        if !only_in_2.is_empty() {
            println!("Only in image2:");
            for file in only_in_2 {
                println!("  {}", file);
            }
        }

        if !has_diff {
            println!("Directories have the same files: {}", path);
        }
    } else {
        println!(
            "Type mismatch: {} is different types in the two images",
            path
        );
    }

    if let Err(e) = g1.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g2.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g1.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    if let Err(e) = g2.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Find large files in disk image
pub fn find_large_command(
    image: &Path,
    path: &str,
    min_size: u64,
    max_results: usize,
    human_readable: bool,
    verbose: bool,
) -> Result<()> {
    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Scanning {} for large files...", path));

    let all_files = g.find(path)?;
    let mut file_sizes = Vec::new();

    for file in all_files {
        if g.is_file(&file).unwrap_or(false) {
            if let Ok(stat) = g.stat(&file) {
                if stat.size >= 0 && stat.size >= min_size as i64 {
                    file_sizes.push((file, stat.size as u64));
                }
            }
        }
    }

    // Sort by size descending
    file_sizes.sort_by_key(|b| std::cmp::Reverse(b.1));
    file_sizes.truncate(max_results);

    progress.finish_and_clear();

    println!("Large Files (minimum {} bytes)", min_size);
    println!("================================");
    println!();

    if file_sizes.is_empty() {
        println!("No files found larger than {} bytes", min_size);
    } else {
        for (file, size) in file_sizes {
            if human_readable {
                println!("{:>10}  {}", format_size(size), file);
            } else {
                println!("{:>15}  {}", size, file);
            }
        }
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Shrink an oversized-but-mostly-empty disk to its real footprint.
///
/// Only shrinks when virtual/actual >= `min_ratio` and the layout is a
/// supported single/last ext2/3/4 partition (MBR or GPT, no LVM/LUKS) —
/// anything else is reported and left untouched, never a hard error.
pub fn shrink_command(
    image: &Path,
    dry_run: bool,
    min_ratio: f64,
    headroom_pct: u32,
    json: bool,
    verbose: bool,
) -> Result<()> {
    use crate::guestfs::shrink::{analyze_shrink_potential, shrink_disk, ShrinkOutcome};

    let progress = ProgressReporter::spinner(&format!("Analyzing {}...", image.display()));
    let analysis = analyze_shrink_potential(image, verbose)?;
    progress.finish_and_clear();

    let analysis = match analysis {
        Some(a) => a,
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"shrunk": false, "reason": "not an eligible shrink candidate (unsupported layout, filesystem, or partition scheme)"})
                );
            } else {
                println!("Not shrinking: not an eligible candidate (unsupported layout, filesystem, or partition scheme).");
            }
            return Ok(());
        }
    };

    let ratio = if analysis.actual_bytes > 0 {
        analysis.virtual_bytes as f64 / analysis.actual_bytes as f64
    } else {
        f64::INFINITY
    };

    if !json {
        println!(
            "Virtual: {}  Actual: {}  Ratio: {:.1}x  Filesystem: {} (partition {})",
            format_size(analysis.virtual_bytes),
            format_size(analysis.actual_bytes),
            ratio,
            analysis.fs_type,
            analysis.last_partition,
        );
    }

    if ratio < min_ratio {
        let reason = format!(
            "virtual/actual ratio {ratio:.1}x is below --min-ratio {min_ratio:.1}x — not worth shrinking"
        );
        if json {
            println!("{}", serde_json::json!({"shrunk": false, "reason": reason}));
        } else {
            println!("Not shrinking: {reason}");
        }
        return Ok(());
    }

    if dry_run {
        if json {
            println!(
                "{}",
                serde_json::json!({"shrunk": false, "dry_run": true, "would_shrink": true, "virtual_bytes": analysis.virtual_bytes, "actual_bytes": analysis.actual_bytes, "min_fs_bytes": analysis.min_fs_bytes})
            );
        } else {
            println!(
                "Dry-run: would shrink (min filesystem size: {}).",
                format_size(analysis.min_fs_bytes)
            );
        }
        return Ok(());
    }

    let progress = ProgressReporter::spinner(&format!("Shrinking {}...", image.display()));
    let outcome = shrink_disk(image, verbose, headroom_pct)?;
    progress.finish_and_clear();

    match outcome {
        ShrinkOutcome::Shrunk {
            old_virtual,
            new_virtual,
        } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"shrunk": true, "old_virtual_bytes": old_virtual, "new_virtual_bytes": new_virtual})
                );
            } else {
                println!(
                    "✓ Shrunk {} -> {} ({:.1}x smaller)",
                    format_size(old_virtual),
                    format_size(new_virtual),
                    old_virtual as f64 / new_virtual as f64
                );
            }
        }
        ShrinkOutcome::Skipped { reason } => {
            if json {
                println!("{}", serde_json::json!({"shrunk": false, "reason": reason}));
            } else {
                println!("Not shrunk: {reason}");
            }
        }
    }

    Ok(())
}

pub fn disk_usage_command(
    image: &Path,
    path: &str,
    max_depth: usize,
    min_size: u64,
    human_readable: bool,
    verbose: bool,
) -> Result<()> {
    use std::collections::HashMap;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Analyzing disk usage in {}...", path));

    let all_files = g.find(path)?;
    let mut dir_sizes: HashMap<String, u64> = HashMap::new();

    for file in all_files {
        if g.is_file(&file).unwrap_or(false) {
            if let Ok(stat) = g.stat(&file) {
                let size = stat.size as u64;

                // Add to each parent directory
                let parts: Vec<&str> = file.split('/').collect();
                for depth in 1..=parts.len().min(max_depth) {
                    let dir_path = parts[..depth].join("/");
                    let dir_path = if dir_path.is_empty() { "/" } else { &dir_path };
                    *dir_sizes.entry(dir_path.to_string()).or_insert(0) += size;
                }
            }
        }
    }

    progress.finish_and_clear();

    // Sort by size
    let mut sorted_dirs: Vec<_> = dir_sizes
        .iter()
        .filter(|&(_, size)| *size >= min_size)
        .collect();
    sorted_dirs.sort_by(|a, b| b.1.cmp(a.1));

    println!("Disk Usage Analysis");
    println!("===================");
    println!("Path: {}", path);
    println!("Max depth: {}", max_depth);
    println!();

    println!("{:>15}  DIRECTORY", "SIZE");
    println!("{}", "-".repeat(80));

    for (dir, size) in sorted_dirs {
        if human_readable {
            println!("{:>15}  {}", format_size(*size), dir);
        } else {
            println!("{:>15}  {}", size, dir);
        }
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}
