// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! VMToolsBundle CR — versioned artifact URLs (MinIO / registry).

use kube::api::{Api, Patch, PatchParams, PostParams};
use kube::discovery::ApiResource;
use kube::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const VMTOOLS_VERSION: &str = "0.1.0";

pub const DEFAULT_BUNDLE_NAME: &str = "cluster-default";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VMToolsWindowsArtifacts {
    #[serde(default)]
    pub exe: String,
    #[serde(default)]
    pub msi: String,
    #[serde(default)]
    pub zip: String,
    #[serde(default)]
    pub install_ps1: String,
    #[serde(default)]
    pub zip_sha256: String,
    #[serde(default)]
    pub zip_signature: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VMToolsBundleSpec {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub linux: VMToolsLinuxArtifacts,
    #[serde(default)]
    pub windows: VMToolsWindowsArtifacts,
    #[serde(default)]
    pub iso: String,
    #[serde(default)]
    pub agent_binary_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VMToolsLinuxArtifacts {
    #[serde(default)]
    pub rpm: String,
    #[serde(default)]
    pub deb: String,
    #[serde(default)]
    pub tar: String,
    #[serde(default)]
    pub tar_sha256: String,
    #[serde(default)]
    pub tar_signature: String,
}

pub fn vmtoolsbundle_resource() -> ApiResource {
    ApiResource {
        group: "zeus.zyvor.dev".into(),
        version: "v1alpha1".into(),
        api_version: "zeus.zyvor.dev/v1alpha1".into(),
        kind: "VMToolsBundle".into(),
        plural: "vmtoolsbundles".into(),
    }
}

pub fn default_bundle_base_url(state: &AppState) -> String {
    std::env::var("VMTOOLS_BUNDLE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .or_else(|| {
            state
                .config
                .zeus_public_url
                .as_ref()
                .map(|u| format!("{}/api/v1/engines/vmtools", u.trim_end_matches('/')))
        })
        .unwrap_or_else(|| "http://zyvor-api:8080/api/v1/vmtools/bundle".into())
}

pub fn bundle_spec_from_env(state: &AppState) -> VMToolsBundleSpec {
    let base = default_bundle_base_url(state);
    let version = std::env::var("VMTOOLS_VERSION").unwrap_or_else(|_| VMTOOLS_VERSION.into());
    let channel = std::env::var("VMTOOLS_CHANNEL").unwrap_or_else(|_| "stable".into());
    let agent = crate::kubevirt_guest_agent::resolve_guestkit_binary_url();
    let tar_sha256 = std::env::var("VMTOOLS_LINUX_TAR_SHA256").unwrap_or_default();
    let tar_signature = std::env::var("VMTOOLS_LINUX_TAR_SIGNATURE").unwrap_or_default();
    let windows_zip_sha256 = std::env::var("VMTOOLS_WINDOWS_ZIP_SHA256").unwrap_or_default();
    let windows_zip_signature = std::env::var("VMTOOLS_WINDOWS_ZIP_SIGNATURE").unwrap_or_default();
    VMToolsBundleSpec {
        version: version.clone(),
        channel,
        linux: VMToolsLinuxArtifacts {
            rpm: format!("{base}/linux/zyvor-vm-tools-{version}.rpm"),
            deb: format!("{base}/linux/zyvor-vm-tools_{version}_amd64.deb"),
            tar: format!("{base}/linux/zyvor-vm-tools-linux-amd64.tar.gz"),
            tar_sha256,
            tar_signature,
        },
        windows: VMToolsWindowsArtifacts {
            exe: format!("{base}/windows/zyvor-guest-agent.exe"),
            msi: format!("{base}/windows/zyvor-vm-tools-{version}.msi"),
            zip: format!("{base}/windows/zyvor-vm-tools-windows-{version}.zip"),
            install_ps1: format!("{base}/windows/install.ps1"),
            zip_sha256: windows_zip_sha256,
            zip_signature: windows_zip_signature,
        },
        iso: format!("{base}/zyvor-vm-tools.iso"),
        agent_binary_url: agent,
    }
}

pub async fn fetch_bundle_spec(client: &Client) -> ApiResult<VMToolsBundleSpec> {
    let api: Api<kube::api::DynamicObject> =
        Api::all_with(client.clone(), &vmtoolsbundle_resource());
    match api.get(DEFAULT_BUNDLE_NAME).await {
        Ok(obj) => {
            let val = serde_json::to_value(&obj)
                .map_err(|e| ApiError::internal(format!("serialize VMToolsBundle: {e}")))?;
            Ok(parse_bundle_spec(&val))
        }
        Err(_) => Err(ApiError::internal("VMToolsBundle not found")),
    }
}

pub async fn fetch_bundle_spec_optional(client: &Client) -> Option<VMToolsBundleSpec> {
    fetch_bundle_spec(client).await.ok()
}

fn parse_bundle_spec(val: &Value) -> VMToolsBundleSpec {
    let spec = val.get("spec").cloned().unwrap_or(json!({}));
    let linux = spec.get("linux").cloned().unwrap_or(json!({}));
    let windows = spec.get("windows").cloned().unwrap_or(json!({}));
    VMToolsBundleSpec {
        version: spec
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or(VMTOOLS_VERSION)
            .into(),
        channel: spec
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("stable")
            .into(),
        linux: VMToolsLinuxArtifacts {
            rpm: linux
                .get("rpm")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            deb: linux
                .get("deb")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            tar: linux
                .get("tar")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            tar_sha256: linux
                .get("tarSha256")
                .or_else(|| linux.get("tar_sha256"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            tar_signature: linux
                .get("tarSignature")
                .or_else(|| linux.get("tar_signature"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
        },
        windows: VMToolsWindowsArtifacts {
            exe: windows
                .get("exe")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            msi: windows
                .get("msi")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            zip: windows
                .get("zip")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            install_ps1: windows
                .get("installPs1")
                .or_else(|| windows.get("install_ps1"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            zip_sha256: windows
                .get("zipSha256")
                .or_else(|| windows.get("zip_sha256"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            zip_signature: windows
                .get("zipSignature")
                .or_else(|| windows.get("zip_signature"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
        },
        iso: spec
            .get("iso")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .into(),
        agent_binary_url: spec
            .get("agentBinaryUrl")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .into(),
    }
}

pub async fn upsert_default_bundle(client: &Client, spec: &VMToolsBundleSpec) -> ApiResult<()> {
    let api: Api<kube::api::DynamicObject> =
        Api::all_with(client.clone(), &vmtoolsbundle_resource());
    let body = json!({
        "apiVersion": "zeus.zyvor.dev/v1alpha1",
        "kind": "VMToolsBundle",
        "metadata": {
            "name": DEFAULT_BUNDLE_NAME,
            "labels": {
                "app.kubernetes.io/name": "zyvor",
                "app.kubernetes.io/component": "vmtools",
            }
        },
        "spec": {
            "version": spec.version,
            "channel": spec.channel,
            "linux": {
                "rpm": spec.linux.rpm,
                "deb": spec.linux.deb,
                "tar": spec.linux.tar,
            },
            "windows": {
                "exe": spec.windows.exe,
                "msi": spec.windows.msi,
                "zip": spec.windows.zip,
                "installPs1": spec.windows.install_ps1,
                "zipSha256": spec.windows.zip_sha256,
                "zipSignature": spec.windows.zip_signature,
            },
            "iso": spec.iso,
            "agentBinaryUrl": spec.agent_binary_url,
        }
    });
    if api.get(DEFAULT_BUNDLE_NAME).await.is_ok() {
        api.patch(
            DEFAULT_BUNDLE_NAME,
            &PatchParams::default(),
            &Patch::Merge(&json!({ "spec": body.get("spec").cloned().unwrap_or(Value::Null) })),
        )
        .await
        .map_err(|e| ApiError::internal(format!("patch VMToolsBundle: {e}")))?;
    } else {
        let obj: kube::api::DynamicObject = serde_json::from_value(body)
            .map_err(|e| ApiError::internal(format!("build VMToolsBundle: {e}")))?;
        api.create(&PostParams::default(), &obj)
            .await
            .map_err(|e| ApiError::internal(format!("create VMToolsBundle: {e}")))?;
    }
    Ok(())
}
