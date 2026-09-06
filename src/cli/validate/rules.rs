// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rule evaluation helpers

use anyhow::Result;

/// Evaluate a custom rule expression
#[allow(dead_code)]
pub fn evaluate_custom_rule(_expression: &str) -> Result<bool> {
    // This would implement a simple DSL for custom rules
    // For now, just return false
    Ok(false)
}

/// Parse severity level
#[allow(dead_code)]
pub fn parse_severity(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "critical" | "crit" => "critical".to_string(),
        "high" | "h" => "high".to_string(),
        "medium" | "med" | "m" => "medium".to_string(),
        "low" | "l" => "low".to_string(),
        _ => "medium".to_string(),
    }
}
