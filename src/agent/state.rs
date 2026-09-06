// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Shared mutable agent runtime state.
//!
//! One [`AgentRuntime`] exists per daemon process. It is threaded through
//! the request handler and channel loops, and exposed as a process global
//! so long-lived modules that predate it (updater, snapshot hooks) can flip
//! flags without signature churn.

use crate::agent::transport::SharedWriter;
use guestkit_agent_protocol::heartbeat::AgentState;
use guestkit_agent_protocol::{Heartbeat, JsonRpcResponse};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

/// Responses cached for idempotent replay are kept this long.
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(600);

/// Nonce LRU + idempotency response cache for mutating requests.
#[derive(Default)]
pub struct ReplayCache {
    nonce_order: VecDeque<String>,
    nonces: HashSet<String>,
    idempotent: HashMap<String, (Instant, JsonRpcResponse)>,
}

/// A live transport channel registered with the daemon.
pub struct ChannelHandle {
    /// Human-readable channel identity (device path or transport kind).
    pub name: String,
    /// False on the QGA-shared channel: a libvirt host that speaks plain
    /// QGA would choke on interleaved notification frames, so subscribing
    /// there is refused and push stays impossible by construction.
    pub push_capable: bool,
    pub subscribed: AtomicBool,
    pub writer: SharedWriter,
}

pub struct AgentRuntime {
    pub started_at: Instant,
    pub heartbeat_seq: AtomicU64,
    /// Set by the updater while staging/applying a self-update.
    pub updating: AtomicBool,
    /// Set by snapshot hooks around fsfreeze (qga's own freeze path keeps
    /// its state in `qga::fs_frozen()`; heartbeat checks both).
    pub fs_frozen_hint: AtomicBool,
    pub telemetry: Arc<crate::agent::telemetry::TelemetryStore>,
    state: RwLock<AgentState>,
    channels: Mutex<Vec<Arc<ChannelHandle>>>,
    last_heartbeat: Mutex<Option<Heartbeat>>,
    replay: Mutex<ReplayCache>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            heartbeat_seq: AtomicU64::new(0),
            updating: AtomicBool::new(false),
            fs_frozen_hint: AtomicBool::new(false),
            telemetry: Arc::new(crate::agent::telemetry::TelemetryStore::default()),
            state: RwLock::new(AgentState::Starting),
            channels: Mutex::new(Vec::new()),
            last_heartbeat: Mutex::new(None),
            replay: Mutex::new(ReplayCache::default()),
        }
    }
}

static GLOBAL: OnceLock<Arc<AgentRuntime>> = OnceLock::new();

impl AgentRuntime {
    /// Process-wide runtime. First caller initializes it; the daemon calls
    /// this during startup so every later access sees the same instance.
    pub fn global() -> Arc<AgentRuntime> {
        Arc::clone(GLOBAL.get_or_init(|| Arc::new(AgentRuntime::default())))
    }

    pub fn state(&self) -> AgentState {
        *self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_state(&self, s: AgentState) {
        *self.state.write().unwrap_or_else(|e| e.into_inner()) = s;
    }

    pub fn register_channel(&self, handle: Arc<ChannelHandle>) {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(handle);
    }

    /// Drop a channel whose transport died so heartbeat stops pushing to it.
    pub fn unregister_channel(&self, handle: &Arc<ChannelHandle>) {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|c| !Arc::ptr_eq(c, handle));
    }

    pub fn channels(&self) -> Vec<Arc<ChannelHandle>> {
        self.channels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn next_heartbeat_seq(&self) -> u64 {
        self.heartbeat_seq.fetch_add(1, Ordering::Relaxed)
    }

    pub fn store_heartbeat(&self, hb: Heartbeat) {
        *self
            .last_heartbeat
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(hb);
    }

    pub fn last_heartbeat(&self) -> Option<Heartbeat> {
        self.last_heartbeat
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn fs_frozen(&self) -> bool {
        self.fs_frozen_hint.load(Ordering::Relaxed) || crate::agent::qga::fs_frozen()
    }

    /// Record a nonce; false when it was already seen (replay). Oldest
    /// entries fall out once `cap` distinct nonces are tracked.
    pub fn nonce_fresh(&self, nonce: &str, cap: usize) -> bool {
        let mut cache = self.replay.lock().unwrap_or_else(|e| e.into_inner());
        if cache.nonces.contains(nonce) {
            return false;
        }
        cache.nonces.insert(nonce.to_string());
        cache.nonce_order.push_back(nonce.to_string());
        while cache.nonce_order.len() > cap.max(1) {
            if let Some(old) = cache.nonce_order.pop_front() {
                cache.nonces.remove(&old);
            }
        }
        true
    }

    pub fn idempotent_get(&self, key: &str) -> Option<JsonRpcResponse> {
        let mut cache = self.replay.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .idempotent
            .retain(|_, (at, _)| at.elapsed() < IDEMPOTENCY_TTL);
        cache.idempotent.get(key).map(|(_, resp)| resp.clone())
    }

    pub fn idempotent_store(&self, key: &str, resp: JsonRpcResponse) {
        let mut cache = self.replay.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .idempotent
            .insert(key.to_string(), (Instant::now(), resp));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transitions() {
        let rt = AgentRuntime::default();
        assert_eq!(rt.state(), AgentState::Starting);
        rt.set_state(AgentState::Healthy);
        assert_eq!(rt.state(), AgentState::Healthy);
    }

    #[test]
    fn heartbeat_seq_monotonic() {
        let rt = AgentRuntime::default();
        assert_eq!(rt.next_heartbeat_seq(), 0);
        assert_eq!(rt.next_heartbeat_seq(), 1);
    }
}
