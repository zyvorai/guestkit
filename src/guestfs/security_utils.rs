// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Security validation utilities to prevent command injection and path traversal attacks.

use crate::core::error::{Error, Result};

/// Security validator for paths and system operations
pub struct PathValidator;

impl PathValidator {
    /// Validate a device path to prevent command injection
    ///
    /// Ensures the path:
    /// - Starts with /dev/
    /// - Contains only alphanumeric characters, /, -, and _ after /dev/
    /// - Does not contain shell metacharacters
    pub fn validate_device_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Err(Error::InvalidOperation(
                "Device path cannot be empty".to_string(),
            ));
        }

        if !path.starts_with("/dev/") {
            return Err(Error::InvalidOperation(format!(
                "Device path must start with /dev/: {}",
                path
            )));
        }

        // Check for shell metacharacters that could enable command injection
        const DANGEROUS_CHARS: &[char] = &[
            ';', '&', '|', '$', '`', '(', ')', '{', '}', '[', ']', '<', '>', '\n', '\r', '\\',
            '\'', '"', ' ', '\t', '*', '?',
        ];

        for ch in DANGEROUS_CHARS {
            if path.contains(*ch) {
                return Err(Error::InvalidOperation(format!(
                    "Device path contains dangerous character '{}': {}",
                    ch, path
                )));
            }
        }

        // Ensure reasonable length
        if path.len() > 255 {
            return Err(Error::InvalidOperation(format!(
                "Device path too long (max 255): {}",
                path.len()
            )));
        }

        Ok(())
    }

    /// Validate a filesystem path to prevent path traversal attacks
    ///
    /// Ensures the path:
    /// - Does not contain ".." sequences
    /// - Does not contain null bytes
    /// - Has reasonable length
    pub fn validate_fs_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Err(Error::InvalidOperation(
                "Filesystem path cannot be empty".to_string(),
            ));
        }

        // Check for path traversal attempts
        if path.contains("..") {
            return Err(Error::InvalidOperation(format!(
                "Path contains '..' (path traversal attack): {}",
                path
            )));
        }

        // Check for null bytes
        if path.contains('\0') {
            return Err(Error::InvalidOperation(
                "Path contains null byte".to_string(),
            ));
        }

        // Ensure reasonable length
        if path.len() > 4096 {
            return Err(Error::InvalidOperation(format!(
                "Path too long (max 4096): {}",
                path.len()
            )));
        }

        Ok(())
    }

    /// Validate cpio format string to prevent command injection
    ///
    /// Only allows known safe cpio formats
    pub fn validate_cpio_format(format: &str) -> Result<()> {
        const VALID_FORMATS: &[&str] = &[
            "newc", "crc", "odc", "bin", "ustar", "tar", "hpbin", "hpodc",
        ];

        if !VALID_FORMATS.contains(&format) {
            return Err(Error::InvalidOperation(format!(
                "Invalid cpio format '{}'. Allowed: {:?}",
                format, VALID_FORMATS
            )));
        }

        Ok(())
    }

    /// Validate that a path component is safe (no directory traversal)
    pub fn validate_path_component(component: &str) -> Result<()> {
        if component.is_empty() || component == "." || component == ".." {
            return Err(Error::InvalidOperation(format!(
                "Invalid path component: {}",
                component
            )));
        }

        if component.contains('/') || component.contains('\0') {
            return Err(Error::InvalidOperation(format!(
                "Path component contains invalid characters: {}",
                component
            )));
        }

        Ok(())
    }

    /// Sanitize a device path by checking it's valid and returning it
    pub fn sanitize_device_path(path: &str) -> Result<String> {
        Self::validate_device_path(path)?;
        Ok(path.to_string())
    }

    /// Sanitize a filesystem path by checking it's valid and returning it
    pub fn sanitize_fs_path(path: &str) -> Result<String> {
        Self::validate_fs_path(path)?;
        Ok(path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_device_paths() {
        assert!(PathValidator::validate_device_path("/dev/sda").is_ok());
        assert!(PathValidator::validate_device_path("/dev/sda1").is_ok());
        assert!(PathValidator::validate_device_path("/dev/vda").is_ok());
        assert!(PathValidator::validate_device_path("/dev/nvme0n1").is_ok());
        assert!(PathValidator::validate_device_path("/dev/loop0").is_ok());
    }

    #[test]
    fn test_invalid_device_paths() {
        // Command injection attempts
        assert!(PathValidator::validate_device_path("/dev/sda; rm -rf /").is_err());
        assert!(PathValidator::validate_device_path("/dev/sda && malicious").is_err());
        assert!(PathValidator::validate_device_path("/dev/sda | cat").is_err());
        assert!(PathValidator::validate_device_path("/dev/sda$(evil)").is_err());
        assert!(PathValidator::validate_device_path("/dev/sda`cmd`").is_err());

        // Invalid prefixes
        assert!(PathValidator::validate_device_path("sda").is_err());
        assert!(PathValidator::validate_device_path("/home/user/file").is_err());

        // Empty path
        assert!(PathValidator::validate_device_path("").is_err());
    }

    #[test]
    fn test_valid_fs_paths() {
        assert!(PathValidator::validate_fs_path("/").is_ok());
        assert!(PathValidator::validate_fs_path("/etc/passwd").is_ok());
        assert!(PathValidator::validate_fs_path("/home/user/file.txt").is_ok());
        assert!(PathValidator::validate_fs_path("relative/path").is_ok());
    }

    #[test]
    fn test_invalid_fs_paths() {
        // Path traversal attempts
        assert!(PathValidator::validate_fs_path("../etc/passwd").is_err());
        assert!(PathValidator::validate_fs_path("/dir/../../../etc/shadow").is_err());
        assert!(PathValidator::validate_fs_path("./../../root").is_err());

        // Null bytes
        assert!(PathValidator::validate_fs_path("/path\0/file").is_err());

        // Empty path
        assert!(PathValidator::validate_fs_path("").is_err());
    }

    #[test]
    fn test_valid_cpio_formats() {
        assert!(PathValidator::validate_cpio_format("newc").is_ok());
        assert!(PathValidator::validate_cpio_format("crc").is_ok());
        assert!(PathValidator::validate_cpio_format("tar").is_ok());
    }

    #[test]
    fn test_invalid_cpio_formats() {
        assert!(PathValidator::validate_cpio_format("invalid").is_err());
        assert!(PathValidator::validate_cpio_format("newc; rm -rf /").is_err());
        assert!(PathValidator::validate_cpio_format("").is_err());
    }

    #[test]
    fn test_path_components() {
        assert!(PathValidator::validate_path_component("file.txt").is_ok());
        assert!(PathValidator::validate_path_component("dir").is_ok());

        assert!(PathValidator::validate_path_component("..").is_err());
        assert!(PathValidator::validate_path_component(".").is_err());
        assert!(PathValidator::validate_path_component("").is_err());
        assert!(PathValidator::validate_path_component("dir/file").is_err());
    }
}
