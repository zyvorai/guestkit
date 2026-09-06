// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Application-aware snapshot plugins.
//!
//! Each plugin knows how to flush/quiesce one application class before a
//! filesystem freeze so the snapshot is application-consistent, and how to
//! resume it afterwards. Plugins are best-effort with hard timeouts: a
//! failing plugin downgrades the snapshot report to crash-consistent for
//! that application instead of blocking the snapshot.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

pub const PLUGIN_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReport {
    pub plugin: String,
    pub discovered: bool,
    pub quiesced: bool,
    pub resumed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub trait SnapshotPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    /// Is the application present and running on this guest?
    fn discover(&self) -> bool;
    /// Flush/quiesce; returns a human detail string.
    fn quiesce(&self) -> anyhow::Result<String>;
    /// Undo quiesce effects after the snapshot completes (or on watchdog).
    fn resume(&self) -> anyhow::Result<String>;
}

fn run_with_timeout(mut cmd: Command) -> anyhow::Result<std::process::Output> {
    // std::process has no built-in timeout; spawn + poll.
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let deadline = std::time::Instant::now() + PLUGIN_TIMEOUT;
    loop {
        if let Some(_status) = child.try_wait()? {
            return Ok(child.wait_with_output()?);
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            anyhow::bail!("plugin command timed out after {PLUGIN_TIMEOUT:?}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn service_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn command_exists(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {cmd}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- PostgreSQL ---

pub struct PostgresPlugin;

impl SnapshotPlugin for PostgresPlugin {
    fn name(&self) -> &'static str {
        "postgresql"
    }

    fn discover(&self) -> bool {
        (service_active("postgresql.service")
            || service_active("postgresql@.service")
            || service_active("postgresql-16.service")
            || service_active("postgresql-15.service"))
            && command_exists("psql")
    }

    fn quiesce(&self) -> anyhow::Result<String> {
        // CHECKPOINT forces all dirty pages to disk so the frozen image
        // replays minimal WAL. Runs as the postgres OS user.
        let mut cmd = Command::new("su");
        cmd.args(["-s", "/bin/sh", "postgres", "-c", "psql -c CHECKPOINT"]);
        let out = run_with_timeout(cmd)?;
        if out.status.success() {
            Ok("CHECKPOINT flushed".to_string())
        } else {
            anyhow::bail!("psql CHECKPOINT: {}", String::from_utf8_lossy(&out.stderr))
        }
    }

    fn resume(&self) -> anyhow::Result<String> {
        // CHECKPOINT needs no resume.
        Ok("no resume needed".to_string())
    }
}

// --- MySQL / MariaDB ---

pub struct MysqlPlugin;

impl SnapshotPlugin for MysqlPlugin {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn discover(&self) -> bool {
        (service_active("mysqld.service")
            || service_active("mysql.service")
            || service_active("mariadb.service"))
            && command_exists("mysql")
    }

    fn quiesce(&self) -> anyhow::Result<String> {
        // FLUSH TABLES + FLUSH BINARY LOGS pushes data and log state to
        // disk. A held FLUSH TABLES WITH READ LOCK would need a persistent
        // session; the freeze that follows immediately provides the
        // stable point instead.
        let mut cmd = Command::new("mysql");
        cmd.args(["-e", "FLUSH TABLES; FLUSH BINARY LOGS;"]);
        let out = run_with_timeout(cmd)?;
        if out.status.success() {
            Ok("tables and binary logs flushed".to_string())
        } else {
            anyhow::bail!("mysql flush: {}", String::from_utf8_lossy(&out.stderr))
        }
    }

    fn resume(&self) -> anyhow::Result<String> {
        Ok("no resume needed".to_string())
    }
}

// --- Redis ---

pub struct RedisPlugin;

impl SnapshotPlugin for RedisPlugin {
    fn name(&self) -> &'static str {
        "redis"
    }

    fn discover(&self) -> bool {
        service_active("redis.service") || service_active("redis-server.service")
    }

    fn quiesce(&self) -> anyhow::Result<String> {
        if !command_exists("redis-cli") {
            anyhow::bail!("redis-cli not found");
        }
        let mut cmd = Command::new("redis-cli");
        cmd.arg("BGSAVE");
        let out = run_with_timeout(cmd)?;
        if !out.status.success() {
            anyhow::bail!("BGSAVE: {}", String::from_utf8_lossy(&out.stderr));
        }
        // Wait for the background save to finish (rdb_bgsave_in_progress:0).
        let deadline = std::time::Instant::now() + PLUGIN_TIMEOUT;
        loop {
            let info = Command::new("redis-cli")
                .args(["INFO", "persistence"])
                .output()?;
            let text = String::from_utf8_lossy(&info.stdout).to_string();
            if text.contains("rdb_bgsave_in_progress:0") {
                return Ok("BGSAVE complete".to_string());
            }
            if std::time::Instant::now() > deadline {
                anyhow::bail!("BGSAVE did not finish before timeout");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn resume(&self) -> anyhow::Result<String> {
        Ok("no resume needed".to_string())
    }
}

// --- Custom hook directories (customer scripts) ---

pub struct HookDirPlugin;

impl HookDirPlugin {
    fn run_dir(dir: &str) -> anyhow::Result<String> {
        let mut ran = 0usize;
        let mut failed = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for path in paths {
                if path.extension().and_then(|e| e.to_str()) != Some("sh") {
                    continue;
                }
                let mut cmd = Command::new("sh");
                cmd.arg(&path);
                match run_with_timeout(cmd) {
                    Ok(out) if out.status.success() => ran += 1,
                    Ok(_) | Err(_) => {
                        failed.push(path.display().to_string());
                    }
                }
            }
        }
        if failed.is_empty() {
            Ok(format!("{ran} hook(s) ran"))
        } else {
            anyhow::bail!("hooks failed: {}", failed.join(", "))
        }
    }

    fn hook_dirs(kind: &str) -> [String; 2] {
        [
            format!("/etc/guestkit/hooks/{kind}"),
            format!("/etc/zyvor/hooks/{kind}"),
        ]
    }
}

impl SnapshotPlugin for HookDirPlugin {
    fn name(&self) -> &'static str {
        "custom-hooks"
    }

    fn discover(&self) -> bool {
        Self::hook_dirs("pre-snapshot")
            .iter()
            .chain(Self::hook_dirs("post-snapshot").iter())
            .any(|d| std::path::Path::new(d).is_dir())
    }

    fn quiesce(&self) -> anyhow::Result<String> {
        let mut messages = Vec::new();
        for dir in Self::hook_dirs("pre-snapshot") {
            if std::path::Path::new(&dir).is_dir() {
                messages.push(format!("{dir}: {}", Self::run_dir(&dir)?));
            }
        }
        Ok(messages.join("; "))
    }

    fn resume(&self) -> anyhow::Result<String> {
        let mut messages = Vec::new();
        for dir in Self::hook_dirs("post-snapshot") {
            if std::path::Path::new(&dir).is_dir() {
                messages.push(format!("{dir}: {}", Self::run_dir(&dir)?));
            }
        }
        Ok(messages.join("; "))
    }
}

/// All built-in plugins, in quiesce order (applications before hooks).
pub fn builtin_plugins() -> Vec<Box<dyn SnapshotPlugin>> {
    vec![
        Box::new(PostgresPlugin),
        Box::new(MysqlPlugin),
        Box::new(RedisPlugin),
        Box::new(HookDirPlugin),
    ]
}
