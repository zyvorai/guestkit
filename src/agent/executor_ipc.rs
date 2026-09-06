// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Privileged executor IPC over Unix socket.

use anyhow::{Context, Result};
use guestkit_agent_protocol::{read_frame, write_frame};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread;

pub const EXEC_SOCKET_PATH: &str = "/var/run/zyvor/guest-agent-exec.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    pub action: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(unix)]
pub fn spawn_executor_server() -> Result<()> {
    IN_HELPER.store(true, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::var("ZYVOR_EXEC_SOCKET").unwrap_or_else(|_| EXEC_SOCKET_PATH.to_string());
    if Path::new(&path).exists() {
        fs::remove_file(&path).ok();
    }
    let parent = Path::new(&path)
        .parent()
        .unwrap_or(Path::new("/var/run/zyvor"));
    fs::create_dir_all(parent).ok();

    let listener =
        UnixListener::bind(&path).with_context(|| format!("bind executor socket {path}"))?;
    secure_executor_socket(&path);
    log::info!("Zyvor guest agent executor listening on {path}");

    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            thread::spawn(move || {
                if let Err(e) = serve_connection(conn) {
                    log::debug!("executor client error: {e}");
                }
            });
        }
    });

    Ok(())
}

#[cfg(unix)]
fn secure_executor_socket(path: &str) {
    use nix::unistd::{chown, Group, Uid};
    use std::os::unix::fs::PermissionsExt;

    if let Ok(Some(group)) = Group::from_name("zyvor-agent") {
        let _ = chown(path, Some(Uid::from_raw(0)), Some(group.gid));
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o660);
            let _ = fs::set_permissions(path, perms);
        }
    }
}

#[cfg(not(unix))]
fn secure_executor_socket(_path: &str) {}

#[cfg(unix)]
fn serve_connection(stream: UnixStream) -> Result<()> {
    let mut reader = stream.try_clone()?;
    let mut writer = stream;
    let frame = read_frame(&mut reader).map_err(|e| anyhow::anyhow!("{e}"))?;
    let req: ExecRequest = serde_json::from_slice(&frame).context("parse exec request")?;
    let executor = crate::agent::executor::Executor::new();
    let resp = dispatch(&executor, &req);
    let bytes = serde_json::to_vec(&resp)?;
    write_frame(&mut writer, &bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

#[cfg(unix)]
fn dispatch(executor: &crate::agent::executor::Executor, req: &ExecRequest) -> ExecResponse {
    match req.action.as_str() {
        "restart_unit" => {
            let unit = req
                .params
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match executor.restart_unit(unit) {
                Ok(msg) => ExecResponse {
                    ok: true,
                    result: Some(Value::String(msg)),
                    error: None,
                },
                Err(e) => ExecResponse {
                    ok: false,
                    result: None,
                    error: Some(e.to_string()),
                },
            }
        }
        "execute_remediation_plan" => {
            let plan_id = req
                .params
                .get("plan_id")
                .and_then(|v| v.as_str())
                .unwrap_or("local");
            let actions: Vec<crate::agent::executor::RemediationActionSpec> = req
                .params
                .get("actions")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let result = executor.execute_remediation_plan(plan_id, &actions);
            ExecResponse {
                ok: result.success,
                result: Some(serde_json::to_value(result).unwrap_or(Value::Null)),
                error: None,
            }
        }
        "collect_support_bundle" => match executor.collect_support_bundle() {
            Ok(bytes) => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                ExecResponse {
                    ok: true,
                    result: Some(serde_json::json!({
                        "format": "tar.zst",
                        "encoding": "base64",
                        "data": STANDARD.encode(bytes),
                    })),
                    error: None,
                }
            }
            Err(e) => ExecResponse {
                ok: false,
                result: None,
                error: Some(e.to_string()),
            },
        },
        "start_unit" | "stop_unit" => {
            let op = req.action.trim_end_matches("_unit");
            let unit = req
                .params
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match executor.control_unit(op, unit) {
                Ok(msg) => ExecResponse {
                    ok: true,
                    result: Some(Value::String(msg)),
                    error: None,
                },
                Err(e) => ExecResponse {
                    ok: false,
                    result: None,
                    error: Some(e.to_string()),
                },
            }
        }
        "power_action" => {
            let action = req
                .params
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("reboot");
            let delay_secs = req
                .params
                .get("delay_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            match run_power_action(action, delay_secs) {
                Ok(msg) => ExecResponse {
                    ok: true,
                    result: Some(Value::String(msg)),
                    error: None,
                },
                Err(e) => ExecResponse {
                    ok: false,
                    result: None,
                    error: Some(e.to_string()),
                },
            }
        }
        "expand_filesystem" => {
            let command = req
                .params
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Only the specific expansion commands the agent generates.
            let allowed = command.starts_with("xfs_growfs ")
                || command.starts_with("resize2fs ")
                || command.starts_with("btrfs filesystem resize ")
                || command.starts_with("lvextend ");
            if !allowed || command.contains(';') || command.contains('|') || command.contains('&') {
                ExecResponse {
                    ok: false,
                    result: None,
                    error: Some(format!("refused expansion command: {command}")),
                }
            } else {
                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .status()
                {
                    Ok(s) if s.success() => ExecResponse {
                        ok: true,
                        result: Some(Value::String(format!("expanded via {command}"))),
                        error: None,
                    },
                    Ok(s) => ExecResponse {
                        ok: false,
                        result: None,
                        error: Some(format!("{command}: {s}")),
                    },
                    Err(e) => ExecResponse {
                        ok: false,
                        result: None,
                        error: Some(e.to_string()),
                    },
                }
            }
        }
        "time_sync" => match run_time_sync() {
            Ok(msg) => ExecResponse {
                ok: true,
                result: Some(Value::String(msg)),
                error: None,
            },
            Err(e) => ExecResponse {
                ok: false,
                result: None,
                error: Some(e.to_string()),
            },
        },
        "apply_staged_update" => match crate::agent::updater::apply_staged_update_privileged() {
            Ok(msg) => ExecResponse {
                ok: true,
                result: Some(serde_json::json!({ "message": msg })),
                error: None,
            },
            Err(e) => ExecResponse {
                ok: false,
                result: None,
                error: Some(e.to_string()),
            },
        },
        other => ExecResponse {
            ok: false,
            result: None,
            error: Some(format!("unsupported executor action: {other}")),
        },
    }
}

/// Schedule a reboot/shutdown via the platform shutdown command.
pub fn run_power_action(action: &str, delay_secs: u64) -> Result<String> {
    use std::process::Command;
    let status = if cfg!(target_os = "windows") {
        let flag = if action == "reboot" { "/r" } else { "/s" };
        Command::new("shutdown")
            .args([flag, "/t", &delay_secs.to_string()])
            .status()?
    } else {
        // shutdown(8) takes minutes; round the delay up.
        let minutes = delay_secs.div_ceil(60);
        let flag = if action == "reboot" { "-r" } else { "-h" };
        Command::new("shutdown")
            .args([flag, &format!("+{minutes}")])
            .status()?
    };
    if status.success() {
        Ok(format!("{action} scheduled in {delay_secs}s"))
    } else {
        anyhow::bail!("shutdown command failed: {status}")
    }
}

/// Trigger an immediate clock sync via chrony/timedatectl (or w32tm).
pub fn run_time_sync() -> Result<String> {
    use std::process::Command;
    let attempts: &[(&str, &[&str])] = if cfg!(target_os = "windows") {
        &[("w32tm", &["/resync"])]
    } else {
        &[
            ("chronyc", &["makestep"]),
            ("timedatectl", &["set-ntp", "true"]),
        ]
    };
    for (cmd, args) in attempts {
        if let Ok(output) = Command::new(cmd).args(*args).output() {
            if output.status.success() {
                return Ok(format!("time sync triggered via {cmd}"));
            }
        }
    }
    anyhow::bail!("no time sync mechanism succeeded (tried chronyc/timedatectl or w32tm)")
}

#[cfg(not(unix))]
pub fn call_executor(_action: &str, _params: Value) -> Result<Value> {
    anyhow::bail!("privileged helper IPC is Unix-only; Windows runs in-process")
}

#[cfg(not(unix))]
pub fn spawn_executor_server() -> Result<()> {
    anyhow::bail!("privileged helper IPC is Unix-only")
}

#[cfg(unix)]
pub fn call_executor(action: &str, params: Value) -> Result<Value> {
    let path = std::env::var("ZYVOR_EXEC_SOCKET").unwrap_or_else(|_| EXEC_SOCKET_PATH.to_string());
    if !Path::new(&path).exists() {
        return Err(anyhow::anyhow!("executor socket not available at {path}"));
    }
    let mut stream = UnixStream::connect(&path).with_context(|| format!("connect {path}"))?;
    let req = ExecRequest {
        action: action.to_string(),
        params,
    };
    let bytes = serde_json::to_vec(&req)?;
    write_frame(&mut stream, &bytes)?;
    let mut reader = stream.try_clone()?;
    let frame = read_frame(&mut reader).map_err(|e| anyhow::anyhow!("{e}"))?;
    let resp: ExecResponse = serde_json::from_slice(&frame).context("parse exec response")?;
    if resp.ok {
        Ok(resp.result.unwrap_or(Value::Null))
    } else {
        Err(anyhow::anyhow!(
            "{}",
            resp.error.unwrap_or_else(|| "executor failed".into())
        ))
    }
}

/// Set inside the privileged helper process so executor methods invoked by
/// `dispatch` run their local fallback instead of connecting back to the
/// helper's own socket (which would recurse indefinitely).
static IN_HELPER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn executor_available() -> bool {
    if cfg!(not(unix)) {
        return false;
    }
    if IN_HELPER.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    let path = std::env::var("ZYVOR_EXEC_SOCKET").unwrap_or_else(|_| EXEC_SOCKET_PATH.to_string());
    Path::new(&path).exists()
}
