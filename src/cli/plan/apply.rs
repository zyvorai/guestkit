// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Plan application - executes fix plans with safety checks

use super::types::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Applies fix plans to VM disks (offline, via guestfs — Unix host only).
#[cfg(not(target_os = "windows"))]
pub struct PlanApplicator {
    vm_path: String,
    dry_run: bool,
    /// When true, skip the full-image `std::fs::copy` backup. Intended for
    /// low-risk registry-only plans (e.g. enable RDP) where copying a 30–40 GiB
    /// Windows golden is slower and riskier than the edit itself.
    skip_backup: bool,
}

#[cfg(not(target_os = "windows"))]
impl PlanApplicator {
    /// Create a new plan applicator (full-image backup before apply).
    pub fn new(vm_path: String, dry_run: bool) -> Self {
        Self {
            vm_path,
            dry_run,
            skip_backup: false,
        }
    }

    /// Skip the full-disk backup taken before apply.
    pub fn skip_backup(mut self, skip: bool) -> Self {
        self.skip_backup = skip;
        self
    }

    /// Apply a fix plan
    pub fn apply(&self, plan: &FixPlan) -> Result<ApplyResult> {
        if self.dry_run {
            return Ok(ApplyResult {
                success: true,
                operations_applied: 0,
                operations_failed: 0,
                operations_skipped: plan.operations.len(),
                message: "Dry run completed - no changes made".to_string(),
                outcomes: Vec::new(),
                rollback_dir: None,
            });
        }

        // Check VM exists
        if !Path::new(&self.vm_path).exists() {
            return Ok(ApplyResult {
                success: false,
                operations_applied: 0,
                operations_failed: 0,
                operations_skipped: plan.operations.len(),
                message: format!("VM disk not found: {}", self.vm_path),
                outcomes: Vec::new(),
                rollback_dir: None,
            });
        }

        // Create backup — or skip when the caller opts in (registry-only / CI).
        let backup_path = if self.skip_backup {
            eprintln!("Backup skipped (--skip-backup)");
            None
        } else {
            match self.create_backup() {
                Ok(p) => {
                    eprintln!("Backup created: {}", p);
                    Some(p)
                }
                Err(e) => {
                    return Ok(ApplyResult {
                        success: false,
                        operations_applied: 0,
                        operations_failed: 1,
                        operations_skipped: plan.operations.len(),
                        message: format!("Failed to create backup, refusing to apply plan: {}", e),
                        outcomes: Vec::new(),
                        rollback_dir: None,
                    });
                }
            }
        };

        // Open VM with Guestfs
        let mut g = match crate::guestfs::Guestfs::new() {
            Ok(g) => g,
            Err(e) => {
                return Ok(ApplyResult {
                    success: false,
                    operations_applied: 0,
                    operations_failed: 1,
                    operations_skipped: plan.operations.len(),
                    message: format!("Failed to create Guestfs handle: {}", e),
                    outcomes: Vec::new(),
                    rollback_dir: None,
                });
            }
        };

        if let Err(e) = g.add_drive(&self.vm_path) {
            let _ = g.shutdown();
            return Ok(ApplyResult {
                success: false,
                operations_applied: 0,
                operations_failed: 1,
                operations_skipped: plan.operations.len(),
                message: format!("Failed to add drive: {}", e),
                outcomes: Vec::new(),
                rollback_dir: None,
            });
        }

        if let Err(e) = g.launch() {
            let _ = g.shutdown();
            return Ok(ApplyResult {
                success: false,
                operations_applied: 0,
                operations_failed: 1,
                operations_skipped: plan.operations.len(),
                message: format!("Failed to launch Guestfs: {}", e),
                outcomes: Vec::new(),
                rollback_dir: None,
            });
        }

        // Mount root filesystem
        let roots = g.inspect_os()?;
        if !roots.is_empty() {
            let root = &roots[0];
            if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
                // Mount in order of path length (shortest first = mount parents before children)
                let mut mounts: Vec<_> = mountpoints.into_iter().collect();
                mounts.sort_by_key(|(mount, _)| mount.len());
                for (mount, device) in &mounts {
                    // Force-off / fast-startup leaves NTFS dirty; ntfs-3g then
                    // mounts RO and hive download/upload silently fails (0 ops).
                    // Same repair path as agent-inject --windows.
                    if g.vfs_type(device).ok().as_deref() == Some("ntfs") {
                        if let Err(e) = g.ntfsfix_opts(device, false, true) {
                            log::warn!("ntfsfix {device} before plan apply: {e}");
                        }
                    }
                    if let Err(e) = g.mount(device, mount) {
                        log::warn!("Failed to mount {} at {}: {}", device, mount, e);
                    }
                }
            }
        } else {
            // No OS found - refuse to blindly mount without validation
            let _ = g.shutdown();
            return Ok(ApplyResult {
                success: false,
                operations_applied: 0,
                operations_failed: 1,
                operations_skipped: plan.operations.len(),
                message:
                    "No operating system detected in VM disk. Cannot apply plan without a valid OS."
                        .to_string(),
                outcomes: Vec::new(),
                rollback_dir: None,
            });
        }

        // Topological sort of operations
        let sorted_ops = super::topo_sort::topological_sort(plan);

        let mut applied = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        for op in &sorted_ops {
            match self.apply_operation(&mut g, op) {
                Ok(true) => applied += 1,
                Ok(false) => skipped += 1,
                Err(e) => {
                    eprintln!("Operation {} failed: {}", op.id, e);
                    failed += 1;
                }
            }
        }

        // Ensure cleanup happens regardless of operation outcomes
        if let Err(e) = g.umount_all() {
            log::warn!("Failed to unmount filesystems during cleanup: {}", e);
        }
        if let Err(e) = g.shutdown() {
            log::warn!("Failed to shutdown guestfs handle during cleanup: {}", e);
        }

        let success = failed == 0;
        let message = if success {
            format!("Plan applied successfully ({} operations)", applied)
        } else {
            // If there were failures and we have a backup, note it
            if let Some(ref bp) = backup_path {
                format!(
                    "{} operations applied, {} failed. Backup available at: {}",
                    applied, failed, bp
                )
            } else {
                format!("{} operations applied, {} failed", applied, failed)
            }
        };

        Ok(ApplyResult {
            success,
            operations_applied: applied,
            operations_failed: failed,
            operations_skipped: skipped,
            message,
            outcomes: Vec::new(),
            rollback_dir: None,
        })
    }

    /// Apply a single operation, returning Ok(true) for applied, Ok(false) for skipped
    fn apply_operation(&self, g: &mut crate::guestfs::Guestfs, op: &Operation) -> Result<bool> {
        match &op.op_type {
            OperationType::FileEdit(fe) => {
                let content = g
                    .cat(&fe.file)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", fe.file, e))?;
                let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

                for change in &fe.changes {
                    // Find and replace matching lines
                    let mut found = false;
                    for line in lines.iter_mut() {
                        if line.trim() == change.before.trim() {
                            *line = change.after.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        // Try line number if specified and valid
                        if change.line > 0 && change.line <= lines.len() {
                            lines[change.line - 1] = change.after.clone();
                        }
                    }
                }

                let new_content = lines.join("\n") + "\n";
                let temp = tempfile::NamedTempFile::new()?;
                std::fs::write(temp.path(), new_content.as_bytes())?;
                g.upload(
                    temp.path()
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Temp file path contains invalid UTF-8"))?,
                    &fe.file,
                )
                .map_err(|e| anyhow::anyhow!("Failed to upload {}: {}", fe.file, e))?;

                Ok(true)
            }
            OperationType::FileWrite(fw) => {
                let parent = std::path::Path::new(&fw.path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("/");
                if parent != "/" && !parent.is_empty() {
                    g.mkdir_p(parent)
                        .map_err(|e| anyhow::anyhow!("mkdir_p failed for {}: {}", parent, e))?;
                }
                let content = if fw.content.ends_with('\n') {
                    fw.content.clone()
                } else {
                    format!("{}\n", fw.content)
                };
                g.write(&fw.path, content.as_bytes())
                    .map_err(|e| anyhow::anyhow!("write failed for {}: {}", fw.path, e))?;
                if let Some(ref mode_str) = fw.mode {
                    let mode = i32::from_str_radix(mode_str, 8).unwrap_or(0o644);
                    let _ = g.chmod(mode, &fw.path);
                }
                Ok(true)
            }
            OperationType::Symlink(sl) => {
                let parent = std::path::Path::new(&sl.link_path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("/");
                if parent != "/" && !parent.is_empty() {
                    g.mkdir_p(parent)
                        .map_err(|e| anyhow::anyhow!("mkdir_p failed for {}: {}", parent, e))?;
                }
                // Replace existing link/file so re-apply is idempotent.
                let _ = g.rm(&sl.link_path);
                g.ln_sf(&sl.target, &sl.link_path).map_err(|e| {
                    anyhow::anyhow!("ln_sf failed {} -> {}: {}", sl.target, sl.link_path, e)
                })?;
                Ok(true)
            }
            OperationType::FileDelete(fd) => {
                let exists =
                    g.exists(&fd.path).unwrap_or(false) || g.is_symlink(&fd.path).unwrap_or(false);
                if !exists {
                    if fd.missing_ok {
                        return Ok(true);
                    }
                    anyhow::bail!("File to delete not found: {}", fd.path);
                }
                g.rm(&fd.path)
                    .map_err(|e| anyhow::anyhow!("rm failed for {}: {}", fd.path, e))?;
                Ok(true)
            }
            OperationType::CommandExec(ce) => {
                crate::cli::plan::firstboot_stage::apply_or_stage_command(g, ce)
            }
            OperationType::FilePermissions(fp) => {
                let mode_str = if fp.mode.is_empty() { "0" } else { &fp.mode };
                let mode = i32::from_str_radix(mode_str, 8).map_err(|_| {
                    anyhow::anyhow!(
                        "Invalid octal permission mode '{}' for {}",
                        fp.mode,
                        fp.path
                    )
                })?;
                if !(0..=0o7777).contains(&mode) {
                    anyhow::bail!(
                        "Permission mode '{}' out of range (0000-7777) for {}",
                        fp.mode,
                        fp.path
                    );
                }
                g.chmod(mode, &fp.path)
                    .map_err(|e| anyhow::anyhow!("chmod failed on {}: {}", fp.path, e))?;
                Ok(true)
            }
            OperationType::DirectoryCreate(dc) => {
                g.mkdir_p(&dc.path)
                    .map_err(|e| anyhow::anyhow!("mkdir_p failed for {}: {}", dc.path, e))?;
                if let Some(ref mode_str) = dc.mode {
                    let mode = i32::from_str_radix(mode_str, 8).unwrap_or(0o755);
                    let _ = g.chmod(mode, &dc.path);
                }
                Ok(true)
            }
            OperationType::FileCopy(fc) => {
                g.cp_a(&fc.source, &fc.destination).map_err(|e| {
                    anyhow::anyhow!("cp_a failed {} -> {}: {}", fc.source, fc.destination, e)
                })?;
                Ok(true)
            }
            OperationType::SelinuxMode(sm) => {
                if let Ok(content) = g.cat(&sm.file) {
                    // Replace only non-comment lines starting with SELINUX=
                    let new_content: String = content
                        .lines()
                        .map(|line| {
                            let trimmed = line.trim_start();
                            if !trimmed.starts_with('#')
                                && trimmed.starts_with(&format!("SELINUX={}", sm.current))
                            {
                                format!("SELINUX={}", sm.target)
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n";
                    let temp = tempfile::NamedTempFile::new()?;
                    std::fs::write(temp.path(), new_content.as_bytes())?;
                    g.upload(
                        temp.path().to_str().ok_or_else(|| {
                            anyhow::anyhow!("Temp file path contains invalid UTF-8")
                        })?,
                        &sm.file,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to upload {}: {}", sm.file, e))?;
                    Ok(true)
                } else {
                    eprintln!("Warning: SELinux config file not found: {}", sm.file);
                    Ok(false)
                }
            }
            OperationType::PackageInstall(pi) => {
                crate::cli::plan::package_stage::stage_packages_offline(g, pi)
            }
            OperationType::ServiceOperation(so) => {
                crate::cli::plan::firstboot_stage::stage_service_offline(g, so)
            }
            OperationType::RegistryEdit(re) => self.apply_registry_edit(g, re),
            OperationType::DriverInject(di) => self.apply_driver_inject(g, di),
        }
    }

    #[cfg(all(feature = "registry-write", feature = "agent"))]
    fn apply_driver_inject(
        &self,
        g: &mut crate::guestfs::Guestfs,
        di: &crate::cli::plan::types::DriverInject,
    ) -> Result<bool> {
        let host_dir = di.resolve_host_dir().ok_or_else(|| {
            anyhow::anyhow!(
                "DriverInject '{}': set host_dir, or source to a host directory, \
                 or GUESTKIT_VIRTIO_WIN pointing at a virtio-win tree",
                di.driver_name
            )
        })?;
        let roots = g
            .inspect_os()
            .map_err(|e| anyhow::anyhow!("inspect_os: {e}"))?;
        let root = roots
            .first()
            .ok_or_else(|| anyhow::anyhow!("no OS root for driver inject"))?;
        crate::agent::inject::inject_windows_driver_dir(g, root, &host_dir, &di.driver_name, false)
            .with_context(|| {
                format!(
                    "offline DriverInject for {} from {}",
                    di.driver_name,
                    host_dir.display()
                )
            })?;
        Ok(true)
    }

    #[cfg(not(all(feature = "registry-write", feature = "agent")))]
    fn apply_driver_inject(
        &self,
        _g: &mut crate::guestfs::Guestfs,
        di: &crate::cli::plan::types::DriverInject,
    ) -> Result<bool> {
        eprintln!(
            "Warning: Driver injection for {} skipped — rebuild with \
             `--features registry-write,agent` and set host_dir / GUESTKIT_VIRTIO_WIN",
            di.driver_name
        );
        Ok(false)
    }

    /// Apply a Windows registry edit to an offline hive via libhivex.
    #[cfg(feature = "registry-write")]
    fn apply_registry_edit(
        &self,
        g: &mut crate::guestfs::Guestfs,
        re: &RegistryEdit,
    ) -> Result<bool> {
        let (hive_path, subpath) = self.resolve_registry_hive(g, &re.key)?;

        // Pull the hive out to the host, mutate it, push it back.
        let temp = tempfile::NamedTempFile::new()?;
        let host_path = temp
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Temp hive path contains invalid UTF-8"))?;
        g.download_hive(&hive_path, host_path)
            .map_err(|e| anyhow::anyhow!("Failed to download hive {}: {}", hive_path, e))?;

        crate::guestfs::hivex_ffi::set_registry_value(
            temp.path(),
            &subpath,
            &re.value,
            &re.data_type,
            &re.new_data,
        )
        .with_context(|| format!("Registry edit failed for {}", re.key))?;

        g.upload_hive(host_path, &hive_path)
            .map_err(|e| anyhow::anyhow!("Failed to upload hive {}: {}", hive_path, e))?;
        Ok(true)
    }

    /// Without the `registry-write` feature we cannot mutate hives offline.
    #[cfg(not(feature = "registry-write"))]
    fn apply_registry_edit(
        &self,
        _g: &mut crate::guestfs::Guestfs,
        re: &RegistryEdit,
    ) -> Result<bool> {
        eprintln!(
            "Warning: Registry edit ({}) skipped — rebuild with `--features registry-write` for offline hive writes",
            re.key
        );
        Ok(false)
    }

    /// Map a `HKLM\<hive>\...` key to the guest hive file path and the key
    /// components below the hive root.
    #[cfg(feature = "registry-write")]
    fn resolve_registry_hive(
        &self,
        g: &mut crate::guestfs::Guestfs,
        key: &str,
    ) -> Result<(String, Vec<String>)> {
        let normalized = key.replace('/', "\\");
        let parts: Vec<&str> = normalized.split('\\').filter(|s| !s.is_empty()).collect();
        if parts.len() < 2 {
            anyhow::bail!("Registry key too short to resolve a hive: {key}");
        }

        let root_key = parts[0].to_ascii_uppercase();
        if !matches!(root_key.as_str(), "HKLM" | "HKEY_LOCAL_MACHINE") {
            anyhow::bail!(
                "Offline registry writes only support HKLM keys (got {})",
                parts[0]
            );
        }

        let roots = g.inspect_os()?;
        let root = roots
            .first()
            .ok_or_else(|| anyhow::anyhow!("No OS root detected for registry write"))?
            .clone();

        let hive_name = parts[1].to_ascii_uppercase();
        let hive_path = match hive_name.as_str() {
            "SOFTWARE" => g.inspect_get_windows_software_hive(&root)?,
            "SYSTEM" => g.inspect_get_windows_system_hive(&root)?,
            "SAM" | "SECURITY" | "DEFAULT" | "COMPONENTS" => {
                let systemroot = g.inspect_get_windows_systemroot(&root)?;
                format!("{systemroot}/System32/config/{hive_name}")
            }
            other => anyhow::bail!("Unsupported registry hive: {other}"),
        };

        let subpath: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
        Ok((hive_path, subpath))
    }
}

#[cfg(not(target_os = "windows"))]
impl PlanApplicator {
    /// Parse a command string into arguments, handling single and double quotes.
    /// This is a safe alternative to split_whitespace which doesn't handle quoting.
    #[allow(dead_code)] // kept for live/export callers and unit tests
    fn parse_shell_words(input: &str) -> Result<Vec<String>> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut chars = input.chars().peekable();
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                ' ' | '\t' if !in_single_quote && !in_double_quote => {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                }
                '\\' if !in_single_quote => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if in_single_quote || in_double_quote {
            anyhow::bail!("Unterminated quote in command: {}", input);
        }

        if !current.is_empty() {
            args.push(current);
        }

        Ok(args)
    }

    /// Validate a plan before applying
    pub fn validate(&self, plan: &FixPlan) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check VM exists
        if !Path::new(&self.vm_path).exists() {
            errors.push(format!("VM disk not found: {}", self.vm_path));
        }

        // Check for circular dependencies
        if self.has_circular_dependencies(plan) {
            errors.push("Plan contains circular dependencies".to_string());
        }

        // Check for missing dependencies
        for op in &plan.operations {
            for dep_id in &op.depends_on {
                if !plan.operations.iter().any(|o| &o.id == dep_id) {
                    errors.push(format!(
                        "Operation {} depends on non-existent operation {}",
                        op.id, dep_id
                    ));
                }
            }
        }

        // Warn about non-reversible operations
        let non_reversible: Vec<&str> = plan
            .operations
            .iter()
            .filter(|op| !op.reversible)
            .map(|op| op.id.as_str())
            .collect();

        if !non_reversible.is_empty() {
            warnings.push(format!(
                "Non-reversible operations: {}",
                non_reversible.join(", ")
            ));
        }

        Ok(ValidationResult { errors, warnings })
    }

    /// Check for circular dependencies using Kahn's algorithm
    fn has_circular_dependencies(&self, plan: &FixPlan) -> bool {
        use std::collections::{HashMap, HashSet, VecDeque};

        if plan.operations.is_empty() {
            return false;
        }

        // Collect valid operation IDs
        let valid_ids: HashSet<&str> = plan.operations.iter().map(|op| op.id.as_str()).collect();

        // Build adjacency list and in-degree map (only for existing operations)
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for op in &plan.operations {
            in_degree.entry(op.id.as_str()).or_insert(0);
            adj.entry(op.id.as_str()).or_default();
        }

        for op in &plan.operations {
            for dep_id in &op.depends_on {
                // Only count edges from dependencies that actually exist in the plan
                if valid_ids.contains(dep_id.as_str()) {
                    adj.entry(dep_id.as_str()).or_default().push(op.id.as_str());
                    *in_degree.entry(op.id.as_str()).or_insert(0) += 1;
                }
            }
        }

        // Start with nodes that have no incoming edges
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&node, _)| node)
            .collect();

        let mut visited = 0usize;

        while let Some(node) = queue.pop_front() {
            visited += 1;
            if let Some(neighbors) = adj.get(node) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        // If not all nodes were visited, there's a cycle
        visited != plan.operations.len()
    }

    /// Create backup before applying plan
    #[allow(dead_code)]
    fn create_backup(&self) -> Result<String> {
        let vm = Path::new(&self.vm_path);
        if !vm.exists() {
            anyhow::bail!("VM disk not found: {}", self.vm_path);
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let stem = vm.file_stem().unwrap_or_default().to_string_lossy();
        let ext = vm
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let backup_name = format!("{}.backup_{}{}", stem, timestamp, ext);

        let backup_path = vm
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&backup_name);

        std::fs::copy(&self.vm_path, &backup_path)
            .with_context(|| format!("Failed to create backup: {}", backup_path.display()))?;

        Ok(backup_path.to_string_lossy().to_string())
    }

    /// Rollback to a previous state
    pub fn rollback(&self, backup_path: &str) -> Result<()> {
        let backup = Path::new(backup_path);
        let vm = Path::new(&self.vm_path);

        if !backup.exists() {
            anyhow::bail!("Backup file not found: {}", backup_path);
        }

        if !vm.exists() {
            anyhow::bail!("VM disk not found: {}", self.vm_path);
        }

        std::fs::copy(backup, vm).with_context(|| {
            format!(
                "Failed to restore backup from {} to {}",
                backup_path, self.vm_path
            )
        })?;

        Ok(())
    }
}

/// Per-operation execution record (the spec's six repair artifacts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationOutcome {
    pub op_id: String,
    pub status: OpStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_state: Option<String>,
    pub planned_change: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationOutcome>,
    pub rollback_procedure: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpStatus {
    Applied,
    Skipped,
    Failed,
    RolledBack,
    DryRun,
    NotReached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutcome {
    pub command: String,
    pub exit_code: Option<i32>,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Result of applying a plan
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyResult {
    pub success: bool,
    pub operations_applied: usize,
    pub operations_failed: usize,
    pub operations_skipped: usize,
    pub message: String,
    /// Per-operation execution records (live executor; empty from the
    /// offline guestfs applicator, which relies on full-image backup).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<OperationOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_dir: Option<String>,
}

/// Result of validating a plan
#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Returns true if no errors were found
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_temp_vm() -> (TempDir, String) {
        let dir = TempDir::new().unwrap();
        let vm_path = dir.path().join("test.qcow2");
        let mut file = fs::File::create(&vm_path).unwrap();
        file.write_all(b"fake vm disk").unwrap();
        (
            dir,
            vm_path
                .to_str()
                .expect("Test path contains invalid UTF-8")
                .to_string(),
        )
    }

    #[test]
    fn test_applicator_creation() {
        let applicator = PlanApplicator::new("test.qcow2".to_string(), true);
        assert_eq!(applicator.vm_path, "test.qcow2");
        assert!(applicator.dry_run);
        assert!(!applicator.skip_backup);
    }

    #[test]
    fn test_applicator_skip_backup_builder() {
        let applicator = PlanApplicator::new("vm.qcow2".to_string(), false).skip_backup(true);
        assert!(applicator.skip_backup);
        assert!(!applicator.dry_run);
    }

    #[test]
    fn test_applicator_creation_non_dry_run() {
        let applicator = PlanApplicator::new("vm.qcow2".to_string(), false);
        assert_eq!(applicator.vm_path, "vm.qcow2");
        assert!(!applicator.dry_run);
    }

    #[test]
    fn test_dry_run() {
        let applicator = PlanApplicator::new("test.qcow2".to_string(), true);
        let plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());
        let result = applicator.apply(&plan).unwrap();
        assert!(result.success);
        assert_eq!(result.operations_applied, 0);
    }

    #[test]
    fn test_dry_run_with_operations() {
        let applicator = PlanApplicator::new("test.qcow2".to_string(), true);
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-001".to_string(),
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
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        let result = applicator.apply(&plan).unwrap();
        assert!(result.success);
        assert_eq!(result.operations_applied, 0);
        assert_eq!(result.operations_skipped, 1);
        assert!(result.message.contains("Dry run"));
    }

    #[test]
    fn test_validate_vm_exists() {
        let (_dir, vm_path) = create_temp_vm();
        let applicator = PlanApplicator::new(vm_path.clone(), true);
        let plan = FixPlan::new(vm_path, "security".to_string());

        let result = applicator.validate(&plan).unwrap();
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_vm_not_found() {
        let applicator = PlanApplicator::new("/nonexistent/vm.qcow2".to_string(), true);
        let plan = FixPlan::new("/nonexistent/vm.qcow2".to_string(), "security".to_string());

        let result = applicator.validate(&plan).unwrap();
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("VM disk not found"));
    }

    #[test]
    fn test_validate_missing_dependency() {
        let (_dir, vm_path) = create_temp_vm();
        let applicator = PlanApplicator::new(vm_path.clone(), true);
        let mut plan = FixPlan::new(vm_path, "security".to_string());

        plan.add_operation(Operation {
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
            depends_on: vec!["op-001".to_string()], // Non-existent dependency
            validation: None,
            undo: None,
        });

        let result = applicator.validate(&plan).unwrap();
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("depends on non-existent operation"));
    }

    #[test]
    fn test_validate_non_reversible_warning() {
        let (_dir, vm_path) = create_temp_vm();
        let applicator = PlanApplicator::new(vm_path.clone(), true);
        let mut plan = FixPlan::new(vm_path, "security".to_string());

        plan.add_operation(Operation {
            id: "op-001".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "rm -rf /data".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::High,
            description: "Delete data".to_string(),
            risk: Priority::High,
            reversible: false, // Not reversible
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        let result = applicator.validate(&plan).unwrap();
        assert!(result.is_valid());
        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("Non-reversible operations"));
    }

    #[test]
    fn test_validate_valid_dependencies() {
        let (_dir, vm_path) = create_temp_vm();
        let applicator = PlanApplicator::new(vm_path.clone(), true);
        let mut plan = FixPlan::new(vm_path, "security".to_string());

        plan.add_operation(Operation {
            id: "op-001".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo first".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Medium,
            description: "First operation".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        plan.add_operation(Operation {
            id: "op-002".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "echo second".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Medium,
            description: "Second operation".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec!["op-001".to_string()], // Valid dependency
            validation: None,
            undo: None,
        });

        let result = applicator.validate(&plan).unwrap();
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_apply_result_structure() {
        let result = ApplyResult {
            success: true,
            operations_applied: 5,
            operations_failed: 0,
            operations_skipped: 2,
            message: "All operations completed".to_string(),
            outcomes: Vec::new(),
            rollback_dir: None,
        };

        assert!(result.success);
        assert_eq!(result.operations_applied, 5);
        assert_eq!(result.operations_failed, 0);
        assert_eq!(result.operations_skipped, 2);
    }

    #[test]
    fn test_validation_result_structure() {
        let result = ValidationResult {
            errors: vec![],
            warnings: vec!["Warning message".to_string()],
        };

        assert!(result.is_valid());
        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_rollback_missing_backup() {
        let applicator = PlanApplicator::new("test.qcow2".to_string(), false);
        let result = applicator.rollback("/nonexistent/backup/path");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Backup file not found"));
    }

    #[test]
    fn test_rollback_with_valid_files() {
        let dir = TempDir::new().unwrap();
        let vm_path = dir.path().join("test.qcow2");
        let backup_path = dir.path().join("test.backup.qcow2");
        fs::write(&vm_path, b"current state").unwrap();
        fs::write(&backup_path, b"backup state").unwrap();

        let applicator = PlanApplicator::new(
            vm_path
                .to_str()
                .expect("Test path contains invalid UTF-8")
                .to_string(),
            false,
        );
        applicator
            .rollback(
                backup_path
                    .to_str()
                    .expect("Test path contains invalid UTF-8"),
            )
            .unwrap();

        let content = fs::read(&vm_path).unwrap();
        assert_eq!(content, b"backup state");
    }

    #[test]
    fn test_apply_non_dry_run_missing_vm() {
        let applicator = PlanApplicator::new("/nonexistent/vm.qcow2".to_string(), false);
        let plan = FixPlan::new("/nonexistent/vm.qcow2".to_string(), "security".to_string());

        let result = applicator.apply(&plan).unwrap();
        assert!(!result.success);
        assert!(result.message.contains("VM disk not found") || result.message.contains("Failed"));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let applicator = PlanApplicator::new("/nonexistent/vm.qcow2".to_string(), true);
        let mut plan = FixPlan::new("/nonexistent/vm.qcow2".to_string(), "security".to_string());

        // Add operation with missing dependency
        plan.add_operation(Operation {
            id: "op-002".to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "test".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Medium,
            description: "Test".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec!["missing-op".to_string()],
            validation: None,
            undo: None,
        });

        let result = applicator.validate(&plan).unwrap();
        assert!(!result.is_valid());
        // Should have both VM not found and missing dependency errors
        assert!(result.errors.len() >= 2);
    }

    #[test]
    fn test_validate_empty_plan() {
        let (_dir, vm_path) = create_temp_vm();
        let applicator = PlanApplicator::new(vm_path.clone(), true);
        let plan = FixPlan::new(vm_path, "security".to_string());

        let result = applicator.validate(&plan).unwrap();
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }
}
