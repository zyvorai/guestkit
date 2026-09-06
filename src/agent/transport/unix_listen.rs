// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Local Unix socket API for in-guest read-only queries.

use crate::agent::handler::RequestHandler;
use anyhow::{Context, Result};
use guestkit_agent_protocol::{read_frame, write_frame};
use std::fs;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

/// Canonical GuestKit socket; the legacy zyvor path stays bound for
/// existing tooling.
pub const DEFAULT_SOCKET_PATH: &str = "/run/guestkit/agent.sock";
pub const LEGACY_SOCKET_PATH: &str = "/var/run/zyvor/guest-agent.sock";

pub fn spawn_local_socket(handler: Arc<RequestHandler>, socket_path: Option<String>) -> Result<()> {
    match socket_path {
        Some(path) => bind_one(handler, path),
        None => {
            // Two independent listeners: symlinked sockets don't reliably
            // connect on every platform, and two binds are cheap.
            bind_one(Arc::clone(&handler), DEFAULT_SOCKET_PATH.to_string())?;
            if let Err(e) = bind_one(handler, LEGACY_SOCKET_PATH.to_string()) {
                log::warn!("legacy socket unavailable: {e}");
            }
            Ok(())
        }
    }
}

fn bind_one(handler: Arc<RequestHandler>, path: String) -> Result<()> {
    if Path::new(&path).exists() {
        fs::remove_file(&path).ok();
    }
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent).ok();
    }

    let listener = UnixListener::bind(&path).with_context(|| format!("bind {path}"))?;
    log::info!("GuestKit agent local API listening on {path}");

    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let handler = Arc::clone(&handler);
            thread::spawn(move || {
                if let Err(e) = serve_connection(conn, &handler) {
                    log::debug!("local socket client error: {e}");
                }
            });
        }
    });

    Ok(())
}

fn serve_connection(stream: UnixStream, handler: &RequestHandler) -> Result<()> {
    let mut reader = stream.try_clone()?;
    let mut writer = stream;
    let frame = read_frame(&mut reader).map_err(|e| anyhow::anyhow!("{e}"))?;
    let response = handler.handle_frame(&frame);
    write_frame(&mut writer, &response).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
