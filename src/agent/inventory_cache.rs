// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline + online inventory correlation cache (spec §31).
//!
//! The running agent periodically writes a compact snapshot of what only a
//! live agent can see — running services, live network, users, containers,
//! last heartbeat — to a well-known on-disk location. When the VM is later
//! powered off, GuestKit's offline disk-inspection engine reads this cache
//! from the mounted image, so an offline assessment can reason about the
//! guest's last-known running state, not just its static disk contents.
//!
//! The file carries a self-integrity SHA-256 over its payload so offline
//! readers can detect truncation/tampering. This is the on-agent half of
//! the "combined assessment" (offline disk + live cache + hypervisor).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Canonical cache location (spec §31). Overridable for tests/e2e.
pub fn cache_path() -> PathBuf {
    if let Ok(dir) = std::env::var("GUESTKIT_STATE_DIR") {
        return PathBuf::from(dir).join("inventory.snapshot");
    }
    if cfg!(windows) {
        PathBuf::from("C:\\ProgramData\\Zyvor\\GuestKit\\inventory.snapshot")
    } else {
        PathBuf::from("/var/lib/guestkit/inventory.snapshot")
    }
}

/// Path of the cache relative to a guest filesystem root (offline read).
pub const CACHE_RELATIVE: &str = "var/lib/guestkit/inventory.snapshot";
pub const CACHE_RELATIVE_WINDOWS: &str = "ProgramData/Zyvor/GuestKit/inventory.snapshot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineInventoryCache {
    pub schema: u32,
    pub written_at: String,
    pub agent_version: String,
    pub hostname: String,
    pub boot_id: String,
    /// Integrity digest over the `payload` object (hex SHA-256).
    pub integrity_sha256: String,
    /// The live-only facts: heartbeat, running services, live network,
    /// users, containers.
    pub payload: Value,
}

const CACHE_SCHEMA: u32 = 1;

fn digest(payload: &Value) -> String {
    use sha2::{Digest, Sha256};
    let canonical = serde_json::to_vec(payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    hex::encode(hasher.finalize())
}

/// Build the live-only payload from current agent state.
fn build_payload(rt: &crate::agent::state::AgentRuntime) -> (Value, String, String) {
    let heartbeat = rt
        .last_heartbeat()
        .unwrap_or_else(|| crate::agent::heartbeat::build_heartbeat(rt));
    let hostname = sysinfo::System::host_name().unwrap_or_default();
    let boot_id = heartbeat.boot_id.clone();

    // Cheap live facts; each collector already tolerates being unavailable.
    let failed_units = &heartbeat.critical_services_failed;
    let users = crate::agent::users::inventory();
    let containers = crate::agent::containers::inventory();
    let netintel = crate::agent::netintel::collect();

    let payload = json!({
        "heartbeat": heartbeat,
        "failed_services": failed_units,
        "users": {
            "count": users.get("user_count").cloned().unwrap_or(json!(0)),
            "sudoers": users.get("sudoers").cloned().unwrap_or(json!([])),
        },
        "containers": {
            "count": containers.get("container_count").cloned().unwrap_or(json!(0)),
            "runtimes": containers.get("runtimes").cloned().unwrap_or(json!([])),
            "kubernetes": containers.get("kubernetes").cloned().unwrap_or(Value::Null),
        },
        "network": {
            "listening": netintel.total_listening,
            "established": netintel.total_established,
        },
    });
    (payload, hostname, boot_id)
}

/// Write the inventory cache atomically. Best-effort; logs on failure.
pub fn write_cache(rt: &crate::agent::state::AgentRuntime) -> anyhow::Result<()> {
    let (payload, hostname, boot_id) = build_payload(rt);
    let cache = OnlineInventoryCache {
        schema: CACHE_SCHEMA,
        written_at: chrono::Utc::now().to_rfc3339(),
        agent_version: crate::VERSION.to_string(),
        hostname,
        boot_id,
        integrity_sha256: digest(&payload),
        payload,
    };
    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&cache)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Read and integrity-verify a cache file from a mounted guest filesystem
/// root (offline path). Returns None if absent or corrupt.
pub fn read_cache_from_root(root: &Path) -> Option<OnlineInventoryCache> {
    for rel in [CACHE_RELATIVE, CACHE_RELATIVE_WINDOWS] {
        let path = root.join(rel);
        if let Some(cache) = read_cache_file(&path) {
            return Some(cache);
        }
    }
    None
}

fn read_cache_file(path: &Path) -> Option<OnlineInventoryCache> {
    let bytes = std::fs::read(path).ok()?;
    let cache: OnlineInventoryCache = serde_json::from_slice(&bytes).ok()?;
    // Reject a tampered/truncated payload.
    if digest(&cache.payload) != cache.integrity_sha256 {
        log::warn!(
            "inventory cache at {} failed integrity check",
            path.display()
        );
        return None;
    }
    Some(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_round_trip() {
        let payload = json!({"a": 1, "b": [2, 3]});
        let d = digest(&payload);
        let cache = OnlineInventoryCache {
            schema: CACHE_SCHEMA,
            written_at: "2026-07-19T00:00:00Z".into(),
            agent_version: "test".into(),
            hostname: "host".into(),
            boot_id: "boot".into(),
            integrity_sha256: d,
            payload,
        };
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("inventory.snapshot");
        std::fs::write(&path, serde_json::to_vec_pretty(&cache).unwrap()).unwrap();
        let read = read_cache_file(&path).unwrap();
        assert_eq!(read.hostname, "host");
    }

    #[test]
    fn tampered_payload_rejected() {
        let payload = json!({"a": 1});
        let cache = OnlineInventoryCache {
            schema: CACHE_SCHEMA,
            written_at: "t".into(),
            agent_version: "test".into(),
            hostname: "host".into(),
            boot_id: "boot".into(),
            integrity_sha256: "deadbeef".into(), // wrong digest
            payload,
        };
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("inventory.snapshot");
        std::fs::write(&path, serde_json::to_vec_pretty(&cache).unwrap()).unwrap();
        assert!(read_cache_file(&path).is_none());
    }

    #[test]
    fn read_from_root_finds_linux_path() {
        let tmp = tempfile::tempdir().unwrap();
        let full = tmp.path().join(CACHE_RELATIVE);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        let payload = json!({"x": 1});
        let cache = OnlineInventoryCache {
            schema: 1,
            written_at: "t".into(),
            agent_version: "v".into(),
            hostname: "h".into(),
            boot_id: "b".into(),
            integrity_sha256: digest(&payload),
            payload,
        };
        std::fs::write(&full, serde_json::to_vec(&cache).unwrap()).unwrap();
        assert!(read_cache_from_root(tmp.path()).is_some());
    }
}
