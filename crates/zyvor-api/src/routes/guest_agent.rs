// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guest agent push registration, heartbeat, and report storage.

use axum::extract::{Path, State};
use axum::Json;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct RegisterGuestAgentRequest {
    pub hostname: String,
    pub agent_version: String,
    #[serde(default)]
    pub bootstrap_token: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub vm_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterGuestAgentResponse {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GuestReportPayload {
    #[serde(default)]
    pub guest_health: Value,
    #[serde(default)]
    pub metrics: Value,
    #[serde(default)]
    pub recent_events: Value,
}

fn report_key(agent_id: &str) -> String {
    format!("guest-agent:report:{agent_id}")
}

fn vm_report_key(namespace: &str, name: &str) -> String {
    format!("guest-agent:vm-report:{namespace}:{name}")
}

fn vm_agent_id_key(namespace: &str, name: &str) -> String {
    format!("guest-agent:vm-agent:{namespace}:{name}")
}

fn heartbeat_key(agent_id: &str) -> String {
    format!("guest-agent:heartbeat:{agent_id}")
}

pub async fn guest_agent_bootstrap_info(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let ca = crate::guest_agent_ca::AgentCa::from_config(state.config.agent_ca_dir.clone());
  let mtls_push_url = mtls_push_url_for_config(&state.config);
    Ok(Json(ApiResponse::ok(json!({
        "token_required": state.config.agent_bootstrap_token.is_some(),
        "mtls_ready": ca.is_ready(),
        "mtls_push_url": mtls_push_url,
        "register_path": "/api/v1/guest-agents/register",
        "bootstrap_path": "/api/v1/guest-agents/bootstrap",
        "protocol_version": "1.2",
    }))))
}

fn mtls_push_url_for_config(config: &crate::config::Config) -> Option<String> {
    if config.agent_mtls_bind_addr.is_none() {
        return None;
    }
    if let Some(url) = config.agent_mtls_public_url.as_ref() {
        return Some(url.trim_end_matches('/').to_string());
    }
    let port = config
        .agent_mtls_bind_addr
        .as_ref()
        .and_then(|addr| addr.rsplit(':').next())
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8443);
    let public = config
        .zeus_public_url
        .as_deref()
        .unwrap_or(config.public_base_url.as_str());
    url::Url::parse(public)
        .ok()
        .and_then(|mut u| {
            u.set_port(Some(port)).ok();
            Some(u.to_string().trim_end_matches('/').to_string())
        })
}

fn cert_sha256_fingerprint(pem: &str) -> Option<String> {
    use openssl::hash::MessageDigest;
    use openssl::x509::X509;
    let cert = X509::from_pem(pem.as_bytes()).ok()?;
    let digest = cert.digest(MessageDigest::sha256()).ok()?;
    Some(
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
}

#[derive(Debug, Deserialize)]
pub struct BootstrapCertRequest {
    pub hostname: String,
    #[serde(default)]
    pub bootstrap_token: Option<String>,
    #[serde(default)]
    pub csr_pem: Option<String>,
}

fn enforce_agent_bootstrap(
    state: &AppState,
    provided: Option<&str>,
) -> ApiResult<()> {
    match &state.config.agent_bootstrap_token {
        Some(expected) => {
            if provided.unwrap_or("") != expected {
                Err(ApiError::unauthorized("invalid or missing bootstrap token"))
            } else {
                Ok(())
            }
        }
        None if state.config.auth_enabled || state.config.agent_mtls_bind_addr.is_some() => {
            Err(ApiError::internal(
                "guest agent bootstrap token not configured — set AGENT_BOOTSTRAP_TOKEN",
            ))
        }
        None => Ok(()),
    }
}

pub async fn guest_agent_bootstrap_cert(
    State(state): State<AppState>,
    Json(body): Json<BootstrapCertRequest>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    enforce_agent_bootstrap(&state, body.bootstrap_token.as_deref())?;

    let ca = crate::guest_agent_ca::AgentCa::from_config(state.config.agent_ca_dir.clone());
    let issued = ca.issue_client_cert(&body.hostname)?;

    if let Some(fp) = cert_sha256_fingerprint(&issued.cert_pem) {
        let mut redis = state.redis.clone();
        let _ = redis
            .set::<_, _, ()>(
                format!("guest-agent:certfp:{}", body.hostname),
                fp,
            )
            .await;
    }

    Ok(Json(ApiResponse::ok(json!({
        "status": "issued",
        "hostname": body.hostname,
        "cert_pem": issued.cert_pem,
        "key_pem": issued.key_pem,
        "ca_pem": issued.ca_pem,
        "expires_at": issued.expires_at,
        "cert_path_hint": "/var/lib/zyvor/agent.crt",
        "key_path_hint": "/var/lib/zyvor/agent.key",
        "ca_path_hint": "/var/lib/zyvor/ca.crt",
        "mtls_ready": true,
    }))))
}

pub async fn register_guest_agent(
    State(state): State<AppState>,
    Json(body): Json<RegisterGuestAgentRequest>,
) -> ApiResult<Json<ApiResponse<RegisterGuestAgentResponse>>> {
    enforce_agent_bootstrap(&state, body.bootstrap_token.as_deref())?;

    let agent_id = if body.hostname.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        format!(
            "{}-{}",
            body.hostname.replace('.', "-"),
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("id")
        )
    };

    let mut redis = state.redis.clone();
    let meta = serde_json::json!({
        "hostname": body.hostname,
        "agent_version": body.agent_version,
        "namespace": body.namespace,
        "vm_name": body.vm_name,
        "registered_at": chrono::Utc::now().to_rfc3339(),
    });
    redis
        .set::<_, _, ()>(
            format!("guest-agent:meta:{agent_id}"),
            serde_json::to_string(&meta).unwrap_or_default(),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let (Some(ns), Some(vm)) = (&body.namespace, &body.vm_name) {
        redis
            .set::<_, _, ()>(vm_agent_id_key(ns, vm), &agent_id)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok(Json(ApiResponse::ok(RegisterGuestAgentResponse {
        agent_id,
    })))
}

fn events_key(agent_id: &str) -> String {
    format!("guest-agent:events:{agent_id}")
}

pub async fn guest_agent_heartbeat(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let mut redis = state.redis.clone();
    let payload = serde_json::json!({
        "agent_id": agent_id,
        "body": body,
        "at": chrono::Utc::now().to_rfc3339(),
    });
    if let Some(events) = body.get("recent_events") {
        redis
            .set_ex::<_, _, ()>(
                events_key(&agent_id),
                serde_json::to_string(events).unwrap_or_default(),
                3600,
            )
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    redis
        .set_ex::<_, _, ()>(
            heartbeat_key(&agent_id),
            serde_json::to_string(&payload).unwrap_or_default(),
            300,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Protocol 1.3 rich heartbeat: reflect agent state and migration
    // readiness (and, when pushed, the full assessment) into the
    // VMGuestAgent CR so status flows without a separate proxy call.
    if body.get("heartbeat").is_some() || body.get("migration_assessment").is_some() {
        let meta_raw: Option<String> = redis
            .get(format!("guest-agent:meta:{agent_id}"))
            .await
            .unwrap_or(None);
        if let (Some(meta_str), Some(client)) = (meta_raw, state.kube.clone()) {
            if let Ok(meta) = serde_json::from_str::<Value>(&meta_str) {
                if let (Some(ns), Some(vm)) = (
                    meta.get("namespace").and_then(|v| v.as_str()),
                    meta.get("vm_name").and_then(|v| v.as_str()),
                ) {
                    if let Some(hb) = body.get("heartbeat") {
                        crate::kubevirt_guest_cr::patch_vmguestagent_heartbeat(
                            &client, ns, vm, hb,
                        )
                        .await;
                    }
                    if let Some(assessment) = body.get("migration_assessment") {
                        crate::kubevirt_guest_cr::patch_vmguestagent_migration(
                            &client, ns, vm, assessment,
                        )
                        .await;
                    }
                }
            }
        }
    }

    Ok(Json(ApiResponse::ok(json!({ "accepted": true }))))
}

pub async fn guest_agent_report(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<GuestReportPayload>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let mut redis = state.redis.clone();
    let report = serde_json::json!({
        "agent_id": agent_id,
        "guest_health": body.guest_health,
        "metrics": body.metrics,
        "recent_events": body.recent_events,
        "received_at": chrono::Utc::now().to_rfc3339(),
    });
    let raw = serde_json::to_string(&report).unwrap_or_default();
    redis
        .set_ex::<_, _, ()>(report_key(&agent_id), &raw, 86400)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

  // Map to VM if meta has namespace/name
    let meta_raw: Option<String> = redis
        .get(format!("guest-agent:meta:{agent_id}"))
        .await
        .unwrap_or(None);
    if let Some(meta_str) = meta_raw {
        if let Ok(meta) = serde_json::from_str::<Value>(&meta_str) {
            if let (Some(ns), Some(vm)) = (
                meta.get("namespace").and_then(|v| v.as_str()),
                meta.get("vm_name").and_then(|v| v.as_str()),
            ) {
                redis
                    .set_ex::<_, _, ()>(vm_report_key(ns, vm), &raw, 86400)
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;

                if let Some(client) = state.kube.clone() {
                    let version = meta
                        .get("agent_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    crate::kubevirt_guest_cr::patch_vmguestagent_health(
                        &client,
                        ns,
                        vm,
                        &body.guest_health,
                        version,
                    )
                    .await;
                    crate::packetwolf_correlate::emit_guest_report_correlation(
                        &state.config,
                        Some(&client),
                        ns,
                        vm,
                        &body.guest_health,
                        &body.metrics,
                        &body.recent_events,
                        version,
                    );
                }
            }
        }
    }

    Ok(Json(ApiResponse::ok(json!({ "accepted": true }))))
}

pub async fn get_guest_agent_report(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let mut redis = state.redis.clone();
    let raw: Option<String> = redis
        .get(report_key(&agent_id))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let report = raw
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(json!({}));
    Ok(Json(ApiResponse::ok(report)))
}

pub async fn fetch_vm_guest_report(
    redis: &mut redis::aio::ConnectionManager,
    namespace: &str,
    name: &str,
) -> Option<Value> {
    let raw: Option<String> = redis.get(vm_report_key(namespace, name)).await.ok().flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

pub async fn pull_guest_rpc(
    proxy_url: &str,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let params_empty = params.as_object().map(|o| o.is_empty()).unwrap_or(true);
    let path = match method {
        "guestkit.getGuestHealth" if params_empty => "/guest/health",
        "guestkit.getSystemdUnits" if params_empty => "/guest/systemd",
        "guestkit.getGuestInfo" if params_empty => "/guest/info",
        "guestkit.getProcesses" if params_empty => "/guest/processes",
        "guestkit.getEvidence" if params_empty => "/evidence",
        _ => "/rpc",
    };

    if path == "/rpc" {
        let resp = client
            .post(format!("{proxy_url}/rpc"))
            .json(&json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    } else {
        let resp = client
            .get(format!("{proxy_url}{path}"))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }
}
