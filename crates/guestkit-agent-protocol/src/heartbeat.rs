// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rich heartbeat wire types (protocol 1.3).
//!
//! The heartbeat is a lean, cheap-to-build snapshot of agent and guest
//! health, pushed periodically on subscribed channels and returned by
//! `guestkit.getAgentHealth`. The full [`crate::health::GuestHealth`]
//! report remains the deep-inspection payload.

use serde::{Deserialize, Serialize};

/// Coarse agent lifecycle state derived on every heartbeat tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Daemon started, transport not yet exchanged a frame.
    #[default]
    Starting,
    /// Transport connected, first health evaluation pending.
    Connected,
    Healthy,
    /// Failed critical services, disk nearly full, or sustained pressure.
    Degraded,
    /// Self-update staging or applying.
    Updating,
    /// Filesystems are frozen (snapshot/cutover in progress).
    Quiesced,
    /// Guest is in systemd degraded/emergency state.
    RecoveryMode,
}

/// PSI (pressure stall information) `avg10` values. Linux only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PressureSummary {
    pub cpu: f32,
    pub memory: f32,
    pub io: f32,
}

/// Periodic agent heartbeat payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Monotonic sequence number since agent start.
    pub seq: u64,
    pub agent_state: AgentState,
    /// Kernel boot identifier (`/proc/sys/kernel/random/boot_id`; synthetic
    /// hash of boot time + machine GUID on Windows).
    pub boot_id: String,
    pub os_uptime_secs: u64,
    pub agent_uptime_secs: u64,
    pub agent_version: String,
    pub protocol_version: String,
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub root_disk_usage_percent: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressure: Option<PressureSummary>,
    pub pending_reboot: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_reboot_reasons: Vec<String>,
    /// Failed units/services considered critical (empty when healthy).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub critical_services_failed: Vec<String>,
    /// Cheap readiness predicate; authoritative scoring comes from
    /// `guestkit.migration.assess`.
    pub migration_ready: bool,
    pub fs_frozen: bool,
    /// RFC 3339 emission time.
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_round_trip() {
        let hb = Heartbeat {
            seq: 42,
            agent_state: AgentState::Healthy,
            boot_id: "8b51".into(),
            os_uptime_secs: 86400,
            pressure: Some(PressureSummary {
                cpu: 0.11,
                memory: 0.22,
                io: 0.07,
            }),
            migration_ready: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&hb).unwrap();
        assert!(json.contains("\"agent_state\":\"healthy\""));
        let back: Heartbeat = serde_json::from_str(&json).unwrap();
        assert_eq!(back.seq, 42);
        assert_eq!(back.agent_state, AgentState::Healthy);
    }

    #[test]
    fn state_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&AgentState::RecoveryMode).unwrap(),
            "\"recovery_mode\""
        );
    }
}
