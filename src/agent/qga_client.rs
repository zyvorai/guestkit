// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Host-side raw QEMU guest-agent client.
//!
//! Talks the QGA wire format (one JSON object per line) over a unix socket.
//! This is the GuestKit replacement for `virsh qemu-agent-command`.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Well-known QGA channel socket locations used by libvirt and KubeVirt
/// virt-launcher. First existing socket wins.
pub const QGA_SOCKET_CANDIDATES: &[&str] = &[
    "/var/run/kubevirt-private/libvirt/qemu/channel/target/org.qemu.guest_agent.0",
    "/var/run/libvirt/qemu/channel/target/org.qemu.guest_agent.0",
    "/var/lib/libvirt/qemu/channel/target/org.qemu.guest_agent.0",
    "/var/run/kubevirt/guest-agent.sock",
];

/// Discover a QGA unix socket on this host (or inside virt-launcher).
pub fn discover_qga_socket(extra: &[PathBuf]) -> Option<PathBuf> {
    for p in extra {
        if is_socket(p) {
            return Some(p.clone());
        }
    }
    for raw in QGA_SOCKET_CANDIDATES {
        let p = PathBuf::from(raw);
        if is_socket(&p) {
            return Some(p);
        }
    }
    search_tree(Path::new("/var/run/kubevirt-private"))
        .or_else(|| search_tree(Path::new("/var/run/libvirt")))
        .or_else(|| search_tree(Path::new("/var/lib/libvirt/qemu/channel")))
}

fn is_socket(path: &Path) -> bool {
    path.exists()
        && std::fs::metadata(path)
            .map(|m| m.file_type().is_socket())
            .unwrap_or(false)
}

fn search_tree(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = search_tree(&path) {
                return Some(found);
            }
        } else {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if (name.contains("guest_agent") || name.contains("org.qemu.guest_agent"))
                && is_socket(&path)
            {
                return Some(path);
            }
        }
    }
    None
}

/// Build a QGA request body.
pub fn qga_request(execute: &str, arguments: Option<Value>) -> Value {
    match arguments {
        Some(args) if !args.is_null() => json!({
            "execute": execute,
            "arguments": args
        }),
        _ => json!({ "execute": execute }),
    }
}

/// Send one QGA command to `socket_path` and parse the JSON reply.
pub fn call_qga_socket(
    socket_path: &str,
    execute: &str,
    arguments: Option<Value>,
    timeout: Duration,
) -> Result<Value> {
    let body = qga_request(execute, arguments);
    call_qga_socket_raw(socket_path, &serde_json::to_string(&body)?, timeout)
}

/// Send a raw JSON line to the QGA socket and parse the JSON reply.
pub fn call_qga_socket_raw(socket_path: &str, json_line: &str, timeout: Duration) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connect to QGA socket {socket_path}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let mut payload = json_line.as_bytes().to_vec();
    if !payload.ends_with(b"\n") {
        payload.push(b'\n');
    }
    stream.write_all(&payload).context("write QGA request")?;
    stream.flush().ok();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).context("read QGA response")?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        anyhow::bail!("empty response from QGA socket {socket_path}");
    }
    serde_json::from_str(trimmed).with_context(|| format!("parse QGA response {trimmed}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_without_arguments() {
        let v = qga_request("guest-ping", None);
        assert_eq!(v["execute"], "guest-ping");
        assert!(v.get("arguments").is_none());
    }

    #[test]
    fn request_with_arguments() {
        let v = qga_request("guest-exec", Some(json!({"path": "/bin/true"})));
        assert_eq!(v["execute"], "guest-exec");
        assert_eq!(v["arguments"]["path"], "/bin/true");
    }

    #[test]
    fn request_ignores_null_arguments() {
        let v = qga_request("guest-ping", Some(Value::Null));
        assert!(v.get("arguments").is_none());
    }
}
