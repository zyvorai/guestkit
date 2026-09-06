// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Channel update check with SHA256-verified staging for privileged apply.

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(not(target_os = "windows"))]
const STAGED_AGENT: &str = "/var/lib/zyvor/staged/zyvor-guest-agent";
#[cfg(target_os = "windows")]
const STAGED_AGENT: &str = "C:\\ProgramData\\zyvor\\staged\\zyvor-guest-agent.exe";
#[cfg(not(target_os = "windows"))]
const STAGED_META: &str = "/var/lib/zyvor/staged/update.json";
#[cfg(target_os = "windows")]
const STAGED_META: &str = "C:\\ProgramData\\zyvor\\staged\\update.json";

#[derive(Debug, Clone, Deserialize)]
struct ApiEnvelope<T> {
    data: T,
}

#[derive(Debug, Clone, Deserialize)]
struct BundleInfo {
    version: String,
    channel: String,
    linux_tar_url: Option<String>,
    linux_tar_sha256: Option<String>,
    linux_tar_signature: Option<String>,
    windows_zip_url: Option<String>,
    windows_zip_sha256: Option<String>,
    windows_zip_signature: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StagedUpdateMeta {
    pub version: String,
    pub channel: String,
    pub artifact_url: String,
    pub sha256: String,
    pub staged_at: String,
    #[serde(default)]
    pub platform: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub remote_version: Option<String>,
    pub channel: String,
    pub update_available: bool,
    pub artifact_url: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_signature: Option<String>,
    pub platform: String,
}

pub async fn check_update() -> Result<UpdateCheckResult> {
    let current = crate::VERSION.to_string();
    let bundle = fetch_bundle_info().await?;
    let update_available = version_greater(&bundle.version, &current);
    let platform = current_platform();
    let (artifact_url, artifact_sha256, artifact_signature) = if platform == "windows" {
        (
            bundle.windows_zip_url.clone(),
            bundle.windows_zip_sha256.clone(),
            bundle.windows_zip_signature.clone(),
        )
    } else {
        (
            bundle.linux_tar_url.clone(),
            bundle.linux_tar_sha256.clone(),
            bundle.linux_tar_signature.clone(),
        )
    };
    Ok(UpdateCheckResult {
        current_version: current,
        remote_version: Some(bundle.version.clone()),
        channel: bundle.channel,
        update_available,
        artifact_url,
        artifact_sha256,
        artifact_signature,
        platform,
    })
}

pub async fn stage_update(apply: bool) -> Result<String> {
    let check = check_update().await?;
    if !check.update_available {
        return Ok(format!(
            "Zyvor GuestAgent {} is current (channel {})",
            check.current_version, check.channel
        ));
    }

    let url = check
        .artifact_url
        .as_ref()
        .filter(|u| !u.is_empty())
        .with_context(|| format!("bundle has no {} artifact URL", check.platform))?;
    let expected_sha = check
        .artifact_sha256
        .as_ref()
        .filter(|s| !s.is_empty())
        .with_context(|| {
            format!(
                "bundle has no {} sha256; refusing unsigned download",
                check.platform
            )
        })?;

    let manifest = build_update_manifest(&check, expected_sha);
    if let Some(sig) = check.artifact_signature.as_ref().filter(|s| !s.is_empty()) {
        crate::agent::update_sign::verify_manifest(&manifest, sig)
            .context("update manifest signature verification failed")?;
    } else {
        anyhow::bail!("bundle has no artifact signature; refusing unsigned download");
    }

    let bytes = download_bytes(url).await?;
    let actual_sha = hex_sha256(&bytes);
    if !actual_sha.eq_ignore_ascii_case(expected_sha) {
        anyhow::bail!("artifact sha256 mismatch (expected {expected_sha}, got {actual_sha})");
    }

    let staged_agent = Path::new(STAGED_AGENT);
    let staged_dir = staged_agent
        .parent()
        .unwrap_or(Path::new(if cfg!(target_os = "windows") {
            "C:\\ProgramData\\zyvor\\staged"
        } else {
            "/var/lib/zyvor/staged"
        }));
    fs::create_dir_all(staged_dir).context("create staged dir")?;

    if check.platform == "windows" {
        extract_agent_exe_from_zip(&bytes, staged_agent)?;
    } else {
        extract_agent_binary_from_tar_gz(&bytes, staged_agent)?;
    }

    let meta = StagedUpdateMeta {
        version: check.remote_version.clone().unwrap_or_default(),
        channel: check.channel.clone(),
        artifact_url: url.clone(),
        sha256: actual_sha,
        staged_at: chrono::Utc::now().to_rfc3339(),
        platform: check.platform.clone(),
    };
    fs::write(STAGED_META, serde_json::to_string_pretty(&meta)?)?;

    if apply {
        if crate::agent::executor_ipc::executor_available() {
            let result = crate::agent::executor_ipc::call_executor(
                "apply_staged_update",
                serde_json::json!({}),
            )?;
            return Ok(result
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| "staged update applied via executor".into()));
        }
        return Ok(format!(
            "staged {} to {}; executor unavailable — apply via privileged helper",
            meta.version, STAGED_AGENT
        ));
    }

    Ok(format!(
        "staged {} (sha256 verified) at {}",
        meta.version, STAGED_AGENT
    ))
}

fn build_update_manifest(
    check: &UpdateCheckResult,
    expected_sha: &str,
) -> crate::agent::update_sign::UpdateManifest {
    let version = check.remote_version.clone().unwrap_or_default();
    let channel = check.channel.clone();
    if check.platform == "windows" {
        crate::agent::update_sign::UpdateManifest {
            version,
            channel,
            linux_tar_sha256: String::new(),
            windows_zip_sha256: expected_sha.to_string(),
        }
    } else {
        crate::agent::update_sign::UpdateManifest {
            version,
            channel,
            linux_tar_sha256: expected_sha.to_string(),
            windows_zip_sha256: String::new(),
        }
    }
}

fn current_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".into()
    } else {
        "linux".into()
    }
}

async fn fetch_bundle_info() -> Result<BundleInfo> {
    let base = resolve_bundle_base()?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let url = format!("{base}/api/v1/vmtools/bundle");
    let resp = client.get(&url).send().await.context("fetch bundle")?;
    if !resp.status().is_success() {
        anyhow::bail!("bundle HTTP {}", resp.status());
    }
    let envelope = resp
        .json::<ApiEnvelope<BundleInfo>>()
        .await
        .context("parse bundle response")?;
    Ok(envelope.data)
}

fn resolve_bundle_base() -> Result<String> {
    if let Ok(url) = std::env::var("ZYVOR_UPDATE_URL") {
        if !url.trim().is_empty() {
            return Ok(url.trim_end_matches('/').to_string());
        }
    }
    let config = crate::agent::transport::zeus_push::load_config();
    if let Some(url) = config.zeus_url.filter(|u| !u.is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    if let Ok(url) = std::env::var("ZYVOR_ZEUS_URL") {
        if !url.trim().is_empty() {
            return Ok(url.trim_end_matches('/').to_string());
        }
    }
    anyhow::bail!("no ZYVOR_UPDATE_URL or zeus_url configured for update check")
}

async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let resp = client.get(url).send().await.context("download artifact")?;
    if !resp.status().is_success() {
        anyhow::bail!("artifact HTTP {}", resp.status());
    }
    resp.bytes()
        .await
        .context("read artifact bytes")
        .map(|b| b.to_vec())
}

fn extract_agent_binary_from_tar_gz(tar_gz: &[u8], dest: &Path) -> Result<()> {
    use std::io::Cursor;
    let gz = flate2::read::GzDecoder::new(Cursor::new(tar_gz));
    let mut archive = tar::Archive::new(gz);
    let staged_dir = dest.parent().unwrap_or(Path::new("/var/lib/zyvor/staged"));
    let temp_dir = staged_dir.join(format!(
        "extract-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("tmp")
    ));
    fs::create_dir_all(&temp_dir).context("create extract dir")?;
    archive.unpack(&temp_dir).context("unpack tar.gz")?;

    let candidate = find_file_recursive(&temp_dir, "guestkitd")
        .or_else(|_| find_file_recursive(&temp_dir, "zyvor-guest-agent"))
        .context("agent binary (guestkitd / zyvor-guest-agent) not found in artifact")?;
    fs::copy(&candidate, dest)
        .with_context(|| format!("copy staged binary to {}", dest.display()))?;
    fs::remove_dir_all(&temp_dir).ok();
    #[cfg(unix)]
    fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_agent_exe_from_zip(zip_bytes: &[u8], dest: &Path) -> Result<()> {
    use std::process::Command;
    let staged_dir = dest
        .parent()
        .unwrap_or(Path::new("C:\\ProgramData\\zyvor\\staged"));
    let zip_path = staged_dir.join("update.zip");
    let extract_dir = staged_dir.join(format!(
        "extract-{}",
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("tmp")
    ));
    fs::write(&zip_path, zip_bytes).context("write update zip")?;
    fs::create_dir_all(&extract_dir).context("create extract dir")?;
    let script = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        zip_path.display(),
        extract_dir.display()
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .context("powershell Expand-Archive")?;
    if !status.success() {
        anyhow::bail!("Expand-Archive failed");
    }
    let candidate = find_file_recursive(&extract_dir, "guestkitd.exe")
        .or_else(|_| find_file_recursive(&extract_dir, "zyvor-guest-agent.exe"))
        .or_else(|_| find_file_recursive(&extract_dir, "zyvor-guest-agent"))
        .context("agent exe (guestkitd.exe / zyvor-guest-agent.exe) not found in zip")?;
    fs::copy(&candidate, dest).with_context(|| format!("copy staged exe to {}", dest.display()))?;
    fs::remove_dir_all(&extract_dir).ok();
    fs::remove_file(&zip_path).ok();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn extract_agent_exe_from_zip(_zip_bytes: &[u8], _dest: &Path) -> Result<()> {
    anyhow::bail!("windows zip updates are only supported on Windows guests")
}

fn find_file_recursive(dir: &Path, name: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(found) = find_file_recursive(&path, name) {
                return Ok(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Ok(path);
        }
    }
    anyhow::bail!("file {name} not found under {}", dir.display())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn version_greater(remote: &str, current: &str) -> bool {
    parse_version(remote) > parse_version(current)
}

fn parse_version(raw: &str) -> Vec<u64> {
    raw.split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn apply_staged_update_privileged() -> Result<String> {
    let meta_raw = fs::read_to_string(STAGED_META).context("read staged update metadata")?;
    let meta: StagedUpdateMeta =
        serde_json::from_str(&meta_raw).context("parse staged update metadata")?;
    if !Path::new(STAGED_AGENT).exists() {
        anyhow::bail!("staged binary missing at {STAGED_AGENT}");
    }
    let bytes = fs::read(STAGED_AGENT).context("read staged binary")?;
    let actual_sha = hex_sha256(&bytes);
    if !actual_sha.eq_ignore_ascii_case(&meta.sha256) {
        anyhow::bail!("staged binary sha256 mismatch");
    }

    if meta.platform == "windows" || cfg!(target_os = "windows") {
        return apply_staged_update_windows(&meta);
    }

    // Canonical names first; the legacy paths are symlinks to guestkitd on
    // rebranded installs and real files on pre-rebrand ones.
    let targets = [
        "/usr/bin/guestkitd",
        "/usr/local/bin/guestkitd",
        "/usr/bin/zyvor-guest-agent",
        "/usr/local/bin/zyvor-guest-agent",
    ];
    let mut updated = Vec::new();
    for target in targets {
        // Only overwrite existing installs (or their symlink targets); a
        // missing canonical path on a legacy install is skipped.
        let path = Path::new(target);
        let exists_or_symlink = path.exists() || path.is_symlink();
        if !exists_or_symlink {
            continue;
        }
        // Writing through a symlink would double-update guestkitd; replace
        // only real files.
        if path.is_symlink() {
            continue;
        }
        if Path::new(target)
            .parent()
            .map(|p| p.exists())
            .unwrap_or(false)
        {
            fs::copy(STAGED_AGENT, target)
                .with_context(|| format!("install staged binary to {target}"))?;
            #[cfg(unix)]
            fs::set_permissions(target, fs::Permissions::from_mode(0o755))?;
            updated.push(target.to_string());
        }
    }
    if updated.is_empty() {
        anyhow::bail!("no install target found for staged agent binary");
    }

    restart_agent_services_linux();

    Ok(format!(
        "applied {} to {} (sha256 verified, services restarted)",
        meta.version,
        updated.join(", ")
    ))
}

#[cfg(target_os = "windows")]
fn apply_staged_update_windows(meta: &StagedUpdateMeta) -> Result<String> {
    use std::process::Command;
    let install_dir = Path::new(r"C:\Program Files\Zyvor\VM Tools");
    fs::create_dir_all(install_dir).context("create install dir")?;
    let dest = install_dir.join("zyvor-guest-agent.exe");
    let _ = Command::new("sc.exe")
        .args(["stop", "ZyvorGuestAgent"])
        .status();
    fs::copy(STAGED_AGENT, &dest).context("install staged exe")?;
    let _ = Command::new("sc.exe")
        .args(["start", "ZyvorGuestAgent"])
        .status();
    Ok(format!(
        "applied {} to {} (sha256 verified, service restarted)",
        meta.version,
        dest.display()
    ))
}

#[cfg(not(target_os = "windows"))]
fn apply_staged_update_windows(_meta: &StagedUpdateMeta) -> Result<String> {
    anyhow::bail!("windows staged apply only supported on Windows")
}

#[cfg(target_os = "linux")]
fn restart_agent_services_linux() {
    use std::process::Command;
    for unit in [
        "zyvor-guest-agent.service",
        "zyvor-guest-agent-exec.service",
    ] {
        let _ = Command::new("systemctl")
            .args(["try-restart", unit])
            .status();
    }
}

#[cfg(not(target_os = "linux"))]
fn restart_agent_services_linux() {}

pub async fn run_scheduled_update() -> Result<String> {
    let policy = crate::agent::policy::AgentPolicy::load();
    if !policy.actions.self_update.enabled {
        return Ok("self-update disabled by agent policy".into());
    }
    let check = check_update().await?;
    if !check.update_available {
        return Ok(format!(
            "Zyvor GuestAgent {} is current (channel {})",
            check.current_version, check.channel
        ));
    }
    if !policy.can_auto_apply_update() {
        return Ok(format!(
            "update {} available on channel {} (auto_apply disabled)",
            check.remote_version.unwrap_or_default(),
            check.channel
        ));
    }
    stage_update(true).await
}

pub fn sign_manifest_cli(manifest_json: &str) -> Result<String> {
    let manifest: crate::agent::update_sign::UpdateManifest =
        serde_json::from_str(manifest_json).context("parse manifest json")?;
    crate::agent::update_sign::sign_manifest(&manifest)
}
