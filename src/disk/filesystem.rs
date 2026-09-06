// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Filesystem detection
//!
//! Pure Rust implementation for detecting filesystem types

use crate::core::{Error, Result};
use crate::disk::partition::Partition;
use crate::disk::reader::DiskReader;

/// Filesystem type
#[derive(Debug, Clone, PartialEq)]
pub enum FileSystemType {
    /// ext2/ext3/ext4
    Ext,
    /// NTFS
    Ntfs,
    /// FAT32
    Fat32,
    /// exFAT
    ExFat,
    /// XFS
    Xfs,
    /// Btrfs
    Btrfs,
    /// ZFS
    Zfs,
    /// UFS (BSD)
    Ufs,
    /// HFS+ (macOS)
    HfsPlus,
    /// APFS (macOS)
    Apfs,
    /// ISO9660 (CD/DVD)
    Iso9660,
    /// Linux Swap
    Swap,
    /// Unknown filesystem
    Unknown,
}

/// Filesystem information
#[derive(Debug, Clone)]
pub struct FileSystem {
    fs_type: FileSystemType,
    label: Option<String>,
    uuid: Option<String>,
}

impl FileSystem {
    /// Detect filesystem from partition
    pub fn detect(reader: &mut DiskReader, partition: &Partition) -> Result<Self> {
        let offset = partition.start_lba.checked_mul(512).ok_or_else(|| {
            Error::Detection(format!(
                "Integer overflow computing partition offset: start_lba={} * 512",
                partition.start_lba
            ))
        })?;

        // Array of detector functions for cleaner dispatch
        let detectors: &[fn(&mut DiskReader, u64) -> Result<FileSystem>] = &[
            Self::detect_ext,
            Self::detect_ntfs,
            Self::detect_fat32,
            Self::detect_exfat,
            Self::detect_xfs,
            Self::detect_btrfs,
            Self::detect_zfs,
            Self::detect_ufs,
            Self::detect_hfsplus,
            Self::detect_apfs,
            Self::detect_iso9660,
            Self::detect_swap,
        ];

        // Validate that offset won't overflow when detector functions add to it.
        // The largest internal offset used is 0x8000 + 2048 = 34816 for ISO9660.
        // We also need 65536 + 512 for btrfs/ufs. Check the largest (65536 + 1376).
        if offset.checked_add(65536 + 1376).is_none() {
            return Err(Error::Detection(format!(
                "Partition offset {} is too large, would overflow during filesystem detection",
                offset
            )));
        }

        // Try each detector in order
        for detector in detectors {
            if let Ok(fs) = detector(reader, offset) {
                return Ok(fs);
            }
        }

        // No filesystem detected
        Ok(Self {
            fs_type: FileSystemType::Unknown,
            label: None,
            uuid: None,
        })
    }

    /// Detect ext2/ext3/ext4 filesystem
    fn detect_ext(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // ext superblock is at offset 1024 from partition start
        let superblock_offset = partition_offset + 1024;
        let mut superblock = vec![0u8; 264];
        reader.read_exact_at(superblock_offset, &mut superblock)?;

        // Check magic number at offset 56-57 (0xEF53)
        if superblock[56] == 0x53 && superblock[57] == 0xEF {
            // Read volume label at offset 120 (16 bytes)
            let label_bytes = &superblock[120..136];
            let label = String::from_utf8_lossy(label_bytes)
                .trim_end_matches('\0')
                .to_string();

            let label = if label.is_empty() { None } else { Some(label) };

            // Read UUID at offset 104 (16 bytes)
            let uuid_bytes = &superblock[104..120];
            let uuid = format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                uuid_bytes[0], uuid_bytes[1], uuid_bytes[2], uuid_bytes[3],
                uuid_bytes[4], uuid_bytes[5],
                uuid_bytes[6], uuid_bytes[7],
                uuid_bytes[8], uuid_bytes[9],
                uuid_bytes[10], uuid_bytes[11], uuid_bytes[12], uuid_bytes[13], uuid_bytes[14], uuid_bytes[15]
            );

            return Ok(Self {
                fs_type: FileSystemType::Ext,
                label,
                uuid: Some(uuid),
            });
        }

        Err(Error::Detection("Not an ext filesystem".to_string()))
    }

    /// Detect NTFS filesystem
    fn detect_ntfs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        let mut boot_sector = vec![0u8; 512];
        reader.read_exact_at(partition_offset, &mut boot_sector)?;

        // Check NTFS signature "NTFS    " at offset 3
        if &boot_sector[3..11] == b"NTFS    " {
            return Ok(Self {
                fs_type: FileSystemType::Ntfs,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not an NTFS filesystem".to_string()))
    }

    /// Detect FAT32 filesystem
    fn detect_fat32(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        let mut boot_sector = vec![0u8; 512];
        reader.read_exact_at(partition_offset, &mut boot_sector)?;

        // Check FAT32 signature "FAT32   " at offset 82
        if boot_sector.len() >= 90 && &boot_sector[82..90] == b"FAT32   " {
            return Ok(Self {
                fs_type: FileSystemType::Fat32,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not a FAT32 filesystem".to_string()))
    }

    /// Detect XFS filesystem
    fn detect_xfs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        let mut superblock = vec![0u8; 512];
        reader.read_exact_at(partition_offset, &mut superblock)?;

        // Check XFS magic "XFSB"
        if &superblock[0..4] == b"XFSB" {
            return Ok(Self {
                fs_type: FileSystemType::Xfs,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not an XFS filesystem".to_string()))
    }

    /// Detect Btrfs filesystem
    fn detect_btrfs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // Btrfs superblock is at offset 65536
        let superblock_offset = partition_offset + 65536;
        let mut superblock = vec![0u8; 512];
        reader.read_exact_at(superblock_offset, &mut superblock)?;

        // Check Btrfs magic "_BHRfS_M"
        if &superblock[64..72] == b"_BHRfS_M" {
            return Ok(Self {
                fs_type: FileSystemType::Btrfs,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not a Btrfs filesystem".to_string()))
    }

    /// Detect ZFS filesystem
    fn detect_zfs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // ZFS uberblock magic number: 0x00bab10c (little-endian: 0x0c, 0xb1, 0xba, 0x00)
        const ZFS_UBERBLOCK_MAGIC: [u8; 4] = [0x0c, 0xb1, 0xba, 0x00];

        // ZFS has multiple labels at different offsets (128K, 256K, 512K, 1M)
        // Check the first uberblock at 128KB
        let label_offset = partition_offset + 131072; // 128KB
        let mut buffer = vec![0u8; 4];
        reader.read_exact_at(label_offset, &mut buffer)?;

        // Check for the ZFS uberblock magic number
        if buffer[0..4] == ZFS_UBERBLOCK_MAGIC {
            return Ok(Self {
                fs_type: FileSystemType::Zfs,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not a ZFS filesystem".to_string()))
    }

    /// Detect UFS (BSD) filesystem
    fn detect_ufs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // UFS superblock is at offset 8192 for UFS1, or 65536 for UFS2
        // Try UFS2 first (more modern)
        let superblock_offset = partition_offset + 65536;
        let mut superblock = vec![0u8; 1376];
        reader.read_exact_at(superblock_offset, &mut superblock)?;

        // Check UFS2 magic: 0x19540119
        if superblock.len() >= 1376 {
            let magic = u32::from_le_bytes([
                superblock[1372],
                superblock[1373],
                superblock[1374],
                superblock[1375],
            ]);
            if magic == 0x19540119 {
                return Ok(Self {
                    fs_type: FileSystemType::Ufs,
                    label: None,
                    uuid: None,
                });
            }
        }

        // Try UFS1 at offset 8192
        let superblock_offset = partition_offset + 8192;
        let mut superblock = vec![0u8; 1376];
        reader.read_exact_at(superblock_offset, &mut superblock)?;

        if superblock.len() >= 1376 {
            let magic = u32::from_le_bytes([
                superblock[1372],
                superblock[1373],
                superblock[1374],
                superblock[1375],
            ]);
            if magic == 0x011954 || magic == 0x19540119 {
                return Ok(Self {
                    fs_type: FileSystemType::Ufs,
                    label: None,
                    uuid: None,
                });
            }
        }

        Err(Error::Detection("Not a UFS filesystem".to_string()))
    }

    /// Detect HFS+ filesystem (macOS)
    fn detect_hfsplus(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // HFS+ volume header is at offset 1024
        let header_offset = partition_offset + 1024;
        let mut header = vec![0u8; 512];
        reader.read_exact_at(header_offset, &mut header)?;

        // Check HFS+ signature "H+" or "HX" at offset 0-1
        if header.len() >= 2 && header[0] == b'H' && (header[1] == b'+' || header[1] == b'X') {
            return Ok(Self {
                fs_type: FileSystemType::HfsPlus,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not an HFS+ filesystem".to_string()))
    }

    /// Detect APFS filesystem (macOS)
    fn detect_apfs(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // APFS container superblock is at the start of the partition
        let mut superblock = vec![0u8; 4096];
        reader.read_exact_at(partition_offset, &mut superblock)?;

        // Check APFS magic "NXSB" (container superblock) or "APSB" (volume superblock)
        if superblock.len() >= 36 {
            // Magic is at offset 32-35
            if &superblock[32..36] == b"NXSB" || &superblock[32..36] == b"APSB" {
                return Ok(Self {
                    fs_type: FileSystemType::Apfs,
                    label: None,
                    uuid: None,
                });
            }
        }

        Err(Error::Detection("Not an APFS filesystem".to_string()))
    }

    /// Detect exFAT filesystem
    fn detect_exfat(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        let mut sector = vec![0u8; 512];
        reader.read_exact_at(partition_offset, &mut sector)?;

        // exFAT signature at offset 3: "EXFAT   " (8 bytes)
        if sector.len() >= 11 && &sector[3..11] == b"EXFAT   " {
            return Ok(Self {
                fs_type: FileSystemType::ExFat,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not an exFAT filesystem".to_string()))
    }

    /// Detect ISO9660 filesystem (CD/DVD)
    fn detect_iso9660(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // Primary Volume Descriptor at offset 0x8000 (sector 16)
        let mut buf = vec![0u8; 2048];
        reader.read_exact_at(partition_offset + 0x8000, &mut buf)?;

        // Check for CD001 signature at offset 1
        if buf.len() >= 6 && &buf[1..6] == b"CD001" {
            return Ok(Self {
                fs_type: FileSystemType::Iso9660,
                label: None,
                uuid: None,
            });
        }

        Err(Error::Detection("Not an ISO9660 filesystem".to_string()))
    }

    /// Detect Linux Swap
    fn detect_swap(reader: &mut DiskReader, partition_offset: u64) -> Result<Self> {
        // Swap signature is at the end of the first page (4096 bytes)
        // Signature can be "SWAPSPACE2" or "SWAP-SPACE"
        let mut buf = vec![0u8; 4096];
        reader.read_exact_at(partition_offset, &mut buf)?;

        // Check for SWAPSPACE2 at offset 4086 (pagesize - 10)
        if buf.len() >= 4096 {
            let sig_offset = 4096 - 10;
            if &buf[sig_offset..sig_offset + 10] == b"SWAPSPACE2" {
                return Ok(Self {
                    fs_type: FileSystemType::Swap,
                    label: None,
                    uuid: None,
                });
            }
            // Also check for older SWAP-SPACE signature
            if &buf[sig_offset..sig_offset + 10] == b"SWAP-SPACE" {
                return Ok(Self {
                    fs_type: FileSystemType::Swap,
                    label: None,
                    uuid: None,
                });
            }
        }

        Err(Error::Detection("Not a swap partition".to_string()))
    }

    /// Get filesystem type
    pub fn fs_type(&self) -> &FileSystemType {
        &self.fs_type
    }

    /// Get filesystem label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Get filesystem UUID
    pub fn uuid(&self) -> Option<&str> {
        self.uuid.as_deref()
    }

    /// Read file from filesystem (basic implementation)
    pub fn read_file(
        &self,
        _reader: &mut DiskReader,
        _partition: &Partition,
        path: &str,
    ) -> Result<Vec<u8>> {
        // Direct file reading from raw filesystem blocks requires parsing the
        // filesystem structure (superblock, inodes, directory entries, extent trees).
        // This is handled by the mount-based path through guestfs instead.
        Err(Error::Detection(format!(
            "Direct file reading not supported for path: {} — use guestfs mount-based access instead",
            path
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_types() {
        assert_eq!(FileSystemType::Ext, FileSystemType::Ext);
        assert_eq!(FileSystemType::Ntfs, FileSystemType::Ntfs);
    }
}
