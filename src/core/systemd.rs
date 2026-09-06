// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd analysis and inspection tools

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod boot;
pub mod journal;
pub mod services;

/// Systemd journal entry
#[derive(Debug, Clone)]
pub struct JournalEntry {
    /// Timestamp (microseconds since epoch)
    pub timestamp: u64,
    /// Priority level (0-7, syslog levels)
    pub priority: u8,
    /// Service/unit name
    pub unit: Option<String>,
    /// Message text
    pub message: String,
    /// Process ID
    pub pid: Option<u32>,
    /// Additional fields
    pub fields: HashMap<String, String>,
}

impl JournalEntry {
    /// Get priority as string
    pub fn priority_str(&self) -> &'static str {
        match self.priority {
            0 => "EMERG",
            1 => "ALERT",
            2 => "CRIT",
            3 => "ERR",
            4 => "WARNING",
            5 => "NOTICE",
            6 => "INFO",
            7 => "DEBUG",
            _ => "UNKNOWN",
        }
    }

    /// Format timestamp as human-readable string
    pub fn timestamp_str(&self) -> String {
        let secs = self.timestamp / 1_000_000;
        let secs_i64 = i64::try_from(secs).unwrap_or(i64::MAX);
        chrono::DateTime::<chrono::Utc>::from_timestamp(secs_i64, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }
}

/// Systemd service information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceInfo {
    /// Service name
    pub name: String,
    /// Service state (active, inactive, failed, etc.)
    pub state: ServiceState,
    /// Unit file path
    pub unit_file: Option<PathBuf>,
    /// Service description
    pub description: Option<String>,
    /// Dependencies (Requires, Wants, After, Before)
    pub dependencies: ServiceDependencies,
    /// Is enabled for auto-start
    pub enabled: bool,
    /// Main PID (if running)
    pub main_pid: Option<u32>,
}

/// Service state
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ServiceState {
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Unknown,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceState::Active => write!(f, "active"),
            ServiceState::Inactive => write!(f, "inactive"),
            ServiceState::Failed => write!(f, "failed"),
            ServiceState::Activating => write!(f, "activating"),
            ServiceState::Deactivating => write!(f, "deactivating"),
            ServiceState::Unknown => write!(f, "unknown"),
        }
    }
}

/// Service dependencies
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ServiceDependencies {
    /// Services that must be started before this one
    pub requires: Vec<String>,
    /// Services that should be started before this one (optional)
    pub wants: Vec<String>,
    /// Services that must start before this one
    pub after: Vec<String>,
    /// Services that must start after this one
    pub before: Vec<String>,
}

/// Boot timing information
#[derive(Debug, Clone)]
pub struct BootTiming {
    /// Total boot time (milliseconds)
    pub total_time: u64,
    /// Kernel boot time (milliseconds)
    pub kernel_time: u64,
    /// Initrd boot time (milliseconds)
    pub initrd_time: u64,
    /// Userspace boot time (milliseconds)
    pub userspace_time: u64,
    /// Service timings
    pub services: Vec<ServiceTiming>,
}

/// Service boot timing
#[derive(Debug, Clone)]
pub struct ServiceTiming {
    /// Service name
    pub name: String,
    /// Time to activate (milliseconds)
    pub activation_time: u64,
    /// Start time offset from boot (milliseconds)
    pub start_offset: u64,
}

impl BootTiming {
    /// Get slowest services
    pub fn slowest_services(&self, limit: usize) -> Vec<&ServiceTiming> {
        let mut services = self.services.iter().collect::<Vec<_>>();
        services.sort_by_key(|b| std::cmp::Reverse(b.activation_time));
        services.into_iter().take(limit).collect()
    }

    /// Get critical chain (services that delayed boot)
    pub fn critical_chain(&self) -> Vec<&ServiceTiming> {
        let mut services = self.services.iter().collect::<Vec<_>>();
        services.sort_by_key(|a| a.start_offset + a.activation_time);
        services.into_iter().rev().take(10).collect()
    }
}

/// Systemd analyzer
#[derive(Debug)]
pub struct SystemdAnalyzer {
    /// Root path of the inspected system
    root_path: PathBuf,
}

impl SystemdAnalyzer {
    /// Create a new systemd analyzer
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Get systemd directory path
    pub fn systemd_dir(&self) -> PathBuf {
        self.root_path.join("etc/systemd/system")
    }

    /// Get journal directory path
    pub fn journal_dir(&self) -> PathBuf {
        self.root_path.join("var/log/journal")
    }

    /// Check if systemd is used
    pub fn is_systemd(&self) -> bool {
        self.systemd_dir().exists() || self.root_path.join("lib/systemd").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_entry_priority_str() {
        let entry = JournalEntry {
            timestamp: 0,
            priority: 3,
            unit: None,
            message: "Test".to_string(),
            pid: None,
            fields: HashMap::new(),
        };
        assert_eq!(entry.priority_str(), "ERR");
    }

    #[test]
    fn test_service_state_display() {
        assert_eq!(ServiceState::Active.to_string(), "active");
        assert_eq!(ServiceState::Failed.to_string(), "failed");
    }

    #[test]
    fn test_boot_timing_slowest_services() {
        let timing = BootTiming {
            total_time: 10000,
            kernel_time: 2000,
            initrd_time: 1000,
            userspace_time: 7000,
            services: vec![
                ServiceTiming {
                    name: "fast".to_string(),
                    activation_time: 100,
                    start_offset: 0,
                },
                ServiceTiming {
                    name: "slow".to_string(),
                    activation_time: 5000,
                    start_offset: 0,
                },
                ServiceTiming {
                    name: "medium".to_string(),
                    activation_time: 1000,
                    start_offset: 0,
                },
            ],
        };

        let slowest = timing.slowest_services(2);
        assert_eq!(slowest.len(), 2);
        assert_eq!(slowest[0].name, "slow");
        assert_eq!(slowest[1].name, "medium");
    }

    #[test]
    fn test_systemd_analyzer_creation() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        assert_eq!(
            analyzer.systemd_dir(),
            PathBuf::from("/tmp/etc/systemd/system")
        );
        assert_eq!(
            analyzer.journal_dir(),
            PathBuf::from("/tmp/var/log/journal")
        );
    }
}
