// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Policy-scoped file operations (`guestkit.file*`).
//!
//! Everything here is confined by the `capabilities.file_ops` policy:
//! disabled by default, path-prefix allowlist, size cap, and symlink
//! canonicalization so a link inside an allowed path cannot reach outside.

use crate::agent::policy::FileOpsPolicy;
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Canonicalize and verify a path sits under an allowed prefix.
/// For writes to not-yet-existing files, the parent is canonicalized.
fn authorize_path(policy: &FileOpsPolicy, raw: &str, for_write: bool) -> Result<PathBuf> {
    if policy.allowed_paths.is_empty() {
        bail!("file_ops.allowed_paths is empty — no paths are permitted");
    }
    let requested = Path::new(raw);
    let canonical = match requested.canonicalize() {
        Ok(p) => p,
        Err(_) if for_write => {
            let parent = requested
                .parent()
                .context("path has no parent")?
                .canonicalize()
                .context("parent directory does not exist")?;
            parent.join(requested.file_name().context("path has no file name")?)
        }
        Err(e) => bail!("cannot resolve {raw}: {e}"),
    };
    let allowed = policy.allowed_paths.iter().any(|prefix| {
        prefix
            .canonicalize()
            .map(|p| canonical.starts_with(&p))
            .unwrap_or(false)
    });
    if !allowed {
        bail!(
            "path {} resolves outside the allowed prefixes",
            canonical.display()
        );
    }
    Ok(canonical)
}

pub fn read(policy: &FileOpsPolicy, params: &Value) -> Result<Value> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .context("missing required param: path")?;
    let path = authorize_path(policy, raw, false)?;
    let meta = std::fs::metadata(&path)?;
    if meta.len() > policy.max_bytes {
        bail!(
            "file is {} bytes, over the {}-byte policy cap",
            meta.len(),
            policy.max_bytes
        );
    }
    let bytes = std::fs::read(&path)?;
    Ok(json!({
        "path": path.display().to_string(),
        "size": bytes.len(),
        "encoding": "base64",
        "data": B64.encode(&bytes),
    }))
}

pub fn write(policy: &FileOpsPolicy, params: &Value) -> Result<Value> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .context("missing required param: path")?;
    let data = params
        .get("data")
        .and_then(Value::as_str)
        .context("missing required param: data (base64)")?;
    let bytes = B64.decode(data).context("data is not valid base64")?;
    if bytes.len() as u64 > policy.max_bytes {
        bail!(
            "payload is {} bytes, over the {}-byte policy cap",
            bytes.len(),
            policy.max_bytes
        );
    }
    let path = authorize_path(policy, raw, true)?;

    // Atomic replace with backup of any existing content.
    let backup = if path.exists() {
        let backup_path = path.with_extension("guestkit-bak");
        std::fs::copy(&path, &backup_path)?;
        Some(backup_path.display().to_string())
    } else {
        None
    };
    let tmp = path.with_extension("guestkit-tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(json!({
        "path": path.display().to_string(),
        "written": bytes.len(),
        "backup": backup,
    }))
}

pub fn stat(policy: &FileOpsPolicy, params: &Value) -> Result<Value> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .context("missing required param: path")?;
    let path = authorize_path(policy, raw, false)?;
    let meta = std::fs::metadata(&path)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        Some(format!("{:o}", meta.permissions().mode() & 0o7777))
    };
    #[cfg(not(unix))]
    let mode: Option<String> = None;
    Ok(json!({
        "path": path.display().to_string(),
        "size": meta.len(),
        "is_dir": meta.is_dir(),
        "modified_unix": modified,
        "mode": mode,
        "readonly": meta.permissions().readonly(),
    }))
}

pub fn list(policy: &FileOpsPolicy, params: &Value) -> Result<Value> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .context("missing required param: path")?;
    let path = authorize_path(policy, raw, false)?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path)?.take(1000) {
        let entry = entry?;
        let meta = entry.metadata()?;
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "is_dir": meta.is_dir(),
            "size": meta.len(),
        }));
    }
    Ok(json!({ "path": path.display().to_string(), "entries": entries }))
}

pub fn checksum(policy: &FileOpsPolicy, params: &Value) -> Result<Value> {
    use sha2::{Digest, Sha256};
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .context("missing required param: path")?;
    let path = authorize_path(policy, raw, false)?;
    let bytes = std::fs::read(&path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(json!({
        "path": path.display().to_string(),
        "algorithm": "sha256",
        "checksum": hex::encode(hasher.finalize()),
        "size": bytes.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_for(dir: &Path) -> FileOpsPolicy {
        FileOpsPolicy {
            enabled: true,
            allowed_paths: vec![dir.to_path_buf()],
            max_bytes: 1024,
        }
    }

    #[test]
    fn read_write_round_trip_with_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let policy = policy_for(tmp.path());
        let file = tmp.path().join("config.txt");
        std::fs::write(&file, "original").unwrap();

        let read1 = read(&policy, &json!({"path": file.display().to_string()})).unwrap();
        assert_eq!(
            B64.decode(read1["data"].as_str().unwrap()).unwrap(),
            b"original"
        );

        let result = write(
            &policy,
            &json!({"path": file.display().to_string(), "data": B64.encode("updated")}),
        )
        .unwrap();
        assert!(result["backup"].as_str().unwrap().ends_with("guestkit-bak"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "updated");
    }

    #[test]
    fn path_outside_allowlist_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("hostname");
        std::fs::write(&target, "x").unwrap();
        let policy = policy_for(tmp.path());
        let err = read(&policy, &json!({"path": target.display().to_string()})).unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
    }

    #[test]
    fn symlink_escape_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret");
        std::fs::write(&secret, "s3cret").unwrap();
        let link = tmp.path().join("innocent");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        #[cfg(unix)]
        {
            let policy = policy_for(tmp.path());
            let err = read(&policy, &json!({"path": link.display().to_string()})).unwrap_err();
            assert!(err.to_string().contains("outside"), "{err}");
        }
    }

    #[test]
    fn size_cap_enforced() {
        let tmp = tempfile::tempdir().unwrap();
        let policy = policy_for(tmp.path());
        let file = tmp.path().join("big");
        std::fs::write(&file, vec![0u8; 4096]).unwrap();
        let err = read(&policy, &json!({"path": file.display().to_string()})).unwrap_err();
        assert!(err.to_string().contains("cap"), "{err}");
    }

    #[test]
    fn empty_allowlist_denies_everything() {
        let policy = FileOpsPolicy {
            enabled: true,
            allowed_paths: vec![],
            max_bytes: 1024,
        };
        let err = read(&policy, &json!({"path": "/tmp/x"})).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}
