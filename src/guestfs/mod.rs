// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pure Rust implementation of GuestFS-compatible API
//!
//! This module provides a GuestFS-compatible API implemented entirely in Rust,
//! allowing disk image inspection and manipulation .

pub mod acl_ops;
pub mod archive;
pub mod attr_ops;
pub mod backup_ops;
pub mod base64_ops;
pub mod bcache_ops;
pub mod blockdev_ops;
pub mod boot;
pub mod btrfs;
pub mod cap_ops;
pub mod checksum;
pub mod command;
pub mod compress_ops;
pub mod cpio_ops;
pub mod dd_ops;
pub mod device;
pub mod device_inventory;
pub mod diagnostics;
pub mod disk_mgmt;
pub mod disk_ops;
pub mod dosfs_ops;
pub mod ext_ops;
pub mod f2fs_ops;
pub mod file_ops;
pub mod filesystem;
pub mod forensic;
pub mod fstab;
pub mod fstab_rewriter;
pub mod glob_ops;
pub mod grub_ops;
pub mod grub_repair;
pub mod handle;
#[cfg(feature = "registry-write")]
pub mod hivex_ffi;
pub mod hivex_ops;
pub mod inotify_ops;
pub mod inspect;
pub mod inspect_enhanced;
pub mod inspect_ext_ops;
pub mod internal;
pub mod iso;
pub mod jfs_ops;
pub mod journal_ops;
pub mod label_ops;
pub mod ldm_ops;
pub mod link_ops;
pub mod luks;
pub mod lvm;
pub mod lvm_clone;
pub mod md_ops;
pub mod metadata;
pub mod minix_ops;
pub mod misc;
pub mod mount;
pub mod mpath_ops;
pub mod network;
pub mod nilfs_ops;
pub mod node_ops;
pub mod ntfs;
pub mod owner_ops;
pub mod package;
pub mod package_install;
pub mod part_mgmt;
pub mod part_type_ops;
pub mod partition;
pub mod pread_ops;
pub mod reiserfs_ops;
pub mod rsync_ops;
pub mod sam_aes;
#[cfg(feature = "registry-write")]
pub mod sam_password;
pub mod security;
pub mod security_utils;
pub mod sed_ops;
pub mod selinux_ops;
pub mod service;
pub mod shrink;
pub mod smart_ops;
pub mod squashfs_ops;
pub mod ssh;
pub mod swap_ops;
pub mod sync_ops;
pub mod syslinux_ops;
pub mod sysprep_ops;
pub mod system;
pub mod system_info;
pub mod template_ops;
pub mod time_ops;
pub mod transfer;
pub mod tsk_ops;
pub mod ufs_ops;
pub mod util_ops;
pub mod utils;
pub mod validation;
pub mod virt_ops;
pub mod windows;
pub mod windows_registry;
pub mod xfs;
pub mod yara_ops;
pub mod zfs_ops;

// Ergonomic API extensions
pub mod builder;
pub mod types;

pub use handle::Guestfs;
pub use inspect::*;
pub use inspect_enhanced::*;
pub use metadata::Stat;

// Re-export type-safe types for convenience
pub use builder::GuestfsBuilder;
pub use types::{Distro, FilesystemType, MountOpts, OsType, PackageManager, PartitionTableType};
