// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Agent remediation policy (allowlist).

use guestkit_agent_protocol::{AgentError, RpcMethod};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_POLICY_PATH: &str = "/etc/zyvor/agent-policy.yaml";
/// Preferred (GuestKit-branded) policy path; checked before the legacy one.
const GUESTKIT_POLICY_PATH: &str = "/etc/guestkit/agent-policy.yaml";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPolicy {
    #[serde(default)]
    pub actions: PolicyActions,
    #[serde(default)]
    pub capabilities: CapabilityToggles,
    #[serde(default)]
    pub methods: MethodPolicy,
    #[serde(default)]
    pub security: SecurityPolicy,
}

/// Coarse feature-category switches a local administrator can flip
/// without enumerating individual methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToggles {
    #[serde(default = "default_true")]
    pub inventory: bool,
    #[serde(default = "default_true")]
    pub telemetry: bool,
    #[serde(default = "default_true")]
    pub events: bool,
    #[serde(default = "default_true")]
    pub network_test: bool,
    #[serde(default)]
    pub file_ops: FileOpsPolicy,
    #[serde(default)]
    pub storage_ops: StorageOpsPolicy,
    #[serde(default)]
    pub packages: PackagePolicy,
    #[serde(default = "default_true")]
    pub certificates: bool,
    /// Read-only user/session/access inventory.
    #[serde(default = "default_true")]
    pub users: bool,
    /// Tamper/integrity baseline + check.
    #[serde(default = "default_true")]
    pub integrity: bool,
}

impl Default for CapabilityToggles {
    fn default() -> Self {
        Self {
            inventory: true,
            telemetry: true,
            events: true,
            network_test: true,
            file_ops: FileOpsPolicy::default(),
            storage_ops: StorageOpsPolicy::default(),
            packages: PackagePolicy::default(),
            certificates: true,
            users: true,
            integrity: true,
        }
    }
}

/// Package/patch management. Inventory and update queries are read-only and
/// on by default; installation mutates the system and ships disabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePolicy {
    #[serde(default = "default_true")]
    pub inventory: bool,
    #[serde(default = "default_true")]
    pub updates: bool,
    #[serde(default)]
    pub install: bool,
}

impl Default for PackagePolicy {
    fn default() -> Self {
        Self {
            inventory: true,
            updates: true,
            install: false,
        }
    }
}

/// File operations are sensitive and ship disabled.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileOpsPolicy {
    #[serde(default)]
    pub enabled: bool,
    /// Canonicalized path prefixes reads/writes must stay inside.
    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,
    #[serde(default = "FileOpsPolicy::default_max_bytes")]
    pub max_bytes: u64,
}

impl FileOpsPolicy {
    fn default_max_bytes() -> u64 {
        1024 * 1024
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOpsPolicy {
    #[serde(default = "default_true")]
    pub rescan: bool,
    #[serde(default = "default_true")]
    pub trim: bool,
    /// Filesystem expansion mutates partitions/LVs; off unless opted in.
    #[serde(default)]
    pub expand: bool,
}

impl Default for StorageOpsPolicy {
    fn default() -> Self {
        Self {
            rescan: true,
            trim: true,
            expand: false,
        }
    }
}

/// Explicit method allow/deny lists (glob patterns on the wire method
/// name, e.g. `guestkit.migration.*`). Deny wins; empty allow = all.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MethodPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// When true, mutating requests without `ts`/`ttl_ms` are rejected.
    #[serde(default)]
    pub require_request_expiry: bool,
    #[serde(default = "SecurityPolicy::default_max_ttl_ms")]
    pub max_ttl_ms: u64,
    #[serde(default = "SecurityPolicy::default_nonce_cache")]
    pub nonce_cache_size: usize,
}

impl SecurityPolicy {
    fn default_max_ttl_ms() -> u64 {
        300_000
    }
    fn default_nonce_cache() -> usize {
        4096
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            require_request_expiry: false,
            max_ttl_ms: Self::default_max_ttl_ms(),
            nonce_cache_size: Self::default_nonce_cache(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyActions {
    #[serde(default)]
    pub restart_unit: RestartUnitPolicy,
    #[serde(default)]
    pub run_shell_command: ShellPolicy,
    #[serde(default)]
    pub reboot_vm: ApprovalPolicy,
    #[serde(default)]
    pub self_update: SelfUpdatePolicy,
    #[serde(default)]
    pub migration: MigrationPolicy,
    /// Guest customization (hostname/timezone/DNS): mutates system identity,
    /// off by default.
    #[serde(default)]
    pub customization: CustomizationPolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomizationPolicy {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPolicy {
    /// Read-only assessment and plan preview.
    #[serde(default = "default_true")]
    pub assess: bool,
    /// Applying repair plans (still dry-run by default per request).
    #[serde(default)]
    pub repair: bool,
    /// Destructive repairs: VMware Tools uninstall, ghost-NIC removal,
    /// BCD writes.
    #[serde(default)]
    pub repair_destructive: bool,
}

impl Default for MigrationPolicy {
    fn default() -> Self {
        Self {
            assess: true,
            repair: false,
            repair_destructive: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelfUpdatePolicy {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_apply: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RestartUnitPolicy {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_units: Vec<String>,
    #[serde(default)]
    pub require_approval: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShellPolicy {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    #[serde(default)]
    pub require_approval: bool,
}

impl AgentPolicy {
    pub fn load() -> Self {
        let path = std::env::var("ZYVOR_AGENT_POLICY").unwrap_or_else(|_| {
            if Path::new(GUESTKIT_POLICY_PATH).exists() {
                GUESTKIT_POLICY_PATH.to_string()
            } else {
                DEFAULT_POLICY_PATH.to_string()
            }
        });
        if Path::new(&path).exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_yaml::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Self::default_permissive_dev()
        }
    }

    pub fn default_permissive_dev() -> Self {
        Self {
            actions: PolicyActions {
                restart_unit: RestartUnitPolicy {
                    enabled: true,
                    allowed_units: vec![],
                    require_approval: false,
                },
                run_shell_command: ShellPolicy { enabled: false },
                reboot_vm: ApprovalPolicy {
                    require_approval: true,
                },
                self_update: SelfUpdatePolicy {
                    enabled: false,
                    auto_apply: false,
                },
                migration: MigrationPolicy::default(),
                customization: CustomizationPolicy::default(),
            },
            capabilities: CapabilityToggles::default(),
            methods: MethodPolicy::default(),
            security: SecurityPolicy::default(),
        }
    }

    pub fn can_auto_apply_update(&self) -> bool {
        self.actions.self_update.enabled && self.actions.self_update.auto_apply
    }

    pub fn can_restart_unit(&self, unit: &str) -> bool {
        if !self.actions.restart_unit.enabled {
            return false;
        }
        if self.actions.restart_unit.allowed_units.is_empty() {
            return true;
        }
        let allowed: HashSet<&str> = self
            .actions
            .restart_unit
            .allowed_units
            .iter()
            .map(String::as_str)
            .collect();
        allowed.contains(unit)
    }

    pub fn shell_enabled(&self) -> bool {
        self.actions.run_shell_command.enabled
    }

    /// Category and glob authorization for a parsed method. Unit-level
    /// checks (`allowed_units`) remain with the individual handlers.
    pub fn authorize(&self, method: &RpcMethod, wire_name: &str) -> Result<(), AgentError> {
        let denied = |what: &str| {
            Err(AgentError::PolicyDenied(format!(
                "{what} disabled by local policy"
            )))
        };

        if glob_match(&self.methods.deny, wire_name) {
            return denied(wire_name);
        }
        if !self.methods.allow.is_empty() && !glob_match(&self.methods.allow, wire_name) {
            return denied(wire_name);
        }

        use RpcMethod::*;
        match method {
            GetEvidence | GetGuestInfo | GetStatus | GetFilesystem | GetProcesses | GetProcess
            | GetSystemdUnits | GetSystemdUnit | GetFailedUnits | GetSystemdEvents
                if !self.capabilities.inventory =>
            {
                denied("inventory")
            }
            GetMetrics
            | GetCpuStats
            | GetMemoryStats
            | GetPerformanceSummary
            | GetPerformanceHistory
                if !self.capabilities.telemetry =>
            {
                denied("telemetry")
            }
            SubscribeEvents if !self.capabilities.events => denied("events"),
            NetworkTest if !self.capabilities.network_test => denied("network_test"),
            NetworkConnections | SecurityPosture if !self.capabilities.inventory => {
                denied("inventory")
            }
            FileRead | FileWrite | FileStat | FileList | FileChecksum
                if !self.capabilities.file_ops.enabled =>
            {
                denied("file_ops")
            }
            StorageRescan if !self.capabilities.storage_ops.rescan => denied("storage rescan"),
            StorageTrim if !self.capabilities.storage_ops.trim => denied("storage trim"),
            StorageExpand if !self.capabilities.storage_ops.expand => denied("storage expand"),
            StartUnit | StopUnit | RestartUnit if !self.actions.restart_unit.enabled => {
                denied("service control")
            }
            MigrationAssess | MigrationPlan | MigrationPreCheck | MigrationValidate
            | BaselineCapture | BaselineDiff
                if !self.actions.migration.assess =>
            {
                denied("migration assessment")
            }
            MigrationRepair if !self.actions.migration.repair => denied("migration repair"),
            PackagesInventory if !self.capabilities.packages.inventory => {
                denied("package inventory")
            }
            PackagesUpdates if !self.capabilities.packages.updates => denied("package updates"),
            PackagesInstall if !self.capabilities.packages.install => denied("package install"),
            CertificatesInventory if !self.capabilities.certificates => {
                denied("certificate inventory")
            }
            UsersInventory if !self.capabilities.users => denied("user inventory"),
            ContainersInventory | InventoryCacheWrite if !self.capabilities.inventory => {
                denied("inventory")
            }
            IntegrityBaseline | IntegrityCheck if !self.capabilities.integrity => {
                denied("integrity monitoring")
            }
            SetHostname | SetTimezone | SetDns if !self.actions.customization.enabled => {
                denied("customization")
            }
            _ => Ok(()),
        }
    }

    /// Categories currently enabled, for capability advertisement.
    pub fn enabled_categories(&self) -> Vec<String> {
        let mut cats = Vec::new();
        let caps = &self.capabilities;
        for (on, name) in [
            (caps.inventory, "inventory"),
            (caps.telemetry, "telemetry"),
            (caps.events, "events"),
            (caps.network_test, "network_test"),
            (caps.file_ops.enabled, "file_ops"),
            (
                caps.storage_ops.rescan || caps.storage_ops.trim,
                "storage_ops",
            ),
            (self.actions.restart_unit.enabled, "service_control"),
            (self.actions.run_shell_command.enabled, "shell"),
            (self.actions.migration.assess, "migration"),
            (self.actions.migration.repair, "migration_repair"),
            (caps.packages.inventory || caps.packages.updates, "packages"),
            (caps.packages.install, "packages_install"),
            (caps.certificates, "certificates"),
            (caps.users, "users"),
            (caps.integrity, "integrity"),
            (self.actions.customization.enabled, "customization"),
        ] {
            if on {
                cats.push(name.to_string());
            }
        }
        cats
    }
}

fn glob_match(patterns: &[String], name: &str) -> bool {
    patterns.iter().any(|p| {
        glob::Pattern::new(p)
            .map(|pat| pat.matches(name))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let p = AgentPolicy::default();
        assert!(p.capabilities.telemetry);
        assert!(!p.capabilities.file_ops.enabled);
        assert!(!p.capabilities.storage_ops.expand);
        assert!(!p.security.require_request_expiry);
        assert!(p
            .authorize(&RpcMethod::GetCpuStats, "guestkit.getCpuStats")
            .is_ok());
        assert!(p
            .authorize(&RpcMethod::FileRead, "guestkit.fileRead")
            .is_err());
        assert!(p
            .authorize(&RpcMethod::StorageExpand, "guestkit.storageExpand")
            .is_err());
        assert!(p
            .authorize(&RpcMethod::StorageRescan, "guestkit.storageRescan")
            .is_ok());
    }

    #[test]
    fn deny_glob_wins() {
        let mut p = AgentPolicy::default();
        p.methods.deny = vec!["guestkit.migration.*".to_string()];
        assert!(p
            .authorize(&RpcMethod::MigrationAssess, "guestkit.migration.assess")
            .is_err());
        assert!(p.authorize(&RpcMethod::Ping, "guestkit.ping").is_ok());
    }

    #[test]
    fn allow_list_restricts() {
        let mut p = AgentPolicy::default();
        p.methods.allow = vec!["guestkit.ping".to_string(), "guestkit.get*".to_string()];
        assert!(p.authorize(&RpcMethod::Ping, "guestkit.ping").is_ok());
        assert!(p
            .authorize(&RpcMethod::GetCpuStats, "guestkit.getCpuStats")
            .is_ok());
        assert!(p.authorize(&RpcMethod::Reboot, "guestkit.reboot").is_err());
    }

    #[test]
    fn category_toggle_disables_group() {
        let mut p = AgentPolicy::default();
        p.capabilities.telemetry = false;
        assert!(p
            .authorize(&RpcMethod::GetCpuStats, "guestkit.getCpuStats")
            .is_err());
    }

    #[test]
    fn phase6_defaults() {
        let p = AgentPolicy::default();
        // Read-heavy inventory on by default.
        assert!(p
            .authorize(&RpcMethod::PackagesInventory, "guestkit.packages.inventory")
            .is_ok());
        assert!(p
            .authorize(&RpcMethod::PackagesUpdates, "guestkit.packages.updates")
            .is_ok());
        assert!(p
            .authorize(
                &RpcMethod::CertificatesInventory,
                "guestkit.certificates.inventory"
            )
            .is_ok());
        assert!(p
            .authorize(&RpcMethod::UsersInventory, "guestkit.users.inventory")
            .is_ok());
        // Mutating: install and customization off by default.
        assert!(p
            .authorize(&RpcMethod::PackagesInstall, "guestkit.packages.install")
            .is_err());
        assert!(p
            .authorize(&RpcMethod::SetHostname, "guestkit.system.setHostname")
            .is_err());
        let cats = p.enabled_categories();
        assert!(cats.contains(&"packages".to_string()));
        assert!(cats.contains(&"certificates".to_string()));
        assert!(!cats.contains(&"packages_install".to_string()));
        assert!(!cats.contains(&"customization".to_string()));
    }

    #[test]
    fn legacy_yaml_without_new_sections_parses() {
        let yaml = r#"
actions:
  restart_unit:
    enabled: true
    allowed_units: ["nginx.service"]
"#;
        let p: AgentPolicy = serde_yaml::from_str(yaml).unwrap();
        assert!(p.can_restart_unit("nginx.service"));
        assert!(!p.can_restart_unit("sshd.service"));
        assert!(p.capabilities.telemetry); // new sections default sanely
        assert_eq!(p.security.max_ttl_ms, 300_000);
    }
}
