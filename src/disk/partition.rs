// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Partition table parser
//!
//! Pure Rust implementation for parsing MBR and GPT partition tables

use crate::core::{Error, Result};
use crate::disk::reader::DiskReader;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Partition type
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionType {
    /// Master Boot Record
    MBR,
    /// GUID Partition Table
    GPT,
    /// Unknown partition scheme
    Unknown,
}

/// Partition information
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition number (1-based)
    pub number: u32,
    /// Start sector (LBA)
    pub start_lba: u64,
    /// Size in sectors
    pub size_sectors: u64,
    /// Partition type ID
    pub type_id: u8,
    /// Bootable flag
    pub bootable: bool,
    /// Partition type GUID (for GPT partitions)
    pub type_guid: Option<String>,
}

/// Partition table
pub struct PartitionTable {
    partitions: Vec<Partition>,
    table_type: PartitionType,
}

impl PartitionTable {
    /// Parse partition table from disk
    pub fn parse(reader: &mut DiskReader) -> Result<Self> {
        // Read first sector (MBR/protective MBR)
        let mut mbr_sector = vec![0u8; 512];
        reader.read_exact_at(0, &mut mbr_sector)?;

        // Check for GPT signature
        if Self::is_gpt(&mbr_sector) {
            Self::parse_gpt(reader)
        } else if Self::is_mbr(&mbr_sector) {
            Self::parse_mbr(&mbr_sector)
        } else {
            Ok(Self {
                partitions: Vec::new(),
                table_type: PartitionType::Unknown,
            })
        }
    }

    /// Check if disk has GPT
    fn is_gpt(mbr: &[u8]) -> bool {
        // Check for protective MBR (partition type 0xEE)
        if mbr.len() < 512 {
            return false;
        }

        // Check MBR signature
        if mbr[510] != 0x55 || mbr[511] != 0xAA {
            return false;
        }

        // Protective MBR: GPT marker 0xEE in any of the four primary entries
        (0..4).any(|i| mbr[446 + i * 16 + 4] == 0xEE)
    }

    /// Check if disk has valid MBR
    fn is_mbr(mbr: &[u8]) -> bool {
        if mbr.len() < 512 {
            return false;
        }

        // Check MBR signature
        mbr[510] == 0x55 && mbr[511] == 0xAA
    }

    /// Parse MBR partition table
    fn parse_mbr(mbr: &[u8]) -> Result<Self> {
        let mut partitions = Vec::new();

        // Parse 4 primary partitions (entries at offset 446)
        for i in 0..4 {
            let offset = 446 + (i * 16);
            let entry = &mbr[offset..offset + 16];

            let type_id = entry[4];
            if type_id == 0 {
                // Empty partition
                continue;
            }

            let bootable = entry[0] == 0x80;
            let start_lba = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as u64;
            let size_sectors =
                u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as u64;

            partitions.push(Partition {
                number: (i + 1) as u32,
                start_lba,
                size_sectors,
                type_id,
                bootable,
                type_guid: None,
            });
        }

        Ok(Self {
            partitions,
            table_type: PartitionType::MBR,
        })
    }

    /// Parse GPT partition table
    fn parse_gpt(reader: &mut DiskReader) -> Result<Self> {
        // Read GPT header (sector 1)
        let mut gpt_header = vec![0u8; 512];
        reader.read_exact_at(512, &mut gpt_header)?;

        // Check GPT signature "EFI PART"
        if &gpt_header[0..8] != b"EFI PART" {
            return Err(Error::Detection("Invalid GPT signature".to_string()));
        }

        let mut cursor = Cursor::new(&gpt_header[72..]);
        let partition_entries_lba = cursor.read_u64::<LittleEndian>().map_err(Error::Io)?;
        let num_entries = cursor.read_u32::<LittleEndian>().map_err(Error::Io)?;
        let entry_size = cursor.read_u32::<LittleEndian>().map_err(Error::Io)?;

        // Validate entry size to prevent excessive memory allocation or undersize entries
        if !(48..=8192).contains(&entry_size) {
            return Err(Error::Detection(format!(
                "Invalid GPT entry size: {}",
                entry_size
            )));
        }

        // Read partition entries
        let mut partitions = Vec::new();
        let entries_offset = partition_entries_lba * 512;

        for i in 0..num_entries.min(128) {
            let offset = entries_offset + (i as u64 * entry_size as u64);
            let mut entry = vec![0u8; entry_size as usize];
            reader.read_exact_at(offset, &mut entry)?;

            // Check if partition is used (non-zero type GUID)
            if entry.iter().take(16).all(|&b| b == 0) {
                continue;
            }

            // Extract partition type GUID (bytes 0-15)
            let type_guid = format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                entry[3], entry[2], entry[1], entry[0],  // Little-endian DWORD
                entry[5], entry[4],                       // Little-endian WORD
                entry[7], entry[6],                       // Little-endian WORD
                entry[8], entry[9],                       // Big-endian bytes
                entry[10], entry[11], entry[12], entry[13], entry[14], entry[15]
            );

            let mut cursor = Cursor::new(&entry[32..]);
            let start_lba = cursor.read_u64::<LittleEndian>().map_err(Error::Io)?;
            let end_lba = cursor.read_u64::<LittleEndian>().map_err(Error::Io)?;

            if start_lba == 0 && end_lba == 0 {
                continue;
            }

            if end_lba < start_lba {
                continue;
            }

            partitions.push(Partition {
                number: (i + 1),
                start_lba,
                size_sectors: end_lba - start_lba + 1,
                type_id: 0, // GPT doesn't use type_id
                bootable: false,
                type_guid: Some(type_guid),
            });
        }

        Ok(Self {
            partitions,
            table_type: PartitionType::GPT,
        })
    }

    /// Get all partitions
    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }

    /// Get partition table type
    pub fn table_type(&self) -> &PartitionType {
        &self.table_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_types() {
        assert_eq!(PartitionType::MBR, PartitionType::MBR);
        assert_eq!(PartitionType::GPT, PartitionType::GPT);
    }

    #[test]
    fn test_is_gpt_protective_mbr_any_slot() {
        let mut mbr = [0u8; 512];
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        // 0xEE in third primary entry
        mbr[446 + 2 * 16 + 4] = 0xEE;
        assert!(PartitionTable::is_gpt(&mbr));
        assert!(!PartitionTable::is_gpt(&[0u8; 512]));
    }
}
