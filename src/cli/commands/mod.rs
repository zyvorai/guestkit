// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI commands implementation
//!
//! CLI command handlers receive many parameters from clap argument parsing.
#![allow(clippy::too_many_arguments)]

pub mod analysis;
pub mod assurance;
pub mod batch;
pub mod disk_ops;
pub mod file_ops;
pub mod inspect;
#[cfg(feature = "mcp")]
#[cfg(not(target_os = "windows"))]
pub mod mcp;
pub mod security;
pub mod systemd;
pub mod tools;

pub use analysis::*;
pub use assurance::*;
pub use batch::*;
pub use disk_ops::*;
pub use file_ops::*;
pub use inspect::*;
#[cfg(feature = "mcp")]
#[cfg(not(target_os = "windows"))]
pub use mcp::*;
pub use security::*;
pub use systemd::*;
pub use tools::*;

// Shared imports used by multiple submodules
use crate::cli::formatters::*;
use crate::cli::profiles::{FindingStatus, ProfileReport};
use crate::Guestfs;
use anyhow::Result;

/// Collect inspection data into a structured report
pub(crate) fn collect_inspection_data(
    g: &mut Guestfs,
    root: &str,
    _verbose: bool,
) -> Result<InspectionReport> {
    let mut report = InspectionReport {
        image_path: None,
        os: OsInfo {
            root: root.to_string(),
            os_type: g.inspect_get_type(root).ok(),
            distribution: g.inspect_get_distro(root).ok(),
            product_name: g.inspect_get_product_name(root).ok(),
            architecture: g.inspect_get_arch(root).ok(),
            version: {
                if let (Ok(major), Ok(minor)) = (
                    g.inspect_get_major_version(root),
                    g.inspect_get_minor_version(root),
                ) {
                    Some(VersionInfo { major, minor })
                } else {
                    None
                }
            },
            hostname: g.inspect_get_hostname(root).ok(),
            package_format: g.inspect_get_package_format(root).ok(),
            init_system: g.inspect_get_init_system(root).ok(),
            package_manager: g.inspect_get_package_management(root).ok(),
            format: g.inspect_get_format(root).ok(),
        },
        system_config: Some(SystemConfig {
            timezone: g.inspect_timezone(root).ok(),
            locale: g.inspect_locale(root).ok(),
            selinux: g.inspect_selinux(root).ok(),
            cloud_init: g.inspect_cloud_init(root).ok(),
            vm_tools: g.inspect_vm_tools(root).ok(),
        }),
        network: {
            let interfaces = g.inspect_network(root).ok();
            let dns_servers = g.inspect_dns(root).ok();
            if interfaces.is_some() || dns_servers.is_some() {
                Some(NetworkInfo {
                    interfaces,
                    dns_servers,
                })
            } else {
                None
            }
        },
        users: {
            if let Ok(all_users) = g.inspect_users(root) {
                let regular_users: Vec<_> = all_users
                    .iter()
                    .filter(|u| {
                        u.uid
                            .parse::<u32>()
                            .map(|uid| (1000..65534).contains(&uid))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                let system_users_count = all_users
                    .iter()
                    .filter(|u| {
                        u.uid
                            .parse::<u32>()
                            .map(|uid| uid > 0 && uid < 1000)
                            .unwrap_or(false)
                    })
                    .count();

                Some(UsersInfo {
                    regular_users,
                    system_users_count,
                    total_users: all_users.len(),
                })
            } else {
                None
            }
        },
        ssh: g
            .inspect_ssh_config(root)
            .ok()
            .map(|config| SshConfig { config }),
        services: {
            let enabled_services = g.inspect_systemd_services(root).unwrap_or_default();
            let timers = g.inspect_systemd_timers(root).unwrap_or_default();
            if !enabled_services.is_empty() || !timers.is_empty() {
                Some(ServicesInfo {
                    enabled_services,
                    timers,
                })
            } else {
                None
            }
        },
        runtimes: {
            let language_runtimes = g.inspect_runtimes(root).unwrap_or_default();
            let container_runtimes = g.inspect_container_runtimes(root).unwrap_or_default();
            if !language_runtimes.is_empty() || !container_runtimes.is_empty() {
                Some(RuntimesInfo {
                    language_runtimes,
                    container_runtimes,
                })
            } else {
                None
            }
        },
        storage: {
            let lvm = g.inspect_lvm(root).ok().filter(|l| {
                !l.physical_volumes.is_empty()
                    || !l.volume_groups.is_empty()
                    || !l.logical_volumes.is_empty()
            });
            let swap_devices = g.inspect_swap(root).ok().filter(|s| !s.is_empty());
            let fstab_mounts = g.inspect_fstab(root).ok().map(|mounts| {
                mounts
                    .into_iter()
                    .map(|(device, mountpoint, fstype)| FstabMount {
                        device,
                        mountpoint,
                        fstype,
                    })
                    .collect()
            });

            if lvm.is_some() || swap_devices.is_some() || fstab_mounts.is_some() {
                Some(StorageInfo {
                    lvm,
                    swap_devices,
                    fstab_mounts,
                })
            } else {
                None
            }
        },
        boot: g
            .inspect_boot_config(root)
            .ok()
            .filter(|b| b.bootloader != "unknown"),
        scheduled_tasks: {
            let cron_jobs = g.inspect_cron(root).unwrap_or_default();
            let systemd_timers = g.inspect_systemd_timers(root).unwrap_or_default();
            if !cron_jobs.is_empty() || !systemd_timers.is_empty() {
                Some(ScheduledTasksInfo {
                    cron_jobs,
                    systemd_timers,
                })
            } else {
                None
            }
        },
        security: {
            if let Ok(certs) = g.inspect_certificates(root) {
                let kernel_params = g.inspect_kernel_params(root).unwrap_or_default();
                Some(SecurityInfo {
                    certificates_count: certs.len(),
                    certificate_paths: certs.into_iter().take(5).map(|c| c.path).collect(),
                    kernel_parameters_count: kernel_params.len(),
                })
            } else {
                None
            }
        },
        packages: None,   // Will be filled if we mount and check packages
        disk_usage: None, // Will be filled if we mount and get statvfs
        windows: None,    // Will be filled for Windows systems
    };

    // Try to mount and get additional info (packages, disk usage)
    if g.mount(root, "/").is_ok() {
        // Get disk usage
        if let Ok(usage_map) = g.statvfs("/") {
            let blocks = *usage_map.get("blocks").unwrap_or(&0);
            let bsize = *usage_map.get("bsize").unwrap_or(&4096);
            let bfree = *usage_map.get("bfree").unwrap_or(&0);

            let total_bytes = blocks.saturating_mul(bsize);
            let free_bytes = bfree.saturating_mul(bsize);
            let used_bytes = total_bytes.saturating_sub(free_bytes);
            let used_percent = if total_bytes > 0 {
                (used_bytes as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };

            report.disk_usage = Some(DiskUsageInfo {
                total_bytes,
                used_bytes,
                free_bytes,
                used_percent,
            });
        }

        // Get package info
        if let Ok(pkg_fmt) = g.inspect_get_package_format(root) {
            let count = match pkg_fmt.as_str() {
                "rpm" => g.rpm_list().ok().map(|p| p.len()).unwrap_or(0),
                "deb" => g.dpkg_list().ok().map(|p| p.len()).unwrap_or(0),
                _ => 0,
            };

            let kernels = g
                .ls("/boot")
                .ok()
                .map(|files| {
                    files
                        .iter()
                        .filter(|f| f.starts_with("vmlinuz-") || f.starts_with("vmlinux-"))
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            report.packages = Some(PackagesInfo {
                format: pkg_fmt,
                count,
                kernels,
            });
        }

        g.umount("/").ok();
    }

    // Windows-specific inspection
    if let Some(ref os_type) = report.os.os_type {
        if os_type == "windows" {
            let software = g.inspect_windows_software(root).ok();
            let services = g.inspect_windows_services(root).ok();
            let network_adapters = g.inspect_windows_network(root).ok();
            let updates = g.inspect_windows_updates(root).ok();
            let event_logs = g.inspect_windows_events(root, "System", 10).ok();

            if software.is_some()
                || services.is_some()
                || network_adapters.is_some()
                || updates.is_some()
                || event_logs.is_some()
            {
                report.windows = Some(WindowsInfo {
                    software,
                    services,
                    network_adapters,
                    updates,
                    event_logs,
                });
            }
        }
    }

    Ok(report)
}

/// Print profile report in text format
pub(crate) fn print_profile_report(report: &ProfileReport) {
    println!("Profile: {}", report.profile_name);
    println!();

    for section in &report.sections {
        println!("━━━ {} ━━━", section.title);
        println!();

        for finding in &section.findings {
            let status_symbol = match finding.status {
                FindingStatus::Pass => "✓",
                FindingStatus::Warning => "⚠",
                FindingStatus::Fail => "✗",
                FindingStatus::Info => "ℹ",
            };

            let risk_display = if let Some(risk) = finding.risk_level {
                format!(" [{}]", risk)
            } else {
                String::new()
            };

            println!(
                "  {} {}: {}{}",
                status_symbol, finding.item, finding.message, risk_display
            );
        }
        println!();
    }

    if let Some(summary) = &report.summary {
        println!("━━━ Summary ━━━");
        println!("{}", summary);
        println!();
    }

    if let Some(risk) = report.overall_risk {
        println!("Overall Risk: {}", risk);
    }
}

/// Print an inspection report using the specified format
pub(crate) fn print_inspection_report(
    report: &InspectionReport,
    output_format: Option<OutputFormat>,
    _verbose: bool,
) -> Result<()> {
    // Text format uses JSON pretty-print for cached results since the
    // raw text display requires a live guestfs handle
    let format = output_format.unwrap_or(OutputFormat::Json);
    let formatter = get_formatter(format, true)?;
    let output = formatter.format(report)?;
    println!("{}", output);
    Ok(())
}

/// Initialize a Guestfs handle for read-only inspection of a disk image.
///
/// Creates a new handle, adds the drive read-only, and launches the appliance.
pub(crate) fn init_guestfs_ro(image: &std::path::Path, verbose: bool) -> Result<Guestfs> {
    let mut g = Guestfs::new()?;
    g.set_verbose(verbose);
    g.add_drive_ro(image.to_str().ok_or_else(|| {
        anyhow::anyhow!(
            "Disk image path contains invalid UTF-8: {}",
            image.display()
        )
    })?)?;
    g.launch()?;
    Ok(g)
}

/// Mount all inspected filesystems read-only, sorted by mount path length
/// (shortest first) so parent mounts happen before children.
///
/// Returns the root device string if an OS was found, or None.
pub(crate) fn mount_all_ro(g: &mut Guestfs) -> Option<String> {
    // inspect_os() failing (guestfs launch issue, mount error, permission
    // problem, anything) and "genuinely no OS on this image" both collapse
    // to the same `None` here — callers only ever branch on Some/None, and
    // widening this to Result<Option<String>> would ripple through every
    // one of them. Logging the real error before discarding it at least
    // makes a real failure distinguishable from a clean "no OS" in the
    // caller's logs, instead of silently becoming the same generic
    // "No operating system found" message either way.
    let roots = g.inspect_os().unwrap_or_else(|e| {
        log::warn!("inspect_os() failed, treating as no OS found: {e:#}");
        Vec::new()
    });
    if roots.is_empty() {
        return None;
    }
    let root = roots[0].clone();
    if let Ok(mountpoints) = g.inspect_get_mountpoints(&root) {
        let mut mounts: Vec<_> = mountpoints.iter().collect();
        mounts.sort_by_key(|(mount, _)| mount.len());
        for (mount, device) in mounts {
            if let Err(e) = g.mount_ro(device, mount) {
                // `/` is often already mounted by inspect_get_mountpoints;
                // already_mounted is treated as success inside mount_ro.
                log::warn!("mount_ro({device} -> {mount}) failed: {e:#}");
            }
        }
    }
    Some(root)
}

/// Format a byte size into human-readable string
pub(crate) fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut size = size as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{}{}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}
