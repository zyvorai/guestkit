// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rolling performance telemetry.
//!
//! Three tiers of fixed-size history are kept in memory (~1.3 MB total):
//! 1-second samples for 15 minutes, 10-second for 6 hours, 1-minute for
//! 7 days — so diagnostics survive control-plane connectivity gaps.
//! History resets on agent restart (persistence deliberately deferred).

pub mod ring;
pub mod sampler;

use guestkit_agent_protocol::telemetry::{MetricStats, PerfSeries, PerfSummary, PerfTier};
use ring::Ring;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub const FINE_CAP: usize = 900; // 15 min @ 1 s
pub const MEDIUM_CAP: usize = 2160; // 6 h  @ 10 s
pub const COARSE_CAP: usize = 10080; // 7 d  @ 1 min

/// One telemetry observation. Rate-like fields (`disk_*`, `net_*`) are
/// deltas over the sample's window; the rest are gauges.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct PerfSample {
    pub ts: u64,
    pub cpu_pct: f32,
    /// Equal to `cpu_pct` in fine samples; max of folded members in
    /// medium/coarse so aggregation doesn't hide spikes.
    pub cpu_pct_max: f32,
    pub load1: f32,
    pub mem_used: u64,
    pub mem_avail: u64,
    pub swap_used: u64,
    pub mem_pct: f32,
    pub disk_used_pct: f32,
    pub disk_read_b: u64,
    pub disk_write_b: u64,
    pub net_rx_b: u64,
    pub net_tx_b: u64,
    pub psi_cpu: f32,
    pub psi_mem: f32,
    pub psi_io: f32,
    pub procs: u32,
}

const METRIC_NAMES: &[&str] = &[
    "cpu_pct",
    "cpu_pct_max",
    "load1",
    "mem_used",
    "mem_avail",
    "swap_used",
    "mem_pct",
    "disk_used_pct",
    "disk_read_b",
    "disk_write_b",
    "net_rx_b",
    "net_tx_b",
    "psi_cpu",
    "psi_mem",
    "psi_io",
    "procs",
];

fn metric_value(s: &PerfSample, name: &str) -> f64 {
    match name {
        "cpu_pct" => s.cpu_pct as f64,
        "cpu_pct_max" => s.cpu_pct_max as f64,
        "load1" => s.load1 as f64,
        "mem_used" => s.mem_used as f64,
        "mem_avail" => s.mem_avail as f64,
        "swap_used" => s.swap_used as f64,
        "mem_pct" => s.mem_pct as f64,
        "disk_used_pct" => s.disk_used_pct as f64,
        "disk_read_b" => s.disk_read_b as f64,
        "disk_write_b" => s.disk_write_b as f64,
        "net_rx_b" => s.net_rx_b as f64,
        "net_tx_b" => s.net_tx_b as f64,
        "psi_cpu" => s.psi_cpu as f64,
        "psi_mem" => s.psi_mem as f64,
        "psi_io" => s.psi_io as f64,
        "procs" => s.procs as f64,
        _ => f64::NAN,
    }
}

/// Fold a window of fine samples into one coarser sample: mean for gauges,
/// sum for deltas, max preserved for CPU.
pub fn fold(samples: &[PerfSample]) -> Option<PerfSample> {
    let n = samples.len();
    if n == 0 {
        return None;
    }
    let nf = n as f32;
    let mut out = PerfSample {
        ts: samples.last()?.ts,
        ..Default::default()
    };
    for s in samples {
        out.cpu_pct += s.cpu_pct / nf;
        out.cpu_pct_max = out.cpu_pct_max.max(s.cpu_pct_max);
        out.load1 += s.load1 / nf;
        out.mem_used += s.mem_used / n as u64;
        out.mem_avail += s.mem_avail / n as u64;
        out.swap_used += s.swap_used / n as u64;
        out.mem_pct += s.mem_pct / nf;
        out.disk_used_pct += s.disk_used_pct / nf;
        out.disk_read_b += s.disk_read_b;
        out.disk_write_b += s.disk_write_b;
        out.net_rx_b += s.net_rx_b;
        out.net_tx_b += s.net_tx_b;
        out.psi_cpu += s.psi_cpu / nf;
        out.psi_mem += s.psi_mem / nf;
        out.psi_io += s.psi_io / nf;
        out.procs = s.procs;
    }
    Some(out)
}

pub struct TelemetryStore {
    fine: Mutex<Ring<PerfSample>>,
    medium: Mutex<Ring<PerfSample>>,
    coarse: Mutex<Ring<PerfSample>>,
}

impl Default for TelemetryStore {
    fn default() -> Self {
        Self {
            fine: Mutex::new(Ring::new(FINE_CAP)),
            medium: Mutex::new(Ring::new(MEDIUM_CAP)),
            coarse: Mutex::new(Ring::new(COARSE_CAP)),
        }
    }
}

/// On-disk persistence path for the coarse (7-day) ring.
pub fn persist_path() -> std::path::PathBuf {
    let base = std::env::var("GUESTKIT_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                std::path::PathBuf::from("C:\\ProgramData\\GuestKit")
            } else {
                std::path::PathBuf::from("/var/lib/guestkit")
            }
        });
    base.join("telemetry-coarse.bin")
}

impl TelemetryStore {
    /// Persist the coarse ring so the 7-day history survives restarts.
    pub fn save_coarse(&self) -> anyhow::Result<()> {
        let samples: Vec<PerfSample> = self
            .coarse
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .copied()
            .collect();
        let path = persist_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = bincode::serialize(&samples)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Restore a previously persisted coarse ring (stale samples beyond
    /// 7 days are naturally displaced as new ones arrive).
    pub fn load_coarse(&self) {
        let Ok(bytes) = std::fs::read(persist_path()) else {
            return;
        };
        let Ok(samples) = bincode::deserialize::<Vec<PerfSample>>(&bytes) else {
            log::warn!("telemetry persistence file unreadable — starting fresh");
            return;
        };
        let mut coarse = self.coarse.lock().unwrap_or_else(|e| e.into_inner());
        for sample in samples.into_iter().take(COARSE_CAP) {
            coarse.push(sample);
        }
    }

    fn tier_ring(&self, tier: PerfTier) -> &Mutex<Ring<PerfSample>> {
        match tier {
            PerfTier::Fine => &self.fine,
            PerfTier::Medium => &self.medium,
            PerfTier::Coarse => &self.coarse,
        }
    }

    /// Record a fine sample and cascade folds on tier boundaries.
    pub fn record(&self, sample: PerfSample) {
        let fine_len = {
            let mut fine = self.fine.lock().unwrap_or_else(|e| e.into_inner());
            fine.push(sample);
            fine.len()
        };
        if fine_len % 10 == 0 {
            let members = self
                .fine
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .last_n(10);
            if let Some(folded) = fold(&members) {
                self.medium
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(folded);
            }
        }
        if fine_len % 60 == 0 {
            let members = self
                .fine
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .last_n(60);
            if let Some(folded) = fold(&members) {
                self.coarse
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(folded);
            }
        }
    }

    pub fn latest(&self) -> Option<PerfSample> {
        self.fine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .latest()
            .copied()
    }

    /// Columnar series for a tier, optionally bounded by [from_ts, to_ts]
    /// and restricted to named metrics.
    pub fn series(
        &self,
        tier: PerfTier,
        from_ts: Option<u64>,
        to_ts: Option<u64>,
        metrics: Option<&[String]>,
    ) -> PerfSeries {
        let samples: Vec<PerfSample> = {
            let ring = self
                .tier_ring(tier)
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            ring.iter()
                .filter(|s| from_ts.map(|f| s.ts >= f).unwrap_or(true))
                .filter(|s| to_ts.map(|t| s.ts <= t).unwrap_or(true))
                .copied()
                .collect()
        };
        let names: Vec<&str> = match metrics {
            Some(requested) => METRIC_NAMES
                .iter()
                .copied()
                .filter(|n| requested.iter().any(|r| r == n))
                .collect(),
            None => METRIC_NAMES.to_vec(),
        };
        let mut columns: BTreeMap<String, Vec<Option<f64>>> = BTreeMap::new();
        for name in &names {
            columns.insert(
                name.to_string(),
                samples
                    .iter()
                    .map(|s| {
                        let v = metric_value(s, name);
                        if v.is_nan() {
                            None
                        } else {
                            Some(v)
                        }
                    })
                    .collect(),
            );
        }
        PerfSeries {
            tier,
            start_ts: samples.first().map(|s| s.ts).unwrap_or(0),
            step_secs: tier.step_secs(),
            metrics: columns,
        }
    }

    /// min/avg/max/p95 per metric over the trailing `window_secs`.
    pub fn summary(&self, tier: PerfTier, window_secs: u64) -> PerfSummary {
        let samples: Vec<PerfSample> = {
            let ring = self
                .tier_ring(tier)
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let newest = ring.latest().map(|s| s.ts).unwrap_or(0);
            let cutoff = newest.saturating_sub(window_secs);
            ring.iter().filter(|s| s.ts >= cutoff).copied().collect()
        };
        let mut metrics = BTreeMap::new();
        for name in METRIC_NAMES {
            let mut values: Vec<f64> = samples
                .iter()
                .map(|s| metric_value(s, name))
                .filter(|v| !v.is_nan())
                .collect();
            if values.is_empty() {
                continue;
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p95_idx = ((values.len() as f64 * 0.95).ceil() as usize).clamp(1, values.len()) - 1;
            metrics.insert(
                name.to_string(),
                MetricStats {
                    min: values[0],
                    avg: values.iter().sum::<f64>() / values.len() as f64,
                    max: *values.last().unwrap(),
                    p95: values[p95_idx],
                },
            );
        }
        PerfSummary {
            tier,
            window_secs,
            sample_count: samples.len(),
            metrics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ts: u64, cpu: f32) -> PerfSample {
        PerfSample {
            ts,
            cpu_pct: cpu,
            cpu_pct_max: cpu,
            net_rx_b: 100,
            ..Default::default()
        }
    }

    #[test]
    fn fold_means_gauges_sums_deltas_keeps_max() {
        let folded = fold(&[sample(1, 10.0), sample(2, 30.0)]).unwrap();
        assert!((folded.cpu_pct - 20.0).abs() < 0.01);
        assert_eq!(folded.cpu_pct_max, 30.0);
        assert_eq!(folded.net_rx_b, 200);
        assert_eq!(folded.ts, 2);
    }

    #[test]
    fn record_cascades_folds() {
        let store = TelemetryStore::default();
        for i in 0..60 {
            store.record(sample(i, 50.0));
        }
        assert_eq!(store.fine.lock().unwrap().len(), 60);
        assert_eq!(store.medium.lock().unwrap().len(), 6);
        assert_eq!(store.coarse.lock().unwrap().len(), 1);
    }

    #[test]
    fn series_filters_by_time_and_metric() {
        let store = TelemetryStore::default();
        for i in 0..10 {
            store.record(sample(100 + i, i as f32));
        }
        let s = store.series(
            PerfTier::Fine,
            Some(105),
            None,
            Some(&["cpu_pct".to_string()]),
        );
        assert_eq!(s.metrics.len(), 1);
        assert_eq!(s.metrics["cpu_pct"].len(), 5);
        assert_eq!(s.start_ts, 105);
    }

    #[test]
    fn summary_stats() {
        let store = TelemetryStore::default();
        for i in 0..100 {
            store.record(sample(i, i as f32));
        }
        let sum = store.summary(PerfTier::Fine, 1000);
        let cpu = &sum.metrics["cpu_pct"];
        assert_eq!(cpu.min, 0.0);
        assert_eq!(cpu.max, 99.0);
        assert!((cpu.avg - 49.5).abs() < 0.01);
        assert!(cpu.p95 >= 94.0);
        assert_eq!(sum.sample_count, 100);
    }
}
