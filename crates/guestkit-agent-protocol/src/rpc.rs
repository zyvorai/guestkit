// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! JSON-RPC 2.0 message types.

use crate::error::{AgentError, RpcErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parsed RPC method identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcMethod {
    Ping,
    GetVersion,
    GetCapabilities,
    GetEvidence,
    GetStatus,
    Doctor,
    MigrateScore,
    GetMetrics,
    GetFilesystem,
    Exec,
    EnableRdp,
    DisableRdp,
    RunFixPlan,
    RunFixPlanRollback,
    GetGuestHealth,
    GetSystemdUnits,
    GetFailedUnits,
    GetBootAnalysis,
    GetJournalSlice,
    GetLoginState,
    GetDnsState,
    GetTimedateState,
    GetSnapshotReadiness,
    FreezeFilesystem,
    ThawFilesystem,
    RestartUnit,
    ExecuteRemediationPlan,
    CollectSupportBundle,
    GetGuestInfo,
    GetSystemdUnit,
    GetSystemdEvents,
    GetProcesses,
    // Protocol 1.3: agent health + events
    GetAgentHealth,
    SubscribeEvents,
    UnsubscribeEvents,
    // Protocol 1.3: performance telemetry
    GetCpuStats,
    GetMemoryStats,
    GetPerformanceSummary,
    GetPerformanceHistory,
    // Protocol 1.3: service / process / network / storage / file / time / power
    StartUnit,
    StopUnit,
    GetProcess,
    NetworkTest,
    NetworkConnections,
    SecurityPosture,
    StorageRescan,
    StorageTrim,
    StorageExpand,
    FileRead,
    FileWrite,
    FileStat,
    FileList,
    FileChecksum,
    SetTime,
    TimeSyncNow,
    Reboot,
    Shutdown,
    // Protocol 1.3: orchestrated snapshots
    SnapshotPrepare,
    SnapshotComplete,
    // Protocol 1.3: container awareness + offline cache
    ContainersInventory,
    InventoryCacheWrite,
    IntegrityBaseline,
    IntegrityCheck,
    // Protocol 1.3: enterprise automation (Phase 6)
    PackagesInventory,
    PackagesUpdates,
    PackagesInstall,
    CertificatesInventory,
    UsersInventory,
    SetHostname,
    SetTimezone,
    SetDns,
    // Protocol 1.3: migration assurance
    MigrationAssess,
    MigrationPlan,
    MigrationRepair,
    MigrationPreCheck,
    MigrationCutoverEnter,
    MigrationCutoverExit,
    MigrationValidate,
    BaselineCapture,
    BaselineDiff,
    Unknown(String),
}

impl RpcMethod {
    pub fn parse(name: &str) -> Self {
        use crate::capabilities::*;
        match name {
            METHOD_PING => Self::Ping,
            METHOD_GET_VERSION => Self::GetVersion,
            METHOD_GET_CAPABILITIES => Self::GetCapabilities,
            METHOD_GET_EVIDENCE => Self::GetEvidence,
            METHOD_GET_STATUS => Self::GetStatus,
            METHOD_DOCTOR => Self::Doctor,
            METHOD_MIGRATE_SCORE => Self::MigrateScore,
            METHOD_GET_METRICS => Self::GetMetrics,
            METHOD_GET_FILESYSTEM => Self::GetFilesystem,
            METHOD_EXEC => Self::Exec,
            METHOD_ENABLE_RDP => Self::EnableRdp,
            METHOD_DISABLE_RDP => Self::DisableRdp,
            METHOD_RUN_FIX_PLAN => Self::RunFixPlan,
            METHOD_RUN_FIX_PLAN_ROLLBACK => Self::RunFixPlanRollback,
            METHOD_GET_GUEST_HEALTH => Self::GetGuestHealth,
            METHOD_GET_SYSTEMD_UNITS => Self::GetSystemdUnits,
            METHOD_GET_FAILED_UNITS => Self::GetFailedUnits,
            METHOD_GET_BOOT_ANALYSIS => Self::GetBootAnalysis,
            METHOD_GET_JOURNAL_SLICE => Self::GetJournalSlice,
            METHOD_GET_LOGIN_STATE => Self::GetLoginState,
            METHOD_GET_DNS_STATE => Self::GetDnsState,
            METHOD_GET_TIMEDATE_STATE => Self::GetTimedateState,
            METHOD_GET_SNAPSHOT_READINESS => Self::GetSnapshotReadiness,
            METHOD_FREEZE_FILESYSTEM => Self::FreezeFilesystem,
            METHOD_THAW_FILESYSTEM => Self::ThawFilesystem,
            METHOD_RESTART_UNIT => Self::RestartUnit,
            METHOD_EXECUTE_REMEDIATION_PLAN => Self::ExecuteRemediationPlan,
            METHOD_COLLECT_SUPPORT_BUNDLE => Self::CollectSupportBundle,
            METHOD_GET_GUEST_INFO => Self::GetGuestInfo,
            METHOD_GET_SYSTEMD_UNIT => Self::GetSystemdUnit,
            METHOD_GET_SYSTEMD_EVENTS => Self::GetSystemdEvents,
            METHOD_GET_PROCESSES => Self::GetProcesses,
            METHOD_GET_AGENT_HEALTH => Self::GetAgentHealth,
            METHOD_SUBSCRIBE_EVENTS => Self::SubscribeEvents,
            METHOD_UNSUBSCRIBE_EVENTS => Self::UnsubscribeEvents,
            METHOD_GET_CPU_STATS => Self::GetCpuStats,
            METHOD_GET_MEMORY_STATS => Self::GetMemoryStats,
            METHOD_GET_PERFORMANCE_SUMMARY => Self::GetPerformanceSummary,
            METHOD_GET_PERFORMANCE_HISTORY => Self::GetPerformanceHistory,
            METHOD_START_UNIT => Self::StartUnit,
            METHOD_STOP_UNIT => Self::StopUnit,
            METHOD_GET_PROCESS => Self::GetProcess,
            METHOD_NETWORK_TEST => Self::NetworkTest,
            METHOD_NETWORK_CONNECTIONS => Self::NetworkConnections,
            METHOD_SECURITY_POSTURE => Self::SecurityPosture,
            METHOD_STORAGE_RESCAN => Self::StorageRescan,
            METHOD_STORAGE_TRIM => Self::StorageTrim,
            METHOD_STORAGE_EXPAND => Self::StorageExpand,
            METHOD_FILE_READ => Self::FileRead,
            METHOD_FILE_WRITE => Self::FileWrite,
            METHOD_FILE_STAT => Self::FileStat,
            METHOD_FILE_LIST => Self::FileList,
            METHOD_FILE_CHECKSUM => Self::FileChecksum,
            METHOD_SET_TIME => Self::SetTime,
            METHOD_TIME_SYNC_NOW => Self::TimeSyncNow,
            METHOD_REBOOT => Self::Reboot,
            METHOD_SHUTDOWN => Self::Shutdown,
            METHOD_SNAPSHOT_PREPARE => Self::SnapshotPrepare,
            METHOD_SNAPSHOT_COMPLETE => Self::SnapshotComplete,
            METHOD_CONTAINERS_INVENTORY => Self::ContainersInventory,
            METHOD_INTEGRITY_BASELINE => Self::IntegrityBaseline,
            METHOD_INTEGRITY_CHECK => Self::IntegrityCheck,
            METHOD_INVENTORY_CACHE_WRITE => Self::InventoryCacheWrite,
            METHOD_PACKAGES_INVENTORY => Self::PackagesInventory,
            METHOD_PACKAGES_UPDATES => Self::PackagesUpdates,
            METHOD_PACKAGES_INSTALL => Self::PackagesInstall,
            METHOD_CERTIFICATES_INVENTORY => Self::CertificatesInventory,
            METHOD_USERS_INVENTORY => Self::UsersInventory,
            METHOD_SET_HOSTNAME => Self::SetHostname,
            METHOD_SET_TIMEZONE => Self::SetTimezone,
            METHOD_SET_DNS => Self::SetDns,
            METHOD_MIGRATION_ASSESS => Self::MigrationAssess,
            METHOD_MIGRATION_PLAN => Self::MigrationPlan,
            METHOD_MIGRATION_REPAIR => Self::MigrationRepair,
            METHOD_MIGRATION_PRE_CHECK => Self::MigrationPreCheck,
            METHOD_MIGRATION_CUTOVER_ENTER => Self::MigrationCutoverEnter,
            METHOD_MIGRATION_CUTOVER_EXIT => Self::MigrationCutoverExit,
            METHOD_MIGRATION_VALIDATE => Self::MigrationValidate,
            METHOD_BASELINE_CAPTURE => Self::BaselineCapture,
            METHOD_BASELINE_DIFF => Self::BaselineDiff,
            other => Self::parse_alias(other),
        }
    }

    /// Spec-style dotted operation names accepted as aliases for the
    /// canonical `guestkit.*` camelCase methods (e.g. host tooling written
    /// against the product spec's `service.restart` naming).
    fn parse_alias(name: &str) -> Self {
        match name {
            "agent.ping" => Self::Ping,
            "agent.info" => Self::GetVersion,
            "agent.capabilities" => Self::GetCapabilities,
            "agent.health" => Self::GetAgentHealth,
            "inventory.os" | "inventory.full" => Self::GetEvidence,
            "cpu.stats" => Self::GetCpuStats,
            "memory.stats" => Self::GetMemoryStats,
            "performance.summary" => Self::GetPerformanceSummary,
            "performance.history" => Self::GetPerformanceHistory,
            "process.list" => Self::GetProcesses,
            "process.inspect" => Self::GetProcess,
            "service.list" => Self::GetSystemdUnits,
            "service.inspect" => Self::GetSystemdUnit,
            "service.start" => Self::StartUnit,
            "service.stop" => Self::StopUnit,
            "service.restart" => Self::RestartUnit,
            "network.test" => Self::NetworkTest,
            "network.connections" | "network.listeners" => Self::NetworkConnections,
            "security.posture" => Self::SecurityPosture,
            "containers.inventory" | "containers.list" => Self::ContainersInventory,
            "integrity.baseline" | "security.integrity.baseline" => Self::IntegrityBaseline,
            "integrity.check" | "security.integrity.check" => Self::IntegrityCheck,
            "inventory.cache" => Self::InventoryCacheWrite,
            "packages.inventory" | "packages.list" => Self::PackagesInventory,
            "packages.updates" => Self::PackagesUpdates,
            "packages.install" => Self::PackagesInstall,
            "certificates.inventory" | "certificates.list" => Self::CertificatesInventory,
            "users.inventory" | "users.list" => Self::UsersInventory,
            "customization.hostname" => Self::SetHostname,
            "customization.timezone" => Self::SetTimezone,
            "customization.dns" => Self::SetDns,
            "storage.rescan" => Self::StorageRescan,
            "storage.trim" => Self::StorageTrim,
            "storage.expand" => Self::StorageExpand,
            "file.read" => Self::FileRead,
            "file.write" => Self::FileWrite,
            "file.stat" => Self::FileStat,
            "file.list" => Self::FileList,
            "file.checksum" => Self::FileChecksum,
            "system.time.set" => Self::SetTime,
            "system.time.sync" => Self::TimeSyncNow,
            "system.reboot" => Self::Reboot,
            "system.shutdown" => Self::Shutdown,
            "snapshot.prepare" | "snapshot.preflight" => Self::SnapshotPrepare,
            "snapshot.complete" => Self::SnapshotComplete,
            "snapshot.freeze" => Self::FreezeFilesystem,
            "snapshot.thaw" => Self::ThawFilesystem,
            "migration.assess" => Self::MigrationAssess,
            "migration.plan" => Self::MigrationPlan,
            "migration.validate" => Self::MigrationValidate,
            "support.collect" => Self::CollectSupportBundle,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// True for methods that change guest state (as opposed to read-only
    /// inspection). The handler's security choke point applies policy
    /// authorization, request expiry, and replay protection to these.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::Exec
                | Self::EnableRdp
                | Self::DisableRdp
                | Self::RunFixPlan
                | Self::RunFixPlanRollback
                | Self::FreezeFilesystem
                | Self::ThawFilesystem
                | Self::RestartUnit
                | Self::ExecuteRemediationPlan
                | Self::StartUnit
                | Self::StopUnit
                | Self::StorageRescan
                | Self::StorageTrim
                | Self::StorageExpand
                | Self::FileWrite
                | Self::SetTime
                | Self::TimeSyncNow
                | Self::Reboot
                | Self::Shutdown
                | Self::MigrationRepair
                | Self::SnapshotPrepare
                | Self::SnapshotComplete
                | Self::PackagesInstall
                | Self::SetHostname
                | Self::SetTimezone
                | Self::SetDns
                | Self::InventoryCacheWrite
                | Self::IntegrityBaseline
                | Self::MigrationCutoverEnter
                | Self::MigrationCutoverExit
                | Self::SubscribeEvents
                | Self::UnsubscribeEvents
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Option<Value>,
    /// Request issue time (RFC 3339). With `ttl_ms`, bounds the validity
    /// window; requests past `ts + ttl_ms` are rejected with `RequestExpired`
    /// when the policy requires expiry. Absent on legacy (≤1.2) clients.
    #[serde(default)]
    pub ts: Option<String>,
    /// Validity window in milliseconds from `ts`.
    #[serde(default)]
    pub ttl_ms: Option<u64>,
    /// Single-use token for replay protection on mutating methods.
    #[serde(default)]
    pub nonce: Option<String>,
    /// Dedupe key: a mutating request retried with the same key returns the
    /// cached response instead of re-executing.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Server-initiated push message (JSON-RPC 2.0 notification: no `id`,
/// no response expected). Used for heartbeat/event streaming on channels
/// that have subscribed via `guestkit.subscribeEvents`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
    pub id: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, code: RpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcErrorObject {
                code: code.as_i32(),
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    pub fn from_agent_error(id: Option<Value>, err: AgentError) -> Self {
        Self::error(id, err.rpc_code(), err.message())
    }
}

impl JsonRpcRequest {
    pub fn parse(bytes: &[u8]) -> Result<Self, AgentError> {
        serde_json::from_slice(bytes).map_err(|e| AgentError::Parse(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), AgentError> {
        if self.jsonrpc != "2.0" {
            return Err(AgentError::InvalidRequest(format!(
                "unsupported jsonrpc version: {}",
                self.jsonrpc
            )));
        }
        if self.method.is_empty() {
            return Err(AgentError::InvalidRequest("missing method".into()));
        }
        Ok(())
    }

    pub fn method(&self) -> RpcMethod {
        RpcMethod::parse(&self.method)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ping_request() {
        let req =
            JsonRpcRequest::parse(br#"{"jsonrpc":"2.0","method":"guestkit.ping","id":1}"#).unwrap();
        assert_eq!(req.method(), RpcMethod::Ping);
    }

    #[test]
    fn legacy_request_without_envelope_fields_parses() {
        let req = JsonRpcRequest::parse(
            br#"{"jsonrpc":"2.0","method":"guestkit.restartUnit","params":{"unit":"nginx.service"},"id":7}"#,
        )
        .unwrap();
        assert_eq!(req.method(), RpcMethod::RestartUnit);
        assert!(req.ts.is_none());
        assert!(req.ttl_ms.is_none());
        assert!(req.nonce.is_none());
        assert!(req.idempotency_key.is_none());
    }

    #[test]
    fn envelope_fields_round_trip() {
        let req = JsonRpcRequest::parse(
            br#"{"jsonrpc":"2.0","method":"guestkit.reboot","id":1,
                 "ts":"2026-07-19T12:00:00Z","ttl_ms":30000,
                 "nonce":"n-1","idempotency_key":"k-1"}"#,
        )
        .unwrap();
        assert_eq!(req.ts.as_deref(), Some("2026-07-19T12:00:00Z"));
        assert_eq!(req.ttl_ms, Some(30000));
        assert_eq!(req.nonce.as_deref(), Some("n-1"));
        assert_eq!(req.idempotency_key.as_deref(), Some("k-1"));
    }

    #[test]
    fn dotted_aliases_resolve() {
        assert_eq!(RpcMethod::parse("agent.ping"), RpcMethod::Ping);
        assert_eq!(RpcMethod::parse("service.restart"), RpcMethod::RestartUnit);
        assert_eq!(RpcMethod::parse("agent.health"), RpcMethod::GetAgentHealth);
        assert_eq!(
            RpcMethod::parse("migration.assess"),
            RpcMethod::MigrationAssess
        );
        assert_eq!(
            RpcMethod::parse("no.such.method"),
            RpcMethod::Unknown("no.such.method".to_string())
        );
    }

    #[test]
    fn canonical_names_still_resolve() {
        use crate::capabilities::*;
        assert_eq!(
            RpcMethod::parse(METHOD_MIGRATION_REPAIR),
            RpcMethod::MigrationRepair
        );
        assert_eq!(
            RpcMethod::parse(METHOD_GET_PERFORMANCE_HISTORY),
            RpcMethod::GetPerformanceHistory
        );
    }

    #[test]
    fn notification_has_no_id() {
        let n = JsonRpcNotification::new(
            crate::capabilities::NOTIFICATION_HEARTBEAT,
            serde_json::json!({"seq": 1}),
        );
        let json = serde_json::to_string(&n).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(json.contains("guestkit.event.heartbeat"));
    }

    #[test]
    fn mutating_classification() {
        assert!(RpcMethod::Reboot.is_mutating());
        assert!(RpcMethod::MigrationRepair.is_mutating());
        assert!(!RpcMethod::Ping.is_mutating());
        assert!(!RpcMethod::GetEvidence.is_mutating());
        assert!(!RpcMethod::MigrationAssess.is_mutating());
    }
}
