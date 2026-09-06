// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Output formatters for inspection results

use crate::guestfs::inspect_enhanced::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Comprehensive inspection report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    pub os: OsInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_config: Option<SystemConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<UsersInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<ServicesInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtimes: Option<RuntimesInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot: Option<BootConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_tasks: Option<ScheduledTasksInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<PackagesInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_usage: Option<DiskUsageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<WindowsInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<Vec<WindowsApplication>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<WindowsService>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_adapters: Option<Vec<WindowsNetworkAdapter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updates: Option<Vec<WindowsUpdate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_logs: Option<Vec<WindowsEventLogEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub major: i32,
    pub minor: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selinux: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<Vec<NetworkInterface>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_servers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersInfo {
    pub regular_users: Vec<UserAccount>,
    pub system_users_count: usize,
    pub total_users: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesInfo {
    pub enabled_services: Vec<SystemService>,
    pub timers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimesInfo {
    pub language_runtimes: HashMap<String, String>,
    pub container_runtimes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lvm: Option<LVMInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_devices: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fstab_mounts: Option<Vec<FstabMount>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FstabMount {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTasksInfo {
    pub cron_jobs: Vec<String>,
    pub systemd_timers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub certificates_count: usize,
    pub certificate_paths: Vec<String>,
    pub kernel_parameters_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesInfo {
    pub format: String,
    pub count: usize,
    pub kernels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsageInfo {
    pub total_bytes: i64,
    pub used_bytes: i64,
    pub free_bytes: i64,
    pub used_percent: f64,
}

/// Output format enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
    Csv,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "yaml" | "yml" => Ok(OutputFormat::Yaml),
            "csv" => Ok(OutputFormat::Csv),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

/// Trait for formatting inspection results
pub trait OutputFormatter {
    fn format(&self, report: &InspectionReport) -> Result<String>;
}

/// JSON output formatter
pub struct JsonFormatter {
    pub pretty: bool,
}

impl OutputFormatter for JsonFormatter {
    fn format(&self, report: &InspectionReport) -> Result<String> {
        let result = if self.pretty {
            serde_json::to_string_pretty(report)?
        } else {
            serde_json::to_string(report)?
        };
        Ok(result)
    }
}

/// YAML output formatter
pub struct YamlFormatter;

impl OutputFormatter for YamlFormatter {
    fn format(&self, report: &InspectionReport) -> Result<String> {
        let result = serde_yaml::to_string(report)?;
        Ok(result)
    }
}

/// CSV output formatter (for tabular data like users, packages)
pub struct CsvFormatter {
    pub data_type: CsvDataType,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum CsvDataType {
    Users,
    Services,
    Packages,
}

impl OutputFormatter for CsvFormatter {
    fn format(&self, report: &InspectionReport) -> Result<String> {
        let mut wtr = csv::Writer::from_writer(vec![]);

        match self.data_type {
            CsvDataType::Users => {
                if let Some(users) = &report.users {
                    wtr.write_record(["username", "uid", "gid", "home", "shell"])?;
                    for user in &users.regular_users {
                        wtr.write_record([
                            &user.username,
                            &user.uid,
                            &user.gid,
                            &user.home,
                            &user.shell,
                        ])?;
                    }
                }
            }
            CsvDataType::Services => {
                if let Some(services) = &report.services {
                    wtr.write_record(["name", "enabled", "state"])?;
                    for service in &services.enabled_services {
                        wtr.write_record([
                            &service.name,
                            &service.enabled.to_string(),
                            &service.state,
                        ])?;
                    }
                }
            }
            CsvDataType::Packages => {
                if let Some(packages) = &report.packages {
                    wtr.write_record(["kernel_version", "format", "total_count"])?;
                    for kernel in &packages.kernels {
                        wtr.write_record([kernel, &packages.format, &packages.count.to_string()])?;
                    }
                }
            }
        }

        wtr.flush()?;
        let data = String::from_utf8(wtr.into_inner()?)?;
        Ok(data)
    }
}

/// Get formatter for output format
pub fn get_formatter(format: OutputFormat, pretty: bool) -> Result<Box<dyn OutputFormatter>> {
    match format {
        OutputFormat::Json => Ok(Box::new(JsonFormatter { pretty })),
        OutputFormat::Yaml => Ok(Box::new(YamlFormatter)),
        OutputFormat::Text => Err(anyhow::anyhow!(
            "Text format should use existing display logic"
        )),
        OutputFormat::Csv => Ok(Box::new(CsvFormatter {
            data_type: CsvDataType::Users,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_output_format_from_str_text() {
        let format = OutputFormat::from_str("text").unwrap();
        assert_eq!(format, OutputFormat::Text);
    }

    #[test]
    fn test_output_format_from_str_json() {
        let format = OutputFormat::from_str("json").unwrap();
        assert_eq!(format, OutputFormat::Json);
    }

    #[test]
    fn test_output_format_from_str_yaml() {
        assert_eq!(OutputFormat::from_str("yaml").unwrap(), OutputFormat::Yaml);
        assert_eq!(OutputFormat::from_str("yml").unwrap(), OutputFormat::Yaml);
    }

    #[test]
    fn test_output_format_from_str_csv() {
        let format = OutputFormat::from_str("csv").unwrap();
        assert_eq!(format, OutputFormat::Csv);
    }

    #[test]
    fn test_output_format_from_str_case_insensitive() {
        assert!(OutputFormat::from_str("JSON").is_ok());
        assert!(OutputFormat::from_str("Text").is_ok());
        assert!(OutputFormat::from_str("YAML").is_ok());
        assert!(OutputFormat::from_str("CSV").is_ok());
    }

    #[test]
    fn test_output_format_from_str_invalid() {
        assert!(OutputFormat::from_str("xml").is_err());
        assert!(OutputFormat::from_str("html").is_err());
        assert!(OutputFormat::from_str("").is_err());
    }

    #[test]
    fn test_version_info_creation() {
        let version = VersionInfo {
            major: 22,
            minor: 4,
        };

        assert_eq!(version.major, 22);
        assert_eq!(version.minor, 4);
    }

    #[test]
    fn test_os_info_creation() {
        let os = OsInfo {
            root: "/dev/sda1".to_string(),
            os_type: Some("linux".to_string()),
            distribution: Some("ubuntu".to_string()),
            product_name: Some("Ubuntu 22.04 LTS".to_string()),
            architecture: Some("x86_64".to_string()),
            version: Some(VersionInfo {
                major: 22,
                minor: 4,
            }),
            hostname: Some("test-host".to_string()),
            package_format: Some("deb".to_string()),
            init_system: Some("systemd".to_string()),
            package_manager: Some("apt".to_string()),
            format: Some("qcow2".to_string()),
        };

        assert_eq!(os.root, "/dev/sda1");
        assert_eq!(os.os_type, Some("linux".to_string()));
        assert_eq!(os.distribution, Some("ubuntu".to_string()));
        assert!(os.version.is_some());
    }

    #[test]
    fn test_system_config_creation() {
        let config = SystemConfig {
            timezone: Some("UTC".to_string()),
            locale: Some("en_US.UTF-8".to_string()),
            selinux: Some("enforcing".to_string()),
            cloud_init: Some(true),
            vm_tools: Some(vec!["qemu-guest-agent".to_string()]),
        };

        assert_eq!(config.timezone, Some("UTC".to_string()));
        assert_eq!(config.cloud_init, Some(true));
        assert_eq!(config.vm_tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_get_formatter_json_pretty() {
        // Just ensure formatter can be created without panicking
        let _formatter = get_formatter(OutputFormat::Json, true);
        // Successfully created a JsonFormatter
    }

    #[test]
    fn test_get_formatter_json_compact() {
        // Just ensure formatter can be created without panicking
        let _formatter = get_formatter(OutputFormat::Json, false);
        // Successfully created a JsonFormatter
    }

    #[test]
    fn test_get_formatter_yaml() {
        // Just ensure formatter can be created without panicking
        let _formatter = get_formatter(OutputFormat::Yaml, false);
        // Successfully created a YamlFormatter
    }

    #[test]
    fn test_get_formatter_csv() {
        // Just ensure formatter can be created without panicking
        let _formatter = get_formatter(OutputFormat::Csv, false);
        // Successfully created a CsvFormatter
    }

    #[test]
    fn test_windows_info_creation() {
        let windows = WindowsInfo {
            software: None,
            services: None,
            network_adapters: None,
            updates: None,
            event_logs: None,
        };

        assert!(windows.software.is_none());
        assert!(windows.services.is_none());
    }
}
