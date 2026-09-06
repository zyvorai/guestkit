// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 2 — snapshot tool registry for agentic loops.

use crate::ai::semantic::analyze_semantic;
use crate::ai::{generate_recommendations, SemanticAnalysis};
use crate::boot::BootabilityReport;
use crate::evidence::snapshot::{EvidenceSnapshot, SystemdUnit};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// Registry of deterministic tools over an evidence snapshot.
pub struct SnapshotTools<'a> {
    evidence: &'a EvidenceSnapshot,
    boot: Option<&'a BootabilityReport>,
    semantic: SemanticAnalysis,
}

impl<'a> SnapshotTools<'a> {
    pub fn new(evidence: &'a EvidenceSnapshot, boot: Option<&'a BootabilityReport>) -> Self {
        Self {
            evidence,
            boot,
            semantic: analyze_semantic(evidence),
        }
    }

    pub fn with_boot_target(
        _evidence: &'a EvidenceSnapshot,
        _target: crate::boot::BootTarget,
    ) -> Self {
        Self::new(_evidence, None)
    }

    pub fn tool_names() -> &'static [&'static str] {
        &[
            "list_systemd_units",
            "get_unit_details",
            "get_boot_blockers",
            "get_semantic_summary",
            "get_windows_risks",
            "get_recommendations",
        ]
    }

    pub fn call(&self, name: &str, args: &Value) -> Result<Value> {
        match name {
            "list_systemd_units" => Ok(self.list_systemd_units(args)),
            "get_unit_details" => self.get_unit_details(args),
            "get_boot_blockers" => Ok(self.get_boot_blockers()),
            "get_semantic_summary" => Ok(json!(self.semantic)),
            "get_windows_risks" => Ok(json!(self.semantic.windows_risks)),
            "get_recommendations" => {
                let recs = generate_recommendations(self.evidence, self.boot, &self.semantic);
                Ok(json!(recs))
            }
            other => Err(anyhow!("unknown tool: {other}")),
        }
    }

    fn list_systemd_units(&self, args: &Value) -> Value {
        let filter = args.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let units: Vec<&SystemdUnit> = self
            .evidence
            .systemd
            .as_ref()
            .map(|s| {
                s.units
                    .iter()
                    .filter(|u| filter.is_empty() || u.unit_type == filter)
                    .collect()
            })
            .unwrap_or_default();
        json!(units
            .iter()
            .map(|u| {
                json!({
                    "name": u.name,
                    "type": u.unit_type,
                    "state": format!("{:?}", u.state),
                    "path": u.path,
                })
            })
            .collect::<Vec<_>>())
    }

    fn get_unit_details(&self, args: &Value) -> Result<Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("get_unit_details requires args.name"))?;
        let unit = self
            .evidence
            .systemd
            .as_ref()
            .and_then(|s| s.units.iter().find(|u| u.name == name))
            .ok_or_else(|| anyhow!("unit not found: {name}"))?;
        Ok(json!(unit))
    }

    fn get_boot_blockers(&self) -> Value {
        if let Some(boot) = self.boot {
            json!({
                "score": boot.score,
                "blockers": boot.blockers,
                "warnings": boot.warnings,
            })
        } else {
            json!({"message": "boot report not available"})
        }
    }
}
