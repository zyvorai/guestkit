// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Normalized guest health model for Zeus OS.

use serde::{Deserialize, Serialize};

use crate::guest_info::GuestHealthComponents;

/// Overall guest health level.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthLevel {
    #[default]
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

/// Normalized in-guest health report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestHealth {
    pub vm_hostname: String,
    pub guest_health: HealthLevel,
    pub boot_state: String,
    pub systemd_state: String,
    pub failed_units: usize,
    pub critical_services: Vec<CriticalService>,
    pub network: NetworkHealth,
    pub storage: StorageHealth,
    pub security: SecurityHealthSummary,
    pub recommendations: Vec<Recommendation>,
    pub collected_at: String,
    pub agent_version: String,
    #[serde(default)]
    pub score: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "GuestHealthComponents::is_default")]
    pub components: GuestHealthComponents,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal_hints: Vec<String>,
}

impl GuestHealthComponents {
    fn is_default(c: &GuestHealthComponents) -> bool {
        c.boot == HealthLevel::Unknown
            && c.systemd == HealthLevel::Unknown
            && c.network == HealthLevel::Unknown
            && c.dns == HealthLevel::Unknown
            && c.storage == HealthLevel::Unknown
            && c.security == HealthLevel::Unknown
            && c.agent == HealthLevel::Unknown
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CriticalService {
    pub name: String,
    pub state: String,
    pub sub_state: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_exit_code: Option<i32>,
    pub suggested_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkHealth {
    pub default_route: bool,
    pub dns_working: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_error: Option<String>,
    pub interfaces_up: usize,
    pub cluster_dns_reachable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageHealth {
    pub root_usage_percent: u8,
    pub inode_usage_percent: u8,
    pub read_only_mounts: usize,
    pub pressure_io: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityHealthSummary {
    pub pending_security_updates: bool,
    pub firewall_enabled: bool,
    pub selinux: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub priority: u8,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub action: String,
}

/// Login/session state from org.freedesktop.login1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginState {
    pub logged_in_users: Vec<LoggedInUser>,
    pub inhibitors: Vec<ShutdownInhibitor>,
    pub idle_hint: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoggedInUser {
    pub name: String,
    pub seat: String,
    pub session_type: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShutdownInhibitor {
    pub who: String,
    pub what: String,
    pub why: String,
    pub mode: String,
}

/// Time/NTP health from org.freedesktop.timedate1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimedateHealth {
    pub timezone: String,
    pub ntp_enabled: bool,
    pub ntp_synchronized: bool,
    pub rtc_in_local_time: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_secs: Option<i64>,
}

/// DNS resolver health from org.freedesktop.resolve1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsHealth {
    pub dns_servers: Vec<String>,
    pub search_domains: Vec<String>,
    pub dnssec: String,
    pub llmnr: String,
    pub mdns: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// Boot analysis summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootAnalysis {
    pub total_boot_time_ms: u64,
    pub kernel_time_ms: u64,
    pub initrd_time_ms: u64,
    pub userspace_time_ms: u64,
    pub slow_units: Vec<BootUnitTiming>,
    pub critical_chain: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootUnitTiming {
    pub name: String,
    pub time_ms: u64,
}

/// Journal slice for a unit or boot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JournalSlice {
    pub unit: String,
    pub boot_id: String,
    pub entries: Vec<JournalEntrySummary>,
    pub summary: String,
    #[serde(default)]
    pub error_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_patterns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<JournalEntrySummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JournalEntrySummary {
    pub timestamp: String,
    pub priority: u8,
    pub unit: String,
    pub message: String,
}

/// Snapshot readiness report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotReadinessReport {
    pub ready: bool,
    pub fs_frozen: bool,
    pub quiesce_supported: bool,
    pub guest_agent_connected: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hook_results: Vec<HookResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookResult {
    pub name: String,
    pub success: bool,
    pub message: String,
}

/// Remediation plan execution result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemediationResult {
    pub plan_id: String,
    pub success: bool,
    pub actions: Vec<RemediationActionResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemediationActionResult {
    pub action: String,
    pub success: bool,
    pub message: String,
}
