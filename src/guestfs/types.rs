// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Type-safe enums for filesystem and partition operations

use std::fmt;

/// Filesystem type for mkfs operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilesystemType {
    /// ext2 filesystem
    Ext2,
    /// ext3 filesystem
    Ext3,
    /// ext4 filesystem (default for most Linux distros)
    Ext4,
    /// XFS filesystem
    Xfs,
    /// BTRFS filesystem
    Btrfs,
    /// VFAT/FAT32 filesystem
    Vfat,
    /// NTFS filesystem
    Ntfs,
    /// exFAT filesystem
    Exfat,
    /// F2FS flash filesystem
    F2fs,
    /// JFS filesystem
    Jfs,
    /// ReiserFS
    Reiserfs,
    /// MINIX filesystem
    Minix,
}

impl FilesystemType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            FilesystemType::Ext2 => "ext2",
            FilesystemType::Ext3 => "ext3",
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Xfs => "xfs",
            FilesystemType::Btrfs => "btrfs",
            FilesystemType::Vfat => "vfat",
            FilesystemType::Ntfs => "ntfs",
            FilesystemType::Exfat => "exfat",
            FilesystemType::F2fs => "f2fs",
            FilesystemType::Jfs => "jfs",
            FilesystemType::Reiserfs => "reiserfs",
            FilesystemType::Minix => "minix",
        }
    }

    /// Parse from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ext2" => Some(FilesystemType::Ext2),
            "ext3" => Some(FilesystemType::Ext3),
            "ext4" => Some(FilesystemType::Ext4),
            "xfs" => Some(FilesystemType::Xfs),
            "btrfs" => Some(FilesystemType::Btrfs),
            "vfat" | "fat32" | "fat" => Some(FilesystemType::Vfat),
            "ntfs" => Some(FilesystemType::Ntfs),
            "exfat" => Some(FilesystemType::Exfat),
            "f2fs" => Some(FilesystemType::F2fs),
            "jfs" => Some(FilesystemType::Jfs),
            "reiserfs" => Some(FilesystemType::Reiserfs),
            "minix" => Some(FilesystemType::Minix),
            _ => None,
        }
    }

    /// Check if this filesystem supports labels
    pub fn supports_labels(&self) -> bool {
        !matches!(self, FilesystemType::Vfat)
    }

    /// Check if this filesystem supports UUIDs
    pub fn supports_uuid(&self) -> bool {
        matches!(
            self,
            FilesystemType::Ext2
                | FilesystemType::Ext3
                | FilesystemType::Ext4
                | FilesystemType::Xfs
                | FilesystemType::Btrfs
        )
    }
}

impl fmt::Display for FilesystemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Partition table type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTableType {
    /// MBR (Master Boot Record) / DOS partition table
    Mbr,
    /// GPT (GUID Partition Table)
    Gpt,
}

impl PartitionTableType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionTableType::Mbr => "mbr",
            PartitionTableType::Gpt => "gpt",
        }
    }
}

impl fmt::Display for PartitionTableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// OS type detected by inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OsType {
    Linux,
    Windows,
    Hurd,
    FreeBsd,
    NetBsd,
    OpenBsd,
    Minix,
    Unknown,
}

impl std::str::FromStr for OsType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "linux" => OsType::Linux,
            "windows" => OsType::Windows,
            "hurd" => OsType::Hurd,
            "freebsd" => OsType::FreeBsd,
            "netbsd" => OsType::NetBsd,
            "openbsd" => OsType::OpenBsd,
            "minix" => OsType::Minix,
            _ => OsType::Unknown,
        })
    }
}

impl OsType {
    /// Parse from string (convenience method for backward compatibility)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        // Safe: FromStr impl is Infallible (always returns Ok with Unknown fallback)
        s.parse().unwrap_or(OsType::Unknown)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OsType::Linux => "linux",
            OsType::Windows => "windows",
            OsType::Hurd => "hurd",
            OsType::FreeBsd => "freebsd",
            OsType::NetBsd => "netbsd",
            OsType::OpenBsd => "openbsd",
            OsType::Minix => "minix",
            OsType::Unknown => "unknown",
        }
    }
}

impl fmt::Display for OsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Linux distribution detected by inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Distro {
    Ubuntu,
    Debian,
    Fedora,
    Rhel,
    CentOs,
    Archlinux,
    Gentoo,
    Opensuse,
    Suse,
    Alpine,
    Void,
    Nixos,
    Unknown,
}

impl std::str::FromStr for Distro {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "ubuntu" => Distro::Ubuntu,
            "debian" => Distro::Debian,
            "fedora" => Distro::Fedora,
            "rhel" => Distro::Rhel,
            "centos" => Distro::CentOs,
            "archlinux" | "arch" => Distro::Archlinux,
            "gentoo" => Distro::Gentoo,
            "opensuse" => Distro::Opensuse,
            "suse" | "sles" => Distro::Suse,
            "alpine" => Distro::Alpine,
            "void" => Distro::Void,
            "nixos" => Distro::Nixos,
            _ => Distro::Unknown,
        })
    }
}

impl Distro {
    /// Parse from string (convenience method for backward compatibility)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        // Safe: FromStr impl is Infallible (always returns Ok with Unknown fallback)
        s.parse().unwrap_or(Distro::Unknown)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Distro::Ubuntu => "ubuntu",
            Distro::Debian => "debian",
            Distro::Fedora => "fedora",
            Distro::Rhel => "rhel",
            Distro::CentOs => "centos",
            Distro::Archlinux => "archlinux",
            Distro::Gentoo => "gentoo",
            Distro::Opensuse => "opensuse",
            Distro::Suse => "suse",
            Distro::Alpine => "alpine",
            Distro::Void => "void",
            Distro::Nixos => "nixos",
            Distro::Unknown => "unknown",
        }
    }

    /// Get package manager for this distribution
    pub fn package_manager(&self) -> Option<PackageManager> {
        match self {
            Distro::Ubuntu | Distro::Debian => Some(PackageManager::Dpkg),
            Distro::Fedora | Distro::Rhel | Distro::CentOs => Some(PackageManager::Rpm),
            Distro::Archlinux => Some(PackageManager::Pacman),
            Distro::Gentoo => Some(PackageManager::Portage),
            Distro::Alpine => Some(PackageManager::Apk),
            Distro::Nixos => Some(PackageManager::Nix),
            _ => None,
        }
    }
}

impl fmt::Display for Distro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Package manager type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Dpkg,
    Rpm,
    Pacman,
    Portage,
    Apk,
    Nix,
    Xbps,
}

impl PackageManager {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "deb" | "dpkg" => Some(PackageManager::Dpkg),
            "rpm" => Some(PackageManager::Rpm),
            "pacman" => Some(PackageManager::Pacman),
            "portage" => Some(PackageManager::Portage),
            "apk" => Some(PackageManager::Apk),
            "nix" => Some(PackageManager::Nix),
            "xbps" => Some(PackageManager::Xbps),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PackageManager::Dpkg => "deb",
            PackageManager::Rpm => "rpm",
            PackageManager::Pacman => "pacman",
            PackageManager::Portage => "portage",
            PackageManager::Apk => "apk",
            PackageManager::Nix => "nix",
            PackageManager::Xbps => "xbps",
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Mount options builder
#[derive(Debug, Default, Clone)]
pub struct MountOpts {
    pub readonly: bool,
    pub options: Vec<String>,
}

impl MountOpts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn option<S: Into<String>>(mut self, opt: S) -> Self {
        self.options.push(opt.into());
        self
    }

    pub fn to_string(&self) -> Option<String> {
        if self.options.is_empty() {
            None
        } else {
            Some(self.options.join(","))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_type_display() {
        assert_eq!(FilesystemType::Ext4.to_string(), "ext4");
        assert_eq!(FilesystemType::Btrfs.to_string(), "btrfs");
    }

    #[test]
    fn test_filesystem_type_parse() {
        assert_eq!(FilesystemType::from_str("ext4"), Some(FilesystemType::Ext4));
        assert_eq!(
            FilesystemType::from_str("btrfs"),
            Some(FilesystemType::Btrfs)
        );
        assert_eq!(FilesystemType::from_str("unknown"), None);
    }

    #[test]
    fn test_filesystem_supports_features() {
        assert!(FilesystemType::Ext4.supports_labels());
        assert!(FilesystemType::Ext4.supports_uuid());
        assert!(!FilesystemType::Vfat.supports_labels());
    }

    #[test]
    fn test_distro_package_manager() {
        assert_eq!(Distro::Ubuntu.package_manager(), Some(PackageManager::Dpkg));
        assert_eq!(Distro::Fedora.package_manager(), Some(PackageManager::Rpm));
        assert_eq!(
            Distro::Archlinux.package_manager(),
            Some(PackageManager::Pacman)
        );
    }

    #[test]
    fn test_mount_opts_builder() {
        let opts = MountOpts::new()
            .readonly()
            .option("subvol=@home")
            .option("compress=zstd");

        assert!(opts.readonly);
        assert_eq!(
            opts.to_string(),
            Some("subvol=@home,compress=zstd".to_string())
        );
    }
}
