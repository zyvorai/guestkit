// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! qemu-img front-end owned by GuestKit.
//!
//! Scripts should call `guestkit img …` instead of shelling out to qemu-img.
//! The binary is still qemu-img (same on-disk format semantics); the CLI,
//! JSON shape, and error strings are GuestKit's.

use crate::core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Wrapper around the host `qemu-img` binary.
#[derive(Debug, Clone)]
pub struct QemuImg {
    bin: PathBuf,
}

impl Default for QemuImg {
    fn default() -> Self {
        Self::new()
    }
}

impl QemuImg {
    pub fn new() -> Self {
        let bin = std::env::var("GUESTKIT_QEMU_IMG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("qemu-img"));
        Self { bin }
    }

    pub fn with_bin<P: AsRef<Path>>(bin: P) -> Self {
        Self {
            bin: bin.as_ref().to_path_buf(),
        }
    }

    pub fn info_json<P: AsRef<Path>>(&self, image: P) -> Result<Value> {
        let out = self.run(&["info", "--output=json", path_str(image.as_ref())])?;
        serde_json::from_slice(&out.stdout)
            .map_err(|e| Error::InvalidFormat(format!("qemu-img info json: {e}")))
    }

    pub fn check<P: AsRef<Path>>(&self, image: P, repair: bool) -> Result<ImgCheckReport> {
        let image = path_str(image.as_ref());
        let args: Vec<&str> = if repair {
            vec!["check", "-r", "leaks", "--output=json", image]
        } else {
            vec!["check", "--output=json", image]
        };
        let out = self.run_allow_nonzero(&args)?;
        let parsed = serde_json::from_slice::<Value>(&out.stdout).ok();
        Ok(ImgCheckReport {
            image: image.to_string(),
            ok: out.status.success(),
            exit_code: out.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            report: parsed,
        })
    }

    pub fn snapshot_list<P: AsRef<Path>>(&self, image: P) -> Result<Value> {
        let out = self.run(&["snapshot", "-l", "--output=json", path_str(image.as_ref())])?;
        if out.stdout.is_empty() {
            return Ok(Value::Array(vec![]));
        }
        serde_json::from_slice(&out.stdout).or_else(|_| {
            Ok(Value::String(
                String::from_utf8_lossy(&out.stdout).into_owned(),
            ))
        })
    }

    pub fn snapshot_create<P: AsRef<Path>>(&self, image: P, name: &str) -> Result<()> {
        self.run(&["snapshot", "-c", name, path_str(image.as_ref())])?;
        Ok(())
    }

    pub fn snapshot_delete<P: AsRef<Path>>(&self, image: P, name: &str) -> Result<()> {
        self.run(&["snapshot", "-d", name, path_str(image.as_ref())])?;
        Ok(())
    }

    pub fn snapshot_apply<P: AsRef<Path>>(&self, image: P, name: &str) -> Result<()> {
        self.run(&["snapshot", "-a", name, path_str(image.as_ref())])?;
        Ok(())
    }

    pub fn resize<P: AsRef<Path>>(&self, image: P, size: &str) -> Result<()> {
        self.run(&["resize", path_str(image.as_ref()), size])?;
        Ok(())
    }

    pub fn rebase<P: AsRef<Path>>(&self, image: P, backing: P, unsafe_mode: bool) -> Result<()> {
        let image_s = image.as_ref().to_string_lossy().into_owned();
        let backing_s = backing.as_ref().to_string_lossy().into_owned();
        let mut owned = vec!["rebase".to_string()];
        if unsafe_mode {
            owned.push("-u".into());
        }
        owned.push("-b".into());
        owned.push(backing_s);
        owned.push(image_s);
        let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        self.run(&refs)?;
        Ok(())
    }

    pub fn commit<P: AsRef<Path>>(&self, image: P) -> Result<()> {
        self.run(&["commit", path_str(image.as_ref())])?;
        Ok(())
    }

    fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        let out = self.run_allow_nonzero(args)?;
        if !out.status.success() {
            return Err(Error::CommandFailed(format!(
                "{} {} failed ({}): {}",
                self.bin.display(),
                args.join(" "),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(out)
    }

    fn run_allow_nonzero(&self, args: &[&str]) -> Result<std::process::Output> {
        Command::new(&self.bin).args(args).output().map_err(|e| {
            Error::CommandFailed(format!(
                "failed to execute {} (set GUESTKIT_QEMU_IMG): {e}",
                self.bin.display()
            ))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImgCheckReport {
    pub image: String,
    pub ok: bool,
    pub exit_code: i32,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Value>,
}

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn bin_defaults_and_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("GUESTKIT_QEMU_IMG");
        assert_eq!(QemuImg::new().bin, PathBuf::from("qemu-img"));

        std::env::set_var("GUESTKIT_QEMU_IMG", "/opt/qemu/qemu-img");
        let q = QemuImg::new();
        std::env::remove_var("GUESTKIT_QEMU_IMG");
        assert_eq!(q.bin, PathBuf::from("/opt/qemu/qemu-img"));
    }
}
