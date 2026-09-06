// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! In-guest agent daemon and host-side proxy.

#[cfg(unix)]
pub mod agent_call;
pub mod audit;
pub mod certificates;
pub mod cli;
pub mod containers;
pub mod customization;
pub mod daemon;
pub mod exec;
pub mod executor;
pub mod executor_ipc;
pub mod file_ops;
pub mod handler;
pub mod heartbeat;
#[cfg(not(target_os = "windows"))]
pub mod inject;
pub mod integrity;
pub mod inventory_cache;
#[cfg(unix)]
pub mod local_client;
pub mod netintel;
pub mod nettest;
pub mod packages;
pub mod policy;
pub mod posture;
#[cfg(not(target_os = "windows"))]
pub mod proxy;
pub mod qga;
#[cfg(unix)]
pub mod qga_client;
pub mod rdp;
pub mod snapshot;
pub mod snapshot_hooks;
pub mod state;
pub mod storage_ops;
pub mod support_bundle;
pub mod telemetry;
pub mod transport;
pub mod update_sign;
pub mod updater;
pub mod users;

#[cfg(unix)]
pub use agent_call::call_agent_socket;
pub use cli::{run_agent, AgentArgs, AgentChannel};
#[cfg(unix)]
pub use cli::{run_agent_call, run_agent_proxy, AgentCallArgs, AgentProxyArgs};
pub use daemon::AgentDaemon;

/// Ping guest agent via libvirt channel unix socket (host-side, Unix only).
#[cfg(unix)]
pub fn ping_agent_socket(socket_path: &str) -> bool {
    use guestkit_agent_protocol::{read_frame, write_frame};
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return false;
    };
    let req = br#"{"execute":"guest-ping"}"#;
    if write_frame(&mut stream, req).is_err() {
        return false;
    }
    let Ok(frame) = read_frame(&mut stream) else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&frame)
        .ok()
        .and_then(|v| v.get("return").cloned())
        .is_some()
}
