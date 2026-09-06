// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Zeus VM Tools — bundle, fleet coverage, install orchestration.

use axum::extract::{Path, Query, State};
use axum::Json;
use kube::api::{Api, Patch, PatchParams};
use kube::discovery::ApiResource;
use kube::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::{ApiError, ApiResult};
use crate::kubevirt_guest_agent::{
    install_guest_agent, install_vmtools_iso, vm_is_windows, zyvor_tools_connected,
    GuestAgentInstallResult,
};
use crate::vmtools_bundle::{self, fetch_bundle_spec_optional, VMToolsBundleSpec};
use crate::models::ApiResponse;
use crate::routes::kubevirt::{
    fetch_vm, fetch_vmi, get_guest_agent_info, list_dynamic_all, vm_resource, vmi_resource,
    GuestAgentInfo,
};
use crate::state::AppState;

pub use crate::vmtools_bundle::VMTOOLS_VERSION;

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsBundleInfo {
    pub version: String,
    pub channel: String,
    pub linux_rpm_url: Option<String>,
    pub linux_deb_url: Option<String>,
    pub linux_tar_url: Option<String>,
    pub windows_exe_url: Option<String>,
    pub windows_msi_url: Option<String>,
    pub windows_zip_url: Option<String>,
    pub windows_install_ps1_url: Option<String>,
    pub iso_url: Option<String>,
    pub agent_binary_url: String,
    pub linux_tar_sha256: Option<String>,
    pub linux_tar_signature: Option<String>,
    pub windows_zip_sha256: Option<String>,
    pub windows_zip_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsCoverage {
    pub total_vms: usize,
    pub installed: usize,
    pub connected: usize,
    pub pending: usize,
    pub missing: usize,
    pub outdated: usize,
    pub windows_virtio_win: usize,
    pub health_healthy: usize,
    pub health_degraded: usize,
    pub health_unhealthy: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsVmStatus {
    pub namespace: String,
    pub name: String,
    pub product: String,
    pub installed: bool,
    pub connected: bool,
    pub version: Option<String>,
    pub recommended_method: String,
    pub os_name: Option<String>,
    pub os_family: String,
    pub ip_address: Option<String>,
    pub qga_ready: bool,
    pub zyvor_agent_ready: bool,
    pub snapshot_quiesce: bool,
    pub message: String,
    pub guest_agent: GuestAgentInfo,
}

#[derive(Debug, Clone)]
pub struct VMToolsCrSnapshot {
    pub installed: bool,
    pub connected: bool,
    pub os_name: Option<String>,
    pub ip_address: Option<String>,
    pub qga_ready: bool,
    pub zyvor_agent_ready: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct InstallQuery {
    #[serde(default)]
    pub restart: Option<bool>,
    /// Install method: auto, cloud-init, qga, iso
    #[serde(default)]
    pub method: Option<String>,
}

pub fn bundle_info_with_spec(state: &AppState, spec: Option<&VMToolsBundleSpec>) -> VMToolsBundleInfo {
    let spec = spec.cloned().unwrap_or_else(|| vmtools_bundle::bundle_spec_from_env(state));
    let agent = if spec.agent_binary_url.is_empty() {
        crate::kubevirt_guest_agent::resolve_guestkit_binary_url()
    } else {
        spec.agent_binary_url.clone()
    };
    VMToolsBundleInfo {
        version: if spec.version.is_empty() {
            VMTOOLS_VERSION.into()
        } else {
            spec.version.clone()
        },
        channel: if spec.channel.is_empty() {
            std::env::var("VMTOOLS_CHANNEL").unwrap_or_else(|_| "stable".into())
        } else {
            spec.channel.clone()
        },
        linux_rpm_url: non_empty(Some(spec.linux.rpm.clone())),
        linux_deb_url: non_empty(Some(spec.linux.deb.clone())),
        linux_tar_url: non_empty(Some(spec.linux.tar.clone())),
        windows_exe_url: non_empty(Some(spec.windows.exe.clone())),
        windows_msi_url: non_empty(Some(spec.windows.msi.clone())),
        windows_zip_url: non_empty(Some(spec.windows.zip.clone())),
        windows_install_ps1_url: non_empty(Some(spec.windows.install_ps1.clone())),
        iso_url: non_empty(Some(spec.iso.clone())),
        agent_binary_url: agent,
        linux_tar_sha256: non_empty(
            std::env::var("VMTOOLS_LINUX_TAR_SHA256")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let sha = spec.linux.tar_sha256.clone();
                    if sha.is_empty() {
                        None
                    } else {
                        Some(sha)
                    }
                }),
        ),
        linux_tar_signature: non_empty(
            std::env::var("VMTOOLS_LINUX_TAR_SIGNATURE")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let sig = spec.linux.tar_signature.clone();
                    if sig.is_empty() {
                        None
                    } else {
                        Some(sig)
                    }
                }),
        ),
        windows_zip_sha256: non_empty(
            std::env::var("VMTOOLS_WINDOWS_ZIP_SHA256")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let sha = spec.windows.zip_sha256.clone();
                    if sha.is_empty() {
                        None
                    } else {
                        Some(sha)
                    }
                }),
        ),
        windows_zip_signature: non_empty(
            std::env::var("VMTOOLS_WINDOWS_ZIP_SIGNATURE")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    let sig = spec.windows.zip_signature.clone();
                    if sig.is_empty() {
                        None
                    } else {
                        Some(sig)
                    }
                }),
        ),
    }
}

fn non_empty(url: Option<String>) -> Option<String> {
    url.filter(|u| !u.trim().is_empty())
}

pub async fn bundle_info_async(state: &AppState, client: &Client) -> VMToolsBundleInfo {
    let spec = fetch_bundle_spec_optional(client).await;
    bundle_info_with_spec(state, spec.as_ref())
}

pub async fn get_bundle(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<VMToolsBundleInfo>>> {
    let client = kube_client(&state)?;
    Ok(Json(ApiResponse::ok(bundle_info_async(&state, &client).await)))
}

pub async fn get_coverage(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<VMToolsCoverage>>> {
    let client = kube_client(&state)?;
    let bundle = bundle_info_async(&state, &client).await;
    let target_version = bundle.version.clone();
    let vms = list_dynamic_all(&client, &vm_resource()).await?;
    let vmis = list_dynamic_all(&client, &vmi_resource()).await?;
    let mut vmi_map: HashMap<(String, String), Value> = HashMap::new();
    for vmi in vmis {
        let ns = vmi
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = vmi
            .pointer("/metadata/labels/kubevirt.io/vm")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                vmi.pointer("/metadata/name")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_default();
        vmi_map.insert((ns, name), vmi);
    }

    let mut coverage = VMToolsCoverage {
        total_vms: 0,
        installed: 0,
        connected: 0,
        pending: 0,
        missing: 0,
        outdated: 0,
        windows_virtio_win: 0,
        health_healthy: 0,
        health_degraded: 0,
        health_unhealthy: 0,
    };

    let (health_healthy, health_degraded, health_unhealthy) = if let Some(client) = state.kube.as_ref() {
        crate::guest_action_policy::count_guest_health(client).await
    } else {
        (0, 0, 0)
    };
    coverage.health_healthy = health_healthy;
    coverage.health_degraded = health_degraded;
    coverage.health_unhealthy = health_unhealthy;

    for vm in vms {
        let name = vm.pointer("/metadata/name").and_then(|v| v.as_str()).unwrap_or("");
        let namespace = vm
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() || namespace.is_empty() {
            continue;
        }
        coverage.total_vms += 1;
        let vmi = vmi_map.get(&(namespace.to_string(), name.to_string()));
        let is_win = vm_is_windows(Some(&vm), vmi);
        if is_win {
            coverage.windows_virtio_win += 1;
        }
        let label = vm
            .pointer("/metadata/labels/zeus.zyvor.dev/guest-tools")
            .and_then(|v| v.as_str())
            .unwrap_or("missing");
        let zyvor_ok = vmi
            .map(|v| zyvor_tools_connected(&vm, Some(v)))
            .unwrap_or(false);
        if zyvor_ok {
            coverage.connected += 1;
            coverage.installed += 1;
        } else if label == "pending" {
            coverage.pending += 1;
        } else {
            coverage.missing += 1;
        }
        let version = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/tools-version")
            .and_then(|v| v.as_str());
        if zyvor_ok {
            if let Some(ver) = version {
                if version_outdated(ver, &target_version) {
                    coverage.outdated += 1;
                }
            }
        }
    }

    Ok(Json(ApiResponse::ok(coverage)))
}

pub async fn get_vm_vmtools(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<VMToolsVmStatus>>> {
    let guest = get_guest_agent_info(
        State(state.clone()),
        Path((namespace.clone(), name.clone())),
    )
    .await?
    .0
    .data;
    let status = build_vm_vmtools_status(&state, &namespace, &name, guest).await?;
    Ok(Json(ApiResponse::ok(status)))
}

pub async fn install_vm_vmtools(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<InstallQuery>,
) -> ApiResult<Json<ApiResponse<GuestAgentInstallResult>>> {
    let client = kube_client(&state)?;
    let restart = query.restart.unwrap_or(true);
    let method = query.method.as_deref().unwrap_or("auto").to_lowercase();
    let result = if method == "iso" {
        let bundle = bundle_info_async(&state, &client).await;
        let iso_url = bundle
            .iso_url
            .ok_or_else(|| ApiError::bad_request("VM Tools ISO URL is not configured"))?;
        install_vmtools_iso(client.clone(), &namespace, &name, &iso_url, restart).await?
    } else if method == "qga" || method == "cloud-init" || method == "auto" {
        install_guest_agent(client.clone(), &namespace, &name, restart, Some(&method)).await?
    } else {
        install_guest_agent(client.clone(), &namespace, &name, restart, Some("auto")).await?
    };
    if result.success && !result.is_windows {
        let bundle = bundle_info_async(&state, &client).await;
        let vm = fetch_vm(&client, &namespace, &name).await;
        let vmi = fetch_vmi(&client, &namespace, &name).await;
        apply_install_labels(
            &client,
            &namespace,
            &name,
            vm.as_ref(),
            vmi.as_ref(),
            &bundle.version,
            &result,
        )
        .await;
    }
    Ok(Json(ApiResponse::ok(result)))
}

pub async fn run_vm_diagnostics(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    let guest = get_guest_agent_info(
        State(state.clone()),
        Path((namespace.clone(), name.clone())),
    )
    .await?
    .0
    .data;
    if !guest.agent_connected {
        return Err(ApiError::bad_request(
            "Guest agent not connected — install Zeus VM Tools first",
        ));
    }
    sync_vm_tools_labels(
        state.kube.as_ref().unwrap(),
        &namespace,
        &name,
        "connected",
        VMTOOLS_VERSION,
        Some(VMToolsCrSnapshot {
            installed: true,
            connected: true,
            os_name: guest.os_name.clone(),
            ip_address: guest
                .interfaces
                .as_ref()
                .and_then(|ifs| {
                    ifs.iter()
                        .find_map(|i| i.get("ipAddress").and_then(|v| v.as_str()))
                })
                .map(String::from),
            qga_ready: true,
            zyvor_agent_ready: true,
        }),
    )
    .await;
    Ok(Json(ApiResponse::ok(json!({
        "health": guest.health,
        "message": guest.message,
        "os_name": guest.os_name,
        "agent_version": guest.guest_agent_version,
        "diagnostics": "Live guest agent reachable — run Doctor via agent RPC for full score"
    }))))
}

async fn build_vm_vmtools_status(
    state: &AppState,
    namespace: &str,
    name: &str,
    guest: GuestAgentInfo,
) -> ApiResult<VMToolsVmStatus> {
    let client = state.kube.as_ref();
    let vm = if let Some(c) = client {
        fetch_vm(c, namespace, name).await
    } else {
        None
    };
    let is_win = vm_is_windows(vm.as_ref(), None);
    let os_family = if is_win { "windows" } else { "linux" }.to_string();
    let installed_label = vm
        .as_ref()
        .and_then(|v| v.pointer("/metadata/labels/zeus.zyvor.dev/guest-tools"))
        .and_then(|v| v.as_str());
    let version = vm
        .as_ref()
        .and_then(|v| v.pointer("/metadata/annotations/zeus.zyvor.dev/tools-version"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let installed = guest.agent_connected
        || installed_label == Some("connected")
        || installed_label == Some("pending");
    let recommended_method = if is_win {
        "virtio-win".into()
    } else if guest.agent_connected {
        "connected".into()
    } else if guest.vmi_running {
        "cloud-init".into()
    } else {
        "iso".into()
    };
    let message = if is_win {
        "Install QEMU Guest Agent from Zeus OS Guest Tools (virtio-win ISO).".into()
    } else if guest.agent_connected {
        format!(
            "Zeus VM Tools connected{}",
            guest
                .guest_agent_version
                .as_ref()
                .map(|v| format!(" — v{v}"))
                .unwrap_or_default()
        )
    } else if guest.vmi_running {
        "Install Zeus VM Tools via cloud-init, ISO attach, or restart the VM.".into()
    } else {
        "Start the VM or use offline GuestKit injection to bootstrap the agent.".into()
    };

    if guest.agent_connected {
        if let Some(c) = client {
            let snap = VMToolsCrSnapshot {
                installed: true,
                connected: true,
                os_name: guest.os_name.clone(),
                ip_address: guest
                    .interfaces
                    .as_ref()
                    .and_then(|ifs| {
                        ifs.iter()
                            .find_map(|i| i.get("ipAddress").and_then(|v| v.as_str()))
                    })
                    .map(String::from),
                qga_ready: true,
                zyvor_agent_ready: true,
            };
            sync_vm_tools_labels(
                c,
                namespace,
                name,
                "connected",
                version.as_deref().unwrap_or(VMTOOLS_VERSION),
                Some(snap),
            )
            .await;
        }
    }

    Ok(VMToolsVmStatus {
        namespace: namespace.into(),
        name: name.into(),
        product: "Zeus VM Tools".into(),
        installed,
        connected: guest.agent_connected,
        version,
        recommended_method,
        os_name: guest.os_name.clone(),
        os_family,
        ip_address: guest
            .interfaces
            .as_ref()
            .and_then(|ifs| {
                ifs.iter()
                    .find_map(|i| i.get("ipAddress").and_then(|v| v.as_str()))
            })
            .map(String::from),
        qga_ready: guest.agent_connected,
        zyvor_agent_ready: guest.agent_connected,
        snapshot_quiesce: guest.agent_connected && !is_win,
        message,
        guest_agent: guest,
    })
}

pub async fn sync_vm_tools_labels(
    client: &Client,
    namespace: &str,
    name: &str,
    tools_state: &str,
    version: &str,
    cr_snapshot: Option<VMToolsCrSnapshot>,
) {
    let ar = vm_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let patch = json!({
        "metadata": {
            "labels": {
                "zeus.zyvor.dev/guest-tools": tools_state,
            },
            "annotations": {
                "zeus.zyvor.dev/tools-version": version,
                "zeus.zyvor.dev/last-heartbeat": chrono::Utc::now().to_rfc3339(),
            }
        }
    });
    let _ = api
        .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await;
    if let Some(snap) = cr_snapshot {
        sync_vmguestagent_cr(client, namespace, name, version, &snap).await;
    }
}

fn vmguestagent_resource() -> ApiResource {
    ApiResource {
        group: "zeus.zyvor.dev".into(),
        version: "v1alpha1".into(),
        api_version: "zeus.zyvor.dev/v1alpha1".into(),
        kind: "VMGuestAgent".into(),
        plural: "vmguestagents".into(),
    }
}

async fn sync_vmguestagent_cr(
    client: &Client,
    namespace: &str,
    vm_name: &str,
    version: &str,
    snap: &VMToolsCrSnapshot,
) {
    let cr_name = format!("{vm_name}-vmtools");
    let ar = vmguestagent_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let spec_obj = json!({
        "apiVersion": "zeus.zyvor.dev/v1alpha1",
        "kind": "VMGuestAgent",
        "metadata": {
            "name": cr_name,
            "namespace": namespace,
            "labels": {
                "zeus.zyvor.dev/vm": vm_name,
            }
        },
        "spec": {
            "vmRef": { "namespace": namespace, "name": vm_name },
            "desiredVersion": version,
        }
    });
    if api.get(&cr_name).await.is_ok() {
        let _ = api
            .patch(
                &cr_name,
                &PatchParams::default(),
                &Patch::Merge(&json!({
                    "spec": spec_obj.get("spec").cloned().unwrap_or(Value::Null)
                })),
            )
            .await;
    } else if let Ok(obj) = serde_json::from_value::<kube::api::DynamicObject>(spec_obj) {
        let _ = api.create(&kube::api::PostParams::default(), &obj).await;
    }
    let status = json!({
        "status": {
            "installed": snap.installed,
            "connected": snap.connected,
            "version": version,
            "os": snap.os_name.clone().unwrap_or_default(),
            "ip": snap.ip_address.clone().unwrap_or_default(),
            "qgaReady": snap.qga_ready,
            "zyvorAgentReady": snap.zyvor_agent_ready,
            "lastHeartbeat": chrono::Utc::now().to_rfc3339(),
        }
    });
    let _ = api
        .patch_status(&cr_name, &PatchParams::default(), &Patch::Merge(&status))
        .await;
}

pub const DEFAULT_POLICY_NAME: &str = "cluster-default";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VMToolsPolicySpec {
    #[serde(default)]
    pub selector: Value,
    #[serde(default)]
    pub auto_install: bool,
    #[serde(default)]
    pub auto_upgrade: bool,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_reboot_policy")]
    pub reboot_policy: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
}

fn default_max_concurrent() -> u32 {
    3
}

fn default_channel() -> String {
    "stable".into()
}

fn default_reboot_policy() -> String {
    "if-needed".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsPolicyView {
    pub name: String,
    pub spec: VMToolsPolicySpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VMToolsReconcileResult {
    pub policy: String,
    pub scanned: usize,
    pub matched: usize,
    pub installed: usize,
    pub pending: usize,
    pub upgraded: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PutPolicyBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub spec: Option<VMToolsPolicySpec>,
}

pub async fn get_policy(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<VMToolsPolicyView>>> {
    let client = kube_client(&state)?;
    let view = fetch_policy(&client, DEFAULT_POLICY_NAME).await?;
    Ok(Json(ApiResponse::ok(view)))
}

pub async fn put_policy(
    State(state): State<AppState>,
    Json(body): Json<PutPolicyBody>,
) -> ApiResult<Json<ApiResponse<VMToolsPolicyView>>> {
    let client = kube_client(&state)?;
    let name = body
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_POLICY_NAME.into());
    let spec = body.spec.unwrap_or(VMToolsPolicySpec {
        selector: json!({}),
        auto_install: false,
        auto_upgrade: false,
        channel: default_channel(),
        reboot_policy: default_reboot_policy(),
        max_concurrent: default_max_concurrent(),
    });
    upsert_policy(&client, &name, &spec).await?;
    let view = fetch_policy(&client, &name).await?;
    Ok(Json(ApiResponse::ok(view)))
}

pub async fn reconcile_policy(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<VMToolsReconcileResult>>> {
    let client = kube_client(&state)?;
    let policy = fetch_policy(&client, DEFAULT_POLICY_NAME).await?;
    if !policy.spec.auto_install && !policy.spec.auto_upgrade {
        return Ok(Json(ApiResponse::ok(VMToolsReconcileResult {
            policy: policy.name,
            scanned: 0,
            matched: 0,
            installed: 0,
            pending: 0,
            upgraded: 0,
            skipped: 0,
            errors: vec!["autoInstall and autoUpgrade are disabled on cluster policy".into()],
        })));
    }

    let bundle = bundle_info_async(&state, &client).await;
    let target_version = bundle.version.clone();

    let vms = list_dynamic_all(&client, &vm_resource()).await?;
    let vmis = list_dynamic_all(&client, &vmi_resource()).await?;
    let mut vmi_map: HashMap<(String, String), Value> = HashMap::new();
    for vmi in vmis {
        let ns = vmi
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = vmi
            .pointer("/metadata/labels/kubevirt.io/vm")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                vmi.pointer("/metadata/name")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_default();
        vmi_map.insert((ns, name), vmi);
    }

    let mut result = VMToolsReconcileResult {
        policy: policy.name.clone(),
        scanned: 0,
        matched: 0,
        installed: 0,
        pending: 0,
        upgraded: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    let max_actions = policy.spec.max_concurrent.max(1);
    let mut actions_taken = 0u32;

    for vm in vms {
        let name = vm.pointer("/metadata/name").and_then(|v| v.as_str()).unwrap_or("");
        let namespace = vm
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() || namespace.is_empty() {
            continue;
        }
        result.scanned += 1;
        if !matches_policy_selector(&vm, &policy.spec.selector) {
            continue;
        }
        result.matched += 1;
        let vmi = vmi_map.get(&(namespace.to_string(), name.to_string()));
        if vm_is_windows(Some(&vm), vmi) {
            result.skipped += 1;
            continue;
        }

        let zyvor_ok = vmi
            .map(|v| zyvor_tools_connected(&vm, Some(v)))
            .unwrap_or(false);
        let outdated = vm_tools_version(&vm)
            .map(|v| version_outdated(&v, &target_version))
            .unwrap_or(true);

        if zyvor_ok && !outdated {
            result.skipped += 1;
            continue;
        }

        let needs_install = policy.spec.auto_install && !zyvor_ok;
        let needs_upgrade = policy.spec.auto_upgrade && zyvor_ok && outdated;
        if !needs_install && !needs_upgrade {
            result.skipped += 1;
            continue;
        }

        if actions_taken >= max_actions {
            result.skipped += 1;
            continue;
        }

        match reconcile_install_vm(ReconcileInstallRequest {
            client: client.clone(),
            namespace,
            name,
            vm: &vm,
            vmi,
            policy: &policy.spec,
            bundle: &bundle,
            is_upgrade: needs_upgrade,
        })
        .await
        {
            Ok(outcome) => {
                actions_taken += 1;
                match outcome {
                ReconcileOutcome::Installed => result.installed += 1,
                ReconcileOutcome::Pending => result.pending += 1,
                ReconcileOutcome::Upgraded => result.upgraded += 1,
                ReconcileOutcome::Skipped => result.skipped += 1,
                }
            }
            Err(e) => result.errors.push(format!("{namespace}/{name}: {}", e.message)),
        }
    }

    Ok(Json(ApiResponse::ok(result)))
}

fn vmtoolspolicy_resource() -> ApiResource {
    ApiResource {
        group: "zeus.zyvor.dev".into(),
        version: "v1alpha1".into(),
        api_version: "zeus.zyvor.dev/v1alpha1".into(),
        kind: "VMToolsPolicy".into(),
        plural: "vmtoolspolicies".into(),
    }
}

async fn fetch_policy(client: &Client, name: &str) -> ApiResult<VMToolsPolicyView> {
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), &vmtoolspolicy_resource());
    match api.get(name).await {
        Ok(obj) => {
            let val = serde_json::to_value(&obj)
                .map_err(|e| ApiError::internal(format!("serialize policy: {e}")))?;
            Ok(VMToolsPolicyView {
                name: name.into(),
                spec: parse_policy_spec(&val),
                status: val.get("status").cloned(),
            })
        }
        Err(_) => Ok(VMToolsPolicyView {
            name: name.into(),
            spec: VMToolsPolicySpec {
                selector: json!({}),
                auto_install: false,
                auto_upgrade: false,
                channel: default_channel(),
                reboot_policy: default_reboot_policy(),
                max_concurrent: default_max_concurrent(),
            },
            status: None,
        }),
    }
}

fn parse_policy_spec(val: &Value) -> VMToolsPolicySpec {
    let spec = val.get("spec").cloned().unwrap_or(json!({}));
    VMToolsPolicySpec {
        selector: spec.get("selector").cloned().unwrap_or(json!({})),
        auto_install: spec
            .get("autoInstall")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_upgrade: spec
            .get("autoUpgrade")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        channel: spec
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("stable")
            .into(),
        reboot_policy: spec
            .get("rebootPolicy")
            .and_then(|v| v.as_str())
            .unwrap_or("if-needed")
            .into(),
        max_concurrent: spec
            .get("maxConcurrent")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or_else(default_max_concurrent),
    }
}

async fn upsert_policy(client: &Client, name: &str, spec: &VMToolsPolicySpec) -> ApiResult<()> {
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), &vmtoolspolicy_resource());
    let body = json!({
        "apiVersion": "zeus.zyvor.dev/v1alpha1",
        "kind": "VMToolsPolicy",
        "metadata": { "name": name },
        "spec": {
            "selector": spec.selector,
            "autoInstall": spec.auto_install,
            "autoUpgrade": spec.auto_upgrade,
            "channel": spec.channel,
            "rebootPolicy": spec.reboot_policy,
            "maxConcurrent": spec.max_concurrent,
        }
    });
    if api.get(name).await.is_ok() {
        api.patch(
            name,
            &PatchParams::default(),
            &Patch::Merge(&json!({ "spec": body.get("spec").cloned().unwrap_or(Value::Null) })),
        )
        .await
        .map_err(|e| ApiError::internal(format!("patch VMToolsPolicy: {e}")))?;
    } else {
        let obj: kube::api::DynamicObject = serde_json::from_value(body)
            .map_err(|e| ApiError::internal(format!("build VMToolsPolicy: {e}")))?;
        api.create(&kube::api::PostParams::default(), &obj)
            .await
            .map_err(|e| ApiError::internal(format!("create VMToolsPolicy: {e}")))?;
    }
    Ok(())
}

fn matches_policy_selector(vm: &Value, selector: &Value) -> bool {
    let Some(match_labels) = selector.get("matchLabels").and_then(|v| v.as_object()) else {
        return true;
    };
    if match_labels.is_empty() {
        return true;
    }
    let labels = vm
        .pointer("/metadata/labels")
        .and_then(|v| v.as_object());
    match_labels.iter().all(|(k, want)| {
        labels
            .and_then(|l| l.get(k))
            .map(|got| got == want)
            .unwrap_or(false)
    })
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("VM Tools requires in-cluster Kubernetes access"))
}

fn agent_connected(vmi: &Value) -> bool {
    vmi.pointer("/status/conditions")
        .and_then(|c| c.as_array())
        .map(|conds| {
            conds.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("AgentConnected")
                    && c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileOutcome {
    Installed,
    Pending,
    Upgraded,
    Skipped,
}

fn vm_tools_version(vm: &Value) -> Option<String> {
    vm.pointer("/metadata/annotations/zeus.zyvor.dev/tools-version")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn vmi_phase_running(vmi: Option<&Value>) -> bool {
    vmi.and_then(|v| {
        v.pointer("/status/phase")
            .and_then(|p| p.as_str())
            .map(|p| p == "Running")
    })
    .unwrap_or(false)
}

fn reconcile_prefer_iso(policy: &VMToolsPolicySpec) -> bool {
    std::env::var("VMTOOLS_RECONCILE_ISO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || policy.channel.eq_ignore_ascii_case("iso")
}

struct ReconcileInstallRequest<'a> {
    client: Client,
    namespace: &'a str,
    name: &'a str,
    vm: &'a Value,
    vmi: Option<&'a Value>,
    policy: &'a VMToolsPolicySpec,
    bundle: &'a VMToolsBundleInfo,
    is_upgrade: bool,
}

async fn reconcile_install_vm(req: ReconcileInstallRequest<'_>) -> ApiResult<ReconcileOutcome> {
    let ReconcileInstallRequest {
        client,
        namespace,
        name,
        vm,
        vmi,
        policy,
        bundle,
        is_upgrade,
    } = req;
    let restart = policy.reboot_policy != "never";
    let running = vmi_phase_running(vmi);
    let agent_live = vmi.map(agent_connected).unwrap_or(false);

    let install = if running && agent_live {
        install_guest_agent(client.clone(), namespace, name, false, Some("auto")).await?
    } else if running && !agent_live && reconcile_prefer_iso(policy) {
        let iso_url = bundle
            .iso_url
            .clone()
            .ok_or_else(|| ApiError::bad_request("VM Tools ISO URL is not configured"))?;
        install_vmtools_iso(client.clone(), namespace, name, &iso_url, restart).await?
    } else {
        install_guest_agent(client.clone(), namespace, name, restart, Some("auto")).await?
    };

    if !install.success {
        return Ok(ReconcileOutcome::Skipped);
    }

    let fresh_vmi = fetch_vmi(&client, namespace, name).await;
    let fresh_vm = fetch_vm(&client, namespace, name)
        .await
        .or_else(|| Some(vm.clone()));
    let outcome = apply_install_labels(
        &client,
        namespace,
        name,
        fresh_vm.as_ref(),
        fresh_vmi.as_ref(),
        &bundle.version,
        &install,
    )
    .await;

    Ok(if is_upgrade && outcome == ReconcileOutcome::Installed {
        ReconcileOutcome::Upgraded
    } else {
        outcome
    })
}

async fn apply_install_labels(
    client: &Client,
    namespace: &str,
    name: &str,
    vm: Option<&Value>,
    vmi: Option<&Value>,
    version: &str,
    install: &GuestAgentInstallResult,
) -> ReconcileOutcome {
    let zyvor_ok = vm
        .map(|v| zyvor_tools_connected(v, vmi))
        .unwrap_or(false);

    if zyvor_ok {
        sync_vm_tools_labels(client, namespace, name, "connected", version, None).await;
        return ReconcileOutcome::Installed;
    }

    if install.pending || install.success {
        sync_vm_tools_labels(client, namespace, name, "pending", version, None).await;
        return ReconcileOutcome::Pending;
    }

    ReconcileOutcome::Skipped
}

fn version_outdated(installed: &str, target: &str) -> bool {
    if installed.trim().is_empty() || target.trim().is_empty() {
        return true;
    }
    if installed == target {
        return false;
    }
    fn parse_parts(v: &str) -> Vec<u64> {
        v.split(['.', '-'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    }
    let a = parse_parts(installed);
    let b = parse_parts(target);
    let n = a.len().max(b.len());
    for i in 0..n {
        let ai = a.get(i).copied().unwrap_or(0);
        let bi = b.get(i).copied().unwrap_or(0);
        if ai < bi {
            return true;
        }
        if ai > bi {
            return false;
        }
    }
    false
}
