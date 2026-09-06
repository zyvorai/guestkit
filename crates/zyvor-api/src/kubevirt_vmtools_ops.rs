// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Zeus VM Tools guest lifecycle via KubeVirt VMI subresources (freeze, softreboot, stop).

use axum::extract::{Path, State};
use axum::Json;
use kube::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::routes::kubevirt::{fetch_vmi, json_str};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsOpResult {
    pub success: bool,
    pub action: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExecRequest {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ShutdownRequest {
    #[serde(default)]
    pub grace_period_seconds: Option<i64>,
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("VM Tools ops require in-cluster Kubernetes access"))
}

async fn require_running_vmi(client: &Client, namespace: &str, name: &str) -> ApiResult<Value> {
    let vmi = fetch_vmi(client, namespace, name)
        .await
        .ok_or_else(|| ApiError::bad_request(format!("VM {namespace}/{name} is not running")))?;
    let phase = json_str(&vmi, &["status", "phase"]).unwrap_or_default();
    if phase != "Running" {
        return Err(ApiError::bad_request(format!(
            "VM {namespace}/{name} is not running (phase={phase})"
        )));
    }
    Ok(vmi)
}

fn agent_connected(vmi: &Value) -> bool {
    vmi.pointer("/status/conditions")
        .and_then(|c| c.as_array())
        .map(|conds| {
            conds.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("AgentConnected")
                    && c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false)
}

async fn invoke_vmi_subresource(
    client: &Client,
    namespace: &str,
    name: &str,
    action: &str,
    body: Value,
) -> ApiResult<()> {
    let url = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachineinstances/{name}/{action}"
    );
    let payload = serde_json::to_vec(&body)
        .map_err(|e| ApiError::internal(format!("serialize {action} body: {e}")))?;
    let req = http::Request::builder()
        .method("PUT")
        .uri(&url)
        .header("Content-Type", "application/json")
        .body(payload)
        .map_err(|e| ApiError::internal(format!("build {action} request: {e}")))?;
    match client.request::<Value>(req).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            // freeze/unfreeze/softreboot often return 200 with an empty body
            if msg.contains("EOF while parsing") || msg.contains("error decoding response body") {
                Ok(())
            } else {
                Err(ApiError::internal(format!(
                    "{action} VMI {namespace}/{name}: {msg}"
                )))
            }
        }
    }
}

async fn invoke_vm_stop(
    client: &Client,
    namespace: &str,
    name: &str,
    grace_period_seconds: i64,
) -> ApiResult<()> {
    let url = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachines/{name}/stop"
    );
    let body = json!({ "gracePeriod": grace_period_seconds });
    let payload = serde_json::to_vec(&body)
        .map_err(|e| ApiError::internal(format!("serialize stop body: {e}")))?;
    let req = http::Request::builder()
        .method("PUT")
        .uri(&url)
        .header("Content-Type", "application/json")
        .body(payload)
        .map_err(|e| ApiError::internal(format!("build stop request: {e}")))?;
    client
        .request::<Value>(req)
        .await
        .map_err(|e| ApiError::internal(format!("stop VM {namespace}/{name}: {e}")))?;
    Ok(())
}

pub async fn quiesce_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<VMToolsOpResult>>> {
    let client = kube_client(&state)?;
    let vmi = require_running_vmi(&client, &namespace, &name).await?;
    if !agent_connected(&vmi) {
        return Err(ApiError::bad_request(
            "Guest agent not connected — install Zeus VM Tools before snapshot quiesce",
        ));
    }
    invoke_vmi_subresource(
        &client,
        &namespace,
        &name,
        "freeze",
        json!({ "unfreezeTimeout": "300s" }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(VMToolsOpResult {
        success: true,
        action: "quiesce".into(),
        message: format!("Filesystem quiesced for {namespace}/{name}"),
        phase: json_str(&vmi, &["status", "phase"]),
    })))
}

pub async fn unquiesce_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<VMToolsOpResult>>> {
    let client = kube_client(&state)?;
    let vmi = require_running_vmi(&client, &namespace, &name).await?;
    invoke_vmi_subresource(&client, &namespace, &name, "unfreeze", json!({})).await?;
    Ok(Json(ApiResponse::ok(VMToolsOpResult {
        success: true,
        action: "unquiesce".into(),
        message: format!("Filesystem thawed for {namespace}/{name}"),
        phase: json_str(&vmi, &["status", "phase"]),
    })))
}

pub async fn reboot_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<VMToolsOpResult>>> {
    let client = kube_client(&state)?;
    let vmi = require_running_vmi(&client, &namespace, &name).await?;
    if !agent_connected(&vmi) {
        return Err(ApiError::bad_request(
            "Guest agent not connected — use cluster Restart or install Zeus VM Tools first",
        ));
    }
    invoke_vmi_subresource(&client, &namespace, &name, "softreboot", json!({})).await?;
    Ok(Json(ApiResponse::ok(VMToolsOpResult {
        success: true,
        action: "reboot".into(),
        message: format!("Guest soft reboot requested for {namespace}/{name}"),
        phase: json_str(&vmi, &["status", "phase"]),
    })))
}

pub async fn shutdown_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(body): Json<ShutdownRequest>,
) -> ApiResult<Json<ApiResponse<VMToolsOpResult>>> {
    let client = kube_client(&state)?;
    let vmi = fetch_vmi(&client, &namespace, &name).await;
    let running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");
    if !running {
        return Err(ApiError::bad_request(format!(
            "VM {namespace}/{name} is not running"
        )));
    }
    let grace = body.grace_period_seconds.unwrap_or(60).clamp(0, 300);
    invoke_vm_stop(&client, &namespace, &name, grace).await?;
    Ok(Json(ApiResponse::ok(VMToolsOpResult {
        success: true,
        action: "shutdown".into(),
        message: format!(
            "Graceful shutdown requested for {namespace}/{name} (gracePeriod={grace}s)"
        ),
        phase: Some("Stopping".into()),
    })))
}

pub async fn exec_vm_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(body): Json<ExecRequest>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let client = kube_client(&state)?;
    let vmi = require_running_vmi(&client, &namespace, &name).await?;
    if !agent_connected(&vmi) {
        return Err(ApiError::bad_request(
            "Guest agent not connected — install qemu-guest-agent or Zeus VM Tools first",
        ));
    }

    let command = if let Some(cmd) = body.command.as_deref().filter(|s| !s.is_empty()) {
        cmd.to_string()
    } else {
        let path = body
            .path
            .as_deref()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| ApiError::bad_request("Provide command or path"))?;
        format!("{} {}", path, body.args.join(" "))
    };
    let max_out =
        crate::guest_action_policy::enforce_exec(state.kube.as_ref(), &command, false).await?;

    let (path, args) = if let Some(cmd) = body.command.as_deref().filter(|s| !s.is_empty()) {
        if cmd.contains('\\') || cmd.to_ascii_lowercase().contains("powershell") {
            (
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".to_string(),
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-Command".into(),
                    cmd.to_string(),
                ],
            )
        } else {
            ("/bin/sh".into(), vec!["-c".into(), cmd.to_string()])
        }
    } else {
        let path = body
            .path
            .filter(|p| !p.is_empty())
            .ok_or_else(|| ApiError::bad_request("Provide command or path"))?;
        (path, body.args)
    };

    let result = crate::kubevirt_qga::qga_exec(&client, &namespace, &name, &path, &args, 120).await?;
    let (stdout, stderr) = crate::kubevirt_qga::truncate_exec_output(
        &result.stdout,
        &result.stderr,
        max_out,
    );
    Ok(Json(ApiResponse::ok(json!({
        "success": result.exit_code == 0,
        "exitCode": result.exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "method": "qga-guest-exec",
        "hostMediated": true,
    }))))
}
