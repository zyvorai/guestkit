// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline KubeVirt VM boot inspection via GuestKit assurance APIs (stopped VMs).

use axum::extract::{Path, State};
use axum::Json;
use guestkit::run_boot_inspect;
use kube::api::Api;
use kube::discovery::ApiResource;
use kube::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path as FsPath, PathBuf};

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::routes::kubevirt::{json_str, vm_resource, vmi_resource};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootInspectInfo {
    pub available: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub os_release: String,
    #[serde(default)]
    pub fstab_valid: bool,
    #[serde(default)]
    pub bootloader: String,
    #[serde(default)]
    pub cloud_init_present: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct BootInspectBody {
    pub namespace: Option<String>,
    pub vm: Option<String>,
    pub pvc: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

pub async fn get_boot_inspect(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<BootInspectInfo>>> {
    let info = boot_inspect_for_vm(&state, &namespace, &name, None).await?;
    Ok(Json(ApiResponse::ok(info)))
}

pub async fn post_boot_inspect_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    body: Option<Json<BootInspectBody>>,
) -> ApiResult<Json<ApiResponse<BootInspectInfo>>> {
    let pvc = body.as_ref().and_then(|b| b.pvc.clone());
    let info = boot_inspect_for_vm(&state, &namespace, &name, pvc).await?;
    Ok(Json(ApiResponse::ok(info)))
}

pub async fn post_boot_inspect(
    State(state): State<AppState>,
    Json(body): Json<BootInspectBody>,
) -> ApiResult<Json<ApiResponse<BootInspectInfo>>> {
    let namespace = body
        .namespace
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("namespace is required"))?;
    let vm = body
        .vm
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("vm is required"))?;
    let info = boot_inspect_for_vm(&state, &namespace, &vm, body.pvc.clone()).await?;
    Ok(Json(ApiResponse::ok(info)))
}

pub(crate) async fn boot_inspect_for_vm(
    state: &AppState,
    namespace: &str,
    vm_name: &str,
    pvc_override: Option<String>,
) -> ApiResult<BootInspectInfo> {
    let client = state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("KubeVirt boot inspect requires in-cluster Kubernetes access"))?;

    let vm = fetch_vm(&client, namespace, vm_name).await?;
    let vmi = fetch_vmi(&client, namespace, vm_name).await;
    let vmi_running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");

    let pvc = pvc_override
        .filter(|s| !s.is_empty())
        .or_else(|| root_pvc_from_vm(&vm));

    if vmi_running {
        return Ok(heuristic_from_vm(&vm, pvc.as_deref(), true));
    }

    let Some(pvc_name) = pvc else {
        return Ok(BootInspectInfo {
            available: false,
            source: "vm_spec".into(),
            message: "No root PVC found in VM spec — attach a persistentVolumeClaim volume for offline inspect"
                .into(),
            ..heuristic_from_vm(&vm, None, false)
        });
    };

    if let Some(disk_path) = resolve_disk_path(&client, namespace, &pvc_name).await {
        let disk_display = disk_path.display().to_string();
        match tokio::task::spawn_blocking(move || inspect_disk_offline(&disk_path)).await {
            Ok(Ok(info)) => return Ok(info),
            Ok(Err(e)) => {
                tracing::warn!("GuestKit boot inspect failed for {disk_display}: {e:#}");
                let mut fallback = heuristic_from_vm(&vm, Some(&pvc_name), false);
                fallback.available = false;
                fallback.source = "guestkit".into();
                fallback.message = format!(
                    "Disk resolved at {disk_display} but GuestKit boot inspect failed: {e}"
                );
                return Ok(fallback);
            }
            Err(e) => {
                tracing::warn!("boot inspect task failed: {e}");
            }
        }
    }

    let mut fallback = heuristic_from_vm(&vm, Some(&pvc_name), false);
    fallback.message = format!(
        "Root PVC {pvc_name} found but disk path could not be resolved — set KUBEVIRT_DISK_ROOT or mount host volume paths"
    );
    Ok(fallback)
}

async fn fetch_vm(client: &Client, namespace: &str, name: &str) -> ApiResult<Value> {
    let ar = vm_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let obj = api
        .get(name)
        .await
        .map_err(|_| ApiError::not_found(format!("VirtualMachine {namespace}/{name} not found")))?;
    serde_json::to_value(obj).map_err(|e| ApiError::internal(e.to_string()))
}

async fn fetch_vmi(client: &Client, namespace: &str, name: &str) -> Option<Value> {
    let ar = vmi_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    api.get(name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok())
}

pub(crate) fn root_pvc_from_vm(vm: &Value) -> Option<String> {
    let disks = vm
        .pointer("/spec/template/spec/domain/devices/disks")
        .and_then(|v| v.as_array())?;
    let boot_disk = disks
        .iter()
        .find(|d| d.get("bootOrder").and_then(|b| b.as_u64()) == Some(1))
        .or_else(|| disks.first())
        .and_then(|d| d.get("name"))
        .and_then(|n| n.as_str())?;

    let volumes = vm.pointer("/spec/template/spec/volumes")?.as_array()?;
    for vol in volumes {
        if vol.get("name").and_then(|n| n.as_str()) != Some(boot_disk) {
            continue;
        }
        if let Some(name) = vol
            .get("persistentVolumeClaim")
            .and_then(|p| p.get("claimName"))
            .and_then(|n| n.as_str())
        {
            return Some(name.to_string());
        }
        if let Some(name) = vol
            .get("dataVolume")
            .and_then(|d| d.get("name"))
            .and_then(|n| n.as_str())
        {
            return Some(name.to_string());
        }
    }

    for vol in volumes {
        if let Some(name) = vol
            .get("persistentVolumeClaim")
            .and_then(|p| p.get("claimName"))
            .and_then(|n| n.as_str())
        {
            return Some(name.to_string());
        }
        if let Some(name) = vol
            .get("dataVolume")
            .and_then(|d| d.get("name"))
            .and_then(|n| n.as_str())
        {
            return Some(name.to_string());
        }
    }
    None
}

pub(crate) async fn resolve_disk_path(client: &Client, namespace: &str, pvc_name: &str) -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("KUBEVIRT_BOOT_INSPECT_DISK") {
        let path = explicit
            .replace("{namespace}", namespace)
            .replace("{pvc}", pvc_name);
        if FsPath::new(&path).exists() {
            return Some(PathBuf::from(path));
        }
    }

    let pvc = fetch_namespaced_resource(client, pvc_resource(), namespace, pvc_name).await?;
    let pv_name = json_str(&pvc, &["spec", "volumeName"])
        .or_else(|| json_str(&pvc, &["status", "volumeName"]))?;

    let mut candidates = Vec::new();
    if let Some(root) = std::env::var("KUBEVIRT_DISK_ROOT").ok().filter(|s| !s.is_empty()) {
        candidates.push(PathBuf::from(format!("{root}/{namespace}/{pvc_name}")));
        candidates.push(PathBuf::from(format!("{root}/{namespace}/{pvc_name}.qcow2")));
        candidates.push(PathBuf::from(format!("{root}/{pvc_name}")));
        candidates.push(PathBuf::from(format!("{root}/{pvc_name}.qcow2")));
    }

    if let Some(pv) = fetch_cluster_resource(client, pv_resource(), &pv_name).await {
        if let Some(path) = pv
            .pointer("/spec/hostPath")
            .and_then(|h| h.get("path"))
            .and_then(|p| p.as_str())
        {
            candidates.push(PathBuf::from(path));
        }
        if let Some(path) = pv
            .pointer("/spec/local")
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
        {
            candidates.push(PathBuf::from(path));
        }
        if let Some(handle) = pv
            .pointer("/spec/csi")
            .and_then(|c| c.get("volumeHandle"))
            .and_then(|h| h.as_str())
        {
            candidates.push(PathBuf::from(format!("/dev/longhorn/{handle}")));
            if handle.starts_with("pvc-") {
                candidates.push(PathBuf::from(format!("/dev/longhorn/{pvc_name}")));
            }
        }
    }

    for path in candidates {
        if let Some(resolved) = resolve_existing_disk(&path) {
            return Some(resolved);
        }
    }

    find_longhorn_replica_image(pvc_name, pvc.get("metadata")?)
}

fn resolve_existing_disk(path: &FsPath) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if path.is_dir() {
        for name in ["disk.img", "disk.qcow2", "disk.raw"] {
            let candidate = path.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_longhorn_replica_image(pvc_name: &str, metadata: &Value) -> Option<PathBuf> {
    let replicas_root = std::env::var("LONGHORN_REPLICAS_ROOT")
        .unwrap_or_else(|_| "/var/lib/longhorn/replicas".into());
    let root = FsPath::new(&replicas_root);
    if !root.is_dir() {
        return None;
    }
    let uid = metadata
        .get("uid")
        .and_then(|u| u.as_str())
        .unwrap_or_default();
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir.file_name()?.to_string_lossy();
        if !name.contains(pvc_name) && (uid.is_empty() || !name.contains(uid)) {
            continue;
        }
        if let Ok(files) = std::fs::read_dir(&dir) {
            for file in files.flatten() {
                let path = file.path();
                if path.is_file() {
                    let fname = path.file_name()?.to_string_lossy();
                    if fname.starts_with("volume") && fname.ends_with(".img") {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn pvc_resource() -> ApiResource {
    ApiResource {
        group: String::new(),
        version: "v1".into(),
        api_version: "v1".into(),
        kind: "PersistentVolumeClaim".into(),
        plural: "persistentvolumeclaims".into(),
    }
}

fn pv_resource() -> ApiResource {
    ApiResource {
        group: String::new(),
        version: "v1".into(),
        api_version: "v1".into(),
        kind: "PersistentVolume".into(),
        plural: "persistentvolumes".into(),
    }
}

async fn fetch_namespaced_resource(
    client: &Client,
    ar: ApiResource,
    namespace: &str,
    name: &str,
) -> Option<Value> {
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    api.get(name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok())
}

async fn fetch_cluster_resource(client: &Client, ar: ApiResource, name: &str) -> Option<Value> {
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), &ar);
    api.get(name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok())
}

fn inspect_disk_offline(disk_path: &FsPath) -> anyhow::Result<BootInspectInfo> {
    let summary = run_boot_inspect(disk_path, "kubevirt", false)?;
    Ok(BootInspectInfo {
        available: true,
        source: "guestkit".into(),
        os_release: summary.os_release,
        fstab_valid: summary.fstab_valid,
        bootloader: summary.bootloader,
        cloud_init_present: summary.cloud_init_present,
        message: summary.message,
    })
}

fn heuristic_from_vm(vm: &Value, pvc: Option<&str>, vmi_running: bool) -> BootInspectInfo {
    let firmware = vm
        .pointer("/spec/template/spec/domain/firmware/bootloader")
        .and_then(|v| v.as_str())
        .unwrap_or("bios");
    let cloud_init = vm
        .pointer("/spec/template/spec/volumes")
        .and_then(|v| v.as_array())
        .map(|vols| {
            vols.iter().any(|vol| {
                vol.get("cloudInitNoCloud").is_some() || vol.get("cloudInitConfigDrive").is_some()
            })
        })
        .unwrap_or(false);
    let os_release = vm
        .pointer("/metadata/labels/kubevirt.io~1os")
        .or_else(|| vm.pointer("/metadata/labels/kubevirt.io/os"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let message = if vmi_running {
        "VM is running — live guest agent probes are authoritative; stop VM for GuestKit offline boot inspect"
            .into()
    } else if let Some(pvc) = pvc {
        format!(
            "Offline hints from VM spec (root PVC {pvc}) — disk path resolution or GuestKit boot inspect pending"
        )
    } else {
        "Offline boot hints from VM spec".into()
    };

    BootInspectInfo {
        available: !vmi_running,
        source: if vmi_running {
            "vm_spec_heuristic".into()
        } else {
            "vm_spec".into()
        },
        os_release,
        fstab_valid: true,
        bootloader: firmware.to_string(),
        cloud_init_present: cloud_init,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn root_pvc_prefers_boot_order_disk() {
        let vm = json!({
            "spec": {
                "template": {
                    "spec": {
                        "domain": {
                            "devices": {
                                "disks": [
                                    { "name": "cloudinit", "bootOrder": 2 },
                                    { "name": "rootdisk", "bootOrder": 1 }
                                ]
                            }
                        },
                        "volumes": [
                            { "name": "rootdisk", "persistentVolumeClaim": { "claimName": "root-pvc" } },
                            { "name": "cloudinit", "cloudInitNoCloud": {} }
                        ]
                    }
                }
            }
        });
        assert_eq!(root_pvc_from_vm(&vm).as_deref(), Some("root-pvc"));
    }
}
