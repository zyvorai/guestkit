// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Host-mediated polling for AirgapLive VMs without push telemetry.
//!
//! Each reconcile cycle stores per-VM poll payloads (method latency + transport
//! attempts) and a fleet telemetry rollup for ops dashboards.

use guestkit_agent_protocol::capabilities::{
    METHOD_GET_CAPABILITIES, METHOD_GET_GUEST_HEALTH,
};
use redis::AsyncCommands;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

use crate::error::ApiResult;
use crate::routes::kubevirt::{list_dynamic_all, vm_resource};
use crate::state::AppState;

use super::capabilities::{ControlState, TransportAttempt};
use super::transport::{probe_guest_context, pull_method};

const POLL_KEY_PREFIX: &str = "guest-agent:vm-poll";
const FLEET_KEY: &str = "guest-agent:poll-fleet";
const POLL_TTL_SECS: u64 = 600;
const FLEET_TTL_SECS: u64 = 900;

fn poll_key(namespace: &str, name: &str) -> String {
    format!("{POLL_KEY_PREFIX}:{namespace}:{name}")
}

pub async fn store_poll_result(
    redis: &mut redis::aio::ConnectionManager,
    namespace: &str,
    name: &str,
    payload: &Value,
) -> Result<(), crate::error::ApiError> {
    let raw =
        serde_json::to_string(payload).map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(poll_key(namespace, name), &raw, POLL_TTL_SECS)
        .await
        .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    Ok(())
}

pub async fn load_poll_result(
    redis: &mut redis::aio::ConnectionManager,
    namespace: &str,
    name: &str,
) -> Result<Option<Value>, crate::error::ApiError> {
    let raw: Option<String> = redis
        .get(poll_key(namespace, name))
        .await
        .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    match raw {
        Some(s) => {
            let v = serde_json::from_str(&s)
                .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

pub async fn load_fleet_telemetry(
    redis: &mut redis::aio::ConnectionManager,
) -> Result<Option<Value>, crate::error::ApiError> {
    let raw: Option<String> = redis
        .get(FLEET_KEY)
        .await
        .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    match raw {
        Some(s) => {
            let v = serde_json::from_str(&s)
                .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

async fn store_fleet_telemetry(
    redis: &mut redis::aio::ConnectionManager,
    payload: &Value,
) -> Result<(), crate::error::ApiError> {
    let raw =
        serde_json::to_string(payload).map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(FLEET_KEY, &raw, FLEET_TTL_SECS)
        .await
        .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    Ok(())
}

fn attempts_json(attempts: &[TransportAttempt]) -> Value {
    serde_json::to_value(attempts).unwrap_or_else(|_| json!([]))
}

/// Timed pull that records method latency + transport attempts for telemetry.
async fn timed_pull(
    state: &AppState,
    namespace: &str,
    name: &str,
    method: &str,
) -> Value {
    let t0 = Instant::now();
    match pull_method(state, namespace, name, method, json!({})).await {
        Ok(r) => json!({
            "ok": true,
            "method": method,
            "latencyMs": t0.elapsed().as_millis() as u64,
            "transport": r.transport.as_str(),
            "attempts": attempts_json(&r.attempts),
            "value": r.value,
        }),
        Err(e) => json!({
            "ok": false,
            "method": method,
            "latencyMs": t0.elapsed().as_millis() as u64,
            "error": e.to_string(),
        }),
    }
}

pub async fn reconcile_airgap_polls(state: &AppState) -> ApiResult<Value> {
    let started = Instant::now();
    let client = match state.kube.as_ref() {
        Some(c) => c,
        None => {
            let summary = json!({
                "scanned": 0,
                "polled": 0,
                "skipped": 0,
                "errors": ["kubernetes client unavailable"],
                "durationMs": 0,
                "reconciledAt": chrono::Utc::now().to_rfc3339(),
            });
            let _ = store_fleet_telemetry(&mut state.redis.clone(), &summary).await;
            return Ok(summary);
        }
    };

    let vms = list_dynamic_all(client, &vm_resource())
        .await
        .unwrap_or_default();
    let mut scanned = 0usize;
    let mut polled = 0usize;
    let mut skipped = 0usize;
    let mut method_ok = 0usize;
    let mut method_err = 0usize;
    let mut total_latency_ms = 0u64;
    let mut errors = Vec::new();
    let mut samples = Vec::new();

    for vm in vms {
        let namespace = vm
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = vm
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if namespace.is_empty() || name.is_empty() {
            continue;
        }
        scanned += 1;
        let ctx = probe_guest_context(state, &namespace, &name).await;
        if ctx.control_state != ControlState::AirgapLive || ctx.push_registered {
            skipped += 1;
            continue;
        }

        let vm_t0 = Instant::now();
        let ping = timed_pull(state, &namespace, &name, "guestkit.ping").await;
        let health = timed_pull(state, &namespace, &name, METHOD_GET_GUEST_HEALTH).await;
        let caps = timed_pull(state, &namespace, &name, METHOD_GET_CAPABILITIES).await;

        for pull in [&ping, &health, &caps] {
            let lat = pull.get("latencyMs").and_then(|v| v.as_u64()).unwrap_or(0);
            total_latency_ms = total_latency_ms.saturating_add(lat);
            if pull.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                method_ok += 1;
            } else {
                method_err += 1;
                if let Some(err) = pull.get("error").and_then(|v| v.as_str()) {
                    errors.push(format!("{namespace}/{name}: {err}"));
                }
            }
        }

        let poll_ok = ping.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        let poll_data = json!({
            "namespace": namespace,
            "name": name,
            "controlState": ctx.control_state.as_str(),
            "transport": ctx.active_transport.as_str(),
            "agentVersion": ctx.agent_version,
            "polledAt": chrono::Utc::now().to_rfc3339(),
            "durationMs": vm_t0.elapsed().as_millis() as u64,
            "ok": poll_ok,
            "probeAttempts": attempts_json(&ctx.attempts),
            "methods": {
                "ping": ping,
                "guestHealth": health,
                "capabilities": caps,
            },
            // Back-compat aliases used by older consumers
            "ping": ping.get("value").cloned().unwrap_or(Value::Null),
            "pingTransport": ping.get("transport").cloned().unwrap_or(Value::Null),
            "guestHealth": health.get("value").cloned().unwrap_or(Value::Null),
            "capabilities": caps.get("value").cloned().unwrap_or(Value::Null),
        });

        if let Err(e) =
            store_poll_result(&mut state.redis.clone(), &namespace, &name, &poll_data).await
        {
            errors.push(format!("{namespace}/{name} redis: {}", e.message));
        } else {
            polled += 1;
            if samples.len() < 25 {
                samples.push(json!({
                    "namespace": namespace,
                    "name": name,
                    "ok": poll_ok,
                    "durationMs": poll_data["durationMs"],
                    "transport": ctx.active_transport.as_str(),
                }));
            }
        }
    }

    let duration_ms = started.elapsed().as_millis() as u64;
    let summary = json!({
        "scanned": scanned,
        "polled": polled,
        "skipped": skipped,
        "methodOk": method_ok,
        "methodErr": method_err,
        "totalMethodLatencyMs": total_latency_ms,
        "avgMethodLatencyMs": if method_ok + method_err > 0 {
            total_latency_ms / (method_ok + method_err) as u64
        } else {
            0
        },
        "durationMs": duration_ms,
        "errors": errors,
        "samples": samples,
        "reconciledAt": chrono::Utc::now().to_rfc3339(),
        "telemetryMode": "pull_via_virt_launcher",
    });

    if let Err(e) = store_fleet_telemetry(&mut state.redis.clone(), &summary).await {
        tracing::warn!("airgap fleet telemetry store failed: {}", e.message);
    }

    Ok(summary)
}

/// Background worker: poll airgap VMs on an interval (default 30s).
pub fn spawn_airgap_poll_worker(state: AppState) {
    let interval_secs = std::env::var("GUEST_AIRGAP_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    if std::env::var("GUEST_AIRGAP_POLL_ENABLED")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        tracing::info!("guest airgap poll worker disabled");
        return;
    }
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            match reconcile_airgap_polls(&state).await {
                Ok(summary) => {
                    let polled = summary.get("polled").and_then(|v| v.as_u64()).unwrap_or(0);
                    let errs = summary
                        .get("errors")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    if polled > 0 || errs > 0 {
                        tracing::info!(
                            polled,
                            errors = errs,
                            duration_ms = summary.get("durationMs").and_then(|v| v.as_u64()).unwrap_or(0),
                            avg_latency_ms = summary
                                .get("avgMethodLatencyMs")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            "airgap poll reconcile"
                        );
                    } else {
                        tracing::debug!("airgap poll reconcile: {summary}");
                    }
                }
                Err(e) => tracing::warn!("airgap poll reconcile failed: {}", e.message),
            }
        }
    });
    tracing::info!("guest airgap poll worker started (interval={interval_secs}s)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_key_format() {
        assert_eq!(poll_key("ns", "vm"), "guest-agent:vm-poll:ns:vm");
    }

    #[test]
    fn attempts_json_empty() {
        let v = attempts_json(&[]);
        assert!(v.as_array().unwrap().is_empty());
    }
}
