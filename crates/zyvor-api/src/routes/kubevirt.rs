// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live KubeVirt VM discovery (in-cluster Kubernetes API).

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use kube::api::{Api, ListParams};
use kube::discovery::ApiResource;
use kube::{Client};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct KubeVirtVmSummary {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub phase: Option<String>,
    pub node: Option<String>,
    pub ip_address: Option<String>,
    pub guest_agent_connected: Option<bool>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub hostname: Option<String>,
    pub is_windows: bool,
    pub root_pvc: Option<String>,
    pub age: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_tools: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_version: Option<String>,
    /// True when zyvor-guest-agent is live (KubeVirt guestAgentVersion or connected label).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_connected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_health: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_units: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packetwolf_correlation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packetwolf_fleet: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct VmListQuery {
    pub namespace: Option<String>,
    pub search: Option<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestAgentInfo {
    pub health: String,
    pub agent_connected: bool,
    pub vmi_running: bool,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub hostname: Option<String>,
    pub is_windows: bool,
    pub guest_agent_version: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<Vec<Value>>,
}

pub fn vm_resource() -> ApiResource {
    ApiResource {
        group: "kubevirt.io".into(),
        version: "v1".into(),
        api_version: "kubevirt.io/v1".into(),
        kind: "VirtualMachine".into(),
        plural: "virtualmachines".into(),
    }
}

pub fn vmi_resource() -> ApiResource {
    ApiResource {
        group: "kubevirt.io".into(),
        version: "v1".into(),
        api_version: "kubevirt.io/v1".into(),
        kind: "VirtualMachineInstance".into(),
        plural: "virtualmachineinstances".into(),
    }
}

pub fn cdi_dv_resource() -> ApiResource {
    ApiResource {
        group: "cdi.kubevirt.io".into(),
        version: "v1beta1".into(),
        api_version: "cdi.kubevirt.io/v1beta1".into(),
        kind: "DataVolume".into(),
        plural: "datavolumes".into(),
    }
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("KubeVirt discovery requires in-cluster Kubernetes access"))
}

pub(crate) fn json_str(obj: &Value, path: &[&str]) -> Option<String> {
    let mut cur = obj;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn format_age(ts: Option<&DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "—".into();
    };
    let secs = (Utc::now() - *ts).num_seconds().max(0);
    if secs < 3600 {
        return format!("{}m", secs / 60);
    }
    if secs < 86_400 {
        return format!("{}h", secs / 3600);
    }
    format!("{}d", secs / 86_400)
}

fn vm_printable_status(vm: &Value) -> String {
    json_str(vm, &["status", "printableStatus"])
        .or_else(|| json_str(vm, &["status", "ready"]))
        .unwrap_or_else(|| "Unknown".into())
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

fn extract_ip(vmi: &Value) -> Option<String> {
    if let Some(ifaces) = vmi.pointer("/status/interfaces").and_then(|v| v.as_array()) {
        for iface in ifaces {
            if let Some(ip) = iface.get("ipAddress").and_then(|v| v.as_str()) {
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    vmi.pointer("/status/guestOSInfo")
        .and_then(|g| g.get("hostname"))
        .and_then(|v| v.as_str())
        .filter(|s| s.contains('.'))
        .map(|s| s.to_string())
}

fn guest_os(vmi: &Value) -> (Option<String>, Option<String>, Option<String>) {
    let guest = vmi.pointer("/status/guestOSInfo");
    let os_name = guest
        .and_then(|g| g.get("name").or_else(|| g.get("prettyName")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let os_version = guest
        .and_then(|g| g.get("version").or_else(|| g.get("id")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let hostname = guest
        .and_then(|g| g.get("hostname"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (os_name, os_version, hostname)
}

pub(crate) fn build_guest_info(vm: Option<&Value>, vmi: Option<&Value>, vmi_running: bool) -> GuestAgentInfo {
    let connected = vmi.map(agent_connected).unwrap_or(false);
    let (os_name, os_version, hostname) = vmi.map(guest_os).unwrap_or((None, None, None));
    let is_windows = crate::kubevirt_guest_agent::vm_is_windows(vm, vmi);
    let health = if !vmi_running {
        "absent"
    } else if connected {
        "connected"
    } else {
        "degraded"
    };
    let message = if !vmi_running {
        "VM is not running — start it in Zeus OS or kubectl to reach the guest agent.".into()
    } else if connected {
        if is_windows {
            "Guest agent connected (Windows). Live inspect and quiesced snapshots available.".into()
        } else {
            "Guest agent connected — live inspect and exec are available.".into()
        }
    } else if is_windows {
        "Guest agent not connected — install QEMU Guest Agent from virtio-win guest tools via Zeus OS Guest Tools, then restart.".into()
    } else {
        "Guest agent not connected — use Install agent below or Zeus OS Guest Tools, then restart the VM.".into()
    };
    GuestAgentInfo {
        health: health.into(),
        agent_connected: connected,
        vmi_running,
        os_name: os_name.clone(),
        os_version,
        hostname,
        is_windows,
        guest_agent_version: vmi
            .and_then(|v| json_str(v, &["status", "guestAgentVersion"])),
        message,
        interfaces: vmi
            .and_then(|v| v.pointer("/status/interfaces"))
            .and_then(|v| v.as_array())
            .cloned(),
    }
}

pub(crate) async fn list_dynamic_all(client: &Client, ar: &ApiResource) -> ApiResult<Vec<Value>> {
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), ar);
    let listed = api
        .list(&ListParams::default())
        .await
        .map_err(|e| ApiError::internal(format!("kube list {}: {e}", ar.plural)))?;
    Ok(listed
        .items
        .into_iter()
        .map(|obj| serde_json::to_value(obj).unwrap_or(Value::Null))
        .filter(|v| !v.is_null())
        .collect())
}

pub(crate) async fn fetch_vm(client: &Client, namespace: &str, name: &str) -> Option<Value> {
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vm_resource());
    api.get(name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok())
}

pub(crate) async fn fetch_vmi(client: &Client, namespace: &str, name: &str) -> Option<Value> {
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vmi_resource());
    api.get(name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok())
}

fn vm_matches_filters(vm: &KubeVirtVmSummary, query: &VmListQuery) -> bool {
    if let Some(ns) = query.namespace.as_deref() {
        if !ns.is_empty() && vm.namespace != ns {
            return false;
        }
    }
    if let Some(phase) = query.phase.as_deref() {
        if !phase.is_empty() {
            let p = vm.phase.as_deref().unwrap_or("").to_lowercase();
            let s = vm.status.to_lowercase();
            let want = phase.to_lowercase();
            let matches = p.contains(&want)
                || s.contains(&want)
                || (want == "stopped" && (p.is_empty() || p == "succeeded" || s.contains("stop")));
            if !matches {
                return false;
            }
        }
    }
    if let Some(search) = query.search.as_deref() {
        if !search.is_empty() {
            let needle = search.to_lowercase();
            let hay = format!("{}/{} {} {}", vm.namespace, vm.name, vm.status, vm.ip_address.as_deref().unwrap_or("")).to_lowercase();
            if !hay.contains(&needle) {
                return false;
            }
        }
    }
    true
}

pub async fn list_kubevirt_namespaces(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<Vec<String>>>> {
    let client = kube_client(&state)?;
    let vms = list_dynamic_all(&client, &vm_resource()).await?;
    let mut namespaces: Vec<String> = vms
        .iter()
        .filter_map(|vm| json_str(vm, &["metadata", "namespace"]))
        .collect();
    namespaces.sort();
    namespaces.dedup();
    Ok(Json(ApiResponse::ok(namespaces)))
}

pub async fn list_kubevirt_vms(
    State(state): State<AppState>,
    Query(query): Query<VmListQuery>,
) -> ApiResult<Json<ApiResponse<Vec<KubeVirtVmSummary>>>> {
    let client = kube_client(&state)?;
    let vms = list_dynamic_all(&client, &vm_resource()).await?;
    let vmis = list_dynamic_all(&client, &vmi_resource()).await?;

    let mut vmi_map: HashMap<(String, String), Value> = HashMap::new();
    for vmi in vmis {
        let ns = json_str(&vmi, &["metadata", "namespace"]).unwrap_or_default();
        let vm_name = vmi
            .pointer("/metadata/labels")
            .and_then(|l| l.get("kubevirt.io/vm"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| json_str(&vmi, &["metadata", "name"]).unwrap_or_default());
        vmi_map.insert((ns, vm_name), vmi);
    }

    let mut out = Vec::new();
    for vm in vms {
        let name = json_str(&vm, &["metadata", "name"]).unwrap_or_default();
        let namespace = json_str(&vm, &["metadata", "namespace"]).unwrap_or_default();
        if name.is_empty() || namespace.is_empty() {
            continue;
        }
        let created = vm
            .pointer("/metadata/creationTimestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let vmi = vmi_map.get(&(namespace.clone(), name.clone()));
        let phase = vmi.and_then(|v| json_str(v, &["status", "phase"]));
        let (os_name, os_version, hostname) = vmi.map(guest_os).unwrap_or((None, None, None));
        let is_windows = crate::kubevirt_guest_agent::vm_is_windows(Some(&vm), vmi);
        let root_pvc = crate::kubevirt_boot_inspect::root_pvc_from_vm(&vm);
        let guest_tools_label = json_str(&vm, &["metadata", "labels", "zeus.zyvor.dev/guest-tools"]);
        let tools_version = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/tools-version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let tools_connected = crate::kubevirt_guest_agent::zyvor_tools_connected(&vm, vmi);
        let guest_tools = if tools_connected {
            Some("connected".into())
        } else {
            guest_tools_label
        };
        let guest_health = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/guest-health")
            .and_then(|v| v.as_str())
            .map(String::from);
        let health_score = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/health-score")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let failed_units = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/failed-units")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let packetwolf_correlation = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/packetwolf-correlation")
            .and_then(|v| v.as_str())
            .map(String::from);
        let packetwolf_fleet = vm
            .pointer("/metadata/annotations/zeus.zyvor.dev/packetwolf-fleet-correlation")
            .and_then(|v| v.as_str())
            .map(String::from);
        out.push(KubeVirtVmSummary {
            name: name.clone(),
            namespace: namespace.clone(),
            status: vm_printable_status(&vm),
            phase,
            node: vmi.and_then(|v| json_str(v, &["status", "nodeName"])),
            ip_address: vmi.and_then(extract_ip),
            guest_agent_connected: vmi.map(agent_connected),
            os_name,
            os_version,
            hostname,
            is_windows,
            root_pvc,
            age: format_age(created.as_ref()),
            guest_tools,
            tools_version,
            tools_connected: Some(tools_connected),
            guest_health,
            health_score,
            failed_units,
            packetwolf_correlation,
            packetwolf_fleet,
        });
    }

    out.retain(|vm| vm_matches_filters(vm, &query));

    out.sort_by(|a, b| a.namespace.cmp(&b.namespace).then(a.name.cmp(&b.name)));
    Ok(Json(ApiResponse::ok(out)))
}

pub async fn get_guest_agent_info(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<GuestAgentInfo>>> {
    let client = kube_client(&state)?;
    let vm_ar = vm_resource();
    let vm_api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), &namespace, &vm_ar);
    let vm = vm_api
        .get(&name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok());

    let ar = vmi_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client, &namespace, &ar);
    let vmi = api
        .get(&name)
        .await
        .ok()
        .and_then(|obj| serde_json::to_value(obj).ok());
    let vmi_running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");
    let info = build_guest_info(vm.as_ref(), vmi.as_ref(), vmi_running);
    Ok(Json(ApiResponse::ok(info)))
}

#[derive(Debug, Deserialize, Default)]
pub struct GuestAgentInstallQuery {
    #[serde(default)]
    pub restart: Option<bool>,
    #[serde(default)]
    pub method: Option<String>,
}

pub async fn install_guest_agent(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<GuestAgentInstallQuery>,
) -> ApiResult<Json<ApiResponse<crate::kubevirt_guest_agent::GuestAgentInstallResult>>> {
    let client = kube_client(&state)?;
    let restart = query.restart.unwrap_or(true);
    let method = query.method.as_deref();
    let result =
        crate::kubevirt_guest_agent::install_guest_agent(client, &namespace, &name, restart, method)
            .await?;
    Ok(Json(ApiResponse::ok(result)))
}
