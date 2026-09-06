// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Plan type definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete fix plan containing all operations and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPlan {
    /// Plan format version
    pub version: String,

    /// VM disk path
    pub vm: String,

    /// When the plan was generated
    pub generated: DateTime<Utc>,

    /// Profile that generated this plan
    pub profile: String,

    /// Overall risk level
    pub overall_risk: String,

    /// Estimated duration
    pub estimated_duration: String,

    /// Plan metadata
    pub metadata: PlanMetadata,

    /// List of operations to perform
    pub operations: Vec<Operation>,

    /// Actions to run after all operations complete
    pub post_apply: Vec<PostApplyAction>,
}

/// Metadata about the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanMetadata {
    /// Who/what generated the plan
    pub author: String,

    /// Whether human review is required
    pub review_required: bool,

    /// Whether all operations are reversible
    pub reversible: bool,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A single fix operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Unique operation ID
    pub id: String,

    /// Operation type
    #[serde(flatten)]
    pub op_type: OperationType,

    /// Priority level
    pub priority: Priority,

    /// Human-readable description
    pub description: String,

    /// Risk level of this operation
    pub risk: Priority,

    /// Whether this operation can be reversed
    pub reversible: bool,

    /// IDs of operations this depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Optional validation to run after operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationCheck>,

    /// Optional undo information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<UndoInfo>,
}

/// Types of operations that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationType {
    /// Edit a file
    FileEdit(FileEdit),

    /// Install packages
    PackageInstall(PackageInstall),

    /// Service operation (enable, start, restart)
    ServiceOperation(ServiceOperation),

    /// SELinux mode change
    SelinuxMode(SELinuxMode),

    /// Windows registry edit
    RegistryEdit(RegistryEdit),

    /// Execute a command
    CommandExec(CommandExec),

    /// Copy a file
    FileCopy(FileCopy),

    /// Create a directory
    DirectoryCreate(DirectoryCreate),

    /// Set file permissions
    FilePermissions(FilePermissions),

    /// Inject a Windows driver (pnputil live; virtio-win extraction offline)
    DriverInject(DriverInject),

    /// Write (create or overwrite) a whole file
    FileWrite(FileWrite),

    /// Create a symbolic link (guestfs `ln_sf` — offline-friendly)
    Symlink(Symlink),

    /// Delete a file (guestfs `rm` — offline-friendly)
    FileDelete(FileDelete),
}

/// Create or overwrite a file with exact contents (offline-friendly).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWrite {
    /// Guest path
    pub path: String,

    /// Full file contents
    pub content: String,

    /// Optional octal mode (e.g. "0644")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// Symbolic link creation (force-replace via guestfs `ln_sf`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symlink {
    /// Link target (prefer relative paths under `/etc` to avoid guestfs escape checks)
    pub target: String,

    /// Path of the symlink to create
    pub link_path: String,
}

/// Delete a guest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDelete {
    /// Guest path to remove
    pub path: String,

    /// If true, missing path is success
    #[serde(default)]
    pub missing_ok: bool,
}

/// Windows driver injection. Kept as a first-class operation (rather than
/// an opaque CommandExec) so repair plans stay introspectable and
/// auditable. Boot-critical registration (Start=0, Group) is a separate
/// RegistryEdit chained via depends_on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverInject {
    /// Path to the .inf file inside the guest (or image, offline).
    pub inf_path: String,

    /// Driver name (e.g. "vioscsi", "netkvm").
    pub driver_name: String,

    /// Whether this driver must be registered boot-critical.
    #[serde(default)]
    pub boot_critical: bool,

    /// Where the driver came from (e.g. "virtio-win 0.1.262").
    #[serde(default)]
    pub source: String,

    /// Host directory containing .inf/.sys/.cat for offline injection.
    /// Falls back to `source` if it is an existing directory, or
    /// `$GUESTKIT_VIRTIO_WIN/<driver_name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_dir: Option<String>,
}

impl DriverInject {
    /// Resolve a host directory of Windows driver files for offline apply.
    ///
    /// Order: explicit `host_dir` → `source` if it is a directory →
    /// `$GUESTKIT_VIRTIO_WIN/<driver>` (and common amd64 / 2k22 / w10 layouts).
    pub fn resolve_host_dir(&self) -> Option<std::path::PathBuf> {
        use std::path::PathBuf;
        if let Some(ref d) = self.host_dir {
            let p = PathBuf::from(d);
            if p.is_dir() {
                return Some(p);
            }
        }
        let src = PathBuf::from(&self.source);
        if src.is_dir() {
            return Some(src);
        }
        if let Ok(base) = std::env::var("GUESTKIT_VIRTIO_WIN") {
            let candidates = [
                PathBuf::from(&base).join(&self.driver_name),
                PathBuf::from(&base).join(&self.driver_name).join("amd64"),
                PathBuf::from(&base)
                    .join(&self.driver_name)
                    .join("2k22")
                    .join("amd64"),
                PathBuf::from(&base)
                    .join(&self.driver_name)
                    .join("2k19")
                    .join("amd64"),
                PathBuf::from(&base)
                    .join(&self.driver_name)
                    .join("w10")
                    .join("amd64"),
                PathBuf::from(&base)
                    .join(&self.driver_name)
                    .join("w11")
                    .join("amd64"),
            ];
            for c in candidates {
                if c.is_dir() {
                    return Some(c);
                }
            }
        }
        None
    }
}

/// File editing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    /// Path to file
    pub file: String,

    /// Whether to create backup
    #[serde(default = "default_true")]
    pub backup: bool,

    /// Changes to make
    pub changes: Vec<FileChange>,
}

/// A single file change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Line number (1-indexed)
    pub line: usize,

    /// Content before change
    pub before: String,

    /// Content after change
    pub after: String,

    /// Optional context lines for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Package installation operation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageInstall {
    /// Packages to install
    pub packages: Vec<String>,

    /// Estimated total size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_size: Option<String>,

    /// Optional host directory of `.rpm` / `.deb` for offline first-boot staging
    /// (also reads `GUESTKIT_PACKAGE_CACHE`; with `GUESTKIT_PACKAGE_FETCH=1`,
    /// missing packages are downloaded into the cache via host `dnf`/`apt-get`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_cache: Option<String>,
}

/// Service operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceOperation {
    /// Service name
    pub service: String,

    /// Desired state (enabled/disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Whether to start the service
    #[serde(default)]
    pub start: bool,

    /// Whether to restart the service
    #[serde(default)]
    pub restart: bool,
}

/// SELinux mode change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SELinuxMode {
    /// Config file path
    pub file: String,

    /// Current mode
    pub current: String,

    /// Target mode
    pub target: String,

    /// Optional warning message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Windows registry edit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEdit {
    /// Registry key path
    pub key: String,

    /// Value name
    pub value: String,

    /// Current data
    pub current_data: serde_json::Value,

    /// New data
    pub new_data: serde_json::Value,

    /// Data type (DWORD, String, etc.)
    pub data_type: String,
}

/// Command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExec {
    /// Command to execute
    pub command: String,

    /// Expected exit code
    #[serde(default)]
    pub expected_exit: i32,

    /// Optional timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Interpreter override (e.g. "powershell -NoProfile -Command").
    /// Default: `sh -c` on Unix, PowerShell on Windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreter: Option<String>,
}

/// File copy operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopy {
    /// Source path
    pub source: String,

    /// Destination path
    pub destination: String,

    /// Whether to create backup of destination
    #[serde(default = "default_true")]
    pub backup: bool,
}

/// Directory creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryCreate {
    /// Path to create
    pub path: String,

    /// Permissions (octal string like "0755")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// File permissions change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePermissions {
    /// Path to file/directory
    pub path: String,

    /// New mode (octal string like "0644")
    pub mode: String,

    /// Optional owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Optional group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Validation check to run after operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    /// Command to run
    pub command: String,

    /// Expected exit code
    #[serde(default)]
    pub expected_exit: i32,

    /// Optional expected output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
}

/// Information for undoing an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UndoInfo {
    /// File changes to restore
    FileChanges(Vec<FileChange>),

    /// Command to run for undo
    Command { command: String },

    /// Generic undo data
    Data(HashMap<String, serde_json::Value>),
}

/// Priority levels for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
            Priority::Info => "info",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Priority::Critical => "🔴",
            Priority::High => "🟠",
            Priority::Medium => "🟡",
            Priority::Low => "🟢",
            Priority::Info => "ℹ️",
        }
    }
}

/// Actions to perform after all operations complete
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PostApplyAction {
    /// Restart services
    ServiceRestart { services: Vec<String> },

    /// Run validation
    Validation {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_output: Option<String>,
    },

    /// Display message
    Message { message: String },

    /// Reboot required
    RebootRequired { reason: String },
}

impl FixPlan {
    /// Create a new empty plan
    #[allow(dead_code)]
    pub fn new(vm: String, profile: String) -> Self {
        Self {
            version: "1.0".to_string(),
            vm,
            generated: Utc::now(),
            profile,
            overall_risk: "unknown".to_string(),
            estimated_duration: "unknown".to_string(),
            metadata: PlanMetadata {
                author: "guestkit-profiles".to_string(),
                review_required: true,
                reversible: true,
                description: None,
                tags: Vec::new(),
            },
            operations: Vec::new(),
            post_apply: Vec::new(),
        }
    }

    /// Add an operation to the plan
    #[allow(dead_code)]
    pub fn add_operation(&mut self, operation: Operation) {
        self.operations.push(operation);
    }

    /// Get operations sorted by priority
    #[allow(dead_code)]
    pub fn operations_by_priority(&self) -> Vec<&Operation> {
        let mut ops: Vec<&Operation> = self.operations.iter().collect();
        ops.sort_by_key(|op| op.priority);
        ops
    }

    /// Get count by priority
    pub fn count_by_priority(&self, priority: Priority) -> usize {
        self.operations
            .iter()
            .filter(|op| op.priority == priority)
            .count()
    }

    /// Check if plan has any critical operations
    #[allow(dead_code)]
    pub fn has_critical(&self) -> bool {
        self.operations
            .iter()
            .any(|op| op.priority == Priority::Critical)
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_creation() {
        let plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());
        assert_eq!(plan.version, "1.0");
        assert_eq!(plan.vm, "test.qcow2");
        assert_eq!(plan.profile, "security");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical < Priority::High);
        assert!(Priority::High < Priority::Medium);
        assert!(Priority::Medium < Priority::Low);
    }

    #[test]
    fn test_priority_as_str() {
        assert_eq!(Priority::Critical.as_str(), "critical");
        assert_eq!(Priority::High.as_str(), "high");
        assert_eq!(Priority::Medium.as_str(), "medium");
        assert_eq!(Priority::Low.as_str(), "low");
        assert_eq!(Priority::Info.as_str(), "info");
    }

    #[test]
    fn test_priority_emoji() {
        assert_eq!(Priority::Critical.emoji(), "🔴");
        assert_eq!(Priority::High.emoji(), "🟠");
        assert_eq!(Priority::Medium.emoji(), "🟡");
        assert_eq!(Priority::Low.emoji(), "🟢");
        assert_eq!(Priority::Info.emoji(), "ℹ️");
    }

    #[test]
    fn test_plan_metadata_creation() {
        let metadata = PlanMetadata {
            author: "guestkit".to_string(),
            review_required: true,
            reversible: true,
            description: Some("Test plan".to_string()),
            tags: vec!["security".to_string(), "hardening".to_string()],
        };

        assert_eq!(metadata.author, "guestkit");
        assert!(metadata.review_required);
        assert!(metadata.reversible);
        assert_eq!(metadata.tags.len(), 2);
    }

    #[test]
    fn test_operation_creation() {
        let op = Operation {
            id: "op-001".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "systemctl restart sshd".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::High,
            description: "Restart SSH service".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        };

        assert_eq!(op.id, "op-001");
        assert_eq!(op.priority, Priority::High);
        assert!(op.reversible);
    }

    #[test]
    fn test_file_edit_operation() {
        let file_edit = FileEdit {
            file: "/etc/ssh/sshd_config".to_string(),
            backup: true,
            changes: vec![FileChange {
                line: 15,
                before: "PermitRootLogin yes".to_string(),
                after: "PermitRootLogin no".to_string(),
                context: None,
            }],
        };

        assert_eq!(file_edit.file, "/etc/ssh/sshd_config");
        assert!(file_edit.backup);
        assert_eq!(file_edit.changes.len(), 1);
    }

    #[test]
    fn test_package_install_operation() {
        let pkg_install = PackageInstall {
            packages: vec!["fail2ban".to_string(), "aide".to_string()],
            estimated_size: Some("10MB".to_string()),
            host_cache: None,
        };

        assert_eq!(pkg_install.packages.len(), 2);
        assert!(pkg_install.packages.contains(&"fail2ban".to_string()));
        assert_eq!(pkg_install.estimated_size, Some("10MB".to_string()));
    }

    #[test]
    fn test_service_operation() {
        let service_op = ServiceOperation {
            service: "firewalld".to_string(),
            state: Some("enabled".to_string()),
            start: true,
            restart: false,
        };

        assert_eq!(service_op.service, "firewalld");
        assert_eq!(service_op.state, Some("enabled".to_string()));
        assert!(service_op.start);
    }

    #[test]
    fn test_selinux_mode() {
        let selinux = SELinuxMode {
            file: "/etc/selinux/config".to_string(),
            current: "permissive".to_string(),
            target: "enforcing".to_string(),
            warning: None,
        };

        assert_eq!(selinux.file, "/etc/selinux/config");
        assert_eq!(selinux.current, "permissive");
        assert_eq!(selinux.target, "enforcing");
    }

    #[test]
    fn test_file_change_structure() {
        let change = FileChange {
            line: 42,
            before: "old value".to_string(),
            after: "new value".to_string(),
            context: Some("# Configuration".to_string()),
        };

        assert_eq!(change.line, 42);
        assert_eq!(change.before, "old value");
        assert_eq!(change.after, "new value");
        assert!(change.context.is_some());
    }

    #[test]
    fn test_post_apply_action_message() {
        let action = PostApplyAction::Message {
            message: "Configuration updated".to_string(),
        };

        match action {
            PostApplyAction::Message { message } => {
                assert_eq!(message, "Configuration updated");
            }
            _ => panic!("Expected Message variant"),
        }
    }

    #[test]
    fn test_post_apply_action_reboot() {
        let action = PostApplyAction::RebootRequired {
            reason: "Kernel update".to_string(),
        };

        match action {
            PostApplyAction::RebootRequired { reason } => {
                assert_eq!(reason, "Kernel update");
            }
            _ => panic!("Expected RebootRequired variant"),
        }
    }

    #[test]
    fn test_validation_check() {
        let validation = ValidationCheck {
            command: "systemctl is-active sshd".to_string(),
            expected_exit: 0,
            expected_output: None,
        };

        assert_eq!(validation.expected_exit, 0);
        assert!(validation.expected_output.is_none());
    }

    #[test]
    fn test_undo_info_command() {
        let undo = UndoInfo::Command {
            command: "systemctl stop service".to_string(),
        };

        match undo {
            UndoInfo::Command { command } => {
                assert_eq!(command, "systemctl stop service");
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[test]
    fn test_command_exec() {
        let cmd = CommandExec {
            command: "apt-get update".to_string(),
            expected_exit: 0,
            timeout: Some(300),
            interpreter: None,
        };

        assert_eq!(cmd.command, "apt-get update");
        assert_eq!(cmd.expected_exit, 0);
        assert_eq!(cmd.timeout, Some(300));
    }

    #[test]
    fn test_file_copy() {
        let copy = FileCopy {
            source: "/etc/default/sshd".to_string(),
            destination: "/etc/ssh/sshd_config".to_string(),
            backup: true,
        };

        assert_eq!(copy.source, "/etc/default/sshd");
        assert_eq!(copy.destination, "/etc/ssh/sshd_config");
        assert!(copy.backup);
    }

    #[test]
    fn test_directory_create() {
        let dir = DirectoryCreate {
            path: "/var/log/audit".to_string(),
            mode: Some("0755".to_string()),
        };

        assert_eq!(dir.path, "/var/log/audit");
        assert_eq!(dir.mode, Some("0755".to_string()));
    }

    #[test]
    fn test_file_permissions() {
        let perms = FilePermissions {
            path: "/etc/shadow".to_string(),
            mode: "0000".to_string(),
            owner: Some("root".to_string()),
            group: Some("root".to_string()),
        };

        assert_eq!(perms.path, "/etc/shadow");
        assert_eq!(perms.mode, "0000");
        assert!(perms.owner.is_some());
        assert!(perms.group.is_some());
    }

    #[test]
    fn test_registry_edit() {
        use serde_json::json;

        let reg = RegistryEdit {
            key: "HKLM\\SOFTWARE\\Test".to_string(),
            value: "Setting".to_string(),
            current_data: json!("Disabled"),
            new_data: json!("Enabled"),
            data_type: "REG_SZ".to_string(),
        };

        assert_eq!(reg.key, "HKLM\\SOFTWARE\\Test");
        assert_eq!(reg.value, "Setting");
        assert_eq!(reg.data_type, "REG_SZ");
    }

    #[test]
    fn test_operation_with_dependencies() {
        let op = Operation {
            id: "op-002".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo test".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Medium,
            description: "Test operation".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec!["op-001".to_string()],
            validation: None,
            undo: None,
        };

        assert_eq!(op.depends_on.len(), 1);
        assert_eq!(op.depends_on[0], "op-001");
    }

    #[test]
    fn test_operation_with_validation() {
        let op = Operation {
            id: "op-003".to_string(),
            op_type: OperationType::ServiceOperation(ServiceOperation {
                service: "nginx".to_string(),
                state: Some("enabled".to_string()),
                start: false,
                restart: true,
            }),
            priority: Priority::High,
            description: "Restart nginx".to_string(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: Some(ValidationCheck {
                command: "systemctl is-active nginx".to_string(),
                expected_exit: 0,
                expected_output: Some("active".to_string()),
            }),
            undo: None,
        };

        assert!(op.validation.is_some());
        let validation = op.validation.unwrap();
        assert_eq!(validation.expected_output, Some("active".to_string()));
    }
}
