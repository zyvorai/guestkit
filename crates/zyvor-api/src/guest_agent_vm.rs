// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Per-VM guest agent JSON-RPC via QEMU guest-agent guest-exec.

use base64::{engine::general_purpose::STANDARD, Engine};
use kube::Client;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::kubevirt_qga::{qga_available, qga_exec_powershell, qga_exec_shell};

pub const IN_GUEST_SOCKET_PATH: &str = "/var/run/zyvor/guest-agent.sock";

fn build_rpc_request(method: &str, params: Value) -> ApiResult<String> {
    let req = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let req_bytes =
        serde_json::to_vec(&req).map_err(|e| ApiError::internal(format!("serialize rpc: {e}")))?;
    Ok(STANDARD.encode(&req_bytes))
}

fn parse_rpc_stdout(stdout: &str) -> ApiResult<Value> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(ApiError::internal("empty guest rpc response"));
    }
    let envelope: Value = serde_json::from_str(trimmed)
        .map_err(|e| ApiError::internal(format!("parse guest rpc response: {e} — {trimmed}")))?;
    Ok(envelope)
}

/// JSON-RPC via in-guest Unix socket (requires agent daemon + socket file).
pub async fn vm_in_guest_socket_rpc(
    client: &Client,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
    is_windows: bool,
) -> ApiResult<Value> {
    if !qga_available(client, namespace, name).await {
        return Err(ApiError::bad_request(
            "QEMU guest agent not available for in-guest socket RPC",
        ));
    }

    let b64 = build_rpc_request(method, params)?;
    let script = if is_windows {
        format!(
            r#"$b64='{b64}'; $agent='C:\Program Files\Zyvor\zyvor-guest-agent.exe'; \
if (-not (Test-Path $agent)) {{ exit 2 }}; \
$bytes=[Convert]::FromBase64String($b64); \
$req=[Text.Encoding]::UTF8.GetString($bytes); \
$req | & $agent rpc 2>$null; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}"#
        )
    } else {
        format!(
            "B64='{b64}'; SOCK='{IN_GUEST_SOCKET_PATH}'; \
             [ -S \"$SOCK\" ] || exit 2; \
             AGENT=$(command -v zyvor-guest-agent 2>/dev/null || echo /usr/local/bin/zyvor-guest-agent); \
             echo \"$B64\" | base64 -d | \"$AGENT\" rpc 2>/dev/null"
        )
    };

    let exec = if is_windows {
        qga_exec_powershell(client, namespace, name, &script, 120).await?
    } else {
        qga_exec_shell(client, namespace, name, &script, 120).await?
    };

    if exec.exit_code == 2 {
        return Err(ApiError::bad_request("zyvor-guest-agent socket not available"));
    }
    if exec.exit_code != 0 {
        return Err(ApiError::bad_request(format!(
            "in-guest socket rpc failed (exit {}): {}",
            exec.exit_code,
            exec.stderr.trim()
        )));
    }

    parse_rpc_stdout(&exec.stdout)
}

/// Invoke GuestKit JSON-RPC inside the guest via `zyvor-guest-agent rpc` (QGA guest-exec).
pub async fn vm_guestkit_rpc(
    client: &Client,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
) -> ApiResult<Value> {
    if !qga_available(client, namespace, name).await {
        return Err(ApiError::bad_request(
            "QEMU guest agent not available for per-VM pull",
        ));
    }

    let b64 = build_rpc_request(method, params)?;
    let script = format!(
        "B64='{b64}'; AGENT=$(command -v zyvor-guest-agent 2>/dev/null || echo /usr/local/bin/zyvor-guest-agent); \
         echo \"$B64\" | base64 -d | \"$AGENT\" rpc 2>/dev/null || \
         echo \"$B64\" | base64 -d | \"$AGENT\" --rpc 2>/dev/null || \
         echo \"$B64\" | base64 -d | /usr/local/bin/zyvor-guest-agent rpc 2>/dev/null"
    );

    let exec = qga_exec_shell(client, namespace, name, &script, 120).await?;
    if exec.exit_code != 0 {
        return Err(ApiError::bad_request(format!(
            "guest rpc failed (exit {}): {}",
            exec.exit_code,
            exec.stderr.trim()
        )));
    }

    parse_rpc_stdout(&exec.stdout)
}

/// Quick probe: is zyvor-guest-agent daemon active inside the guest?
pub async fn vm_agent_daemon_active(
    client: &Client,
    namespace: &str,
    name: &str,
    is_windows: bool,
) -> bool {
    let script = if is_windows {
        "(Get-Service -Name 'zyvor-guest-agent' -ErrorAction SilentlyContinue | Where-Object Status -eq 'Running') -ne $null"
    } else {
        "systemctl is-active zyvor-guest-agent 2>/dev/null | grep -q '^active$'"
    };
    let exec = if is_windows {
        qga_exec_powershell(client, namespace, name, script, 15).await
    } else {
        qga_exec_shell(client, namespace, name, script, 15).await
    };
    matches!(exec, Ok(ref r) if r.exit_code == 0)
}
