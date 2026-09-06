// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Strategy-aware guest agent installation (including QGA file bootstrap).

use serde::Deserialize;
use serde_json::json;

use crate::error::{ApiError, ApiResult};
use crate::kubevirt_guest_agent::{install_guest_agent, qga_bootstrap_script, GuestAgentInstallResult};
use crate::state::AppState;
use crate::vmtools_bundle::default_bundle_base_url;

use super::transport::probe_guest_context;

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallStrategy {
    #[default]
    Auto,
    CloudInitCurl,
    QgaFileBootstrap,
    QgaCurlBootstrap,
    OfflineInject,
    IsoAttach,
    BakedImage,
}

pub struct InstallAgentRequest {
    pub strategy: InstallStrategy,
    pub restart: bool,
    pub bundle_url: Option<String>,
}

pub async fn install_agent_strategy(
    state: &AppState,
    namespace: &str,
    name: &str,
    req: InstallAgentRequest,
) -> ApiResult<serde_json::Value> {
    let ctx = probe_guest_context(state, namespace, name).await;
    let strategy = match req.strategy {
        InstallStrategy::Auto => select_auto_strategy(&ctx),
        other => other,
    };

    match strategy {
        InstallStrategy::QgaFileBootstrap => {
            qga_file_bootstrap_install(state, namespace, name, req.bundle_url.as_deref()).await
        }
        InstallStrategy::QgaCurlBootstrap => {
            let client = state.kube.as_ref().ok_or_else(|| {
                ApiError::bad_request("Kubernetes client required for QGA bootstrap")
            })?;
            if !ctx.qga_connected {
                return Err(ApiError::bad_request(
                    "QGA not connected — cannot bootstrap agent",
                ));
            }
            let url = req
                .bundle_url
                .or_else(|| {
                    Some(format!(
                        "{}/linux/zyvor-guest-agent",
                        default_bundle_base_url(state).trim_end_matches('/')
                    ))
                })
                .ok_or_else(|| ApiError::bad_request("VMTOOLS bundle URL not configured"))?;
            let script = qga_bootstrap_script(&url);
            let result = if ctx.is_windows {
                crate::kubevirt_qga::qga_exec_powershell(client, namespace, name, &script, 180)
                    .await?
            } else {
                crate::kubevirt_qga::qga_exec_shell(client, namespace, name, &script, 180).await?
            };
            Ok(json!({
                "strategy": "qga_curl_bootstrap",
                "networkRequired": false,
                "success": result.exit_code == 0,
                "exitCode": result.exit_code,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }))
        }
        InstallStrategy::BakedImage => Ok(json!({
            "strategy": "baked_image",
            "pending": false,
            "message": "Agent appears baked into image or cloud-init",
            "zyvorAgentInstalled": ctx.zyvor_agent_installed,
        })),
        InstallStrategy::OfflineInject => Err(ApiError::bad_request(
            "Use POST /guest/repair-plan with inject_zyvor_agent for halted VMs",
        )),
        _ => {
            let method = match strategy {
                InstallStrategy::IsoAttach => "iso",
                InstallStrategy::CloudInitCurl => "cloud-init",
                _ => "auto",
            };
            let client = state.kube.as_ref().ok_or_else(|| {
                ApiError::bad_request("Kubernetes client required for agent install")
            })?;
            let result: GuestAgentInstallResult = install_guest_agent(
                client.clone(),
                namespace,
                name,
                req.restart,
                Some(method),
            )
            .await?;
            Ok(serde_json::to_value(result).unwrap_or(json!({})))
        }
    }
}

fn select_auto_strategy(ctx: &super::transport::GuestContext) -> InstallStrategy {
    if !ctx.vmi_running {
        return InstallStrategy::OfflineInject;
    }
    if ctx.zyvor_agent_installed && ctx.agent_daemon_running {
        return InstallStrategy::BakedImage;
    }
    if ctx.qga_connected && !ctx.network_available {
        return InstallStrategy::QgaFileBootstrap;
    }
    if ctx.qga_connected {
        return InstallStrategy::QgaCurlBootstrap;
    }
    InstallStrategy::CloudInitCurl
}

async fn qga_file_bootstrap_install(
    state: &AppState,
    namespace: &str,
    name: &str,
    bundle_url: Option<&str>,
) -> ApiResult<serde_json::Value> {
    let client = state.kube.as_ref().ok_or_else(|| {
        ApiError::bad_request("Kubernetes client required for QGA file bootstrap")
    })?;

    let ctx = probe_guest_context(state, namespace, name).await;
    let bundle_bytes = if ctx.is_windows {
        load_windows_bundle_bytes(state, bundle_url).await?
    } else {
        load_agent_bundle_bytes(state, bundle_url).await?
    };
    let guest_path = if ctx.is_windows {
        "C:\\\\ProgramData\\\\Zyvor\\\\zyvor-vm-tools.zip"
    } else {
        "/tmp/zyvor-vm-tools.tar.gz"
    };

    crate::kubevirt_qga::qga_file_write(client, namespace, name, guest_path, &bundle_bytes, 300)
        .await?;

    let install_script = if ctx.is_windows {
        r#"$dest='C:\ProgramData\Zyvor\install'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path 'C:\ProgramData\Zyvor\zyvor-vm-tools.zip' -DestinationPath $dest -Force
if (Test-Path "$dest\zyvor-guest-agent.exe") { Copy-Item "$dest\zyvor-guest-agent.exe" 'C:\Program Files\Zyvor\zyvor-guest-agent.exe' -Force }
if (Test-Path "$dest\install.ps1") { & "$dest\install.ps1" -ErrorAction SilentlyContinue }
(Get-Service zyvor-guest-agent -ErrorAction SilentlyContinue).Status"#
    } else {
        r#"set -eu
mkdir -p /tmp/zyvor-install
tar xzf /tmp/zyvor-vm-tools.tar.gz -C /tmp/zyvor-install
install -m755 /tmp/zyvor-install/zyvor-guest-agent /usr/local/bin/zyvor-guest-agent
install -m644 /tmp/zyvor-install/zyvor-guest-agent.service /etc/systemd/system/zyvor-guest-agent.service
systemctl daemon-reload
systemctl enable --now zyvor-guest-agent
systemctl is-active zyvor-guest-agent"#
    };

    let exec = if ctx.is_windows {
        crate::kubevirt_qga::qga_exec_powershell(client, namespace, name, install_script, 180)
            .await?
    } else {
        crate::kubevirt_qga::qga_exec_shell(client, namespace, name, install_script, 180).await?
    };

    Ok(json!({
        "strategy": "qga_file_bootstrap",
        "networkRequired": false,
        "transport": "qga-builtin",
        "success": exec.exit_code == 0,
        "exitCode": exec.exit_code,
        "stdout": exec.stdout,
        "stderr": exec.stderr,
        "message": "Installed zyvor-guest-agent via QGA file-write (airgap)",
    }))
}

async fn load_windows_bundle_bytes(
    state: &AppState,
    bundle_url: Option<&str>,
) -> Result<Vec<u8>, ApiError> {
    let version = std::env::var("VMTOOLS_VERSION").unwrap_or_else(|_| "0.1.0".into());
    let url = bundle_url
        .map(String::from)
        .or_else(|| {
            Some(format!(
                "{}/windows/zyvor-vm-tools-windows-{}.zip",
                default_bundle_base_url(state).trim_end_matches('/'),
                version
            ))
        })
        .ok_or_else(|| ApiError::bad_request("Windows VM tools bundle URL not configured"))?;
    fetch_bundle_url(&url).await
}

async fn fetch_bundle_url(url: &str) -> Result<Vec<u8>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(format!("fetch bundle: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError::bad_request(format!(
            "bundle fetch HTTP {}",
            resp.status()
        )));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| ApiError::internal(e.to_string()))
}

async fn load_agent_bundle_bytes(
    state: &AppState,
    bundle_url: Option<&str>,
) -> Result<Vec<u8>, ApiError> {
    let url = bundle_url
        .map(String::from)
        .or_else(|| {
            Some(format!(
                "{}/linux/zyvor-vm-tools-linux-amd64.tar.gz",
                default_bundle_base_url(state).trim_end_matches('/')
            ))
        })
        .ok_or_else(|| ApiError::bad_request("VM tools bundle URL not configured"))?;
    fetch_bundle_url(&url).await
}
