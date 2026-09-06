// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Markdown report generation with Mermaid diagrams

use crate::cli::formatters::InspectionReport;
use anyhow::Result;

/// Escape special characters for Markdown table cells
fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('#', "\\#")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape special characters for Mermaid diagram labels
fn mermaid_escape(s: &str) -> String {
    s.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('<', "‹")
        .replace('>', "›")
        .replace('`', "'")
        .replace('{', "(")
        .replace('}', ")")
}

/// Markdown export options
#[derive(Debug, Clone)]
pub struct MarkdownExportOptions {
    /// Include Mermaid diagrams
    pub include_diagrams: bool,
    /// Include table of contents
    pub include_toc: bool,
    /// Include badges/shields
    pub include_badges: bool,
}

impl Default for MarkdownExportOptions {
    fn default() -> Self {
        Self {
            include_diagrams: true,
            include_toc: true,
            include_badges: true,
        }
    }
}

/// Generate Markdown report from inspection data with Mermaid diagrams
pub fn generate_markdown_report(report: &InspectionReport) -> Result<String> {
    generate_markdown_report_with_options(report, MarkdownExportOptions::default())
}

/// Generate Markdown report with custom options
pub fn generate_markdown_report_with_options(
    report: &InspectionReport,
    options: MarkdownExportOptions,
) -> Result<String> {
    let mut md = String::new();

    // Header
    let vm_name = report
        .os
        .hostname
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    md.push_str(&format!("# 🖥️ VM Inspection Report: {}\n\n", vm_name));
    md.push_str(&format!("**Generated:** {}\n\n", timestamp));

    // Add badges if enabled
    if options.include_badges {
        md.push_str(&generate_badges(report));
    }

    md.push_str("---\n\n");

    // Table of Contents
    if options.include_toc {
        md.push_str(&generate_toc(report, options.include_diagrams));
    }

    // Operating System
    md.push_str("## Operating System\n\n");

    if let Some(ref os_type) = report.os.os_type {
        md.push_str(&format!("- **Type:** {}\n", os_type));
    }

    if let Some(ref distro) = report.os.distribution {
        md.push_str(&format!("- **Distribution:** {}\n", distro));
    }

    if let Some(ref version) = report.os.version {
        md.push_str(&format!(
            "- **Version:** {}.{}\n",
            version.major, version.minor
        ));
    }

    if let Some(ref arch) = report.os.architecture {
        md.push_str(&format!("- **Architecture:** {}\n", arch));
    }

    if let Some(ref hostname) = report.os.hostname {
        md.push_str(&format!("- **Hostname:** {}\n", hostname));
    }

    if let Some(ref product) = report.os.product_name {
        md.push_str(&format!("- **Product:** {}\n", product));
    }

    if let Some(ref pkg_format) = report.os.package_format {
        md.push_str(&format!("- **Package Format:** {}\n", pkg_format));
    }

    if let Some(ref pkg_mgmt) = report.os.package_manager {
        md.push_str(&format!("- **Package Management:** {}\n", pkg_mgmt));
    }

    md.push('\n');

    // Packages
    if let Some(ref packages) = report.packages {
        md.push_str("## Packages\n\n");
        md.push_str(&format!("**Total Packages:** {}\n\n", packages.count));

        if !packages.kernels.is_empty() {
            md.push_str("### Installed Kernels\n\n");
            for kernel in &packages.kernels {
                md.push_str(&format!("- {}\n", kernel));
            }
            md.push('\n');
        }
    }

    // Services
    if let Some(ref services) = report.services {
        md.push_str("## Services\n\n");
        md.push_str(&format!(
            "**Enabled Services:** {}\n\n",
            services.enabled_services.len()
        ));

        if !services.enabled_services.is_empty() {
            md.push_str("| Service | State |\n");
            md.push_str("|---------|-------|\n");

            for service in services.enabled_services.iter().take(50) {
                md.push_str(&format!(
                    "| {} | {} |\n",
                    md_escape(&service.name),
                    md_escape(&service.state)
                ));
            }

            if services.enabled_services.len() > 50 {
                md.push_str(&format!(
                    "\n*...and {} more services*\n",
                    services.enabled_services.len() - 50
                ));
            }

            md.push('\n');
        }
    }

    // Users
    if let Some(ref users) = report.users {
        md.push_str("## User Accounts\n\n");
        md.push_str(&format!(
            "**Regular Users:** {}\n",
            users.regular_users.len()
        ));
        md.push_str(&format!(
            "**System Accounts:** {}\n\n",
            users.system_users_count
        ));

        if !users.regular_users.is_empty() {
            md.push_str("### Regular Users\n\n");
            md.push_str("| Username | UID | Home Directory |\n");
            md.push_str("|----------|-----|----------------|\n");

            for user in &users.regular_users {
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    md_escape(&user.username),
                    md_escape(&user.uid),
                    md_escape(&user.home)
                ));
            }

            md.push('\n');
        }
    }

    // Network
    if let Some(ref network) = report.network {
        md.push_str("## Network Configuration\n\n");

        if let Some(ref interfaces) = network.interfaces {
            md.push_str(&format!("**Network Interfaces:** {}\n\n", interfaces.len()));

            md.push_str("| Interface | IP Address | MAC Address | DHCP |\n");
            md.push_str("|-----------|------------|-------------|------|\n");

            for iface in interfaces {
                let dhcp = if iface.dhcp { "Yes" } else { "No" };
                md.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    md_escape(&iface.name),
                    md_escape(&iface.ip_address.join(", ")),
                    md_escape(&iface.mac_address),
                    dhcp
                ));
            }

            md.push('\n');
        }

        if let Some(ref dns) = network.dns_servers {
            if !dns.is_empty() {
                md.push_str("### DNS Servers\n\n");
                for server in dns {
                    md.push_str(&format!("- {}\n", server));
                }
                md.push('\n');
            }
        }
    }

    // Filesystems
    if let Some(ref storage) = report.storage {
        if let Some(ref fstab_mounts) = storage.fstab_mounts {
            md.push_str("## Filesystems\n\n");
            md.push_str(&format!("**Filesystems:** {}\n\n", fstab_mounts.len()));

            md.push_str("| Device | Mount Point | Filesystem Type |\n");
            md.push_str("|--------|-------------|------------------|\n");

            for fs in fstab_mounts {
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    md_escape(&fs.device),
                    md_escape(&fs.mountpoint),
                    md_escape(&fs.fstype)
                ));
            }

            md.push('\n');
        }
    }

    // Storage
    if let Some(ref storage) = report.storage {
        md.push_str("## Storage\n\n");

        if let Some(ref lvm) = storage.lvm {
            if !lvm.physical_volumes.is_empty() {
                md.push_str("### LVM Physical Volumes\n\n");
                for pv in &lvm.physical_volumes {
                    md.push_str(&format!("- {}\n", md_escape(pv)));
                }
                md.push('\n');
            }

            if !lvm.volume_groups.is_empty() {
                md.push_str("### LVM Volume Groups\n\n");
                for vg in &lvm.volume_groups {
                    md.push_str(&format!(
                        "- {} ({}, {} LVs)\n",
                        md_escape(&vg.name),
                        md_escape(&vg.size),
                        vg.lv_count
                    ));
                }
                md.push('\n');
            }

            if !lvm.logical_volumes.is_empty() {
                md.push_str("### LVM Logical Volumes\n\n");
                for lv in &lvm.logical_volumes {
                    md.push_str(&format!(
                        "- {} (VG: {}, Size: {})\n",
                        md_escape(&lv.name),
                        md_escape(&lv.vg_name),
                        md_escape(&lv.size)
                    ));
                }
                md.push('\n');
            }
        }
    }

    // System Configuration
    if let Some(ref config) = report.system_config {
        md.push_str("## System Configuration\n\n");

        if let Some(ref timezone) = config.timezone {
            md.push_str(&format!("- **Timezone:** {}\n", timezone));
        }

        if let Some(ref selinux) = config.selinux {
            md.push_str(&format!("- **SELinux:** {}\n", selinux));
        }

        md.push('\n');
    }

    // System Architecture Diagram (Mermaid)
    if options.include_diagrams {
        md.push_str(&generate_architecture_diagram(report));
    }

    // Network Topology Diagram (Mermaid)
    if options.include_diagrams && report.network.is_some() {
        md.push_str(&generate_network_diagram(report));
    }

    // Storage Hierarchy Diagram (Mermaid)
    if options.include_diagrams && report.storage.is_some() {
        md.push_str(&generate_storage_diagram(report));
    }

    // Footer
    md.push_str("---\n\n");
    md.push_str("*Generated by GuestKit - Pure Rust VM Inspection Toolkit*\n\n");
    md.push_str(
        "> **Note:** Mermaid diagrams are rendered on GitHub, GitLab, and many Markdown viewers.\n",
    );

    Ok(md)
}

/// Generate badges section
fn generate_badges(report: &InspectionReport) -> String {
    let mut badges = String::new();

    if let Some(ref os_type) = report.os.os_type {
        badges.push_str(&format!(
            "![OS](https://img.shields.io/badge/OS-{}-blue) ",
            os_type.replace(' ', "%20")
        ));
    }

    if let Some(ref distro) = report.os.distribution {
        badges.push_str(&format!(
            "![Distribution](https://img.shields.io/badge/Distribution-{}-green) ",
            distro.replace(' ', "%20")
        ));
    }

    if let Some(ref arch) = report.os.architecture {
        badges.push_str(&format!(
            "![Architecture](https://img.shields.io/badge/Architecture-{}-orange) ",
            arch.replace(' ', "%20")
        ));
    }

    if let Some(ref packages) = report.packages {
        badges.push_str(&format!(
            "![Packages](https://img.shields.io/badge/Packages-{}-purple)",
            packages.count
        ));
    }

    badges.push_str("\n\n");
    badges
}

/// Generate table of contents
fn generate_toc(report: &InspectionReport, include_diagrams: bool) -> String {
    let mut toc = String::from("## 📑 Table of Contents\n\n");

    toc.push_str("- [Operating System](#operating-system)\n");

    if include_diagrams {
        toc.push_str("- [System Architecture](#system-architecture)\n");
    }

    if report.packages.is_some() {
        toc.push_str("- [Packages](#packages)\n");
    }

    if report.services.is_some() {
        toc.push_str("- [Services](#services)\n");
    }

    if report.users.is_some() {
        toc.push_str("- [User Accounts](#user-accounts)\n");
    }

    if report.network.is_some() {
        toc.push_str("- [Network Configuration](#network-configuration)\n");
        if include_diagrams {
            toc.push_str("- [Network Topology](#network-topology)\n");
        }
    }

    if report.storage.is_some() {
        toc.push_str("- [Filesystems](#filesystems)\n");
        toc.push_str("- [Storage](#storage)\n");
        if include_diagrams {
            toc.push_str("- [Storage Hierarchy](#storage-hierarchy)\n");
        }
    }

    if report.system_config.is_some() {
        toc.push_str("- [System Configuration](#system-configuration)\n");
    }

    toc.push_str("\n---\n\n");
    toc
}

/// Generate system architecture Mermaid diagram
fn generate_architecture_diagram(report: &InspectionReport) -> String {
    let mut diagram = String::from("## 🏗️ System Architecture\n\n");
    diagram.push_str("```mermaid\ngraph TB\n");

    let vm_name = report
        .os
        .hostname
        .clone()
        .unwrap_or_else(|| "VM".to_string());

    diagram.push_str(&format!(
        "    VM[\"{} Virtual Machine\"]\n",
        mermaid_escape(&vm_name)
    ));

    // OS Layer
    if let Some(ref distro) = report.os.distribution {
        diagram.push_str(&format!(
            "    OS[\"Operating System<br/>{}",
            mermaid_escape(distro)
        ));
        if let Some(ref version) = report.os.version {
            diagram.push_str(&format!(" {}.{}", version.major, version.minor));
        }
        diagram.push_str("\"]\n");
        diagram.push_str("    VM --> OS\n");
    }

    // Package Management
    if let Some(ref pkg_mgr) = report.os.package_manager {
        diagram.push_str(&format!(
            "    PKG[\"Package Manager<br/>{}\"]\n",
            mermaid_escape(pkg_mgr)
        ));
        diagram.push_str("    OS --> PKG\n");
    }

    // Services
    if let Some(ref services) = report.services {
        diagram.push_str(&format!(
            "    SVC[\"Services<br/>{} enabled\"]\n",
            services.enabled_services.len()
        ));
        diagram.push_str("    OS --> SVC\n");
    }

    // Network
    if let Some(ref network) = report.network {
        if let Some(ref interfaces) = network.interfaces {
            diagram.push_str(&format!(
                "    NET[\"Network<br/>{} interfaces\"]\n",
                interfaces.len()
            ));
            diagram.push_str("    OS --> NET\n");
        }
    }

    // Storage
    if let Some(ref storage) = report.storage {
        if let Some(ref fstab) = storage.fstab_mounts {
            diagram.push_str(&format!(
                "    FS[\"Filesystems<br/>{} mounted\"]\n",
                fstab.len()
            ));
            diagram.push_str("    OS --> FS\n");
        }
    }

    // Users
    if let Some(ref users) = report.users {
        diagram.push_str(&format!(
            "    USR[\"Users<br/>{} accounts\"]\n",
            users.total_users
        ));
        diagram.push_str("    OS --> USR\n");
    }

    diagram.push_str("```\n\n");
    diagram
}

/// Generate network topology Mermaid diagram
fn generate_network_diagram(report: &InspectionReport) -> String {
    let mut diagram = String::from("## 🌐 Network Topology\n\n");
    diagram.push_str("```mermaid\ngraph LR\n");

    if let Some(ref network) = report.network {
        diagram.push_str("    VM[Virtual Machine]\n");

        if let Some(ref interfaces) = network.interfaces {
            for (idx, iface) in interfaces.iter().take(10).enumerate() {
                let iface_id = format!("IF{}", idx);
                diagram.push_str(&format!(
                    "    {}[\"{}\"]\n",
                    iface_id,
                    mermaid_escape(&iface.name)
                ));

                let ip_list = if !iface.ip_address.is_empty() {
                    iface.ip_address.join("<br/>")
                } else {
                    "No IP".to_string()
                };

                diagram.push_str(&format!(
                    "    IP{}[\"{}\"]\n",
                    idx,
                    mermaid_escape(&ip_list)
                ));

                diagram.push_str(&format!("    VM --> {}\n", iface_id));
                diagram.push_str(&format!("    {} --> IP{}\n", iface_id, idx));

                if iface.dhcp {
                    diagram.push_str(&format!("    IP{} -.DHCP.-> Network\n", idx));
                } else {
                    diagram.push_str(&format!("    IP{} --> Network\n", idx));
                }
            }
        }

        if let Some(ref dns_servers) = network.dns_servers {
            if !dns_servers.is_empty() {
                diagram.push_str("    DNS[\"DNS Servers\"]\n");
                diagram.push_str("    VM -.-> DNS\n");
            }
        }
    }

    diagram.push_str("```\n\n");
    diagram
}

/// Generate storage hierarchy Mermaid diagram
fn generate_storage_diagram(report: &InspectionReport) -> String {
    let mut diagram = String::from("## 💾 Storage Hierarchy\n\n");
    diagram.push_str("```mermaid\ngraph TB\n");

    if let Some(ref storage) = report.storage {
        diagram.push_str("    DISK[Physical Disk]\n");

        // LVM hierarchy
        if let Some(ref lvm) = storage.lvm {
            if !lvm.physical_volumes.is_empty() {
                for (idx, pv) in lvm.physical_volumes.iter().take(5).enumerate() {
                    diagram.push_str(&format!("    PV{}[\"PV: {}\"]\n", idx, mermaid_escape(pv)));
                    diagram.push_str(&format!("    DISK --> PV{}\n", idx));
                }
            }

            if !lvm.volume_groups.is_empty() {
                for (idx, vg) in lvm.volume_groups.iter().take(5).enumerate() {
                    diagram.push_str(&format!(
                        "    VG{}[\"VG: {}\"]\n",
                        idx,
                        mermaid_escape(&vg.name)
                    ));
                    if idx < lvm.physical_volumes.len() {
                        diagram.push_str(&format!("    PV{} --> VG{}\n", idx, idx));
                    } else {
                        diagram.push_str(&format!("    PV0 --> VG{}\n", idx));
                    }
                }
            }

            if !lvm.logical_volumes.is_empty() {
                for (idx, lv) in lvm.logical_volumes.iter().take(5).enumerate() {
                    diagram.push_str(&format!(
                        "    LV{}[\"LV: {}\"]\n",
                        idx,
                        mermaid_escape(&lv.name)
                    ));
                    if idx < lvm.volume_groups.len() {
                        diagram.push_str(&format!("    VG{} --> LV{}\n", idx, idx));
                    } else {
                        diagram.push_str(&format!("    VG0 --> LV{}\n", idx));
                    }
                }
            }
        }

        // Filesystems
        if let Some(ref fstab) = storage.fstab_mounts {
            for (idx, fs) in fstab.iter().take(5).enumerate() {
                diagram.push_str(&format!(
                    "    FS{}[\"{}\"]\n",
                    idx,
                    mermaid_escape(&fs.mountpoint)
                ));

                // Try to connect to LVM if present
                if let Some(ref lvm) = storage.lvm {
                    if !lvm.logical_volumes.is_empty() && idx < lvm.logical_volumes.len() {
                        diagram.push_str(&format!("    LV{} --> FS{}\n", idx, idx));
                    } else {
                        diagram.push_str(&format!("    DISK --> FS{}\n", idx));
                    }
                } else {
                    diagram.push_str(&format!("    DISK --> FS{}\n", idx));
                }
            }
        }
    }

    diagram.push_str("```\n\n");
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::formatters::{OsInfo, VersionInfo};

    #[test]
    fn test_markdown_export_options_default() {
        let options = MarkdownExportOptions::default();
        assert!(options.include_diagrams);
        assert!(options.include_toc);
        assert!(options.include_badges);
    }

    #[test]
    fn test_generate_badges() {
        let report = InspectionReport {
            image_path: None,
            os: OsInfo {
                root: "/".to_string(),
                os_type: Some("linux".to_string()),
                distribution: Some("ubuntu".to_string()),
                product_name: None,
                architecture: Some("x86_64".to_string()),
                version: None,
                hostname: None,
                package_format: None,
                init_system: None,
                package_manager: None,
                format: None,
            },
            system_config: None,
            network: None,
            users: None,
            ssh: None,
            services: None,
            packages: None,
            storage: None,
            security: None,
            runtimes: None,
            boot: None,
            scheduled_tasks: None,
            disk_usage: None,
            windows: None,
        };

        let badges = generate_badges(&report);
        assert!(badges.contains("OS-linux"));
        assert!(badges.contains("Distribution-ubuntu"));
        assert!(badges.contains("Architecture-x86_64"));
    }

    #[test]
    fn test_generate_architecture_diagram() {
        let report = InspectionReport {
            image_path: None,
            os: OsInfo {
                root: "/".to_string(),
                os_type: Some("linux".to_string()),
                distribution: Some("ubuntu".to_string()),
                product_name: None,
                architecture: None,
                version: Some(VersionInfo {
                    major: 22,
                    minor: 4,
                }),
                hostname: Some("test-vm".to_string()),
                package_format: None,
                init_system: None,
                package_manager: Some("apt".to_string()),
                format: None,
            },
            system_config: None,
            network: None,
            users: None,
            ssh: None,
            services: None,
            packages: None,
            storage: None,
            security: None,
            runtimes: None,
            boot: None,
            scheduled_tasks: None,
            disk_usage: None,
            windows: None,
        };

        let diagram = generate_architecture_diagram(&report);
        assert!(diagram.contains("```mermaid"));
        assert!(diagram.contains("test-vm Virtual Machine"));
        assert!(diagram.contains("Operating System"));
        assert!(diagram.contains("ubuntu"));
        assert!(diagram.contains("Package Manager"));
        assert!(diagram.contains("apt"));
    }

    #[test]
    fn test_generate_markdown_report_basic() {
        let report = InspectionReport {
            image_path: None,
            os: OsInfo {
                root: "/".to_string(),
                os_type: Some("linux".to_string()),
                distribution: Some("ubuntu".to_string()),
                product_name: None,
                architecture: Some("x86_64".to_string()),
                version: Some(VersionInfo {
                    major: 22,
                    minor: 4,
                }),
                hostname: Some("test-vm".to_string()),
                package_format: None,
                init_system: None,
                package_manager: None,
                format: None,
            },
            system_config: None,
            network: None,
            users: None,
            ssh: None,
            services: None,
            packages: None,
            storage: None,
            security: None,
            runtimes: None,
            boot: None,
            scheduled_tasks: None,
            disk_usage: None,
            windows: None,
        };

        let result = generate_markdown_report(&report);
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.contains("# 🖥️ VM Inspection Report: test-vm"));
        assert!(markdown.contains("## Operating System"));
        assert!(markdown.contains("ubuntu"));
        assert!(markdown.contains("Mermaid diagrams"));
    }

    #[test]
    fn test_generate_markdown_without_diagrams() {
        let report = InspectionReport {
            image_path: None,
            os: OsInfo {
                root: "/".to_string(),
                os_type: Some("linux".to_string()),
                distribution: Some("ubuntu".to_string()),
                product_name: None,
                architecture: None,
                version: None,
                hostname: Some("test-vm".to_string()),
                package_format: None,
                init_system: None,
                package_manager: None,
                format: None,
            },
            system_config: None,
            network: None,
            users: None,
            ssh: None,
            services: None,
            packages: None,
            storage: None,
            security: None,
            runtimes: None,
            boot: None,
            scheduled_tasks: None,
            disk_usage: None,
            windows: None,
        };

        let options = MarkdownExportOptions {
            include_diagrams: false,
            include_toc: true,
            include_badges: false,
        };

        let result = generate_markdown_report_with_options(&report, options);
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(!markdown.contains("```mermaid"));
        assert!(markdown.contains("## 📑 Table of Contents"));
    }
}
