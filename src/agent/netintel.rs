// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Process-to-socket network intelligence (spec §8/PacketWolf).
//!
//! Builds the in-guest half of the PacketWolf correlation:
//! `PID → process → systemd unit → socket → destination`. The host-side
//! PacketWolf flow data joins on (local port, remote addr:port).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub proto: String,
    pub local_addr: String,
    pub local_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    pub state: String,
    pub pid: u32,
    pub process: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// One aggregated process→destination edge of the egress map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressEdge {
    pub process: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub destination: String,
    pub connections: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkIntelligence {
    pub collected_at: String,
    pub listeners: Vec<Connection>,
    pub connections: Vec<Connection>,
    /// process → destination aggregation for policy recommendation.
    pub egress: Vec<EgressEdge>,
    pub total_established: usize,
    pub total_listening: usize,
    pub unique_remotes: usize,
}

const TCP_STATES: &[(&str, &str)] = &[
    ("01", "established"),
    ("02", "syn_sent"),
    ("03", "syn_recv"),
    ("04", "fin_wait1"),
    ("05", "fin_wait2"),
    ("06", "time_wait"),
    ("07", "close"),
    ("08", "close_wait"),
    ("09", "last_ack"),
    ("0A", "listen"),
    ("0B", "closing"),
];

fn tcp_state(hex: &str) -> String {
    TCP_STATES
        .iter()
        .find(|(h, _)| *h == hex)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| format!("0x{hex}"))
}

/// Parse "0100007F:1F90" (v4) or 32-hex-char v6 into (addr, port).
fn parse_hex_addr(field: &str) -> Option<(String, u16)> {
    let (addr_hex, port_hex) = field.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;
    let addr = if addr_hex.len() == 8 {
        let raw = u32::from_str_radix(addr_hex, 16).ok()?;
        std::net::Ipv4Addr::from(raw.to_le_bytes()).to_string()
    } else if addr_hex.len() == 32 {
        // v6: four little-endian 32-bit groups.
        let mut bytes = [0u8; 16];
        for (i, chunk) in addr_hex.as_bytes().chunks(8).enumerate() {
            let group = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
            bytes[i * 4..i * 4 + 4].copy_from_slice(&group.to_le_bytes());
        }
        std::net::Ipv6Addr::from(bytes).to_string()
    } else {
        return None;
    };
    Some((addr, port))
}

/// One pass over /proc to map socket inodes to PIDs (avoids the
/// O(sockets × processes) rescan of the naive approach).
#[cfg(target_os = "linux")]
fn build_inode_pid_map() -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return map;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(link) = std::fs::read_link(fd.path()) {
                let link = link.to_string_lossy();
                if let Some(inode) = link
                    .strip_prefix("socket:[")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    map.insert(inode.to_string(), pid);
                }
            }
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn proc_name(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| format!("pid-{pid}"))
}

#[cfg(target_os = "linux")]
fn proc_unit(pid: u32) -> Option<String> {
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    cgroup
        .lines()
        .find_map(|l| l.rsplit('/').next())
        .filter(|u| u.ends_with(".service") || u.ends_with(".scope"))
        .map(str::to_string)
}

#[cfg(target_os = "linux")]
fn parse_table(
    path: &str,
    proto: &str,
    inode_map: &HashMap<String, u32>,
    out: &mut Vec<Connection>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let Some((local_addr, local_port)) = parse_hex_addr(parts[1]) else {
            continue;
        };
        let remote = parse_hex_addr(parts[2]);
        let state = if proto.starts_with("udp") {
            "".to_string()
        } else {
            tcp_state(parts[3])
        };
        let inode = parts[9];
        let pid = inode_map.get(inode).copied().unwrap_or(0);
        let (remote_addr, remote_port) = match remote {
            Some((addr, port)) if port != 0 => (Some(addr), Some(port)),
            _ => (None, None),
        };
        out.push(Connection {
            proto: proto.to_string(),
            local_addr,
            local_port,
            remote_addr,
            remote_port,
            state,
            pid,
            process: if pid > 0 {
                proc_name(pid)
            } else {
                String::new()
            },
            unit: if pid > 0 { proc_unit(pid) } else { None },
        });
    }
}

#[cfg(target_os = "linux")]
pub fn collect() -> NetworkIntelligence {
    let inode_map = build_inode_pid_map();
    let mut all = Vec::new();
    for (path, proto) in [
        ("/proc/net/tcp", "tcp"),
        ("/proc/net/tcp6", "tcp6"),
        ("/proc/net/udp", "udp"),
        ("/proc/net/udp6", "udp6"),
    ] {
        parse_table(path, proto, &inode_map, &mut all);
    }
    summarize(all)
}

#[cfg(not(target_os = "linux"))]
pub fn collect() -> NetworkIntelligence {
    // Windows: netstat/IP Helper correlation comes with the Windows
    // observability pass; return an empty (but well-formed) report.
    summarize(Vec::new())
}

fn summarize(all: Vec<Connection>) -> NetworkIntelligence {
    let (listeners, connections): (Vec<Connection>, Vec<Connection>) =
        all.into_iter().partition(|c| {
            c.state == "listen" || (c.proto.starts_with("udp") && c.remote_addr.is_none())
        });

    // Egress aggregation: only outbound established flows to non-loopback.
    let mut edges: BTreeMap<(String, Option<String>, String), usize> = BTreeMap::new();
    let mut remotes: BTreeMap<String, ()> = BTreeMap::new();
    let mut established = 0usize;
    for conn in &connections {
        if conn.state == "established" {
            established += 1;
        }
        let (Some(addr), Some(port)) = (&conn.remote_addr, conn.remote_port) else {
            continue;
        };
        if addr == "127.0.0.1" || addr == "::1" || addr == "0.0.0.0" {
            continue;
        }
        if conn.state != "established" {
            continue;
        }
        let dest = format!("{addr}:{port}");
        remotes.insert(addr.clone(), ());
        *edges
            .entry((conn.process.clone(), conn.unit.clone(), dest))
            .or_insert(0) += 1;
    }

    let egress = edges
        .into_iter()
        .map(|((process, unit, destination), count)| EgressEdge {
            process,
            unit,
            destination,
            connections: count,
        })
        .collect();

    NetworkIntelligence {
        collected_at: chrono::Utc::now().to_rfc3339(),
        total_established: established,
        total_listening: listeners.len(),
        unique_remotes: remotes.len(),
        listeners,
        connections,
        egress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_addr_v4() {
        let (addr, port) = parse_hex_addr("0100007F:1F90").unwrap();
        assert_eq!(addr, "127.0.0.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn hex_addr_v6_loopback() {
        let (addr, port) = parse_hex_addr("00000000000000000000000001000000:0050").unwrap();
        assert_eq!(addr, "::1");
        assert_eq!(port, 80);
    }

    #[test]
    fn summarize_partitions_and_aggregates() {
        let conns = vec![
            Connection {
                proto: "tcp".into(),
                local_addr: "10.0.0.5".into(),
                local_port: 443,
                remote_addr: None,
                remote_port: None,
                state: "listen".into(),
                pid: 100,
                process: "nginx".into(),
                unit: Some("nginx.service".into()),
            },
            Connection {
                proto: "tcp".into(),
                local_addr: "10.0.0.5".into(),
                local_port: 51000,
                remote_addr: Some("10.0.2.20".into()),
                remote_port: Some(5432),
                state: "established".into(),
                pid: 200,
                process: "app".into(),
                unit: Some("app.service".into()),
            },
            Connection {
                proto: "tcp".into(),
                local_addr: "10.0.0.5".into(),
                local_port: 51001,
                remote_addr: Some("10.0.2.20".into()),
                remote_port: Some(5432),
                state: "established".into(),
                pid: 200,
                process: "app".into(),
                unit: Some("app.service".into()),
            },
        ];
        let intel = summarize(conns);
        assert_eq!(intel.total_listening, 1);
        assert_eq!(intel.total_established, 2);
        assert_eq!(intel.unique_remotes, 1);
        assert_eq!(intel.egress.len(), 1);
        assert_eq!(intel.egress[0].connections, 2);
        assert_eq!(intel.egress[0].destination, "10.0.2.20:5432");
    }

    #[test]
    fn linux_collect_smoke() {
        // Well-formed on every platform; on Linux it sees real sockets.
        let intel = collect();
        assert!(!intel.collected_at.is_empty());
    }
}
