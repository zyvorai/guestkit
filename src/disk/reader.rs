// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk image reader
//!
//! Pure Rust implementation for reading disk images (raw, qcow2, etc.)

use crate::core::{DiskFormat, Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// ioctl constant for getting block device size (Linux x86/x86_64)
#[cfg(target_os = "linux")]
const BLKGETSIZE64: libc::c_ulong = 0x80081272;

/// Disk image reader
pub struct DiskReader {
    file: File,
    format: DiskFormat,
    size: u64,
}

impl DiskReader {
    /// Open a disk image
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let mut file = File::open(path_ref).map_err(Error::Io)?;

        // Detect format by reading magic bytes
        let format = Self::detect_format(&mut file)?;

        // Get file size
        // For block devices, metadata().len() doesn't work - need to use ioctl
        let size = if Self::is_block_device(path_ref) {
            // For block devices, use BLKGETSIZE64 ioctl on Linux
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::io::AsRawFd;

                let mut size_bytes: u64 = 0;
                let result = unsafe {
                    libc::ioctl(
                        file.as_raw_fd(),
                        BLKGETSIZE64 as _,
                        &mut size_bytes as *mut u64,
                    )
                };

                if result == 0 {
                    size_bytes
                } else {
                    return Err(Error::Io(std::io::Error::last_os_error()));
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                // Fallback for non-Linux: try seeking (might not work)
                file.seek(SeekFrom::End(0)).map_err(Error::Io)?
            }
        } else {
            // For regular files, use metadata
            file.metadata().map_err(Error::Io)?.len()
        };

        // Reset to start
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;

        Ok(Self { file, format, size })
    }

    /// Check if path is a block device
    fn is_block_device(path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                return metadata.file_type().is_block_device();
            }
        }
        false
    }

    /// Detect on-disk image format from magic bytes (not file extension).
    pub fn detect_image_format<P: AsRef<Path>>(path: P) -> Result<DiskFormat> {
        let mut file = File::open(path).map_err(Error::Io)?;
        Self::detect_format(&mut file)
    }

    /// Detect disk image format from magic bytes
    fn detect_format(file: &mut File) -> Result<DiskFormat> {
        let mut magic = [0u8; 4];
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;

        // Use read() instead of read_exact() for block devices
        // Block devices might not fill the entire buffer
        let bytes_read = file.read(&mut magic).map_err(Error::Io)?;

        if bytes_read < 4 {
            // Not enough data to detect format, assume raw
            return Ok(DiskFormat::Raw);
        }

        // QCOW2 magic: "QFI\xfb"
        if &magic == b"QFI\xfb" {
            return Ok(DiskFormat::Qcow2);
        }

        // Check for other formats
        // VMDK magic at start
        if &magic[0..3] == b"KDM" || &magic[0..3] == b"COW" {
            return Ok(DiskFormat::Vmdk);
        }

        // VHD magic at end (512 bytes from end) "conectix"
        // VDI magic "<<< "

        // Default to raw if no magic found
        Ok(DiskFormat::Raw)
    }

    /// Read bytes at offset
    pub fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.file.seek(SeekFrom::Start(offset)).map_err(Error::Io)?;
        self.file.read(buf).map_err(Error::Io)
    }

    /// Get disk format
    pub fn format(&self) -> &DiskFormat {
        &self.format
    }

    /// Get disk size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Read exact bytes at offset
    pub fn read_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.file.seek(SeekFrom::Start(offset)).map_err(Error::Io)?;

        // For block devices, we might need to read in chunks
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.file.read(&mut buf[total_read..]) {
                Ok(0) => {
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!(
                            "Failed to read {} bytes at offset {}, only got {} bytes",
                            buf.len(),
                            offset,
                            total_read
                        ),
                    )));
                }
                Ok(n) => total_read += n,
                Err(e) => return Err(Error::Io(e)),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_device_detection() {
        // Regular files should not be detected as block devices
        assert!(!DiskReader::is_block_device(std::path::Path::new(
            "/etc/hosts"
        )));
        assert!(!DiskReader::is_block_device(std::path::Path::new(
            "/nonexistent"
        )));
    }
}
