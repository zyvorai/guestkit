// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! DNS and reachability probes for KubeVirt / VMRogue bootstrap validation.

use crate::evidence::snapshot::NetworkProbeEvidence;
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

pub fn collect_network_probes_live() -> NetworkProbeEvidence {
    let mut probes = NetworkProbeEvidence::default();

    if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in content.lines() {
            if let Some(ip) = line.strip_prefix("nameserver ") {
                probes.dns_servers.push(ip.trim().to_string());
            }
        }
    }

    let cluster_dns = std::env::var("VMROGUE_CLUSTER_DNS")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| probes.dns_servers.first().cloned())
        .unwrap_or_else(|| "10.43.0.10".into());

    probes.cluster_dns_reachable = tcp_probe(&cluster_dns, 53, 1500);

    let api_hosts = [
        std::env::var("VMROGUE_API_CLUSTER_IP").ok(),
        Some("vmrogue-api.vmrogue-system.svc.cluster.local".into()),
    ];
    for host in api_hosts.into_iter().flatten() {
        if host.parse::<std::net::IpAddr>().is_ok() {
            if tcp_probe(&host, 443, 1500) {
                probes.api_service_reachable = true;
                probes
                    .probe_details
                    .push(format!("TCP 443 reachable on {host}"));
                break;
            }
        } else if Command::new("getent")
            .args(["hosts", &host])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            probes.probe_details.push(format!("DNS resolves {host}"));
            probes.api_service_reachable = true;
            break;
        }
    }

    probes.internet_reachable = tcp_probe("1.1.1.1", 443, 1500) || tcp_probe("8.8.8.8", 53, 1500);

    probes.probe_details.push(format!(
        "cluster_dns={cluster_dns} reachable={}",
        probes.cluster_dns_reachable
    ));

    probes
}

fn tcp_probe(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{host}:{port}");
    addr.to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .and_then(|socket| {
            TcpStream::connect_timeout(&socket, Duration::from_millis(timeout_ms)).ok()
        })
        .is_some()
}
