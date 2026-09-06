// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! One-shot JSON-RPC call to a guest agent unix socket (host-side).

use anyhow::{Context, Result};
use guestkit_agent_protocol::{read_line, write_line, JsonRpcResponse};
use serde_json::Value;
use std::io::BufReader;
use std::os::unix::net::UnixStream;

/// Invoke one GuestKit JSON-RPC method on the agent socket and return the parsed response.
///
/// Libvirt/KubeVirt expose the guest agent channel as a unix socket with newline-delimited
/// JSON frames (same as QGA over virtio-serial), not length-prefixed JSON-RPC.
pub fn call_agent_socket(socket_path: &str, method: &str, params: Value) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connect to agent socket {socket_path}"))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });
    let payload = serde_json::to_vec(&req)?;
    write_line(&mut stream, &payload).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut reader = BufReader::new(stream);
    let frame = read_line(&mut reader).map_err(|e| anyhow::anyhow!("{e}"))?;
    let resp: JsonRpcResponse = serde_json::from_slice(&frame).context("parse agent response")?;
    if let Some(err) = resp.error {
        anyhow::bail!("agent RPC error {}: {}", err.code, err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_request_payload() {
        let params = serde_json::json!({ "target": "kvm" });
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "guestkit.doctor",
            "params": params,
            "id": 1
        });
        assert_eq!(req["method"], "guestkit.doctor");
    }
}
