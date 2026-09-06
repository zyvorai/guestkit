// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! QEMU guest-agent commands via virt-launcher (KubeVirt has no guest-exec subresource).
//!
//! Transport talks the QGA unix socket inside the virt-launcher pod. It does
//! **not** invoke `virsh` unless `GUESTKIT_ALLOW_VIRSH=1`.

use base64::Engine;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{AttachParams, Api, ListParams};
use kube::Client;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;

use crate::error::{ApiError, ApiResult};

const VIRT_LAUNCHER_CONTAINER: &str = "compute";

#[derive(Debug, Clone)]
pub struct QgaExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

pub fn libvirt_domain(namespace: &str, name: &str) -> String {
    format!("{namespace}_{name}")
}

pub async fn qga_available(client: &Client, namespace: &str, name: &str) -> bool {
    qga_ping(client, namespace, name).await.is_ok()
}

pub async fn qga_ping(client: &Client, namespace: &str, name: &str) -> ApiResult<()> {
    let domain = libvirt_domain(namespace, name);
    let pod = find_virt_launcher_pod(client, namespace, name).await?;
    let out = virsh_qga_json(client, namespace, &pod, &domain, json!({"execute": "guest-ping"}))
        .await?;
    if out.get("return").is_some() {
        Ok(())
    } else if let Some(err) = out.get("error") {
        Err(ApiError::bad_request(format!("guest-ping failed: {err}")))
    } else {
        Err(ApiError::internal(format!("unexpected guest-ping response: {out}")))
    }
}

pub async fn qga_exec(
    client: &Client,
    namespace: &str,
    name: &str,
    path: &str,
    args: &[String],
    timeout_secs: u64,
) -> ApiResult<QgaExecResult> {
    let domain = libvirt_domain(namespace, name);
    let pod = find_virt_launcher_pod(client, namespace, name).await?;
    let exec_body = json!({
        "execute": "guest-exec",
        "arguments": {
            "path": path,
            "arg": args,
            "capture-output": true,
            "env": []
        }
    });
    let resp = virsh_qga_json(client, namespace, &pod, &domain, exec_body).await?;
    let pid = resp
        .pointer("/return/pid")
        .and_then(|p| p.as_u64())
        .ok_or_else(|| ApiError::internal(format!("guest-exec missing pid: {resp}")))?;

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs.max(5));
    loop {
        let status_body = json!({
            "execute": "guest-exec-status",
            "arguments": { "pid": pid }
        });
        let status =
            virsh_qga_json(client, namespace, &pod, &domain, status_body).await?;
        let ret = status
            .get("return")
            .ok_or_else(|| ApiError::internal(format!("guest-exec-status: {status}")))?;
        if ret.get("exited").and_then(|v| v.as_bool()) == Some(true) {
            let stdout = decode_b64_field(ret.get("out-data"));
            let stderr = decode_b64_field(ret.get("err-data"));
            let exit_code = ret
                .get("exitcode")
                .and_then(|v| v.as_i64())
                .unwrap_or(1);
            return Ok(QgaExecResult {
                exit_code,
                stdout,
                stderr,
            });
        }
        if std::time::Instant::now() >= deadline {
            return Err(ApiError::internal(format!(
                "guest-exec pid {pid} timed out after {timeout_secs}s"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

pub async fn qga_exec_shell(
    client: &Client,
    namespace: &str,
    name: &str,
    script: &str,
    timeout_secs: u64,
) -> ApiResult<QgaExecResult> {
    qga_exec(
        client,
        namespace,
        name,
        "/bin/sh",
        &["-c".into(), script.into()],
        timeout_secs,
    )
    .await
}

pub async fn qga_exec_powershell(
    client: &Client,
    namespace: &str,
    name: &str,
    script: &str,
    timeout_secs: u64,
) -> ApiResult<QgaExecResult> {
    qga_exec(
        client,
        namespace,
        name,
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        &[
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-Command".into(),
            script.into(),
        ],
        timeout_secs,
    )
    .await
}

async fn find_virt_launcher_pod(client: &Client, namespace: &str, vm_name: &str) -> ApiResult<String> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    for label in [
        format!("kubevirt.io/vm={vm_name}"),
        format!("kubevirt.io/domain={vm_name}"),
        format!("vm.kubevirt.io/name={vm_name}"),
    ] {
        let lp = ListParams::default().labels(&label);
        if let Ok(list) = pods.list(&lp).await {
            if let Some(pod) = list.items.into_iter().find(|p| {
                p.status
                    .as_ref()
                    .and_then(|s| s.phase.as_deref())
                    == Some("Running")
            }) {
                return pod
                    .metadata
                    .name
                    .clone()
                    .ok_or_else(|| ApiError::internal("virt-launcher pod missing name"));
            }
        }
    }
    Err(ApiError::bad_request(format!(
        "No running virt-launcher pod for VM {namespace}/{vm_name}"
    )))
}

async fn virsh_qga_json(
    client: &Client,
    namespace: &str,
    pod: &str,
    domain: &str,
    body: Value,
) -> ApiResult<Value> {
    let json_cmd = crate::qga_socket::encode_request(&body)
        .map_err(|e| ApiError::internal(format!("serialize QGA command: {e}")))?;
    let stdout = virsh_qga_raw(client, namespace, pod, domain, &json_cmd).await?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(ApiError::internal("empty response from QGA socket"));
    }
    serde_json::from_str(trimmed)
        .map_err(|e| ApiError::internal(format!("parse QGA response {trimmed}: {e}")))
}

async fn virsh_qga_raw(
    client: &Client,
    namespace: &str,
    pod: &str,
    domain: &str,
    json_cmd: &str,
) -> ApiResult<String> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let ap = AttachParams {
        container: Some(VIRT_LAUNCHER_CONTAINER.into()),
        stdout: true,
        stderr: true,
        ..Default::default()
    };
    let mut cmd = crate::qga_socket::qga_exec_argv(domain, crate::qga_socket::allow_virsh_fallback());
    cmd.push(json_cmd.to_string());
    let mut attached = pods
        .exec(pod, cmd, &ap)
        .await
        .map_err(|e| ApiError::internal(format!("exec QGA client in {pod}: {e}")))?;
    let mut stdout = Vec::new();
    if let Some(mut out) = attached.stdout() {
        out.read_to_end(&mut stdout)
            .await
            .map_err(|e| ApiError::internal(format!("read QGA stdout: {e}")))?;
    }
    let mut stderr = Vec::new();
    if let Some(mut err) = attached.stderr() {
        err.read_to_end(&mut stderr)
            .await
            .map_err(|e| ApiError::internal(format!("read QGA stderr: {e}")))?;
    }
    if !stderr.is_empty() && stdout.is_empty() {
        return Err(ApiError::internal(format!(
            "QGA command failed: {}",
            String::from_utf8_lossy(&stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

fn decode_b64_field(value: Option<&Value>) -> String {
    value
        .and_then(|v| v.as_str())
        .map(|s| {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_else(|_| s.to_string())
        })
        .unwrap_or_default()
}

fn encode_b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

const QGA_FILE_CHUNK: usize = 48 * 1024;

/// Write bytes to a guest path via guest-file-open/write/close (airgap installer path).
pub async fn qga_file_write(
    client: &Client,
    namespace: &str,
    name: &str,
    guest_path: &str,
    bytes: &[u8],
    timeout_secs: u64,
) -> ApiResult<()> {
    let domain = libvirt_domain(namespace, name);
    let pod = find_virt_launcher_pod(client, namespace, name).await?;
    let open_body = json!({
        "execute": "guest-file-open",
        "arguments": {
            "path": guest_path,
            "mode": "w+"
        }
    });
    let open_resp = virsh_qga_json(client, namespace, &pod, &domain, open_body).await?;
    let handle = open_resp
        .pointer("/return")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ApiError::internal(format!("guest-file-open missing handle: {open_resp}")))?;

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs.max(30));
    let mut offset = 0usize;
    while offset < bytes.len() {
        if std::time::Instant::now() >= deadline {
            let _ = qga_file_close(client, namespace, &pod, &domain, handle).await;
            return Err(ApiError::internal(format!(
                "guest-file-write timed out after {timeout_secs}s"
            )));
        }
        let end = (offset + QGA_FILE_CHUNK).min(bytes.len());
        let chunk = &bytes[offset..end];
        let write_body = json!({
            "execute": "guest-file-write",
            "arguments": {
                "handle": handle,
                "buf-b64": encode_b64(chunk),
                "count": chunk.len()
            }
        });
        let write_resp =
            virsh_qga_json(client, namespace, &pod, &domain, write_body).await?;
        let written = write_resp
            .pointer("/return/count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        if written == 0 {
            let _ = qga_file_close(client, namespace, &pod, &domain, handle).await;
            return Err(ApiError::internal(format!(
                "guest-file-write stalled at offset {offset}: {write_resp}"
            )));
        }
        offset += written;
    }
    qga_file_close(client, namespace, &pod, &domain, handle).await
}

/// Read a guest file via guest-file-open/read/close.
pub async fn qga_file_read(
    client: &Client,
    namespace: &str,
    name: &str,
    guest_path: &str,
    max_bytes: usize,
    timeout_secs: u64,
) -> ApiResult<Vec<u8>> {
    let domain = libvirt_domain(namespace, name);
    let pod = find_virt_launcher_pod(client, namespace, name).await?;
    let open_body = json!({
        "execute": "guest-file-open",
        "arguments": {
            "path": guest_path,
            "mode": "r"
        }
    });
    let open_resp = virsh_qga_json(client, namespace, &pod, &domain, open_body).await?;
    let handle = open_resp
        .pointer("/return")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ApiError::internal(format!("guest-file-open missing handle: {open_resp}")))?;

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs.max(30));
    let mut out = Vec::new();
    loop {
        if std::time::Instant::now() >= deadline {
            let _ = qga_file_close(client, namespace, &pod, &domain, handle).await;
            return Err(ApiError::internal(format!(
                "guest-file-read timed out after {timeout_secs}s"
            )));
        }
        if out.len() >= max_bytes {
            break;
        }
        let to_read = (max_bytes - out.len()).min(QGA_FILE_CHUNK);
        let read_body = json!({
            "execute": "guest-file-read",
            "arguments": {
                "handle": handle,
                "count": to_read
            }
        });
        let read_resp = virsh_qga_json(client, namespace, &pod, &domain, read_body).await?;
        let ret = read_resp
            .get("return")
            .ok_or_else(|| ApiError::internal(format!("guest-file-read: {read_resp}")))?;
        if ret.get("eof").and_then(|v| v.as_bool()) == Some(true) {
            break;
        }
        let chunk_b64 = ret.get("buf-b64").and_then(|v| v.as_str()).unwrap_or("");
        if chunk_b64.is_empty() {
            break;
        }
        let chunk = base64::engine::general_purpose::STANDARD
            .decode(chunk_b64)
            .map_err(|e| ApiError::internal(format!("decode guest-file-read: {e}")))?;
        out.extend_from_slice(&chunk);
    }
    qga_file_close(client, namespace, &pod, &domain, handle).await?;
    Ok(out)
}

async fn qga_file_close(
    client: &Client,
    namespace: &str,
    pod: &str,
    domain: &str,
    handle: i64,
) -> ApiResult<()> {
    let close_body = json!({
        "execute": "guest-file-close",
        "arguments": { "handle": handle }
    });
    let close_resp = virsh_qga_json(client, namespace, pod, domain, close_body).await?;
    if close_resp.get("return").is_some() {
        Ok(())
    } else if let Some(err) = close_resp.get("error") {
        Err(ApiError::internal(format!("guest-file-close: {err}")))
    } else {
        Ok(())
    }
}

/// Truncate guest exec output for policy limits.
pub fn truncate_exec_output(stdout: &str, stderr: &str, max_bytes: usize) -> (String, String) {
    let mut out = stdout.to_string();
    let mut err = stderr.to_string();
    if out.len() > max_bytes {
        out.truncate(max_bytes);
        out.push_str("…[truncated]");
    }
    if err.len() > max_bytes {
        err.truncate(max_bytes);
        err.push_str("…[truncated]");
    }
    (out, err)
}
