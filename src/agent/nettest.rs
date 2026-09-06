// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guest-side network diagnostics (`guestkit.networkTest`).
//!
//! DNS resolution and TCP connect probes with per-target latency, plus
//! default-gateway discovery. ICMP is deliberately absent: it needs
//! CAP_NET_RAW and adds little over a TCP probe.

use serde::Serialize;
use serde_json::Value;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_TARGETS: usize = 16;

#[derive(Debug, Serialize)]
pub struct ProbeResult {
    pub target: String,
    pub success: bool,
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NetworkTestReport {
    pub dns: Vec<ProbeResult>,
    pub tcp: Vec<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayInfo>,
}

#[derive(Debug, Serialize)]
pub struct GatewayInfo {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

pub fn run(params: &Value) -> NetworkTestReport {
    let dns_targets: Vec<String> = params
        .get("dns")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let tcp_targets: Vec<String> = params
        .get("tcp")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // Gateway check is the default probe when nothing else is requested.
    let want_gateway = params
        .get("gateway")
        .and_then(Value::as_bool)
        .unwrap_or(dns_targets.is_empty() && tcp_targets.is_empty());

    NetworkTestReport {
        dns: dns_targets
            .iter()
            .take(MAX_TARGETS)
            .map(|name| probe_dns(name))
            .collect(),
        tcp: tcp_targets
            .iter()
            .take(MAX_TARGETS)
            .map(|addr| probe_tcp(addr))
            .collect(),
        gateway: want_gateway.then(gateway_info),
    }
}

fn probe_dns(name: &str) -> ProbeResult {
    let start = Instant::now();
    // Port is irrelevant; ToSocketAddrs needs one to drive the resolver.
    match format!("{name}:443").to_socket_addrs() {
        Ok(mut addrs) => {
            let first = addrs.next().map(|a| a.ip().to_string());
            ProbeResult {
                target: name.to_string(),
                success: first.is_some(),
                latency_ms: Some(start.elapsed().as_millis() as u64),
                detail: first,
            }
        }
        Err(e) => ProbeResult {
            target: name.to_string(),
            success: false,
            latency_ms: None,
            detail: Some(e.to_string()),
        },
    }
}

fn probe_tcp(target: &str) -> ProbeResult {
    let addr = match target.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => {
            return ProbeResult {
                target: target.to_string(),
                success: false,
                latency_ms: None,
                detail: Some("unresolvable".to_string()),
            }
        }
    };
    let start = Instant::now();
    match TcpStream::connect_timeout(&addr, TCP_TIMEOUT) {
        Ok(_) => ProbeResult {
            target: target.to_string(),
            success: true,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            detail: None,
        },
        Err(e) => ProbeResult {
            target: target.to_string(),
            success: false,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            detail: Some(e.to_string()),
        },
    }
}

#[cfg(target_os = "linux")]
fn gateway_info() -> GatewayInfo {
    // /proc/net/route: default route has destination 00000000; gateway is
    // little-endian hex.
    let Ok(text) = std::fs::read_to_string("/proc/net/route") else {
        return GatewayInfo {
            found: false,
            address: None,
        };
    };
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 3 && f[1] == "00000000" {
            if let Ok(gw) = u32::from_str_radix(f[2], 16) {
                let octets = gw.to_le_bytes();
                return GatewayInfo {
                    found: true,
                    address: Some(std::net::Ipv4Addr::from(octets).to_string()),
                };
            }
        }
    }
    GatewayInfo {
        found: false,
        address: None,
    }
}

#[cfg(not(target_os = "linux"))]
fn gateway_info() -> GatewayInfo {
    GatewayInfo {
        found: false,
        address: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_params_defaults_to_gateway_probe() {
        let report = run(&json!({}));
        assert!(report.dns.is_empty());
        assert!(report.tcp.is_empty());
        assert!(report.gateway.is_some());
    }

    #[test]
    fn localhost_dns_resolves() {
        let report = run(&json!({"dns": ["localhost"]}));
        assert_eq!(report.dns.len(), 1);
        assert!(report.dns[0].success);
        assert!(report.gateway.is_none());
    }

    #[test]
    fn unresolvable_tcp_target_fails_cleanly() {
        let report = run(&json!({"tcp": ["nonexistent.invalid:1"]}));
        assert!(!report.tcp[0].success);
    }

    #[test]
    fn target_list_is_capped() {
        let many: Vec<String> = (0..40).map(|i| format!("host{i}.invalid")).collect();
        let report = run(&json!({ "dns": many }));
        assert_eq!(report.dns.len(), MAX_TARGETS);
    }
}
