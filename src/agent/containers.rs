// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Container and Kubernetes awareness inside VMs (spec §10).
//!
//! Read-only discovery of container runtimes (Docker/Podman/containerd via
//! CRI) and Kubernetes node membership, plus migration-risk analysis of
//! workloads that don't move cleanly between hypervisors (HostPath mounts,
//! privileged/host-networked containers, device passthrough, local storage).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    #[serde(default)]
    pub privileged: bool,
    #[serde(default)]
    pub host_network: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_path_mounts: Vec<String>,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

fn has_cmd(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {cmd}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The CRI socket to hand crictl. k3s and rke2 run their own containerd on
/// non-default socket paths; crictl won't find them without an explicit
/// endpoint, so probe the known locations.
fn cri_endpoint() -> Option<String> {
    for sock in [
        "/run/k3s/containerd/containerd.sock",
        "/run/containerd/containerd.sock",
        "/var/run/crio/crio.sock",
        "/run/crio/crio.sock",
    ] {
        if Path::new(sock).exists() {
            return Some(format!("unix://{sock}"));
        }
    }
    None
}

/// Run crictl with the resolved runtime endpoint.
fn crictl(args: &[&str]) -> Option<String> {
    if !has_cmd("crictl") {
        return None;
    }
    let mut full: Vec<String> = Vec::new();
    if let Some(ep) = cri_endpoint() {
        full.push("--runtime-endpoint".to_string());
        full.push(ep);
    }
    full.extend(args.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    run("crictl", &refs)
}

fn runtime_version(cmd: &str) -> Option<String> {
    run(cmd, &["--version"]).map(|s| s.lines().next().unwrap_or("").trim().to_string())
}

/// Detected runtimes and their versions.
fn detect_runtimes() -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    if has_cmd("docker") && Path::new("/var/run/docker.sock").exists() {
        out.push(("docker".to_string(), runtime_version("docker")));
    }
    if has_cmd("podman") {
        out.push(("podman".to_string(), runtime_version("podman")));
    }
    // containerd is the CRI runtime on most Kubernetes nodes; k3s/rke2 use
    // their own socket paths, resolved by cri_endpoint().
    if cri_endpoint().is_some() || has_cmd("containerd") {
        out.push(("containerd".to_string(), runtime_version("containerd")));
    }
    out
}

/// Docker/Podman container inventory via the CLI's Go-template output.
fn cli_containers(cmd: &str) -> Vec<ContainerSummary> {
    // One JSON object per line keeps parsing simple across versions.
    let fmt = "{{json .}}";
    let Some(out) = run(cmd, &["ps", "-a", "--no-trunc", "--format", fmt]) else {
        return Vec::new();
    };
    let mut containers = Vec::new();
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = v
            .get("ID")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = v
            .get("Names")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let image = v
            .get("Image")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let state = v
            .get("State")
            .or_else(|| v.get("Status"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // Deep flags require an inspect; do it only for running containers to
        // bound cost.
        let (privileged, host_network, host_path_mounts) =
            if state.contains("running") || state.contains("Up") {
                inspect_flags(cmd, &id)
            } else {
                (false, false, Vec::new())
            };
        containers.push(ContainerSummary {
            id: id.chars().take(12).collect(),
            name,
            image,
            state,
            privileged,
            host_network,
            host_path_mounts,
        });
    }
    containers
}

fn inspect_flags(cmd: &str, id: &str) -> (bool, bool, Vec<String>) {
    let Some(out) = run(cmd, &["inspect", id]) else {
        return (false, false, Vec::new());
    };
    let Ok(v) = serde_json::from_str::<Value>(&out) else {
        return (false, false, Vec::new());
    };
    let obj = v.get(0).unwrap_or(&v);
    let hostcfg = obj.pointer("/HostConfig");
    let privileged = hostcfg
        .and_then(|h| h.get("Privileged"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host_network = hostcfg
        .and_then(|h| h.get("NetworkMode"))
        .and_then(Value::as_str)
        .map(|m| m == "host")
        .unwrap_or(false);
    let host_path_mounts = obj
        .get("Mounts")
        .and_then(Value::as_array)
        .map(|mounts| {
            mounts
                .iter()
                .filter(|m| m.get("Type").and_then(Value::as_str) == Some("bind"))
                .filter_map(|m| m.get("Source").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (privileged, host_network, host_path_mounts)
}

/// Kubernetes node membership and pod/container counts via crictl (CRI).
fn kubernetes_info() -> Option<Value> {
    let is_node = Path::new("/var/lib/kubelet").exists()
        || Path::new("/etc/kubernetes").exists()
        || Path::new("/var/lib/rancher/k3s").exists();
    if !is_node {
        return None;
    }
    // crictl talks to the CRI socket; count running pods/containers.
    let pods = crictl(&["pods", "-q"]).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count());
    let containers =
        crictl(&["ps", "-q"]).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count());
    let distribution = if Path::new("/var/lib/rancher/k3s").exists() {
        "k3s"
    } else if Path::new("/var/lib/rancher/rke2").exists() {
        "rke2"
    } else {
        "kubernetes"
    };
    let kubelet_running = Command::new("systemctl")
        .args(["is-active", "--quiet", "kubelet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        || Command::new("systemctl")
            .args(["is-active", "--quiet", "k3s"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    Some(json!({
        "is_node": true,
        "distribution": distribution,
        "kubelet_running": kubelet_running,
        "running_pods": pods,
        "running_containers": containers,
    }))
}

/// CRI containers (containerd on a k8s node without Docker) via crictl.
fn cri_containers() -> Vec<ContainerSummary> {
    let Some(out) = crictl(&["ps", "-a", "-o", "json"]) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&out) else {
        return Vec::new();
    };
    v.get("containers")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .take(200)
                .map(|c| ContainerSummary {
                    id: c
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(12)
                        .collect(),
                    name: c
                        .pointer("/metadata/name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    image: c
                        .pointer("/image/image")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    state: c
                        .get("state")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .replace("CONTAINER_", "")
                        .to_lowercase(),
                    ..Default::default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Full container inventory + migration-risk assessment.
pub fn inventory() -> Value {
    let runtimes = detect_runtimes();
    let mut containers = Vec::new();

    if runtimes.iter().any(|(n, _)| n == "docker") {
        containers.extend(cli_containers("docker"));
    }
    if runtimes.iter().any(|(n, _)| n == "podman") {
        containers.extend(cli_containers("podman"));
    }
    // On a k8s/containerd node without Docker, use crictl.
    if containers.is_empty() && has_cmd("crictl") {
        containers.extend(cri_containers());
    }

    let k8s = kubernetes_info();

    // Migration-risk flags: workloads bound to this host/hypervisor.
    let mut risks = Vec::new();
    let privileged = containers.iter().filter(|c| c.privileged).count();
    let host_net = containers.iter().filter(|c| c.host_network).count();
    let hostpath: Vec<String> = containers
        .iter()
        .flat_map(|c| c.host_path_mounts.iter().cloned())
        .filter(|p| !p.starts_with("/var/lib/docker") && !p.starts_with("/var/lib/containerd"))
        .collect();
    if privileged > 0 {
        risks.push(format!(
            "{privileged} privileged container(s) — may depend on host devices/kernel"
        ));
    }
    if host_net > 0 {
        risks.push(format!(
            "{host_net} container(s) use host networking — IP/port assumptions may break on migration"
        ));
    }
    if !hostpath.is_empty() {
        risks.push(format!(
            "{} HostPath/bind mount(s) reference host filesystem paths that must exist on the destination",
            hostpath.len()
        ));
    }
    if k8s.is_some() {
        risks.push(
            "Kubernetes node: local persistent volumes and node identity do not migrate with the VM"
                .to_string(),
        );
    }

    json!({
        "runtimes": runtimes
            .iter()
            .map(|(n, v)| json!({ "name": n, "version": v }))
            .collect::<Vec<_>>(),
        "container_count": containers.len(),
        "running": containers.iter().filter(|c| c.state.contains("running") || c.state.contains("Up")).count(),
        "privileged": privileged,
        "host_networked": host_net,
        "host_path_mounts": hostpath,
        "kubernetes": k8s,
        "migration_risks": risks,
        "containers": containers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_is_well_formed() {
        let inv = inventory();
        assert!(inv.get("runtimes").is_some());
        assert!(inv.get("container_count").is_some());
        assert!(inv.get("migration_risks").is_some());
    }

    #[test]
    fn inspect_flags_parse_privileged_and_hostpath() {
        // Simulate `docker inspect` output shape.
        let sample = json!([{
            "HostConfig": { "Privileged": true, "NetworkMode": "host" },
            "Mounts": [
                { "Type": "bind", "Source": "/etc/hosts" },
                { "Type": "volume", "Source": "vol1" }
            ]
        }]);
        let obj = sample.get(0).unwrap();
        let hostcfg = obj.pointer("/HostConfig").unwrap();
        assert!(hostcfg.get("Privileged").unwrap().as_bool().unwrap());
        assert_eq!(
            hostcfg.get("NetworkMode").unwrap().as_str().unwrap(),
            "host"
        );
        let binds: Vec<_> = obj
            .get("Mounts")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m.get("Type").and_then(Value::as_str) == Some("bind"))
            .collect();
        assert_eq!(binds.len(), 1);
    }
}
