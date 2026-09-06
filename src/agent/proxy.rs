// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Host-side proxy: libvirt unix socket or vsock listener ↔ optional HTTP bridge.

use crate::agent::handler::RequestHandler;
use crate::agent::transport::vsock_host::{self, VsockGuestStream};
use anyhow::{bail, Context, Result};
use guestkit_agent_protocol::{read_frame, write_frame, JsonRpcResponse};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::unix::io::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct FramedAgentClient {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
}

impl FramedAgentClient {
    fn from_unix(socket_path: &str) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .with_context(|| format!("connect to agent socket {socket_path}"))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
        let reader = stream.try_clone()?;
        Ok(Self {
            reader: Box::new(reader),
            writer: Box::new(stream),
        })
    }

    fn from_vsock_fd(fd: RawFd) -> Result<Self> {
        use std::os::unix::io::FromRawFd;
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let reader = file.try_clone()?;
        Ok(Self {
            reader: Box::new(reader),
            writer: Box::new(file),
        })
    }

    fn call_raw(&mut self, payload: Vec<u8>) -> Result<Vec<u8>> {
        write_frame(&mut self.writer, &payload)?;
        read_frame(&mut self.reader).map_err(|e| anyhow::anyhow!("{e}"))
    }

    fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });
        let frame = self.call_raw(serde_json::to_vec(&req)?)?;
        serde_json::from_slice(&frame).context("parse agent response")
    }
}

enum ProxyBackend {
    Unix(String),
    Vsock(Arc<Mutex<Option<FramedAgentClient>>>),
}

pub async fn run_proxy(
    socket_path: Option<&str>,
    listen: Option<&str>,
    vsock_port: Option<u32>,
) -> Result<()> {
    let backend = match (socket_path, vsock_port) {
        (Some(path), None) => ProxyBackend::Unix(path.to_string()),
        (None, Some(port)) => {
            let slot = Arc::new(Mutex::new(None));
            spawn_vsock_acceptor(port, Arc::clone(&slot));
            ProxyBackend::Vsock(slot)
        }
        (Some(path), Some(port)) => {
            log::info!("agent-proxy: vsock port {port} enabled alongside unix socket {path}");
            let slot = Arc::new(Mutex::new(None));
            spawn_vsock_acceptor(port, Arc::clone(&slot));
            ProxyBackend::Unix(path.to_string())
        }
        (None, None) => bail!("agent-proxy requires --socket or --vsock-port"),
    };

    if let Some(addr) = listen {
        let addr: SocketAddr = addr.parse().context("parse --listen address")?;
        log::info!("GuestKit agent-proxy HTTP listening on {addr}");
        let listener = TcpListener::bind(addr).await?;
        loop {
            let (stream, peer) = listener.accept().await?;
            log::debug!("HTTP connection from {peer}");
            let backend = backend.clone_for_task();
            tokio::spawn(async move {
                if let Err(e) = handle_http(stream, backend).await {
                    log::error!("HTTP handler error: {e}");
                }
            });
        }
    } else if let ProxyBackend::Vsock(slot) = backend {
        log::info!("GuestKit agent-proxy vsock relay (blocking RPC on accepted guests)");
        let listener = vsock_host::listen(vsock_port)?;
        listener.accept_blocking_loop(|stream| serve_vsock_rpc(stream, &slot))?;
        Ok(())
    } else if let ProxyBackend::Unix(path) = backend {
        log::info!("GuestKit agent-proxy relay on {path} (stdin/stdout frames)");
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut agent = UnixStream::connect(&path)?;
            loop {
                let frame_in = read_frame(&mut std::io::stdin())?;
                write_frame(&mut agent, &frame_in)?;
                let frame_out = read_frame(&mut agent)?;
                write_frame(&mut std::io::stdout(), &frame_out)?;
            }
        })
        .await??;
        Ok(())
    } else {
        Ok(())
    }
}

impl ProxyBackend {
    fn clone_for_task(&self) -> Self {
        match self {
            Self::Unix(path) => Self::Unix(path.clone()),
            Self::Vsock(slot) => Self::Vsock(Arc::clone(slot)),
        }
    }

    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        match self {
            Self::Unix(path) => {
                let mut client = FramedAgentClient::from_unix(path)?;
                client.call(method, params)
            }
            Self::Vsock(slot) => {
                let mut guard = slot
                    .lock()
                    .map_err(|_| anyhow::anyhow!("vsock client lock poisoned"))?;
                let client = guard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("no guest connected on vsock yet"))?;
                client.call(method, params)
            }
        }
    }
}

fn spawn_vsock_acceptor(port: u32, slot: Arc<Mutex<Option<FramedAgentClient>>>) {
    std::thread::spawn(move || {
        let listener = match vsock_host::listen(Some(port)) {
            Ok(l) => l,
            Err(e) => {
                log::error!("vsock listen failed: {e}");
                return;
            }
        };
        log::info!("GuestKit agent-proxy vsock listening on port {port}");
        loop {
            match listener.accept() {
                Ok(stream) => register_vsock_guest(stream, &slot),
                Err(e) => log::error!("vsock accept failed: {e}"),
            }
        }
    });
}

fn register_vsock_guest(stream: VsockGuestStream, slot: &Arc<Mutex<Option<FramedAgentClient>>>) {
    let cid = stream.guest_cid;
    match FramedAgentClient::from_vsock_fd(stream.into_raw_fd()) {
        Ok(client) => {
            log::info!("vsock guest connected (cid={cid})");
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(client);
            }
        }
        Err(e) => log::error!("vsock client setup failed: {e}"),
    }
}

fn serve_vsock_rpc(
    stream: VsockGuestStream,
    slot: &Arc<Mutex<Option<FramedAgentClient>>>,
) -> Result<()> {
    register_vsock_guest(stream, slot);
    Ok(())
}

async fn handle_http(mut stream: TcpStream, backend: ProxyBackend) -> Result<()> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return write_http_error(&mut stream, 400, "bad request").await;
    }
    let method = parts[0];
    let path = parts[1];

    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(n);
    let body = &buf[body_start..n];

    let path_only = path.split('?').next().unwrap_or(path);
    let query = path.split('?').nth(1).unwrap_or("");
    let query_target = query
        .split('&')
        .find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next()?;
            let v = kv.next().unwrap_or("");
            if k == "target" {
                Some(v)
            } else {
                None
            }
        })
        .unwrap_or("kvm");

    let rpc_method = match (method, path_only) {
        ("POST", "/rpc") | ("POST", "/api/rpc") => "__passthrough__",
        ("GET", "/guest/health") | ("GET", "/api/guest/health") => "guestkit.getGuestHealth",
        ("GET", "/guest/info") | ("GET", "/api/guest/info") => "guestkit.getGuestInfo",
        ("GET", "/guest/processes") | ("GET", "/api/guest/processes") => "guestkit.getProcesses",
        ("GET", "/guest/systemd") | ("GET", "/api/guest/systemd") => "guestkit.getSystemdUnits",
        ("GET", "/guest/journal") | ("GET", "/api/guest/journal") => "guestkit.getJournalSlice",
        ("GET", "/evidence") | ("GET", "/api/evidence") => "guestkit.getEvidence",
        ("GET", "/doctor") | ("GET", "/api/doctor") => "guestkit.doctor",
        ("GET", "/ping") | ("GET", "/api/ping") => "guestkit.ping",
        ("GET", "/capabilities") | ("GET", "/api/capabilities") => "guestkit.getCapabilities",
        ("POST", "/fix-plan") | ("POST", "/api/fix-plan") => "guestkit.runFixPlan",
        _ => {
            return write_http_error(&mut stream, 404, "not found").await;
        }
    };

    let (call_method, params) = if rpc_method == "__passthrough__" {
        let body_json: serde_json::Value =
            serde_json::from_slice(body).unwrap_or_else(|_| serde_json::json!({}));
        let m = body_json
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if m.is_empty() {
            return write_http_error(&mut stream, 400, "method required").await;
        }
        let p = body_json
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        (m, p)
    } else if rpc_method == "guestkit.runFixPlan" {
        (
            rpc_method.to_string(),
            serde_json::from_slice::<serde_json::Value>(body)
                .unwrap_or_else(|_| serde_json::json!({})),
        )
    } else if rpc_method == "guestkit.doctor" {
        (
            rpc_method.to_string(),
            serde_json::json!({ "target": query_target }),
        )
    } else if rpc_method == "guestkit.getJournalSlice" {
        let unit = query
            .split('&')
            .find_map(|pair| {
                let mut kv = pair.splitn(2, '=');
                let k = kv.next()?;
                let v = kv.next().unwrap_or("");
                if k == "unit" {
                    Some(v)
                } else {
                    None
                }
            })
            .unwrap_or("");
        let boot = query
            .split('&')
            .find_map(|pair| {
                let mut kv = pair.splitn(2, '=');
                let k = kv.next()?;
                let v = kv.next().unwrap_or("");
                if k == "boot" {
                    Some(v)
                } else {
                    None
                }
            })
            .unwrap_or("current");
        let limit = query
            .split('&')
            .find_map(|pair| {
                let mut kv = pair.splitn(2, '=');
                let k = kv.next()?;
                let v = kv.next().unwrap_or("");
                if k == "limit" {
                    v.parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(200);
        (
            rpc_method.to_string(),
            serde_json::json!({ "unit": unit, "boot": boot, "limit": limit }),
        )
    } else {
        (rpc_method.to_string(), serde_json::json!({}))
    };

    let response =
        tokio::task::spawn_blocking(move || backend.call(&call_method, params)).await??;

    let status = if response.get("error").is_some() {
        500
    } else {
        200
    };
    let body = serde_json::to_string(&response)?;
    write_http_json(&mut stream, status, &body).await
}

async fn write_http_json(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn write_http_error(stream: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    let body = serde_json::json!({ "error": message }).to_string();
    write_http_json(stream, status, &body).await
}

/// Local in-process handler for unit tests without a unix socket.
pub fn handle_local_request(bytes: &[u8]) -> JsonRpcResponse {
    let handler = RequestHandler::new();
    handler.handle(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ping() {
        let resp = handle_local_request(br#"{"jsonrpc":"2.0","method":"guestkit.ping","id":1}"#);
        assert!(resp.result.is_some());
    }
}
