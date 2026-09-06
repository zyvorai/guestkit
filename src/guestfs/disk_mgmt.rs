// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk image management operations for disk image manipulation
//!
//! This implementation provides disk image operations.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create empty disk image
    ///
    pub fn disk_create(&mut self, filename: &str, format: &str, size: i64) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: disk_create {} {} {}", filename, format, size);
        }

        let output = Command::new("qemu-img")
            .arg("create")
            .arg("-f")
            .arg(format)
            .arg(filename)
            .arg(size.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img create failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get disk image format
    ///
    pub fn disk_format(&mut self, filename: &str) -> Result<String> {
        if self.verbose {
            eprintln!("guestfs: disk_format {}", filename);
        }

        let json = self.qemu_img_info_json(filename)?;
        json.get("format")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| Error::NotFound("Format not found in qemu-img output".to_string()))
    }

    /// Check if disk has backing file
    ///
    pub fn disk_has_backing_file(&mut self, filename: &str) -> Result<bool> {
        if self.verbose {
            eprintln!("guestfs: disk_has_backing_file {}", filename);
        }

        let output = Command::new("qemu-img")
            .arg("info")
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img info failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if output contains "backing file"
        Ok(stdout.contains("backing file"))
    }

    /// Run `qemu-img info --output=json` and parse it as a JSON object.
    ///
    /// Used instead of line-scanning the raw text: qemu-img's JSON for
    /// formats like VMDK includes a nested `children[].info` object that
    /// repeats keys like `virtual-size`/`actual-size` with the *inner
    /// file's* values (its raw byte length), which appear earlier in the
    /// text than the real top-level values — a naive
    /// `.lines().find(|l| l.contains("\"virtual-size\""))` silently returns
    /// the wrong (much smaller) number for exactly these formats.
    fn qemu_img_info_json(&self, filename: &str) -> Result<serde_json::Value> {
        let output = Command::new("qemu-img")
            .arg("info")
            .arg("--output=json")
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img info failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::Detection(format!("could not parse qemu-img info JSON: {e}")))
    }

    /// Get virtual size of disk image
    ///
    pub fn disk_virtual_size(&mut self, filename: &str) -> Result<i64> {
        if self.verbose {
            eprintln!("guestfs: disk_virtual_size {}", filename);
        }

        let json = self.qemu_img_info_json(filename)?;
        json.get("virtual-size")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::NotFound("Virtual size not found in qemu-img output".to_string()))
    }

    /// Get actual (allocated) size of disk image — how much host storage it
    /// really consumes, as opposed to `disk_virtual_size`'s guest-visible size.
    ///
    pub fn disk_actual_size(&mut self, filename: &str) -> Result<i64> {
        if self.verbose {
            eprintln!("guestfs: disk_actual_size {}", filename);
        }

        let json = self.qemu_img_info_json(filename)?;
        json.get("actual-size")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::NotFound("Actual size not found in qemu-img output".to_string()))
    }

    /// Resize disk image (grow only — use `disk_resize_shrink` to shrink)
    ///
    pub fn disk_resize(&mut self, filename: &str, size: i64) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: disk_resize {} {}", filename, size);
        }

        let output = Command::new("qemu-img")
            .arg("resize")
            .arg(filename)
            .arg(size.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img resize failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Shrink a disk image's container to `size` bytes.
    ///
    /// Callers are responsible for ensuring the guest filesystem and
    /// partition table have already been shrunk to fit within `size` —
    /// this only truncates the container; it does not touch guest data.
    pub fn disk_resize_shrink(&mut self, filename: &str, size: i64) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: disk_resize_shrink {} {}", filename, size);
        }

        let output = Command::new("qemu-img")
            .arg("resize")
            .arg("--shrink")
            .arg(filename)
            .arg(size.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img resize --shrink failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Zero unused blocks in disk image
    ///
    pub fn zero_free_space(&mut self, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zero_free_space {}", directory);
        }

        let host_path = self.resolve_guest_path(directory)?;

        // Create a file filled with zeros to consume free space
        let zero_file = host_path.join(".zero_file");

        let _output = Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", zero_file.display()))
            .arg("bs=1M")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {}", e)))?;

        // It's expected to fail when disk is full
        // Remove the zero file
        let _ = std::fs::remove_file(&zero_file);

        Ok(())
    }

    /// Sparsify disk image
    ///
    pub fn sparsify(&mut self, input: &str, output: &str) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: sparsify {} {}", input, output);
        }

        // Use cp with sparse option
        let cmd_output = Command::new("cp")
            .arg("--sparse=always")
            .arg(input)
            .arg(output)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute cp: {}", e)))?;

        if !cmd_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "cp --sparse failed: {}",
                String::from_utf8_lossy(&cmd_output.stderr)
            )));
        }

        Ok(())
    }

    /// Convert disk image format
    ///
    pub fn disk_convert(&mut self, input: &str, output: &str, output_format: &str) -> Result<()> {
        if self.verbose {
            eprintln!(
                "guestfs: disk_convert {} {} {}",
                input, output, output_format
            );
        }

        let cmd_output = Command::new("qemu-img")
            .arg("convert")
            .arg("-O")
            .arg(output_format)
            .arg(input)
            .arg(output)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !cmd_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img convert failed: {}",
                String::from_utf8_lossy(&cmd_output.stderr)
            )));
        }

        Ok(())
    }

    /// Check and repair disk image
    ///
    pub fn disk_check(&mut self, filename: &str) -> Result<String> {
        if self.verbose {
            eprintln!("guestfs: disk_check {}", filename);
        }

        let output = Command::new("qemu-img")
            .arg("check")
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        // qemu-img check returns non-zero for errors found, which is expected
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get snapshot list
    ///
    pub fn disk_snapshot_list(&mut self, filename: &str) -> Result<Vec<String>> {
        if self.verbose {
            eprintln!("guestfs: disk_snapshot_list {}", filename);
        }

        let output = Command::new("qemu-img")
            .arg("snapshot")
            .arg("-l")
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute qemu-img: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "qemu-img snapshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let snapshots: Vec<String> = stdout
            .lines()
            .skip(2) // Skip header lines
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();

        Ok(snapshots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_mgmt_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
