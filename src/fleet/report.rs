// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fleet analysis report.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetAnalysisReport {
    pub total_vms: usize,
    pub clusters: Vec<VmCluster>,
    pub snowflakes: Vec<SnowflakeVm>,
    pub migration_blockers: Vec<MigrationBlocker>,
    pub golden_image_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_vms: Vec<FleetFailedVm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetFailedVm {
    pub image: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmCluster {
    pub id: usize,
    pub count: usize,
    pub label: String,
    pub members: Vec<String>,
    pub os: String,
    pub kernel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowflakeVm {
    pub image: String,
    pub reason: String,
    pub similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBlocker {
    pub image: String,
    pub issue: String,
    pub boot_score: f64,
}
