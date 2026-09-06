// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Compression operations for disk image manipulation
//!
//! This implementation provides file compression and decompression functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Compress file with gzip
    ///
    pub fn compress_out(&mut self, ctype: &str, file: &str, output: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: compress_out {} {} {}", ctype, file, output);
        }

        let host_path = self.resolve_guest_path(file)?;

        let compress_cmd = match ctype {
            "gzip" | "gz" => "gzip",
            "bzip2" | "bz2" => "bzip2",
            "xz" => "xz",
            "lzop" | "lzo" => "lzop",
            "compress" => "compress",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unknown compression type: {}",
                    ctype
                )))
            }
        };

        let output_cmd = Command::new(compress_cmd)
            .arg("-c")
            .arg(&host_path)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to execute {}: {}", compress_cmd, e))
            })?;

        if !output_cmd.status.success() {
            return Err(Error::CommandFailed(format!(
                "{} failed: {}",
                compress_cmd,
                String::from_utf8_lossy(&output_cmd.stderr)
            )));
        }

        std::fs::write(output, &output_cmd.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// Decompress file
    ///
    pub fn compress_device_out(&mut self, ctype: &str, device: &str, output: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: compress_device_out {} {} {}",
                ctype, device, output
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let compress_cmd = match ctype {
            "gzip" | "gz" => "gzip",
            "bzip2" | "bz2" => "bzip2",
            "xz" => "xz",
            "lzop" | "lzo" => "lzop",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unknown compression type: {}",
                    ctype
                )))
            }
        };

        let output_cmd = Command::new(compress_cmd)
            .arg("-c")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to execute {}: {}", compress_cmd, e))
            })?;

        std::fs::write(output, &output_cmd.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// Copy file with compression
    ///
    /// Additional functionality for compressed file operations
    pub fn copy_file_compressed(&mut self, src: &str, dest: &str, ctype: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_file_compressed {} {} {}", src, dest, ctype);
        }

        let src_path = self.resolve_guest_path(src)?;
        let dest_path = self.resolve_guest_path(dest)?;

        let compress_cmd = match ctype {
            "gzip" | "gz" => "gzip",
            "bzip2" | "bz2" => "bzip2",
            "xz" => "xz",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unknown compression type: {}",
                    ctype
                )))
            }
        };

        let output = Command::new(compress_cmd)
            .arg("-c")
            .arg(&src_path)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to execute {}: {}", compress_cmd, e))
            })?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "{} failed: {}",
                compress_cmd,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        std::fs::write(&dest_path, &output.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// Decompress file
    ///
    /// Additional functionality for decompression
    pub fn decompress_file(&mut self, src: &str, dest: &str, ctype: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: decompress_file {} {} {}", src, dest, ctype);
        }

        let src_path = self.resolve_guest_path(src)?;
        let dest_path = self.resolve_guest_path(dest)?;

        let decompress_cmd = match ctype {
            "gzip" | "gz" => "gunzip",
            "bzip2" | "bz2" => "bunzip2",
            "xz" => "unxz",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Unknown compression type: {}",
                    ctype
                )))
            }
        };

        let output = Command::new(decompress_cmd)
            .arg("-c")
            .arg(&src_path)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to execute {}: {}", decompress_cmd, e))
            })?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "{} failed: {}",
                decompress_cmd,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        std::fs::write(&dest_path, &output.stdout).map_err(Error::Io)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
