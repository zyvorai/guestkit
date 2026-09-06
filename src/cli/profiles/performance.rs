// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance tuning profile

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection};
use crate::Guestfs;
use anyhow::Result;

pub struct PerformanceProfile;

impl InspectionProfile for PerformanceProfile {
    fn name(&self) -> &str {
        "performance"
    }

    fn description(&self) -> &str {
        "Performance tuning opportunities and bottleneck detection"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            // Section 1: Kernel Parameters
            self.analyze_kernel_params(g, root),
            // Section 2: Swap Configuration
            self.analyze_swap(g, root),
            // Section 3: Disk I/O
            self.analyze_disk_io(g, root),
            // Section 4: Network Tuning
            self.analyze_network_tuning(g, root),
            // Section 5: Services & Resources
            self.analyze_services(g, root),
        ];

        Ok(ProfileReport {
            profile_name: "Performance Tuning".to_string(),
            sections,
            overall_risk: None,
            summary: Some(
                "Review tuning opportunities to optimize system performance.".to_string(),
            ),
        })
    }
}

impl PerformanceProfile {
    fn analyze_kernel_params(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(params) = g.inspect_kernel_params(root) {
            findings.push(Finding {
                item: "Kernel Parameters".to_string(),
                status: FindingStatus::Info,
                message: format!("{} parameters configured", params.len()),
                risk_level: None,
            });

            // Check for common performance-related parameters
            let perf_params = vec![
                ("vm.swappiness", "60"),
                ("net.core.somaxconn", "4096"),
                ("fs.file-max", "1000000"),
                ("net.ipv4.tcp_max_syn_backlog", "8192"),
            ];

            for (param, recommended) in perf_params {
                if let Some(value) = params.get(param) {
                    findings.push(Finding {
                        item: param.to_string(),
                        status: FindingStatus::Info,
                        message: value.clone(),
                        risk_level: None,
                    });
                } else {
                    findings.push(Finding {
                        item: param.to_string(),
                        status: FindingStatus::Warning,
                        message: format!("Not configured (consider setting to {})", recommended),
                        risk_level: None,
                    });
                }
            }
        }

        ReportSection {
            title: "Kernel Parameters".to_string(),
            findings,
        }
    }

    fn analyze_swap(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(swap_devices) = g.inspect_swap(root) {
            if swap_devices.is_empty() {
                findings.push(Finding {
                    item: "Swap Space".to_string(),
                    status: FindingStatus::Warning,
                    message: "No swap space configured".to_string(),
                    risk_level: None,
                });
            } else {
                findings.push(Finding {
                    item: "Swap Devices".to_string(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} swap device(s): {}",
                        swap_devices.len(),
                        swap_devices.join(", ")
                    ),
                    risk_level: None,
                });
            }
        }

        // Check swappiness parameter
        if let Ok(params) = g.inspect_kernel_params(root) {
            if let Some(swappiness_val) = params.get("vm.swappiness") {
                let value: i32 = swappiness_val.parse().unwrap_or(60);

                if value > 60 {
                    findings.push(Finding {
                        item: "Swappiness".to_string(),
                        status: FindingStatus::Warning,
                        message: format!(
                            "vm.swappiness = {} (high - consider lowering to 10-30 for better performance)",
                            value
                        ),
                        risk_level: None,
                    });
                } else {
                    findings.push(Finding {
                        item: "Swappiness".to_string(),
                        status: FindingStatus::Pass,
                        message: format!("vm.swappiness = {}", value),
                        risk_level: None,
                    });
                }
            }
        }

        ReportSection {
            title: "Swap Configuration".to_string(),
            findings,
        }
    }

    fn analyze_disk_io(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check mount options in fstab
        if let Ok(mounts) = g.inspect_fstab(root) {
            findings.push(Finding {
                item: "Filesystem Mounts".to_string(),
                status: FindingStatus::Info,
                message: format!("{} mount points in fstab", mounts.len()),
                risk_level: None,
            });

            // Look for performance-related mount options
            for (device, mountpoint, fstype) in mounts.iter().take(5) {
                // Check if using performance-optimized options
                findings.push(Finding {
                    item: mountpoint.clone(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} ({}) - check for noatime, nodiratime options",
                        device, fstype
                    ),
                    risk_level: None,
                });
            }
        }

        // Check for LVM (which can impact performance)
        if let Ok(lvm) = g.inspect_lvm(root) {
            if !lvm.logical_volumes.is_empty() {
                findings.push(Finding {
                    item: "LVM Configuration".to_string(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} logical volumes (consider stripe configuration for performance)",
                        lvm.logical_volumes.len()
                    ),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Disk I/O Configuration".to_string(),
            findings,
        }
    }

    fn analyze_network_tuning(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(params) = g.inspect_kernel_params(root) {
            // Network buffer sizes
            let net_params = vec![
                "net.core.rmem_max",
                "net.core.wmem_max",
                "net.ipv4.tcp_rmem",
                "net.ipv4.tcp_wmem",
                "net.core.netdev_max_backlog",
            ];

            for param in net_params {
                if let Some(value) = params.get(param) {
                    findings.push(Finding {
                        item: param.to_string(),
                        status: FindingStatus::Info,
                        message: value.clone(),
                        risk_level: None,
                    });
                } else {
                    findings.push(Finding {
                        item: param.to_string(),
                        status: FindingStatus::Warning,
                        message: "Not tuned (consider optimizing for high-throughput workloads)"
                            .to_string(),
                        risk_level: None,
                    });
                }
            }
        }

        // Check if network interfaces use DHCP or static
        if let Ok(interfaces) = g.inspect_network(root) {
            for iface in interfaces {
                let config_type = if iface.dhcp { "DHCP" } else { "Static" };
                findings.push(Finding {
                    item: iface.name.clone(),
                    status: FindingStatus::Info,
                    message: format!("{} configuration", config_type),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Network Tuning".to_string(),
            findings,
        }
    }

    fn analyze_services(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(services) = g.inspect_systemd_services(root) {
            findings.push(Finding {
                item: "Enabled Services".to_string(),
                status: FindingStatus::Info,
                message: format!(
                    "{} services enabled (review for unnecessary services)",
                    services.len()
                ),
                risk_level: None,
            });

            // Identify potentially resource-heavy services
            let heavy_services = vec![
                "postgresql",
                "mysql",
                "mariadb",
                "docker",
                "containerd",
                "elasticsearch",
                "mongod",
                "redis",
                "apache2",
                "nginx",
            ];

            for heavy in &heavy_services {
                if let Some(service) = services.iter().find(|s| s.name.contains(heavy)) {
                    findings.push(Finding {
                        item: service.name.clone(),
                        status: FindingStatus::Info,
                        message: "Resource-intensive service detected - ensure proper resource allocation".to_string(),
                        risk_level: None,
                    });
                }
            }
        }

        // Check for timers that might impact performance
        if let Ok(timers) = g.inspect_systemd_timers(root) {
            if !timers.is_empty() {
                findings.push(Finding {
                    item: "Scheduled Tasks".to_string(),
                    status: FindingStatus::Info,
                    message: format!(
                        "{} timer(s) - review scheduling to avoid peak load times",
                        timers.len()
                    ),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Services & Resource Usage".to_string(),
            findings,
        }
    }
}
