// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! `guestkit mcp-serve` — MCP server over stdio for a single VM disk image.

use crate::ai::GuestkitMcpServer;
use crate::assurance::{boot_target_from_str, collect_assurance_data};
use crate::boot::analyze_bootability;
use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::path::Path;

/// Mount+inspect one disk image, then serve the 6 read-only evidence tools
/// over MCP via stdio until the client disconnects. Meant to be launched by
/// an MCP host (Claude Desktop, etc.), not run interactively.
pub fn mcp_serve_command(image: &Path, target: &str, verbose: bool) -> Result<()> {
    let boot_target = boot_target_from_str(target);
    let (evidence, _) = collect_assurance_data(image, boot_target, verbose)?;
    let boot = analyze_bootability(&evidence, boot_target);

    if verbose {
        eprintln!(
            "guestkit mcp-serve: {} ({} {}) — listening on stdio",
            image.display(),
            evidence.os.distribution,
            evidence.os.version
        );
    }

    let server = GuestkitMcpServer::new(evidence, Some(boot));

    let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    rt.block_on(async move {
        let running = server
            .serve(rmcp::transport::io::stdio())
            .await
            .context("failed to start MCP server")?;
        running
            .waiting()
            .await
            .context("MCP server exited with an error")?;
        Ok(())
    })
}
