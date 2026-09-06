// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! 1-second telemetry sampling task.

use super::{PerfSample, TelemetryStore};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Disks, Networks, System};

struct SamplerState {
    sys: System,
    networks: Networks,
    prev_disk_read: u64,
    prev_disk_write: u64,
}

const PERSIST_EVERY_TICKS: u64 = 300; // 5 minutes

pub async fn run_sampler(store: Arc<TelemetryStore>) {
    store.load_coarse();
    let mut tick: u64 = 0;
    let mut state = SamplerState {
        sys: System::new(),
        networks: Networks::new_with_refreshed_list(),
        prev_disk_read: 0,
        prev_disk_write: 0,
    };
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let sample = tokio::task::block_in_place(|| take_sample(&mut state));
        store.record(sample);
        tick += 1;
        if tick.is_multiple_of(PERSIST_EVERY_TICKS) {
            if let Err(e) = store.save_coarse() {
                log::debug!("telemetry persist: {e}");
            }
        }
    }
}

fn take_sample(state: &mut SamplerState) -> PerfSample {
    state.sys.refresh_cpu_usage();
    state.sys.refresh_memory();
    state.networks.refresh(true);

    let total_mem = state.sys.total_memory();
    let used_mem = state.sys.used_memory();
    let mem_pct = if total_mem > 0 {
        (used_mem as f64 / total_mem as f64 * 100.0) as f32
    } else {
        0.0
    };

    let root_mount = if cfg!(windows) { "C:\\" } else { "/" };
    let disks = Disks::new_with_refreshed_list();
    let disk_used_pct = disks
        .iter()
        .find(|d| d.mount_point().to_string_lossy() == root_mount)
        .map(|d| {
            if d.total_space() > 0 {
                ((d.total_space() - d.available_space()) as f64 / d.total_space() as f64 * 100.0)
                    as f32
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    let (net_rx_b, net_tx_b) = state
        .networks
        .iter()
        .fold((0u64, 0u64), |(rx, tx), (_, data)| {
            (rx + data.received(), tx + data.transmitted())
        });

    let (disk_read_total, disk_write_total) = disk_io_totals();
    let disk_read_b = disk_read_total.saturating_sub(state.prev_disk_read);
    let disk_write_b = disk_write_total.saturating_sub(state.prev_disk_write);
    // First sample after start has no baseline — report 0, not since-boot.
    let (disk_read_b, disk_write_b) = if state.prev_disk_read == 0 && state.prev_disk_write == 0 {
        (0, 0)
    } else {
        (disk_read_b, disk_write_b)
    };
    state.prev_disk_read = disk_read_total;
    state.prev_disk_write = disk_write_total;

    let psi = crate::agent::heartbeat::psi_for_telemetry();
    let cpu_pct = state.sys.global_cpu_usage();

    PerfSample {
        ts: chrono::Utc::now().timestamp() as u64,
        cpu_pct,
        cpu_pct_max: cpu_pct,
        load1: System::load_average().one as f32,
        mem_used: used_mem,
        mem_avail: state.sys.available_memory(),
        swap_used: state.sys.used_swap(),
        mem_pct,
        disk_used_pct,
        disk_read_b,
        disk_write_b,
        net_rx_b,
        net_tx_b,
        psi_cpu: psi.map(|p| p.cpu).unwrap_or(0.0),
        psi_mem: psi.map(|p| p.memory).unwrap_or(0.0),
        psi_io: psi.map(|p| p.io).unwrap_or(0.0),
        procs: proc_count(),
    }
}

/// Cumulative bytes (read, written) across physical block devices since
/// boot. Linux via /proc/diskstats; 0 elsewhere (PDH counters deferred).
#[cfg(target_os = "linux")]
fn disk_io_totals() -> (u64, u64) {
    const SECTOR: u64 = 512;
    let Ok(text) = std::fs::read_to_string("/proc/diskstats") else {
        return (0, 0);
    };
    let mut read = 0u64;
    let mut written = 0u64;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        // name sectors_read=f[5] sectors_written=f[9]; skip partitions and
        // virtual devices (loop, ram, dm counts double with underlying dev).
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        let is_partition = name
            .chars()
            .last()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("xvd"));
        if is_partition || name.starts_with("loop") || name.starts_with("ram") {
            continue;
        }
        read += f[5].parse::<u64>().unwrap_or(0) * SECTOR;
        written += f[9].parse::<u64>().unwrap_or(0) * SECTOR;
    }
    (read, written)
}

#[cfg(not(target_os = "linux"))]
fn disk_io_totals() -> (u64, u64) {
    (0, 0)
}

#[cfg(target_os = "linux")]
fn proc_count() -> u32 {
    std::fs::read_dir("/proc")
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .chars()
                        .all(|c| c.is_ascii_digit())
                })
                .count() as u32
        })
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn proc_count() -> u32 {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().len() as u32
}
