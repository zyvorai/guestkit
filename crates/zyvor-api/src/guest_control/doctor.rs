// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Agent Doctor — preflight probes and decision tree.

use guestkit_agent_protocol::capabilities::METHOD_DOCTOR;
use serde::Serialize;
use serde_json::{json, Value};

use crate::kubevirt_guest_pull::rpc_result;
use crate::state::AppState;

use super::capabilities::ControlState;
use super::transport::{probe_guest_context, pull_method, context_to_capabilities};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorNode {
    pub id: String,
    pub question: String,
    pub answer: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDoctorReport {
    pub control_state: String,
    pub transport: String,
    pub nodes: Vec<DoctorNode>,
    pub recommended_actions: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_doctor: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_score: Option<u8>,
}

pub async fn run_agent_doctor(
    state: &AppState,
    namespace: &str,
    name: &str,
    run_live: bool,
) -> AgentDoctorReport {
    let ctx = probe_guest_context(state, namespace, name).await;
    let caps = context_to_capabilities(&ctx);

    let mut nodes = vec![
        DoctorNode {
            id: "vm_running".into(),
            question: "Is VM running?".into(),
            answer: if ctx.vmi_running { "yes" } else { "no" }.into(),
            ok: ctx.vmi_running,
            detail: None,
        },
        DoctorNode {
            id: "qga_channel".into(),
            question: "Is QGA channel connected?".into(),
            answer: if ctx.qga_connected { "yes" } else { "no" }.into(),
            ok: ctx.qga_connected,
            detail: None,
        },
        DoctorNode {
            id: "guest_ping".into(),
            question: "Does guest-ping respond?".into(),
            answer: if ctx.qga_connected { "yes" } else { "no" }.into(),
            ok: ctx.qga_connected,
            detail: None,
        },
        DoctorNode {
            id: "zyvor_agent_installed".into(),
            question: "Is zyvor-guest-agent installed?".into(),
            answer: if ctx.zyvor_agent_installed {
                "yes"
            } else {
                "no"
            }
            .into(),
            ok: ctx.zyvor_agent_installed,
            detail: ctx.agent_version.clone(),
        },
        DoctorNode {
            id: "agent_daemon".into(),
            question: "Is agent daemon running?".into(),
            answer: if ctx.agent_daemon_running {
                "yes"
            } else {
                "no"
            }
            .into(),
            ok: ctx.agent_daemon_running,
            detail: None,
        },
        DoctorNode {
            id: "guest_network".into(),
            question: "Is guest network available?".into(),
            answer: if ctx.network_available {
                "yes"
            } else {
                "no"
            }
            .into(),
            ok: ctx.network_available,
            detail: None,
        },
        DoctorNode {
            id: "push_registration".into(),
            question: "Is push registration alive?".into(),
            answer: if ctx.push_registered { "yes" } else { "no" }.into(),
            ok: ctx.push_registered,
            detail: None,
        },
        DoctorNode {
            id: "offline_repair".into(),
            question: "Is offline repair possible?".into(),
            answer: if ctx.offline_repair_available {
                "yes"
            } else {
                "no"
            }
            .into(),
            ok: ctx.offline_repair_available,
            detail: None,
        },
    ];

    if ctx.is_windows {
        nodes.push(DoctorNode {
            id: "windows_platform".into(),
            question: "Windows guest — VirtIO/QGA service path?".into(),
            answer: if ctx.qga_connected { "qga_ready" } else { "needs_virtio_win" }.into(),
            ok: ctx.qga_connected,
            detail: Some("Install VirtIO drivers + QEMU Guest Agent for full Windows airgap control".into()),
        });
    }

    let mut recommended_actions = caps.recommended_actions.clone();
    if ctx.control_state == ControlState::QgaOnly && !ctx.zyvor_agent_installed {
        recommended_actions.push("install_agent_via_qga".into());
        nodes.push(DoctorNode {
            id: "recommendation".into(),
            question: "Recommended action".into(),
            answer: "Install Zyvor Guest Agent using QGA (no guest network required)".into(),
            ok: true,
            detail: None,
        });
    }
    if ctx.control_state == ControlState::AirgapLive {
        nodes.push(DoctorNode {
            id: "airgap_mode".into(),
            question: "Airgap live control available?".into(),
            answer: "yes — host-mediated polling works without guest network".into(),
            ok: true,
            detail: None,
        });
    }

    let live_doctor = if run_live && ctx.zyvor_agent_installed {
        pull_method(state, namespace, name, METHOD_DOCTOR, json!({}))
            .await
            .ok()
            .map(|r| rpc_result(r.value))
    } else {
        None
    };

    let readiness_score = compute_readiness_score(&ctx, live_doctor.as_ref());

    AgentDoctorReport {
        control_state: ctx.control_state.as_str().to_string(),
        transport: ctx.active_transport.as_str().to_string(),
        nodes,
        recommended_actions,
        warnings: caps.warnings,
        live_doctor,
        readiness_score: Some(readiness_score),
    }
}

fn compute_readiness_score(ctx: &super::transport::GuestContext, live: Option<&Value>) -> u8 {
    let mut score: i32 = 0;
    if ctx.vmi_running {
        score += 15;
    } else if ctx.offline_repair_available {
        score += 10;
    }
    if ctx.qga_connected {
        score += 25;
    }
    if ctx.zyvor_agent_installed {
        score += 25;
    }
    if ctx.agent_daemon_running {
        score += 15;
    }
    if ctx.network_available {
        score += 10;
    } else if ctx.qga_connected && ctx.zyvor_agent_installed {
        score += 8;
    }
    if ctx.push_registered {
        score += 10;
    }
    if let Some(doc) = live {
        if let Some(boot) = doc.pointer("/bootability/score").and_then(|v| v.as_u64()) {
            score = score.saturating_add((boot / 10) as i32);
        }
    }
    score.clamp(0, 100) as u8
}
