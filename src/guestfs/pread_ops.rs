// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Positional read/write operations for disk image manipulation
//!
//! This implementation provides pread/pwrite functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

impl Guestfs {
    /// Read from file at offset
    ///
    pub fn pread(&mut self, path: &str, count: i32, offset: i64) -> Result<Vec<u8>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: pread {} {} {}", path, count, offset);
        }

        // Validate transfer size to prevent excessive memory allocation
        const MAX_TRANSFER_SIZE: usize = 100 * 1024 * 1024; // 100MB
        if count < 0 || count as u64 > MAX_TRANSFER_SIZE as u64 {
            return Err(Error::InvalidOperation(format!(
                "Transfer size {} exceeds maximum of {} bytes",
                count, MAX_TRANSFER_SIZE
            )));
        }

        if offset < 0 {
            return Err(Error::InvalidOperation(format!(
                "Negative offset not allowed: {}",
                offset
            )));
        }

        let host_path = self.resolve_guest_path(path)?;
        let mut file = File::open(&host_path).map_err(Error::Io)?;

        file.seek(SeekFrom::Start(offset as u64))
            .map_err(Error::Io)?;

        let mut buffer = vec![0u8; count as usize];
        let bytes_read = file.read(&mut buffer).map_err(Error::Io)?;

        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// Read from device at offset
    ///
    pub fn pread_device(&mut self, device: &str, count: i32, offset: i64) -> Result<Vec<u8>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: pread_device {} {} {}", device, count, offset);
        }

        // Validate transfer size to prevent excessive memory allocation
        const MAX_TRANSFER_SIZE: usize = 100 * 1024 * 1024; // 100MB
        if count < 0 || count as u64 > MAX_TRANSFER_SIZE as u64 {
            return Err(Error::InvalidOperation(format!(
                "Transfer size {} exceeds maximum of {} bytes",
                count, MAX_TRANSFER_SIZE
            )));
        }

        self.setup_nbd_if_needed()?;

        if offset < 0 {
            return Err(Error::InvalidOperation(format!(
                "Negative offset not allowed: {}",
                offset
            )));
        }

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let mut file = File::open(nbd_device_path).map_err(Error::Io)?;

        file.seek(SeekFrom::Start(offset as u64))
            .map_err(Error::Io)?;

        let mut buffer = vec![0u8; count as usize];
        let bytes_read = file.read(&mut buffer).map_err(Error::Io)?;

        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// Write to file at offset
    ///
    pub fn pwrite(&mut self, path: &str, content: &[u8], offset: i64) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: pwrite {} {} bytes at {}",
                path,
                content.len(),
                offset
            );
        }

        if offset < 0 {
            return Err(Error::InvalidOperation(format!(
                "Negative offset not allowed: {}",
                offset
            )));
        }

        let host_path = self.resolve_guest_path(path)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&host_path)
            .map_err(Error::Io)?;

        file.seek(SeekFrom::Start(offset as u64))
            .map_err(Error::Io)?;

        let bytes_written = file.write(content).map_err(Error::Io)?;

        Ok(bytes_written as i32)
    }

    /// Write to device at offset
    ///
    pub fn pwrite_device(&mut self, device: &str, content: &[u8], offset: i64) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: pwrite_device {} {} bytes at {}",
                device,
                content.len(),
                offset
            );
        }

        self.setup_nbd_if_needed()?;

        if offset < 0 {
            return Err(Error::InvalidOperation(format!(
                "Negative offset not allowed: {}",
                offset
            )));
        }

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(nbd_device_path)
            .map_err(Error::Io)?;

        file.seek(SeekFrom::Start(offset as u64))
            .map_err(Error::Io)?;

        let bytes_written = file.write(content).map_err(Error::Io)?;

        Ok(bytes_written as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pread_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
