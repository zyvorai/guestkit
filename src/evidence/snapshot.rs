// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Evidence snapshot schema (v2).

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 5;

/// Normalized evidence collected from an offline disk image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSnapshot {
    pub schema_version: u32,
    pub image_path: String,
    pub collected_at: String,
    pub root: String,
    pub os: OsEvidence,
    pub storage: StorageEvidence,
    pub boot: BootEvidence,
    pub network: NetworkEvidence,
    pub packages: PackageEvidence,
    pub security: SecurityEvidence,
    pub vm_tools: VmToolsEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub systemd: Option<SystemdInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<WindowsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubevirt: Option<KubevirtEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<CloudInitEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_probes: Option<NetworkProbeEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_readiness: Option<SnapshotReadinessEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<ProcessEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware: Option<HardwareEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux_migration: Option<LinuxMigrationEvidence>,
    /// Integrity-verified snapshot of last-known *running* state, written by
    /// the live agent and read back during offline disk inspection (§31).
    /// Opaque JSON to keep this schema dependency-light; shape is
    /// `agent::inventory_cache::OnlineInventoryCache`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_cache: Option<serde_json::Value>,
}

/// Linux migration-relevant state that no other section captures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinuxMigrationEvidence {
    /// Predictable interface naming active (systemd .link / no net.ifnames=0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predictable_nic_names: Option<bool>,
    /// Config files that pin static IPs (sysconfig/netplan/NetworkManager).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub static_ip_configs: Vec<String>,
    /// Hypervisor-specific kernel modules currently loaded (vmw_*, hv_*, xen*).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hypervisor_modules: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsEvidence {
    pub os_type: String,
    pub distribution: String,
    pub version: String,
    pub architecture: String,
    pub hostname: String,
    pub init_system: String,
    pub package_manager: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageEvidence {
    pub fstab_entries: Vec<FstabEntry>,
    pub crypttab_entries: Vec<CrypttabEntry>,
    pub swap_devices: Vec<String>,
    pub root_filesystem: String,
    pub partition_uuids: Vec<PartitionUuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_space_root_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_disk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_controller: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FstabEntry {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    pub options: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrypttabEntry {
    pub name: String,
    pub device: String,
    pub keyfile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionUuid {
    pub device: String,
    pub uuid: String,
    pub fstype: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootEvidence {
    pub bootloader: String,
    pub default_entry: String,
    pub kernel_cmdline: String,
    pub kernel_paths: Vec<String>,
    pub initramfs_paths: Vec<String>,
    pub efi_present: bool,
    pub grub_cfg_path: Option<String>,
    pub loaded_modules: Vec<String>,
    pub pending_relabel: bool,
    pub cloud_init_present: bool,
    /// Modules bundled in the default initramfs (virtio focus).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initramfs_modules: Vec<String>,
    /// Secure Boot enabled? None when undetectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure_boot: Option<bool>,
    /// "bios" or "uefi"; empty when unknown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub firmware: String,
    #[serde(default)]
    pub serial_console_configured: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkEvidence {
    pub interfaces: Vec<String>,
    pub dns_servers: Vec<String>,
    pub udev_persistent_net: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_interfaces: Vec<NetworkInterfaceLive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_gateway: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub network_stack: String,
    /// Raw routing table lines (`ip route` format) for drift comparison.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkInterfaceLive {
    pub name: String,
    pub state: String,
    pub mac: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<String>,
    pub carrier: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageEvidence {
    pub count: usize,
    pub kernels: Vec<String>,
    pub sample_packages: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityEvidence {
    pub selinux: String,
    pub apparmor: bool,
    pub firewall_enabled: bool,
    pub ssh_root_login: Option<bool>,
    pub auditd: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_ports: Vec<u16>,
    #[serde(default)]
    pub pending_security_updates: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmToolsEvidence {
    pub detected: Vec<String>,
}

/// Systemd unit and static analysis collected from disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub unit_count: usize,
    pub service_count: usize,
    pub timer_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<SystemdUnit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problem_hints: Vec<SystemdProblemHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<SystemdRuntimeInfo>,
}

/// Live systemd manager state from D-Bus org.freedesktop.systemd1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdRuntimeInfo {
    pub manager_state: String,
    pub failed_unit_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager: Option<SystemdManagerInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<SystemdRuntimeUnit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<SystemdJob>,
}

/// Manager-level properties from org.freedesktop.systemd1.Manager.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdManagerInfo {
    pub system_state: String,
    pub n_failed_units: u32,
    pub n_jobs: u32,
    pub architecture: String,
    pub virtualization: String,
    pub boot_timestamps: SystemdBootTimestamps,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdBootTimestamps {
    pub firmware_us: Option<u64>,
    pub loader_us: Option<u64>,
    pub kernel_us: Option<u64>,
    pub initrd_us: Option<u64>,
    pub userspace_us: Option<u64>,
    pub finish_us: Option<u64>,
    pub security_start_us: Option<u64>,
    pub security_finish_us: Option<u64>,
    pub units_load_start_us: Option<u64>,
    pub units_load_finish_us: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdRuntimeUnit {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    pub fragment_path: String,
    pub main_pid: u32,
    pub n_restarts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_main_start_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_main_exit_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_main_status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroup_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_in_paths: Vec<String>,
    pub need_daemon_reload: bool,
    pub can_start: bool,
    pub can_stop: bool,
    pub can_reload: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_main_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_usec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_start_usec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_stop_usec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watchdog_usec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oom_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reload_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clean_result: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdJob {
    pub id: u32,
    pub unit: String,
    pub job_type: String,
    pub state: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdUnit {
    pub name: String,
    pub unit_type: String,
    pub path: String,
    pub state: SystemdUnitState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remain_after_exit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub before: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wants: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wanted_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_calendar: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SystemdUnitSection>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemdUnitState {
    #[default]
    Disabled,
    Enabled,
    Masked,
    Static,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdUnitSection {
    pub name: String,
    pub keys: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdProblemHint {
    pub unit: String,
    pub code: String,
    pub severity: SystemdProblemSeverity,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemdProblemSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KubevirtEvidence {
    pub hostname: String,
    pub virtio_channel_present: bool,
    pub virtio_channel_path: String,
    pub cloud_init_config_present: bool,
    pub config_drive_mounted: bool,
    pub agent_service_active: bool,
    pub guestkit_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtio_disks: Vec<VirtioDiskEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VirtioDiskEntry {
    pub device: String,
    pub serial: String,
    pub mountpoints: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudInitEvidence {
    pub installed: bool,
    pub status: String,
    pub datasource: String,
    pub boot_finished: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_log_line: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkProbeEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns_servers: Vec<String>,
    pub cluster_dns_reachable: bool,
    pub api_service_reachable: bool,
    pub internet_reachable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_details: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotReadinessEvidence {
    pub fs_frozen: bool,
    pub guest_agent_connected: bool,
    pub quiesce_supported: bool,
    pub fstrim_recommended: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsEvidence {
    pub systemroot: String,
    pub product_name: String,
    pub version: String,
    pub domain_joined: bool,
    pub domain_name: Option<String>,
    pub rdp_enabled: bool,
    pub pending_reboot: bool,
    pub bitlocker_detected: bool,
    pub installed_apps_count: usize,
    pub services_count: usize,
    pub drivers_count: usize,
    pub hypervisor_remnants: Vec<String>,
    pub av_edr: Vec<String>,
    pub minidump_count: usize,
    #[serde(default)]
    pub bcd_store_found: bool,
    #[serde(default)]
    pub bootmgr_found: bool,
    /// Detected System Reserved / ESP boot volume (separate from Windows OS volume).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_reserved: Option<SystemReservedPartition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<WindowsServiceEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installed_apps: Vec<WindowsAppEntry>,
    #[serde(default)]
    pub persistence: WindowsPersistenceEvidence,
    #[serde(default)]
    pub event_logs: WindowsEventLogSummary,
    /// VirtIO driver install state (viostor/vioscsi/netkvm/vioser/balloon).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtio_drivers: Vec<WindowsDriverEntry>,
    /// Installed hotfixes / KBs (capped sample for migration diagnostics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hotfixes: Vec<WindowsHotfixEntry>,
    #[serde(default)]
    pub hotfix_count: usize,
    /// `$hf_mig$` present — incomplete hotfix migration data on disk.
    #[serde(default)]
    pub hf_mig_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitlocker: Option<BitLockerState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vss: Option<VssHealth>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ghost_nics: Vec<GhostNicEntry>,
    /// Static NIC configurations, captured for post-migration IP transfer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub static_nic_configs: Vec<WindowsNicConfig>,
    /// True when driver signature enforcement is active (no testsigning /
    /// nointegritychecks). None when undetectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_signature_enforcement: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esp_present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<ActivationInfo>,
}

/// Separate boot volume for Windows (legacy System Reserved or UEFI ESP).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemReservedPartition {
    /// Block device path (e.g. `/dev/sda1`).
    pub device: String,
    pub fstype: String,
    /// `system_reserved` (BIOS/NTFS), `esp` (UEFI/FAT), or `unknown_boot`.
    pub role: String,
    pub has_bootmgr: bool,
    pub has_bcd: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// One virtio (or otherwise migration-critical) Windows driver.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsDriverEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// SCM start type as text ("boot", "system", "auto", "manual", "disabled").
    #[serde(default)]
    pub start_type: String,
    /// Start type Boot (0) — required for boot-critical storage drivers.
    #[serde(default)]
    pub boot_critical: bool,
    #[serde(default)]
    pub present: bool,
    /// `System32\drivers\<name>.sys` present on disk (None = not probed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sys_present: Option<bool>,
}

/// Installed Windows hotfix / update KB for migration diagnostics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsHotfixEntry {
    pub kb: String,
    /// `registry`, `filesystem`, `cbs`, or `hf_mig`.
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BitLockerState {
    /// Any volume with protection currently on (not suspended).
    #[serde(default)]
    pub any_protected: bool,
    /// Offline probe found FVE artifacts but could not confirm ProtectionStatus.
    #[serde(default)]
    pub offline_uncertain: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<BitLockerVolume>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BitLockerVolume {
    pub mount_point: String,
    /// "on", "off", "suspended", "artifacts", or "unknown".
    pub protection: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VssHealth {
    #[serde(default)]
    pub writers_total: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writers_failed: Vec<String>,
    #[serde(default)]
    pub healthy: bool,
    /// True when health was inferred offline (services / SVI), not live writers.
    #[serde(default)]
    pub offline_inferred: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GhostNicEntry {
    pub instance_id: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsNicConfig {
    pub name: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ip_addresses: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
    #[serde(default)]
    pub dhcp: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivationInfo {
    #[serde(default)]
    pub licensed: bool,
    /// "KMS", "MAK", "OEM", "Retail", "Volume", or empty when unknown.
    #[serde(default)]
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsServiceEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    pub start_type: WindowsStartType,
    pub service_type: WindowsServiceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kernel_driver: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsStartType {
    Boot,
    System,
    Automatic,
    Manual,
    Disabled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsServiceType {
    KernelDriver,
    FileSystemDriver,
    Win32,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsAppEntry {
    pub name: String,
    pub version: String,
    pub publisher: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_location: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsPersistenceEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run_keys: Vec<WindowsPersistenceEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scheduled_tasks: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsPersistenceEntry {
    pub location: String,
    pub name: String,
    pub command: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsEventLogSummary {
    pub log_count: usize,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forensic: Option<WindowsForensicProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsForensicProfile {
    pub failed_logons: u32,
    pub successful_logons: u32,
    pub service_failures: u32,
    pub unexpected_shutdowns: u32,
    pub privilege_escalations: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suspicious_event_ids: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_critical: Vec<WindowsForensicEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsForensicEvent {
    pub event_id: u32,
    pub channel: String,
    pub source: String,
    pub level: String,
    pub time_created: String,
    pub summary: String,
}

/// Process and cgroup intelligence from /proc.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_cpu: Vec<ProcessSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_memory: Vec<ProcessSummary>,
    pub zombie_count: usize,
    pub d_state_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listening_ports: Vec<ListeningPort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_cpu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_memory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_io: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessSummary {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_kb: u64,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListeningPort {
    pub port: u16,
    pub pid: u32,
    pub process: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Hardware and identity inventory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HardwareEvidence {
    pub dmi_manufacturer: String,
    pub dmi_product: String,
    pub dmi_uuid: String,
    pub machine_id: String,
    pub virtio_net_count: usize,
    pub virtio_blk_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_devices: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zeus_vm_uid: Option<String>,
}

impl EvidenceSnapshot {
    pub fn get_field(&self, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('.').collect();
        match parts.as_slice() {
            ["os", "distribution"] => Some(self.os.distribution.clone()),
            ["os", "version"] => Some(self.os.version.clone()),
            ["os", "hostname"] => Some(self.os.hostname.clone()),
            ["security", "selinux"] => Some(self.security.selinux.clone()),
            ["security", "ssh", "root_login"] => {
                self.security.ssh_root_login.map(|v| v.to_string())
            }
            ["boot", "bootloader"] => Some(self.boot.bootloader.clone()),
            ["packages", "count"] => Some(self.packages.count.to_string()),
            ["systemd", "service_count"] => {
                self.systemd.as_ref().map(|s| s.service_count.to_string())
            }
            ["systemd", "problem_count"] => self
                .systemd
                .as_ref()
                .map(|s| s.problem_hints.len().to_string()),
            _ => None,
        }
    }
}
