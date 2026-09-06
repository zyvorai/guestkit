// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guest capability contract and control states.

use guestkit_agent_protocol::capabilities::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuestTransport {
    VirtioSerial,
    #[serde(rename = "qga-exec")]
    QgaExecRpc,
    #[serde(rename = "qga-builtin")]
    QgaBuiltin,
    #[serde(rename = "in-guest-socket")]
    InGuestSocket,
    #[serde(rename = "https-push")]
    HttpsPush,
    #[serde(rename = "offline-disk")]
    OfflineDisk,
    #[serde(rename = "console-only")]
    ConsoleOnly,
}

impl GuestTransport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VirtioSerial => "virtio-serial",
            Self::QgaExecRpc => "qga-exec",
            Self::QgaBuiltin => "qga-builtin",
            Self::InGuestSocket => "in-guest-socket",
            Self::HttpsPush => "https-push",
            Self::OfflineDisk => "offline-disk",
            Self::ConsoleOnly => "console-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlState {
    FullAgent,
    AirgapLive,
    QgaOnly,
    DiskOnly,
    ConsoleOnly,
    BlindVm,
}

impl ControlState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FullAgent => "full_agent",
            Self::AirgapLive => "airgap_live",
            Self::QgaOnly => "qga_only",
            Self::DiskOnly => "disk_only",
            Self::ConsoleOnly => "console_only",
            Self::BlindVm => "blind_vm",
        }
    }

    pub fn ui_label(self) -> &'static str {
        match self {
            Self::FullAgent => "Full Agent",
            Self::AirgapLive => "Airgap Live",
            Self::QgaOnly => "QGA Only",
            Self::DiskOnly => "Disk Only",
            Self::ConsoleOnly => "Console Only",
            Self::BlindVm => "Blind VM",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCapabilitySupports {
    pub evidence: bool,
    pub exec: bool,
    pub file_read: bool,
    pub file_write: bool,
    pub freeze: bool,
    pub service_control: bool,
    pub self_update: bool,
    pub push_telemetry: bool,
    pub doctor: bool,
    pub offline_repair: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCapabilityContract {
    pub network: bool,
    pub qga: bool,
    pub zyvor_agent: bool,
    pub agent_daemon_running: bool,
    pub push_registered: bool,
    pub transport: String,
    pub control_state: String,
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_version: Option<String>,
    pub supports: GuestCapabilitySupports,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportAttempt {
    pub tier: String,
    pub ok: bool,
    pub latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn build_capabilities(
    network: bool,
    qga: bool,
    zyvor_agent: bool,
    agent_daemon_running: bool,
    push_registered: bool,
    transport: GuestTransport,
    control_state: ControlState,
    agent_version: Option<String>,
    offline_repair: bool,
    is_windows: bool,
) -> GuestCapabilityContract {
    let mut warnings = Vec::new();
    let mut recommended_actions = Vec::new();

    if !network {
        warnings.push("guest network unavailable".into());
    }
    if qga && !zyvor_agent {
        recommended_actions.push("install_agent_via_qga".into());
    }
    if !qga && control_state == ControlState::DiskOnly {
        recommended_actions.push("offline_inject_agent".into());
    }
    if !qga && control_state == ControlState::ConsoleOnly {
        recommended_actions.push("attach_virtio_guestagent_channel".into());
    }

    let supports = GuestCapabilitySupports {
        evidence: zyvor_agent || push_registered,
        exec: qga && (zyvor_agent || control_state == ControlState::QgaOnly),
        file_read: qga,
        file_write: qga,
        freeze: qga,
        service_control: zyvor_agent && agent_daemon_running,
        self_update: network && zyvor_agent,
        push_telemetry: network && push_registered,
        doctor: zyvor_agent || offline_repair || qga,
        offline_repair,
    };

    if is_windows && !zyvor_agent && qga {
        recommended_actions.push("install_windows_agent_via_qga".into());
    }

    GuestCapabilityContract {
        network,
        qga,
        zyvor_agent,
        agent_daemon_running,
        push_registered,
        transport: transport.as_str().to_string(),
        control_state: control_state.as_str().to_string(),
        protocol_version: PROTOCOL_VERSION.to_string(),
        agent_version,
        supports,
        warnings,
        recommended_actions,
    }
}

pub fn infer_control_state(
    vmi_running: bool,
    qga: bool,
    zyvor_agent: bool,
    agent_daemon_running: bool,
    network: bool,
    push_registered: bool,
    offline_repair: bool,
) -> ControlState {
    if !vmi_running {
        return if offline_repair {
            ControlState::DiskOnly
        } else {
            ControlState::BlindVm
        };
    }
    if !qga {
        return ControlState::ConsoleOnly;
    }
    if zyvor_agent && agent_daemon_running && (network || push_registered) {
        return ControlState::FullAgent;
    }
    if zyvor_agent && agent_daemon_running {
        return ControlState::AirgapLive;
    }
    if qga {
        return ControlState::QgaOnly;
    }
    ControlState::BlindVm
}

pub fn rpc_capabilities_to_contract(
    rpc: &Value,
    ctx: &super::transport::GuestContext,
) -> GuestCapabilityContract {
    let methods: Vec<String> = rpc
        .pointer("/result/methods")
        .or_else(|| rpc.get("methods"))
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let agent_version = rpc
        .pointer("/result/agent_version")
        .or_else(|| rpc.get("agent_version"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let mut caps = build_capabilities(
        ctx.network_available,
        ctx.qga_connected,
        ctx.zyvor_agent_installed,
        ctx.agent_daemon_running,
        ctx.push_registered,
        ctx.active_transport,
        ctx.control_state,
        agent_version,
        ctx.offline_repair_available,
        ctx.is_windows,
    );
    if !methods.is_empty() {
        caps.supports.evidence = methods.iter().any(|m| m.contains("getEvidence"));
        caps.supports.doctor = methods.iter().any(|m| m.contains("doctor"));
        caps.supports.exec = methods.iter().any(|m| m.contains("exec"));
    }
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_state_labels() {
        assert_eq!(ControlState::AirgapLive.as_str(), "airgap_live");
        assert_eq!(ControlState::AirgapLive.ui_label(), "Airgap Live");
    }

    #[test]
    fn infer_airgap_live() {
        let state = infer_control_state(true, true, true, true, false, false, false);
        assert_eq!(state, ControlState::AirgapLive);
    }

    #[test]
    fn infer_disk_only() {
        let state = infer_control_state(false, false, false, false, false, false, true);
        assert_eq!(state, ControlState::DiskOnly);
    }

    #[test]
    fn build_capabilities_airgap_warnings() {
        let caps = build_capabilities(
            false,
            true,
            false,
            false,
            false,
            GuestTransport::QgaBuiltin,
            ControlState::QgaOnly,
            None,
            false,
            false,
        );
        assert!(caps.warnings.iter().any(|w| w.contains("network")));
        assert!(caps
            .recommended_actions
            .iter()
            .any(|a| a == "install_agent_via_qga"));
    }

    #[test]
    fn transport_serde_roundtrip() {
        let t = GuestTransport::QgaExecRpc;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"qga-exec\"");
        let back: GuestTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
