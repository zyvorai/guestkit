// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub redis_url: String,
    pub storage_path: PathBuf,
    pub storage_public_url: String,
    pub default_namespace: String,
    pub storage_class: String,
    /// Maximum multipart upload size in bytes (default 2 GiB).
    pub max_upload_bytes: usize,
    /// Default guestkit agent-proxy URL for live (online) VM inspection.
    pub agent_proxy_url: Option<String>,
    pub zeus_public_url: Option<String>,
    pub cluster_name: Option<String>,
    /// Extra directories the UI may browse for on-server disks (comma-separated env).
    pub storage_browse_paths: Vec<PathBuf>,
    pub auth_enabled: bool,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub jwt_expiry_hours: u32,
    pub public_base_url: String,
    pub ui_base_url: String,
    pub default_role: String,
    pub nfs_mount_root: PathBuf,
    /// When set, guest agent registration must present this bootstrap token.
    pub agent_bootstrap_token: Option<String>,
    pub agent_ca_dir: PathBuf,
    /// Optional dedicated mTLS listener for guest-agent push (e.g. 0.0.0.0:8443).
    pub agent_mtls_bind_addr: Option<String>,
    /// Public URL agents use for mTLS push (e.g. https://agent-api.zyvor.local).
    pub agent_mtls_public_url: Option<String>,
    /// Optional PacketWolf correlation ingest URL for guest health events.
    pub packetwolf_correlation_url: Option<String>,
    /// Optional PacketWolf fleet batch correlation URL (defaults to correlation URL + /fleet).
    pub packetwolf_fleet_correlate_url: Option<String>,
    /// Interval between fleet correlation sweeps in seconds.
    pub packetwolf_fleet_interval_secs: u64,
}

impl Config {
    pub fn storage_browse_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.storage_path.clone()];
        for path in &self.storage_browse_paths {
            if !roots.iter().any(|r| r == path) {
                roots.push(path.clone());
            }
        }
        roots
    }

    pub fn jwt_expiry_secs(&self) -> usize {
        (self.jwt_expiry_hours as usize) * 3600
    }

    pub fn default_role(&self) -> &str {
        &self.default_role
    }

    /// True when the API listens only on loopback (127.0.0.1 / ::1).
    pub fn is_localhost_bind(&self) -> bool {
        self.bind_addr
            .parse::<std::net::SocketAddr>()
            .map(|addr| match addr.ip() {
                std::net::IpAddr::V4(v4) => v4.is_loopback(),
                std::net::IpAddr::V6(v6) => v6.is_loopback(),
            })
            .unwrap_or(false)
    }

    /// Log security warnings for misconfigured or evaluation-only deployments.
    pub fn emit_startup_warnings(&self) {
        if !self.auth_enabled {
            if self.is_localhost_bind() {
                tracing::warn!(
                    "SECURITY: AUTH_ENABLED=false — evaluation/local mode only. \
                     Set AUTH_ENABLED=true and a strong JWT_SECRET before production."
                );
            } else {
                tracing::warn!(
                    "SECURITY: AUTH_ENABLED=false while listening on {} — \
                     all API routes are unauthenticated. NOT safe for production.",
                    self.bind_addr
                );
            }
        }
        if self.agent_bootstrap_token.is_none() {
            tracing::warn!(
                "SECURITY: AGENT_BOOTSTRAP_TOKEN unset — guest agent registration and \
                 certificate issuance are open. Set AGENT_BOOTSTRAP_TOKEN before production."
            );
        }
        if self.redis_url.contains("redis://redis:6379")
            && !self.redis_url.contains('@')
            && self.auth_enabled
        {
            tracing::warn!(
                "SECURITY: Redis has no password in REDIS_URL — enable Redis AUTH in production."
            );
        }
    }

    pub fn oidc_redirect_uri(&self) -> String {
        format!(
            "{}/api/v1/auth/oidc/callback",
            self.public_base_url.trim_end_matches('/')
        )
    }

    pub fn from_env() -> Result<Self> {
        let public_base_url = std::env::var("PUBLIC_BASE_URL")
            .or_else(|_| std::env::var("API_PUBLIC_URL"))
            .unwrap_or_else(|_| "http://localhost:8080".into());
        let ui_base_url = std::env::var("UI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:30081".into());
        let auth_enabled = std::env::var("AUTH_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        // Fail closed: when auth is enabled, refuse to start unless a real
        // JWT signing secret is provided. Previously this silently fell back
        // to a hardcoded, globally-known key ("change-me-in-production"),
        // which would let anyone forge valid operator tokens.
        let jwt_secret = match std::env::var("JWT_SECRET") {
            Ok(s) if !s.trim().is_empty() && s != "change-me-in-production" => s,
            _ if auth_enabled => {
                anyhow::bail!(
                    "AUTH_ENABLED=true but JWT_SECRET is unset or insecure. \
                     Set JWT_SECRET to a strong random value (e.g. `openssl rand -base64 32`) \
                     before enabling authentication."
                );
            }
            _ => "dev-local-auth-disabled".into(),
        };

        let config = Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://zyvor:zyvor@localhost:5432/zyvor".into()),
            redis_url: std::env::var("REDIS_URL")
                .or_else(|_| std::env::var("QUEUE_URL"))
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            storage_path: PathBuf::from(
                std::env::var("STORAGE_PATH").unwrap_or_else(|_| "/var/lib/zyvor/images".into()),
            ),
            storage_public_url: std::env::var("STORAGE_PUBLIC_URL")
                .unwrap_or_else(|_| "http://minio:9000/vm-images".into()),
            default_namespace: std::env::var("DEFAULT_NAMESPACE")
                .unwrap_or_else(|_| "zyvor".into()),
            storage_class: std::env::var("STORAGE_CLASS")
                .unwrap_or_else(|_| "longhorn".into()),
            max_upload_bytes: std::env::var("MAX_UPLOAD_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2 * 1024 * 1024 * 1024),
            agent_proxy_url: std::env::var("AGENT_PROXY_URL")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            zeus_public_url: std::env::var("ZEUS_PUBLIC_URL")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            cluster_name: std::env::var("CLUSTER_NAME")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            storage_browse_paths: std::env::var("STORAGE_BROWSE_PATHS")
                .ok()
                .map(|raw| {
                    raw.split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect()
                })
                .unwrap_or_default(),
            auth_enabled,
            jwt_secret,
            jwt_issuer: std::env::var("JWT_ISSUER").unwrap_or_else(|_| "guestkit".into()),
            jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            public_base_url,
            ui_base_url,
            default_role: std::env::var("DEFAULT_ROLE").unwrap_or_else(|_| "operator".into()),
            nfs_mount_root: PathBuf::from(
                std::env::var("NFS_MOUNT_ROOT").unwrap_or_else(|_| "/mnt/nfs".into()),
            ),
            agent_bootstrap_token: std::env::var("AGENT_BOOTSTRAP_TOKEN")
                .ok()
                .filter(|t| !t.trim().is_empty()),
            agent_ca_dir: PathBuf::from(
                std::env::var("AGENT_CA_DIR").unwrap_or_else(|_| "/var/lib/zyvor/agent-ca".into()),
            ),
            agent_mtls_bind_addr: std::env::var("AGENT_MTLS_BIND_ADDR")
                .ok()
                .filter(|a| !a.trim().is_empty()),
            agent_mtls_public_url: std::env::var("AGENT_MTLS_PUBLIC_URL")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            packetwolf_correlation_url: std::env::var("PACKETWOLF_CORRELATION_URL")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            packetwolf_fleet_correlate_url: std::env::var("PACKETWOLF_FLEET_CORRELATE_URL")
                .ok()
                .filter(|u| !u.trim().is_empty()),
            packetwolf_fleet_interval_secs: std::env::var("PACKETWOLF_FLEET_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        };

        validate_agent_bootstrap_requirements(
            config.auth_enabled,
            &config.agent_mtls_bind_addr,
            &config.agent_bootstrap_token,
        )?;

        Ok(config)
    }
}

/// Fail closed when production-oriented auth or mTLS is enabled without agent bootstrap.
fn validate_agent_bootstrap_requirements(
    auth_enabled: bool,
    agent_mtls_bind_addr: &Option<String>,
    agent_bootstrap_token: &Option<String>,
) -> Result<()> {
    if auth_enabled && agent_bootstrap_token.is_none() {
        anyhow::bail!(
            "AUTH_ENABLED=true but AGENT_BOOTSTRAP_TOKEN is unset. \
             Guest agent registration and certificate issuance must be protected in production. \
             Set AGENT_BOOTSTRAP_TOKEN to a strong random value (e.g. `openssl rand -base64 32`)."
        );
    }
    if agent_mtls_bind_addr.is_some() && agent_bootstrap_token.is_none() {
        anyhow::bail!(
            "AGENT_MTLS_BIND_ADDR is set but AGENT_BOOTSTRAP_TOKEN is unset. \
             mTLS guest-agent push requires a bootstrap token before issuing client certificates."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_bind_detection() {
        let mut cfg = Config {
            bind_addr: "127.0.0.1:8080".into(),
            database_url: String::new(),
            redis_url: String::new(),
            storage_path: PathBuf::new(),
            storage_public_url: String::new(),
            default_namespace: String::new(),
            storage_class: String::new(),
            max_upload_bytes: 0,
            agent_proxy_url: None,
            zeus_public_url: None,
            cluster_name: None,
            storage_browse_paths: vec![],
            auth_enabled: false,
            jwt_secret: String::new(),
            jwt_issuer: String::new(),
            jwt_expiry_hours: 0,
            public_base_url: String::new(),
            ui_base_url: String::new(),
            default_role: String::new(),
            nfs_mount_root: PathBuf::new(),
            agent_bootstrap_token: None,
            agent_ca_dir: PathBuf::new(),
            agent_mtls_bind_addr: None,
            agent_mtls_public_url: None,
            packetwolf_correlation_url: None,
            packetwolf_fleet_correlate_url: None,
            packetwolf_fleet_interval_secs: 0,
        };
        assert!(cfg.is_localhost_bind());
        cfg.bind_addr = "0.0.0.0:8080".into();
        assert!(!cfg.is_localhost_bind());
    }

    #[test]
    fn auth_enabled_requires_agent_bootstrap_token() {
        let err = validate_agent_bootstrap_requirements(true, &None, &None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("AGENT_BOOTSTRAP_TOKEN"));
    }

    #[test]
    fn agent_mtls_requires_bootstrap_token() {
        let mtls = Some("0.0.0.0:8443".into());
        let err = validate_agent_bootstrap_requirements(false, &mtls, &None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("AGENT_BOOTSTRAP_TOKEN"));
    }
}
