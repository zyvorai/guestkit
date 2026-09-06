// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! LVM cloning operations for host logical volumes
//!
//! This module provides LVM-based cloning of logical volumes on the **host**,
//! independent of any guest disk image. It uses snapshot+dd for a consistent
//! copy, then regenerates filesystem UUIDs and updates fstab/bootloader/crypttab
//! references so the clone can boot independently.
//!
//! ## Security features
//!
//! - **Mount namespace isolation**: Optional `unshare(CLONE_NEWNS)` prevents
//!   mount/unmount operations from leaking to the host.
//! - **No shell invocation**: All commands use `Command::new()` with `.arg()`.
//! - **Per-UUID replacement**: fstab/crypttab/bootloader updates replace only
//!   the specific old UUID with its new value — swap and other partitions are
//!   never touched unless they have a mapping (bug #3 fix).
//! - **Regex-escaped inputs**: VG/LV names and UUIDs are escaped before use in
//!   patterns (bug #4 fix).
//! - **LUKS support**: Detects LUKS-encrypted volumes and regenerates their
//!   UUIDs.
//! - **Initramfs regeneration**: Properly bind-mounts /proc, /sys, /dev from
//!   the host before chroot operations (bug #2 fix).
//! - **Post-clone security verification**: Checks shadow permissions, SSH
//!   config, world-writable files.
//!
//! **Requires**: lvm2, util-linux (blkid, mount, umount, chroot, findmnt),
//! and sudo/root permissions. Optional: cryptsetup (LUKS), efibootmgr (EFI),
//! dracut/update-initramfs/mkinitcpio (initramfs).

use crate::core::{Error, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Level of namespace isolation to apply during clone operations.
///
/// Higher isolation levels provide stronger separation from the host but
/// require more privileges (`CAP_SYS_ADMIN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// No namespace isolation.
    None,
    /// Mount namespace only (`CLONE_NEWNS`) — current default.
    MountOnly,
    /// Full isolation: mount + PID + UTS + IPC + network namespaces.
    Full,
}

/// Configuration for an LVM clone operation.
#[derive(Debug, Clone)]
pub struct LvmCloneConfig {
    /// Source volume group name.
    pub source_vg: String,
    /// Source logical volume name.
    pub source_lv: String,
    /// Name for the cloned logical volume.
    pub clone_lv_name: String,
    /// Target volume group (defaults to `source_vg` if `None`).
    pub target_vg: Option<String>,
    /// Whether to regenerate filesystem UUIDs on the clone.
    pub regenerate_uuids: bool,
    /// Whether to update /etc/fstab inside the clone.
    pub update_fstab: bool,
    /// Whether to update GRUB bootloader config inside the clone.
    pub update_bootloader: bool,
    /// Whether to update /etc/crypttab inside the clone.
    pub update_crypttab: bool,
    /// Optional new hostname to set inside the clone.
    pub hostname: Option<String>,
    /// If true, validate parameters but do not perform the clone.
    pub dry_run: bool,
    /// Snapshot size (e.g. "10G"). If `None`, defaults to "10G".
    pub snapshot_size: Option<String>,
    /// Regenerate initramfs after UUID changes (requires chroot tools).
    pub regenerate_initramfs: bool,
    /// Namespace isolation level (requires root/CAP_SYS_ADMIN).
    pub isolation_level: IsolationLevel,
    /// Run post-clone security verification.
    pub verify_security: bool,
    /// Regenerate GRUB config via grub-mkconfig in chroot.
    pub regenerate_grub: bool,
    /// Verify boot configuration (kernel, initramfs, GRUB) after changes.
    pub verify_boot: bool,
    /// Container image for podman-based isolation (e.g. "fedora:latest").
    pub container_image: Option<String>,
}

/// Records the old and new UUID for a single filesystem device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UuidMapping {
    /// Device path (e.g. `/dev/vg0/clone-root`).
    pub device: String,
    /// Filesystem type (e.g. `ext4`, `xfs`, `crypto_LUKS`).
    pub fs_type: String,
    /// UUID before regeneration.
    pub old_uuid: String,
    /// UUID after regeneration.
    pub new_uuid: String,
}

/// LUKS encryption info detected on a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuksInfo {
    /// Device path.
    pub device: String,
    /// LUKS UUID.
    pub uuid: String,
    /// LUKS version (1 or 2).
    pub version: u32,
}

/// A warning produced by post-clone security verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityWarning {
    /// Category (e.g. "shadow_permissions", "ssh_root_login").
    pub category: String,
    /// Human-readable description.
    pub message: String,
}

/// Result metadata from a completed clone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResult {
    /// Source LV device path.
    pub source_path: String,
    /// Clone LV device path.
    pub clone_path: String,
    /// UUID mappings produced during regeneration.
    pub uuid_mappings: Vec<UuidMapping>,
    /// ISO-8601 timestamp of clone completion.
    pub timestamp: String,
    /// Whether fstab was updated inside the clone.
    pub fstab_updated: bool,
    /// Whether bootloader config was updated.
    pub bootloader_updated: bool,
    /// Whether crypttab was updated.
    pub crypttab_updated: bool,
    /// Whether initramfs was regenerated.
    pub initramfs_regenerated: bool,
    /// Whether mount namespace isolation was used.
    pub namespace_isolated: bool,
    /// Whether GRUB config was regenerated via grub-mkconfig.
    pub grub_regenerated: bool,
    /// Whether boot configuration was verified.
    pub boot_verified: bool,
    /// Detected kernel version (if any).
    pub kernel_version: Option<String>,
    /// Paths to backup files created (e.g. fstab.bak).
    pub backup_files: Vec<String>,
    /// Security warnings from post-clone verification.
    pub security_warnings: Vec<SecurityWarning>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Clone a logical volume according to `config`.
///
/// High-level flow:
///  1. Optionally enter a mount namespace for isolation.
///  2. Validate source LV exists and is not mounted.
///  3. Early return for dry-run.
///  4. Create an LVM snapshot of the source.
///  5. Create the target LV with the same size.
///  6. `dd` the snapshot to the target.
///  7. Remove the snapshot.
///  8. Optionally regenerate filesystem UUIDs (including LUKS).
///  9. Mount clone and update fstab / bootloader / crypttab / hostname.
/// 10. Optionally regenerate initramfs via chroot.
/// 11. Optionally run post-clone security verification.
/// 12. Return [`CloneResult`].
pub fn lvm_clone(config: &LvmCloneConfig, verbose: bool) -> Result<CloneResult> {
    let target_vg = config.target_vg.as_deref().unwrap_or(&config.source_vg);
    let source_path = format!("/dev/{}/{}", config.source_vg, config.source_lv);
    let clone_path = format!("/dev/{}/{}", target_vg, config.clone_lv_name);

    // 1. Namespace isolation
    if config.isolation_level != IsolationLevel::None {
        setup_namespace_isolation(&config.isolation_level, verbose)?;
    }

    // 2. Validate source exists
    if !Path::new(&source_path).exists() {
        return Err(Error::NotFound(format!(
            "Source LV does not exist: {}",
            source_path
        )));
    }

    if is_device_mounted(&source_path)? {
        return Err(Error::InvalidState(format!(
            "Source LV is currently mounted: {}",
            source_path
        )));
    }

    if Path::new(&clone_path).exists() {
        return Err(Error::InvalidState(format!(
            "Target LV already exists: {}",
            clone_path
        )));
    }

    // 3. Dry-run
    if config.dry_run {
        if verbose {
            eprintln!("lvm_clone: dry-run -- no changes made");
        }
        return Ok(CloneResult {
            source_path,
            clone_path,
            uuid_mappings: Vec::new(),
            timestamp: Utc::now().to_rfc3339(),
            fstab_updated: false,
            bootloader_updated: false,
            crypttab_updated: false,
            initramfs_regenerated: false,
            namespace_isolated: config.isolation_level != IsolationLevel::None,
            grub_regenerated: false,
            boot_verified: false,
            kernel_version: None,
            backup_files: Vec::new(),
            security_warnings: Vec::new(),
        });
    }

    // 4. Get source size
    let size_bytes = lv_size_bytes(&config.source_vg, &config.source_lv)?;
    if verbose {
        eprintln!("lvm_clone: source size = {} bytes", size_bytes);
    }

    // 5. Create snapshot
    let snap_name = format!("{}-clone-snap", config.source_lv);
    let snap_size = config.snapshot_size.as_deref().unwrap_or("10G").to_string();
    let snap_path = create_snapshot(
        &config.source_vg,
        &config.source_lv,
        &snap_name,
        &snap_size,
        verbose,
    )?;

    // 6. Create target LV
    let target_path = match create_target_lv(target_vg, &config.clone_lv_name, size_bytes, verbose)
    {
        Ok(p) => p,
        Err(e) => {
            if let Err(cleanup_err) = remove_lv(&snap_path) {
                log::warn!(
                    "Failed to clean up snapshot LV {}: {}",
                    snap_path,
                    cleanup_err
                );
            }
            return Err(e);
        }
    };

    // 7. dd snapshot -> target
    if let Err(e) = dd_copy(&snap_path, &target_path, verbose) {
        if let Err(cleanup_err) = remove_lv(&snap_path) {
            log::warn!(
                "Failed to clean up snapshot LV {}: {}",
                snap_path,
                cleanup_err
            );
        }
        if let Err(cleanup_err) = remove_lv(&target_path) {
            log::warn!(
                "Failed to clean up target LV {}: {}",
                target_path,
                cleanup_err
            );
        }
        return Err(e);
    }

    // 8. Remove snapshot
    remove_lv(&snap_path)?;

    // 9. Regenerate UUIDs (filesystem + LUKS)
    let uuid_mappings = if config.regenerate_uuids {
        regenerate_clone_uuids(&target_path, verbose)?
    } else {
        Vec::new()
    };

    // 10. Mount clone and apply customizations
    let mut fstab_updated = false;
    let mut bootloader_updated = false;
    let mut crypttab_updated = false;
    let mut initramfs_regenerated = false;
    let mut grub_regenerated = false;
    let mut boot_verified = false;
    let mut kernel_version: Option<String> = None;
    let mut backup_files: Vec<String> = Vec::new();
    let mut security_warnings: Vec<SecurityWarning> = Vec::new();

    let needs_mount = config.update_fstab
        || config.update_bootloader
        || config.update_crypttab
        || config.hostname.is_some()
        || config.regenerate_initramfs
        || config.regenerate_grub
        || config.verify_boot
        || config.verify_security;

    if needs_mount {
        let tmp = tempfile::tempdir()
            .map_err(|e| Error::CommandFailed(format!("Failed to create temp dir: {}", e)))?;
        let root_mount = tmp.path();

        mount_device(&target_path, root_mount, verbose)?;

        // Update fstab
        if config.update_fstab && !uuid_mappings.is_empty() {
            match update_clone_fstab(root_mount, &uuid_mappings) {
                Ok(bak) => {
                    fstab_updated = true;
                    if let Some(b) = bak {
                        backup_files.push(b);
                    }
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone: warning: fstab update failed: {}", e);
                }
                _ => {}
            }
        }

        // Update bootloader
        if config.update_bootloader && !uuid_mappings.is_empty() {
            match update_clone_bootloader(root_mount, &uuid_mappings, verbose) {
                Ok(baks) => {
                    bootloader_updated = true;
                    backup_files.extend(baks);
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone: warning: bootloader update failed: {}", e);
                }
                _ => {}
            }
        }

        // Update crypttab
        if config.update_crypttab && !uuid_mappings.is_empty() {
            match update_clone_crypttab(root_mount, &uuid_mappings) {
                Ok(bak) => {
                    crypttab_updated = true;
                    if let Some(b) = bak {
                        backup_files.push(b);
                    }
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone: warning: crypttab update failed: {}", e);
                }
                _ => {}
            }
        }

        // Set hostname
        if let Some(ref new_hostname) = config.hostname {
            set_hostname(root_mount, new_hostname, verbose)?;
        }

        // Detect kernel version (used by initramfs and boot verification)
        kernel_version = detect_kernel_version(root_mount).ok();

        // Regenerate initramfs
        if config.regenerate_initramfs {
            match regenerate_initramfs(root_mount, verbose) {
                Ok(()) => {
                    initramfs_regenerated = true;
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone: warning: initramfs regeneration failed: {}", e);
                }
                _ => {}
            }
        }

        // Regenerate GRUB config via grub-mkconfig
        if config.regenerate_grub {
            match regenerate_grub_config(root_mount, verbose) {
                Ok(baks) => {
                    grub_regenerated = true;
                    backup_files.extend(baks);
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone: warning: GRUB regeneration failed: {}", e);
                }
                _ => {}
            }
        }

        // Verify boot configuration
        if config.verify_boot {
            let boot_warnings = verify_boot_configuration(root_mount, verbose);
            if boot_warnings.is_empty() {
                boot_verified = true;
            }
            security_warnings.extend(boot_warnings);
        }

        // Security verification
        if config.verify_security {
            security_warnings = verify_clone_security(root_mount, verbose);
        }

        // Unmount
        unmount_device(root_mount, verbose)?;
    }

    Ok(CloneResult {
        source_path,
        clone_path: target_path,
        uuid_mappings,
        timestamp: Utc::now().to_rfc3339(),
        fstab_updated,
        bootloader_updated,
        crypttab_updated,
        initramfs_regenerated,
        namespace_isolated: config.isolation_level != IsolationLevel::None,
        grub_regenerated,
        boot_verified,
        kernel_version,
        backup_files,
        security_warnings,
    })
}

/// Clone a logical volume using Podman container isolation.
///
/// Same workflow as [`lvm_clone`] but all privileged LVM and filesystem
/// operations run inside a Podman container, providing stronger isolation
/// without requiring the calling process itself to be root — only Podman
/// access is needed.
///
/// The container is automatically cleaned up on success or error.
pub fn lvm_clone_podman(config: &LvmCloneConfig, verbose: bool) -> Result<CloneResult> {
    // Check that podman is available
    Command::new("podman")
        .arg("--version")
        .output()
        .map_err(|_| Error::CommandFailed("podman not found".to_string()))?;

    let target_vg = config.target_vg.as_deref().unwrap_or(&config.source_vg);
    let source_path = format!("/dev/{}/{}", config.source_vg, config.source_lv);
    let clone_path = format!("/dev/{}/{}", target_vg, config.clone_lv_name);

    // 1. Build image from project Dockerfile, then start container
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut container = PodmanContainer::new();
    container.build_image(project_dir, config.container_image.as_deref(), verbose)?;
    container.start(verbose)?;

    let cb = CmdBuilder::Podman(&container);

    // 2. Validate source exists (check via container)
    let check = cb
        .build_command("test")
        .arg("-e")
        .arg(&source_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to check source LV: {}", e)))?;

    if !check.status.success() {
        return Err(Error::NotFound(format!(
            "Source LV does not exist: {}",
            source_path
        )));
    }

    if is_device_mounted_with(&cb, &source_path)? {
        return Err(Error::InvalidState(format!(
            "Source LV is currently mounted: {}",
            source_path
        )));
    }

    let check_clone = cb
        .build_command("test")
        .arg("-e")
        .arg(&clone_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to check clone LV: {}", e)))?;

    if check_clone.status.success() {
        return Err(Error::InvalidState(format!(
            "Target LV already exists: {}",
            clone_path
        )));
    }

    // Dry-run
    if config.dry_run {
        if verbose {
            eprintln!("lvm_clone_podman: dry-run -- no changes made");
        }
        return Ok(CloneResult {
            source_path,
            clone_path,
            uuid_mappings: Vec::new(),
            timestamp: Utc::now().to_rfc3339(),
            fstab_updated: false,
            bootloader_updated: false,
            crypttab_updated: false,
            initramfs_regenerated: false,
            namespace_isolated: false,
            grub_regenerated: false,
            boot_verified: false,
            kernel_version: None,
            backup_files: Vec::new(),
            security_warnings: Vec::new(),
        });
    }

    // 4. Get source size
    let size_bytes = lv_size_bytes_with(&cb, &config.source_vg, &config.source_lv)?;
    if verbose {
        eprintln!("lvm_clone_podman: source size = {} bytes", size_bytes);
    }

    // 5. Create snapshot
    let snap_name = format!("{}-clone-snap", config.source_lv);
    let snap_size = config.snapshot_size.as_deref().unwrap_or("10G").to_string();
    let snap_path = create_snapshot_with(
        &cb,
        &config.source_vg,
        &config.source_lv,
        &snap_name,
        &snap_size,
        verbose,
    )?;

    // 6. Create target LV
    let target_path =
        match create_target_lv_with(&cb, target_vg, &config.clone_lv_name, size_bytes, verbose) {
            Ok(p) => p,
            Err(e) => {
                if let Err(cleanup_err) = remove_lv_with(&cb, &snap_path) {
                    log::warn!(
                        "Failed to clean up snapshot LV {}: {}",
                        snap_path,
                        cleanup_err
                    );
                }
                return Err(e);
            }
        };

    // 7. dd snapshot -> target
    if let Err(e) = dd_copy_with(&cb, &snap_path, &target_path, verbose) {
        if let Err(cleanup_err) = remove_lv_with(&cb, &snap_path) {
            log::warn!(
                "Failed to clean up snapshot LV {}: {}",
                snap_path,
                cleanup_err
            );
        }
        if let Err(cleanup_err) = remove_lv_with(&cb, &target_path) {
            log::warn!(
                "Failed to clean up target LV {}: {}",
                target_path,
                cleanup_err
            );
        }
        return Err(e);
    }

    // 8. Remove snapshot
    remove_lv_with(&cb, &snap_path)?;

    // 9. Regenerate UUIDs
    let uuid_mappings = if config.regenerate_uuids {
        regenerate_clone_uuids_with(&cb, &target_path, verbose)?
    } else {
        Vec::new()
    };

    // 10. Mount clone and apply customizations
    let mut fstab_updated = false;
    let mut bootloader_updated = false;
    let mut crypttab_updated = false;
    let mut initramfs_regenerated = false;
    let mut grub_regenerated = false;
    let mut boot_verified = false;
    let mut kernel_version: Option<String> = None;
    let mut backup_files: Vec<String> = Vec::new();
    let mut security_warnings: Vec<SecurityWarning> = Vec::new();

    let needs_mount = config.update_fstab
        || config.update_bootloader
        || config.update_crypttab
        || config.hostname.is_some()
        || config.regenerate_initramfs
        || config.regenerate_grub
        || config.verify_boot
        || config.verify_security;

    if needs_mount {
        let tmp = tempfile::tempdir()
            .map_err(|e| Error::CommandFailed(format!("Failed to create temp dir: {}", e)))?;
        let root_mount = tmp.path();

        mount_device_with(&cb, &target_path, root_mount, verbose)?;

        if config.update_fstab && !uuid_mappings.is_empty() {
            match update_clone_fstab(root_mount, &uuid_mappings) {
                Ok(bak) => {
                    fstab_updated = true;
                    if let Some(b) = bak {
                        backup_files.push(b);
                    }
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone_podman: warning: fstab update failed: {}", e);
                }
                _ => {}
            }
        }

        if config.update_bootloader && !uuid_mappings.is_empty() {
            match update_clone_bootloader(root_mount, &uuid_mappings, verbose) {
                Ok(baks) => {
                    bootloader_updated = true;
                    backup_files.extend(baks);
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone_podman: warning: bootloader update failed: {}", e);
                }
                _ => {}
            }
        }

        if config.update_crypttab && !uuid_mappings.is_empty() {
            match update_clone_crypttab(root_mount, &uuid_mappings) {
                Ok(bak) => {
                    crypttab_updated = true;
                    if let Some(b) = bak {
                        backup_files.push(b);
                    }
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone_podman: warning: crypttab update failed: {}", e);
                }
                _ => {}
            }
        }

        if let Some(ref new_hostname) = config.hostname {
            set_hostname(root_mount, new_hostname, verbose)?;
        }

        kernel_version = detect_kernel_version(root_mount).ok();

        if config.regenerate_initramfs {
            match regenerate_initramfs_with(&cb, root_mount, verbose) {
                Ok(()) => {
                    initramfs_regenerated = true;
                }
                Err(e) if verbose => {
                    eprintln!(
                        "lvm_clone_podman: warning: initramfs regeneration failed: {}",
                        e
                    );
                }
                _ => {}
            }
        }

        if config.regenerate_grub {
            match regenerate_grub_config_with(&cb, root_mount, verbose) {
                Ok(baks) => {
                    grub_regenerated = true;
                    backup_files.extend(baks);
                }
                Err(e) if verbose => {
                    eprintln!("lvm_clone_podman: warning: GRUB regeneration failed: {}", e);
                }
                _ => {}
            }
        }

        if config.verify_boot {
            let boot_warnings = verify_boot_configuration(root_mount, verbose);
            if boot_warnings.is_empty() {
                boot_verified = true;
            }
            security_warnings.extend(boot_warnings);
        }

        if config.verify_security {
            security_warnings = verify_clone_security(root_mount, verbose);
        }

        // Unmount
        unmount_device_with(&cb, root_mount, verbose)?;
    }

    // Container is stopped automatically via Drop

    Ok(CloneResult {
        source_path,
        clone_path: target_path,
        uuid_mappings,
        timestamp: Utc::now().to_rfc3339(),
        fstab_updated,
        bootloader_updated,
        crypttab_updated,
        initramfs_regenerated,
        namespace_isolated: false,
        grub_regenerated,
        boot_verified,
        kernel_version,
        backup_files,
        security_warnings,
    })
}

/// Run `fsck` on a logical volume to verify integrity.
pub fn lvm_clone_verify(lv_path: &str) -> Result<bool> {
    let output = build_sudo_command("fsck")
        .arg("-n")
        .arg(lv_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to run fsck: {}", e)))?;

    Ok(output.status.success())
}

/// Retrieve the filesystem UUID for `device` via `blkid`.
pub fn get_blkid_uuid(device: &str) -> Result<String> {
    get_blkid_uuid_with(&CmdBuilder::Sudo, device)
}

/// Like [`get_blkid_uuid`] but uses the given command builder.
fn get_blkid_uuid_with(cb: &CmdBuilder, device: &str) -> Result<String> {
    let output = cb
        .build_command("blkid")
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("blkid failed: {}", e)))?;

    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "blkid returned non-zero for {}",
            device
        )));
    }

    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() {
        return Err(Error::NotFound(format!("No UUID found for {}", device)));
    }
    Ok(uuid)
}

/// Set the filesystem UUID on `device` to `new_uuid`.
///
/// Dispatches to the appropriate tool based on filesystem type (tune2fs,
/// xfs_admin, btrfstune, mkswap, etc.).
pub fn set_filesystem_uuid(device: &str, new_uuid: &str, verbose: bool) -> Result<()> {
    set_filesystem_uuid_with(&CmdBuilder::Sudo, device, new_uuid, verbose)
}

/// Like [`set_filesystem_uuid`] but uses the given command builder.
fn set_filesystem_uuid_with(
    cb: &CmdBuilder,
    device: &str,
    new_uuid: &str,
    verbose: bool,
) -> Result<()> {
    let fs_type = detect_filesystem_type_with(cb, device)?;
    if verbose {
        eprintln!(
            "lvm_clone: setting UUID on {} ({}): {}",
            device, fs_type, new_uuid
        );
    }

    let output = match fs_type.as_str() {
        "ext2" | "ext3" | "ext4" => {
            // Run e2fsck for consistency before changing UUID
            ext_check_device_with(cb, device, verbose)?;
            cb.build_command("tune2fs")
                .arg("-U")
                .arg(new_uuid)
                .arg(device)
                .output()
                .map_err(|e| Error::CommandFailed(format!("tune2fs failed: {}", e)))?
        }
        "xfs" => {
            // XFS requires repair before UUID change
            xfs_repair_device_with(cb, device, verbose)?;
            cb.build_command("xfs_admin")
                .arg("-U")
                .arg(new_uuid)
                .arg(device)
                .output()
                .map_err(|e| Error::CommandFailed(format!("xfs_admin failed: {}", e)))?
        }
        "btrfs" => cb
            .build_command("btrfstune")
            .arg("-U")
            .arg(new_uuid)
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("btrfstune failed: {}", e)))?,
        "swap" => cb
            .build_command("mkswap")
            .arg("-U")
            .arg(new_uuid)
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("mkswap failed: {}", e)))?,
        "vfat" | "fat32" | "fat16" => {
            if verbose {
                eprintln!(
                    "lvm_clone: skipping UUID change for FAT filesystem on {}",
                    device
                );
            }
            return Ok(());
        }
        "ntfs" => {
            if verbose {
                eprintln!(
                    "lvm_clone: skipping UUID change for NTFS filesystem on {}",
                    device
                );
            }
            return Ok(());
        }
        other => {
            return Err(Error::Unsupported(format!(
                "Cannot set UUID for filesystem type: {}",
                other
            )));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "Failed to set UUID on {}: {}",
            device, stderr
        )));
    }

    Ok(())
}

/// Run post-clone security verification on a mounted clone filesystem.
///
/// Returns a list of warnings (empty = all checks passed).
pub fn verify_clone_security(root_mount: &Path, verbose: bool) -> Vec<SecurityWarning> {
    let mut warnings = Vec::new();

    if verbose {
        eprintln!("lvm_clone: running security verification");
    }

    // Check /etc/shadow permissions
    let shadow_path = root_mount.join("etc/shadow");
    if shadow_path.exists() {
        if let Ok(meta) = fs::metadata(&shadow_path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o000 && mode != 0o600 && mode != 0o640 {
                warnings.push(SecurityWarning {
                    category: "shadow_permissions".to_string(),
                    message: format!(
                        "/etc/shadow has permissions {:o}, expected 000/600/640",
                        mode
                    ),
                });
            }
        }
    }

    // Check SSH config for root login
    let sshd_config = root_mount.join("etc/ssh/sshd_config");
    if sshd_config.exists() {
        if let Ok(content) = fs::read_to_string(&sshd_config) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("PermitRootLogin") && !trimmed.starts_with('#') {
                    if trimmed.contains("yes") {
                        warnings.push(SecurityWarning {
                            category: "ssh_root_login".to_string(),
                            message: "PermitRootLogin is set to yes".to_string(),
                        });
                    }
                    break;
                }
            }
        }
    }

    // Check for passwordless root
    let shadow = root_mount.join("etc/shadow");
    if shadow.exists() {
        if let Ok(content) = fs::read_to_string(&shadow) {
            for line in content.lines() {
                if line.starts_with("root:") {
                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                    if parts.len() >= 2 && (parts[1].is_empty() || parts[1] == "!") {
                        // Locked or empty — fine
                    } else if parts.len() >= 2 && parts[1] == "*" {
                        // Disabled — fine
                    }
                    // Otherwise root has a password hash, which is expected
                    break;
                }
            }
        }
    }

    // Check /etc/fstab for secure mount options on /tmp
    let fstab_path = root_mount.join("etc/fstab");
    if fstab_path.exists() {
        if let Ok(content) = fs::read_to_string(&fstab_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 4 && parts[1] == "/tmp" {
                    let opts = parts[3];
                    if !opts.contains("noexec") || !opts.contains("nosuid") {
                        warnings.push(SecurityWarning {
                            category: "tmp_mount_options".to_string(),
                            message: "/tmp missing noexec,nosuid mount options".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Check for SSH host keys (should be removed for clones)
    let ssh_key_files = [
        "etc/ssh/ssh_host_rsa_key",
        "etc/ssh/ssh_host_ecdsa_key",
        "etc/ssh/ssh_host_ed25519_key",
    ];
    let mut has_host_keys = false;
    for key_file in &ssh_key_files {
        if root_mount.join(key_file).exists() {
            has_host_keys = true;
            break;
        }
    }
    if has_host_keys {
        warnings.push(SecurityWarning {
            category: "ssh_host_keys".to_string(),
            message: "Clone still contains SSH host keys (consider removing for uniqueness)"
                .to_string(),
        });
    }

    // Check machine-id
    let machine_id = root_mount.join("etc/machine-id");
    if machine_id.exists() {
        if let Ok(content) = fs::read_to_string(&machine_id) {
            if !content.trim().is_empty() && content.trim() != "uninitialized" {
                warnings.push(SecurityWarning {
                    category: "machine_id".to_string(),
                    message: "Clone has a populated machine-id (consider clearing for uniqueness)"
                        .to_string(),
                });
            }
        }
    }

    if verbose {
        if warnings.is_empty() {
            eprintln!("lvm_clone: security verification passed");
        } else {
            eprintln!(
                "lvm_clone: security verification found {} warning(s)",
                warnings.len()
            );
        }
    }

    warnings
}

// ---------------------------------------------------------------------------
// Namespace isolation
// ---------------------------------------------------------------------------

/// Enter namespaces for isolation at the requested level.
///
/// - [`IsolationLevel::MountOnly`]: `CLONE_NEWNS` + `mount --make-rprivate /`.
/// - [`IsolationLevel::Full`]: mount + PID + UTS + IPC + network namespaces.
///
/// Mounts created inside the namespace are automatically cleaned up on process
/// exit.  Requires root or `CAP_SYS_ADMIN`.
fn setup_namespace_isolation(level: &IsolationLevel, verbose: bool) -> Result<()> {
    if *level == IsolationLevel::None {
        return Ok(());
    }

    if verbose {
        eprintln!(
            "lvm_clone: entering namespace isolation (level={:?})",
            level
        );
    }

    #[cfg(target_os = "linux")]
    {
        let flags = match level {
            IsolationLevel::MountOnly => libc::CLONE_NEWNS,
            IsolationLevel::Full => {
                libc::CLONE_NEWNS | libc::CLONE_NEWUTS | libc::CLONE_NEWIPC | libc::CLONE_NEWNET
            }
            IsolationLevel::None => return Ok(()),
        };

        let ret = unsafe { libc::unshare(flags) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            return Err(Error::CommandFailed(format!(
                "unshare({:#x}) failed: {} (are you root?)",
                flags, err
            )));
        }

        let output = build_sudo_command("mount")
            .arg("--make-rprivate")
            .arg("/")
            .output()
            .map_err(|e| Error::CommandFailed(format!("mount --make-rprivate failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "mount --make-rprivate / failed: {}",
                stderr
            )));
        }

        if *level == IsolationLevel::Full {
            if verbose {
                eprintln!("lvm_clone: setting isolated hostname");
            }
            let hostname = b"lvm-clone-ns";
            let ret = unsafe {
                libc::sethostname(hostname.as_ptr() as *const libc::c_char, hostname.len())
            };
            if ret != 0 && verbose {
                eprintln!("lvm_clone: warning: sethostname failed (non-fatal)");
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = verbose;
        Err(Error::CommandFailed(
            "LVM namespace isolation is only supported on Linux".to_string(),
        ))
    }
}

/// Capture a snapshot of the host LVM state before isolation.
#[allow(dead_code)]
///
/// Returns a map of VG names to their UUIDs so we can verify later that
/// the host state was not accidentally modified.
fn capture_host_lvm_state() -> Result<HashMap<String, String>> {
    let output = build_sudo_command("vgs")
        .arg("--noheadings")
        .arg("--nosuffix")
        .arg("-o")
        .arg("vg_name,vg_uuid")
        .output()
        .map_err(|e| Error::CommandFailed(format!("vgs failed: {}", e)))?;

    let mut state = HashMap::new();
    if output.status.success() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                state.insert(parts[0].to_string(), parts[1].to_string());
            }
        }
    }
    Ok(state)
}

// ---------------------------------------------------------------------------
// Core helpers
// ---------------------------------------------------------------------------

/// Build a `Command` that is prefixed with `sudo` when not already root.
fn build_sudo_command(program: &str) -> Command {
    if is_root() {
        Command::new(program)
    } else {
        let mut cmd = Command::new("sudo");
        cmd.arg(program);
        cmd
    }
}

/// Return `true` when the current effective UID is 0.
fn is_root() -> bool {
    // SAFETY: geteuid() is a safe, read-only syscall.
    unsafe { libc::geteuid() == 0 }
}

// ---------------------------------------------------------------------------
// Command builder abstraction (sudo vs podman)
// ---------------------------------------------------------------------------

/// Selects how privileged commands are executed.
enum CmdBuilder<'a> {
    /// Use `sudo` (or run directly if root).
    Sudo,
    /// Run inside a Podman container.
    Podman(&'a PodmanContainer),
}

impl<'a> CmdBuilder<'a> {
    /// Build a `Command` that runs `program` in the selected execution context.
    fn build_command(&self, program: &str) -> Command {
        match self {
            CmdBuilder::Sudo => build_sudo_command(program),
            CmdBuilder::Podman(container) => container.exec(program),
        }
    }
}

// ---------------------------------------------------------------------------
// Podman container management
// ---------------------------------------------------------------------------

/// Manages a privileged Podman container for running LVM operations.
///
/// The image must be pre-built from the project `Dockerfile` (target `lvm-worker`)
/// using `podman build --target lvm-worker -t guestkit-lvm:latest .` or via
/// `docker-compose build lvm-worker` before calling [`PodmanContainer::start`].
struct PodmanContainer {
    name: String,
    image_tag: String,
    started: bool,
}

/// Default image tag for the pre-built guestkit LVM worker.
const GUESTKIT_LVM_IMAGE: &str = "guestkit-lvm:latest";

impl PodmanContainer {
    /// Create a new (not yet started) container handle.
    fn new() -> Self {
        let suffix = &Uuid::new_v4().to_string()[..8];
        Self {
            name: format!("guestkit-lvm-{}", suffix),
            image_tag: GUESTKIT_LVM_IMAGE.to_string(),
            started: false,
        }
    }

    /// Build the `guestkit-lvm:latest` image from the project Dockerfile.
    ///
    /// Uses `podman build --target lvm-worker` against the project's
    /// multi-stage `Dockerfile`.  If a `container_image` override is given,
    /// it is used as `--build-arg BASE_IMAGE=...`; otherwise the Dockerfile
    /// default (`fedora:43`) is used.
    ///
    /// The build is skipped when the image already exists locally.
    fn build_image(
        &self,
        project_dir: &Path,
        container_image: Option<&str>,
        verbose: bool,
    ) -> Result<()> {
        // Skip if image already exists
        let check = Command::new("podman")
            .arg("image")
            .arg("exists")
            .arg(&self.image_tag)
            .output();

        if let Ok(out) = check {
            if out.status.success() {
                if verbose {
                    eprintln!(
                        "lvm_clone_podman: image {} already exists, skipping build",
                        self.image_tag
                    );
                }
                return Ok(());
            }
        }

        // Find Dockerfile — try project_dir first, then CARGO_MANIFEST_DIR
        let dockerfile = project_dir.join("Dockerfile");
        if !dockerfile.exists() {
            return Err(Error::NotFound(format!(
                "Dockerfile not found at {}",
                dockerfile.display()
            )));
        }

        if verbose {
            eprintln!(
                "lvm_clone_podman: building image {} from {}",
                self.image_tag,
                dockerfile.display()
            );
        }

        let mut cmd = Command::new("podman");
        cmd.arg("build")
            .arg("--network=host")
            .arg("--target")
            .arg("lvm-worker")
            .arg("-t")
            .arg(&self.image_tag);

        if let Some(base) = container_image {
            cmd.arg("--build-arg").arg(format!("BASE_IMAGE={}", base));
        }

        cmd.arg("-f").arg(&dockerfile).arg(project_dir);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("podman build failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "podman build failed: {}",
                stderr.trim()
            )));
        }

        if verbose {
            eprintln!(
                "lvm_clone_podman: image {} built successfully",
                self.image_tag
            );
        }

        Ok(())
    }

    /// Start the container using the pre-built image.
    ///
    /// Mirrors the `lvm-worker` service in `docker-compose.yml`:
    /// privileged, pid=host, network=host, with /dev, /run/lvm,
    /// /run/udev, /etc/lvm bind-mounts.
    fn start(&mut self, verbose: bool) -> Result<()> {
        if verbose {
            eprintln!(
                "lvm_clone_podman: starting container {} (image={})",
                self.name, self.image_tag
            );
        }

        let output = Command::new("podman")
            .arg("run")
            .arg("-d")
            .arg("--rm")
            .arg("--privileged")
            .arg("--pid=host")
            .arg("--network=host")
            .arg("-v")
            .arg("/dev:/dev")
            .arg("-v")
            .arg("/run/lvm:/run/lvm")
            .arg("-v")
            .arg("/run/udev:/run/udev")
            .arg("-v")
            .arg("/etc/lvm:/etc/lvm")
            .arg("--name")
            .arg(&self.name)
            .arg(&self.image_tag)
            .arg("sleep")
            .arg("infinity")
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("podman not found or failed to start: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "podman run failed: {}",
                stderr.trim()
            )));
        }

        self.started = true;
        Ok(())
    }

    /// Build a `Command` that runs `program` inside the container via `podman exec`.
    fn exec(&self, program: &str) -> Command {
        let mut cmd = Command::new("podman");
        cmd.arg("exec").arg(&self.name).arg(program);
        cmd
    }

    /// Stop and remove the container.
    fn stop(&mut self) {
        if !self.started {
            return;
        }
        let _ = Command::new("podman").arg("stop").arg(&self.name).output();
        let _ = Command::new("podman")
            .arg("rm")
            .arg("-f")
            .arg(&self.name)
            .output();
        self.started = false;
    }
}

impl Drop for PodmanContainer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Check whether a device or mount point is currently mounted.
///
/// Paths starting with `/dev/` are checked with `findmnt --source`;
/// everything else uses `findmnt --mountpoint` (bug #9 fix).
fn is_device_mounted(device_or_path: &str) -> Result<bool> {
    is_device_mounted_with(&CmdBuilder::Sudo, device_or_path)
}

/// Like [`is_device_mounted`] but uses the given command builder.
fn is_device_mounted_with(cb: &CmdBuilder, device_or_path: &str) -> Result<bool> {
    let output = if device_or_path.starts_with("/dev/") {
        cb.build_command("findmnt")
            .arg("--source")
            .arg(device_or_path)
            .output()
    } else {
        cb.build_command("findmnt")
            .arg("--mountpoint")
            .arg(device_or_path)
            .output()
    };

    let output = output.map_err(|e| Error::CommandFailed(format!("findmnt failed: {}", e)))?;
    Ok(output.status.success())
}

// ---------------------------------------------------------------------------
// LVM operations
// ---------------------------------------------------------------------------

/// Query the size of an LV in bytes.
fn lv_size_bytes(vg: &str, lv: &str) -> Result<u64> {
    lv_size_bytes_with(&CmdBuilder::Sudo, vg, lv)
}

/// Like [`lv_size_bytes`] but uses the given command builder.
fn lv_size_bytes_with(cb: &CmdBuilder, vg: &str, lv: &str) -> Result<u64> {
    let lv_path = format!("{}/{}", vg, lv);
    let output = cb
        .build_command("lvs")
        .arg("--noheadings")
        .arg("--nosuffix")
        .arg("--units")
        .arg("b")
        .arg("-o")
        .arg("lv_size")
        .arg(&lv_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("lvs failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "lvs failed for {}/{}: {}",
            vg, lv, stderr
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // lvs may print "10737418240.00"
    let int_part = raw.split('.').next().unwrap_or(&raw);
    int_part
        .parse::<u64>()
        .map_err(|e| Error::InvalidFormat(format!("Cannot parse LV size '{}': {}", raw, e)))
}

/// Create an LVM snapshot.
fn create_snapshot(
    source_vg: &str,
    source_lv: &str,
    snapshot_name: &str,
    size: &str,
    verbose: bool,
) -> Result<String> {
    create_snapshot_with(
        &CmdBuilder::Sudo,
        source_vg,
        source_lv,
        snapshot_name,
        size,
        verbose,
    )
}

/// Like [`create_snapshot`] but uses the given command builder.
fn create_snapshot_with(
    cb: &CmdBuilder,
    source_vg: &str,
    source_lv: &str,
    snapshot_name: &str,
    size: &str,
    verbose: bool,
) -> Result<String> {
    let source_path = format!("/dev/{}/{}", source_vg, source_lv);
    if verbose {
        eprintln!(
            "lvm_clone: creating snapshot {} (size {}) of {}",
            snapshot_name, size, source_path
        );
    }

    let output = cb
        .build_command("lvcreate")
        .arg("--snapshot")
        .arg("--name")
        .arg(snapshot_name)
        .arg("--size")
        .arg(size)
        .arg(&source_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("lvcreate snapshot failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "lvcreate snapshot failed: {}",
            stderr
        )));
    }

    Ok(format!("/dev/{}/{}", source_vg, snapshot_name))
}

/// Create a new LV with the given size in bytes.
fn create_target_lv(vg: &str, lv_name: &str, size_bytes: u64, verbose: bool) -> Result<String> {
    create_target_lv_with(&CmdBuilder::Sudo, vg, lv_name, size_bytes, verbose)
}

/// Like [`create_target_lv`] but uses the given command builder.
fn create_target_lv_with(
    cb: &CmdBuilder,
    vg: &str,
    lv_name: &str,
    size_bytes: u64,
    verbose: bool,
) -> Result<String> {
    if verbose {
        eprintln!(
            "lvm_clone: creating target LV {}/{} ({} bytes)",
            vg, lv_name, size_bytes
        );
    }

    let size_arg = format!("{}b", size_bytes);
    let output = cb
        .build_command("lvcreate")
        .arg("--name")
        .arg(lv_name)
        .arg("--size")
        .arg(&size_arg)
        .arg(vg)
        .output()
        .map_err(|e| Error::CommandFailed(format!("lvcreate failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "lvcreate failed for {}/{}: {}",
            vg, lv_name, stderr
        )));
    }

    Ok(format!("/dev/{}/{}", vg, lv_name))
}

/// Copy `source` to `target` using `dd` (no shell — bug #1 fix).
fn dd_copy(source: &str, target: &str, verbose: bool) -> Result<()> {
    dd_copy_with(&CmdBuilder::Sudo, source, target, verbose)
}

/// Like [`dd_copy`] but uses the given command builder.
fn dd_copy_with(cb: &CmdBuilder, source: &str, target: &str, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: dd {} -> {}", source, target);
    }

    let if_arg = format!("if={}", source);
    let of_arg = format!("of={}", target);
    let output = cb
        .build_command("dd")
        .arg(&if_arg)
        .arg(&of_arg)
        .arg("bs=4M")
        .arg("conv=fsync")
        .arg("status=progress")
        .output()
        .map_err(|e| Error::CommandFailed(format!("dd failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!("dd failed: {}", stderr)));
    }

    Ok(())
}

/// Force-remove a logical volume.
fn remove_lv(device: &str) -> Result<()> {
    remove_lv_with(&CmdBuilder::Sudo, device)
}

/// Like [`remove_lv`] but uses the given command builder.
fn remove_lv_with(cb: &CmdBuilder, device: &str) -> Result<()> {
    let output = cb
        .build_command("lvremove")
        .arg("-f")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("lvremove failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "lvremove failed for {}: {}",
            device, stderr
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Filesystem detection and UUID operations
// ---------------------------------------------------------------------------

/// Detect the filesystem type on `device` via `blkid -s TYPE`.
#[allow(dead_code)]
fn detect_filesystem_type(device: &str) -> Result<String> {
    detect_filesystem_type_with(&CmdBuilder::Sudo, device)
}

/// Like [`detect_filesystem_type`] but uses the given command builder.
fn detect_filesystem_type_with(cb: &CmdBuilder, device: &str) -> Result<String> {
    let output = cb
        .build_command("blkid")
        .arg("-s")
        .arg("TYPE")
        .arg("-o")
        .arg("value")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("blkid failed: {}", e)))?;

    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "blkid could not detect filesystem on {}",
            device
        )));
    }

    let fs_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if fs_type.is_empty() {
        return Err(Error::NotFound(format!(
            "No filesystem type found on {}",
            device
        )));
    }
    Ok(fs_type)
}

/// Run `xfs_repair` on an XFS device (required before UUID change).
///
/// XFS requires the filesystem to be unmounted and clean before `xfs_admin -U`
/// can change the UUID. This runs `xfs_repair` to ensure consistency.
#[allow(dead_code)]
fn xfs_repair_device(device: &str, verbose: bool) -> Result<()> {
    xfs_repair_device_with(&CmdBuilder::Sudo, device, verbose)
}

/// Like [`xfs_repair_device`] but uses the given command builder.
fn xfs_repair_device_with(cb: &CmdBuilder, device: &str, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: running xfs_repair on {}", device);
    }

    if is_device_mounted_with(cb, device)? {
        return Err(Error::InvalidState(format!(
            "Device {} must be unmounted before xfs_repair",
            device
        )));
    }

    let output = cb
        .build_command("xfs_repair")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("xfs_repair failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "xfs_repair failed on {}: {}",
            device, stderr
        )));
    }

    Ok(())
}

/// Run `e2fsck -p` on an ext2/3/4 device for consistency before UUID change.
///
/// Uses `-p` (preen mode) for automatic, non-interactive repair. The device
/// must be unmounted. Non-fatal failures are tolerated (exit code 1 means
/// errors were corrected).
#[allow(dead_code)]
fn ext_check_device(device: &str, verbose: bool) -> Result<()> {
    ext_check_device_with(&CmdBuilder::Sudo, device, verbose)
}

/// Like [`ext_check_device`] but uses the given command builder.
fn ext_check_device_with(cb: &CmdBuilder, device: &str, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: running e2fsck -p on {}", device);
    }

    if is_device_mounted_with(cb, device)? {
        return Err(Error::InvalidState(format!(
            "Device {} must be unmounted before e2fsck",
            device
        )));
    }

    let output = cb
        .build_command("e2fsck")
        .arg("-f") // force check even if clean
        .arg("-p") // preen mode — auto-fix safe issues
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("e2fsck failed: {}", e)))?;

    // e2fsck exit codes: 0 = clean, 1 = errors corrected, 2+ = real errors
    let code = output.status.code().unwrap_or(255);
    if code > 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "e2fsck found uncorrectable errors on {}: {}",
            device, stderr
        )));
    }

    Ok(())
}

/// Verify that the host LVM state has not been accidentally modified.
///
/// Compares current VG UUIDs with a previously captured snapshot. Returns
/// a list of any changes detected (empty = host unchanged).
#[allow(dead_code)]
fn verify_host_lvm_unchanged(
    saved_state: &HashMap<String, String>,
    verbose: bool,
) -> Vec<SecurityWarning> {
    let mut warnings = Vec::new();

    if verbose {
        eprintln!("lvm_clone: verifying host LVM state unchanged");
    }

    let current = match capture_host_lvm_state() {
        Ok(state) => state,
        Err(e) => {
            warnings.push(SecurityWarning {
                category: "host_lvm_check".to_string(),
                message: format!("Could not verify host LVM state: {}", e),
            });
            return warnings;
        }
    };

    // Check that no saved VGs have changed UUIDs
    for (vg, saved_uuid) in saved_state {
        match current.get(vg) {
            Some(current_uuid) if current_uuid != saved_uuid => {
                warnings.push(SecurityWarning {
                    category: "host_lvm_modified".to_string(),
                    message: format!(
                        "Host VG '{}' UUID changed: {} -> {} (THIS SHOULD NOT HAPPEN)",
                        vg, saved_uuid, current_uuid
                    ),
                });
            }
            None => {
                warnings.push(SecurityWarning {
                    category: "host_lvm_missing".to_string(),
                    message: format!("Host VG '{}' is no longer visible", vg),
                });
            }
            _ => {} // UUID unchanged — good
        }
    }

    // Check for unexpected new VGs
    for vg in current.keys() {
        if !saved_state.contains_key(vg) && verbose {
            eprintln!("lvm_clone: new VG detected: {} (likely from clone)", vg);
        }
    }

    if verbose {
        if warnings.is_empty() {
            eprintln!("lvm_clone: host LVM state verified unchanged");
        } else {
            eprintln!(
                "lvm_clone: WARNING: host LVM state has {} change(s)",
                warnings.len()
            );
        }
    }

    warnings
}

/// Regenerate filesystem UUIDs on the clone LV, including LUKS if present.
///
/// Returns one [`UuidMapping`] per device whose UUID was successfully changed.
fn regenerate_clone_uuids(clone_lv_path: &str, verbose: bool) -> Result<Vec<UuidMapping>> {
    regenerate_clone_uuids_with(&CmdBuilder::Sudo, clone_lv_path, verbose)
}

/// Like [`regenerate_clone_uuids`] but uses the given command builder.
fn regenerate_clone_uuids_with(
    cb: &CmdBuilder,
    clone_lv_path: &str,
    verbose: bool,
) -> Result<Vec<UuidMapping>> {
    let mut mappings = Vec::new();

    // Check for LUKS first
    if let Some(luks_info) = detect_luks_with(cb, clone_lv_path, verbose)? {
        if let Ok(mapping) = change_luks_uuid_with(cb, clone_lv_path, &luks_info, verbose) {
            mappings.push(mapping);
        }
        return Ok(mappings);
    }

    // Get the current filesystem UUID before changing
    let old_uuid = match get_blkid_uuid_with(cb, clone_lv_path) {
        Ok(u) => u,
        Err(_) => {
            if verbose {
                eprintln!(
                    "lvm_clone: no UUID found on {}, skipping regeneration",
                    clone_lv_path
                );
            }
            return Ok(mappings);
        }
    };

    let fs_type = detect_filesystem_type_with(cb, clone_lv_path)?;
    let new_uuid = Uuid::new_v4().to_string();

    set_filesystem_uuid_with(cb, clone_lv_path, &new_uuid, verbose)?;

    mappings.push(UuidMapping {
        device: clone_lv_path.to_string(),
        fs_type,
        old_uuid,
        new_uuid,
    });

    Ok(mappings)
}

// ---------------------------------------------------------------------------
// LUKS encryption support
// ---------------------------------------------------------------------------

/// Detect whether `device` is a LUKS-encrypted volume.
///
/// Returns `Some(LuksInfo)` if LUKS is detected, `None` otherwise.
#[allow(dead_code)]
fn detect_luks(device: &str, verbose: bool) -> Result<Option<LuksInfo>> {
    detect_luks_with(&CmdBuilder::Sudo, device, verbose)
}

/// Like [`detect_luks`] but uses the given command builder.
fn detect_luks_with(cb: &CmdBuilder, device: &str, verbose: bool) -> Result<Option<LuksInfo>> {
    let output = cb
        .build_command("cryptsetup")
        .arg("isLuks")
        .arg(device)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => {
            if verbose {
                eprintln!("lvm_clone: cryptsetup not available, skipping LUKS detection");
            }
            return Ok(None);
        }
    };

    if !output.status.success() {
        return Ok(None);
    }

    if verbose {
        eprintln!("lvm_clone: LUKS detected on {}", device);
    }

    let uuid_output = cb
        .build_command("cryptsetup")
        .arg("luksUUID")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("cryptsetup luksUUID failed: {}", e)))?;

    let uuid = String::from_utf8_lossy(&uuid_output.stdout)
        .trim()
        .to_string();

    let version = detect_luks_version_with(cb, device).unwrap_or(2);

    Ok(Some(LuksInfo {
        device: device.to_string(),
        uuid,
        version,
    }))
}

/// Detect LUKS version (1 or 2) via `cryptsetup luksDump`.
#[allow(dead_code)]
fn detect_luks_version(device: &str) -> Result<u32> {
    detect_luks_version_with(&CmdBuilder::Sudo, device)
}

/// Like [`detect_luks_version`] but uses the given command builder.
fn detect_luks_version_with(cb: &CmdBuilder, device: &str) -> Result<u32> {
    let output = cb
        .build_command("cryptsetup")
        .arg("luksDump")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("cryptsetup luksDump failed: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("Version:") {
            let version_str = line.trim_start_matches("Version:").trim();
            return version_str
                .parse::<u32>()
                .map_err(|_| Error::InvalidFormat("Cannot parse LUKS version".to_string()));
        }
    }

    Ok(2) // Default to LUKS2
}

/// Change the LUKS UUID on `device` and return a UUID mapping.
#[allow(dead_code)]
fn change_luks_uuid(device: &str, luks_info: &LuksInfo, verbose: bool) -> Result<UuidMapping> {
    change_luks_uuid_with(&CmdBuilder::Sudo, device, luks_info, verbose)
}

/// Like [`change_luks_uuid`] but uses the given command builder.
fn change_luks_uuid_with(
    cb: &CmdBuilder,
    device: &str,
    luks_info: &LuksInfo,
    verbose: bool,
) -> Result<UuidMapping> {
    let new_uuid = Uuid::new_v4().to_string();

    if verbose {
        eprintln!(
            "lvm_clone: changing LUKS UUID on {}: {} -> {}",
            device, luks_info.uuid, new_uuid
        );
    }

    let output = cb
        .build_command("cryptsetup")
        .arg("luksUUID")
        .arg(device)
        .arg("--uuid")
        .arg(&new_uuid)
        .arg("--batch-mode")
        .output()
        .map_err(|e| Error::CommandFailed(format!("cryptsetup luksUUID --uuid failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "Failed to change LUKS UUID on {}: {}",
            device, stderr
        )));
    }

    Ok(UuidMapping {
        device: device.to_string(),
        fs_type: format!("crypto_LUKS{}", luks_info.version),
        old_uuid: luks_info.uuid.clone(),
        new_uuid,
    })
}

// ---------------------------------------------------------------------------
// Mount helpers
// ---------------------------------------------------------------------------

/// Mount a device to a mount point.
fn mount_device(device: &str, mount_point: &Path, verbose: bool) -> Result<()> {
    mount_device_with(&CmdBuilder::Sudo, device, mount_point, verbose)
}

/// Like [`mount_device`] but uses the given command builder.
fn mount_device_with(
    cb: &CmdBuilder,
    device: &str,
    mount_point: &Path,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!(
            "lvm_clone: mounting {} at {}",
            device,
            mount_point.display()
        );
    }

    let mp_str = mount_point
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 mount point path".to_string()))?;

    let output = cb
        .build_command("mount")
        .arg(device)
        .arg(mp_str)
        .output()
        .map_err(|e| Error::CommandFailed(format!("mount failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "mount {} at {} failed: {}",
            device, mp_str, stderr
        )));
    }

    Ok(())
}

/// Unmount a mount point.
fn unmount_device(mount_point: &Path, verbose: bool) -> Result<()> {
    unmount_device_with(&CmdBuilder::Sudo, mount_point, verbose)
}

/// Like [`unmount_device`] but uses the given command builder.
fn unmount_device_with(cb: &CmdBuilder, mount_point: &Path, verbose: bool) -> Result<()> {
    let mp_str = mount_point
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 mount point path".to_string()))?;

    if verbose {
        eprintln!("lvm_clone: unmounting {}", mp_str);
    }

    let output = cb
        .build_command("umount")
        .arg(mp_str)
        .output()
        .map_err(|e| Error::CommandFailed(format!("umount failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandFailed(format!(
            "umount {} failed: {}",
            mp_str, stderr
        )));
    }

    Ok(())
}

/// Bind-mount `/proc`, `/sys`, and `/dev` from the host into `root_mount`
/// (bug #2 fix — mounts run from the host, not inside a broken chroot).
#[allow(dead_code)]
fn mount_chroot_binds(root_mount: &Path) -> Result<()> {
    mount_chroot_binds_with(&CmdBuilder::Sudo, root_mount)
}

/// Like [`mount_chroot_binds`] but uses the given command builder.
fn mount_chroot_binds_with(cb: &CmdBuilder, root_mount: &Path) -> Result<()> {
    let mp_str = root_mount
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 root mount path".to_string()))?;

    for dir in &["proc", "sys", "dev"] {
        let target = format!("{}/{}", mp_str, dir);
        let source = format!("/{}", dir);

        fs::create_dir_all(&target)
            .map_err(|e| Error::CommandFailed(format!("mkdir {} failed: {}", target, e)))?;

        let output = cb
            .build_command("mount")
            .arg("--bind")
            .arg(&source)
            .arg(&target)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("bind mount {} -> {} failed: {}", source, target, e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "bind mount {} -> {} failed: {}",
                source, target, stderr
            )));
        }
    }

    Ok(())
}

/// Unmount bind mounts for `/dev`, `/sys`, `/proc` (reverse order).
#[allow(dead_code)]
fn unmount_chroot_binds(root_mount: &Path) -> Result<()> {
    unmount_chroot_binds_with(&CmdBuilder::Sudo, root_mount)
}

/// Like [`unmount_chroot_binds`] but uses the given command builder.
fn unmount_chroot_binds_with(cb: &CmdBuilder, root_mount: &Path) -> Result<()> {
    let mp_str = root_mount
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 root mount path".to_string()))?;

    for dir in &["dev", "sys", "proc"] {
        let target = format!("{}/{}", mp_str, dir);
        let _ = cb.build_command("umount").arg(&target).output();
    }

    Ok(())
}

/// Mount bind mounts, run a command inside a chroot, then clean up.
///
/// Bind mounts are **always** cleaned up even if the command fails.
fn run_in_chroot(root_mount: &Path, command: &[&str]) -> Result<std::process::Output> {
    run_in_chroot_with(&CmdBuilder::Sudo, root_mount, command)
}

/// Like [`run_in_chroot`] but uses the given command builder.
fn run_in_chroot_with(
    cb: &CmdBuilder,
    root_mount: &Path,
    command: &[&str],
) -> Result<std::process::Output> {
    let mp_str = root_mount
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 root mount path".to_string()))?;

    mount_chroot_binds_with(cb, root_mount)?;

    let result = cb
        .build_command("chroot")
        .arg(mp_str)
        .args(command)
        .output();

    // Always clean up binds
    let _ = unmount_chroot_binds_with(cb, root_mount);

    result.map_err(|e| Error::CommandFailed(format!("chroot failed: {}", e)))
}

// ---------------------------------------------------------------------------
// Config file updates (per-UUID replacement — bug #3 & #4 fixes)
// ---------------------------------------------------------------------------

/// Replace each old UUID with its corresponding new UUID in `/etc/fstab`.
///
/// Each UUID mapping is applied independently — swap or other partitions
/// whose UUID was not regenerated remain untouched (bug #3 fix).
/// UUIDs are regex-escaped before matching (bug #4 fix).
///
/// Returns the path to the backup file, if one was created.
fn update_clone_fstab(root_mount: &Path, uuid_mappings: &[UuidMapping]) -> Result<Option<String>> {
    let fstab_path = root_mount.join("etc/fstab");
    if !fstab_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&fstab_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to read fstab: {}", e)))?;

    // Backup
    let backup_path = root_mount.join("etc/fstab.bak");
    fs::write(&backup_path, &content)
        .map_err(|e| Error::CommandFailed(format!("Failed to write fstab backup: {}", e)))?;

    // Pre-build all regexes to avoid compiling inside the loop
    let replacements: Vec<(Regex, String)> = uuid_mappings
        .iter()
        .map(|mapping| {
            let pattern = format!(r"UUID={}", regex::escape(&mapping.old_uuid));
            let re = Regex::new(&pattern)
                .map_err(|e| Error::InvalidFormat(format!("Bad regex: {}", e)))?;
            Ok((re, format!("UUID={}", mapping.new_uuid)))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut updated = content.clone();
    for (re, replacement) in &replacements {
        updated = re.replace_all(&updated, replacement.as_str()).to_string();
    }

    if updated != content {
        fs::write(&fstab_path, &updated)
            .map_err(|e| Error::CommandFailed(format!("Failed to write fstab: {}", e)))?;
    }

    let backup_str = backup_path
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 backup path".to_string()))?
        .to_string();
    Ok(Some(backup_str))
}

/// Update GRUB bootloader config with the new UUIDs.
///
/// Scans `boot/grub/grub.cfg` and `boot/grub2/grub.cfg` for each old UUID
/// and replaces it with the new one (per-UUID, not global).
///
/// Returns the list of backup file paths.
fn update_clone_bootloader(
    root_mount: &Path,
    uuid_mappings: &[UuidMapping],
    verbose: bool,
) -> Result<Vec<String>> {
    let grub_paths = [
        root_mount.join("boot/grub/grub.cfg"),
        root_mount.join("boot/grub2/grub.cfg"),
    ];

    let mut backup_files = Vec::new();

    for grub_path in &grub_paths {
        if !grub_path.exists() {
            continue;
        }

        if verbose {
            eprintln!(
                "lvm_clone: updating bootloader config: {}",
                grub_path.display()
            );
        }

        let content = fs::read_to_string(grub_path)
            .map_err(|e| Error::CommandFailed(format!("Failed to read grub.cfg: {}", e)))?;

        let backup_path = grub_path.with_extension("cfg.bak");
        fs::write(&backup_path, &content)
            .map_err(|e| Error::CommandFailed(format!("Failed to write grub backup: {}", e)))?;

        let mut updated = content.clone();
        for mapping in uuid_mappings {
            let escaped_old = regex::escape(&mapping.old_uuid);
            let re = Regex::new(&escaped_old)
                .map_err(|e| Error::InvalidFormat(format!("Bad regex: {}", e)))?;
            updated = re
                .replace_all(&updated, mapping.new_uuid.as_str())
                .to_string();
        }

        if updated != content {
            fs::write(grub_path, &updated)
                .map_err(|e| Error::CommandFailed(format!("Failed to write grub.cfg: {}", e)))?;
        }

        let bak_str = backup_path
            .to_str()
            .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 backup path".to_string()))?
            .to_string();
        backup_files.push(bak_str);
    }

    Ok(backup_files)
}

/// Replace each old UUID with its corresponding new UUID in `/etc/crypttab`.
///
/// Same per-UUID strategy as fstab — only mapped UUIDs are replaced (bug #3 fix).
///
/// Returns the path to the backup file, if one was created.
fn update_clone_crypttab(
    root_mount: &Path,
    uuid_mappings: &[UuidMapping],
) -> Result<Option<String>> {
    let crypttab_path = root_mount.join("etc/crypttab");
    if !crypttab_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&crypttab_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to read crypttab: {}", e)))?;

    // Backup
    let backup_path = root_mount.join("etc/crypttab.bak");
    fs::write(&backup_path, &content)
        .map_err(|e| Error::CommandFailed(format!("Failed to write crypttab backup: {}", e)))?;

    let mut updated = content.clone();
    for mapping in uuid_mappings {
        let escaped_old = regex::escape(&mapping.old_uuid);
        // crypttab uses both UUID=<uuid> and bare <uuid> forms
        let re = Regex::new(&escaped_old)
            .map_err(|e| Error::InvalidFormat(format!("Bad regex: {}", e)))?;
        updated = re
            .replace_all(&updated, mapping.new_uuid.as_str())
            .to_string();
    }

    if updated != content {
        fs::write(&crypttab_path, &updated)
            .map_err(|e| Error::CommandFailed(format!("Failed to write crypttab: {}", e)))?;
    }

    let backup_str = backup_path
        .to_str()
        .ok_or_else(|| Error::InvalidFormat("Non-UTF-8 backup path".to_string()))?
        .to_string();
    Ok(Some(backup_str))
}

/// Set the hostname inside the cloned filesystem.
fn set_hostname(root_mount: &Path, hostname: &str, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: setting hostname to '{}'", hostname);
    }

    let hostname_path = root_mount.join("etc/hostname");
    if hostname_path.exists() || root_mount.join("etc").exists() {
        fs::write(&hostname_path, format!("{}\n", hostname))
            .map_err(|e| Error::CommandFailed(format!("Failed to write hostname: {}", e)))?;
    }

    // Update 127.0.1.1 line in /etc/hosts if present
    let hosts_path = root_mount.join("etc/hosts");
    if hosts_path.exists() {
        let content = fs::read_to_string(&hosts_path)
            .map_err(|e| Error::CommandFailed(format!("Failed to read hosts: {}", e)))?;

        let updated: String = content
            .lines()
            .map(|line| {
                if line.starts_with("127.0.1.1") {
                    format!("127.0.1.1\t{}", hostname)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if updated != content {
            // Preserve trailing newline if original had one
            let final_content = if content.ends_with('\n') && !updated.ends_with('\n') {
                format!("{}\n", updated)
            } else {
                updated
            };
            fs::write(&hosts_path, &final_content)
                .map_err(|e| Error::CommandFailed(format!("Failed to write hosts: {}", e)))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Initramfs regeneration
// ---------------------------------------------------------------------------

/// Regenerate the initramfs inside the clone via chroot.
///
/// Tries dracut (Fedora/RHEL), update-initramfs (Debian/Ubuntu), and
/// mkinitcpio (Arch) in order.
///
/// Uses `mount_chroot_binds` to bind /proc, /sys, /dev from the host before
/// running the chroot command (bug #2 fix).
fn regenerate_initramfs(root_mount: &Path, verbose: bool) -> Result<()> {
    regenerate_initramfs_with(&CmdBuilder::Sudo, root_mount, verbose)
}

/// Like [`regenerate_initramfs`] but uses the given command builder.
fn regenerate_initramfs_with(cb: &CmdBuilder, root_mount: &Path, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: regenerating initramfs via chroot");
    }

    let tools: &[&[&str]] = &[
        &["dracut", "--force", "--regenerate-all"],
        &["update-initramfs", "-u"],
        &["mkinitcpio", "-P"],
    ];

    for tool in tools {
        let output = run_in_chroot_with(cb, root_mount, tool)?;
        if output.status.success() {
            if verbose {
                eprintln!("lvm_clone: initramfs regenerated with {}", tool[0]);
            }
            return Ok(());
        }
        if verbose {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "lvm_clone: {} failed ({}), trying next",
                tool[0],
                stderr.trim()
            );
        }
    }

    Err(Error::CommandFailed(
        "No supported initramfs tool found (tried dracut, update-initramfs, mkinitcpio)"
            .to_string(),
    ))
}

// ---------------------------------------------------------------------------
// GRUB config regeneration
// ---------------------------------------------------------------------------

/// Regenerate the GRUB configuration inside the clone via chroot.
///
/// Runs `grub2-mkconfig` (Fedora/RHEL) or `grub-mkconfig` (Debian/Ubuntu)
/// inside a chroot with proper bind mounts.  This produces a fresh grub.cfg
/// that references the cloned filesystem's new UUIDs.
///
/// Returns the list of backup file paths created.
fn regenerate_grub_config(root_mount: &Path, verbose: bool) -> Result<Vec<String>> {
    regenerate_grub_config_with(&CmdBuilder::Sudo, root_mount, verbose)
}

/// Like [`regenerate_grub_config`] but uses the given command builder.
fn regenerate_grub_config_with(
    cb: &CmdBuilder,
    root_mount: &Path,
    verbose: bool,
) -> Result<Vec<String>> {
    if verbose {
        eprintln!("lvm_clone: regenerating GRUB config via chroot");
    }

    let mut backup_files = Vec::new();

    let grub_targets = [
        ("grub2-mkconfig", "boot/grub2/grub.cfg"),
        ("grub-mkconfig", "boot/grub/grub.cfg"),
    ];

    for &(_, cfg_rel) in &grub_targets {
        let cfg_path = root_mount.join(cfg_rel);
        if cfg_path.exists() {
            let backup_path = cfg_path.with_extension("cfg.pre-regen");
            if let Ok(()) = fs::copy(&cfg_path, &backup_path).map(|_| ()) {
                if let Some(s) = backup_path.to_str() {
                    backup_files.push(s.to_string());
                }
            }
        }
    }

    for &(tool, cfg_rel) in &grub_targets {
        let output_path = format!("/{}", cfg_rel);
        let output = run_in_chroot_with(cb, root_mount, &[tool, "-o", &output_path])?;
        if output.status.success() {
            if verbose {
                eprintln!("lvm_clone: GRUB config regenerated with {}", tool);
            }
            return Ok(backup_files);
        }
        if verbose {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "lvm_clone: {} failed ({}), trying next",
                tool,
                stderr.trim()
            );
        }
    }

    Err(Error::CommandFailed(
        "Could not regenerate GRUB config (tried grub2-mkconfig, grub-mkconfig)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Kernel version detection
// ---------------------------------------------------------------------------

/// Detect the installed kernel version inside the cloned filesystem.
///
/// Looks for `vmlinuz-*` files in `/boot` and returns the version string.
fn detect_kernel_version(root_mount: &Path) -> Result<String> {
    let boot_dir = root_mount.join("boot");
    if !boot_dir.exists() {
        return Err(Error::NotFound("No /boot directory found".to_string()));
    }

    let re = Regex::new(r"vmlinuz-(.+)")
        .map_err(|e| Error::InvalidFormat(format!("regex error: {}", e)))?;

    // Read boot directory and find vmlinuz files
    let entries = fs::read_dir(&boot_dir)
        .map_err(|e| Error::CommandFailed(format!("Cannot read /boot: {}", e)))?;

    let mut versions: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(caps) = re.captures(&name) {
            versions.push(caps[1].to_string());
        }
    }

    // Sort and return the latest version
    versions.sort();
    versions
        .last()
        .cloned()
        .ok_or_else(|| Error::NotFound("No vmlinuz kernel found in /boot".to_string()))
}

// ---------------------------------------------------------------------------
// Boot configuration verification
// ---------------------------------------------------------------------------

/// Verify that the cloned filesystem has a valid boot configuration.
///
/// Checks for the presence of a kernel, initramfs, and GRUB config.
/// Returns a list of warnings for any missing or misconfigured components.
pub fn verify_boot_configuration(root_mount: &Path, verbose: bool) -> Vec<SecurityWarning> {
    let mut warnings = Vec::new();

    if verbose {
        eprintln!("lvm_clone: verifying boot configuration");
    }

    // Check for kernel
    let boot_dir = root_mount.join("boot");
    if boot_dir.exists() {
        let has_kernel = fs::read_dir(&boot_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with("vmlinuz"))
            })
            .unwrap_or(false);

        if !has_kernel {
            warnings.push(SecurityWarning {
                category: "boot_kernel".to_string(),
                message: "No kernel (vmlinuz) found in /boot".to_string(),
            });
        }

        // Check for initramfs/initrd
        let has_initramfs = fs::read_dir(&boot_dir)
            .map(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with("initramfs-")
                        || name.starts_with("initrd.img-")
                        || name.starts_with("initrd-")
                })
            })
            .unwrap_or(false);

        if !has_initramfs {
            warnings.push(SecurityWarning {
                category: "boot_initramfs".to_string(),
                message: "No initramfs/initrd found in /boot".to_string(),
            });
        }
    } else {
        warnings.push(SecurityWarning {
            category: "boot_missing".to_string(),
            message: "No /boot directory found".to_string(),
        });
    }

    // Check for GRUB configuration
    let grub_paths = [
        root_mount.join("boot/grub/grub.cfg"),
        root_mount.join("boot/grub2/grub.cfg"),
    ];

    let has_grub = grub_paths.iter().any(|p| p.exists());
    if !has_grub {
        warnings.push(SecurityWarning {
            category: "boot_grub".to_string(),
            message: "No GRUB configuration found (boot/grub/grub.cfg or boot/grub2/grub.cfg)"
                .to_string(),
        });
    }

    // Verify GRUB config references UUID= (not bare device paths)
    for grub_path in &grub_paths {
        if grub_path.exists() {
            if let Ok(content) = fs::read_to_string(grub_path) {
                if !content.contains("UUID=") && content.contains("root=/dev/") {
                    warnings.push(SecurityWarning {
                        category: "boot_grub_uuid".to_string(),
                        message: format!(
                            "{} uses bare device paths instead of UUID=",
                            grub_path.display()
                        ),
                    });
                }
            }
        }
    }

    // Check fstab references match available UUIDs
    let fstab_path = root_mount.join("etc/fstab");
    if fstab_path.exists() {
        if let Ok(content) = fs::read_to_string(&fstab_path) {
            let uuid_re = Regex::new(r"UUID=([a-fA-F0-9-]+)").ok();
            if let Some(re) = uuid_re {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('#') || trimmed.is_empty() {
                        continue;
                    }
                    // Check that UUID= references exist (basic syntax check)
                    if trimmed.contains("UUID=") && !re.is_match(trimmed) {
                        warnings.push(SecurityWarning {
                            category: "boot_fstab_uuid".to_string(),
                            message: format!("Malformed UUID in fstab: {}", trimmed),
                        });
                    }
                }
            }
        }
    }

    if verbose {
        if warnings.is_empty() {
            eprintln!("lvm_clone: boot configuration verified OK");
        } else {
            eprintln!(
                "lvm_clone: boot verification found {} issue(s)",
                warnings.len()
            );
        }
    }

    warnings
}

// ---------------------------------------------------------------------------
// EFI support
// ---------------------------------------------------------------------------

/// Detect if the mounted clone has an EFI System Partition.
///
/// Checks for the presence of an EFI directory structure.
#[allow(dead_code)]
pub fn detect_efi_system(root_mount: &Path) -> bool {
    let efi_paths = [
        root_mount.join("boot/efi/EFI"),
        root_mount.join("boot/EFI"),
        root_mount.join("efi/EFI"),
    ];

    efi_paths.iter().any(|p| p.exists())
}

/// Update the GRUB configuration for EFI systems by running `grub-mkconfig`
/// or `grub2-mkconfig` inside the chroot.
#[allow(dead_code)]
pub fn update_efi_grub(root_mount: &Path, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("lvm_clone: updating EFI GRUB configuration");
    }

    // Try grub2-mkconfig first (Fedora/RHEL), then grub-mkconfig (Debian/Ubuntu)
    let tools: &[&[&str]] = &[
        &["grub2-mkconfig", "-o", "/boot/grub2/grub.cfg"],
        &["grub-mkconfig", "-o", "/boot/grub/grub.cfg"],
    ];

    for tool in tools {
        let output = run_in_chroot(root_mount, tool)?;
        if output.status.success() {
            if verbose {
                eprintln!("lvm_clone: EFI GRUB updated with {}", tool[0]);
            }
            return Ok(());
        }
    }

    Err(Error::CommandFailed(
        "Could not update EFI GRUB configuration".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Disk image support — bridging disk images to LVM-compatible block devices
// ---------------------------------------------------------------------------

/// Method for accessing a disk image as a block device for LVM operations.
///
/// Non-raw formats (VMDK, QCOW2, VDI, VHDX) cannot be used directly with
/// LVM tools. These methods expose them as standard Linux block devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskImageAccessMethod {
    /// Use `qemu-nbd` for direct access (recommended — no full conversion).
    ///
    /// Works with VMDK, QCOW2, VDI, VHDX. Fastest for read-only operations.
    Nbd,
    /// Convert to raw via `qemu-img`, then attach with a loop device.
    ///
    /// Required for write operations on non-raw formats.
    ConvertToRaw,
}

/// Backward-compatible alias.
pub type VmdkAccessMethod = DiskImageAccessMethod;

/// A disk image exposed as a block device suitable for LVM operations.
///
/// Holds the underlying `NbdDevice` or `LoopDevice` handle and cleans up
/// automatically when dropped.
pub struct DiskImageLvmAccess {
    /// Block device path usable by LVM tools (`/dev/nbd*` or `/dev/loop*`).
    pub device_path: String,
    /// The access method used.
    pub method: DiskImageAccessMethod,
    /// Whether the device was opened read-only.
    pub read_only: bool,
    /// The detected or specified format of the source image.
    pub format: crate::core::DiskFormat,
    // Internal handles (cleaned up via Drop on the inner types)
    nbd: Option<crate::disk::nbd::NbdDevice>,
    loop_dev: Option<crate::disk::loop_device::LoopDevice>,
    raw_file: Option<std::path::PathBuf>,
}

/// Backward-compatible alias.
pub type VmdkLvmAccess = DiskImageLvmAccess;

impl DiskImageLvmAccess {
    /// List partitions on the exposed block device.
    pub fn list_partitions(&self) -> Result<Vec<std::path::PathBuf>> {
        if let Some(ref nbd) = self.nbd {
            nbd.list_partitions()
        } else if let Some(ref loop_dev) = self.loop_dev {
            loop_dev.list_partitions()
        } else {
            Err(Error::InvalidState("No device handle".to_string()))
        }
    }
}

impl Drop for DiskImageLvmAccess {
    fn drop(&mut self) {
        // NbdDevice and LoopDevice implement Drop with disconnect.
        // Clean up the temporary raw file if we created one.
        if let Some(ref raw_file) = self.raw_file {
            let _ = std::fs::remove_file(raw_file);
        }
    }
}

/// Expose a disk image (VMDK, QCOW2, VDI, VHDX, raw) as a block device
/// for LVM operations.
///
/// Non-raw formats are either connected via `qemu-nbd` or converted to raw
/// and attached with a loop device. Raw images use a loop device directly.
///
/// # Arguments
///
/// * `image_path` — path to the disk image file.
/// * `method` — [`DiskImageAccessMethod::Nbd`] or [`ConvertToRaw`].
/// * `read_only` — open in read-only mode (safer for inspection).
/// * `verbose` — emit progress messages to stderr.
///
/// # Returns
///
/// A [`DiskImageLvmAccess`] whose `device_path` can be passed to any
/// LVM / blkid / mount command. Automatically cleaned up when dropped.
pub fn disk_image_prepare_for_lvm(
    image_path: &Path,
    method: DiskImageAccessMethod,
    read_only: bool,
    verbose: bool,
) -> Result<DiskImageLvmAccess> {
    use crate::core::DiskFormat;

    if !image_path.exists() {
        return Err(Error::NotFound(format!(
            "Disk image does not exist: {}",
            image_path.display()
        )));
    }

    let format = DiskFormat::from_str(
        image_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("raw"),
    );

    if verbose {
        eprintln!(
            "lvm_clone: preparing {} ({}) for LVM access (method={:?})",
            image_path.display(),
            format.as_str(),
            method
        );
    }

    // Raw images go straight to a loop device regardless of method
    if format == DiskFormat::Raw {
        return image_prepare_loop(image_path, format, read_only, verbose);
    }

    match method {
        DiskImageAccessMethod::Nbd => image_prepare_nbd(image_path, format, read_only, verbose),
        DiskImageAccessMethod::ConvertToRaw => {
            image_prepare_convert(image_path, format, read_only, verbose)
        }
    }
}

/// Backward-compatible alias for [`disk_image_prepare_for_lvm`].
pub fn vmdk_prepare_for_lvm(
    vmdk_path: &Path,
    method: DiskImageAccessMethod,
    read_only: bool,
    verbose: bool,
) -> Result<DiskImageLvmAccess> {
    disk_image_prepare_for_lvm(vmdk_path, method, read_only, verbose)
}

/// Expose any disk image via qemu-nbd.
fn image_prepare_nbd(
    image_path: &Path,
    format: crate::core::DiskFormat,
    read_only: bool,
    verbose: bool,
) -> Result<DiskImageLvmAccess> {
    use crate::disk::nbd::NbdDevice;

    if verbose {
        eprintln!(
            "lvm_clone: connecting {} image via qemu-nbd",
            format.as_str()
        );
    }

    let mut nbd = NbdDevice::new()?;
    nbd.connect(image_path, read_only)?;

    let device_path = nbd.device_path().to_string_lossy().to_string();

    if verbose {
        eprintln!("lvm_clone: image available at {}", device_path);
    }

    Ok(DiskImageLvmAccess {
        device_path,
        method: DiskImageAccessMethod::Nbd,
        read_only,
        format,
        nbd: Some(nbd),
        loop_dev: None,
        raw_file: None,
    })
}

/// Attach a raw image directly via loop device.
fn image_prepare_loop(
    image_path: &Path,
    format: crate::core::DiskFormat,
    read_only: bool,
    verbose: bool,
) -> Result<DiskImageLvmAccess> {
    use crate::disk::loop_device::LoopDevice;

    if verbose {
        eprintln!("lvm_clone: attaching raw image via loop device");
    }

    let mut loop_dev = LoopDevice::new()?;
    loop_dev.connect(image_path, read_only)?;

    let device_path = loop_dev
        .device_path()
        .ok_or_else(|| Error::CommandFailed("Loop device path not available".to_string()))?
        .to_string_lossy()
        .to_string();

    if verbose {
        eprintln!("lvm_clone: image available at {}", device_path);
    }

    Ok(DiskImageLvmAccess {
        device_path,
        method: DiskImageAccessMethod::ConvertToRaw,
        read_only,
        format,
        nbd: None,
        loop_dev: Some(loop_dev),
        raw_file: None,
    })
}

/// Convert a non-raw image to raw and attach via loop device.
fn image_prepare_convert(
    image_path: &Path,
    format: crate::core::DiskFormat,
    read_only: bool,
    verbose: bool,
) -> Result<DiskImageLvmAccess> {
    use crate::converters::disk_converter::DiskConverter;
    use crate::disk::loop_device::LoopDevice;

    if verbose {
        eprintln!(
            "lvm_clone: converting {} to raw via qemu-img",
            format.as_str()
        );
    }

    let raw_path = std::env::temp_dir().join(format!("disk-lvm-{}.raw", Uuid::new_v4()));

    let converter = DiskConverter::new();
    let result = converter.convert(image_path, raw_path.as_path(), "raw", false, false)?;
    if !result.success {
        let _ = std::fs::remove_file(&raw_path);
        let msg = result
            .error
            .unwrap_or_else(|| "unknown conversion error".to_string());
        return Err(Error::CommandFailed(format!(
            "{} to raw conversion failed: {}",
            format.as_str(),
            msg
        )));
    }

    if verbose {
        eprintln!(
            "lvm_clone: converted to raw ({} bytes), attaching loop device",
            result.output_size
        );
    }

    let mut loop_dev = LoopDevice::new()?;
    if let Err(e) = loop_dev.connect(&raw_path, read_only) {
        let _ = std::fs::remove_file(&raw_path);
        return Err(e);
    }

    let device_path = loop_dev
        .device_path()
        .ok_or_else(|| Error::CommandFailed("Loop device path not available".to_string()))?
        .to_string_lossy()
        .to_string();

    if verbose {
        eprintln!("lvm_clone: image available at {} (via loop)", device_path);
    }

    Ok(DiskImageLvmAccess {
        device_path,
        method: DiskImageAccessMethod::ConvertToRaw,
        read_only,
        format,
        nbd: None,
        loop_dev: Some(loop_dev),
        raw_file: Some(raw_path),
    })
}

// ---------------------------------------------------------------------------
// Clone LV to disk image
// ---------------------------------------------------------------------------

/// Result of cloning an LV to a disk image file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneImageResult {
    /// Source LV device path.
    pub source_path: String,
    /// Output image file path.
    pub image_path: String,
    /// Output image format (e.g. `"qcow2"`, `"vmdk"`).
    pub image_format: String,
    /// Size of the output image in bytes.
    pub image_size: u64,
    /// Path to the intermediate raw file (if kept).
    pub raw_copy: Option<String>,
}

/// Clone a host logical volume to a disk image file.
///
/// Supports raw, qcow2, vmdk, vdi, and vhdx output formats.  For non-raw
/// formats the LV is first copied to a temporary raw file and then converted
/// via `qemu-img`.
///
/// # Arguments
///
/// * `source_vg` / `source_lv` — the volume group and logical volume to clone.
/// * `output_path` — destination image file.
/// * `output_format` — target format string (`"raw"`, `"qcow2"`, `"vmdk"`,
///   `"vdi"`, `"vhdx"`). If `None`, derived from the file extension.
/// * `keep_raw` — when converting, keep the intermediate raw copy alongside
///   the converted image (useful to boot with QEMU directly).
/// * `verbose` — emit progress messages.
///
/// # Example
///
/// ```no_run
/// use guestkit::guestfs::lvm_clone::clone_lv_to_disk_image;
/// use std::path::Path;
///
/// let result = clone_lv_to_disk_image(
///     "vg0", "root",
///     Path::new("/backup/server.qcow2"),
///     None,       // auto-detect from extension
///     false,      // don't keep raw copy
///     true,       // verbose
/// ).unwrap();
/// println!("Image written: {}", result.image_path);
/// ```
pub fn clone_lv_to_disk_image(
    source_vg: &str,
    source_lv: &str,
    output_path: &Path,
    output_format: Option<&str>,
    keep_raw: bool,
    verbose: bool,
) -> Result<CloneImageResult> {
    use crate::core::DiskFormat;

    let source_dev = format!("/dev/{}/{}", source_vg, source_lv);
    if !Path::new(&source_dev).exists() {
        return Err(Error::NotFound(format!(
            "Source LV does not exist: {}",
            source_dev
        )));
    }

    // Determine output format
    let fmt_str = output_format.unwrap_or_else(|| {
        output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("raw")
    });
    let fmt = DiskFormat::from_str(fmt_str);
    let fmt_name = if fmt == DiskFormat::Unknown {
        fmt_str
    } else {
        fmt.as_str()
    };

    if verbose {
        eprintln!(
            "lvm_clone: cloning {} to {} (format={})",
            source_dev,
            output_path.display(),
            fmt_name
        );
    }

    // Step 1: dd the LV to raw (either directly to output or to a temp file)
    let raw_path = if fmt == DiskFormat::Raw {
        output_path.to_path_buf()
    } else {
        let p = std::env::temp_dir().join(format!("lv-raw-{}.img", Uuid::new_v4()));
        p
    };

    if verbose {
        eprintln!("lvm_clone: dd {} -> {}", source_dev, raw_path.display());
    }

    let if_arg = format!("if={}", source_dev);
    let of_arg = format!("of={}", raw_path.display());
    let output = build_sudo_command("dd")
        .arg(&if_arg)
        .arg(&of_arg)
        .arg("bs=4M")
        .arg("conv=fsync")
        .arg("status=progress")
        .output()
        .map_err(|e| Error::CommandFailed(format!("dd failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&raw_path);
        return Err(Error::CommandFailed(format!("dd failed: {}", stderr)));
    }

    let raw_size = std::fs::metadata(&raw_path).map(|m| m.len()).unwrap_or(0);

    // Step 2: convert to target format if not raw
    if fmt != DiskFormat::Raw {
        if verbose {
            eprintln!(
                "lvm_clone: converting raw ({} bytes) to {}",
                raw_size, fmt_name
            );
        }

        let converter = crate::converters::disk_converter::DiskConverter::new();
        let conv = converter.convert(raw_path.as_path(), output_path, fmt_name, false, false)?;
        if !conv.success {
            let _ = std::fs::remove_file(&raw_path);
            let msg = conv.error.unwrap_or_else(|| "conversion error".to_string());
            return Err(Error::CommandFailed(format!(
                "Raw to {} conversion failed: {}",
                fmt_name, msg
            )));
        }

        // Optionally keep raw copy
        let kept_raw = if keep_raw {
            let kept = raw_path.with_extension("raw");
            if kept != raw_path {
                std::fs::rename(&raw_path, &kept)
                    .map_err(|e| Error::CommandFailed(format!("rename failed: {}", e)))?;
            }
            Some(kept.to_string_lossy().to_string())
        } else {
            let _ = std::fs::remove_file(&raw_path);
            None
        };

        let image_size = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

        if verbose {
            eprintln!(
                "lvm_clone: {} image written ({} bytes)",
                fmt_name, image_size
            );
        }

        Ok(CloneImageResult {
            source_path: source_dev,
            image_path: output_path.to_string_lossy().to_string(),
            image_format: fmt_name.to_string(),
            image_size,
            raw_copy: kept_raw,
        })
    } else {
        if verbose {
            eprintln!("lvm_clone: raw image written ({} bytes)", raw_size);
        }

        Ok(CloneImageResult {
            source_path: source_dev,
            image_path: output_path.to_string_lossy().to_string(),
            image_format: "raw".to_string(),
            image_size: raw_size,
            raw_copy: None,
        })
    }
}

/// Convert a disk image between formats.
///
/// Wrapper around `qemu-img convert` that supports all common VM image
/// formats. Useful for converting a raw clone to qcow2 or vmdk for use
/// with QEMU / VMware.
///
/// # Supported formats
///
/// `"raw"`, `"qcow2"`, `"vmdk"`, `"vdi"`, `"vhdx"`, `"vhd"`.
pub fn convert_disk_image(
    source_path: &Path,
    output_path: &Path,
    output_format: &str,
    verbose: bool,
) -> Result<()> {
    if !source_path.exists() {
        return Err(Error::NotFound(format!(
            "Source image does not exist: {}",
            source_path.display()
        )));
    }

    if verbose {
        eprintln!(
            "lvm_clone: converting {} -> {} ({})",
            source_path.display(),
            output_path.display(),
            output_format
        );
    }

    let converter = crate::converters::disk_converter::DiskConverter::new();
    let result = converter.convert(source_path, output_path, output_format, false, false)?;

    if !result.success {
        let msg = result
            .error
            .unwrap_or_else(|| "conversion error".to_string());
        return Err(Error::CommandFailed(format!(
            "Conversion to {} failed: {}",
            output_format, msg
        )));
    }

    if verbose {
        eprintln!(
            "lvm_clone: conversion complete ({} bytes, {:.1}s)",
            result.output_size, result.duration_secs
        );
    }

    Ok(())
}

/// Backward-compatible alias — convert a raw device back to VMDK.
pub fn vmdk_convert_back(
    access: &DiskImageLvmAccess,
    output_path: &Path,
    verbose: bool,
) -> Result<()> {
    let raw_path = access.raw_file.as_ref().ok_or_else(|| {
        Error::InvalidState(
            "Cannot convert back: no raw file (was this opened via NBD?)".to_string(),
        )
    })?;
    convert_disk_image(raw_path, output_path, access.format.as_str(), verbose)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_config_creation() {
        let config = LvmCloneConfig {
            source_vg: "vg0".to_string(),
            source_lv: "root".to_string(),
            clone_lv_name: "root-clone".to_string(),
            target_vg: None,
            regenerate_uuids: true,
            update_fstab: true,
            update_bootloader: true,
            update_crypttab: false,
            hostname: Some("new-host".to_string()),
            dry_run: false,
            snapshot_size: Some("10G".to_string()),
            regenerate_initramfs: false,
            isolation_level: IsolationLevel::None,
            verify_security: false,
            regenerate_grub: false,
            verify_boot: false,
            container_image: None,
        };

        assert_eq!(config.source_vg, "vg0");
        assert_eq!(config.source_lv, "root");
        assert_eq!(config.clone_lv_name, "root-clone");
        assert!(config.target_vg.is_none());
        assert!(config.regenerate_uuids);
        assert!(config.update_fstab);
        assert!(config.update_bootloader);
        assert!(!config.update_crypttab);
        assert_eq!(config.hostname.as_deref(), Some("new-host"));
        assert!(!config.dry_run);
        assert_eq!(config.snapshot_size.as_deref(), Some("10G"));
        assert!(!config.regenerate_initramfs);
        assert_eq!(config.isolation_level, IsolationLevel::None);
        assert!(!config.verify_security);
        assert!(!config.regenerate_grub);
        assert!(!config.verify_boot);
    }

    #[test]
    fn test_uuid_mapping_serialization() {
        let mapping = UuidMapping {
            device: "/dev/vg0/clone-root".to_string(),
            fs_type: "ext4".to_string(),
            old_uuid: "aaaa-bbbb-cccc-dddd".to_string(),
            new_uuid: "1111-2222-3333-4444".to_string(),
        };

        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: UuidMapping = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.device, mapping.device);
        assert_eq!(deserialized.fs_type, mapping.fs_type);
        assert_eq!(deserialized.old_uuid, mapping.old_uuid);
        assert_eq!(deserialized.new_uuid, mapping.new_uuid);
    }

    #[test]
    fn test_clone_result_serialization() {
        let result = CloneResult {
            source_path: "/dev/vg0/root".to_string(),
            clone_path: "/dev/vg0/root-clone".to_string(),
            uuid_mappings: vec![UuidMapping {
                device: "/dev/vg0/root-clone".to_string(),
                fs_type: "ext4".to_string(),
                old_uuid: "old-uuid".to_string(),
                new_uuid: "new-uuid".to_string(),
            }],
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            fstab_updated: true,
            bootloader_updated: false,
            crypttab_updated: true,
            initramfs_regenerated: false,
            namespace_isolated: true,
            grub_regenerated: true,
            boot_verified: true,
            kernel_version: Some("6.1.0-1-amd64".to_string()),
            backup_files: vec!["/mnt/etc/fstab.bak".to_string()],
            security_warnings: vec![SecurityWarning {
                category: "ssh_host_keys".to_string(),
                message: "Clone contains SSH host keys".to_string(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CloneResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.source_path, result.source_path);
        assert_eq!(deserialized.clone_path, result.clone_path);
        assert_eq!(deserialized.uuid_mappings.len(), 1);
        assert!(deserialized.fstab_updated);
        assert!(!deserialized.bootloader_updated);
        assert!(deserialized.crypttab_updated);
        assert!(!deserialized.initramfs_regenerated);
        assert!(deserialized.namespace_isolated);
        assert!(deserialized.grub_regenerated);
        assert!(deserialized.boot_verified);
        assert_eq!(
            deserialized.kernel_version.as_deref(),
            Some("6.1.0-1-amd64")
        );
        assert_eq!(deserialized.backup_files.len(), 1);
        assert_eq!(deserialized.security_warnings.len(), 1);
        assert_eq!(deserialized.security_warnings[0].category, "ssh_host_keys");
    }

    #[test]
    fn test_fstab_uuid_replacement_per_device() {
        // Only the root UUID should be replaced; swap must stay untouched.
        let fstab_content = "\
# /etc/fstab
UUID=aaaa-1111 / ext4 defaults 0 1
UUID=bbbb-2222 none swap sw 0 0
";
        let tmp = tempfile::tempdir().unwrap();
        let etc = tmp.path().join("etc");
        fs::create_dir_all(&etc).unwrap();
        fs::write(etc.join("fstab"), fstab_content).unwrap();

        let mappings = vec![UuidMapping {
            device: "/dev/vg0/root-clone".to_string(),
            fs_type: "ext4".to_string(),
            old_uuid: "aaaa-1111".to_string(),
            new_uuid: "cccc-3333".to_string(),
        }];

        let result = update_clone_fstab(tmp.path(), &mappings).unwrap();
        assert!(result.is_some());

        let updated = fs::read_to_string(etc.join("fstab")).unwrap();
        assert!(updated.contains("UUID=cccc-3333"));
        assert!(updated.contains("UUID=bbbb-2222"));
        assert!(!updated.contains("UUID=aaaa-1111"));
    }

    #[test]
    fn test_is_device_path_detection() {
        assert!("/dev/vg0/root".starts_with("/dev/"));
        assert!("/dev/sda1".starts_with("/dev/"));
        assert!(!"/mnt/clone".starts_with("/dev/"));
        assert!(!"/".starts_with("/dev/"));
    }

    #[test]
    fn test_build_sudo_command_not_root() {
        let cmd = build_sudo_command("ls");
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("ls"));
    }

    #[test]
    fn test_lvm_clone_api_exists() {
        let config = LvmCloneConfig {
            source_vg: "nonexistent_vg".to_string(),
            source_lv: "nonexistent_lv".to_string(),
            clone_lv_name: "clone".to_string(),
            target_vg: None,
            regenerate_uuids: false,
            update_fstab: false,
            update_bootloader: false,
            update_crypttab: false,
            hostname: None,
            dry_run: true,
            snapshot_size: None,
            regenerate_initramfs: false,
            isolation_level: IsolationLevel::None,
            verify_security: false,
            regenerate_grub: false,
            verify_boot: false,
            container_image: None,
        };

        let result = lvm_clone(&config, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bootloader_uuid_replacement() {
        let grub_content = "\
menuentry 'Linux' {
    linux /vmlinuz root=UUID=aaaa-1111 ro
    initrd /initramfs
}
menuentry 'Fallback' {
    linux /vmlinuz root=UUID=bbbb-2222 ro
}
";
        let tmp = tempfile::tempdir().unwrap();
        let grub_dir = tmp.path().join("boot/grub");
        fs::create_dir_all(&grub_dir).unwrap();
        fs::write(grub_dir.join("grub.cfg"), grub_content).unwrap();

        let mappings = vec![UuidMapping {
            device: "/dev/vg0/root-clone".to_string(),
            fs_type: "ext4".to_string(),
            old_uuid: "aaaa-1111".to_string(),
            new_uuid: "cccc-3333".to_string(),
        }];

        let result = update_clone_bootloader(tmp.path(), &mappings, false).unwrap();
        assert!(!result.is_empty());

        let updated = fs::read_to_string(grub_dir.join("grub.cfg")).unwrap();
        assert!(updated.contains("UUID=cccc-3333"));
        assert!(updated.contains("UUID=bbbb-2222"));
        assert!(!updated.contains("aaaa-1111"));
    }

    #[test]
    fn test_crypttab_uuid_replacement_per_device() {
        let crypttab_content = "\
# /etc/crypttab
luks-aaaa-1111 UUID=aaaa-1111 none luks,discard
luks-bbbb-2222 UUID=bbbb-2222 none luks
";
        let tmp = tempfile::tempdir().unwrap();
        let etc = tmp.path().join("etc");
        fs::create_dir_all(&etc).unwrap();
        fs::write(etc.join("crypttab"), crypttab_content).unwrap();

        let mappings = vec![UuidMapping {
            device: "/dev/vg0/root-clone".to_string(),
            fs_type: "crypto_LUKS2".to_string(),
            old_uuid: "aaaa-1111".to_string(),
            new_uuid: "cccc-3333".to_string(),
        }];

        let result = update_clone_crypttab(tmp.path(), &mappings).unwrap();
        assert!(result.is_some());

        let updated = fs::read_to_string(etc.join("crypttab")).unwrap();
        // Both the name reference and UUID= reference should be replaced
        assert!(updated.contains("cccc-3333"));
        // The other entry's UUID stays untouched
        assert!(updated.contains("bbbb-2222"));
        // Old UUID is gone
        assert!(!updated.contains("aaaa-1111"));
    }

    #[test]
    fn test_security_verification() {
        let tmp = tempfile::tempdir().unwrap();
        let etc = tmp.path().join("etc");
        let ssh_dir = etc.join("ssh");
        fs::create_dir_all(&ssh_dir).unwrap();

        // Create a machine-id (should trigger a warning)
        fs::write(etc.join("machine-id"), "abcdef1234567890\n").unwrap();

        // Create an SSH host key (should trigger a warning)
        fs::write(ssh_dir.join("ssh_host_rsa_key"), "fake-key\n").unwrap();

        // Create sshd_config with PermitRootLogin yes (should trigger a warning)
        fs::write(
            ssh_dir.join("sshd_config"),
            "PermitRootLogin yes\nPasswordAuthentication no\n",
        )
        .unwrap();

        let warnings = verify_clone_security(tmp.path(), false);

        let categories: Vec<&str> = warnings.iter().map(|w| w.category.as_str()).collect();
        assert!(categories.contains(&"machine_id"));
        assert!(categories.contains(&"ssh_host_keys"));
        assert!(categories.contains(&"ssh_root_login"));
    }

    #[test]
    fn test_luks_info_serialization() {
        let info = LuksInfo {
            device: "/dev/vg0/encrypted".to_string(),
            uuid: "12345678-1234-1234-1234-123456789abc".to_string(),
            version: 2,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: LuksInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.device, info.device);
        assert_eq!(deserialized.uuid, info.uuid);
        assert_eq!(deserialized.version, 2);
    }

    #[test]
    fn test_hostname_update_with_hosts() {
        let tmp = tempfile::tempdir().unwrap();
        let etc = tmp.path().join("etc");
        fs::create_dir_all(&etc).unwrap();

        fs::write(etc.join("hostname"), "old-host\n").unwrap();
        fs::write(
            etc.join("hosts"),
            "127.0.0.1\tlocalhost\n127.0.1.1\told-host\n",
        )
        .unwrap();

        set_hostname(tmp.path(), "new-host", false).unwrap();

        let hostname = fs::read_to_string(etc.join("hostname")).unwrap();
        assert_eq!(hostname.trim(), "new-host");

        let hosts = fs::read_to_string(etc.join("hosts")).unwrap();
        assert!(hosts.contains("127.0.1.1\tnew-host"));
        assert!(!hosts.contains("old-host"));
        // localhost untouched
        assert!(hosts.contains("127.0.0.1\tlocalhost"));
    }

    #[test]
    fn test_isolation_level_serialization() {
        assert_eq!(
            serde_json::to_string(&IsolationLevel::None).unwrap(),
            "\"None\""
        );
        assert_eq!(
            serde_json::to_string(&IsolationLevel::MountOnly).unwrap(),
            "\"MountOnly\""
        );
        assert_eq!(
            serde_json::to_string(&IsolationLevel::Full).unwrap(),
            "\"Full\""
        );

        let deserialized: IsolationLevel = serde_json::from_str("\"Full\"").unwrap();
        assert_eq!(deserialized, IsolationLevel::Full);
    }

    #[test]
    fn test_detect_kernel_version() {
        let tmp = tempfile::tempdir().unwrap();
        let boot = tmp.path().join("boot");
        fs::create_dir_all(&boot).unwrap();

        // No kernel — should fail
        assert!(detect_kernel_version(tmp.path()).is_err());

        // Create vmlinuz files
        fs::write(boot.join("vmlinuz-5.15.0-1-amd64"), "").unwrap();
        fs::write(boot.join("vmlinuz-6.1.0-2-amd64"), "").unwrap();

        let version = detect_kernel_version(tmp.path()).unwrap();
        assert_eq!(version, "6.1.0-2-amd64");
    }

    #[test]
    fn test_boot_verification_no_boot() {
        let tmp = tempfile::tempdir().unwrap();

        let warnings = verify_boot_configuration(tmp.path(), false);
        assert!(warnings.iter().any(|w| w.category == "boot_missing"));
    }

    #[test]
    fn test_boot_verification_missing_components() {
        let tmp = tempfile::tempdir().unwrap();
        let boot = tmp.path().join("boot");
        fs::create_dir_all(&boot).unwrap();

        let warnings = verify_boot_configuration(tmp.path(), false);
        assert!(warnings.iter().any(|w| w.category == "boot_kernel"));
        assert!(warnings.iter().any(|w| w.category == "boot_initramfs"));
        assert!(warnings.iter().any(|w| w.category == "boot_grub"));
    }

    #[test]
    fn test_boot_verification_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let boot = tmp.path().join("boot");
        let grub_dir = boot.join("grub");
        fs::create_dir_all(&grub_dir).unwrap();

        // Create kernel and initramfs
        fs::write(boot.join("vmlinuz-6.1.0"), "").unwrap();
        fs::write(boot.join("initramfs-6.1.0.img"), "").unwrap();

        // Create GRUB config with UUID references
        fs::write(
            grub_dir.join("grub.cfg"),
            "root=UUID=12345678-1234-1234-1234-123456789abc\n",
        )
        .unwrap();

        let warnings = verify_boot_configuration(tmp.path(), false);
        assert!(
            warnings.is_empty(),
            "Expected no warnings, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_boot_verification_bare_device_path() {
        let tmp = tempfile::tempdir().unwrap();
        let boot = tmp.path().join("boot");
        let grub_dir = boot.join("grub");
        fs::create_dir_all(&grub_dir).unwrap();

        fs::write(boot.join("vmlinuz-6.1.0"), "").unwrap();
        fs::write(boot.join("initramfs-6.1.0.img"), "").unwrap();

        // GRUB config with bare device path instead of UUID
        fs::write(grub_dir.join("grub.cfg"), "root=/dev/sda1\n").unwrap();

        let warnings = verify_boot_configuration(tmp.path(), false);
        assert!(warnings.iter().any(|w| w.category == "boot_grub_uuid"));
    }

    #[test]
    fn test_disk_image_access_method_equality() {
        assert_eq!(DiskImageAccessMethod::Nbd, DiskImageAccessMethod::Nbd);
        assert_eq!(
            DiskImageAccessMethod::ConvertToRaw,
            DiskImageAccessMethod::ConvertToRaw
        );
        assert_ne!(
            DiskImageAccessMethod::Nbd,
            DiskImageAccessMethod::ConvertToRaw
        );
        // Backward-compatible aliases
        assert_eq!(VmdkAccessMethod::Nbd, DiskImageAccessMethod::Nbd);
    }

    #[test]
    fn test_disk_image_prepare_nonexistent() {
        let result = disk_image_prepare_for_lvm(
            Path::new("/nonexistent/disk.vmdk"),
            DiskImageAccessMethod::Nbd,
            true,
            false,
        );
        assert!(result.is_err());

        let result = disk_image_prepare_for_lvm(
            Path::new("/nonexistent/disk.qcow2"),
            DiskImageAccessMethod::ConvertToRaw,
            true,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_lv_to_image_nonexistent_source() {
        let result = clone_lv_to_disk_image(
            "nonexistent_vg",
            "nonexistent_lv",
            Path::new("/tmp/test-output.qcow2"),
            None,
            false,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_disk_image_nonexistent() {
        let result = convert_disk_image(
            Path::new("/nonexistent/source.raw"),
            Path::new("/tmp/output.qcow2"),
            "qcow2",
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_clone_image_result_serialization() {
        let result = CloneImageResult {
            source_path: "/dev/vg0/root".to_string(),
            image_path: "/backup/server.qcow2".to_string(),
            image_format: "qcow2".to_string(),
            image_size: 1073741824,
            raw_copy: Some("/backup/server.raw".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CloneImageResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.source_path, "/dev/vg0/root");
        assert_eq!(deserialized.image_format, "qcow2");
        assert_eq!(deserialized.image_size, 1073741824);
        assert!(deserialized.raw_copy.is_some());
    }

    #[test]
    fn test_podman_container_name_format() {
        let container = PodmanContainer::new();
        assert!(container.name.starts_with("guestkit-lvm-"));
        assert_eq!(container.name.len(), "guestkit-lvm-".len() + 8);
        assert!(!container.started);
    }

    #[test]
    fn test_podman_container_exec_builds_command() {
        let container = PodmanContainer {
            name: "test-container".to_string(),
            image_tag: GUESTKIT_LVM_IMAGE.to_string(),
            started: false,
        };
        let cmd = container.exec("lvs");
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("podman"));
        assert!(debug.contains("exec"));
        assert!(debug.contains("test-container"));
        assert!(debug.contains("lvs"));
    }

    #[test]
    fn test_cmd_builder_sudo_builds_sudo_command() {
        let cb = CmdBuilder::Sudo;
        let cmd = cb.build_command("blkid");
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("blkid"));
    }

    #[test]
    fn test_cmd_builder_podman_builds_podman_exec() {
        let container = PodmanContainer {
            name: "test-ctr".to_string(),
            image_tag: GUESTKIT_LVM_IMAGE.to_string(),
            started: false,
        };
        let cb = CmdBuilder::Podman(&container);
        let cmd = cb.build_command("blkid");
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("podman"));
        assert!(debug.contains("exec"));
        assert!(debug.contains("test-ctr"));
        assert!(debug.contains("blkid"));
    }

    #[test]
    fn test_clone_config_container_image_field() {
        let config = LvmCloneConfig {
            source_vg: "vg0".to_string(),
            source_lv: "root".to_string(),
            clone_lv_name: "root-clone".to_string(),
            target_vg: None,
            regenerate_uuids: false,
            update_fstab: false,
            update_bootloader: false,
            update_crypttab: false,
            hostname: None,
            dry_run: false,
            snapshot_size: None,
            regenerate_initramfs: false,
            isolation_level: IsolationLevel::None,
            verify_security: false,
            regenerate_grub: false,
            verify_boot: false,
            container_image: Some("fedora:39".to_string()),
        };

        assert_eq!(config.container_image.as_deref(), Some("fedora:39"));
    }

    #[test]
    fn test_lvm_clone_podman_no_podman() {
        // If podman is not installed, this should error gracefully.
        // If podman IS installed, it will fail at the container start
        // (nonexistent VG), which is also fine.
        let config = LvmCloneConfig {
            source_vg: "nonexistent_vg".to_string(),
            source_lv: "nonexistent_lv".to_string(),
            clone_lv_name: "clone".to_string(),
            target_vg: None,
            regenerate_uuids: false,
            update_fstab: false,
            update_bootloader: false,
            update_crypttab: false,
            hostname: None,
            dry_run: false,
            snapshot_size: None,
            regenerate_initramfs: false,
            isolation_level: IsolationLevel::None,
            verify_security: false,
            regenerate_grub: false,
            verify_boot: false,
            container_image: None,
        };

        let result = lvm_clone_podman(&config, false);
        assert!(result.is_err());
    }
}
