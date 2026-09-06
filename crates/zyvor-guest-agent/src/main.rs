// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Standalone Zeus VM Tools guest agent entry (Linux + Windows).

use anyhow::{bail, Context, Result};
use std::env;

#[cfg(windows)]
mod service;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = env::args().collect();

    // Self-test: run a battery of read-only RPC methods against the local
    // handler and write the results as JSON to a file. Used to validate the
    // agent (especially the Windows probe code paths) at runtime without a
    // transport/channel — e.g. driven by a boot service inside a VM.
    if let Some(idx) = args.iter().position(|a| a == "selftest") {
        let out = args.get(idx + 1).cloned().unwrap_or_else(|| {
            if cfg!(windows) {
                "C:\\gk-selftest.json".to_string()
            } else {
                "/tmp/gk-selftest.json".to_string()
            }
        });
        let handler = guestkit::agent::handler::RequestHandler::new();
        // Fast, self-contained probes first; heavier live-evidence collection
        // (many PowerShell spawns) last, so a slow/hung probe still leaves the
        // earlier results on disk. Results are flushed after every method.
        let methods = [
            "guestkit.ping",
            "guestkit.getVersion",
            "guestkit.getCapabilities",
            "guestkit.getAgentHealth",
            "guestkit.users.inventory",
            "guestkit.integrity.baseline",
            "guestkit.integrity.check",
            "guestkit.containers.inventory",
            "guestkit.certificates.inventory",
            "guestkit.packages.inventory",
            "guestkit.security.posture",
            "guestkit.getEvidence",
        ];
        let mut results = serde_json::Map::new();
        let flush = |results: &serde_json::Map<String, serde_json::Value>, done: &str| {
            let doc = serde_json::json!({
                "platform": std::env::consts::OS,
                "agent_version": guestkit::VERSION,
                "last_completed": done,
                "results": results,
            });
            let _ = std::fs::write(&out, serde_json::to_vec_pretty(&doc).unwrap_or_default());
        };
        // Start marker so we can tell the exe ran even if the first probe hangs.
        flush(&results, "starting");
        for m in methods {
            let req = format!(r#"{{"jsonrpc":"2.0","method":"{m}","id":1}}"#);
            let resp = handler.handle(req.as_bytes());
            let entry = if let Some(err) = &resp.error {
                serde_json::json!({ "ok": false, "error": err.message })
            } else {
                serde_json::json!({ "ok": true, "result": resp.result })
            };
            results.insert(m.to_string(), entry);
            flush(&results, m);
        }
        println!("selftest written to {out}");
        return Ok(());
    }

    if args.iter().any(|a| a == "rpc" || a == "--rpc") {
        #[cfg(unix)]
        {
            let socket = parse_flag(&args, "--socket");
            return guestkit::agent::local_client::run_rpc_stdio(socket.as_deref());
        }
        #[cfg(not(unix))]
        bail!("rpc requires Unix local socket");
    }

    if args.iter().any(|a| a == "status") {
        #[cfg(unix)]
        {
            let socket = parse_flag(&args, "--socket");
            return guestkit::agent::local_client::print_local_status(socket.as_deref());
        }
        #[cfg(not(unix))]
        bail!("status requires Unix local socket");
    }

    if args.iter().any(|a| a == "--check-update") {
        let check = guestkit::agent::updater::check_update().await?;
        if check.update_available {
            println!(
                "update available: {} -> {} (channel {})",
                check.current_version,
                check.remote_version.unwrap_or_default(),
                check.channel
            );
            if let Some(url) = &check.artifact_url {
                println!("artifact: {url}");
            }
            if let Some(sha) = &check.artifact_sha256 {
                println!("sha256: {sha}");
            }
        } else {
            println!(
                "zyvor-guest-agent {} is current (channel {})",
                check.current_version, check.channel
            );
        }
        return Ok(());
    }

    if args.iter().any(|a| a == "--apply-update") {
        let msg = guestkit::agent::updater::stage_update(true).await?;
        println!("{msg}");
        return Ok(());
    }

    if args.iter().any(|a| a == "--scheduled-update") {
        let msg = guestkit::agent::updater::run_scheduled_update().await?;
        println!("{msg}");
        return Ok(());
    }

    if args.iter().any(|a| a == "sign-manifest") {
        let json = args
            .iter()
            .position(|a| a == "sign-manifest")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .context("usage: zyvor-guest-agent sign-manifest '<json>'")?;
        let sig = guestkit::agent::updater::sign_manifest_cli(json)?;
        println!("{sig}");
        return Ok(());
    }

    if args.iter().any(|a| a == "--service") {
        #[cfg(windows)]
        {
            return service::run_service();
        }
        #[cfg(not(windows))]
        bail!("--service is supported on Windows only");
    }

    let channel = parse_channel(&args);
    let device = parse_flag(&args, "--device");
    let socket = parse_flag(&args, "--socket");

    guestkit::agent::run_agent(guestkit::agent::AgentArgs {
        channel,
        device,
        socket,
        user: parse_flag(&args, "--user"),
    })
    .await
}

fn parse_channel(args: &[String]) -> guestkit::agent::AgentChannel {
    if let Some(i) = args.iter().position(|a| a == "--channel") {
        if let Some(val) = args.get(i + 1) {
            match val.as_str() {
                "stdio" => return guestkit::agent::AgentChannel::Stdio,
                "vsock" => return guestkit::agent::AgentChannel::Vsock,
                "virtio" | _ => return guestkit::agent::AgentChannel::Virtio,
            }
        }
    }
    #[cfg(windows)]
    {
        guestkit::agent::AgentChannel::Stdio
    }
    #[cfg(not(windows))]
    {
        guestkit::agent::AgentChannel::Virtio
    }
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
