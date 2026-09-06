// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Replace `virtctl guestfs` (libguestfs appliance pod) with a GuestKit pod.
//!
//! No extra crates: talks to the cluster through `kubectl` already on PATH.
//! Domain lifecycle (start/stop/console/VNC) stays on upstream virtctl.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

pub const DEFAULT_IMAGE: &str = "ghcr.io/zyvorai/guestkit:latest";
pub const CONTAINER_NAME: &str = "guestkit";
pub const POD_PREFIX: &str = "guestkit-tools";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeMode {
    Filesystem,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Interactive,
    Inspect,
    Doctor,
    Rescue,
}

#[derive(Debug, Clone, Default)]
pub struct VmScheduling {
    pub node_selector_yaml: String,
    pub tolerations_yaml: String,
}

#[derive(Debug, Clone)]
pub struct GuestfsRequest {
    pub namespace: String,
    pub pvc: String,
    pub volume_mode: VolumeMode,
    pub image: String,
    pub pull_policy: String,
    pub privileged: bool,
    pub kvm: bool,
    pub mode: SessionMode,
    pub extra_args: Vec<String>,
    pub scheduling: VmScheduling,
    pub timeout_secs: u64,
}

impl Default for GuestfsRequest {
    fn default() -> Self {
        Self {
            namespace: "default".into(),
            pvc: String::new(),
            volume_mode: VolumeMode::Filesystem,
            image: default_image(),
            pull_policy: "IfNotPresent".into(),
            privileged: true,
            kvm: false,
            mode: SessionMode::Interactive,
            extra_args: Vec::new(),
            scheduling: VmScheduling::default(),
            timeout_secs: 500,
        }
    }
}

pub fn default_image() -> String {
    std::env::var("GUESTKIT_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.to_string())
}

pub fn pod_name_for(pvc: &str) -> String {
    let safe: String = pvc
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let safe = safe.trim_matches('-');
    let suffix = format!("{:08x}", simple_token());
    let budget = 63usize.saturating_sub(POD_PREFIX.len() + 2 + suffix.len());
    let pvc_part: String = safe.chars().take(budget).collect();
    format!("{POD_PREFIX}-{pvc_part}-{suffix}")
}

fn simple_token() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1);
    nanos ^ std::process::id()
}

pub fn parse_volume_mode(raw: &str) -> VolumeMode {
    if raw.trim().eq_ignore_ascii_case("Block") {
        VolumeMode::Block
    } else {
        VolumeMode::Filesystem
    }
}

pub fn pvc_holder_from_pod_list(list: &Value, pvc: &str) -> Option<String> {
    let items = list.get("items")?.as_array()?;
    for pod in items {
        let name = pod.pointer("/metadata/name")?.as_str()?.to_string();
        let phase = pod
            .pointer("/status/phase")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if matches!(phase, "Succeeded" | "Failed") {
            continue;
        }
        let Some(volumes) = pod.pointer("/spec/volumes").and_then(|v| v.as_array()) else {
            continue;
        };
        for vol in volumes {
            let claimed = vol
                .pointer("/persistentVolumeClaim/claimName")
                .and_then(|v| v.as_str());
            if claimed == Some(pvc) {
                return Some(name);
            }
        }
    }
    None
}

pub fn root_pvc_from_vm(vm: &Value) -> Option<String> {
    let volumes = vm
        .pointer("/spec/template/spec/volumes")
        .and_then(|v| v.as_array())?;
    for vol in volumes {
        if let Some(name) = vol
            .pointer("/persistentVolumeClaim/claimName")
            .and_then(|v| v.as_str())
        {
            return Some(name.to_string());
        }
        if let Some(name) = vol.pointer("/dataVolume/name").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
    }
    None
}

pub fn scheduling_from_vm(vm: &Value) -> VmScheduling {
    let mut sched = VmScheduling::default();
    if let Some(ns) = vm.pointer("/spec/template/spec/nodeSelector") {
        if let Ok(yaml) = serde_yaml::to_string(ns) {
            sched.node_selector_yaml = yaml;
        }
    }
    if let Some(tol) = vm.pointer("/spec/template/spec/tolerations") {
        if let Ok(yaml) = serde_yaml::to_string(tol) {
            sched.tolerations_yaml = yaml;
        }
    }
    sched
}

pub fn render_pod_yaml(name: &str, req: &GuestfsRequest) -> String {
    let (volume_snippet, workdir) = match req.volume_mode {
        VolumeMode::Filesystem => (
            "        volumeMounts:\n          - name: volume\n            mountPath: /disk\n        workingDir: /disk\n",
            "",
        ),
        VolumeMode::Block => (
            "        volumeDevices:\n          - name: volume\n            devicePath: /dev/vda\n",
            "",
        ),
    };
    let _ = workdir;
    let kvm = if req.kvm {
        "        resources:\n          limits:\n            devices.kubevirt.io/kvm: \"1\"\n"
    } else {
        ""
    };
    let privileged = if req.privileged { "true" } else { "false" };
    let caps = if req.privileged {
        "          capabilities:\n            add: [\"SYS_ADMIN\"]\n"
    } else {
        "          capabilities:\n            drop: [\"ALL\"]\n"
    };
    let command = container_command(req);
    let mut yaml = format!(
        "apiVersion: v1\nkind: Pod\nmetadata:\n  name: {name}\n  namespace: {ns}\n  labels:\n    app.kubernetes.io/name: guestkit\n    app.kubernetes.io/component: guestfs\n    guestkit.io/pvc: {pvc}\n    guestkit.io/replaces: virtctl-guestfs\n  annotations:\n    guestkit.io/replaces: \"virtctl guestfs (libguestfs)\"\nspec:\n  restartPolicy: Never\n  containers:\n    - name: {container}\n      image: {image}\n      imagePullPolicy: {pull}\n      stdin: true\n      stdinOnce: true\n      tty: true\n      command: {command}\n      env:\n        - name: GUESTKIT_PVC\n          value: {pvc}\n        - name: GUESTKIT_NAMESPACE\n          value: {ns}\n      securityContext:\n        privileged: {privileged}\n        runAsUser: 0\n{caps}{volume}{kvm}  volumes:\n    - name: volume\n      persistentVolumeClaim:\n        claimName: {pvc}\n",
        ns = req.namespace,
        pvc = req.pvc,
        container = CONTAINER_NAME,
        image = req.image,
        pull = req.pull_policy,
        command = command,
        privileged = privileged,
        caps = caps,
        volume = volume_snippet,
        kvm = kvm,
    );
    if !req.scheduling.node_selector_yaml.is_empty() {
        yaml.push_str("  nodeSelector:\n");
        yaml.push_str(&indent_block(&req.scheduling.node_selector_yaml, 4));
    }
    if !req.scheduling.tolerations_yaml.is_empty() {
        yaml.push_str("  tolerations:\n");
        yaml.push_str(&indent_block(&req.scheduling.tolerations_yaml, 4));
    }
    yaml
}

fn indent_block(yaml: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    yaml.lines()
        .filter(|l| !l.starts_with("---") && !l.is_empty())
        .map(|l| format!("{pad}{l}\n"))
        .collect()
}

fn container_command(req: &GuestfsRequest) -> String {
    let extras = req.extra_args.join(" ");
    let script = match req.mode {
        SessionMode::Interactive => {
            "set +e; if [ -e /dev/vda ]; then DISK=/dev/vda; elif [ -d /disk ]; then DISK=$(find /disk -maxdepth 2 -type f \\( -name '*.qcow2' -o -name '*.raw' -o -name '*.img' -o -name '*.vmdk' \\) 2>/dev/null | head -1); [ -n \"$DISK\" ] || DISK=/disk; fi; echo guestkit replacing virtctl guestfs; echo disk=$DISK; guestkit inspect \"$DISK\" || true; exec guestkit shell \"$DISK\" || exec sh".to_string()
        }
        SessionMode::Inspect => format!(
            "set -e; if [ -e /dev/vda ]; then DISK=/dev/vda; elif [ -d /disk ]; then DISK=$(find /disk -maxdepth 2 -type f \\( -name '*.qcow2' -o -name '*.raw' -o -name '*.img' \\) 2>/dev/null | head -1); [ -n \"$DISK\" ] || DISK=/disk; fi; echo disk=$DISK; exec guestkit inspect \"$DISK\" {extras}"
        ),
        SessionMode::Doctor => format!(
            "set -e; if [ -e /dev/vda ]; then DISK=/dev/vda; elif [ -d /disk ]; then DISK=$(find /disk -maxdepth 2 -type f \\( -name '*.qcow2' -o -name '*.raw' -o -name '*.img' \\) 2>/dev/null | head -1); [ -n \"$DISK\" ] || DISK=/disk; fi; echo disk=$DISK; exec guestkit doctor \"$DISK\" --target kubevirt {extras}"
        ),
        SessionMode::Rescue => format!(
            "set -e; if [ -e /dev/vda ]; then DISK=/dev/vda; elif [ -d /disk ]; then DISK=$(find /disk -maxdepth 2 -type f \\( -name '*.qcow2' -o -name '*.raw' -o -name '*.img' \\) 2>/dev/null | head -1); [ -n \"$DISK\" ] || DISK=/disk; fi; echo disk=$DISK; exec guestkit rescue \"$DISK\" {extras}"
        ),
    };
    serde_json::to_string(&vec!["sh", "-c", &script])
        .unwrap_or_else(|_| "[\"sh\",\"-c\",\"exec sh\"]".into())
}

/// Cluster entry: resolve PVC, build pod, attach or stream logs, delete.
pub fn run(mut req: GuestfsRequest, kubectl: &str) -> Result<()> {
    if req.pvc.is_empty() {
        bail!("PVC name is required (or pass --vm to resolve the root disk)");
    }
    let name = pod_name_for(&req.pvc);
    let yaml = render_pod_yaml(&name, &req);

    eprintln!("Use image: {}", req.image);
    match req.volume_mode {
        VolumeMode::Filesystem => eprintln!("The PVC will be mounted at /disk"),
        VolumeMode::Block => eprintln!("The PVC will be attached as /dev/vda"),
    }

    kubectl_apply(kubectl, &req.namespace, &yaml)?;
    let attach_result = wait_and_enter(kubectl, &req, &name);
    let _ = kubectl_delete(kubectl, &req.namespace, &name);
    let _ = &mut req;
    attach_result
}

fn wait_and_enter(kubectl: &str, req: &GuestfsRequest, name: &str) -> Result<()> {
    let timeout = format!("{}s", req.timeout_secs);
    let status = Command::new(kubectl)
        .args([
            "-n",
            &req.namespace,
            "wait",
            "--for=condition=Ready",
            &format!("pod/{name}"),
            &format!("--timeout={timeout}"),
        ])
        .status()
        .with_context(|| format!("spawn {kubectl} wait"))?;
    if !status.success() {
        let _ = Command::new(kubectl)
            .args(["-n", &req.namespace, "describe", "pod", name])
            .status();
        bail!("pod {name} was not Ready in {timeout}");
    }
    match req.mode {
        SessionMode::Interactive => {
            let status = Command::new(kubectl)
                .args([
                    "-n",
                    &req.namespace,
                    "attach",
                    "-it",
                    name,
                    "-c",
                    CONTAINER_NAME,
                ])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .context("kubectl attach")?;
            if !status.success() {
                bail!("kubectl attach failed");
            }
        }
        _ => {
            let status = Command::new(kubectl)
                .args([
                    "-n",
                    &req.namespace,
                    "logs",
                    "-f",
                    name,
                    "-c",
                    CONTAINER_NAME,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .context("kubectl logs")?;
            if !status.success() {
                bail!("kubectl logs failed");
            }
        }
    }
    Ok(())
}

fn kubectl_apply(kubectl: &str, namespace: &str, yaml: &str) -> Result<()> {
    let mut child = Command::new(kubectl)
        .args(["-n", namespace, "apply", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("spawn {kubectl} apply"))?;
    child
        .stdin
        .as_mut()
        .context("kubectl apply stdin")?
        .write_all(yaml.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("kubectl apply failed");
    }
    Ok(())
}

fn kubectl_delete(kubectl: &str, namespace: &str, name: &str) -> Result<()> {
    let _ = Command::new(kubectl)
        .args([
            "-n",
            namespace,
            "delete",
            "pod",
            name,
            "--wait=false",
            "--ignore-not-found=true",
        ])
        .status();
    Ok(())
}

pub fn kubectl_json(kubectl: &str, args: &[&str]) -> Result<Value> {
    let out = Command::new(kubectl)
        .args(args)
        .output()
        .with_context(|| format!("spawn {kubectl}"))?;
    if !out.status.success() {
        bail!(
            "{} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    serde_json::from_slice(&out.stdout).context("parse kubectl json")
}

pub fn resolve_namespace(explicit: Option<&str>, kubectl: &str) -> String {
    if let Some(ns) = explicit {
        if !ns.is_empty() {
            return ns.to_string();
        }
    }
    if let Ok(out) = Command::new(kubectl)
        .args([
            "config",
            "view",
            "--minify",
            "--output",
            "jsonpath={..namespace}",
        ])
        .output()
    {
        if out.status.success() {
            let ns = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !ns.is_empty() {
                return ns;
            }
        }
    }
    "default".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_name_is_dns1123() {
        let name = pod_name_for("My_PVC.with dots");
        assert!(name.starts_with("guestkit-tools-"));
        assert!(name.len() <= 63);
        assert!(name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn filesystem_yaml_mounts_disk() {
        let req = GuestfsRequest {
            namespace: "migration".into(),
            pvc: "legacy-app-disk".into(),
            image: "ghcr.io/zyvorai/guestkit:test".into(),
            ..GuestfsRequest::default()
        };
        let yaml = render_pod_yaml("guestkit-tools-x", &req);
        assert!(yaml.contains("mountPath: /disk"));
        assert!(yaml.contains("claimName: legacy-app-disk"));
        assert!(!yaml.contains("devicePath: /dev/vda"));
        assert!(yaml.contains("guestkit.io/replaces: virtctl-guestfs"));
        assert!(yaml.contains("privileged: true"));
    }

    #[test]
    fn block_yaml_uses_vda() {
        let req = GuestfsRequest {
            pvc: "block-disk".into(),
            volume_mode: VolumeMode::Block,
            ..GuestfsRequest::default()
        };
        let yaml = render_pod_yaml("n", &req);
        assert!(yaml.contains("devicePath: /dev/vda"));
        assert!(!yaml.contains("mountPath: /disk"));
    }

    #[test]
    fn kvm_limit_only_when_requested() {
        let mut req = GuestfsRequest {
            pvc: "d".into(),
            kvm: true,
            ..GuestfsRequest::default()
        };
        assert!(render_pod_yaml("n", &req).contains("devices.kubevirt.io/kvm"));
        req.kvm = false;
        assert!(!render_pod_yaml("n", &req).contains("devices.kubevirt.io/kvm"));
    }

    #[test]
    fn pvc_in_use_detects_running_pod() {
        let list = serde_json::json!({
            "items": [{
                "metadata": {"name": "virt-launcher-vm"},
                "status": {"phase": "Running"},
                "spec": {"volumes": [{
                    "persistentVolumeClaim": {"claimName": "rootdisk"}
                }]}
            }]
        });
        assert_eq!(
            pvc_holder_from_pod_list(&list, "rootdisk").as_deref(),
            Some("virt-launcher-vm")
        );
        assert!(pvc_holder_from_pod_list(&list, "other").is_none());
    }

    #[test]
    fn root_pvc_prefers_claim_then_datavolume() {
        let vm = serde_json::json!({
            "spec": {"template": {"spec": {"volumes": [
                {"name": "cloudinit", "cloudInitNoCloud": {}},
                {"name": "root", "persistentVolumeClaim": {"claimName": "app-disk"}}
            ]}}}
        });
        assert_eq!(root_pvc_from_vm(&vm).as_deref(), Some("app-disk"));
        let vm = serde_json::json!({
            "spec": {"template": {"spec": {"volumes": [
                {"dataVolume": {"name": "dv-root"}}
            ]}}}
        });
        assert_eq!(root_pvc_from_vm(&vm).as_deref(), Some("dv-root"));
    }
}
