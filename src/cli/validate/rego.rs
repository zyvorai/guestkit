// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Tiny Rego deny-evaluator + optional `opa eval` handoff.
//!
//! GuestKit does not embed the OPA runtime. This module:
//! 1. Evaluates a small `deny[msg] { input.PATH OP VALUE }` subset in-process
//!    so CI works with zero extra binaries.
//! 2. If `opa` is on PATH, also runs `opa eval -f json` and merges results.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegoReport {
    pub package: String,
    pub denies: Vec<String>,
    pub engine: String,
    pub allowed: bool,
}

#[derive(Debug, Clone)]
struct DenyRule {
    path: String,
    op: String,
    expected: String,
    msg: String,
}

pub fn eval_file(rego: &Path, input: &Value) -> Result<RegoReport> {
    let src = std::fs::read_to_string(rego).with_context(|| format!("read {}", rego.display()))?;
    eval_source(&src, input)
}

pub fn eval_source(src: &str, input: &Value) -> Result<RegoReport> {
    let package = src
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("package ")
                .map(|p| p.trim().to_string())
        })
        .unwrap_or_else(|| "guestkit".into());

    let mut denies = Vec::new();
    for rule in parse_deny_rules(src) {
        if matches_rule(&rule, input) {
            denies.push(rule.msg);
        }
    }

    let mut engine = "guestkit-subset".to_string();
    if let Ok(extra) = try_opa(src, input) {
        engine = "opa+subset".into();
        for m in extra {
            if !denies.contains(&m) {
                denies.push(m);
            }
        }
    }

    Ok(RegoReport {
        package,
        allowed: denies.is_empty(),
        denies,
        engine,
    })
}

fn parse_deny_rules(src: &str) -> Vec<DenyRule> {
    let mut rules = Vec::new();
    let mut buf = String::new();
    let mut in_deny = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("deny[") && t.contains('{') {
            in_deny = true;
            buf.clear();
            continue;
        }
        if in_deny && t == "}" {
            if let Some(r) = parse_block(&buf) {
                rules.push(r);
            }
            in_deny = false;
            buf.clear();
            continue;
        }
        if in_deny {
            buf.push_str(t);
            buf.push('\n');
        }
    }
    rules
}

fn parse_block(block: &str) -> Option<DenyRule> {
    let mut path = None;
    let mut op = None;
    let mut expected = None;
    let mut msg = "denied".to_string();
    for line in block.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("msg :=") {
            msg = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            continue;
        }
        if !t.starts_with("input.") {
            continue;
        }
        for candidate in [">=", "<=", "!=", "==", ">", "<"] {
            if let Some(idx) = t.find(candidate) {
                path = Some(t[..idx].trim().trim_start_matches("input.").to_string());
                op = Some(candidate.to_string());
                expected = Some(
                    t[idx + candidate.len()..]
                        .trim()
                        .trim_end_matches(',')
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
                break;
            }
        }
    }
    Some(DenyRule {
        path: path?,
        op: op?,
        expected: expected?,
        msg,
    })
}

fn matches_rule(rule: &DenyRule, input: &Value) -> bool {
    let actual = lookup(input, &rule.path);
    compare(actual.as_deref(), &rule.op, &rule.expected)
}

fn lookup(v: &Value, path: &str) -> Option<String> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    match cur {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null => Some("null".into()),
        other => Some(other.to_string()),
    }
}

fn compare(actual: Option<&str>, op: &str, expected: &str) -> bool {
    let Some(actual) = actual else {
        return op == "!=";
    };
    if let (Ok(a), Ok(b)) = (actual.parse::<f64>(), expected.parse::<f64>()) {
        return match op {
            "==" => (a - b).abs() < f64::EPSILON,
            "!=" => (a - b).abs() >= f64::EPSILON,
            ">" => a > b,
            ">=" => a >= b,
            "<" => a < b,
            "<=" => a <= b,
            _ => false,
        };
    }
    match op {
        "==" => actual == expected,
        "!=" => actual != expected,
        _ => false,
    }
}

fn try_opa(src: &str, input: &Value) -> Result<Vec<String>> {
    let opa = which_opa().ok_or_else(|| anyhow::anyhow!("no opa"))?;
    let dir = tempfile::tempdir()?;
    let rego = dir.path().join("policy.rego");
    let inp = dir.path().join("input.json");
    std::fs::write(&rego, src)?;
    std::fs::write(&inp, serde_json::to_vec(input)?)?;
    let out = Command::new(opa)
        .args([
            "eval",
            "-f",
            "json",
            "-d",
            rego.to_str().unwrap(),
            "-i",
            inp.to_str().unwrap(),
            "data.guestkit.deny",
        ])
        .output()?;
    if !out.status.success() {
        anyhow::bail!("opa eval failed");
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    let mut msgs = Vec::new();
    if let Some(arr) = v
        .pointer("/result/0/expressions/0/value")
        .and_then(|x| x.as_array())
    {
        for item in arr {
            if let Some(s) = item.as_str() {
                msgs.push(s.to_string());
            }
        }
    }
    Ok(msgs)
}

fn which_opa() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("OPA_BIN").map(std::path::PathBuf::from) {
        if p.exists() {
            return Some(p);
        }
    }
    Command::new("opa")
        .arg("version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| std::path::PathBuf::from("opa"))
}

/// Facts document consumed by cutover.rego (passport-shaped).
pub fn facts_from_passport_json(raw: &str) -> Result<Value> {
    let v: Value = serde_json::from_str(raw).context("parse passport/facts JSON")?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGO: &str = r#"
package guestkit

deny[msg] {
  input.hard_blocked == true
  msg := "hard-blocked"
}

deny[msg] {
  input.scores.boot < 80
  msg := "boot score below 80"
}
"#;

    #[test]
    fn denies_hard_block_and_low_score() {
        let input = serde_json::json!({
            "hard_blocked": true,
            "scores": { "boot": 40.0, "migration": 40.0 }
        });
        let r = eval_source(REGO, &input).unwrap();
        assert!(!r.allowed);
        assert!(r.denies.iter().any(|m| m.contains("hard-blocked")));
        assert!(r.denies.iter().any(|m| m.contains("boot score")));
    }

    #[test]
    fn allows_clean_passport() {
        let input = serde_json::json!({
            "hard_blocked": false,
            "scores": { "boot": 91.0, "migration": 88.0 }
        });
        let r = eval_source(REGO, &input).unwrap();
        assert!(r.allowed);
        assert!(r.denies.is_empty());
    }
}
