// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration planning profile

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection};
use crate::Guestfs;
use anyhow::Result;

pub struct MigrationProfile;

impl InspectionProfile for MigrationProfile {
    fn name(&self) -> &str {
        "migration"
    }

    fn description(&self) -> &str {
        "Migration planning and compatibility analysis"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            // Section 1: Operating System Details
            self.analyze_os(g, root),
            // Section 2: Package Inventory
            self.analyze_packages(g, root),
            // Section 3: Storage Layout
            self.analyze_storage(g, root),
            // Section 4: Network Configuration
            self.analyze_network(g, root),
            // Section 5: Custom Services & Applications
            self.analyze_custom_services(g, root),
            // Section 6: Data Directories
            self.analyze_data_directories(g, root),
        ];

        Ok(ProfileReport {
            profile_name: "Migration Planning".to_string(),
            sections,
            overall_risk: None,
            summary: Some(
                "Review all sections to plan migration strategy and identify dependencies."
                    .to_string(),
            ),
        })
    }
}

impl MigrationProfile {
    fn analyze_os(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // OS Type
        if let Ok(os_type) = g.inspect_get_type(root) {
            findings.push(Finding {
                item: "OS Type".to_string(),
                status: FindingStatus::Info,
                message: os_type,
                risk_level: None,
            });
        }

        // Distribution
        if let Ok(distro) = g.inspect_get_distro(root) {
            findings.push(Finding {
                item: "Distribution".to_string(),
                status: FindingStatus::Info,
                message: distro,
                risk_level: None,
            });
        }

        // Version
        if let (Ok(major), Ok(minor)) = (
            g.inspect_get_major_version(root),
            g.inspect_get_minor_version(root),
        ) {
            findings.push(Finding {
                item: "Version".to_string(),
                status: FindingStatus::Info,
                message: format!("{}.{}", major, minor),
                risk_level: None,
            });
        }

        // Architecture
        if let Ok(arch) = g.inspect_get_arch(root) {
            findings.push(Finding {
                item: "Architecture".to_string(),
                status: FindingStatus::Info,
                message: arch,
                risk_level: None,
            });
        }

        // Package Manager
        if let Ok(pkg_mgr) = g.inspect_get_package_management(root) {
            findings.push(Finding {
                item: "Package Manager".to_string(),
                status: FindingStatus::Info,
                message: pkg_mgr,
                risk_level: None,
            });
        }

        // Init System
        if let Ok(init) = g.inspect_get_init_system(root) {
            findings.push(Finding {
                item: "Init System".to_string(),
                status: FindingStatus::Info,
                message: init,
                risk_level: None,
            });
        }

        ReportSection {
            title: "Operating System Details".to_string(),
            findings,
        }
    }

    fn analyze_packages(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(pkg_format) = g.inspect_get_package_format(root) {
            findings.push(Finding {
                item: "Package Format".to_string(),
                status: FindingStatus::Info,
                message: pkg_format.clone(),
                risk_level: None,
            });

            // Count packages
            let count = match pkg_format.as_str() {
                "rpm" => g.rpm_list().ok().map(|p| p.len()).unwrap_or(0),
                "deb" => g.dpkg_list().ok().map(|p| p.len()).unwrap_or(0),
                _ => 0,
            };

            findings.push(Finding {
                item: "Total Packages".to_string(),
                status: FindingStatus::Info,
                message: format!("{} packages installed", count),
                risk_level: None,
            });

            // List kernels
            if let Ok(kernels) = g.ls("/boot") {
                let kernel_files: Vec<String> = kernels
                    .iter()
                    .filter(|f| f.starts_with("vmlinuz-") || f.starts_with("vmlinux-"))
                    .map(|s| s.to_string())
                    .collect();

                findings.push(Finding {
                    item: "Installed Kernels".to_string(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} kernel(s): {}",
                        kernel_files.len(),
                        kernel_files.join(", ")
                    ),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Package Inventory".to_string(),
            findings,
        }
    }

    fn analyze_storage(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // LVM configuration
        if let Ok(lvm) = g.inspect_lvm(root) {
            if !lvm.physical_volumes.is_empty() {
                findings.push(Finding {
                    item: "LVM Physical Volumes".to_string(),
                    status: FindingStatus::Info,
                    message: lvm.physical_volumes.join(", "),
                    risk_level: None,
                });
            }

            if !lvm.volume_groups.is_empty() {
                findings.push(Finding {
                    item: "LVM Volume Groups".to_string(),
                    status: FindingStatus::Info,
                    message: lvm
                        .volume_groups
                        .iter()
                        .map(|vg| vg.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    risk_level: None,
                });
            }

            if !lvm.logical_volumes.is_empty() {
                findings.push(Finding {
                    item: "LVM Logical Volumes".to_string(),
                    status: FindingStatus::Info,
                    message: lvm
                        .logical_volumes
                        .iter()
                        .map(|lv| lv.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    risk_level: None,
                });
            }
        }

        // Swap devices
        if let Ok(swap_devices) = g.inspect_swap(root) {
            if !swap_devices.is_empty() {
                findings.push(Finding {
                    item: "Swap Devices".to_string(),
                    status: FindingStatus::Info,
                    message: swap_devices.join(", "),
                    risk_level: None,
                });
            }
        }

        // fstab mounts
        if let Ok(mounts) = g.inspect_fstab(root) {
            findings.push(Finding {
                item: "Mount Points".to_string(),
                status: FindingStatus::Info,
                message: format!("{} entries in /etc/fstab", mounts.len()),
                risk_level: None,
            });

            for (device, mountpoint, fstype) in mounts.iter().take(5) {
                findings.push(Finding {
                    item: mountpoint.clone(),
                    status: FindingStatus::Info,
                    message: format!("{} ({}) -> {}", device, fstype, mountpoint),
                    risk_level: None,
                });
            }
        }

        // Disk usage
        if g.mount(root, "/").is_ok() {
            if let Ok(usage_map) = g.statvfs("/") {
                let blocks = *usage_map.get("blocks").unwrap_or(&0);
                let bsize = *usage_map.get("bsize").unwrap_or(&4096);
                let total_gb = (blocks / 1024).saturating_mul(bsize) / (1024 * 1024);

                findings.push(Finding {
                    item: "Root Filesystem Size".to_string(),
                    status: FindingStatus::Info,
                    message: format!("{} GB", total_gb),
                    risk_level: None,
                });
            }
            g.umount("/").ok();
        }

        ReportSection {
            title: "Storage Configuration".to_string(),
            findings,
        }
    }

    fn analyze_network(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Network interfaces
        if let Ok(interfaces) = g.inspect_network(root) {
            findings.push(Finding {
                item: "Network Interfaces".to_string(),
                status: FindingStatus::Info,
                message: format!("{} interface(s) configured", interfaces.len()),
                risk_level: None,
            });

            for iface in interfaces {
                let dhcp_status = if iface.dhcp { "DHCP" } else { "Static" };
                findings.push(Finding {
                    item: iface.name.clone(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} - IP: {}, MAC: {}",
                        dhcp_status,
                        iface.ip_address.join(", "),
                        iface.mac_address
                    ),
                    risk_level: None,
                });
            }
        }

        // DNS servers
        if let Ok(dns_servers) = g.inspect_dns(root) {
            if !dns_servers.is_empty() {
                findings.push(Finding {
                    item: "DNS Servers".to_string(),
                    status: FindingStatus::Info,
                    message: dns_servers.join(", "),
                    risk_level: None,
                });
            }
        }

        // Hostname
        if let Ok(hostname) = g.inspect_get_hostname(root) {
            findings.push(Finding {
                item: "Hostname".to_string(),
                status: FindingStatus::Info,
                message: hostname,
                risk_level: None,
            });
        }

        ReportSection {
            title: "Network Configuration".to_string(),
            findings,
        }
    }

    fn analyze_custom_services(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Systemd services
        if let Ok(services) = g.inspect_systemd_services(root) {
            findings.push(Finding {
                item: "Enabled Services".to_string(),
                status: FindingStatus::Info,
                message: format!("{} services enabled", services.len()),
                risk_level: None,
            });

            // List key services (first 10)
            for service in services.iter().take(10) {
                findings.push(Finding {
                    item: service.name.clone(),
                    status: FindingStatus::Info,
                    message: format!("Enabled - {}", service.state),
                    risk_level: None,
                });
            }
        }

        // Systemd timers
        if let Ok(timers) = g.inspect_systemd_timers(root) {
            if !timers.is_empty() {
                findings.push(Finding {
                    item: "Systemd Timers".to_string(),
                    status: FindingStatus::Info,
                    message: format!("{} timer(s): {}", timers.len(), timers.join(", ")),
                    risk_level: None,
                });
            }
        }

        // Cron jobs
        if let Ok(cron_jobs) = g.inspect_cron(root) {
            if !cron_jobs.is_empty() {
                findings.push(Finding {
                    item: "Cron Jobs".to_string(),
                    status: FindingStatus::Info,
                    message: format!("{} cron job(s) configured", cron_jobs.len()),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Custom Services & Scheduled Tasks".to_string(),
            findings,
        }
    }

    fn analyze_data_directories(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if g.mount(root, "/").is_ok() {
            // Check common data directories
            let data_dirs = vec!["/opt", "/var", "/srv", "/home"];

            for dir in data_dirs {
                if g.is_dir(dir).unwrap_or(false) {
                    // Try to get directory size (approximate)
                    if let Ok(files) = g.ls(dir) {
                        findings.push(Finding {
                            item: dir.to_string(),
                            status: FindingStatus::Info,
                            message: format!("{} entries", files.len()),
                            risk_level: None,
                        });
                    }
                }
            }

            g.umount("/").ok();
        }

        ReportSection {
            title: "Data Directories".to_string(),
            findings,
        }
    }
}
