// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTML report generation with Chart.js visualizations

use crate::cli::formatters::InspectionReport;
use crate::export::html::{
    FilesystemInfo, InspectionData, NetworkInterface, PackageInfo, UserInfo,
};
use crate::export::{HtmlExportOptions, HtmlExporter};
use anyhow::Result;
use tempfile::NamedTempFile;

/// Generate HTML report from inspection data using Chart.js
pub fn generate_html_report(report: &InspectionReport) -> Result<String> {
    // Convert InspectionReport to InspectionData for the HTML exporter
    let data = convert_to_inspection_data(report);

    // Create HTML exporter with custom options
    let exporter = HtmlExporter::with_options(HtmlExportOptions {
        include_charts: true,
        include_styles: true,
        dark_theme: false,
        include_toc: true,
        responsive: true,
    });

    // Generate HTML to a temporary file
    let temp_file = NamedTempFile::new()?;
    exporter.generate(temp_file.path(), &data)?;

    // Read the generated HTML
    let html = std::fs::read_to_string(temp_file.path())?;

    Ok(html)
}

/// Convert InspectionReport to InspectionData
fn convert_to_inspection_data(report: &InspectionReport) -> InspectionData {
    let hostname = report.os.hostname.as_deref().unwrap_or("Unknown");
    let os_type = report.os.os_type.as_deref().unwrap_or("Unknown");
    let distribution = report.os.distribution.as_deref().unwrap_or("Unknown");
    let version = if let Some(ref v) = report.os.version {
        format!("{}.{}", v.major, v.minor)
    } else {
        "Unknown".to_string()
    };
    let architecture = report.os.architecture.as_deref().unwrap_or("Unknown");
    let product_name = report.os.product_name.as_deref().unwrap_or("Unknown");
    let package_format = report.os.package_format.as_deref().unwrap_or("Unknown");
    let package_manager = report.os.package_manager.as_deref().unwrap_or("Unknown");

    // Convert filesystems from storage/fstab_mounts
    let filesystems = if let Some(ref storage_section) = report.storage {
        if let Some(ref mounts) = storage_section.fstab_mounts {
            mounts
                .iter()
                .map(|fs| {
                    FilesystemInfo {
                        device: fs.device.clone(),
                        mountpoint: fs.mountpoint.clone(),
                        fstype: fs.fstype.clone(),
                        size: 0,      // Size not available in fstab
                        used: 0,      // Used not available in fstab
                        available: 0, // Available not available in fstab
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Convert packages
    let packages = if let Some(ref pkg_section) = report.packages {
        pkg_section
            .kernels
            .iter()
            .take(100)
            .map(|k| PackageInfo {
                name: k.clone(),
                version: format!("{} package", pkg_section.format),
                arch: architecture.to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };

    // Convert users
    let users = if let Some(ref user_section) = report.users {
        user_section
            .regular_users
            .iter()
            .map(|u| UserInfo {
                username: u.username.clone(),
                uid: u.uid.clone(),
                home: u.home.clone(),
                shell: u.shell.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    // Convert network interfaces
    let interfaces = if let Some(ref net_section) = report.network {
        if let Some(ref intfs) = net_section.interfaces {
            intfs
                .iter()
                .map(|i| {
                    NetworkInterface {
                        name: i.name.clone(),
                        mac_address: i.mac_address.clone(),
                        ip_addresses: i.ip_address.join(", "),
                        state: "up".to_string(), // Assume up if listed
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    InspectionData {
        hostname: hostname.to_string(),
        os_type: os_type.to_string(),
        distribution: distribution.to_string(),
        version,
        architecture: architecture.to_string(),
        product_name: product_name.to_string(),
        package_format: package_format.to_string(),
        package_manager: package_manager.to_string(),
        kernel_version: None, // Not available in current report format
        total_memory: None,   // Not available in current report format
        vcpus: None,          // Not available in current report format
        filesystems,
        packages,
        users,
        interfaces,
    }
}
