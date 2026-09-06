// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI entry points for agent subcommands.

use crate::agent::daemon::AgentDaemon;
#[cfg(not(target_os = "windows"))]
use crate::agent::proxy;
use crate::agent::transport::{ChannelKind, TransportConfig};
use anyhow::{Context, Result};
use clap::ValueEnum;

#[derive(Debug, Clone, ValueEnum)]
pub enum AgentChannel {
    Virtio,
    Vsock,
    Stdio,
}

impl AgentChannel {
    pub fn to_kind(&self) -> ChannelKind {
        match self {
            Self::Virtio => ChannelKind::Virtio,
            Self::Vsock => ChannelKind::Vsock,
            Self::Stdio => ChannelKind::Stdio,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentArgs {
    pub channel: AgentChannel,
    pub device: Option<String>,
    pub socket: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentProxyArgs {
    pub socket: Option<String>,
    pub listen: Option<String>,
    pub vsock_port: Option<u32>,
}

pub async fn run_agent(args: AgentArgs) -> Result<()> {
    let config = TransportConfig {
        kind: args.channel.to_kind(),
        device_path: args
            .device
            .unwrap_or_else(|| crate::agent::transport::virtio::DEFAULT_DEVICE.to_string()),
        vsock_cid: args.socket.as_deref().and_then(parse_vsock_cid),
        vsock_port: args.socket.as_deref().and_then(parse_vsock_port),
    };

    if let Some(user) = args.user {
        drop(user);
        // User drop is best-effort; full privilege separation deferred to systemd unit.
        log::warn!("--user is reserved for future privilege separation; running as current user");
    }

    let daemon = AgentDaemon::new(config);
    daemon.run().await.context("agent daemon failed")
}

#[derive(Debug, Clone)]
pub struct AgentCallArgs {
    pub socket: String,
    pub method: String,
    pub params: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QgaArgs {
    pub socket: Option<String>,
    pub execute: Option<String>,
    pub arguments: Option<String>,
    pub raw: Option<String>,
}

/// Drop-in replacement for `virsh qemu-agent-command`.
#[cfg(unix)]
pub async fn run_qga(args: QgaArgs) -> Result<()> {
    let socket = match args.socket {
        Some(s) if !s.is_empty() => s,
        _ => crate::agent::qga_client::discover_qga_socket(&[])
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no QGA socket found; pass --socket (typical libvirt path: \
                     /var/lib/libvirt/qemu/channel/target/<domain>/org.qemu.guest_agent.0)"
                )
            })?
            .to_string_lossy()
            .into_owned(),
    };

    let timeout = std::time::Duration::from_secs(30);
    let result = tokio::task::spawn_blocking(move || {
        if let Some(raw) = args.raw.as_deref().filter(|s| !s.trim().is_empty()) {
            crate::agent::qga_client::call_qga_socket_raw(&socket, raw, timeout)
        } else {
            let execute = args
                .execute
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("pass --execute CMD or --raw JSON"))?;
            let arguments = match args.arguments.as_deref() {
                Some(raw) if !raw.trim().is_empty() => {
                    Some(serde_json::from_str(raw).context("parse --arguments JSON")?)
                }
                _ => None,
            };
            crate::agent::qga_client::call_qga_socket(&socket, execute, arguments, timeout)
        }
    })
    .await??;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(unix)]
pub async fn run_agent_call(args: AgentCallArgs) -> Result<()> {
    let params = match args.params.as_deref() {
        Some(raw) if !raw.trim().is_empty() => {
            serde_json::from_str(raw).context("parse --params JSON")?
        }
        _ => serde_json::json!({}),
    };
    let socket = args.socket;
    let method = args.method;
    let result = tokio::task::spawn_blocking(move || {
        crate::agent::agent_call::call_agent_socket(&socket, &method, params)
    })
    .await??;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub async fn run_agent_proxy(args: AgentProxyArgs) -> Result<()> {
    proxy::run_proxy(
        args.socket.as_deref(),
        args.listen.as_deref(),
        args.vsock_port,
    )
    .await
    .context("agent proxy failed")
}

fn parse_vsock_cid(socket: &str) -> Option<u32> {
    let part = socket.split(':').next()?;
    part.parse().ok()
}

fn parse_vsock_port(socket: &str) -> Option<u32> {
    socket.split(':').nth(1)?.parse().ok()
}
