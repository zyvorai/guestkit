// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration analysis utilities

use super::*;

/// Analyze migration feasibility
pub fn analyze_feasibility(plan: &MigrationPlan) -> FeasibilityReport {
    let critical_blockers = plan
        .issues
        .iter()
        .filter(|i| i.severity == RiskLevel::Critical)
        .count();

    let high_risks = plan
        .issues
        .iter()
        .filter(|i| i.severity == RiskLevel::High)
        .count();

    let is_feasible = critical_blockers == 0 && plan.compatibility_score >= 50.0;

    let confidence = if plan.compatibility_score >= 90.0 {
        "Very High"
    } else if plan.compatibility_score >= 75.0 {
        "High"
    } else if plan.compatibility_score >= 60.0 {
        "Medium"
    } else if plan.compatibility_score >= 40.0 {
        "Low"
    } else {
        "Very Low"
    };

    FeasibilityReport {
        is_feasible,
        confidence: confidence.to_string(),
        critical_blockers,
        high_risks,
        recommendation: if is_feasible {
            "Migration is feasible with proper planning".to_string()
        } else {
            "Migration has significant risks - consider alternatives".to_string()
        },
    }
}

/// Feasibility report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeasibilityReport {
    pub is_feasible: bool,
    pub confidence: String,
    pub critical_blockers: usize,
    pub high_risks: usize,
    pub recommendation: String,
}

/// Calculate downtime estimate
pub fn estimate_downtime(plan: &MigrationPlan) -> DowntimeEstimate {
    let base_minutes = match plan.migration_type.as_str() {
        "OS Upgrade" => 120,      // 2 hours
        "Cloud Migration" => 240, // 4 hours
        "Containerization" => 60, // 1 hour (minimal downtime)
        _ => 120,
    };

    // Add time for issues
    let issue_minutes = plan
        .issues
        .iter()
        .map(|i| match i.severity {
            RiskLevel::Critical => 60,
            RiskLevel::High => 30,
            RiskLevel::Medium => 15,
            RiskLevel::Low => 5,
        })
        .sum::<u32>();

    // Add buffer for unexpected issues
    let total_minutes = (base_minutes + issue_minutes) as f64 * 1.3;

    DowntimeEstimate {
        minimum_minutes: base_minutes,
        expected_minutes: total_minutes as u32,
        maximum_minutes: (total_minutes * 1.5) as u32,
        can_rollback: plan.steps.iter().any(|s| s.rollback.is_some()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DowntimeEstimate {
    pub minimum_minutes: u32,
    pub expected_minutes: u32,
    pub maximum_minutes: u32,
    pub can_rollback: bool,
}

impl DowntimeEstimate {
    pub fn expected_hours(&self) -> f64 {
        self.expected_minutes as f64 / 60.0
    }
}
