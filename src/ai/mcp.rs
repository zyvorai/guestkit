// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) server exposing the same 6 read-only
//! evidence tools as `ai/tools.rs`/`ai/rig_tools.rs`, so external MCP hosts
//! (Claude Desktop, etc.) can query a VM's offline diagnostics directly —
//! independent of guestkit's own AI copilot loop.
//!
//! One server instance holds a single `EvidenceSnapshot` captured at
//! startup (see `guestkit mcp-serve`); it never mutates the guest disk and
//! never re-mounts it.

use crate::ai::rig_tools::SnapshotContext;
use crate::boot::BootabilityReport;
use crate::evidence::snapshot::EvidenceSnapshot;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, Json, ServerHandler};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct GuestkitMcpServer {
    ctx: SnapshotContext,
}

impl GuestkitMcpServer {
    pub fn new(evidence: EvidenceSnapshot, boot: Option<BootabilityReport>) -> Self {
        Self {
            ctx: SnapshotContext::new(Arc::new(evidence), Arc::new(boot)),
        }
    }

    /// Dispatch through the same `SnapshotTools::call` every other tool
    /// surface (agent.rs's text loop, rig_tools.rs) uses, so all three stay
    /// behaviorally identical by construction. Errors become a JSON
    /// `{"error": "..."}` payload rather than an MCP protocol error, since
    /// these are all read-only lookups where "not found"/"bad arg" is a
    /// normal, model-recoverable outcome, not a server fault.
    fn call(&self, name: &str, args: Value) -> Json<Value> {
        Json(
            self.ctx
                .call(name, &args)
                .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })),
        )
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct ListUnitsArgs {
    /// Optional unit type filter, e.g. "service" or "timer". Omit for all units.
    #[serde(default)]
    r#type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UnitNameArgs {
    /// Exact unit name, e.g. "sshd.service".
    name: String,
}

#[tool_router]
impl GuestkitMcpServer {
    #[tool(
        description = "List systemd units from the evidence snapshot, optionally filtered by unit type (service, timer, socket, ...)."
    )]
    async fn list_systemd_units(&self, Parameters(args): Parameters<ListUnitsArgs>) -> Json<Value> {
        self.call(
            "list_systemd_units",
            serde_json::json!({"type": args.r#type}),
        )
    }

    #[tool(description = "Get full details for a single named systemd unit.")]
    async fn get_unit_details(&self, Parameters(args): Parameters<UnitNameArgs>) -> Json<Value> {
        self.call("get_unit_details", serde_json::json!({"name": args.name}))
    }

    #[tool(
        description = "Get the boot assurance score, blockers, and warnings for this VM against its configured migration target."
    )]
    async fn get_boot_blockers(&self) -> Json<Value> {
        self.call("get_boot_blockers", Value::Null)
    }

    #[tool(
        description = "Get the semantic analysis summary: service dependency graph, sandboxing scores, and general findings."
    )]
    async fn get_semantic_summary(&self) -> Json<Value> {
        self.call("get_semantic_summary", Value::Null)
    }

    #[tool(
        description = "Get Windows-specific service risk flags from the semantic analysis (only meaningful for Windows guests)."
    )]
    async fn get_windows_risks(&self) -> Json<Value> {
        self.call("get_windows_risks", Value::Null)
    }

    #[tool(description = "Get the proactive remediation recommendations for this VM.")]
    async fn get_recommendations(&self) -> Json<Value> {
        self.call("get_recommendations", Value::Null)
    }
}

#[tool_handler(
    name = "guestkit",
    instructions = "Read-only offline diagnostics for a single VM disk image, captured at server startup. Tools never mutate the guest disk and never re-mount it — for a different VM, restart the server against that image instead."
)]
impl ServerHandler for GuestkitMcpServer {}
