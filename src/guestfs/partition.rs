// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Partition table operations for disk image manipulation

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

/// Partition information
#[derive(Debug, Clone)]
pub struct PartInfo {
    pub part_num: i32,
    pub part_start: i64,
    pub part_end: i64,
    pub part_size: i64,
}

impl Guestfs {
    /// Get partition list
    ///
    pub fn part_list(&self, device: &str) -> Result<Vec<PartInfo>> {
        self.ensure_ready()?;

        // Ensure it's a whole device
        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_list requires whole device".to_string(),
            ));
        }

        let partition_table = self.partition_table()?;
        let mut parts = Vec::new();

        for partition in partition_table.partitions() {
            parts.push(PartInfo {
                part_num: partition.number as i32,
                part_start: partition.start_lba.saturating_mul(512) as i64,
                part_end: partition
                    .start_lba
                    .saturating_add(partition.size_sectors)
                    .saturating_mul(512) as i64,
                part_size: partition.size_sectors.saturating_mul(512) as i64,
            });
        }

        Ok(parts)
    }

    /// Get partition table type (mbr or gpt)
    ///
    pub fn part_get_parttype(&self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_get_parttype requires whole device".to_string(),
            ));
        }

        let partition_table = self.partition_table()?;

        match partition_table.table_type() {
            crate::disk::PartitionType::MBR => Ok("msdos".to_string()),
            crate::disk::PartitionType::GPT => Ok("gpt".to_string()),
            crate::disk::PartitionType::Unknown => Ok("unknown".to_string()),
        }
    }

    /// Set partition table type (mbr or gpt)
    ///
    /// WARNING: This operation is DESTRUCTIVE and will erase all existing partitions
    /// on the device. Use with caution.
    ///
    pub fn part_set_parttype(&mut self, device: &str, parttype: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_set_parttype {} {}", device, parttype);
        }

        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_set_parttype requires whole device".to_string(),
            ));
        }

        // Convert partition type names to parted format
        let parted_type = match parttype {
            "msdos" | "mbr" => "msdos",
            "gpt" => "gpt",
            _ => {
                return Err(Error::InvalidOperation(format!(
                    "Unsupported partition table type: {}",
                    parttype
                )))
            }
        };

        // Use parted to set partition table type
        let output = std::process::Command::new("parted")
            .arg("-s")
            .arg(device)
            .arg("mklabel")
            .arg(parted_type)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run parted: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!("parted failed: {}", stderr)));
        }

        Ok(())
    }

    /// Get bootable flag for partition
    ///
    pub fn part_get_bootable(&self, device: &str, partnum: i32) -> Result<bool> {
        self.ensure_ready()?;

        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_get_bootable requires whole device".to_string(),
            ));
        }

        let partition_table = self.partition_table()?;

        let partition = partition_table
            .partitions()
            .iter()
            .find(|p| partnum >= 0 && p.number == partnum as u32)
            .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partnum)))?;

        Ok(partition.bootable)
    }

    /// Get MBR partition type ID
    ///
    pub fn part_get_mbr_id(&self, device: &str, partnum: i32) -> Result<i32> {
        self.ensure_ready()?;

        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_get_mbr_id requires whole device".to_string(),
            ));
        }

        let partition_table = self.partition_table()?;

        // Check if it's actually MBR
        if !matches!(
            partition_table.table_type(),
            crate::disk::PartitionType::MBR
        ) {
            return Err(Error::InvalidFormat(
                "Not an MBR partition table".to_string(),
            ));
        }

        let partition = partition_table
            .partitions()
            .iter()
            .find(|p| partnum >= 0 && p.number == partnum as u32)
            .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partnum)))?;

        Ok(partition.type_id as i32)
    }

    /// Convert partition device to parent device
    ///
    pub fn part_to_dev(&self, partition: &str) -> Result<String> {
        // Strip partition number from device name
        // /dev/sda1 -> /dev/sda
        // /dev/vda2 -> /dev/vda

        if let Some(idx) = partition.rfind(|c: char| !c.is_numeric()) {
            Ok(partition[..=idx].to_string())
        } else {
            Err(Error::InvalidFormat(format!(
                "Cannot parse partition: {}",
                partition
            )))
        }
    }

    /// Get partition number from partition device
    ///
    pub fn part_to_partnum(&self, partition: &str) -> Result<i32> {
        // Extract partition number from device name
        // /dev/sda1 -> 1
        // /dev/vda2 -> 2

        let num_str: String = partition
            .chars()
            .rev()
            .take_while(|c| c.is_numeric())
            .collect::<String>()
            .chars()
            .rev()
            .collect();

        if num_str.is_empty() {
            return Err(Error::InvalidFormat(format!(
                "No partition number in: {}",
                partition
            )));
        }

        num_str
            .parse::<i32>()
            .map_err(|_| Error::InvalidFormat(format!("Invalid partition number: {}", num_str)))
    }

    /// Get partition name (GPT partition label)
    ///
    pub fn part_get_name(&mut self, device: &str, partnum: i32) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_get_name {} {}", device, partnum);
        }

        if !self.is_whole_device(device)? {
            return Err(Error::InvalidFormat(
                "part_get_name requires whole device".to_string(),
            ));
        }

        // Use sgdisk to get partition name (GPT only)
        let output = std::process::Command::new("sgdisk")
            .arg("-i")
            .arg(partnum.to_string())
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(
                "Failed to get partition name (may not be GPT)".to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output for "Partition name:"
        for line in stdout.lines() {
            if line.contains("Partition name:") {
                let name = line
                    .split("Partition name:")
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .trim_matches('\'')
                    .to_string();
                return Ok(name);
            }
        }

        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_part_to_dev() {
        let g = Guestfs::new().unwrap();

        assert_eq!(g.part_to_dev("/dev/sda1").unwrap(), "/dev/sda");
        assert_eq!(g.part_to_dev("/dev/sda10").unwrap(), "/dev/sda");
        assert_eq!(g.part_to_dev("/dev/vda2").unwrap(), "/dev/vda");
    }

    #[test]
    fn test_part_to_partnum() {
        let g = Guestfs::new().unwrap();

        assert_eq!(g.part_to_partnum("/dev/sda1").unwrap(), 1);
        assert_eq!(g.part_to_partnum("/dev/sda10").unwrap(), 10);
        assert_eq!(g.part_to_partnum("/dev/vda2").unwrap(), 2);
        assert!(g.part_to_partnum("/dev/sda").is_err());
    }
}
