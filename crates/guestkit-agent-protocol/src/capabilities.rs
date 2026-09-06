// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Agent capability negotiation.

use serde::{Deserialize, Serialize};

/// Protocol version string.
pub const PROTOCOL_VERSION: &str = "1.3";

/// Virtio-serial channel name — same as QEMU guest agent (libvirt `qemu-agent-command`).
pub const VIRTIO_CHANNEL_NAME: &str = "org.qemu.guest_agent.0";

/// Dedicated GuestKit channel for plain libvirt/QEMU hosts. Server-push
/// notifications are only ever emitted here (or on other subscribed,
/// non-QGA channels) — never unsolicited on the shared QGA channel.
pub const VIRTIO_CHANNEL_GUESTKIT: &str = "org.zyvor.guestkit.0";

/// Legacy GuestKit-only channel (deprecated; use [`VIRTIO_CHANNEL_NAME`]).
pub const VIRTIO_CHANNEL_LEGACY: &str = "com.zyvor.guestkit.0";

/// Default guest device path for the virtio channel.
pub const VIRTIO_DEVICE_PATH: &str = "/dev/virtio-ports/org.qemu.guest_agent.0";

/// Guest device path for the dedicated GuestKit channel.
pub const VIRTIO_DEVICE_PATH_GUESTKIT: &str = "/dev/virtio-ports/org.zyvor.guestkit.0";

/// Known RPC methods exposed by the agent.
pub const METHOD_PING: &str = "guestkit.ping";
pub const METHOD_GET_VERSION: &str = "guestkit.getVersion";
pub const METHOD_GET_CAPABILITIES: &str = "guestkit.getCapabilities";
pub const METHOD_GET_EVIDENCE: &str = "guestkit.getEvidence";
pub const METHOD_DOCTOR: &str = "guestkit.doctor";
pub const METHOD_MIGRATE_SCORE: &str = "guestkit.migrateScore";
pub const METHOD_RUN_FIX_PLAN: &str = "guestkit.runFixPlan";
pub const METHOD_RUN_FIX_PLAN_ROLLBACK: &str = "guestkit.runFixPlanRollback";
pub const METHOD_GET_STATUS: &str = "guestkit.getStatus";
pub const METHOD_GET_METRICS: &str = "guestkit.getMetrics";
pub const METHOD_GET_FILESYSTEM: &str = "guestkit.getFilesystem";
pub const METHOD_EXEC: &str = "guestkit.exec";
pub const METHOD_ENABLE_RDP: &str = "guestkit.enableRdp";
pub const METHOD_DISABLE_RDP: &str = "guestkit.disableRdp";
pub const METHOD_GET_GUEST_HEALTH: &str = "guestkit.getGuestHealth";
pub const METHOD_GET_SYSTEMD_UNITS: &str = "guestkit.getSystemdUnits";
pub const METHOD_GET_FAILED_UNITS: &str = "guestkit.getFailedUnits";
pub const METHOD_GET_BOOT_ANALYSIS: &str = "guestkit.getBootAnalysis";
pub const METHOD_GET_JOURNAL_SLICE: &str = "guestkit.getJournalSlice";
pub const METHOD_GET_LOGIN_STATE: &str = "guestkit.getLoginState";
pub const METHOD_GET_DNS_STATE: &str = "guestkit.getDnsState";
pub const METHOD_GET_TIMEDATE_STATE: &str = "guestkit.getTimedateState";
pub const METHOD_GET_SNAPSHOT_READINESS: &str = "guestkit.getSnapshotReadiness";
pub const METHOD_FREEZE_FILESYSTEM: &str = "guestkit.freezeFilesystem";
pub const METHOD_THAW_FILESYSTEM: &str = "guestkit.thawFilesystem";
pub const METHOD_RESTART_UNIT: &str = "guestkit.restartUnit";
pub const METHOD_EXECUTE_REMEDIATION_PLAN: &str = "guestkit.executeRemediationPlan";
pub const METHOD_COLLECT_SUPPORT_BUNDLE: &str = "guestkit.collectSupportBundle";
pub const METHOD_GET_GUEST_INFO: &str = "guestkit.getGuestInfo";
pub const METHOD_GET_SYSTEMD_UNIT: &str = "guestkit.getSystemdUnit";
pub const METHOD_GET_SYSTEMD_EVENTS: &str = "guestkit.getSystemdEvents";
pub const METHOD_GET_PROCESSES: &str = "guestkit.getProcesses";

// --- Protocol 1.3: agent health + events ---
pub const METHOD_GET_AGENT_HEALTH: &str = "guestkit.getAgentHealth";
pub const METHOD_SUBSCRIBE_EVENTS: &str = "guestkit.subscribeEvents";
pub const METHOD_UNSUBSCRIBE_EVENTS: &str = "guestkit.unsubscribeEvents";
/// Notification method name for pushed heartbeats (server → host, no `id`).
pub const NOTIFICATION_HEARTBEAT: &str = "guestkit.event.heartbeat";

// --- Protocol 1.3: performance telemetry ---
pub const METHOD_GET_CPU_STATS: &str = "guestkit.getCpuStats";
pub const METHOD_GET_MEMORY_STATS: &str = "guestkit.getMemoryStats";
pub const METHOD_GET_PERFORMANCE_SUMMARY: &str = "guestkit.getPerformanceSummary";
pub const METHOD_GET_PERFORMANCE_HISTORY: &str = "guestkit.getPerformanceHistory";

// --- Protocol 1.3: service / process / network / storage / file / time / power ---
pub const METHOD_START_UNIT: &str = "guestkit.startUnit";
pub const METHOD_STOP_UNIT: &str = "guestkit.stopUnit";
pub const METHOD_GET_PROCESS: &str = "guestkit.getProcess";
pub const METHOD_NETWORK_TEST: &str = "guestkit.networkTest";
pub const METHOD_NETWORK_CONNECTIONS: &str = "guestkit.network.connections";
pub const METHOD_SECURITY_POSTURE: &str = "guestkit.security.posture";
pub const METHOD_INTEGRITY_BASELINE: &str = "guestkit.integrity.baseline";
pub const METHOD_INTEGRITY_CHECK: &str = "guestkit.integrity.check";
pub const METHOD_STORAGE_RESCAN: &str = "guestkit.storageRescan";
pub const METHOD_STORAGE_TRIM: &str = "guestkit.storageTrim";
pub const METHOD_STORAGE_EXPAND: &str = "guestkit.storageExpand";
pub const METHOD_FILE_READ: &str = "guestkit.fileRead";
pub const METHOD_FILE_WRITE: &str = "guestkit.fileWrite";
pub const METHOD_FILE_STAT: &str = "guestkit.fileStat";
pub const METHOD_FILE_LIST: &str = "guestkit.fileList";
pub const METHOD_FILE_CHECKSUM: &str = "guestkit.fileChecksum";
pub const METHOD_SET_TIME: &str = "guestkit.setTime";
pub const METHOD_TIME_SYNC_NOW: &str = "guestkit.timeSyncNow";
pub const METHOD_REBOOT: &str = "guestkit.reboot";
pub const METHOD_SHUTDOWN: &str = "guestkit.shutdown";

// --- Protocol 1.3: orchestrated snapshots ---
pub const METHOD_SNAPSHOT_PREPARE: &str = "guestkit.snapshot.prepare";
pub const METHOD_SNAPSHOT_COMPLETE: &str = "guestkit.snapshot.complete";

pub const METHOD_CONTAINERS_INVENTORY: &str = "guestkit.containers.inventory";
pub const METHOD_INVENTORY_CACHE_WRITE: &str = "guestkit.inventory.cacheSnapshot";

// --- Protocol 1.3: enterprise automation (Phase 6) ---
pub const METHOD_PACKAGES_INVENTORY: &str = "guestkit.packages.inventory";
pub const METHOD_PACKAGES_UPDATES: &str = "guestkit.packages.updates";
pub const METHOD_PACKAGES_INSTALL: &str = "guestkit.packages.install";
pub const METHOD_CERTIFICATES_INVENTORY: &str = "guestkit.certificates.inventory";
pub const METHOD_USERS_INVENTORY: &str = "guestkit.users.inventory";
pub const METHOD_SET_HOSTNAME: &str = "guestkit.system.setHostname";
pub const METHOD_SET_TIMEZONE: &str = "guestkit.system.setTimezone";
pub const METHOD_SET_DNS: &str = "guestkit.system.setDns";

// --- Protocol 1.3: migration assurance ---
pub const METHOD_MIGRATION_ASSESS: &str = "guestkit.migration.assess";
pub const METHOD_MIGRATION_PLAN: &str = "guestkit.migration.plan";
pub const METHOD_MIGRATION_REPAIR: &str = "guestkit.migration.repair";
pub const METHOD_MIGRATION_PRE_CHECK: &str = "guestkit.migration.preCheck";
pub const METHOD_MIGRATION_CUTOVER_ENTER: &str = "guestkit.migration.cutoverEnter";
pub const METHOD_MIGRATION_CUTOVER_EXIT: &str = "guestkit.migration.cutoverExit";
pub const METHOD_MIGRATION_VALIDATE: &str = "guestkit.migration.validate";
pub const METHOD_BASELINE_CAPTURE: &str = "guestkit.baseline.capture";
pub const METHOD_BASELINE_DIFF: &str = "guestkit.baseline.diff";

/// Capability flags returned during negotiation.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapabilities {
    pub protocol_version: String,
    pub agent_version: String,
    pub platform: String,
    pub methods: Vec<String>,
    pub fix_apply: bool,
    pub windows: bool,
    /// True when this agent supports server-push notifications
    /// (subscribeEvents / event.* frames) on dedicated channels.
    #[serde(default)]
    pub events: bool,
    /// Policy capability categories currently enabled (e.g. "inventory",
    /// "telemetry", "service_control", "file_ops", "storage_ops",
    /// "migration", "migration_repair").
    #[serde(default)]
    pub categories: Vec<String>,
}

impl AgentCapabilities {
    pub fn standard(agent_version: &str) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            agent_version: agent_version.to_string(),
            platform: std::env::consts::OS.to_string(),
            methods: vec![
                METHOD_PING.to_string(),
                METHOD_GET_VERSION.to_string(),
                METHOD_GET_CAPABILITIES.to_string(),
                METHOD_GET_EVIDENCE.to_string(),
                METHOD_GET_STATUS.to_string(),
                METHOD_DOCTOR.to_string(),
                METHOD_MIGRATE_SCORE.to_string(),
                METHOD_GET_METRICS.to_string(),
                METHOD_GET_FILESYSTEM.to_string(),
                METHOD_EXEC.to_string(),
                METHOD_ENABLE_RDP.to_string(),
                METHOD_DISABLE_RDP.to_string(),
                METHOD_GET_GUEST_HEALTH.to_string(),
                METHOD_GET_SYSTEMD_UNITS.to_string(),
                METHOD_GET_FAILED_UNITS.to_string(),
                METHOD_GET_BOOT_ANALYSIS.to_string(),
                METHOD_GET_JOURNAL_SLICE.to_string(),
                METHOD_GET_LOGIN_STATE.to_string(),
                METHOD_GET_DNS_STATE.to_string(),
                METHOD_GET_TIMEDATE_STATE.to_string(),
                METHOD_GET_SNAPSHOT_READINESS.to_string(),
                METHOD_FREEZE_FILESYSTEM.to_string(),
                METHOD_THAW_FILESYSTEM.to_string(),
                METHOD_RESTART_UNIT.to_string(),
                METHOD_EXECUTE_REMEDIATION_PLAN.to_string(),
                METHOD_COLLECT_SUPPORT_BUNDLE.to_string(),
                METHOD_GET_GUEST_INFO.to_string(),
                METHOD_GET_SYSTEMD_UNIT.to_string(),
                METHOD_GET_SYSTEMD_EVENTS.to_string(),
                METHOD_GET_PROCESSES.to_string(),
                METHOD_RUN_FIX_PLAN.to_string(),
                METHOD_RUN_FIX_PLAN_ROLLBACK.to_string(),
                METHOD_GET_AGENT_HEALTH.to_string(),
                METHOD_SUBSCRIBE_EVENTS.to_string(),
                METHOD_UNSUBSCRIBE_EVENTS.to_string(),
                METHOD_GET_CPU_STATS.to_string(),
                METHOD_GET_MEMORY_STATS.to_string(),
                METHOD_GET_PERFORMANCE_SUMMARY.to_string(),
                METHOD_GET_PERFORMANCE_HISTORY.to_string(),
                METHOD_START_UNIT.to_string(),
                METHOD_STOP_UNIT.to_string(),
                METHOD_GET_PROCESS.to_string(),
                METHOD_NETWORK_TEST.to_string(),
                METHOD_NETWORK_CONNECTIONS.to_string(),
                METHOD_SECURITY_POSTURE.to_string(),
                METHOD_TIME_SYNC_NOW.to_string(),
                METHOD_REBOOT.to_string(),
                METHOD_SHUTDOWN.to_string(),
                METHOD_MIGRATION_ASSESS.to_string(),
                METHOD_MIGRATION_PLAN.to_string(),
                METHOD_MIGRATION_REPAIR.to_string(),
                METHOD_MIGRATION_PRE_CHECK.to_string(),
                METHOD_MIGRATION_CUTOVER_ENTER.to_string(),
                METHOD_MIGRATION_CUTOVER_EXIT.to_string(),
                METHOD_MIGRATION_VALIDATE.to_string(),
                METHOD_BASELINE_CAPTURE.to_string(),
                METHOD_BASELINE_DIFF.to_string(),
                METHOD_PACKAGES_INVENTORY.to_string(),
                METHOD_PACKAGES_UPDATES.to_string(),
                METHOD_PACKAGES_INSTALL.to_string(),
                METHOD_CERTIFICATES_INVENTORY.to_string(),
                METHOD_USERS_INVENTORY.to_string(),
                METHOD_SET_HOSTNAME.to_string(),
                METHOD_SET_TIMEZONE.to_string(),
                METHOD_SET_DNS.to_string(),
                METHOD_CONTAINERS_INVENTORY.to_string(),
                METHOD_INVENTORY_CACHE_WRITE.to_string(),
                METHOD_INTEGRITY_BASELINE.to_string(),
                METHOD_INTEGRITY_CHECK.to_string(),
                METHOD_FILE_READ.to_string(),
                METHOD_FILE_WRITE.to_string(),
                METHOD_FILE_STAT.to_string(),
                METHOD_FILE_LIST.to_string(),
                METHOD_FILE_CHECKSUM.to_string(),
                METHOD_SNAPSHOT_PREPARE.to_string(),
                METHOD_SNAPSHOT_COMPLETE.to_string(),
                METHOD_STORAGE_RESCAN.to_string(),
                METHOD_STORAGE_TRIM.to_string(),
                METHOD_STORAGE_EXPAND.to_string(),
            ],
            fix_apply: true,
            windows: cfg!(target_os = "windows"),
            events: true,
            // Filled in from policy once capability categories land;
            // `methods` must only advertise what the agent actually dispatches.
            categories: Vec::new(),
        }
    }
}
