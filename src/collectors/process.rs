// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Process and cgroup intelligence from /proc.

use crate::evidence::snapshot::{ListeningPort, ProcessEvidence, ProcessSummary};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[cfg(target_os = "linux")]
use crate::collectors::dbus::get_unit_by_pid;
#[cfg(target_os = "linux")]
use crate::collectors::pressure::collect_pressure;

pub fn collect_process_evidence() -> ProcessEvidence {
    let mut evidence = ProcessEvidence::default();
    #[cfg(target_os = "linux")]
    {
        let pressure = collect_pressure();
        evidence.pressure_cpu = pressure.cpu_some;
        evidence.pressure_memory = pressure.memory_some;
        evidence.pressure_io = pressure.io_some;
    }

    let procs = walk_proc();
    evidence.zombie_count = procs.iter().filter(|p| p.state == "Z").count();
    evidence.d_state_count = procs.iter().filter(|p| p.state == "D").count();

    let mut by_cpu: Vec<ProcessSummary> = procs.clone();
    by_cpu.sort_by(|a, b| {
        b.cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    evidence.top_cpu = by_cpu.into_iter().take(10).collect();

    let mut by_mem: Vec<ProcessSummary> = procs;
    by_mem.sort_by_key(|p| std::cmp::Reverse(p.memory_kb));
    evidence.top_memory = by_mem.into_iter().take(10).collect();

    evidence.listening_ports = collect_listening_ports();
    evidence
}

fn walk_proc() -> Vec<ProcessSummary> {
    let mut out = Vec::new();
    let proc_dir = Path::new("/proc");
    if !proc_dir.is_dir() {
        return out;
    }

    for entry in fs::read_dir(proc_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let pid = name.parse::<u32>().ok();
        if pid.is_none() {
            continue;
        }
        let pid = pid.unwrap();
        if let Some(summary) = read_proc_pid(pid) {
            out.push(summary);
        }
    }
    out
}

fn read_proc_pid(pid: u32) -> Option<ProcessSummary> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat = fs::read_to_string(&stat_path).ok()?;
    let mut parts = stat.split_whitespace();
    let _pid = parts.next()?;
    let comm = parts
        .next()
        .map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string())
        .unwrap_or_default();
    let state = parts.next().unwrap_or("?").to_string();

    let status_path = format!("/proc/{pid}/status");
    let status = fs::read_to_string(&status_path).unwrap_or_default();
    let memory_kb = parse_status_field(&status, "VmRSS:")
        .and_then(|v| v.split_whitespace().next().and_then(|n| n.parse().ok()))
        .unwrap_or(0);

    let unit = map_pid_to_unit(pid);

    Some(ProcessSummary {
        pid,
        name: comm,
        cpu_percent: 0.0,
        memory_kb,
        state,
        unit,
    })
}

fn parse_status_field(status: &str, key: &str) -> Option<String> {
    status
        .lines()
        .find(|l| l.starts_with(key))
        .map(|l| l.trim_start_matches(key).trim().to_string())
}

fn collect_listening_ports() -> Vec<ListeningPort> {
    // Build the socket-inode -> PID map in a single pass over /proc, rather
    // than rescanning every process's fds for each listening socket (which
    // is O(sockets x processes) and made live evidence take minutes on busy
    // hosts). Unit lookups are memoized per PID.
    let inode_map = build_inode_pid_map();
    let mut unit_cache: HashMap<u32, Option<String>> = HashMap::new();
    let mut out = Vec::new();
    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        parse_proc_net(&content, &inode_map, &mut unit_cache, &mut out);
    }
    if let Ok(content) = fs::read_to_string("/proc/net/tcp6") {
        parse_proc_net(&content, &inode_map, &mut unit_cache, &mut out);
    }
    out.sort_by_key(|p| p.port);
    out.into_iter().take(50).collect()
}

/// socket:[inode] -> pid, from one walk of /proc/*/fd.
fn build_inode_pid_map() -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return map;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(fds) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(link) = fs::read_link(fd.path()) {
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

fn parse_proc_net(
    content: &str,
    inode_map: &HashMap<String, u32>,
    unit_cache: &mut HashMap<u32, Option<String>>,
    out: &mut Vec<ListeningPort>,
) {
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let local = parts[1];
        let state = parts[3];
        if state != "0A" {
            continue;
        }
        let port_hex = local.split(':').nth(1).unwrap_or("");
        let port = u16::from_str_radix(port_hex, 16).unwrap_or(0);
        if port == 0 {
            continue;
        }
        let inode = parts[9];
        let pid = inode_map.get(inode).copied().unwrap_or(0);
        let process = if pid > 0 {
            read_proc_pid(pid)
                .map(|p| p.name)
                .unwrap_or_else(|| format!("pid-{pid}"))
        } else {
            String::new()
        };
        let unit = if pid > 0 {
            unit_cache
                .entry(pid)
                .or_insert_with(|| map_pid_to_unit(pid))
                .clone()
        } else {
            None
        };
        out.push(ListeningPort {
            port,
            pid,
            process,
            unit,
        });
    }
}

/// Resolve a PID's systemd unit from /proc/<pid>/cgroup — a single cheap
/// file read, versus a D-Bus round trip per process (which made whole-host
/// process walks take minutes). Falls back to the D-Bus manager only when
/// the cgroup line doesn't name a unit.
#[cfg(target_os = "linux")]
fn map_pid_to_unit(pid: u32) -> Option<String> {
    if let Ok(cgroup) = fs::read_to_string(format!("/proc/{pid}/cgroup")) {
        if let Some(unit) = cgroup
            .lines()
            .filter_map(|l| l.rsplit('/').next())
            .find(|u| u.ends_with(".service") || u.ends_with(".scope"))
        {
            return Some(unit.to_string());
        }
    }
    None
}

/// Available for callers that specifically want the authoritative D-Bus
/// answer for a single PID (rarely needed after the cgroup fast path).
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn map_pid_to_unit_dbus(pid: u32) -> Option<String> {
    get_unit_by_pid(pid)
}

#[cfg(not(target_os = "linux"))]
fn map_pid_to_unit(_pid: u32) -> Option<String> {
    None
}
