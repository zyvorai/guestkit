// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Root cause analysis report.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseReport {
    pub primary_cause: String,
    pub confidence: f64,
    pub chain: Vec<CauseStep>,
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CauseStep {
    pub step: usize,
    pub description: String,
    pub check_id: Option<String>,
}
