// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Expression-based policy evaluation over evidence snapshots.

use crate::evidence::EvidenceSnapshot;
use anyhow::Result;

/// Evaluate a simple expression against an evidence snapshot.
///
/// Supported forms:
/// - `field == value` / `field != value`
/// - `field >= number` / `field > number` / `field <= number` / `field < number`
/// - `bootability.score >= 80` (requires boot_score parameter)
pub fn evaluate_expr(
    expr: &str,
    evidence: &EvidenceSnapshot,
    boot_score: Option<f64>,
) -> Result<bool> {
    let expr = expr.trim();

    if expr.starts_with("bootability.score") {
        let score = boot_score.unwrap_or(0.0);
        return eval_numeric_compare(expr.strip_prefix("bootability.score").unwrap(), score);
    }

    if let Some((field, op, value)) = parse_comparison(expr) {
        let actual = resolve_field(field, evidence, boot_score)?;
        return eval_op(&actual, op, value);
    }

    anyhow::bail!("Unsupported expression: {}", expr)
}

fn parse_comparison(expr: &str) -> Option<(&str, &str, &str)> {
    for op in [">=", "<=", "!=", "==", ">", "<"] {
        if let Some(idx) = expr.find(op) {
            let field = expr[..idx].trim();
            let value = expr[idx + op.len()..]
                .trim()
                .trim_matches('\'')
                .trim_matches('"');
            return Some((field, op, value));
        }
    }
    None
}

fn resolve_field(
    field: &str,
    evidence: &EvidenceSnapshot,
    boot_score: Option<f64>,
) -> Result<String> {
    if field == "bootability.score" {
        return Ok(boot_score.unwrap_or(0.0).to_string());
    }
    evidence
        .get_field(field)
        .ok_or_else(|| anyhow::anyhow!("Unknown field: {}", field))
}

fn eval_op(actual: &str, op: &str, expected: &str) -> Result<bool> {
    if let (Ok(a), Ok(b)) = (actual.parse::<f64>(), expected.parse::<f64>()) {
        return Ok(match op {
            ">=" => a >= b,
            "<=" => a <= b,
            ">" => a > b,
            "<" => a < b,
            "==" => (a - b).abs() < f64::EPSILON,
            "!=" => (a - b).abs() >= f64::EPSILON,
            _ => false,
        });
    }

    Ok(match op {
        "==" => actual.eq_ignore_ascii_case(expected) || actual == expected,
        "!=" => actual != expected,
        _ => false,
    })
}

fn eval_numeric_compare(suffix: &str, score: f64) -> Result<bool> {
    let suffix = suffix.trim();
    for op in [">=", "<=", ">", "<", "=="] {
        if let Some(rest) = suffix.strip_prefix(op) {
            let expected: f64 = rest.trim().parse()?;
            return eval_op(&score.to_string(), op, &expected.to_string());
        }
    }
    anyhow::bail!(
        "Invalid bootability expression: bootability.score{}",
        suffix
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::snapshot::*;

    fn sample_evidence() -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: 1,
            image_path: "test.qcow2".to_string(),
            collected_at: String::new(),
            root: "/dev/sda1".to_string(),
            os: OsEvidence {
                distribution: "ubuntu".to_string(),
                ..Default::default()
            },
            storage: StorageEvidence::default(),
            boot: BootEvidence::default(),
            network: NetworkEvidence::default(),
            packages: PackageEvidence::default(),
            security: SecurityEvidence {
                ssh_root_login: Some(false),
                ..Default::default()
            },
            vm_tools: VmToolsEvidence::default(),
            systemd: None,
            windows: None,
            kubevirt: None,
            cloud_init: None,
            network_probes: None,
            snapshot_readiness: None,
            process: None,
            hardware: None,
            linux_migration: None,
            online_cache: None,
        }
    }

    #[test]
    fn test_expr_ssh_root_login() {
        let ev = sample_evidence();
        assert!(evaluate_expr("security.ssh.root_login == false", &ev, None).unwrap());
    }

    #[test]
    fn test_expr_boot_score() {
        let ev = sample_evidence();
        assert!(evaluate_expr("bootability.score >= 80", &ev, Some(85.0)).unwrap());
    }
}
