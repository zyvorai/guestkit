// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance-history wire types (protocol 1.3).
//!
//! The agent keeps three fixed-size rolling buffers of samples
//! (1 s × 15 min, 10 s × 6 h, 1 min × 7 d) and serves them through
//! `guestkit.getPerformanceHistory` / `guestkit.getPerformanceSummary`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which rolling buffer a history query targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfTier {
    /// 1-second samples, last 15 minutes.
    Fine,
    /// 10-second samples, last 6 hours.
    Medium,
    /// 1-minute samples, last 7 days.
    Coarse,
}

impl PerfTier {
    pub fn step_secs(self) -> u64 {
        match self {
            Self::Fine => 1,
            Self::Medium => 10,
            Self::Coarse => 60,
        }
    }
}

/// Columnar time series: one aligned value vector per metric name, to keep
/// JSON compact for large windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSeries {
    pub tier: PerfTier,
    /// Unix timestamp (secs) of the first sample.
    pub start_ts: u64,
    pub step_secs: u64,
    /// Metric name → values, all vectors the same length. Gaps are `null`
    /// (deserialized as NaN-free `Option` on strict consumers).
    pub metrics: BTreeMap<String, Vec<Option<f64>>>,
}

/// Aggregate statistics for one metric over a queried window.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MetricStats {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
    pub p95: f64,
}

/// Aggregated view over a window in one tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSummary {
    pub tier: PerfTier,
    pub window_secs: u64,
    pub sample_count: usize,
    pub metrics: BTreeMap<String, MetricStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_round_trip() {
        let mut metrics = BTreeMap::new();
        metrics.insert("cpu_pct".to_string(), vec![Some(1.5), None, Some(2.0)]);
        let s = PerfSeries {
            tier: PerfTier::Fine,
            start_ts: 1_700_000_000,
            step_secs: PerfTier::Fine.step_secs(),
            metrics,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"tier\":\"fine\""));
        let back: PerfSeries = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metrics["cpu_pct"].len(), 3);
        assert_eq!(back.metrics["cpu_pct"][1], None);
    }
}
