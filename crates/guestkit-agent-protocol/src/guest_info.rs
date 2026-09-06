// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Normalized guest identity and service health models.

use serde::{Deserialize, Serialize};

use crate::health::HealthLevel;

/// Static guest identity and OS inventory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestInfo {
    pub hostname: String,
    pub os: GuestOsInfo,
    pub virtualization: GuestVirtualizationInfo,
    pub identity: GuestIdentity,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestOsInfo {
    pub family: String,
    pub id: String,
    pub version: String,
    pub kernel: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestVirtualizationInfo {
    pub detected: String,
    pub qga_installed: bool,
    pub qga_running: bool,
    pub zyvor_agent_version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestIdentity {
    pub machine_id: String,
    pub dmi_uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zeus_vm_uid: Option<String>,
}

/// Per-unit normalized service health view.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub name: String,
    pub state: String,
    pub sub_state: String,
    pub main_pid: u32,
    pub exit_code: Option<i32>,
    pub restart_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
}

/// Component-level health scores for Zeus UI rings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestHealthComponents {
    pub boot: HealthLevel,
    pub systemd: HealthLevel,
    pub network: HealthLevel,
    pub dns: HealthLevel,
    pub storage: HealthLevel,
    pub security: HealthLevel,
    pub agent: HealthLevel,
}

/// Live systemd manager event for black-box recording.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdEvent {
    pub timestamp: String,
    pub kind: String,
    pub unit: String,
    pub detail: String,
}
