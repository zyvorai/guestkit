// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live guest-agent routes (online VM inspection via agent-proxy).

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;
use crate::error::{ApiError, ApiResult};
use crate::jobs::{build_job, enqueue_job};
use crate::models::{ApiResponse, JobEnqueueResponse};
use crate::routes::vms::load_vm;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AgentQuery {
    #[serde(default = "default_target")]
    pub target: String,
}

fn default_target() -> String {
    "kubevirt".to_string()
}

#[derive(Debug, Deserialize)]
pub struct AgentProxyBody {
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default = "default_target")]
    pub target: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentRpcBody {
    #[serde(default)]
    pub proxy_url: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AgentFixBody {
    #[serde(default)]
    pub proxy_url: Option<String>,
    pub plan: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentPingResponse {
    pub reachable: bool,
    pub proxy_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn resolve_proxy_url(state: &AppState, override_url: Option<String>) -> ApiResult<String> {
    if let Some(url) = override_url.filter(|u| !u.trim().is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    state
        .config
        .agent_proxy_url
        .as_ref()
        .map(|u| u.trim_end_matches('/').to_string())
        .ok_or_else(|| {
            ApiError::bad_request(
                "proxy_url is required (body or AGENT_PROXY_URL server config)",
            )
        })
}

async fn submit_agent_job(
    state: &AppState,
    vm_id: Uuid,
    operation: &str,
    payload_type: &str,
    data: serde_json::Value,
) -> ApiResult<JobEnqueueResponse> {
    let job_id = Uuid::new_v4();
    let job = build_job(job_id, operation, payload_type, data)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut redis = state.redis.clone();
    enqueue_job(&mut redis, &job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    sqlx::query(
        "INSERT INTO jobs (id, vm_id, operation, status) VALUES ($1, $2, $3, $4)",
    )
    .bind(job_id)
    .bind(vm_id)
    .bind(operation)
    .bind("pending")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(JobEnqueueResponse {
        job_id,
        operation: operation.to_string(),
        status: "pending".to_string(),
    })
}

pub async fn ping_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AgentProxyBody>,
) -> ApiResult<Json<ApiResponse<AgentPingResponse>>> {
    let _vm = load_vm(&state, id).await?;
    let proxy_url = resolve_proxy_url(&state, body.proxy_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let url = format!("{proxy_url}/ping");
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let agent = resp.json().await.ok();
            Ok(Json(ApiResponse::ok(AgentPingResponse {
                reachable: true,
                proxy_url,
                agent,
                error: None,
            })))
        }
        Ok(resp) => Ok(Json(ApiResponse::ok(AgentPingResponse {
            reachable: false,
            proxy_url,
            agent: None,
            error: Some(format!("agent proxy returned {}", resp.status())),
        }))),
        Err(e) => Ok(Json(ApiResponse::ok(AgentPingResponse {
            reachable: false,
            proxy_url,
            agent: None,
            error: Some(e.to_string()),
        }))),
    }
}

pub async fn agent_evidence(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AgentProxyBody>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let proxy_url = resolve_proxy_url(&state, body.proxy_url)?;
    let resp = submit_agent_job(
        &state,
        vm.id,
        "guestkit.agent.evidence",
        "guestkit.agent.evidence.v1",
        serde_json::json!({
            "proxy_url": proxy_url,
            "target": body.target,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn agent_doctor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<AgentQuery>,
    Json(body): Json<AgentProxyBody>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let proxy_url = resolve_proxy_url(&state, body.proxy_url)?;
    let target = if body.target != "kubevirt" {
        body.target
    } else {
        query.target
    };
    let resp = submit_agent_job(
        &state,
        vm.id,
        "guestkit.agent.doctor",
        "guestkit.agent.doctor.v1",
        serde_json::json!({
            "proxy_url": proxy_url,
            "target": target,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn agent_rpc(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AgentRpcBody>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let proxy_url = resolve_proxy_url(&state, body.proxy_url)?;
    if body.method.trim().is_empty() {
        return Err(ApiError::bad_request("method is required"));
    }
    let resp = submit_agent_job(
        &state,
        vm.id,
        "guestkit.agent.call",
        "guestkit.agent.call.v1",
        serde_json::json!({
            "proxy_url": proxy_url,
            "method": body.method,
            "params": body.params,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn agent_fix(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AgentFixBody>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let proxy_url = resolve_proxy_url(&state, body.proxy_url)?;
    let resp = submit_agent_job(
        &state,
        vm.id,
        "guestkit.agent.fix",
        "guestkit.agent.fix.v1",
        serde_json::json!({
            "proxy_url": proxy_url,
            "plan": body.plan,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}
