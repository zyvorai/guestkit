// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! guestkitctl — local troubleshooting CLI for the GuestKit agent.
//!
//! Talks to the agent's local socket (`/run/guestkit/agent.sock`, legacy
//! zyvor path as fallback) with one framed request per connection.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "guestkitctl", about = "Local GuestKit agent control", version)]
struct Cli {
    /// Agent socket path override
    #[arg(long, global = true, value_name = "PATH")]
    socket: Option<String>,

    /// Raw JSON output
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Agent + guest health summary
    Status,
    /// Rich heartbeat (agent state, pressure, pending reboot)
    Health,
    /// Performance summary
    Perf {
        /// Tier: fine, medium, coarse
        #[arg(long, default_value = "fine")]
        tier: String,
        /// Window in seconds
        #[arg(long, default_value_t = 900)]
        window: u64,
    },
    /// List failed services
    Services {
        /// Show all units, not only failed
        #[arg(long)]
        all: bool,
    },
    /// Migration readiness assessment
    Assess {
        #[arg(long, default_value = "kvm")]
        target: String,
    },
    /// Security posture report
    Posture,
    /// Installed package inventory
    Packages,
    /// Available package updates (security-classified)
    Updates,
    /// Certificate and SSH host-key inventory
    Certs,
    /// Local user and access inventory
    Users,
    /// Container / Kubernetes awareness + migration risks
    Containers,
    /// Capture a security integrity baseline
    IntegrityBaseline,
    /// Check current state against the integrity baseline (tamper detection)
    IntegrityCheck,
    /// Network connections with process/unit correlation
    Connections {
        /// Show only the aggregated egress map
        #[arg(long)]
        egress: bool,
    },
    /// Application-consistent snapshot: quiesce apps + freeze
    SnapshotPrepare {
        /// Auto-thaw watchdog in seconds
        #[arg(long, default_value_t = 120)]
        watchdog: u64,
    },
    /// Complete a prepared snapshot: thaw + resume applications
    SnapshotComplete,
    /// Freeze filesystems
    Freeze,
    /// Thaw filesystems
    Thaw,
    /// Collect a support bundle
    Bundle {
        /// Output file (default: guestkit-support.tar.zst)
        #[arg(short, long, default_value = "guestkit-support.tar.zst")]
        output: String,
    },
    /// Invoke any agent method directly
    Call {
        method: String,
        /// JSON params
        #[arg(long, default_value = "{}")]
        params: String,
    },
}

#[cfg(unix)]
fn call(cli: &Cli, method: &str, params: Value) -> Result<Value> {
    guestkit::agent::local_client::call_local(cli.socket.as_deref(), method, params)
}

/// Windows: framed request/response over the agent's named pipe.
#[cfg(windows)]
fn call(cli: &Cli, method: &str, params: Value) -> Result<Value> {
    use std::io::{Read, Write};
    let pipe_path = cli
        .socket
        .clone()
        .unwrap_or_else(|| guestkit::agent::transport::named_pipe::PIPE_NAME.to_string());
    let mut pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&pipe_path)
        .map_err(|e| anyhow::anyhow!("connect to agent pipe {pipe_path}: {e}"))?;
    let req = serde_json::json!({
        "jsonrpc": "2.0", "method": method, "params": params, "id": 1
    });
    let payload = serde_json::to_vec(&req)?;
    pipe.write_all(&(payload.len() as u32).to_be_bytes())?;
    pipe.write_all(&payload)?;
    pipe.flush()?;
    let mut len_buf = [0u8; 4];
    pipe.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut frame = vec![0u8; len];
    pipe.read_exact(&mut frame)?;
    let resp: serde_json::Value = serde_json::from_slice(&frame)?;
    if let Some(err) = resp.get("error") {
        anyhow::bail!(
            "agent RPC error {}: {}",
            err.get("code").and_then(Value::as_i64).unwrap_or(0),
            err.get("message").and_then(Value::as_str).unwrap_or("")
        );
    }
    Ok(resp.get("result").cloned().unwrap_or(Value::Null))
}

fn print_result(cli: &Cli, value: &Value) {
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    } else {
        print_human(value, 0);
    }
}

fn print_human(value: &Value, indent: usize) {
    let pad = "  ".repeat(indent);
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{pad}{k}:");
                        print_human(v, indent + 1);
                    }
                    _ => println!("{pad}{k}: {v}"),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::Object(_) => {
                        println!("{pad}-");
                        print_human(item, indent + 1);
                    }
                    _ => println!("{pad}- {item}"),
                }
            }
        }
        other => println!("{pad}{other}"),
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match &cli.command {
        Cmd::Status => {
            let health = call(&cli, "guestkit.getGuestHealth", json!({}))?;
            print_result(&cli, &health);
        }
        Cmd::Health => {
            let hb = call(&cli, "guestkit.getAgentHealth", json!({}))?;
            print_result(&cli, &hb);
        }
        Cmd::Perf { tier, window } => {
            let summary = call(
                &cli,
                "guestkit.getPerformanceSummary",
                json!({ "tier": tier, "window_secs": window }),
            )?;
            print_result(&cli, &summary);
        }
        Cmd::Services { all } => {
            let method = if *all {
                "guestkit.getSystemdUnits"
            } else {
                "guestkit.getFailedUnits"
            };
            let units = call(&cli, method, json!({}))?;
            print_result(&cli, &units);
        }
        Cmd::Assess { target } => {
            let assessment = call(
                &cli,
                "guestkit.migration.assess",
                json!({ "target": target }),
            )?;
            if cli.json {
                print_result(&cli, &assessment);
            } else {
                println!(
                    "Migration Score: {}/100  ({})",
                    assessment["overall_score"],
                    assessment["readiness"].as_str().unwrap_or("?")
                );
                if let Some(subs) = assessment["sub_scores"].as_object() {
                    for (k, v) in subs {
                        println!("  {k:<12} {v}");
                    }
                }
                if let Some(blockers) = assessment["critical_blockers"].as_array() {
                    for b in blockers {
                        println!(
                            "  BLOCKER [{}] {}",
                            b["check_id"].as_str().unwrap_or(""),
                            b["message"].as_str().unwrap_or("")
                        );
                    }
                }
            }
        }
        Cmd::Posture => {
            let report = call(&cli, "guestkit.security.posture", json!({}))?;
            if cli.json {
                print_result(&cli, &report);
            } else {
                println!("Security Posture: {}/100", report["overall_score"]);
                if let Some(cats) = report["categories"].as_array() {
                    for cat in cats {
                        println!(
                            "  {:<20} {}",
                            cat["name"].as_str().unwrap_or(""),
                            cat["score"]
                        );
                        if let Some(findings) = cat["findings"].as_array() {
                            for f in findings.iter().filter(|f| f["passed"] == false) {
                                println!(
                                    "    ✗ [{}] {}",
                                    f["id"].as_str().unwrap_or(""),
                                    f["message"].as_str().unwrap_or("")
                                );
                            }
                        }
                    }
                }
            }
        }
        Cmd::Packages => {
            let inv = call(&cli, "guestkit.packages.inventory", json!({}))?;
            if cli.json {
                print_result(&cli, &inv);
            } else {
                println!(
                    "{} packages via {} (running kernel {})",
                    inv["installed_count"],
                    inv["manager"].as_str().unwrap_or("?"),
                    inv["running_kernel"].as_str().unwrap_or("?")
                );
            }
        }
        Cmd::Updates => {
            let up = call(&cli, "guestkit.packages.updates", json!({}))?;
            if cli.json {
                print_result(&cli, &up);
            } else {
                println!(
                    "{} update(s) available, {} security, reboot_required={}",
                    up["available_count"], up["security_count"], up["reboot_required"]
                );
            }
        }
        Cmd::Certs => {
            let inv = call(&cli, "guestkit.certificates.inventory", json!({}))?;
            if cli.json {
                print_result(&cli, &inv);
            } else {
                println!(
                    "{} certificate(s): {} expiring soon, {} expired, {} weak; {} SSH host key(s)",
                    inv["certificate_count"],
                    inv["expiring_soon"],
                    inv["expired"],
                    inv["weak"],
                    inv["ssh_host_keys"]
                        .as_array()
                        .map(|a| a.len())
                        .unwrap_or(0)
                );
                if let Some(certs) = inv["certificates"].as_array() {
                    for c in certs.iter().filter(|c| {
                        c["expiring_soon"] == true || c["expired"] == true || c["weak"] == true
                    }) {
                        println!(
                            "  ! {} (expires in {}d){}",
                            c["subject"].as_str().unwrap_or(""),
                            c["days_until_expiry"],
                            if c["weak"] == true { " [weak]" } else { "" }
                        );
                    }
                }
            }
        }
        Cmd::Users => {
            let inv = call(&cli, "guestkit.users.inventory", json!({}))?;
            print_result(&cli, &inv);
        }
        Cmd::Containers => {
            let inv = call(&cli, "guestkit.containers.inventory", json!({}))?;
            if cli.json {
                print_result(&cli, &inv);
            } else {
                let runtimes: Vec<String> = inv["runtimes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|r| r["name"].as_str().unwrap_or("?").to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                println!(
                    "{} container(s) via [{}]; {} running, {} privileged, {} host-networked",
                    inv["container_count"],
                    runtimes.join(", "),
                    inv["running"],
                    inv["privileged"],
                    inv["host_networked"]
                );
                if !inv["kubernetes"].is_null() {
                    let k = &inv["kubernetes"];
                    println!(
                        "  kubernetes: {} node ({} pods, {} containers running)",
                        k["distribution"].as_str().unwrap_or("?"),
                        k["running_pods"],
                        k["running_containers"]
                    );
                }
                if let Some(risks) = inv["migration_risks"].as_array() {
                    for r in risks {
                        println!("  ! {}", r.as_str().unwrap_or(""));
                    }
                }
            }
        }
        Cmd::IntegrityBaseline => {
            let r = call(&cli, "guestkit.integrity.baseline", json!({}))?;
            print_result(&cli, &r);
        }
        Cmd::IntegrityCheck => {
            let r = call(&cli, "guestkit.integrity.check", json!({}))?;
            if cli.json {
                print_result(&cli, &r);
            } else if r["has_baseline"] == false {
                println!("No integrity baseline — run integrity-baseline first.");
            } else {
                println!(
                    "Integrity: {} change(s), {} high-severity, tampered={}",
                    r["change_count"], r["high_severity"], r["tampered"]
                );
                if let Some(changes) = r["changes"].as_array() {
                    for c in changes {
                        println!(
                            "  [{}] {} {} — {}",
                            c["severity"].as_str().unwrap_or(""),
                            c["kind"].as_str().unwrap_or(""),
                            c["category"].as_str().unwrap_or(""),
                            c["item"].as_str().unwrap_or("")
                        );
                    }
                }
            }
        }
        Cmd::Connections { egress } => {
            let intel = call(&cli, "guestkit.network.connections", json!({}))?;
            if cli.json {
                print_result(&cli, &intel);
            } else if *egress {
                println!("Egress map (process → destination):");
                if let Some(edges) = intel["egress"].as_array() {
                    for e in edges {
                        println!(
                            "  {} [{}] → {} ({} conn)",
                            e["process"].as_str().unwrap_or("?"),
                            e["unit"].as_str().unwrap_or("-"),
                            e["destination"].as_str().unwrap_or("?"),
                            e["connections"]
                        );
                    }
                }
            } else {
                println!(
                    "listening: {}  established: {}  unique remotes: {}",
                    intel["total_listening"], intel["total_established"], intel["unique_remotes"]
                );
                if let Some(listeners) = intel["listeners"].as_array() {
                    for l in listeners {
                        println!(
                            "  LISTEN {}:{} {} [{}]",
                            l["local_addr"].as_str().unwrap_or(""),
                            l["local_port"],
                            l["process"].as_str().unwrap_or(""),
                            l["unit"].as_str().unwrap_or("-")
                        );
                    }
                }
            }
        }
        Cmd::SnapshotPrepare { watchdog } => {
            let r = call(
                &cli,
                "guestkit.snapshot.prepare",
                json!({ "watchdog_secs": watchdog }),
            )?;
            print_result(&cli, &r);
        }
        Cmd::SnapshotComplete => {
            let r = call(&cli, "guestkit.snapshot.complete", json!({}))?;
            print_result(&cli, &r);
        }
        Cmd::Freeze => {
            let r = call(&cli, "guestkit.freezeFilesystem", json!({}))?;
            print_result(&cli, &r);
        }
        Cmd::Thaw => {
            let r = call(&cli, "guestkit.thawFilesystem", json!({}))?;
            print_result(&cli, &r);
        }
        Cmd::Bundle { output } => {
            let r = call(&cli, "guestkit.collectSupportBundle", json!({}))?;
            let data = r
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("no bundle data in response"))?;
            use base64::engine::general_purpose::STANDARD;
            use base64::Engine;
            let bytes = STANDARD.decode(data)?;
            std::fs::write(output, &bytes)?;
            println!("wrote {} bytes to {}", bytes.len(), output);
        }
        Cmd::Call { method, params } => {
            let params: Value = serde_json::from_str(params)?;
            let r = call(&cli, method, params)?;
            print_result(&cli, &r);
        }
    }
    Ok(())
}
