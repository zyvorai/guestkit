// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Common types for guestkit

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Disk image format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskFormat {
    Qcow2,
    Raw,
    Vmdk,
    Vhd,
    Vhdx,
    Vdi,
    Unknown,
}

impl DiskFormat {
    pub fn as_str(&self) -> &str {
        match self {
            DiskFormat::Qcow2 => "qcow2",
            DiskFormat::Raw => "raw",
            DiskFormat::Vmdk => "vmdk",
            DiskFormat::Vhd => "vhd",
            DiskFormat::Vhdx => "vhdx",
            DiskFormat::Vdi => "vdi",
            DiskFormat::Unknown => "unknown",
        }
    }
}

impl DiskFormat {
    /// Parse a format string into a DiskFormat (convenience method).
    /// Kept for backward compatibility with existing call sites.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.parse() {
            Ok(v) => v,
            Err(e) => match e {}, // Infallible — the match is exhaustive over Infallible
        }
    }
}

impl std::str::FromStr for DiskFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(if s.eq_ignore_ascii_case("qcow2") {
            DiskFormat::Qcow2
        } else if s.eq_ignore_ascii_case("raw") {
            DiskFormat::Raw
        } else if s.eq_ignore_ascii_case("vmdk") {
            DiskFormat::Vmdk
        } else if s.eq_ignore_ascii_case("vhd") {
            DiskFormat::Vhd
        } else if s.eq_ignore_ascii_case("vhdx") {
            DiskFormat::Vhdx
        } else if s.eq_ignore_ascii_case("vdi") {
            DiskFormat::Vdi
        } else {
            DiskFormat::Unknown
        })
    }
}

impl std::fmt::Display for DiskFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Guest OS type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestType {
    Linux,
    Windows,
    FreeBSD,
    OpenBSD,
    NetBSD,
    Bsd,
    MacOS,
    Unknown,
}

/// Firmware type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Firmware {
    Bios,
    Uefi,
    Unknown,
}

/// Guest identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestIdentity {
    pub os_type: GuestType,
    pub os_name: String,
    pub os_version: String,
    pub architecture: String,
    pub firmware: Firmware,
    pub init_system: Option<String>,
    pub distro: Option<String>,
}

/// Conversion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub source_format: DiskFormat,
    pub output_format: DiskFormat,
    pub output_size: u64,
    pub duration_secs: f64,
    pub success: bool,
    pub error: Option<String>,
}
