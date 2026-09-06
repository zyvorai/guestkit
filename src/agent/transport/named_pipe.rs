// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows named-pipe local API (`\\.\pipe\guestkit-agent`).
//!
//! Same framed JSON-RPC as the Unix local socket: 4-byte big-endian
//! length prefix + JSON, one request per connection. Server access is
//! limited to Administrators/SYSTEM via the default pipe security
//! descriptor for a SYSTEM-owned service.

pub const PIPE_NAME: &str = r"\\.\pipe\guestkit-agent";

#[cfg(windows)]
pub fn spawn_pipe_server(handler: std::sync::Arc<crate::agent::handler::RequestHandler>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ServerOptions;

    tokio::spawn(async move {
        // The first server instance must be created before clients connect;
        // each accepted connection immediately re-arms a new instance.
        let mut server = match ServerOptions::new()
            .first_pipe_instance(true)
            .create(PIPE_NAME)
        {
            Ok(s) => s,
            Err(e) => {
                log::warn!("named pipe unavailable: {e}");
                return;
            }
        };
        log::info!("GuestKit agent local API listening on {PIPE_NAME}");
        loop {
            if let Err(e) = server.connect().await {
                log::debug!("pipe connect: {e}");
                continue;
            }
            let connected = server;
            server = match ServerOptions::new().create(PIPE_NAME) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("pipe re-arm failed: {e}");
                    return;
                }
            };
            let handler = std::sync::Arc::clone(&handler);
            tokio::spawn(async move {
                let mut conn = connected;
                let mut len_buf = [0u8; 4];
                if conn.read_exact(&mut len_buf).await.is_err() {
                    return;
                }
                let len = u32::from_be_bytes(len_buf) as usize;
                if len > 16 * 1024 * 1024 {
                    return;
                }
                let mut frame = vec![0u8; len];
                if conn.read_exact(&mut frame).await.is_err() {
                    return;
                }
                let response = tokio::task::spawn_blocking(move || handler.handle_frame(&frame))
                    .await
                    .unwrap_or_default();
                let _ = conn.write_all(&(response.len() as u32).to_be_bytes()).await;
                let _ = conn.write_all(&response).await;
                let _ = conn.flush().await;
            });
        }
    });
}

#[cfg(not(windows))]
pub fn spawn_pipe_server(_handler: std::sync::Arc<crate::agent::handler::RequestHandler>) {}
