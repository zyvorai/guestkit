// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Enhanced error diagnostics with miette

use miette::Diagnostic;
use thiserror::Error;

/// Enhanced error types with diagnostic information
#[derive(Debug, Error, Diagnostic)]
pub enum DiagnosticError {
    /// Failed to mount a filesystem
    #[error("Failed to mount {device} at {mountpoint}")]
    #[diagnostic(
        code(guestkit::mount::failed),
        help("Try these solutions:\n  1. Check filesystem type: guestkit filesystems {disk}\n  2. Verify device exists\n  3. Check if encrypted (LUKS)")
    )]
    MountFailed {
        device: String,
        mountpoint: String,
        disk: String,
        #[source]
        source: anyhow::Error,
    },

    /// No operating system detected
    #[error("No operating systems detected in {disk}")]
    #[diagnostic(
        code(guestkit::inspect::no_os),
        help("Possible reasons:\n  • Disk is not bootable\n  • Disk is encrypted (check with: guestkit filesystems)\n  • Unsupported OS type\n  • Corrupted disk image\n\nTry:\n  guestkit filesystems {disk}")
    )]
    NoOsDetected { disk: String },

    /// Appliance failed to launch
    #[error("Failed to launch guestfs appliance")]
    #[diagnostic(
        code(guestkit::launch::failed),
        help("Common causes:\n  1. KVM not available - check: ls -l /dev/kvm\n  2. Insufficient permissions - try: sudo guestkit ...\n  3. Corrupted disk image\n  4. QEMU not installed\n\nDebug:\n  Run with: guestkit -v inspect {disk}")
    )]
    LaunchFailed {
        disk: String,
        #[source]
        source: anyhow::Error,
    },

    /// File not found in disk image
    #[error("File not found: {path}")]
    #[diagnostic(
        code(guestkit::file::not_found),
        help("Verify the file exists:\n  guestkit ls {disk} {parent}\n\nNote: Paths are case-sensitive")
    )]
    FileNotFound {
        disk: String,
        path: String,
        parent: String,
    },

    /// Directory not found
    #[error("Not a directory: {path}")]
    #[diagnostic(
        code(guestkit::dir::not_found),
        help("Check the path exists:\n  guestkit ls {disk} {parent}\n\nList root:\n  guestkit ls {disk} /")
    )]
    NotADirectory {
        disk: String,
        path: String,
        parent: String,
    },

    /// Package listing failed
    #[error("Failed to list packages")]
    #[diagnostic(
        code(guestkit::packages::failed),
        help("This OS may not have a supported package manager.\n\nSupported:\n  • dpkg (Debian/Ubuntu)\n  • RPM (Fedora/RHEL/SUSE)\n  • pacman (Arch Linux)\n\nCheck OS type:\n  guestkit inspect {disk}")
    )]
    PackageListFailed {
        disk: String,
        #[source]
        source: anyhow::Error,
    },

    /// Disk image not found
    #[error("Disk image not found: {disk}")]
    #[diagnostic(
        code(guestkit::disk::not_found),
        help("Check:\n  • File path is correct\n  • File exists: ls -l {disk}\n  • You have read permissions")
    )]
    DiskNotFound { disk: String },

    /// Invalid disk format
    #[error("Invalid or unsupported disk format: {disk}")]
    #[diagnostic(
        code(guestkit::disk::invalid_format),
        help("Supported formats:\n  • QCOW2 (.qcow2)\n  • VMDK (.vmdk)\n  • RAW (.img, .raw)\n  • VDI (.vdi)\n  • VHD (.vhd, .vhdx)\n\nCheck format:\n  qemu-img info {disk}")
    )]
    InvalidDiskFormat {
        disk: String,
        #[source]
        source: anyhow::Error,
    },

    /// Permission denied
    #[error("Permission denied")]
    #[diagnostic(
        code(guestkit::permission::denied),
        help("Most operations require root privileges.\n\nRun with sudo:\n  sudo guestkit {command} {disk}")
    )]
    PermissionDenied { command: String, disk: String },

    /// Generic error with context
    #[error("{message}")]
    #[diagnostic(code(guestkit::error))]
    Generic {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

impl DiagnosticError {
    /// Create a mount failed error
    pub fn mount_failed(
        device: impl Into<String>,
        mountpoint: impl Into<String>,
        disk: impl Into<String>,
        source: anyhow::Error,
    ) -> Self {
        Self::MountFailed {
            device: device.into(),
            mountpoint: mountpoint.into(),
            disk: disk.into(),
            source,
        }
    }

    /// Create a no OS detected error
    pub fn no_os_detected(disk: impl Into<String>) -> Self {
        Self::NoOsDetected { disk: disk.into() }
    }

    /// Create a launch failed error
    pub fn launch_failed(disk: impl Into<String>, source: anyhow::Error) -> Self {
        Self::LaunchFailed {
            disk: disk.into(),
            source,
        }
    }

    /// Create a file not found error
    pub fn file_not_found(disk: impl Into<String>, path: impl Into<String>) -> Self {
        let path_str = path.into();
        let parent = std::path::Path::new(&path_str)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("/")
            .to_string();

        Self::FileNotFound {
            disk: disk.into(),
            path: path_str,
            parent,
        }
    }

    /// Create a not a directory error
    pub fn not_a_directory(disk: impl Into<String>, path: impl Into<String>) -> Self {
        let path_str = path.into();
        let parent = std::path::Path::new(&path_str)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("/")
            .to_string();

        Self::NotADirectory {
            disk: disk.into(),
            path: path_str,
            parent,
        }
    }

    /// Create a package list failed error
    pub fn package_list_failed(disk: impl Into<String>, source: anyhow::Error) -> Self {
        Self::PackageListFailed {
            disk: disk.into(),
            source,
        }
    }

    /// Create a disk not found error
    pub fn disk_not_found(disk: impl Into<String>) -> Self {
        Self::DiskNotFound { disk: disk.into() }
    }

    /// Create an invalid disk format error
    pub fn invalid_disk_format(disk: impl Into<String>, source: anyhow::Error) -> Self {
        Self::InvalidDiskFormat {
            disk: disk.into(),
            source,
        }
    }

    /// Create a permission denied error
    pub fn permission_denied(command: impl Into<String>, disk: impl Into<String>) -> Self {
        Self::PermissionDenied {
            command: command.into(),
            disk: disk.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>, source: Option<anyhow::Error>) -> Self {
        Self::Generic {
            message: message.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_error_creation() {
        let err = DiagnosticError::no_os_detected("test.qcow2");
        assert!(err.to_string().contains("test.qcow2"));

        let err = DiagnosticError::file_not_found("test.qcow2", "/etc/passwd");
        assert!(err.to_string().contains("/etc/passwd"));

        let err = DiagnosticError::permission_denied("inspect", "test.qcow2");
        assert!(err.to_string().contains("Permission denied"));
    }
}
