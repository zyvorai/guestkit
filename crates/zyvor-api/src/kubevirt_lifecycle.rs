// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! KubeVirt VM lifecycle subresources (start / stop / restart).

use axum::extract::{Path, State};
use axum::Json;
use kube::Client;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::routes::kubevirt::{fetch_vmi, json_str};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleResult {
    pub success: bool,
    pub action: String,
    pub message: String,
    pub phase: Option<String>,
    pub printable_status: Option<String>,
}

pub async fn restart_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<LifecycleResult>>> {
    let client = kube_client(&state)?;
    invoke_subresource(&client, &namespace, &name, "restart").await?;
    let phase = wait_for_phase(&client, &namespace, &name, 45).await;
    Ok(Json(ApiResponse::ok(LifecycleResult {
        success: true,
        action: "restart".into(),
        message: format!("Restart requested for {namespace}/{name}"),
        phase: phase.clone(),
        printable_status: phase,
    })))
}

pub async fn start_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<LifecycleResult>>> {
    let client = kube_client(&state)?;
    invoke_subresource(&client, &namespace, &name, "start").await?;
    let phase = wait_for_phase(&client, &namespace, &name, 60).await;
    Ok(Json(ApiResponse::ok(LifecycleResult {
        success: true,
        action: "start".into(),
        message: format!("Start requested for {namespace}/{name}"),
        phase: phase.clone(),
        printable_status: phase,
    })))
}

pub async fn stop_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<LifecycleResult>>> {
    let client = kube_client(&state)?;
    invoke_subresource(&client, &namespace, &name, "stop").await?;
    let phase = wait_for_stopped(&client, &namespace, &name, 60).await;
    Ok(Json(ApiResponse::ok(LifecycleResult {
        success: true,
        action: "stop".into(),
        message: format!("Stop requested for {namespace}/{name}"),
        phase,
        printable_status: Some("Stopped".into()),
    })))
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("KubeVirt lifecycle requires in-cluster Kubernetes access"))
}

async fn invoke_subresource(
    client: &Client,
    namespace: &str,
    name: &str,
    action: &str,
) -> ApiResult<()> {
    let url = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachines/{name}/{action}"
    );
    let req = http::Request::builder()
        .method("PUT")
        .uri(&url)
        .header("Content-Type", "application/json")
        .body(b"{}".to_vec())
        .map_err(|e| ApiError::internal(format!("build {action} request: {e}")))?;
    client
        .request::<Value>(req)
        .await
        .map_err(|e| ApiError::internal(format!("{action} VM {namespace}/{name}: {e}")))?;
    Ok(())
}

async fn wait_for_phase(
    client: &Client,
    namespace: &str,
    name: &str,
    timeout_secs: u64,
) -> Option<String> {
    for _ in 0..timeout_secs {
        if let Some(vmi) = fetch_vmi(client, namespace, name).await {
            if let Some(phase) = json_str(&vmi, &["status", "phase"]) {
                if phase == "Running" || phase == "Scheduled" {
                    return Some(phase);
                }
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
    fetch_vmi(client, namespace, name)
        .await
        .and_then(|v| json_str(&v, &["status", "phase"]))
}

async fn wait_for_stopped(
    client: &Client,
    namespace: &str,
    name: &str,
    timeout_secs: u64,
) -> Option<String> {
    for _ in 0..timeout_secs {
        if fetch_vmi(client, namespace, name).await.is_none() {
            return Some("Stopped".into());
        }
        if let Some(vmi) = fetch_vmi(client, namespace, name).await {
            if let Some(phase) = json_str(&vmi, &["status", "phase"]) {
                if phase == "Succeeded" || phase == "Failed" {
                    return Some(phase);
                }
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
    Some("Stopping".into())
}

/// Stop VM and wait until VMI is gone (used by disk export).
pub async fn stop_vm_for_export(client: &Client, namespace: &str, name: &str) -> ApiResult<()> {
    invoke_subresource(client, namespace, name, "stop").await?;
    for _ in 0..90 {
        if fetch_vmi(client, namespace, name).await.is_none() {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    Err(ApiError::internal(format!(
        "Timed out waiting for {namespace}/{name} to stop"
    )))
}
