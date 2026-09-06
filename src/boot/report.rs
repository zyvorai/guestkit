// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Bootability report types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootabilityReport {
    pub score: f64,
    pub confidence: f64,
    pub target: String,
    pub blockers: Vec<Finding>,
    pub warnings: Vec<Finding>,
    pub checks: Vec<CheckResult>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub severity: CheckSeverity,
    pub message: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckSeverity {
    Blocker,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub check_id: String,
    pub title: String,
    pub message: String,
    pub remediation: Option<String>,
}

impl BootabilityReport {
    pub fn assurance_score_message(&self) -> String {
        format!(
            "{:.0}% boot assurance score on {} (confidence: {:.0}%)",
            self.score,
            self.target,
            self.confidence * 100.0
        )
    }

    /// Alias for [`assurance_score_message`](Self::assurance_score_message).
    pub fn boot_probability_message(&self) -> String {
        self.assurance_score_message()
    }
}

#[cfg(test)]
mod tests {
    use super::BootabilityReport;

    #[test]
    fn assurance_score_message_uses_assurance_wording() {
        let report = BootabilityReport {
            score: 82.0,
            confidence: 0.91,
            target: "kvm".into(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: String::new(),
        };
        let msg = report.assurance_score_message();
        assert!(msg.contains("boot assurance score"));
        assert!(!msg.contains("chance of successful first boot"));
    }
}
