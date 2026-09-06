// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestKit agent JSON-RPC protocol (v1).
//!
//! Framing: 4-byte big-endian length prefix + UTF-8 JSON body.

pub mod capabilities;
pub mod error;
pub mod frame;
pub mod guest_info;
pub mod health;
pub mod heartbeat;
pub mod rpc;
pub mod telemetry;

pub use capabilities::{
    AgentCapabilities, PROTOCOL_VERSION, VIRTIO_CHANNEL_GUESTKIT, VIRTIO_CHANNEL_NAME,
    VIRTIO_DEVICE_PATH, VIRTIO_DEVICE_PATH_GUESTKIT,
};
pub use error::{AgentError, RpcErrorCode};
pub use frame::{read_frame, read_line, write_delimited_line, write_frame, write_line};
pub use guest_info::{
    GuestHealthComponents, GuestIdentity, GuestInfo, GuestOsInfo, GuestVirtualizationInfo,
    ServiceHealth, SystemdEvent,
};
pub use health::{
    BootAnalysis, BootUnitTiming, CriticalService, DnsHealth, GuestHealth, HealthLevel, HookResult,
    JournalEntrySummary, JournalSlice, LoggedInUser, LoginState, NetworkHealth, Recommendation,
    RemediationActionResult, RemediationResult, SecurityHealthSummary, ShutdownInhibitor,
    SnapshotReadinessReport, StorageHealth, TimedateHealth,
};
pub use heartbeat::{AgentState, Heartbeat, PressureSummary};
pub use rpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RpcMethod};
pub use telemetry::{MetricStats, PerfSeries, PerfSummary, PerfTier};
