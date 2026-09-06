// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Transport ladder for guest control operations.

use guestkit_agent_protocol::capabilities::METHOD_GET_CAPABILITIES;
use serde_json::Value;
use std::time::Instant;

use crate::guest_agent_vm::{vm_agent_daemon_active, vm_in_guest_socket_rpc};
use crate::kubevirt_guest_agent::{vm_is_windows, zyvor_tools_connected};
use crate::routes::guest_agent::fetch_vm_guest_report;
use crate::routes::kubevirt::{fetch_vm, fetch_vmi};
use crate::state::AppState;

use super::capabilities::{
    build_capabilities, infer_control_state, ControlState, GuestTransport, TransportAttempt,
};

#[derive(Debug, Clone)]
pub struct GuestContext {
    pub namespace: String,
    pub name: String,
    pub vmi_running: bool,
    pub qga_connected: bool,
    pub zyvor_agent_installed: bool,
    pub agent_daemon_running: bool,
    pub network_available: bool,
    pub push_registered: bool,
    pub offline_repair_available: bool,
    pub is_windows: bool,
    pub control_state: ControlState,
    pub active_transport: GuestTransport,
    pub agent_version: Option<String>,
    pub attempts: Vec<TransportAttempt>,
}

pub struct PullResult {
    pub value: Value,
    pub transport: GuestTransport,
    pub attempts: Vec<TransportAttempt>,
    pub network_required: bool,
}

pub fn method_min_transport(method: &str) -> GuestTransport {
    match method {
        "guestkit.freezeFilesystem" | "guestkit.thawFilesystem" => GuestTransport::QgaBuiltin,
        "guestkit.ping" => GuestTransport::QgaBuiltin,
        _ if method.starts_with("guestkit.") => GuestTransport::QgaExecRpc,
        _ => GuestTransport::QgaExecRpc,
    }
}

pub fn read_only_method(method: &str) -> bool {
    matches!(
        method,
        "guestkit.getEvidence"
            | "guestkit.getGuestHealth"
            | "guestkit.getGuestInfo"
            | "guestkit.getSystemdUnits"
            | "guestkit.getSystemdEvents"
            | "guestkit.getProcesses"
            | "guestkit.getCapabilities"
            | "guestkit.getStatus"
            | "guestkit.doctor"
            | "guestkit.migrateScore"
            | "guestkit.getBootAnalysis"
            | "guestkit.ping"
    )
}

pub async fn probe_guest_context(state: &AppState, namespace: &str, name: &str) -> GuestContext {
    let mut attempts = Vec::new();
    let client = match state.kube.as_ref() {
        Some(c) => c,
        None => {
            return GuestContext {
                namespace: namespace.into(),
                name: name.into(),
                vmi_running: false,
                qga_connected: false,
                zyvor_agent_installed: false,
                agent_daemon_running: false,
                network_available: false,
                push_registered: false,
                offline_repair_available: false,
                is_windows: false,
                control_state: ControlState::BlindVm,
                active_transport: GuestTransport::ConsoleOnly,
                agent_version: None,
                attempts,
            };
        }
    };

    let vm = fetch_vm(client, namespace, name).await;
    let vmi = fetch_vmi(client, namespace, name).await;
    let vmi_running = vmi.is_some();
    let is_windows = crate::kubevirt_guest_agent::vm_is_windows(vm.as_ref(), vmi.as_ref());

    let qga_connected = if vmi_running {
        let t0 = Instant::now();
        let ok = crate::kubevirt_qga::qga_available(client, namespace, name).await;
        attempts.push(TransportAttempt {
            tier: GuestTransport::QgaBuiltin.as_str().into(),
            ok,
            latency_ms: t0.elapsed().as_millis() as u64,
            error: if ok {
                None
            } else {
                Some("guest-ping failed".into())
            },
        });
        ok
    } else {
        false
    };

    let mut zyvor_agent_installed = vm
        .as_ref()
        .map(|v| zyvor_tools_connected(v, vmi.as_ref()))
        .unwrap_or(false);
    let mut agent_daemon_running = false;
    let mut network_available = false;
    let agent_version = vmi
        .as_ref()
        .and_then(|v| {
            v.pointer("/status/guestAgentVersion")
                .and_then(|x| x.as_str())
                .map(String::from)
        })
        .filter(|v| v.to_lowercase().contains("zyvor") || v.to_lowercase().contains("guestkit"));

    if qga_connected {
        let probe_script = if is_windows {
            r#"if (Test-Path 'C:\Program Files\Zyvor\zyvor-guest-agent.exe') { 'installed' } elseif (Get-Command zyvor-guest-agent -ErrorAction SilentlyContinue) { 'installed' } else { 'missing' }"#
                .to_string()
        } else {
            "command -v zyvor-guest-agent >/dev/null 2>&1 && echo installed || echo missing"
                .to_string()
        };
        let t0 = Instant::now();
        let probe = if is_windows {
            crate::kubevirt_qga::qga_exec_powershell(client, namespace, name, &probe_script, 30)
                .await
        } else {
            crate::kubevirt_qga::qga_exec_shell(client, namespace, name, &probe_script, 30).await
        };
        match probe {
            Ok(r) if r.exit_code == 0 && r.stdout.contains("installed") => {
                zyvor_agent_installed = true;
                attempts.push(TransportAttempt {
                    tier: "agent-binary-probe".into(),
                    ok: true,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: None,
                });
            }
            Ok(r) => {
                attempts.push(TransportAttempt {
                    tier: "agent-binary-probe".into(),
                    ok: false,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: Some(r.stdout.trim().to_string()),
                });
            }
            Err(e) => {
                attempts.push(TransportAttempt {
                    tier: "agent-binary-probe".into(),
                    ok: false,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: Some(e.message.clone()),
                });
            }
        }

        if !is_windows {
            let t0 = Instant::now();
            let daemon = crate::kubevirt_qga::qga_exec_shell(
                client,
                namespace,
                name,
                "systemctl is-active zyvor-guest-agent 2>/dev/null || echo inactive",
                30,
            )
            .await;
            agent_daemon_running = matches!(
                daemon,
                Ok(ref r) if r.exit_code == 0 && r.stdout.trim() == "active"
            );
            attempts.push(TransportAttempt {
                tier: "agent-daemon-probe".into(),
                ok: agent_daemon_running,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: None,
            });

            let t0 = Instant::now();
            let net = crate::kubevirt_qga::qga_exec_shell(
                client,
                namespace,
                name,
                "ip -4 route show default 2>/dev/null | grep -q default && echo up || echo down",
                15,
            )
            .await;
            network_available = matches!(net, Ok(ref r) if r.stdout.contains("up"));
            attempts.push(TransportAttempt {
                tier: "guest-network-probe".into(),
                ok: network_available,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: None,
            });
        } else {
            let t0 = Instant::now();
            let svc = crate::kubevirt_qga::qga_exec_powershell(
                client,
                namespace,
                name,
                "(Get-Service -Name 'QEMU-GA','zyvor-guest-agent' -ErrorAction SilentlyContinue | Where-Object Status -eq 'Running').Count",
                30,
            )
            .await;
            agent_daemon_running = matches!(svc, Ok(ref r) if r.stdout.trim().parse::<i32>().unwrap_or(0) >= 1);
            attempts.push(TransportAttempt {
                tier: "windows-service-probe".into(),
                ok: agent_daemon_running,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: None,
            });

            let t0 = Instant::now();
            let net = crate::kubevirt_qga::qga_exec_powershell(
                client,
                namespace,
                name,
                "(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '127.*' -and $_.PrefixOrigin -ne 'WellKnown' }).Count",
                20,
            )
            .await;
            network_available = matches!(net, Ok(ref r) if r.stdout.trim().parse::<i32>().unwrap_or(0) > 0);
            attempts.push(TransportAttempt {
                tier: "windows-network-probe".into(),
                ok: network_available,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: None,
            });
        }
    }

    let mut redis = state.redis.clone();
    let push_registered = fetch_vm_guest_report(&mut redis, namespace, name)
        .await
        .is_some();
    if push_registered {
        attempts.push(TransportAttempt {
            tier: GuestTransport::HttpsPush.as_str().into(),
            ok: true,
            latency_ms: 0,
            error: None,
        });
    }

    let offline_repair_available = !vmi_running && vm.is_some();

    let active_transport = if zyvor_agent_installed && agent_daemon_running && qga_connected {
        GuestTransport::VirtioSerial
    } else if qga_connected && zyvor_agent_installed {
        GuestTransport::QgaExecRpc
    } else if qga_connected {
        GuestTransport::QgaBuiltin
    } else if push_registered {
        GuestTransport::HttpsPush
    } else if offline_repair_available {
        GuestTransport::OfflineDisk
    } else if vmi_running {
        GuestTransport::ConsoleOnly
    } else {
        GuestTransport::OfflineDisk
    };

    let control_state = infer_control_state(
        vmi_running,
        qga_connected,
        zyvor_agent_installed,
        agent_daemon_running,
        network_available,
        push_registered,
        offline_repair_available,
    );

    GuestContext {
        namespace: namespace.into(),
        name: name.into(),
        vmi_running,
        qga_connected,
        zyvor_agent_installed,
        agent_daemon_running,
        network_available,
        push_registered,
        offline_repair_available,
        is_windows,
        control_state,
        active_transport,
        agent_version,
        attempts,
    }
}

pub async fn pull_method(
    state: &AppState,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
) -> Result<PullResult, String> {
    let mut attempts = Vec::new();
    let min = method_min_transport(method);
    let read_only = read_only_method(method);

    if let Some(client) = state.kube.as_ref() {
        let vm = fetch_vm(client, namespace, name).await;
        let vmi = fetch_vmi(client, namespace, name).await;
        let is_windows = vm_is_windows(vm.as_ref(), vmi.as_ref());

        if rank(min) <= rank(GuestTransport::VirtioSerial) {
            let t0 = Instant::now();
            if vm_agent_daemon_active(client, namespace, name, is_windows).await {
                match vm_in_guest_socket_rpc(
                    client,
                    namespace,
                    name,
                    method,
                    params.clone(),
                    is_windows,
                )
                .await
                {
                    Ok(v) => {
                        attempts.push(TransportAttempt {
                            tier: GuestTransport::VirtioSerial.as_str().into(),
                            ok: true,
                            latency_ms: t0.elapsed().as_millis() as u64,
                            error: None,
                        });
                        return Ok(PullResult {
                            value: v,
                            transport: GuestTransport::VirtioSerial,
                            attempts,
                            network_required: false,
                        });
                    }
                    Err(e) => attempts.push(TransportAttempt {
                        tier: GuestTransport::VirtioSerial.as_str().into(),
                        ok: false,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        error: Some(e.message.clone()),
                    }),
                }
            }
        }

        if rank(min) <= rank(GuestTransport::InGuestSocket) {
            let t0 = Instant::now();
            match vm_in_guest_socket_rpc(
                client,
                namespace,
                name,
                method,
                params.clone(),
                is_windows,
            )
            .await
            {
                Ok(v) => {
                    attempts.push(TransportAttempt {
                        tier: GuestTransport::InGuestSocket.as_str().into(),
                        ok: true,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        error: None,
                    });
                    return Ok(PullResult {
                        value: v,
                        transport: GuestTransport::InGuestSocket,
                        attempts,
                        network_required: false,
                    });
                }
                Err(e) => attempts.push(TransportAttempt {
                    tier: GuestTransport::InGuestSocket.as_str().into(),
                    ok: false,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: Some(e.message.clone()),
                }),
            }
        }

        if rank(min) <= rank(GuestTransport::QgaExecRpc) {
            let t0 = Instant::now();
            match crate::guest_agent_vm::vm_guestkit_rpc(
                client, namespace, name, method, params.clone(),
            )
            .await
            {
                Ok(v) => {
                    attempts.push(TransportAttempt {
                        tier: GuestTransport::QgaExecRpc.as_str().into(),
                        ok: true,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        error: None,
                    });
                    return Ok(PullResult {
                        value: v,
                        transport: GuestTransport::QgaExecRpc,
                        attempts,
                        network_required: false,
                    });
                }
                Err(e) => attempts.push(TransportAttempt {
                    tier: GuestTransport::QgaExecRpc.as_str().into(),
                    ok: false,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: Some(e.message.clone()),
                }),
            }
        }

        if read_only {
            let mut redis = state.redis.clone();
            if let Some(cached) = fetch_vm_guest_report(&mut redis, namespace, name).await {
                let t0 = Instant::now();
                let mapped = map_push_report_to_method(&cached, method);
                if mapped.is_some() {
                    attempts.push(TransportAttempt {
                        tier: GuestTransport::HttpsPush.as_str().into(),
                        ok: true,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        error: None,
                    });
                    return Ok(PullResult {
                        value: serde_json::json!({ "result": mapped }),
                        transport: GuestTransport::HttpsPush,
                        attempts,
                        network_required: true,
                    });
                }
            }
        }

        if min == GuestTransport::QgaBuiltin && method == "guestkit.ping" {
            let t0 = Instant::now();
            if crate::kubevirt_qga::qga_available(client, namespace, name).await {
                attempts.push(TransportAttempt {
                    tier: GuestTransport::QgaBuiltin.as_str().into(),
                    ok: true,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: None,
                });
                return Ok(PullResult {
                    value: serde_json::json!({ "result": { "pong": true } }),
                    transport: GuestTransport::QgaBuiltin,
                    attempts,
                    network_required: false,
                });
            }
        }
    }

    if let Some(proxy) = state.config.agent_proxy_url.as_ref() {
        let t0 = Instant::now();
        match crate::routes::guest_agent::pull_guest_rpc(proxy, method, params).await {
            Ok(v) => {
                attempts.push(TransportAttempt {
                    tier: "http-proxy".into(),
                    ok: true,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: None,
                });
                return Ok(PullResult {
                    value: v,
                    transport: GuestTransport::HttpsPush,
                    attempts,
                    network_required: false,
                });
            }
            Err(e) => attempts.push(TransportAttempt {
                tier: "http-proxy".into(),
                ok: false,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: Some(e),
            }),
        }
    }

    Err(format!(
        "no guest transport for {namespace}/{name} method {method}"
    ))
}

fn map_push_report_to_method(report: &Value, method: &str) -> Option<Value> {
    match method {
        "guestkit.getGuestHealth" => report.get("guest_health").cloned(),
        "guestkit.getEvidence" => report.get("evidence").cloned().or_else(|| Some(report.clone())),
        "guestkit.getSystemdEvents" => report.get("recent_events").cloned(),
        _ => None,
    }
}

pub fn context_to_capabilities(ctx: &GuestContext) -> super::capabilities::GuestCapabilityContract {
    build_capabilities(
        ctx.network_available,
        ctx.qga_connected,
        ctx.zyvor_agent_installed,
        ctx.agent_daemon_running,
        ctx.push_registered,
        ctx.active_transport,
        ctx.control_state,
        ctx.agent_version.clone(),
        ctx.offline_repair_available,
        ctx.is_windows,
    )
}

// Ordering helper for min transport comparison
impl PartialOrd for GuestTransport {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GuestTransport {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        rank(*self).cmp(&rank(*other))
    }
}

fn rank(t: GuestTransport) -> u8 {
    match t {
        GuestTransport::VirtioSerial => 0,
        GuestTransport::QgaExecRpc => 1,
        GuestTransport::QgaBuiltin => 2,
        GuestTransport::InGuestSocket => 3,
        GuestTransport::HttpsPush => 4,
        GuestTransport::OfflineDisk => 5,
        GuestTransport::ConsoleOnly => 6,
    }
}

pub async fn fetch_capabilities(
    state: &AppState,
    namespace: &str,
    name: &str,
) -> Result<Value, String> {
    pull_method(
        state,
        namespace,
        name,
        METHOD_GET_CAPABILITIES,
        Value::Object(Default::default()),
    )
    .await
    .map(|r| r.value)
}
