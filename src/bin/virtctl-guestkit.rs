// SPDX-License-Identifier: Apache-2.0
//! virtctl / kubectl plugin: `virtctl guestkit …` and `kubectl guestkit …`.
//!
//! Install:
//!   cargo build --release --bin virtctl-guestkit
//!   install -m 0755 target/release/virtctl-guestkit /usr/local/bin/virtctl-guestkit
//!   ln -s virtctl-guestkit /usr/local/bin/kubectl-guestkit
//!
//! Domain lifecycle stays with virtctl. Guest/disk work is GuestKit:
//!   virtctl-guestkit guestfs <pvc>          # replaces virtctl guestfs
//!   virtctl-guestkit doctor --image disk.qcow2
//!   virtctl-guestkit doctor --vm my-vm      # PVC via kubectl
//!   virtctl-guestkit resolve / gate         # hostDisk cutover helpers

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use guestkit::virtctl_guestfs::{
    default_image, kubectl_json, parse_volume_mode, pvc_holder_from_pod_list, resolve_namespace,
    root_pvc_from_vm, run, scheduling_from_vm, GuestfsRequest, SessionMode,
};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(
    name = "virtctl-guestkit",
    about = "GuestKit plugin for virtctl/kubectl — replaces virtctl guestfs"
)]
struct Cli {
    #[command(subcommand)]
    cmd: PluginCmd,
}

#[derive(Subcommand)]
enum PluginCmd {
    /// Interactive GuestKit session on a PVC — drop-in for `virtctl guestfs`
    Guestfs {
        /// PVC name (optional when --vm is set)
        pvc: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
        /// Resolve root PVC and copy nodeSelector/tolerations from this VM
        #[arg(long)]
        vm: Option<String>,
        /// GuestKit image (default $GUESTKIT_IMAGE or ghcr.io/zyvorai/guestkit:latest)
        #[arg(long)]
        image: Option<String>,
        #[arg(long, default_value = "IfNotPresent")]
        pull_policy: String,
        /// Request devices.kubevirt.io/kvm (not required for GuestKit)
        #[arg(long, default_value_t = false)]
        kvm: bool,
        #[arg(long, default_value_t = true)]
        privileged: bool,
        #[arg(long, default_value_t = 500)]
        timeout: u64,
    },
    /// One-shot `guestkit inspect` against a PVC / VM root disk
    Inspect {
        pvc: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
        #[arg(long)]
        vm: Option<String>,
        #[arg(long)]
        image: Option<String>,
    },
    /// `guestkit doctor` on a local image or a cluster PVC
    Doctor {
        pvc: Option<String>,
        vm: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
        /// Local disk image (no cluster)
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long, default_value = "kubevirt")]
        target: String,
        #[arg(long)]
        explain: bool,
        /// Container image when running against a PVC
        #[arg(long)]
        container_image: Option<String>,
    },
    /// `guestkit rescue` against a PVC
    Rescue {
        pvc: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
        #[arg(long)]
        vm: Option<String>,
        #[arg(short = 'o', long)]
        operation: Option<String>,
        #[arg(long)]
        image: Option<String>,
    },
    /// Resolve hostDisk / PVC name from a VM/VMI (does not mount PVCs)
    Resolve {
        vm: String,
        #[arg(short, long)]
        namespace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run `guestkit gate` against --image or a resolved hostDisk
    Gate {
        vm: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long)]
        passport: Option<PathBuf>,
        #[arg(long, default_value_t = 80.0)]
        fail_below: f64,
    },
    /// Run `guestkit passport emit` on a local disk
    Passport {
        vm: Option<String>,
        #[arg(long)]
        image: PathBuf,
        #[arg(long, default_value = "kubevirt")]
        target: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Run `guestkit passport handoff`
    Handoff {
        #[arg(long)]
        passport: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        fail_below: Option<f64>,
    },
}

fn main() {
    if let Err(e) = run_cli() {
        eprintln!("virtctl-guestkit: {e:#}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    let cli = parse_cli();
    let kubectl = std::env::var("KUBECTL").unwrap_or_else(|_| "kubectl".into());
    match cli.cmd {
        PluginCmd::Guestfs {
            pvc,
            namespace,
            vm,
            image,
            pull_policy,
            kvm,
            privileged,
            timeout,
        } => cluster_session(
            &kubectl,
            SessionMode::Interactive,
            pvc,
            vm,
            namespace,
            image,
            pull_policy,
            kvm,
            privileged,
            timeout,
            Vec::new(),
        ),
        PluginCmd::Inspect {
            pvc,
            namespace,
            vm,
            image,
        } => cluster_session(
            &kubectl,
            SessionMode::Inspect,
            pvc,
            vm,
            namespace,
            image,
            "IfNotPresent".into(),
            false,
            true,
            500,
            Vec::new(),
        ),
        PluginCmd::Doctor {
            pvc,
            vm,
            namespace,
            image,
            target,
            explain,
            container_image,
        } => {
            if let Some(path) = image {
                if let Some(name) = vm.as_deref() {
                    hint_vm(name, namespace.as_deref());
                }
                let mut args = vec![
                    "doctor".into(),
                    path.display().to_string(),
                    "--target".into(),
                    target,
                ];
                if explain {
                    args.push("--explain".into());
                }
                return exec_guestkit(&args);
            }
            let mut extra = Vec::new();
            if explain {
                extra.push("--explain".into());
            }
            cluster_session(
                &kubectl,
                SessionMode::Doctor,
                pvc,
                vm,
                namespace,
                container_image,
                "IfNotPresent".into(),
                false,
                true,
                500,
                extra,
            )
        }
        PluginCmd::Rescue {
            pvc,
            namespace,
            vm,
            operation,
            image,
        } => {
            let extra = operation
                .into_iter()
                .flat_map(|o| vec!["-o".into(), o])
                .collect();
            cluster_session(
                &kubectl,
                SessionMode::Rescue,
                pvc,
                vm,
                namespace,
                image,
                "IfNotPresent".into(),
                false,
                true,
                500,
                extra,
            )
        }
        PluginCmd::Resolve {
            vm,
            namespace,
            json,
        } => {
            let info = inspect_vm(&kubectl, &vm, namespace.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("kind={} name={}", info.kind, info.name);
                for d in &info.disks {
                    println!("  {} {:?}", d.kind, d.path);
                }
            }
            Ok(())
        }
        PluginCmd::Gate {
            vm,
            namespace,
            image,
            passport,
            fail_below,
        } => {
            let mut args = vec!["gate".into(), "--fail-below".into(), fail_below.to_string()];
            if let Some(p) = passport {
                args.push("--passport".into());
                args.push(p.display().to_string());
            } else {
                let img = resolve_image(&kubectl, image, vm.as_deref(), namespace.as_deref())?;
                args.push("--image".into());
                args.push(img.display().to_string());
            }
            exec_guestkit(&args)
        }
        PluginCmd::Passport {
            vm,
            image,
            target,
            output,
        } => {
            if let Some(name) = vm.as_deref() {
                hint_vm(name, None);
            }
            exec_guestkit(&[
                "passport".into(),
                "emit".into(),
                image.display().to_string(),
                "--target".into(),
                target,
                "-o".into(),
                output.display().to_string(),
            ])
        }
        PluginCmd::Handoff {
            passport,
            output,
            fail_below,
        } => {
            let mut args = vec![
                "passport".into(),
                "handoff".into(),
                passport.display().to_string(),
            ];
            if let Some(o) = output {
                args.push("-o".into());
                args.push(o.display().to_string());
            }
            if let Some(n) = fail_below {
                args.push("--fail-below".into());
                args.push(n.to_string());
            }
            exec_guestkit(&args)
        }
    }
}

/// `kubectl guestkit guestfs …` and `virtctl guestkit guestfs …` drop the extra token.
fn parse_cli() -> Cli {
    let mut args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("guestkit") {
        args.remove(1);
    }
    Cli::parse_from(args)
}

#[allow(clippy::too_many_arguments)]
fn cluster_session(
    kubectl: &str,
    mode: SessionMode,
    pvc: Option<String>,
    vm: Option<String>,
    namespace: Option<String>,
    image: Option<String>,
    pull_policy: String,
    kvm: bool,
    privileged: bool,
    timeout: u64,
    extra_args: Vec<String>,
) -> Result<()> {
    let ns = resolve_namespace(namespace.as_deref(), kubectl);
    let (pvc_name, scheduling) = if let Some(vm_name) = vm.as_deref() {
        let vm_json = kubectl_json(kubectl, &["-n", &ns, "get", "vm", vm_name, "-o", "json"])?;
        let sched = scheduling_from_vm(&vm_json);
        let resolved = pvc
            .clone()
            .or_else(|| root_pvc_from_vm(&vm_json))
            .with_context(|| format!("VirtualMachine {ns}/{vm_name} has no PVC/DataVolume"))?;
        (resolved, sched)
    } else {
        (
            pvc.context("PVC name is required (or pass --vm)")?,
            Default::default(),
        )
    };

    let pods = kubectl_json(kubectl, &["-n", &ns, "get", "pods", "-o", "json"])?;
    if let Some(holder) = pvc_holder_from_pod_list(&pods, &pvc_name) {
        bail!(
            "PVC {ns}/{pvc_name} is already attached to pod {holder}. Stop the VM or delete that pod first."
        );
    }

    let pvc_out = Command::new(kubectl)
        .args([
            "-n",
            &ns,
            "get",
            "pvc",
            &pvc_name,
            "-o",
            "jsonpath={.spec.volumeMode}",
        ])
        .output()
        .context("kubectl get pvc")?;
    if !pvc_out.status.success() {
        bail!(
            "PVC {ns}/{pvc_name} not found: {}",
            String::from_utf8_lossy(&pvc_out.stderr)
        );
    }
    let volume_mode = parse_volume_mode(&String::from_utf8_lossy(&pvc_out.stdout));

    let req = GuestfsRequest {
        namespace: ns,
        pvc: pvc_name,
        volume_mode,
        image: image.unwrap_or_else(default_image),
        pull_policy,
        privileged,
        kvm,
        mode,
        extra_args,
        scheduling,
        timeout_secs: timeout,
    };
    run(req, kubectl)
}

fn hint_vm(name: &str, namespace: Option<&str>) {
    let ns = namespace.unwrap_or("default");
    eprintln!("# VM {ns}/{name} — fetch YAML with: kubectl get vm {name} -n {ns} -o yaml");
}

#[derive(serde::Serialize)]
struct VmDisks {
    kind: String,
    name: String,
    disks: Vec<ResolvedDisk>,
}

#[derive(serde::Serialize)]
struct ResolvedDisk {
    kind: String,
    path: Option<String>,
}

fn resolve_image(
    kubectl: &str,
    explicit: Option<PathBuf>,
    vm: Option<&str>,
    namespace: Option<&str>,
) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let name = vm.ok_or_else(|| anyhow::anyhow!("pass --image PATH or a VM name"))?;
    let info = inspect_vm(kubectl, name, namespace)?;
    if let Some(p) = info
        .disks
        .iter()
        .find(|d| d.kind == "hostDisk" && d.path.is_some())
        .and_then(|d| d.path.clone())
    {
        return Ok(PathBuf::from(p));
    }
    let pvcs: Vec<_> = info
        .disks
        .iter()
        .filter(|d| d.kind == "pvc")
        .filter_map(|d| d.path.clone())
        .collect();
    anyhow::bail!(
        "no hostDisk on {name}; PVCs {:?} are not mounted by this plugin — use guestfs/inspect/doctor --vm, or pass --image",
        pvcs
    )
}

fn inspect_vm(kubectl: &str, name: &str, namespace: Option<&str>) -> Result<VmDisks> {
    let ns = namespace.unwrap_or("default");
    let v = kubectl_json(kubectl, &["get", "vm", name, "-n", ns, "-o", "json"])
        .or_else(|_| kubectl_json(kubectl, &["get", "vmi", name, "-n", ns, "-o", "json"]))?;
    let kind = v
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or("VirtualMachine")
        .to_string();
    let volumes = v
        .pointer("/spec/template/spec/volumes")
        .or_else(|| v.pointer("/spec/volumes"))
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut disks = Vec::new();
    for vol in volumes {
        if let Some(p) = vol
            .get("hostDisk")
            .and_then(|h| h.get("path"))
            .and_then(|x| x.as_str())
        {
            disks.push(ResolvedDisk {
                kind: "hostDisk".into(),
                path: Some(p.to_string()),
            });
        } else if let Some(c) = vol
            .get("persistentVolumeClaim")
            .and_then(|p| p.get("claimName"))
            .and_then(|x| x.as_str())
        {
            disks.push(ResolvedDisk {
                kind: "pvc".into(),
                path: Some(c.to_string()),
            });
        }
    }
    Ok(VmDisks {
        kind,
        name: name.into(),
        disks,
    })
}

fn exec_guestkit(args: &[String]) -> Result<()> {
    let bin = std::env::var("GUESTKIT_BIN").unwrap_or_else(|_| "guestkit".into());
    let status = Command::new(&bin)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("spawn {bin}"))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
