// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk format converter using qemu-img

use crate::core::{ConversionResult, DiskFormat, Error, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Disk format converter
pub struct DiskConverter {
    qemu_img_path: PathBuf,
}

impl Default for DiskConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskConverter {
    /// Create a new DiskConverter
    pub fn new() -> Self {
        Self {
            qemu_img_path: PathBuf::from("qemu-img"),
        }
    }

    /// Create a DiskConverter with custom qemu-img path
    pub fn with_qemu_img_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            qemu_img_path: path.as_ref().to_path_buf(),
        }
    }

    /// Convert disk image from one format to another
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::converters::DiskConverter;
    /// use std::path::Path;
    ///
    /// let converter = DiskConverter::new();
    /// let result = converter.convert(
    ///     Path::new("/path/to/source.vmdk"),
    ///     Path::new("/path/to/output.qcow2"),
    ///     "qcow2",
    ///     true,  // compress
    ///     true,  // flatten
    /// ).unwrap();
    ///
    /// if result.success {
    ///     println!("Converted {} -> {}",
    ///         result.source_format.as_str(),
    ///         result.output_format.as_str()
    ///     );
    /// }
    /// ```
    pub fn convert<P: AsRef<Path>>(
        &self,
        source_path: P,
        output_path: P,
        output_format: &str,
        compress: bool,
        _flatten: bool,
    ) -> Result<ConversionResult> {
        let source_path = source_path.as_ref();
        let output_path = output_path.as_ref();
        let start = Instant::now();

        // Detect source format
        let source_format = self.detect_format(source_path)?;
        log::info!("Converting {} -> {}", source_format.as_str(), output_format);

        // Build qemu-img command
        let mut cmd = Command::new(&self.qemu_img_path);
        cmd.arg("convert");

        if compress && output_format == "qcow2" {
            cmd.arg("-c");
        }

        cmd.arg("-O")
            .arg(output_format)
            .arg(source_path)
            .arg(output_path);

        // Execute conversion
        log::debug!("Executing: {:?}", cmd);
        match cmd.output() {
            Ok(output) if output.status.success() => {
                let metadata = std::fs::metadata(output_path).map_err(Error::Io)?;
                let duration = start.elapsed().as_secs_f64();

                log::info!(
                    "Conversion complete: {} bytes in {:.2}s",
                    metadata.len(),
                    duration
                );

                Ok(ConversionResult {
                    source_path: source_path.to_path_buf(),
                    output_path: output_path.to_path_buf(),
                    source_format,
                    output_format: DiskFormat::from_str(output_format),
                    output_size: metadata.len(),
                    duration_secs: duration,
                    success: true,
                    error: None,
                })
            }
            Ok(output) => {
                let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                log::error!("Conversion failed: {}", error_msg);

                Ok(ConversionResult {
                    source_path: source_path.to_path_buf(),
                    output_path: output_path.to_path_buf(),
                    source_format,
                    output_format: DiskFormat::from_str(output_format),
                    output_size: 0,
                    duration_secs: start.elapsed().as_secs_f64(),
                    success: false,
                    error: Some(error_msg),
                })
            }
            Err(e) => Err(Error::CommandFailed(format!(
                "Failed to execute qemu-img: {}",
                e
            ))),
        }
    }

    /// Detect disk image format using qemu-img info
    pub fn detect_format<P: AsRef<Path>>(&self, image_path: P) -> Result<DiskFormat> {
        let image_path = image_path.as_ref();

        let output = Command::new(&self.qemu_img_path)
            .arg("info")
            .arg("--output=json")
            .arg(image_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run qemu-img info: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Detection(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let info: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::InvalidFormat(format!("Failed to parse qemu-img output: {}", e)))?;

        let format_str = info
            .get("format")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Detection("No format field in qemu-img output".to_string()))?;

        Ok(DiskFormat::from_str(format_str))
    }

    /// Get disk image information
    pub fn get_info<P: AsRef<Path>>(&self, image_path: P) -> Result<Value> {
        let image_path = image_path.as_ref();

        let output = Command::new(&self.qemu_img_path)
            .arg("info")
            .arg("--output=json")
            .arg(image_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run qemu-img info: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Detection(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::InvalidFormat(format!("Failed to parse qemu-img output: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_format_conversion() {
        assert_eq!(DiskFormat::from_str("qcow2"), DiskFormat::Qcow2);
        assert_eq!(DiskFormat::from_str("QCOW2"), DiskFormat::Qcow2);
        assert_eq!(DiskFormat::from_str("raw"), DiskFormat::Raw);
        assert_eq!(DiskFormat::from_str("vmdk"), DiskFormat::Vmdk);
        assert_eq!(DiskFormat::from_str("invalid"), DiskFormat::Unknown);
    }

    #[test]
    fn test_disk_format_as_str() {
        assert_eq!(DiskFormat::Qcow2.as_str(), "qcow2");
        assert_eq!(DiskFormat::Raw.as_str(), "raw");
        assert_eq!(DiskFormat::Vmdk.as_str(), "vmdk");
    }
}
