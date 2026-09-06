// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guest OS detection using pure Rust
//!
//! This module implements guest OS detection without external dependencies

use crate::core::{Firmware, GuestIdentity, GuestType, Result};
use crate::disk::{DiskReader, FileSystem, PartitionTable};
use std::path::Path;

/// Guest OS detector
pub struct GuestDetector {}

impl GuestDetector {
    /// Create a new guest detector
    pub fn new() -> Self {
        Self {}
    }

    /// Detect guest OS from disk image
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::detectors::GuestDetector;
    /// use std::path::Path;
    ///
    /// let detector = GuestDetector::new();
    /// let guest = detector.detect_from_image(Path::new("/path/to/disk.qcow2")).unwrap();
    /// println!("OS: {}", guest.os_name);
    /// ```
    pub fn detect_from_image<P: AsRef<Path>>(&self, path: P) -> Result<GuestIdentity> {
        // Open disk image
        let mut reader = DiskReader::open(path.as_ref())?;

        // Parse partition table
        let partition_table = PartitionTable::parse(&mut reader)?;

        // Analyze partitions to detect OS
        let mut os_type = GuestType::Unknown;
        let mut os_name = String::from("Unknown");
        let mut os_version = String::from("Unknown");
        let architecture = String::from("x86_64");
        let mut firmware = Firmware::Bios;
        let mut distro = None;

        // Check for GPT (indicates UEFI)
        if matches!(
            partition_table.table_type(),
            crate::disk::PartitionType::GPT
        ) {
            firmware = Firmware::Uefi;
        }

        // Examine each partition
        for partition in partition_table.partitions() {
            if let Ok(fs) = FileSystem::detect(&mut reader, partition) {
                // Detect OS based on filesystem and partition analysis
                match fs.fs_type() {
                    crate::disk::FileSystemType::Ntfs => {
                        os_type = GuestType::Windows;
                        os_name = "Windows".to_string();
                        os_version = "Unknown".to_string();
                    }

                    crate::disk::FileSystemType::Ext
                    | crate::disk::FileSystemType::Xfs
                    | crate::disk::FileSystemType::Btrfs
                    | crate::disk::FileSystemType::Zfs => {
                        // Linux or BSD (ZFS can be either)
                        // Default to Linux unless BSD hints appear
                        os_type = GuestType::Linux;
                        os_name = "Linux".to_string();

                        if let Some(label) = fs.label() {
                            let l = label.to_lowercase();

                            // Fedora family
                            if l.contains("fedora") {
                                os_name = "Fedora Linux".to_string();
                                distro = Some("fedora".to_string());
                            }
                            // Ubuntu
                            else if l.contains("ubuntu") {
                                os_name = "Ubuntu".to_string();
                                distro = Some("ubuntu".to_string());
                            }
                            // Debian
                            else if l.contains("debian") {
                                os_name = "Debian".to_string();
                                distro = Some("debian".to_string());
                            }
                            // RHEL family
                            else if l.contains("rhel") || l.contains("redhat") {
                                os_name = "Red Hat Enterprise Linux".to_string();
                                distro = Some("rhel".to_string());
                            } else if l.contains("centos") {
                                os_name = "CentOS Linux".to_string();
                                distro = Some("centos".to_string());
                            } else if l.contains("almalinux") || l.contains("alma") {
                                os_name = "AlmaLinux".to_string();
                                distro = Some("almalinux".to_string());
                            } else if l.contains("rocky") {
                                os_name = "Rocky Linux".to_string();
                                distro = Some("rocky".to_string());
                            }
                            // Arch-based
                            else if l.contains("arch") {
                                os_name = "Arch Linux".to_string();
                                distro = Some("arch".to_string());
                            } else if l.contains("manjaro") {
                                os_name = "Manjaro Linux".to_string();
                                distro = Some("manjaro".to_string());
                            }
                            // SUSE family
                            else if l.contains("opensuse") || l.contains("suse") {
                                os_name = "openSUSE".to_string();
                                distro = Some("opensuse".to_string());
                            } else if l.contains("sle") {
                                os_name = "SUSE Linux Enterprise".to_string();
                                distro = Some("sles".to_string());
                            }
                            // Security distros
                            else if l.contains("kali") {
                                os_name = "Kali Linux".to_string();
                                distro = Some("kali".to_string());
                            }
                            // Oracle Linux
                            else if l.contains("oraclelinux")
                                || l.contains("oracle")
                                || l.starts_with("ol_")
                                || l.starts_with("ol-")
                                || l == "ol"
                            {
                                os_name = "Oracle Linux".to_string();
                                distro = Some("oracle".to_string());
                            }
                        }

                        os_version = "Unknown".to_string();
                    }

                    // BSD detection (UFS or ZFS)
                    crate::disk::FileSystemType::Ufs => {
                        os_type = GuestType::Bsd;
                        os_name = "BSD".to_string();

                        // Try to refine based on partition type GUID if available
                        if let Some(guid) = partition.type_guid.as_ref() {
                            let g = guid.to_lowercase();
                            // Match against known BSD partition type GUIDs (full prefix match)
                            // FreeBSD UFS: 516e7cb4-6ecf-11d6-8ff8-00022d09712b
                            // NetBSD FFS:  49f48d5a-b10e-11dc-b99b-0019d1879648
                            // OpenBSD:     824cc7a0-36a8-11e3-890a-952519ad3f61
                            if g.starts_with("516e7cb4") || g.contains("a503") {
                                os_type = GuestType::FreeBSD;
                                os_name = "FreeBSD".to_string();
                            } else if g.starts_with("49f48d5a") || g.contains("a501") {
                                os_type = GuestType::NetBSD;
                                os_name = "NetBSD".to_string();
                            } else if g.starts_with("824cc7a0") || g.contains("a600") {
                                os_type = GuestType::OpenBSD;
                                os_name = "OpenBSD".to_string();
                            }
                        }

                        os_version = "Unknown".to_string();
                    }

                    crate::disk::FileSystemType::HfsPlus => {
                        os_type = GuestType::MacOS;
                        os_name = "macOS (HFS+)".to_string();
                        os_version = "Unknown".to_string();
                    }

                    crate::disk::FileSystemType::Apfs => {
                        os_type = GuestType::MacOS;
                        os_name = "macOS (APFS)".to_string();
                        os_version = "Unknown".to_string();
                    }

                    crate::disk::FileSystemType::Fat32
                        if partition.start_lba < 2048 && partition.size_sectors < 1024 * 1024 =>
                    {
                        firmware = Firmware::Uefi;
                    }

                    _ => {}
                }

                // If we found an OS, break
                if os_type != GuestType::Unknown {
                    break;
                }
            }
        }

        Ok(GuestIdentity {
            os_type,
            os_name,
            os_version,
            architecture,
            firmware,
            init_system: None,
            distro,
        })
    }
}

impl Default for GuestDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = GuestDetector::new();
        let _ = detector;
    }
}
