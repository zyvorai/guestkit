// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Forward guest health snapshots to PacketWolf for VM ↔ flow correlation.

use kube::api::{Api, Patch, PatchParams};
use kube::Client;
use serde_json::{json, Value};
use tracing::warn;

use crate::config::Config;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CorrelationMember {
    pub namespace: String,
    pub vm_name: String,
    pub hostname: String,
    pub guest_health: String,
    pub score: u64,
    pub failed_units: u64,
    pub systemd_state: String,
    pub default_gateway: Option<String>,
    pub interface_count: usize,
    pub ip_addresses: Vec<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub journal_hints: usize,
    pub recent_event_count: usize,
    pub agent_version: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CorrelationEvent {
    source: &'static str,
    namespace: String,
    vm_name: String,
    hostname: String,
    guest_health: String,
    score: u64,
    failed_units: u64,
    systemd_state: String,
    default_gateway: Option<String>,
    interface_count: usize,
    ip_addresses: Vec<String>,
    rx_bytes: u64,
    tx_bytes: u64,
    journal_hints: usize,
    recent_event_count: usize,
    agent_version: String,
    observed_at: String,
}

pub fn emit_guest_report_correlation(
    config: &Config,
    client: Option<&Client>,
    namespace: &str,
    vm_name: &str,
    guest_health: &Value,
    metrics: &Value,
    recent_events: &Value,
    agent_version: &str,
) {
    let url = config
        .packetwolf_correlation_url
        .as_ref()
        .filter(|u| !u.trim().is_empty());
    if url.is_none() && client.is_none() {
        return;
    }

    let member = build_member(namespace, vm_name, guest_health, metrics, recent_events, agent_version);
    let event = CorrelationEvent {
        source: "zyvor-guest-agent",
        namespace: member.namespace.clone(),
        vm_name: member.vm_name.clone(),
        hostname: member.hostname.clone(),
        guest_health: member.guest_health.clone(),
        score: member.score,
        failed_units: member.failed_units,
        systemd_state: member.systemd_state.clone(),
        default_gateway: member.default_gateway.clone(),
        interface_count: member.interface_count,
        ip_addresses: member.ip_addresses.clone(),
        rx_bytes: member.rx_bytes,
        tx_bytes: member.tx_bytes,
        journal_hints: member.journal_hints,
        recent_event_count: member.recent_event_count,
        agent_version: member.agent_version.clone(),
        observed_at: member.observed_at.clone(),
    };

    if let Some(url) = url {
        let url = url.clone();
        let body = event.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build();
            if let Ok(client) = client {
                if let Err(e) = client.post(&url).json(&body).send().await {
                    warn!("PacketWolf correlation POST failed: {e}");
                }
            }
        });
    }

    if let Some(client) = client {
        let ns = namespace.to_string();
        let vm = vm_name.to_string();
        let guest_level = event.guest_health.clone();
        let score = event.score as u8;
        let client = client.clone();
        tokio::spawn(async move {
            patch_packetwolf_annotations(client, &ns, &vm, &guest_level, score).await;
        });
    }
}

pub fn build_member(
    namespace: &str,
    vm_name: &str,
    guest_health: &Value,
    metrics: &Value,
    recent_events: &Value,
    agent_version: &str,
) -> CorrelationMember {
    let hostname = guest_health
        .get("vm_hostname")
        .and_then(|v| v.as_str())
        .unwrap_or(vm_name)
        .to_string();
    let default_gateway = guest_health
        .pointer("/network/default_gateway")
        .or_else(|| metrics.pointer("/network/default_gateway"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let interface_count = guest_health
        .pointer("/network/interfaces_up")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let ip_addresses = extract_ip_addresses(guest_health, metrics);
    let (rx_bytes, tx_bytes) = extract_flow_bytes(guest_health, metrics);
    let journal_hints = guest_health
        .get("journal_hints")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let recent_event_count = recent_events
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    CorrelationMember {
        namespace: namespace.to_string(),
        vm_name: vm_name.to_string(),
        hostname,
        guest_health: guest_health
            .get("guest_health")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        score: guest_health.get("score").and_then(|v| v.as_u64()).unwrap_or(0),
        failed_units: guest_health
            .get("failed_units")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        systemd_state: guest_health
            .get("systemd_state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        default_gateway,
        interface_count,
        ip_addresses,
        rx_bytes,
        tx_bytes,
        journal_hints,
        recent_event_count,
        agent_version: agent_version.to_string(),
        observed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn extract_ip_addresses(_guest_health: &Value, metrics: &Value) -> Vec<String> {
    metrics
        .pointer("/ips")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_flow_bytes(guest_health: &Value, metrics: &Value) -> (u64, u64) {
    let rx = metrics
        .pointer("/network/rx_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let tx = metrics
        .pointer("/network/tx_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if rx > 0 || tx > 0 {
        return (rx, tx);
    }
    let _ = guest_health;
    (0, 0)
}

async fn patch_packetwolf_annotations(
    client: Client,
    namespace: &str,
    vm_name: &str,
    guest_level: &str,
    score: u8,
) {
    let ar = crate::routes::kubevirt::vm_resource();
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &ar);
    let correlation_id = format!("{namespace}/{vm_name}:{guest_level}:{score}");
    let patch = json!({
        "metadata": {
            "annotations": {
                "zeus.zyvor.dev/packetwolf-correlation": correlation_id,
                "zeus.zyvor.dev/packetwolf-correlation-at": chrono::Utc::now().to_rfc3339(),
            }
        }
    });
    if let Err(e) = api
        .patch(vm_name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
    {
        warn!("PacketWolf VM annotation patch failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_member_extracts_ips_and_health() {
        let guest_health = json!({
            "vm_hostname": "vm-a",
            "guest_health": "degraded",
            "score": 72,
            "failed_units": 1,
            "systemd_state": "running",
            "journal_hints": ["hint1"],
            "agent_version": "0.3.9",
        });
        let metrics = json!({
            "ips": ["10.0.0.5"],
            "network": { "rx_bytes": 1000, "tx_bytes": 2000 },
        });
        let member = build_member("default", "vm-a", &guest_health, &metrics, &json!([]), "0.3.9");
        assert_eq!(member.namespace, "default");
        assert_eq!(member.vm_name, "vm-a");
        assert_eq!(member.guest_health, "degraded");
        assert_eq!(member.ip_addresses, vec!["10.0.0.5".to_string()]);
        assert_eq!(member.journal_hints, 1);
    }
}
