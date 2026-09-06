// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live network interface and route collection (procfs + ip -j).

use crate::evidence::snapshot::{NetworkEvidence, NetworkInterfaceLive};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn enrich_network_evidence(network: &mut NetworkEvidence) {
    network.network_stack = detect_network_stack();
    apply_proc_net_dev_stats(network);
    collect_ip_json(network);
    if network.default_gateway.is_none() {
        network.default_gateway = read_default_gateway_proc();
    }
}

fn detect_network_stack() -> String {
    if fs::metadata("/run/NetworkManager").is_ok()
        || Command::new("which")
            .arg("nmcli")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    {
        return "NetworkManager".into();
    }
    if Path::new("/etc/netplan").is_dir() {
        return "netplan".into();
    }
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "systemd-networkd"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return "systemd-networkd".into();
    }
    "unknown".into()
}

fn parse_proc_net_dev(content: &str) -> HashMap<String, (u64, u64)> {
    let mut out = HashMap::new();
    for line in content.lines().skip(2) {
        let mut parts = line.split_whitespace();
        let iface = parts.next().unwrap_or("").trim_end_matches(':');
        if iface.is_empty() || iface == "lo" {
            continue;
        }
        let rx = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        for _ in 0..7 {
            parts.next();
        }
        let tx = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        out.insert(iface.to_string(), (rx, tx));
    }
    out
}

fn read_proc_net_dev_stats() -> HashMap<String, (u64, u64)> {
    fs::read_to_string("/proc/net/dev")
        .map(|content| parse_proc_net_dev(&content))
        .unwrap_or_default()
}

fn apply_proc_net_dev_stats(network: &mut NetworkEvidence) {
    let stats = read_proc_net_dev_stats();
    for (name, (rx, tx)) in stats {
        if let Some(live) = network.live_interfaces.iter_mut().find(|i| i.name == name) {
            live.rx_bytes = Some(rx);
            live.tx_bytes = Some(tx);
            continue;
        }
        if !network.interfaces.contains(&name) {
            network.interfaces.push(name.clone());
        }
        network.live_interfaces.push(NetworkInterfaceLive {
            name,
            state: String::new(),
            mac: String::new(),
            addresses: Vec::new(),
            carrier: true,
            rx_bytes: Some(rx),
            tx_bytes: Some(tx),
        });
    }
}

fn collect_ip_json(network: &mut NetworkEvidence) {
    if let Ok(out) = Command::new("ip").args(["-j", "addr"]).output() {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for iface in json {
                let name = iface["ifname"].as_str().unwrap_or("").to_string();
                if name.is_empty() || name == "lo" {
                    continue;
                }
                let state = iface["operstate"].as_str().unwrap_or("").to_string();
                let mac = iface
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut addresses = Vec::new();
                if let Some(addrinfo) = iface.get("addr_info").and_then(|v| v.as_array()) {
                    for addr in addrinfo {
                        if let Some(local) = addr.get("local").and_then(|v| v.as_str()) {
                            let family = addr.get("family").and_then(|v| v.as_str()).unwrap_or("");
                            let prefix = addr.get("prefixlen").and_then(|v| v.as_u64());
                            if family == "inet" || family == "inet6" {
                                let suffix = prefix.map(|p| format!("/{p}")).unwrap_or_default();
                                addresses.push(format!("{local}{suffix}"));
                            }
                        }
                    }
                }
                if !network.interfaces.contains(&name) {
                    network.interfaces.push(name.clone());
                }
                if let Some(live) = network.live_interfaces.iter_mut().find(|i| i.name == name) {
                    live.state = state;
                    live.mac = mac;
                    live.addresses = addresses;
                    live.carrier = iface
                        .get("carrier")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                } else {
                    network.live_interfaces.push(NetworkInterfaceLive {
                        name,
                        state,
                        mac,
                        addresses,
                        carrier: iface
                            .get("carrier")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        rx_bytes: None,
                        tx_bytes: None,
                    });
                }
            }
        }
    }

    if let Ok(out) = Command::new("ip").args(["-j", "route"]).output() {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for route in json {
                if route.get("dst").and_then(|v| v.as_str()) == Some("default") {
                    network.default_gateway = route
                        .get("gateway")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    break;
                }
            }
        }
    }

    if let Ok(out) = Command::new("ip").args(["-j", "link"]).output() {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            for link in json {
                let name = link.get("ifname").and_then(|v| v.as_str()).unwrap_or("");
                if name.is_empty() || name == "lo" {
                    continue;
                }
                let stats = link.get("stats64").or_else(|| link.get("stats"));
                if let Some(stats) = stats {
                    let rx = stats.get("rx_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                    let tx = stats.get("tx_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                    if let Some(live) = network.live_interfaces.iter_mut().find(|i| i.name == name)
                    {
                        if live.rx_bytes.is_none() {
                            live.rx_bytes = Some(rx);
                        }
                        if live.tx_bytes.is_none() {
                            live.tx_bytes = Some(tx);
                        }
                    }
                }
            }
        }
    }
}

fn read_default_gateway_proc() -> Option<String> {
    let content = fs::read_to_string("/proc/net/route").ok()?;
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[1] == "00000000" {
            let hex = parts[2];
            if hex.len() == 8 {
                let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let c = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let d = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                return Some(format!("{a}.{b}.{c}.{d}"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proc_net_dev_extracts_rx_tx() {
        let sample = "Inter-|   Receive                                                |  Transmit\n\
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n\
  eth0: 1234567890    1000    0    0    0     0          0         0 9876543210    2000    0    0    0     0       0          0\n\
    lo:       42       1    0    0    0     0          0         0       42       1    0    0    0     0       0          0\n";
        let stats = parse_proc_net_dev(sample);
        assert_eq!(stats.get("eth0"), Some(&(1234567890, 9876543210)));
        assert!(!stats.contains_key("lo"));
    }
}
