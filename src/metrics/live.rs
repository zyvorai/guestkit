// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Point-in-time CPU, memory, disk, and network metrics from inside the guest.

use serde::{Deserialize, Serialize};
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestMetricsSnapshot {
    pub collected_at: String,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskIoMetrics,
    pub network: NetworkIoMetrics,
    pub load_average: LoadAverage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_systemd_units: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub cores: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
    pub swap_used_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskIoMetrics {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub usage_percent: f64,
    pub root_used_bytes: u64,
    pub root_total_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkIoMetrics {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

pub fn collect_metrics_live() -> GuestMetricsSnapshot {
    let mut sys = System::new_all();
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage() as f64;
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let avail_mem = sys.available_memory();
    let mem_pct = if total_mem > 0 {
        (used_mem as f64 / total_mem as f64) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let mut read_bytes = 0u64;
    let mut write_bytes = 0u64;
    let mut root_used = 0u64;
    let mut root_total = 0u64;
    for disk in disks.list() {
        read_bytes = read_bytes.saturating_add(disk.usage().total_read_bytes);
        write_bytes = write_bytes.saturating_add(disk.usage().total_written_bytes);
        let mp = disk.mount_point().to_string_lossy();
        if mp == "/" {
            root_total = disk.total_space();
            root_used = disk.total_space().saturating_sub(disk.available_space());
        }
    }
    let disk_pct = if root_total > 0 {
        (root_used as f64 / root_total as f64) * 100.0
    } else {
        0.0
    };

    let networks = Networks::new_with_refreshed_list();
    let mut rx = 0u64;
    let mut tx = 0u64;
    for data in networks.values() {
        rx = rx.saturating_add(data.total_received());
        tx = tx.saturating_add(data.total_transmitted());
    }

    let load = sysinfo::System::load_average();

    GuestMetricsSnapshot {
        collected_at: chrono::Utc::now().to_rfc3339(),
        cpu: CpuMetrics {
            usage_percent: cpu_usage,
            cores: sys.cpus().len(),
        },
        memory: MemoryMetrics {
            total_bytes: total_mem,
            used_bytes: used_mem,
            available_bytes: avail_mem,
            usage_percent: mem_pct,
            swap_used_bytes: sys.used_swap(),
        },
        disk: DiskIoMetrics {
            read_bytes,
            write_bytes,
            usage_percent: disk_pct,
            root_used_bytes: root_used,
            root_total_bytes: root_total,
        },
        network: NetworkIoMetrics {
            rx_bytes: rx,
            tx_bytes: tx,
        },
        load_average: LoadAverage {
            one: load.one,
            five: load.five,
            fifteen: load.fifteen,
        },
        failed_systemd_units: failed_systemd_units(),
    }
}

fn failed_systemd_units() -> Vec<String> {
    use std::process::Command;
    Command::new("systemctl")
        .args(["--failed", "--no-legend", "--plain"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().next().map(str::to_string))
                .take(10)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_metrics_on_host() {
        let m = collect_metrics_live();
        assert!(!m.collected_at.is_empty());
        assert!(m.cpu.cores >= 1);
    }
}
