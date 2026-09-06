// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! In-guest fix plan executor (native, no guestfs).
//!
//! Migration-grade guarantees: every operation records its before-state,
//! planned change, backup location, validation outcome, and rollback
//! procedure; per-operation `ValidationCheck`s actually run; `fail_fast`
//! stops at the first failure and `auto_rollback` restores the snapshots
//! taken so far. `root` is configurable so tests can run against a
//! temporary directory instead of the live filesystem.

use super::apply::{ApplyResult, OpStatus, OperationOutcome, ValidationOutcome};
use super::topo_sort::topological_sort;
use super::types::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LivePlanExecutor {
    dry_run: bool,
    root: PathBuf,
    fail_fast: bool,
    auto_rollback: bool,
}

impl LivePlanExecutor {
    /// Legacy behavior: run everything, count failures at the end.
    pub fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            root: default_root(),
            fail_fast: false,
            auto_rollback: false,
        }
    }

    /// Migration-repair profile: stop on first failure and restore the
    /// snapshots taken so far.
    pub fn for_migration(dry_run: bool) -> Self {
        Self {
            dry_run,
            root: default_root(),
            fail_fast: true,
            auto_rollback: true,
        }
    }

    /// Re-root all paths (tests run against a temp dir).
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    pub fn with_auto_rollback(mut self, auto_rollback: bool) -> Self {
        self.auto_rollback = auto_rollback;
        self
    }

    /// Map an operation path (absolute guest path) under the configured root.
    fn resolve(&self, path: &str) -> PathBuf {
        if self.root == Path::new("/") {
            return PathBuf::from(path);
        }
        let stripped = path
            .trim_start_matches('/')
            .trim_start_matches(|c: char| {
                c.is_ascii_alphabetic() && path.len() > 1 && &path[1..2] == ":"
            })
            .trim_start_matches(":")
            .trim_start_matches('\\');
        self.root.join(stripped)
    }

    fn rollback_root(&self) -> PathBuf {
        if cfg!(windows) && self.root == Path::new("/") {
            PathBuf::from("C:\\ProgramData\\GuestKit\\rollback")
        } else if self.root == Path::new("/") {
            PathBuf::from("/var/lib/guestkit/rollback")
        } else {
            self.root.join("guestkit-rollback")
        }
    }

    pub fn apply(&self, plan: &FixPlan) -> Result<ApplyResult> {
        let plan_id = plan_id_for(plan);
        let rollback_dir = self.rollback_root().join(&plan_id);
        let sorted = topological_sort(plan);

        let mut outcomes: Vec<OperationOutcome> = Vec::with_capacity(sorted.len());

        if self.dry_run {
            for op in &sorted {
                outcomes.push(OperationOutcome {
                    op_id: op.id.clone(),
                    status: OpStatus::DryRun,
                    before_state: self.capture_before(op),
                    planned_change: describe_change(op),
                    executed_change: None,
                    backup_path: None,
                    validation: None,
                    rollback_procedure: describe_rollback(op),
                    error: None,
                });
            }
            return Ok(ApplyResult {
                success: true,
                operations_applied: 0,
                operations_failed: 0,
                operations_skipped: sorted.len(),
                message: "Dry run completed - no changes made".to_string(),
                outcomes,
                rollback_dir: None,
            });
        }

        fs::create_dir_all(&rollback_dir).context("create rollback directory")?;

        let mut applied = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;
        let mut aborted = false;

        for (idx, op) in sorted.iter().enumerate() {
            if aborted {
                outcomes.push(OperationOutcome {
                    op_id: op.id.clone(),
                    status: OpStatus::NotReached,
                    before_state: None,
                    planned_change: describe_change(op),
                    executed_change: None,
                    backup_path: None,
                    validation: None,
                    rollback_procedure: describe_rollback(op),
                    error: None,
                });
                skipped += 1;
                continue;
            }

            let before_state = self.capture_before(op);
            let result = self.apply_operation(op, &rollback_dir);
            let mut outcome = OperationOutcome {
                op_id: op.id.clone(),
                status: OpStatus::Skipped,
                before_state,
                planned_change: describe_change(op),
                executed_change: None,
                backup_path: backup_path_for(op, &rollback_dir),
                validation: None,
                rollback_procedure: describe_rollback(op),
                error: None,
            };

            match result {
                Ok(true) => {
                    let validation = self.run_op_validation(op);
                    let validation_failed = validation.as_ref().map(|v| !v.passed).unwrap_or(false);
                    outcome.validation = validation;
                    if validation_failed {
                        outcome.status = OpStatus::Failed;
                        outcome.error = Some("post-operation validation failed".to_string());
                        failed += 1;
                        log::error!("Operation {} failed validation", op.id);
                        if self.fail_fast {
                            aborted = true;
                        }
                    } else {
                        outcome.status = OpStatus::Applied;
                        outcome.executed_change = Some(describe_change(op));
                        applied += 1;
                    }
                }
                Ok(false) => {
                    outcome.status = OpStatus::Skipped;
                    skipped += 1;
                }
                Err(e) => {
                    log::error!("Operation {} failed: {}", op.id, e);
                    outcome.status = OpStatus::Failed;
                    outcome.error = Some(e.to_string());
                    failed += 1;
                    if self.fail_fast {
                        aborted = true;
                    }
                }
            }
            outcomes.push(outcome);
            let _ = idx;
        }

        let mut rolled_back = false;
        if failed > 0 && self.auto_rollback {
            match self.rollback(&plan_id) {
                Ok(_) => {
                    rolled_back = true;
                    for o in outcomes
                        .iter_mut()
                        .filter(|o| o.status == OpStatus::Applied)
                    {
                        o.status = OpStatus::RolledBack;
                    }
                }
                Err(e) => log::error!("auto-rollback failed: {e}"),
            }
        }

        if failed == 0 {
            self.run_post_apply(&plan.post_apply)?;
        }

        Ok(ApplyResult {
            success: failed == 0,
            operations_applied: applied,
            operations_failed: failed,
            operations_skipped: skipped,
            message: if failed == 0 {
                format!("Live plan applied ({applied} operations, rollback: {rollback_dir:?})")
            } else if rolled_back {
                format!("{applied} applied, {failed} failed — file snapshots restored")
            } else {
                format!("{applied} applied, {failed} failed")
            },
            outcomes,
            rollback_dir: Some(rollback_dir.display().to_string()),
        })
    }

    pub fn rollback(&self, plan_id: &str) -> Result<String> {
        let rollback_dir = self.rollback_root().join(plan_id);
        if !rollback_dir.is_dir() {
            anyhow::bail!("rollback snapshot not found: {plan_id}");
        }

        let manifest_path = rollback_dir.join("manifest.json");
        let manifest: RollbackManifest = if manifest_path.exists() {
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?
        } else {
            RollbackManifest { files: Vec::new() }
        };

        for entry in &manifest.files {
            if entry.backup_path.exists() {
                if let Some(parent) = Path::new(&entry.original_path).parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry.backup_path, &entry.original_path)?;
            } else if entry.removed {
                let _ = fs::remove_file(&entry.original_path);
            }
        }

        Ok(format!("Rollback complete for plan {plan_id}"))
    }

    /// Best-effort description of current state before mutating.
    fn capture_before(&self, op: &Operation) -> Option<String> {
        match &op.op_type {
            OperationType::FileEdit(fe) => file_digest(&self.resolve(&fe.file)),
            OperationType::FileWrite(fw) => file_digest(&self.resolve(&fw.path)),
            OperationType::Symlink(sl) => Some(format!(
                "link={} target={}",
                self.resolve(&sl.link_path).exists(),
                sl.target
            )),
            OperationType::FileDelete(fd) => {
                Some(format!("exists={}", self.resolve(&fd.path).exists()))
            }
            OperationType::FileCopy(fc) => file_digest(&self.resolve(&fc.destination)),
            OperationType::FilePermissions(fp) => {
                let path = self.resolve(&fp.path);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::metadata(&path)
                        .ok()
                        .map(|m| format!("mode {:o}", m.permissions().mode() & 0o7777))
                }
                #[cfg(not(unix))]
                {
                    fs::metadata(&path)
                        .ok()
                        .map(|m| format!("readonly={}", m.permissions().readonly()))
                }
            }
            OperationType::DirectoryCreate(dc) => {
                Some(format!("exists={}", self.resolve(&dc.path).exists()))
            }
            OperationType::SelinuxMode(sm) => fs::read_to_string(self.resolve(&sm.file))
                .ok()
                .and_then(|c| {
                    c.lines()
                        .find(|l| l.trim_start().starts_with("SELINUX="))
                        .map(str::to_string)
                }),
            OperationType::ServiceOperation(so) => Command::new("systemctl")
                .args(["is-active", &so.service])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()),
            OperationType::RegistryEdit(re) => {
                if cfg!(windows) {
                    Command::new("reg.exe")
                        .args(["query", &re.key, "/v", &re.value])
                        .output()
                        .ok()
                        .filter(|o| o.status.success())
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            }
            OperationType::DriverInject(di) => Some(format!(
                "driver {} not yet injected (source {})",
                di.driver_name, di.source
            )),
            OperationType::PackageInstall(_) | OperationType::CommandExec(_) => None,
        }
    }

    fn run_op_validation(&self, op: &Operation) -> Option<ValidationOutcome> {
        let check = op.validation.as_ref()?;
        let output = if cfg!(windows) {
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &check.command])
                .output()
        } else {
            Command::new("sh").arg("-c").arg(&check.command).output()
        };
        Some(match output {
            Ok(out) => {
                let exit = out.status.code();
                let mut passed = exit == Some(check.expected_exit);
                let stdout = String::from_utf8_lossy(&out.stdout);
                if passed {
                    if let Some(expected) = &check.expected_output {
                        passed = stdout.contains(expected.as_str());
                    }
                }
                ValidationOutcome {
                    command: check.command.clone(),
                    exit_code: exit,
                    passed,
                    detail: (!passed).then(|| stdout.chars().take(500).collect()),
                }
            }
            Err(e) => ValidationOutcome {
                command: check.command.clone(),
                exit_code: None,
                passed: false,
                detail: Some(e.to_string()),
            },
        })
    }

    fn apply_operation(&self, op: &Operation, rollback_dir: &Path) -> Result<bool> {
        match &op.op_type {
            OperationType::FileEdit(fe) => {
                let path = self.resolve(&fe.file);
                self.snapshot_file(&path, rollback_dir)?;
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                for change in &fe.changes {
                    let mut found = false;
                    for line in lines.iter_mut() {
                        if line.trim() == change.before.trim() {
                            *line = change.after.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found && change.line > 0 && change.line <= lines.len() {
                        lines[change.line - 1] = change.after.clone();
                    }
                }
                fs::write(&path, lines.join("\n") + "\n")?;
                Ok(true)
            }
            OperationType::FileWrite(fw) => {
                let path = self.resolve(&fw.path);
                if path.exists() {
                    self.snapshot_file(&path, rollback_dir)?;
                }
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let content = if fw.content.ends_with('\n') {
                    fw.content.clone()
                } else {
                    format!("{}\n", fw.content)
                };
                fs::write(&path, content)?;
                if let Some(ref mode_str) = fw.mode {
                    let mode = u32::from_str_radix(mode_str, 8).unwrap_or(0o644);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = fs::metadata(&path)?.permissions();
                        perms.set_mode(mode);
                        fs::set_permissions(&path, perms)?;
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = mode;
                    }
                }
                Ok(true)
            }
            OperationType::Symlink(sl) => {
                let link = self.resolve(&sl.link_path);
                if let Some(parent) = link.parent() {
                    fs::create_dir_all(parent)?;
                }
                if link.exists() || link.is_symlink() {
                    let _ = fs::remove_file(&link);
                }
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(&sl.target, &link)?;
                }
                #[cfg(not(unix))]
                {
                    anyhow::bail!("Symlink operations require Unix");
                }
                Ok(true)
            }
            OperationType::FileDelete(fd) => {
                let path = self.resolve(&fd.path);
                if !path.exists() && !path.is_symlink() {
                    if fd.missing_ok {
                        return Ok(true);
                    }
                    anyhow::bail!("File to delete not found: {}", fd.path);
                }
                if path.exists() {
                    self.snapshot_file(&path, rollback_dir)?;
                }
                fs::remove_file(&path)?;
                Ok(true)
            }
            OperationType::CommandExec(ce) => {
                let status = if let Some(interp) = &ce.interpreter {
                    let mut parts = interp.split_whitespace();
                    let program = parts.next().unwrap_or("sh");
                    Command::new(program).args(parts).arg(&ce.command).status()
                } else if cfg!(windows) {
                    Command::new("powershell")
                        .args(["-NoProfile", "-NonInteractive", "-Command", &ce.command])
                        .status()
                } else {
                    Command::new("sh").arg("-c").arg(&ce.command).status()
                }
                .with_context(|| format!("exec {}", ce.command))?;
                if status.code().unwrap_or(-1) != ce.expected_exit {
                    anyhow::bail!(
                        "command '{}' exited with {:?}, expected {}",
                        ce.command,
                        status.code(),
                        ce.expected_exit
                    );
                }
                Ok(true)
            }
            OperationType::FilePermissions(fp) => {
                let path = self.resolve(&fp.path);
                self.snapshot_file(&path, rollback_dir)?;
                let mode = u32::from_str_radix(if fp.mode.is_empty() { "0" } else { &fp.mode }, 8)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&path)?.permissions();
                    perms.set_mode(mode);
                    fs::set_permissions(&path, perms)?;
                }
                #[cfg(not(unix))]
                {
                    let _ = mode;
                    anyhow::bail!("chmod not supported on this platform");
                }
                Ok(true)
            }
            OperationType::DirectoryCreate(dc) => {
                fs::create_dir_all(self.resolve(&dc.path))?;
                Ok(true)
            }
            OperationType::FileCopy(fc) => {
                let source = self.resolve(&fc.source);
                let destination = self.resolve(&fc.destination);
                if fc.backup && destination.exists() {
                    self.snapshot_file(&destination, rollback_dir)?;
                }
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&source, &destination)?;
                Ok(true)
            }
            OperationType::SelinuxMode(sm) => {
                let path = self.resolve(&sm.file);
                self.snapshot_file(&path, rollback_dir)?;
                if let Ok(content) = fs::read_to_string(&path) {
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
                    fs::write(&path, new_content)?;
                }
                let _ = Command::new("setenforce")
                    .arg(if sm.target == "enforcing" { "1" } else { "0" })
                    .status();
                Ok(true)
            }
            OperationType::PackageInstall(pi) => {
                self.install_packages(&pi.packages)?;
                Ok(true)
            }
            OperationType::ServiceOperation(so) => {
                if cfg!(windows) {
                    if so.restart || so.start {
                        let verb = if so.restart { "stop" } else { "start" };
                        if so.restart {
                            let _ = Command::new("sc.exe").args([verb, &so.service]).status();
                        }
                        let _ = Command::new("sc.exe").args(["start", &so.service]).status();
                    }
                    if let Some(state) = &so.state {
                        let start_mode = if state == "enabled" {
                            "auto"
                        } else {
                            "disabled"
                        };
                        let _ = Command::new("sc.exe")
                            .args(["config", &so.service, "start=", start_mode])
                            .status();
                    }
                } else {
                    if so.restart {
                        let _ = Command::new("systemctl")
                            .args(["restart", &so.service])
                            .status();
                    } else if so.start {
                        let _ = Command::new("systemctl")
                            .args(["start", &so.service])
                            .status();
                    }
                    if let Some(state) = &so.state {
                        let action = if state == "enabled" {
                            "enable"
                        } else {
                            "disable"
                        };
                        let _ = Command::new("systemctl")
                            .args([action, &so.service])
                            .status();
                    }
                }
                Ok(true)
            }
            OperationType::RegistryEdit(re) => {
                if !cfg!(windows) {
                    log::warn!("Registry edit ({}) skipped on Linux", re.key);
                    return Ok(false);
                }
                let data = registry_data_string(&re.new_data);
                let status = Command::new("reg.exe")
                    .args([
                        "add",
                        &re.key,
                        "/v",
                        &re.value,
                        "/t",
                        &registry_type(&re.data_type),
                        "/d",
                        &data,
                        "/f",
                    ])
                    .status()
                    .with_context(|| format!("reg add {}", re.key))?;
                if !status.success() {
                    anyhow::bail!("reg add {} failed: {status}", re.key);
                }
                Ok(true)
            }
            OperationType::DriverInject(di) => {
                if !cfg!(windows) {
                    anyhow::bail!("driver injection is a Windows operation");
                }
                let mut args = vec!["/add-driver", di.inf_path.as_str(), "/install"];
                if di.boot_critical {
                    // pnputil has no boot-critical flag; boot-critical
                    // registration (Start=0, Group) is a separate
                    // RegistryEdit op the planner chains after this one.
                    args = vec!["/add-driver", di.inf_path.as_str(), "/install"];
                }
                let status = Command::new("pnputil.exe")
                    .args(&args)
                    .status()
                    .with_context(|| format!("pnputil add-driver {}", di.inf_path))?;
                if !status.success() {
                    anyhow::bail!("pnputil failed for {}: {status}", di.inf_path);
                }
                Ok(true)
            }
        }
    }

    fn install_packages(&self, packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        if cfg!(windows) {
            anyhow::bail!("package installation is not applicable on Windows guests");
        }
        let status = if Path::new("/usr/bin/apt-get").exists() {
            Command::new("apt-get")
                .args(["install", "-y"])
                .args(packages)
                .status()
        } else if Path::new("/usr/bin/dnf").exists() {
            Command::new("dnf")
                .args(["install", "-y"])
                .args(packages)
                .status()
        } else if Path::new("/usr/bin/zypper").exists() {
            Command::new("zypper")
                .args(["install", "-y"])
                .args(packages)
                .status()
        } else {
            anyhow::bail!("no supported package manager found");
        }?;
        if !status.success() {
            anyhow::bail!("package install failed");
        }
        Ok(())
    }

    fn snapshot_file(&self, path: &Path, rollback_dir: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let backup_name = path
            .display()
            .to_string()
            .trim_start_matches('/')
            .replace(['/', '\\', ':'], "_");
        let backup_path = rollback_dir.join(&backup_name);
        fs::copy(path, &backup_path)?;

        let manifest_path = rollback_dir.join("manifest.json");
        let mut manifest: RollbackManifest = if manifest_path.exists() {
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?
        } else {
            RollbackManifest { files: Vec::new() }
        };
        let original = path.display().to_string();
        if !manifest.files.iter().any(|f| f.original_path == original) {
            manifest.files.push(RollbackFileEntry {
                original_path: original,
                backup_path,
                removed: false,
            });
            fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        }
        Ok(())
    }

    fn run_post_apply(&self, actions: &[PostApplyAction]) -> Result<()> {
        for action in actions {
            match action {
                PostApplyAction::ServiceRestart { services } => {
                    for svc in services {
                        let _ = Command::new("systemctl").args(["restart", svc]).status();
                    }
                }
                PostApplyAction::Validation {
                    command,
                    expected_output,
                } => {
                    let out = Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .output()
                        .with_context(|| format!("validation: {command}"))?;
                    if !out.status.success() {
                        log::warn!("validation command failed: {command}");
                    }
                    if let Some(expected) = expected_output {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        if !stdout.contains(expected) {
                            log::warn!(
                                "validation output mismatch for {command}: expected {expected}"
                            );
                        }
                    }
                }
                PostApplyAction::Message { message } => {
                    log::info!("post-apply: {message}");
                }
                PostApplyAction::RebootRequired { reason } => {
                    log::warn!("reboot required: {reason}");
                }
            }
        }
        Ok(())
    }
}

fn default_root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("C:\\")
    } else {
        PathBuf::from("/")
    }
}

fn file_digest(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!(
        "{} bytes, sha256:{}",
        bytes.len(),
        hex::encode(&hasher.finalize()[..8])
    ))
}

fn describe_change(op: &Operation) -> String {
    match &op.op_type {
        OperationType::FileEdit(fe) => {
            format!("edit {} ({} change(s))", fe.file, fe.changes.len())
        }
        OperationType::FileWrite(fw) => format!("write {}", fw.path),
        OperationType::Symlink(sl) => format!("symlink {} -> {}", sl.link_path, sl.target),
        OperationType::FileDelete(fd) => format!("delete {}", fd.path),
        OperationType::CommandExec(ce) => format!("run: {}", ce.command),
        OperationType::FilePermissions(fp) => format!("chmod {} {}", fp.mode, fp.path),
        OperationType::DirectoryCreate(dc) => format!("mkdir -p {}", dc.path),
        OperationType::FileCopy(fc) => format!("copy {} -> {}", fc.source, fc.destination),
        OperationType::SelinuxMode(sm) => {
            format!("SELinux {} -> {} in {}", sm.current, sm.target, sm.file)
        }
        OperationType::PackageInstall(pi) => {
            format!("install packages: {}", pi.packages.join(", "))
        }
        OperationType::ServiceOperation(so) => format!("service op on {}", so.service),
        OperationType::RegistryEdit(re) => {
            format!("registry set {}\\{} = {}", re.key, re.value, re.new_data)
        }
        OperationType::DriverInject(di) => format!(
            "inject driver {} from {} (boot_critical={})",
            di.driver_name, di.inf_path, di.boot_critical
        ),
    }
}

fn describe_rollback(op: &Operation) -> String {
    match &op.undo {
        Some(UndoInfo::Command { command }) => format!("run: {command}"),
        Some(UndoInfo::FileChanges(changes)) => {
            format!("revert {} file change(s)", changes.len())
        }
        Some(UndoInfo::Data(_)) => "apply recorded undo data".to_string(),
        None => match &op.op_type {
            OperationType::FileEdit(_)
            | OperationType::FileWrite(_)
            | OperationType::FileCopy(_)
            | OperationType::FilePermissions(_)
            | OperationType::FileDelete(_)
            | OperationType::SelinuxMode(_) => {
                "restore file snapshot from rollback directory".to_string()
            }
            _ if !op.reversible => "NOT REVERSIBLE".to_string(),
            _ => "no automatic rollback recorded".to_string(),
        },
    }
}

fn backup_path_for(op: &Operation, rollback_dir: &Path) -> Option<String> {
    let file = match &op.op_type {
        OperationType::FileEdit(fe) => Some(&fe.file),
        OperationType::FileWrite(fw) => Some(&fw.path),
        OperationType::FileDelete(fd) => Some(&fd.path),
        OperationType::FileCopy(fc) => Some(&fc.destination),
        OperationType::FilePermissions(fp) => Some(&fp.path),
        OperationType::SelinuxMode(sm) => Some(&sm.file),
        OperationType::Symlink(sl) => Some(&sl.link_path),
        _ => None,
    }?;
    let backup_name = file.trim_start_matches('/').replace(['/', '\\', ':'], "_");
    Some(rollback_dir.join(backup_name).display().to_string())
}

fn registry_type(data_type: &str) -> String {
    match data_type.to_ascii_uppercase().as_str() {
        "DWORD" => "REG_DWORD".to_string(),
        "QWORD" => "REG_QWORD".to_string(),
        "STRING" | "SZ" => "REG_SZ".to_string(),
        "EXPAND_SZ" => "REG_EXPAND_SZ".to_string(),
        "MULTI_SZ" => "REG_MULTI_SZ".to_string(),
        "BINARY" => "REG_BINARY".to_string(),
        other if other.starts_with("REG_") => other.to_string(),
        _ => "REG_SZ".to_string(),
    }
}

fn registry_data_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn plan_id_for(plan: &FixPlan) -> String {
    format!(
        "{}-{}",
        plan.vm.replace('/', "_"),
        plan.generated.timestamp()
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct RollbackManifest {
    files: Vec<RollbackFileEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RollbackFileEntry {
    original_path: String,
    backup_path: PathBuf,
    removed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn plan_with_ops(ops: Vec<Operation>) -> FixPlan {
        let mut plan = FixPlan::new("test-vm".to_string(), "test".to_string());
        plan.generated = Utc::now();
        plan.operations = ops;
        plan
    }

    fn file_edit_op(id: &str, file: &str, before: &str, after: &str) -> Operation {
        Operation {
            id: id.to_string(),
            op_type: OperationType::FileEdit(FileEdit {
                file: file.to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: before.to_string(),
                    after: after.to_string(),
                    context: None,
                }],
            }),
            priority: Priority::Medium,
            description: format!("edit {file}"),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        }
    }

    fn failing_command_op(id: &str) -> Operation {
        Operation {
            id: id.to_string(),
            op_type: OperationType::CommandExec(CommandExec {
                command: "exit 7".to_string(),
                expected_exit: 0,
                timeout: None,
                interpreter: None,
            }),
            priority: Priority::Medium,
            description: "always fails".to_string(),
            risk: Priority::Low,
            reversible: false,
            depends_on: vec![],
            validation: None,
            undo: None,
        }
    }

    #[test]
    fn dry_run_previews_with_before_state() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("fstab"), "old-line\n").unwrap();
        let plan = plan_with_ops(vec![file_edit_op("op-1", "/fstab", "old-line", "new-line")]);
        let exec = LivePlanExecutor::new(true).with_root(tmp.path());
        let result = exec.apply(&plan).unwrap();
        assert!(result.success);
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.outcomes[0].status, OpStatus::DryRun);
        assert!(result.outcomes[0].before_state.is_some());
        assert!(result.outcomes[0].planned_change.contains("/fstab"));
        // Nothing was modified.
        assert_eq!(
            fs::read_to_string(tmp.path().join("fstab")).unwrap(),
            "old-line\n"
        );
    }

    #[test]
    fn apply_records_outcome_snapshot_and_rollback_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("fstab"), "old-line\n").unwrap();
        let plan = plan_with_ops(vec![file_edit_op("op-1", "/fstab", "old-line", "new-line")]);
        let exec = LivePlanExecutor::new(false).with_root(tmp.path());
        let result = exec.apply(&plan).unwrap();
        assert!(result.success, "{}", result.message);
        assert_eq!(result.outcomes[0].status, OpStatus::Applied);
        assert!(result.outcomes[0].backup_path.is_some());
        assert_eq!(
            fs::read_to_string(tmp.path().join("fstab")).unwrap(),
            "new-line\n"
        );

        // Rollback restores the original content from the snapshot.
        let plan_id = plan_id_for(&plan);
        exec.rollback(&plan_id).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("fstab")).unwrap(),
            "old-line\n"
        );
    }

    #[test]
    fn validation_failure_marks_op_failed() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f"), "x\n").unwrap();
        let mut op = file_edit_op("op-1", "/f", "x", "y");
        op.validation = Some(ValidationCheck {
            command: "exit 3".to_string(),
            expected_exit: 0,
            expected_output: None,
        });
        let plan = plan_with_ops(vec![op]);
        let exec = LivePlanExecutor::new(false).with_root(tmp.path());
        let result = exec.apply(&plan).unwrap();
        assert!(!result.success);
        assert_eq!(result.outcomes[0].status, OpStatus::Failed);
        let v = result.outcomes[0].validation.as_ref().unwrap();
        assert!(!v.passed);
        assert_eq!(v.exit_code, Some(3));
    }

    #[test]
    fn fail_fast_stops_and_auto_rollback_restores() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a"), "one\n").unwrap();
        fs::write(tmp.path().join("b"), "two\n").unwrap();
        let mut edit_a = file_edit_op("op-1", "/a", "one", "ONE");
        let fail = failing_command_op("op-2");
        let mut edit_b = file_edit_op("op-3", "/b", "two", "TWO");
        // Chain dependencies so topological order is deterministic.
        let fail = {
            let mut f = fail;
            f.depends_on = vec!["op-1".to_string()];
            f
        };
        edit_b.depends_on = vec!["op-2".to_string()];
        edit_a.depends_on = vec![];
        let plan = plan_with_ops(vec![edit_a, fail, edit_b]);

        let exec = LivePlanExecutor::for_migration(false).with_root(tmp.path());
        let result = exec.apply(&plan).unwrap();
        assert!(!result.success);
        // op-1 applied then rolled back; op-3 never reached.
        let by_id = |id: &str| result.outcomes.iter().find(|o| o.op_id == id).unwrap();
        assert_eq!(by_id("op-1").status, OpStatus::RolledBack);
        assert_eq!(by_id("op-2").status, OpStatus::Failed);
        assert_eq!(by_id("op-3").status, OpStatus::NotReached);
        // Auto-rollback restored the first file.
        assert_eq!(fs::read_to_string(tmp.path().join("a")).unwrap(), "one\n");
    }

    #[test]
    fn legacy_executor_continues_past_failures() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a"), "one\n").unwrap();
        let mut fail = failing_command_op("op-1");
        fail.depends_on = vec![];
        let mut edit = file_edit_op("op-2", "/a", "one", "ONE");
        edit.depends_on = vec!["op-1".to_string()];
        let plan = plan_with_ops(vec![fail, edit]);
        let exec = LivePlanExecutor::new(false).with_root(tmp.path());
        let result = exec.apply(&plan).unwrap();
        assert!(!result.success);
        assert_eq!(result.operations_applied, 1);
        assert_eq!(result.operations_failed, 1);
        // No auto-rollback in legacy mode: the edit stays applied.
        assert_eq!(fs::read_to_string(tmp.path().join("a")).unwrap(), "ONE\n");
    }
}
