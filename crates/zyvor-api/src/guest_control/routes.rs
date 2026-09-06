// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTTP routes for Zyvor Guest Control Fabric.

use axum::extract::{Path, State};
use axum::Json;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::guest_action_policy::{
    enforce_file_read, enforce_file_write,
};
use crate::guest_actions::{policy_requires_approval, record_guest_action_audit};
use crate::kubevirt_guest_pull::rpc_result;
use crate::models::ApiResponse;
use crate::state::AppState;

use super::capabilities::rpc_capabilities_to_contract;
use super::doctor::run_agent_doctor;
use super::envelope::GuestControlEnvelope;
use super::install::{install_agent_strategy, InstallAgentRequest, InstallStrategy};
use super::transport::{context_to_capabilities, probe_guest_context, pull_method};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallAgentBody {
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub restart: Option<bool>,
    #[serde(default)]
    pub bundle_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GuestRepairPlanBody {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default)]
    pub inject_qga: bool,
    #[serde(default = "default_true")]
    pub inject_zyvor_agent: bool,
    #[serde(default = "default_true")]
    pub enable_systemd: bool,
    #[serde(default)]
    pub fix_cloud_init_network: bool,
    #[serde(default)]
    pub validate_fstab: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct GuestFileReadBody {
    pub path: String,
    #[serde(default = "default_max_read")]
    pub max_bytes: usize,
}

fn default_max_read() -> usize {
    64 * 1024
}

#[derive(Debug, Deserialize)]
pub struct GuestFileWriteBody {
    pub path: String,
    /// Base64-encoded file contents.
    pub content_b64: String,
}

fn parse_install_strategy(raw: Option<&str>) -> InstallStrategy {
    match raw.unwrap_or("auto").to_lowercase().as_str() {
        "cloud_init_curl" | "cloud-init" => InstallStrategy::CloudInitCurl,
        "qga_file_bootstrap" | "qga-file" => InstallStrategy::QgaFileBootstrap,
        "qga_curl_bootstrap" | "qga-curl" => InstallStrategy::QgaCurlBootstrap,
        "offline_inject" | "offline" => InstallStrategy::OfflineInject,
        "iso_attach" | "iso" => InstallStrategy::IsoAttach,
        "baked_image" | "baked" => InstallStrategy::BakedImage,
        _ => InstallStrategy::Auto,
    }
}

pub async fn get_guest_status(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    let caps = context_to_capabilities(&ctx);
    let last_poll = super::polling::load_poll_result(&mut state.redis.clone(), &namespace, &name)
        .await
        .unwrap_or(None);
    let data = json!({
        "namespace": namespace,
        "name": name,
        "vmiRunning": ctx.vmi_running,
        "qgaConnected": ctx.qga_connected,
        "zyvorAgentInstalled": ctx.zyvor_agent_installed,
        "agentDaemonRunning": ctx.agent_daemon_running,
        "networkAvailable": ctx.network_available,
        "pushRegistered": ctx.push_registered,
        "offlineRepairAvailable": ctx.offline_repair_available,
        "isWindows": ctx.is_windows,
        "agentVersion": ctx.agent_version,
        "attempts": ctx.attempts,
        "lastPoll": last_poll,
        "telemetryMode": if ctx.control_state == super::capabilities::ControlState::AirgapLive
            && !ctx.push_registered
        {
            "pull_via_virt_launcher"
        } else if ctx.push_registered {
            "push"
        } else {
            "none"
        },
    });
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps),
        data,
    )))
}

pub async fn get_guest_capabilities(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    let mut caps = context_to_capabilities(&ctx);
    if ctx.zyvor_agent_installed {
        if let Ok(pull) = pull_method(
            &state,
            &namespace,
            &name,
            guestkit_agent_protocol::capabilities::METHOD_GET_CAPABILITIES,
            json!({}),
        )
        .await
        {
            caps = rpc_capabilities_to_contract(&pull.value, &ctx);
        }
    }
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps.clone()),
        serde_json::to_value(caps).unwrap_or(json!({})),
    )))
}

pub async fn get_guest_doctor(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    let report = run_agent_doctor(&state, &namespace, &name, false).await;
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps),
        serde_json::to_value(report).unwrap_or(json!({})),
    )))
}

pub async fn post_guest_doctor(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    let report = run_agent_doctor(&state, &namespace, &name, true).await;
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps),
        serde_json::to_value(report).unwrap_or(json!({})),
    )))
}

pub async fn get_guest_readiness(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    let report = run_agent_doctor(&state, &namespace, &name, ctx.zyvor_agent_installed).await;
    let score = report.readiness_score.unwrap_or(0);
    let level = if score >= 80 {
        "good"
    } else if score >= 50 {
        "warnings"
    } else {
        "recommended"
    };
    let data = json!({
        "score": score,
        "level": level,
        "controlState": report.control_state,
        "transport": report.transport,
        "warnings": report.warnings,
        "recommendedActions": report.recommended_actions,
        "nodes": report.nodes,
        "liveDoctor": report.live_doctor,
    });
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps),
        data,
    )))
}

pub async fn post_install_agent(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    body: Option<Json<InstallAgentBody>>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let merged = body.map(|Json(b)| b).unwrap_or_default();
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    if policy_requires_approval(state.kube.as_ref()).await {
        return Err(ApiError::bad_request(
            "GuestActionPolicy requires approval for install-agent",
        ));
    }
    let result = install_agent_strategy(
        &state,
        &namespace,
        &name,
        InstallAgentRequest {
            strategy: parse_install_strategy(merged.strategy.as_deref()),
            restart: merged.restart.unwrap_or(true),
            bundle_url: merged.bundle_url,
        },
    )
    .await?;
    let caps = context_to_capabilities(&probe_guest_context(&state, &namespace, &name).await);
    Ok(Json(GuestControlEnvelope::success(
        ctx.active_transport,
        ctx.control_state,
        Some(caps),
        result,
    )))
}

pub async fn post_guest_repair_plan(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    body: Option<Json<GuestRepairPlanBody>>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    if ctx.vmi_running {
        return Err(ApiError::bad_request(
            "VM must be stopped for offline guest repair-plan",
        ));
    }
    let client = state
        .kube
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Kubernetes client required"))?;
    let disk = crate::kubevirt_inspect::resolve_stopped_vm_disk_public(
        &state, client, &namespace, &name, "guest repair",
    )
    .await?;
    let resp = crate::jobs::submit_disk_path_job(
        &state,
        disk.shadow_id,
        &disk.disk_path,
        &disk.format,
        "guestkit.repair",
        "guestkit.repair.v1",
        json!({
            "fix": "boot",
            "dry_run": req.dry_run,
            "cluster_vm": disk.label,
            "inject_qga": req.inject_qga,
            "inject_zyvor_agent": req.inject_zyvor_agent,
            "enable_systemd": req.enable_systemd,
            "fix_cloud_init_network": req.fix_cloud_init_network,
            "validate_fstab": req.validate_fstab,
        }),
    )
    .await?;
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        super::capabilities::GuestTransport::OfflineDisk,
        ctx.control_state,
        Some(caps),
        serde_json::to_value(resp).unwrap_or(json!({})),
    )))
}

pub async fn post_guest_file_read(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(body): Json<GuestFileReadBody>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let client = state
        .kube
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Kubernetes client required"))?;
    enforce_file_read(Some(client), &body.path, false).await?;
    if policy_requires_approval(state.kube.as_ref()).await {
        return Err(ApiError::bad_request(
            "GuestActionPolicy requires approval for file read",
        ));
    }
    let max = body.max_bytes.clamp(1, 256 * 1024);
    let bytes =
        crate::kubevirt_qga::qga_file_read(client, &namespace, &name, &body.path, max, 120).await?;
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    record_guest_action_audit(
        &mut state.redis.clone(),
        "file_read",
        &namespace,
        &name,
        ctx.active_transport.as_str(),
        false,
        json!({ "path": body.path, "bytes": bytes.len() }),
    )
    .await?;
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        super::capabilities::GuestTransport::QgaBuiltin,
        ctx.control_state,
        Some(caps),
        json!({
            "path": body.path,
            "size": bytes.len(),
            "contentB64": base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &bytes
            ),
        }),
    )))
}

pub async fn post_guest_file_write(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(body): Json<GuestFileWriteBody>,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let client = state
        .kube
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Kubernetes client required"))?;
    enforce_file_write(Some(client), &body.path, false).await?;
    if policy_requires_approval(state.kube.as_ref()).await {
        return Err(ApiError::bad_request(
            "GuestActionPolicy requires approval for file write",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&body.content_b64)
        .map_err(|e| ApiError::bad_request(format!("invalid base64: {e}")))?;
    crate::kubevirt_qga::qga_file_write(client, &namespace, &name, &body.path, &bytes, 300)
        .await?;
    let ctx = probe_guest_context(&state, &namespace, &name).await;
    record_guest_action_audit(
        &mut state.redis.clone(),
        "file_write",
        &namespace,
        &name,
        ctx.active_transport.as_str(),
        false,
        json!({ "path": body.path, "bytes": bytes.len() }),
    )
    .await?;
    let caps = context_to_capabilities(&ctx);
    Ok(Json(GuestControlEnvelope::success(
        super::capabilities::GuestTransport::QgaBuiltin,
        ctx.control_state,
        Some(caps),
        json!({ "path": body.path, "written": bytes.len() }),
    )))
}

pub async fn post_guest_poll_reconcile(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let result = super::polling::reconcile_airgap_polls(&state).await?;
    Ok(Json(ApiResponse::ok(result)))
}

/// Latest airgap poll sample for one VM (`guest-agent:vm-poll:{ns}:{name}`).
pub async fn get_guest_poll_telemetry(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let poll = super::polling::load_poll_result(&mut state.redis.clone(), &namespace, &name)
        .await?
        .unwrap_or_else(|| {
            json!({
                "namespace": namespace,
                "name": name,
                "ok": false,
                "message": "no poll sample yet (VM not airgap_live or worker not run)",
            })
        });
    Ok(Json(ApiResponse::ok(poll)))
}

/// Fleet rollup from the last reconcile cycle (`guest-agent:poll-fleet`).
pub async fn get_guest_poll_fleet_telemetry(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let fleet = super::polling::load_fleet_telemetry(&mut state.redis.clone())
        .await?
        .unwrap_or_else(|| {
            json!({
                "scanned": 0,
                "polled": 0,
                "message": "no fleet telemetry yet",
            })
        });
    Ok(Json(ApiResponse::ok(fleet)))
}

/// Wrap intel route payloads with guest control envelope metadata.
pub async fn envelope_for_vm(
    state: &AppState,
    namespace: &str,
    name: &str,
    transport: Option<super::capabilities::GuestTransport>,
    data: Value,
) -> GuestControlEnvelope {
    let ctx = probe_guest_context(state, namespace, name).await;
    GuestControlEnvelope::success(
        transport.unwrap_or(ctx.active_transport),
        ctx.control_state,
        Some(context_to_capabilities(&ctx)),
        data,
    )
}

pub async fn pull_with_envelope(
    state: &AppState,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
) -> ApiResult<Json<GuestControlEnvelope>> {
    let ctx = probe_guest_context(state, namespace, name).await;
    let pull = pull_method(state, namespace, name, method, params)
        .await
        .map_err(ApiError::bad_request)?;
    let data = rpc_result(pull.value);
    Ok(Json(GuestControlEnvelope::success(
        pull.transport,
        ctx.control_state,
        Some(context_to_capabilities(&ctx)),
        data,
    )))
}
