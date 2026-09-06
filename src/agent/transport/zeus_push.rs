// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Outbound mTLS push to Zeus control plane.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_CONFIG: &str = "/etc/zyvor/guest-agent.toml";
#[cfg(target_os = "windows")]
const DEFAULT_CONFIG_WINDOWS: &str = "C:\\ProgramData\\zyvor\\guest-agent.toml";
const AGENT_STATE_PATH: &str = "/var/lib/zyvor/agent-state.json";
const DEFAULT_CERT_DIR: &str = "/var/lib/zyvor";
const DEFAULT_CERT_PATH: &str = "/var/lib/zyvor/agent.crt";
const DEFAULT_KEY_PATH: &str = "/var/lib/zyvor/agent.key";
const DEFAULT_CA_PATH: &str = "/var/lib/zyvor/ca.crt";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct AgentStateFile {
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ZeusPushConfig {
    pub zeus_url: Option<String>,
    pub agent_id: Option<String>,
    pub bootstrap_token: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub ca_path: Option<String>,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub vm_name: Option<String>,
}

fn default_interval() -> u64 {
    60
}

pub fn load_config() -> ZeusPushConfig {
    let path = std::env::var("ZYVOR_AGENT_CONFIG").unwrap_or_else(|_| {
        #[cfg(target_os = "windows")]
        {
            DEFAULT_CONFIG_WINDOWS.to_string()
        }
        #[cfg(not(target_os = "windows"))]
        {
            DEFAULT_CONFIG.to_string()
        }
    });
    if Path::new(&path).exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        ZeusPushConfig::default()
    }
}

#[derive(Debug, Serialize)]
struct RegisterRequest {
    hostname: String,
    agent_version: String,
    bootstrap_token: Option<String>,
    namespace: Option<String>,
    vm_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    agent_id: String,
}

#[derive(Debug, Deserialize)]
struct BootstrapCertData {
    cert_pem: String,
    key_pem: String,
    ca_pem: String,
}

pub async fn run_push_worker() -> Result<()> {
    let config = load_config();
    let zeus_url = config
        .zeus_url
        .clone()
        .filter(|u| !u.is_empty())
        .or_else(|| std::env::var("ZYVOR_ZEUS_URL").ok())
        .filter(|u| !u.is_empty());

    if zeus_url.is_none() {
        log::info!("Zeus push disabled (no zeus_url configured)");
        return Ok(());
    }

    let base = zeus_url.unwrap().trim_end_matches('/').to_string();
    let api_base = base.clone();
    let mut push_base = base.clone();
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut client = build_client(&config)?;

    if let Ok(info) = client
        .get(format!("{api_base}/api/v1/guest-agents/bootstrap-info"))
        .send()
        .await
    {
        if let Ok(body) = info.json::<serde_json::Value>().await {
            if body
                .pointer("/data/token_required")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && config.bootstrap_token.is_none()
            {
                log::warn!("Zeus requires AGENT_BOOTSTRAP_TOKEN but guest-agent.toml has no bootstrap_token");
            }
            if let Some(url) = body
                .pointer("/data/mtls_push_url")
                .and_then(|v| v.as_str())
                .filter(|u| !u.is_empty())
            {
                push_base = url.trim_end_matches('/').to_string();
            }
        }
    }

    let mut agent_id = config.agent_id.clone().or_else(load_persisted_agent_id);

    if agent_id.is_none() {
        let body = RegisterRequest {
            hostname: hostname.clone(),
            agent_version: crate::VERSION.to_string(),
            bootstrap_token: config.bootstrap_token.clone(),
            namespace: config
                .namespace
                .clone()
                .or_else(|| std::env::var("ZYVOR_VM_NAMESPACE").ok()),
            vm_name: config
                .vm_name
                .clone()
                .or_else(|| std::env::var("ZYVOR_VM_NAME").ok()),
        };
        let url = format!("{api_base}/api/v1/guest-agents/register");
        if let Ok(resp) = client.post(&url).json(&body).send().await {
            if let Ok(reg) = resp.json::<ApiEnvelope<RegisterResponse>>().await {
                log::info!("registered with Zeus as agent {}", reg.data.agent_id);
                agent_id = Some(reg.data.agent_id.clone());
                persist_agent_id(&reg.data.agent_id);
            }
        }
    }

    if let Err(e) = try_bootstrap_mtls(
        &client,
        &api_base,
        &hostname,
        config.bootstrap_token.as_ref(),
    )
    .await
    {
        log::warn!("mTLS bootstrap skipped: {e}");
    } else if mtls_material_present() {
        client = build_client(&config)?;
        if push_base != api_base {
            log::info!("using Zeus mTLS push endpoint {push_base}");
        }
    }

    let agent_id = agent_id.unwrap_or_else(|| "local".into());
    let interval = Duration::from_secs(config.interval_secs.max(15));
    let push_base = if mtls_material_present() && push_base != api_base {
        push_base
    } else {
        api_base
    };

    loop {
        if let Err(e) = push_heartbeat(&client, &push_base, &agent_id).await {
            log::warn!("heartbeat push failed: {e}");
        }
        if let Err(e) = push_report(&client, &push_base, &agent_id).await {
            log::warn!("report push failed: {e}");
        }
        tokio::time::sleep(interval).await;
    }
}

async fn push_heartbeat(client: &reqwest::Client, base: &str, agent_id: &str) -> Result<()> {
    let url = format!("{base}/api/v1/guest-agents/{agent_id}/heartbeat");
    // `build_agent_status_live`/`recent_systemd_events` touch zbus::blocking (systemd D-Bus),
    // which spins up its own Tokio runtime internally -- calling it directly from this async
    // task (already running on a multi-thread runtime worker) panics with "Cannot start a
    // runtime from within a runtime". `block_in_place` is Tokio's documented escape hatch:
    // it hands this worker thread off so the wrapped sync code can safely block/nest-block.
    let (status, recent_events, heartbeat) = tokio::task::block_in_place(|| {
        let status = crate::evidence::build_agent_status_live().unwrap_or_else(|_| {
            crate::evidence::AgentStatus {
                hostname: "unknown".into(),
                os_name: String::new(),
                os_version: String::new(),
                kernel: String::new(),
                architecture: String::new(),
                ips: vec![],
                boot_mode: String::new(),
                cloud_init_present: false,
                qga_ready: false,
                zyvor_agent_ready: true,
                virtio_modules_loaded: false,
                agent_version: crate::VERSION.to_string(),
                uptime_secs: 0,
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
            }
        });
        let recent_events = recent_systemd_events(50);
        let runtime = crate::agent::state::AgentRuntime::global();
        let heartbeat = runtime
            .last_heartbeat()
            .unwrap_or_else(|| crate::agent::heartbeat::build_heartbeat(&runtime));
        (status, recent_events, heartbeat)
    });
    let body = serde_json::json!({
        "status": status,
        "recent_events": recent_events,
        "heartbeat": heartbeat,
    });
    client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("heartbeat POST")?;
    Ok(())
}

async fn push_report(client: &reqwest::Client, base: &str, agent_id: &str) -> Result<()> {
    // See push_heartbeat: build_push_health touches zbus::blocking (systemd D-Bus) and must
    // not run directly on this async task's worker thread.
    let (recent_events, health_result) =
        tokio::task::block_in_place(|| (recent_systemd_events(100), build_push_health()));
    let (health, metrics) = health_result?;
    let body = serde_json::json!({
        "guest_health": health,
        "metrics": metrics,
        "recent_events": recent_events,
    });
    let url = format!("{base}/api/v1/guest-agents/{agent_id}/report");
    client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("report POST")?;
    Ok(())
}

fn build_push_health() -> Result<(guestkit_agent_protocol::GuestHealth, serde_json::Value)> {
    let health = crate::health::build_guest_health_live()?;
    #[cfg(target_os = "windows")]
    {
        let mut metrics_val = serde_json::json!({});
        if let Some(ev) = crate::collectors::windows_live::collect_windows_live() {
            if let Some(obj) = metrics_val.as_object_mut() {
                obj.insert("ips".into(), serde_json::json!(ev.ip_addresses));
                if let Some(gw) = ev.default_gateway {
                    obj.insert("default_gateway".into(), serde_json::json!(gw));
                }
            }
        }
        Ok((health, metrics_val))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let metrics = crate::metrics::collect_metrics_live();
        let mut metrics_val = serde_json::to_value(metrics)?;
        if let Some(obj) = metrics_val.as_object_mut() {
            obj.insert(
                "ips".into(),
                serde_json::json!(crate::evidence::live::collect_live_ips()),
            );
        }
        Ok((health, metrics_val))
    }
}

fn build_client(config: &ZeusPushConfig) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(30));
    let (cert_path, key_path, ca_path) = resolve_tls_paths(config);

    if let (Some(cert), Some(key)) = (cert_path, key_path) {
        let identity = reqwest::Identity::from_pem(
            &[
                std::fs::read(&cert).context("read cert")?,
                std::fs::read(&key).context("read key")?,
            ]
            .concat(),
        )?;
        builder = builder.identity(identity);
    }

    if let Some(ca) = ca_path {
        let pem = std::fs::read(&ca).context("read ca")?;
        let cert = reqwest::Certificate::from_pem(&pem).context("parse ca pem")?;
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(cert);
    }

    builder.build().context("build reqwest client")
}

fn resolve_tls_paths(
    config: &ZeusPushConfig,
) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let cert = config
        .cert_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| path_if_exists(DEFAULT_CERT_PATH));
    let key = config
        .key_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| path_if_exists(DEFAULT_KEY_PATH));
    let ca = config
        .ca_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| path_if_exists(DEFAULT_CA_PATH));
    (cert, key, ca)
}

fn path_if_exists(path: &str) -> Option<PathBuf> {
    if Path::new(path).exists() {
        Some(PathBuf::from(path))
    } else {
        None
    }
}

fn mtls_material_present() -> bool {
    Path::new(DEFAULT_CERT_PATH).exists() && Path::new(DEFAULT_KEY_PATH).exists()
}

async fn try_bootstrap_mtls(
    client: &reqwest::Client,
    base: &str,
    hostname: &str,
    bootstrap_token: Option<&String>,
) -> Result<()> {
    if mtls_material_present() {
        return Ok(());
    }

    let body = serde_json::json!({
        "hostname": hostname,
        "bootstrap_token": bootstrap_token,
    });
    let url = format!("{base}/api/v1/guest-agents/bootstrap");
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("bootstrap POST")?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("bootstrap HTTP {}", resp.status()));
    }

    let envelope = resp
        .json::<ApiEnvelope<BootstrapCertData>>()
        .await
        .context("parse bootstrap response")?;
    install_mtls_material(&envelope.data)?;
    Ok(())
}

fn install_mtls_material(data: &BootstrapCertData) -> Result<()> {
    fs::create_dir_all(DEFAULT_CERT_DIR).context("create cert dir")?;
    write_secret_file(Path::new(DEFAULT_CERT_PATH), &data.cert_pem)?;
    write_secret_file(Path::new(DEFAULT_KEY_PATH), &data.key_pem)?;
    write_secret_file(Path::new(DEFAULT_CA_PATH), &data.ca_pem)?;
    log::info!("installed mTLS credentials under {DEFAULT_CERT_DIR}");
    Ok(())
}

fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("chmod {}", path.display()))?;
    }
    Ok(())
}

fn load_persisted_agent_id() -> Option<String> {
    std::fs::read_to_string(AGENT_STATE_PATH)
        .ok()
        .and_then(|raw| serde_json::from_str::<AgentStateFile>(&raw).ok())
        .and_then(|s| s.agent_id)
        .filter(|id| !id.is_empty())
}

fn persist_agent_id(agent_id: &str) {
    let state = AgentStateFile {
        agent_id: Some(agent_id.to_string()),
    };
    if let Some(dir) = std::path::Path::new(AGENT_STATE_PATH).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(AGENT_STATE_PATH, json);
    }
}

fn recent_systemd_events(limit: usize) -> Vec<guestkit_agent_protocol::SystemdEvent> {
    #[cfg(target_os = "linux")]
    {
        crate::collectors::dbus::systemd_events::recent_events(limit)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = limit;
        Vec::new()
    }
}
