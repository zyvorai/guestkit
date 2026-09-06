// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestKit agent bootstrap for live KubeVirt VMs (cloud-init + virtio channel).

use base64::Engine;
use kube::api::{Api, Patch, PatchParams, PostParams};
use kube::discovery::ApiResource;
use kube::Client;
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};

const ZYVOR_AGENT_UNIT: &str = r#"[Unit]
Description=Zyvor VM Tools Guest Agent
After=network-online.target
Wants=network-online.target
ConditionPathExists=/dev/virtio-ports/org.qemu.guest_agent.0

[Service]
ExecStart=/usr/local/bin/zyvor-guest-agent
Restart=always
RestartSec=5
User=root
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestAgentInstallResult {
    pub success: bool,
    pub method: String,
    pub is_windows: bool,
    pub cloud_init_updated: bool,
    pub channel_updated: bool,
    pub vm_restarted: bool,
    pub needs_restart: bool,
    /// Install action started but zyvor-guest-agent is not connected yet.
    pub pending: bool,
    pub message: String,
    pub next_steps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_script: Option<String>,
}

pub fn guest_os_label(vm: &Value) -> Option<String> {
    let labels = vm.pointer("/metadata/labels")?;
    for key in [
        "hyper2kvm.io/guest-os",
        "v9s.io/guest-os",
        "zeus-os.io/guest-os",
    ] {
        if let Some(v) = labels.get(key).and_then(|v| v.as_str()) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn vm_is_windows(vm: Option<&Value>, vmi: Option<&Value>) -> bool {
    if let Some(vmi) = vmi {
        if let Some(guest) = vmi.pointer("/status/guestOSInfo") {
            for key in ["name", "prettyName"] {
                if let Some(s) = guest.get(key).and_then(|v| v.as_str()) {
                    if is_windows_os_name(s) {
                        return true;
                    }
                }
            }
        }
    }
    if vm
        .and_then(guest_os_label)
        .map(|s| s.eq_ignore_ascii_case("windows"))
        .unwrap_or(false)
    {
        return true;
    }
    vm.and_then(|vm| {
        vm.pointer("/spec/template/spec/volumes")
            .and_then(|v| v.as_array())
            .map(|vols| vols.iter().any(|v| v.get("sysprep").is_some()))
    })
    .unwrap_or(false)
}

fn json_str(obj: &Value, path: &[&str]) -> Option<String> {
    let mut cur = obj;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn is_windows_os_name(os_name: &str) -> bool {
    let lower = os_name.to_lowercase();
    lower.contains("windows") || lower.starts_with("win")
}

pub fn resolve_guestkit_binary_url() -> String {
    for var in ["GUESTKIT_BINARY_URL", "ZEUS_OS_GUESTKIT_BINARY_URL"] {
        if let Ok(url) = std::env::var(var) {
            let t = url.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    if let Ok(base) = std::env::var("VMTOOLS_BUNDLE_URL") {
        let t = base.trim().trim_end_matches('/');
        if !t.is_empty() {
            return format!("{t}/linux/zyvor-guest-agent");
        }
    }
    if let Ok(url) = std::env::var("ZEUS_OS_PUBLIC_URL") {
        let t = url.trim().trim_end_matches('/');
        if !t.is_empty() {
            return format!("{t}/api/v1/engines/guestkit/binary");
        }
    }
    "http://127.0.0.1:30050/api/v1/engines/guestkit/binary".into()
}

pub fn windows_install_script(agent_url: &str) -> String {
    let url = agent_url.replace('\'', "''");
    format!(
        r#"$ErrorActionPreference='Stop'
$InstallDir="$env:ProgramFiles\Zyvor\VM Tools"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$agentPath=Join-Path $InstallDir 'zyvor-guest-agent.exe'
Invoke-WebRequest -Uri '{url}' -OutFile $agentPath -UseBasicParsing
$svcName='ZyvorGuestAgent'
if (Get-Service -Name $svcName -ErrorAction SilentlyContinue) {{ Stop-Service -Name $svcName -Force -ErrorAction SilentlyContinue; sc.exe delete $svcName | Out-Null; Start-Sleep -Seconds 2 }}
New-Service -Name $svcName -BinaryPathName "`"$agentPath`" --service" -DisplayName 'Zeus VM Tools Guest Agent' -StartupType Automatic | Out-Null
Start-Service -Name $svcName
Write-Output 'ZyvorGuestAgent installed'"#
    )
}

pub fn resolve_windows_agent_url(bundle_agent_url: Option<&str>) -> String {
    if let Some(url) = bundle_agent_url.filter(|u| !u.is_empty()) {
        return url.to_string();
    }
    if let Ok(url) = std::env::var("VMTOOLS_WINDOWS_AGENT_URL") {
        let t = url.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let base = std::env::var("VMTOOLS_BUNDLE_URL")
        .unwrap_or_else(|_| "http://minio:9000/vmtools".into());
    format!("{}/windows/zyvor-guest-agent.exe", base.trim_end_matches('/'))
}

pub fn qga_bootstrap_script(binary_url: &str) -> String {
    format!(
        "mkdir -p /usr/bin /etc/systemd/system && \
curl -fsSL '{binary_url}' -o /usr/local/bin/zyvor-guest-agent && \
chmod +x /usr/local/bin/zyvor-guest-agent && \
cat > /etc/systemd/system/zyvor-guest-agent.service <<'UNIT'\n{ZYVOR_AGENT_UNIT}UNIT\n\
systemctl daemon-reload && systemctl enable --now zyvor-guest-agent"
    )
}

pub fn zyvor_tools_connected(vm: &Value, vmi: Option<&Value>) -> bool {
    if vm
        .pointer("/metadata/labels/zeus.zyvor.dev/guest-tools")
        .and_then(|v| v.as_str())
        == Some("connected")
    {
        return true;
    }
    vmi
        .and_then(|v| json_str(v, &["status", "guestAgentVersion"]))
        .map(|v| {
            let lower = v.to_lowercase();
            lower.contains("guestkit") || lower.contains("zyvor")
        })
        .unwrap_or(false)
}

fn guestkit_already_in_cloud_init(user_data: &str) -> bool {
    let lower = user_data.to_lowercase();
    lower.contains("zyvor-guest-agent") || lower.contains("/usr/local/bin/zyvor-guest-agent")
}

fn legacy_guestkit_bootstrap(user_data: &str) -> bool {
    let lower = user_data.to_lowercase();
    lower.contains("guestkit-agent.service")
        || lower.contains("/usr/local/bin/guestkit")
        || lower.contains("guestkit agent --channel")
}

fn strip_legacy_guestkit_blocks(user_data: &str) -> String {
    let lines: Vec<&str> = user_data.lines().collect();
    let mut out = Vec::new();
    let mut skip = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.contains("guestkit-agent.service")
            || line.contains("/usr/local/bin/guestkit")
            || line.contains("guestkit agent --channel")
        {
            skip = true;
        }
        if skip && line.trim().is_empty() {
            skip = false;
            i += 1;
            continue;
        }
        if !skip {
            out.push(line);
        }
        i += 1;
    }
    out.join("\n")
}

fn append_agent_unit_yaml(out: &mut String) {
    out.push_str("write_files:\n");
    out.push_str("  - path: /etc/systemd/system/zyvor-guest-agent.service\n");
    out.push_str("    permissions: '0644'\n");
    out.push_str("    content: |\n");
    for line in ZYVOR_AGENT_UNIT.lines() {
        out.push_str("      ");
        out.push_str(line);
        out.push('\n');
    }
}

fn append_agent_runcmd_yaml(out: &mut String, binary_url: &str) {
    let url = binary_url.replace('\'', "'\"'\"'");
    out.push_str("runcmd:\n");
    out.push_str(&format!(
        "  - curl -fsSL -o /usr/local/bin/zyvor-guest-agent '{url}' || true\n"
    ));
    out.push_str("  - chmod 755 /usr/local/bin/zyvor-guest-agent\n");
    out.push_str("  - systemctl daemon-reload\n");
    out.push_str("  - systemctl enable --now zyvor-guest-agent\n");
}

fn linux_cloud_init_snippet(binary_url: &str) -> String {
    let mut out = String::from("#cloud-config\n");
    out.push_str("packages:\n  - qemu-guest-agent\n");
    append_agent_unit_yaml(&mut out);
    append_agent_runcmd_yaml(&mut out, binary_url);
    out.push_str("  - systemctl enable --now qemu-guest-agent || true\n");
    out
}

fn merge_linux_guest_agent_cloud_init(user_data: &str, binary_url: &str) -> (String, bool) {
    let trimmed = user_data.trim();
    if trimmed.is_empty() {
        return (linux_cloud_init_snippet(binary_url), true);
    }
    let base = if legacy_guestkit_bootstrap(trimmed) {
        strip_legacy_guestkit_blocks(trimmed)
    } else {
        trimmed.to_string()
    };
    if guestkit_already_in_cloud_init(&base) {
        let changed = base != trimmed;
        return (base, changed);
    }

    let mut out = base;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.contains("packages:") {
        out.push_str("packages:\n  - qemu-guest-agent\n");
    } else if !out.contains("qemu-guest-agent") {
        out.push_str("  - qemu-guest-agent\n");
    }
    if !out.contains("write_files:")
        || (!out.contains("zyvor-guest-agent.service") && !out.contains("guestkit-agent.service"))
    {
        append_agent_unit_yaml(&mut out);
    }
    if !out.contains("runcmd:") {
        append_agent_runcmd_yaml(&mut out, binary_url);
    } else if !out.contains("/usr/local/bin/zyvor-guest-agent") && !out.contains("/usr/bin/zyvor-guest-agent") {
        let url = binary_url.replace('\'', "'\"'\"'");
        out.push_str(&format!(
            "  - curl -fsSL -o /usr/local/bin/zyvor-guest-agent '{url}' || true\n"
        ));
        out.push_str("  - chmod 755 /usr/local/bin/zyvor-guest-agent\n");
        out.push_str("  - systemctl daemon-reload\n");
        out.push_str("  - systemctl enable --now zyvor-guest-agent\n");
    }
    if !out.contains("qemu-guest-agent") && out.contains("runcmd:") {
        out.push_str("  - systemctl enable --now qemu-guest-agent || true\n");
    }
    (out, true)
}

fn guest_agent_channel_present(vm: &Value) -> bool {
    if vm
        .pointer("/spec/template/spec/domain/devices/channels")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter().any(|ch| {
                ch.get("name").and_then(|n| n.as_str()) == Some("org.qemu.guest_agent.0")
                    || ch.pointer("/target/name").and_then(|n| n.as_str())
                        == Some("org.qemu.guest_agent.0")
            })
        })
        .unwrap_or(false)
    {
        return true;
    }

    vm.pointer("/spec/template/spec/domain/devices/disks")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter().any(|disk| {
                disk.get("serial").and_then(|s| s.as_str()) == Some("org.qemu.guest_agent.0")
                    || disk.get("name").and_then(|n| n.as_str()) == Some("guestagent")
            })
        })
        .unwrap_or(false)
}

fn merge_guest_agent_channel_into_vm(vm: &mut Value) -> bool {
    if vm_is_windows(Some(vm), None) || guest_agent_channel_present(vm) {
        return false;
    }

    // KubeVirt 1.8+ exposes the QGA virtio-serial port via a disk serial, not devices.channels.
    {
        let Some(volumes) = vm
            .pointer_mut("/spec/template/spec/volumes")
            .and_then(|v| v.as_array_mut())
        else {
            return false;
        };
        let has_volume = volumes
            .iter()
            .any(|v| v.get("name").and_then(|n| n.as_str()) == Some("guestagent"));
        if !has_volume {
            volumes.push(json!({
                "name": "guestagent",
                "emptyDisk": { "capacity": "1Gi" }
            }));
        }
    }

    let Some(devices) = vm
        .pointer_mut("/spec/template/spec/domain/devices")
        .and_then(|d| d.as_object_mut())
    else {
        return false;
    };
    let disks = devices
        .entry("disks".to_string())
        .or_insert_with(|| Value::Array(vec![]));
    let Some(arr) = disks.as_array_mut() else {
        return false;
    };
    let has_disk = arr
        .iter()
        .any(|d| d.get("name").and_then(|n| n.as_str()) == Some("guestagent"));
    if !has_disk {
        arr.push(json!({
            "name": "guestagent",
            "disk": { "bus": "virtio" },
            "serial": "org.qemu.guest_agent.0"
        }));
    }
    true
}

fn merge_guest_agent_cloud_init_in_vm(vm: &mut Value, binary_url: &str) -> bool {
    if vm_is_windows(Some(vm), None) {
        return false;
    }

    {
        let Some(volumes) = vm
            .pointer_mut("/spec/template/spec/volumes")
            .and_then(|v| v.as_array_mut())
        else {
            return false;
        };

        for vol in volumes.iter_mut() {
            for key in ["cloudInitNoCloud", "cloudInitConfigDrive"] {
                let Some(ci) = vol.get_mut(key) else {
                    continue;
                };
                let user_data = read_cloud_init_user_data(ci);
                let Some(ud) = user_data else {
                    continue;
                };
                let (merged, changed) = merge_linux_guest_agent_cloud_init(&ud, binary_url);
                if changed {
                    write_cloud_init_user_data(ci, &merged);
                }
                return changed;
            }
        }
    }

    let hostname = vm
        .pointer("/metadata/name")
        .and_then(|n| n.as_str())
        .unwrap_or("vm")
        .to_string();
    let seed = format!("#cloud-config\nhostname: {hostname}\nssh_pwauth: true\n");
    let (user_data, _) = merge_linux_guest_agent_cloud_init(&seed, binary_url);

    {
        let Some(volumes) = vm
            .pointer_mut("/spec/template/spec/volumes")
            .and_then(|v| v.as_array_mut())
        else {
            return false;
        };
        volumes.push(json!({
            "name": "cloudinitdisk",
            "cloudInitNoCloud": { "userData": user_data }
        }));
    }

    if let Some(disks) = vm
        .pointer_mut("/spec/template/spec/domain/devices/disks")
        .and_then(|d| d.as_array_mut())
    {
        let has_disk = disks
            .iter()
            .any(|d| d.get("name").and_then(|n| n.as_str()) == Some("cloudinitdisk"));
        if !has_disk {
            disks.push(json!({
                "name": "cloudinitdisk",
                "disk": { "bus": "virtio" }
            }));
        }
    }
    true
}

fn read_cloud_init_user_data(ci: &Value) -> Option<String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    ci.get("userDataBase64")
        .and_then(|v| v.as_str())
        .and_then(|encoded| b64.decode(encoded).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .or_else(|| ci.get("userData").and_then(|v| v.as_str()).map(String::from))
}

fn write_cloud_init_user_data(ci: &mut Value, user_data: &str) {
    if let Some(obj) = ci.as_object_mut() {
        obj.remove("userDataBase64");
        obj.insert("userData".into(), Value::String(user_data.to_string()));
    }
}

fn vm_resource() -> ApiResource {
    ApiResource {
        group: "kubevirt.io".into(),
        version: "v1".into(),
        api_version: "kubevirt.io/v1".into(),
        kind: "VirtualMachine".into(),
        plural: "virtualmachines".into(),
    }
}

fn vmi_resource() -> ApiResource {
    ApiResource {
        group: "kubevirt.io".into(),
        version: "v1".into(),
        api_version: "kubevirt.io/v1".into(),
        kind: "VirtualMachineInstance".into(),
        plural: "virtualmachineinstances".into(),
    }
}

pub async fn install_guest_agent(
    client: Client,
    namespace: &str,
    name: &str,
    restart: bool,
    method: Option<&str>,
) -> ApiResult<GuestAgentInstallResult> {
    let install_method = method.unwrap_or("auto").to_lowercase();
    let vm_ar = vm_resource();
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vm_ar);
    let obj = api
        .get(name)
        .await
        .map_err(|e| ApiError::internal(format!("get VM {namespace}/{name}: {e}")))?;
    let mut vm = serde_json::to_value(&obj)
        .map_err(|e| ApiError::internal(format!("serialize VM: {e}")))?;

    let vmi_ar = vmi_resource();
    let vmi_api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vmi_ar);
    let vmi = vmi_api.get(name).await.ok().and_then(|o| serde_json::to_value(o).ok());
    let vmi_running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");

    let is_windows = vm_is_windows(Some(&vm), vmi.as_ref());
    let agent_live = vmi
        .as_ref()
        .map(|v| {
            v.pointer("/status/conditions")
                .and_then(|c| c.as_array())
                .map(|conds| {
                    conds.iter().any(|c| {
                        c.get("type").and_then(|t| t.as_str()) == Some("AgentConnected")
                            && c.get("status").and_then(|s| s.as_str()) == Some("True")
                    })
                })
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if is_windows {
        let agent_url = resolve_windows_agent_url(None);
        let ps_script = windows_install_script(&agent_url);
        if vmi_running && agent_live && install_method != "cloud-init" {
            match crate::kubevirt_qga::qga_exec_powershell(
                &client,
                namespace,
                name,
                &ps_script,
                180,
            )
            .await
            {
                Ok(exec) if exec.exit_code == 0 => {
                    return Ok(GuestAgentInstallResult {
                        success: true,
                        method: "qga-exec".into(),
                        is_windows: true,
                        cloud_init_updated: false,
                        channel_updated: false,
                        vm_restarted: false,
                        needs_restart: false,
                        pending: false,
                        message: "Installed Zeus VM Tools via QGA guest-exec (PowerShell).".into(),
                        next_steps: vec![
                            "Refresh guest info — ZyvorGuestAgent service should be running.".into(),
                            "virtio-win remains available for QEMU Guest Agent parity.".into(),
                        ],
                        bootstrap_script: None,
                    });
                }
                Ok(exec) => {
                    return Ok(GuestAgentInstallResult {
                        success: false,
                        method: "qga-exec".into(),
                        is_windows: true,
                        cloud_init_updated: false,
                        channel_updated: false,
                        vm_restarted: false,
                        needs_restart: false,
                        pending: true,
                        message: format!(
                            "QGA install failed (exit {}): {}",
                            exec.exit_code,
                            exec.stderr.trim()
                        ),
                        next_steps: vec![
                            "Attach virtio-win.iso via Zeus OS Guest Tools.".into(),
                            "Run install.ps1 manually inside the VM as Administrator.".into(),
                        ],
                        bootstrap_script: Some(ps_script),
                    });
                }
                Err(e) => {
                    return Ok(GuestAgentInstallResult {
                        success: false,
                        method: "qga-exec".into(),
                        is_windows: true,
                        cloud_init_updated: false,
                        channel_updated: false,
                        vm_restarted: false,
                        needs_restart: false,
                        pending: true,
                        message: format!("QGA exec unavailable: {}", e.message),
                        next_steps: vec![
                            "Install QEMU Guest Agent from virtio-win ISO first.".into(),
                            format!("Manual: $env:ZYVOR_AGENT_URL='{agent_url}'; .\\install.ps1"),
                        ],
                        bootstrap_script: Some(ps_script),
                    });
                }
            }
        }
        return Ok(GuestAgentInstallResult {
            success: false,
            method: "virtio-win".into(),
            is_windows: true,
            cloud_init_updated: false,
            channel_updated: false,
            vm_restarted: false,
            needs_restart: false,
            pending: false,
            message: "Windows: attach virtio-win ISO for QEMU Guest Agent, or use native Zeus agent install.ps1.".into(),
            next_steps: vec![
                "Open Zeus OS → VM → Guest Tools and attach virtio-win.iso.".into(),
                "Install QEMU Guest Agent / virtio-win-guest-tools inside the VM.".into(),
                format!("Native agent: $env:ZYVOR_AGENT_URL='{agent_url}'; .\\install.ps1"),
            ],
            bootstrap_script: Some(ps_script),
        });
    }

    if install_method == "qga" && !agent_live {
        return Err(crate::error::ApiError::bad_request(
            "method=qga requires a connected QEMU guest agent",
        ));
    }

    let binary_url = resolve_guestkit_binary_url();
    if vmi_running && agent_live && !zyvor_tools_connected(&vm, vmi.as_ref()) {
        let script = qga_bootstrap_script(&binary_url);
        if install_method == "qga" || install_method == "auto" {
            match crate::kubevirt_qga::qga_exec_shell(&client, namespace, name, &script, 180).await {
                Ok(exec) if exec.exit_code == 0 => {
                    let channel_updated = merge_guest_agent_channel_into_vm(&mut vm);
                    let cloud_init_updated = merge_guest_agent_cloud_init_in_vm(&mut vm, &binary_url);
                    if channel_updated || cloud_init_updated {
                        let patch = json!({
                            "spec": vm.get("spec").cloned().unwrap_or(Value::Null)
                        });
                        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                            .await
                            .map_err(|e| ApiError::internal(format!("patch VM {namespace}/{name}: {e}")))?;
                    }
                    return Ok(GuestAgentInstallResult {
                        success: true,
                        method: "qga-exec".into(),
                        is_windows: false,
                        cloud_init_updated,
                        channel_updated,
                        vm_restarted: false,
                        needs_restart: false,
                        pending: false,
                        message: "Installed zyvor-guest-agent via QGA guest-exec (hands-off).".into(),
                        next_steps: vec![
                            "Refresh guest info — agent should connect over virtio-serial.".into(),
                        ],
                        bootstrap_script: None,
                    });
                }
                Ok(exec) => {
                    return Ok(GuestAgentInstallResult {
                        success: false,
                        method: "qga-exec".into(),
                        is_windows: false,
                        cloud_init_updated: false,
                        channel_updated: false,
                        vm_restarted: false,
                        needs_restart: false,
                        pending: true,
                        message: format!(
                            "QGA bootstrap failed (exit {}): {}",
                            exec.exit_code,
                            exec.stderr.trim()
                        ),
                        next_steps: vec![
                            "Paste bootstrap_script into the VM console as root.".into(),
                        ],
                        bootstrap_script: Some(script),
                    });
                }
                Err(e) if install_method == "qga" => {
                    return Err(crate::error::ApiError::internal(format!(
                        "QGA exec failed: {}",
                        e.message
                    )));
                }
                Err(_) => {
                    // auto: fall through to cloud-init merge below
                }
            }
        }
        let channel_updated = merge_guest_agent_channel_into_vm(&mut vm);
        let cloud_init_updated = merge_guest_agent_cloud_init_in_vm(&mut vm, &binary_url);
        if channel_updated || cloud_init_updated {
            let patch = json!({
                "spec": vm.get("spec").cloned().unwrap_or(Value::Null)
            });
            api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(|e| ApiError::internal(format!("patch VM {namespace}/{name}: {e}")))?;
        }
        return Ok(GuestAgentInstallResult {
            success: true,
            method: "qga-bootstrap".into(),
            is_windows: false,
            cloud_init_updated,
            channel_updated,
            vm_restarted: false,
            needs_restart: false,
            pending: true,
            message: "QEMU guest agent connected — bootstrap script ready (QGA exec unavailable).".into(),
            next_steps: vec![
                "Paste bootstrap_script into the VM console as root.".into(),
                "Refresh guest info after zyvor-guest-agent starts.".into(),
            ],
            bootstrap_script: Some(script),
        });
    }

    if install_method == "qga" {
        return Err(crate::error::ApiError::bad_request(
            "method=qga requires a running VM with connected QEMU guest agent",
        ));
    }

    let channel_updated = merge_guest_agent_channel_into_vm(&mut vm);
    let cloud_init_updated = merge_guest_agent_cloud_init_in_vm(&mut vm, &binary_url);

    if !channel_updated && !cloud_init_updated {
        return Ok(GuestAgentInstallResult {
            success: true,
            method: "none".into(),
            is_windows: false,
            cloud_init_updated: false,
            channel_updated: false,
            vm_restarted: false,
            needs_restart: false,
            pending: false,
            message: "GuestKit agent bootstrap already present in VM spec.".into(),
            next_steps: if vmi_running {
                vec!["Restart the VM if the agent is still not connected.".into()]
            } else {
                vec!["Start the VM — cloud-init will install guestkit-agent on first boot.".into()]
            },
            bootstrap_script: None,
        });
    }

    let patch = json!({
        "spec": vm.get("spec").cloned().unwrap_or(Value::Null)
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map_err(|e| ApiError::internal(format!("patch VM {namespace}/{name}: {e}")))?;

    let mut vm_restarted = false;
    if restart && vmi_running {
        restart_vm(client.clone(), namespace, name).await?;
        vm_restarted = true;
    }

    let mut next_steps = Vec::new();
    if vm_restarted {
        next_steps.push("VM restarted — cloud-init will install guestkit-agent on boot.".into());
    } else if vmi_running {
        next_steps.push("Restart the VM so cloud-init applies guestkit-agent.".into());
    } else {
        next_steps.push("Start the VM — cloud-init will install guestkit-agent on first boot.".into());
    }
    next_steps.push("Refresh guest info after the VM is running again.".into());

    Ok(GuestAgentInstallResult {
        success: true,
        method: "cloud_init".into(),
        is_windows: false,
        cloud_init_updated,
        channel_updated,
        vm_restarted,
        needs_restart: vmi_running && !vm_restarted,
        pending: true,
        message: if cloud_init_updated || channel_updated {
            "Merged GuestKit agent bootstrap into VM spec.".into()
        } else {
            "GuestKit agent bootstrap already present.".into()
        },
        next_steps,
        bootstrap_script: None,
    })
}

fn dv_resource() -> ApiResource {
    ApiResource {
        group: "cdi.kubevirt.io".into(),
        version: "v1beta1".into(),
        api_version: "cdi.kubevirt.io/v1beta1".into(),
        kind: "DataVolume".into(),
        plural: "datavolumes".into(),
    }
}

fn merge_iso_cdrom_into_vm(vm: &mut Value, volume_name: &str, pvc_name: &str) -> bool {
    let mut changed = false;
    if let Some(volumes) = vm
        .pointer_mut("/spec/template/spec/volumes")
        .and_then(|v| v.as_array_mut())
    {
        let exists = volumes
            .iter()
            .any(|v| v.get("name").and_then(|n| n.as_str()) == Some(volume_name));
        if !exists {
            volumes.push(json!({
                "name": volume_name,
                "persistentVolumeClaim": { "claimName": pvc_name }
            }));
            changed = true;
        }
    }
    if let Some(disks) = vm
        .pointer_mut("/spec/template/spec/domain/devices/disks")
        .and_then(|d| d.as_array_mut())
    {
        let exists = disks
            .iter()
            .any(|d| d.get("name").and_then(|n| n.as_str()) == Some(volume_name));
        if !exists {
            disks.push(json!({
                "name": volume_name,
                "cdrom": { "bus": "sata", "readonly": true }
            }));
            changed = true;
        }
    }
    changed
}

pub async fn install_vmtools_iso(
    client: Client,
    namespace: &str,
    name: &str,
    iso_url: &str,
    restart: bool,
) -> ApiResult<GuestAgentInstallResult> {
    let vm_ar = vm_resource();
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vm_ar);
    let obj = api
        .get(name)
        .await
        .map_err(|e| ApiError::internal(format!("get VM {namespace}/{name}: {e}")))?;
    let mut vm = serde_json::to_value(&obj)
        .map_err(|e| ApiError::internal(format!("serialize VM: {e}")))?;

    let vmi_ar = vmi_resource();
    let vmi_api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &vmi_ar);
    let vmi = vmi_api.get(name).await.ok().and_then(|o| serde_json::to_value(o).ok());
    let vmi_running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");

    if vm_is_windows(Some(&vm), vmi.as_ref()) {
        return Ok(GuestAgentInstallResult {
            success: false,
            method: "iso".into(),
            is_windows: true,
            cloud_init_updated: false,
            channel_updated: false,
            vm_restarted: false,
            needs_restart: false,
            pending: false,
            message: "Windows VMs use virtio-win ISO via Zeus OS Guest Tools.".into(),
            next_steps: vec![
                "Open Zeus OS → Guest Tools and attach virtio-win.iso.".into(),
            ],
            bootstrap_script: None,
        });
    }

    let dv_name = format!("{name}-vmtools-iso");
    let dv_api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &dv_resource());
    if dv_api.get(&dv_name).await.is_err() {
        let dv = json!({
            "apiVersion": "cdi.kubevirt.io/v1beta1",
            "kind": "DataVolume",
            "metadata": {
                "name": dv_name,
                "namespace": namespace,
                "labels": {
                    "zeus.zyvor.dev/component": "vmtools-iso",
                    "zeus.zyvor.dev/vm": name,
                }
            },
            "spec": {
                "source": {
                    "http": { "url": iso_url }
                },
                "pvc": {
                    "accessModes": ["ReadWriteOnce"],
                    "resources": { "requests": { "storage": "128Mi" } }
                }
            }
        });
        let dv_obj: kube::api::DynamicObject = serde_json::from_value(dv)
            .map_err(|e| ApiError::internal(format!("build DataVolume: {e}")))?;
        dv_api
            .create(&PostParams::default(), &dv_obj)
            .await
            .map_err(|e| ApiError::internal(format!("create DataVolume {dv_name}: {e}")))?;
    }

    let iso_attached = merge_iso_cdrom_into_vm(&mut vm, "vmtools-iso", &dv_name);
    if iso_attached {
        let patch = json!({
            "spec": vm.get("spec").cloned().unwrap_or(Value::Null)
        });
        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map_err(|e| ApiError::internal(format!("patch VM {namespace}/{name}: {e}")))?;
    }

    let mut vm_restarted = false;
    if restart && vmi_running {
        restart_vm(client.clone(), namespace, name).await?;
        vm_restarted = true;
    }

    Ok(GuestAgentInstallResult {
        success: true,
        method: "iso".into(),
        is_windows: false,
        cloud_init_updated: false,
        channel_updated: false,
        vm_restarted,
        needs_restart: vmi_running && !vm_restarted,
        pending: true,
        message: if iso_attached {
            "Zeus VM Tools ISO attached as CD-ROM — import may take a minute.".into()
        } else {
            "Zeus VM Tools ISO CD-ROM already present in VM spec.".into()
        },
        next_steps: vec![
            format!("Wait for DataVolume {dv_name} to import from {iso_url}."),
            "Mount the CD-ROM in the guest and run /linux/install.sh as root.".into(),
            "Refresh guest info after zyvor-guest-agent is running.".into(),
        ],
        bootstrap_script: None,
    })
}

async fn restart_vm(client: Client, namespace: &str, name: &str) -> ApiResult<()> {
    let url = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachines/{name}/restart"
    );
    kubevirt_subresource_put(&client, &url, b"{}").await
}

async fn kubevirt_subresource_put(client: &Client, url: &str, body: &[u8]) -> ApiResult<()> {
    let req = http::Request::builder()
        .method("PUT")
        .uri(url)
        .header("Content-Type", "application/json")
        .body(body.to_vec())
        .map_err(|e| ApiError::internal(format!("build subresource request: {e}")))?;
    match client.request::<Value>(req).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("EOF while parsing") || msg.contains("error decoding response body") {
                Ok(())
            } else {
                Err(ApiError::internal(format!("subresource PUT {url}: {msg}")))
            }
        }
    }
}
