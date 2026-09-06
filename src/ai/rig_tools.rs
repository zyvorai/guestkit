// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Native rig-core `Tool` wrappers over [`SnapshotTools`].
//!
//! Each of the 6 read-only evidence tools gets its own zero-sized-ish
//! wrapper struct implementing `rig::tool::Tool`, so the model's tool calls
//! are schema-validated and parsed by the provider SDK instead of being
//! regex/JSON-scraped out of raw completion text (see `agent.rs`'s legacy
//! `parse_tool_call` path, still used for providers without a native rig
//! client — Ollama).

use crate::ai::tools::SnapshotTools;
use crate::boot::BootabilityReport;
use crate::evidence::snapshot::EvidenceSnapshot;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde_json::Value;
use std::sync::Arc;

/// Shared, owned context each tool wrapper needs to reconstruct a
/// [`SnapshotTools`] per call. `rig::agent::AgentBuilder::tool()` requires
/// `'static`, so this holds owned/Arc'd data rather than the `&'a` borrows
/// `SnapshotTools` itself takes.
#[derive(Clone)]
pub struct SnapshotContext {
    evidence: Arc<EvidenceSnapshot>,
    boot: Arc<Option<BootabilityReport>>,
}

impl SnapshotContext {
    pub fn new(evidence: Arc<EvidenceSnapshot>, boot: Arc<Option<BootabilityReport>>) -> Self {
        Self { evidence, boot }
    }

    pub fn call(&self, name: &str, args: &Value) -> Result<Value, ToolCallErr> {
        let tools = SnapshotTools::new(&self.evidence, self.boot.as_ref().as_ref());
        tools
            .call(name, args)
            .map_err(|e| ToolCallErr(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("tool call failed: {0}")]
pub struct ToolCallErr(String);

/// Defines a `rig::tool::Tool` impl for one of `SnapshotTools`'s dispatch
/// names, wrapping raw `serde_json::Value` args/output — the same shape
/// `SnapshotTools::call` already uses, so no new arg types are needed.
macro_rules! snapshot_tool {
    ($struct_name:ident, $tool_name:literal, $description:literal, $schema:expr) => {
        #[derive(Clone)]
        pub struct $struct_name(pub SnapshotContext);

        impl Tool for $struct_name {
            const NAME: &'static str = $tool_name;

            type Error = ToolCallErr;
            type Args = Value;
            type Output = Value;

            async fn definition(&self, _prompt: String) -> ToolDefinition {
                ToolDefinition {
                    name: $tool_name.to_string(),
                    description: $description.to_string(),
                    parameters: $schema,
                }
            }

            async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                self.0.call($tool_name, &args)
            }
        }
    };
}

snapshot_tool!(
    ListSystemdUnitsTool,
    "list_systemd_units",
    "List systemd units from the evidence snapshot, optionally filtered by unit type (service, timer, socket, ...).",
    serde_json::json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "description": "Optional unit type filter, e.g. \"service\" or \"timer\". Omit or empty string for all units."
            }
        }
    })
);

snapshot_tool!(
    GetUnitDetailsTool,
    "get_unit_details",
    "Get full details for a single named systemd unit.",
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "Exact unit name, e.g. \"sshd.service\"."
            }
        },
        "required": ["name"]
    })
);

snapshot_tool!(
    GetBootBlockersTool,
    "get_boot_blockers",
    "Get the boot assurance score, blockers, and warnings for this VM against the configured migration target.",
    serde_json::json!({"type": "object", "properties": {}})
);

snapshot_tool!(
    GetSemanticSummaryTool,
    "get_semantic_summary",
    "Get the semantic analysis summary: service dependency graph, sandboxing scores, and general findings.",
    serde_json::json!({"type": "object", "properties": {}})
);

snapshot_tool!(
    GetWindowsRisksTool,
    "get_windows_risks",
    "Get Windows-specific service risk flags from the semantic analysis (only meaningful for Windows guests).",
    serde_json::json!({"type": "object", "properties": {}})
);

snapshot_tool!(
    GetRecommendationsTool,
    "get_recommendations",
    "Get the proactive remediation recommendations engine's output for this VM.",
    serde_json::json!({"type": "object", "properties": {}})
);

/// Register all 6 tools onto a rig `AgentBuilder`, consuming and returning it.
///
/// `AgentBuilder::tool()` changes the builder's type on first call
/// (`AgentBuilder<M>` -> `AgentBuilderSimple<M>`), so this can't be a loop
/// over a slice — it's written out tool-by-tool.
pub fn register_all<M>(
    builder: rig::agent::AgentBuilder<M>,
    ctx: &SnapshotContext,
) -> rig::agent::AgentBuilderSimple<M>
where
    M: rig::completion::CompletionModel,
{
    builder
        .tool(ListSystemdUnitsTool(ctx.clone()))
        .tool(GetUnitDetailsTool(ctx.clone()))
        .tool(GetBootBlockersTool(ctx.clone()))
        .tool(GetSemanticSummaryTool(ctx.clone()))
        .tool(GetWindowsRisksTool(ctx.clone()))
        .tool(GetRecommendationsTool(ctx.clone()))
}
