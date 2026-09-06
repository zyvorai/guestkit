// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestKit-native VM lifecycle: a deliberately small virsh/libvirt replacement.
//!
//! GuestKit owns guest inspection and boot assurance. QEMU owns execution.
//! QMP owns the few runtime lifecycle operations we need.
//!
//! Normal operator surface:
//!   guestkit vm define NAME IMAGE
//!   guestkit vm plan NAME
//!   guestkit vm start NAME
//!   guestkit vm list
//!   guestkit vm status NAME
//!   guestkit vm shutdown NAME
//!   guestkit vm reboot NAME
//!   guestkit vm pause NAME
//!   guestkit vm resume NAME
//!   guestkit vm destroy NAME
//!   guestkit vm undefine NAME
//!
//! There is no libvirt XML and no virsh dependency in this path.

use crate::assurance::collect_assurance_data;
use crate::boot::{BootTarget, BootabilityReport};
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub const VM_DEFINITION_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    X86_64,
    Aarch64,
}

impl Architecture {
    pub fn from_guestkit(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" | "x64" => Ok(Self::X86_64),
            "aarch64" | "arm64" => Ok(Self::Aarch64),
            other => bail!("unsupported guest architecture: {other:?}"),
        }
    }

    fn qemu_binary(self) -> &'static str {
        match self {
            Self::X86_64 => "qemu-system-x86_64",
            Self::Aarch64 => "qemu-system-aarch64",
        }
    }

    fn machine(self) -> &'static str {
        match self {
            Self::X86_64 => "q35",
            Self::Aarch64 => "virt",
        }
    }

    fn virtio_blk(self) -> &'static str {
        match self {
            Self::X86_64 => "virtio-blk-pci",
            Self::Aarch64 => "virtio-blk-device",
        }
    }

    fn virtio_scsi(self) -> &'static str {
        match self {
            Self::X86_64 => "virtio-scsi-pci",
            Self::Aarch64 => "virtio-scsi-device",
        }
    }

    fn virtio_net(self) -> &'static str {
        match self {
            Self::X86_64 => "virtio-net-pci",
            Self::Aarch64 => "virtio-net-device",
        }
    }

    fn virtio_balloon(self) -> &'static str {
        match self {
            Self::X86_64 => "virtio-balloon-pci",
            Self::Aarch64 => "virtio-balloon-device",
        }
    }

    fn virtio_rng(self) -> &'static str {
        match self {
            Self::X86_64 => "virtio-rng-pci",
            Self::Aarch64 => "virtio-rng-device",
        }
    }

    fn virtio_vsock(self) -> &'static str {
        match self {
            Self::X86_64 => "vhost-vsock-pci",
            Self::Aarch64 => "vhost-vsock-device",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Qcow2,
    Raw,
    Vmdk,
    Vdi,
    Vhd,
}

impl ImageFormat {
    pub fn infer(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match ext.as_str() {
            "qcow2" | "qcow" => Ok(Self::Qcow2),
            "raw" | "img" => Ok(Self::Raw),
            "vmdk" => Ok(Self::Vmdk),
            "vdi" => Ok(Self::Vdi),
            "vhd" | "vpc" => Ok(Self::Vhd),
            _ => bail!(
                "cannot infer image format from {}; expected qcow2/raw/img/vmdk/vdi/vhd",
                path.display()
            ),
        }
    }

    fn qemu_name(self) -> &'static str {
        match self {
            Self::Qcow2 => "qcow2",
            Self::Raw => "raw",
            Self::Vmdk => "vmdk",
            Self::Vdi => "vdi",
            Self::Vhd => "vpc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskBus {
    VirtioBlk,
    VirtioScsi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum NetworkConfig {
    None,
    User { ssh_port: Option<u16> },
    Tap { ifname: String, vhost: bool },
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self::User { ssh_port: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UefiConfig {
    pub code: PathBuf,
    pub vars: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmDefinition {
    pub schema_version: u32,
    pub name: String,
    pub image: PathBuf,
    pub image_format: ImageFormat,
    pub architecture: Architecture,
    pub memory_mb: u64,
    pub vcpus: u16,
    pub disk_bus: DiskBus,
    pub network: NetworkConfig,
    pub readonly: bool,
    pub vsock_cid: Option<u32>,
    pub uefi_required: bool,
    pub uefi: Option<UefiConfig>,
    pub min_boot_score: f64,
}

impl VmDefinition {
    fn validate_for_storage(&self) -> Result<()> {
        validate_name(&self.name)?;

        if self.schema_version != VM_DEFINITION_SCHEMA {
            bail!(
                "unsupported VM definition schema {}; expected {}",
                self.schema_version,
                VM_DEFINITION_SCHEMA
            );
        }
        if !self.image.exists() {
            bail!("disk image does not exist: {}", self.image.display());
        }
        if self.memory_mb < 128 {
            bail!("memory_mb must be at least 128");
        }
        if self.vcpus == 0 {
            bail!("vcpus must be greater than zero");
        }
        if !(0.0..=100.0).contains(&self.min_boot_score) {
            bail!("min_boot_score must be in 0..=100");
        }
        if let Some(cid) = self.vsock_cid {
            if cid < 3 {
                bail!("vsock CID 0, 1 and 2 are reserved; choose CID >= 3");
            }
        }
        if let NetworkConfig::Tap { ifname, .. } = &self.network {
            validate_token("TAP interface", ifname)?;
        }
        Ok(())
    }

    fn validate_for_start(&self) -> Result<()> {
        self.validate_for_storage()?;

        if self.uefi_required && self.uefi.is_none() {
            bail!("GuestKit detected UEFI but no UEFI firmware is configured");
        }

        if let Some(uefi) = &self.uefi {
            if !uefi.code.exists() {
                bail!("UEFI code file does not exist: {}", uefi.code.display());
            }
            if !uefi.vars.exists() {
                bail!("UEFI vars file does not exist: {}", uefi.vars.display());
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct VmStore {
    definitions_dir: PathBuf,
    runtime_dir: PathBuf,
}

impl VmStore {
    pub fn system() -> Self {
        let definitions_dir = std::env::var_os("GUESTKIT_VM_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/lib/guestkit/vms"));
        let runtime_dir = std::env::var_os("GUESTKIT_VM_RUN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/run/guestkit/vms"));
        Self::new(definitions_dir, runtime_dir)
    }

    pub fn new(definitions_dir: impl Into<PathBuf>, runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            definitions_dir: definitions_dir.into(),
            runtime_dir: runtime_dir.into(),
        }
    }

    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.definitions_dir)
            .with_context(|| format!("cannot create {}", self.definitions_dir.display()))?;
        fs::create_dir_all(&self.runtime_dir)
            .with_context(|| format!("cannot create {}", self.runtime_dir.display()))?;
        Ok(())
    }

    fn definition_path(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.definitions_dir.join(format!("{name}.json")))
    }

    fn qmp_socket(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.runtime_dir.join(format!("{name}.qmp")))
    }

    fn pid_file(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.runtime_dir.join(format!("{name}.pid")))
    }

    fn save(&self, def: &VmDefinition) -> Result<()> {
        self.ensure_dirs()?;
        def.validate_for_storage()?;
        let path = self.definition_path(&def.name)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(def)?)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn load(&self, name: &str) -> Result<VmDefinition> {
        let path = self.definition_path(name)?;
        let bytes = fs::read(&path).with_context(|| format!("VM {name:?} is not defined"))?;
        let def: VmDefinition = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid VM definition {}", path.display()))?;
        def.validate_for_storage()?;
        Ok(def)
    }

    fn list(&self) -> Result<Vec<VmDefinition>> {
        self.ensure_dirs()?;
        let mut defs = Vec::new();
        for entry in fs::read_dir(&self.definitions_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            match fs::read(&path)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| {
                    serde_json::from_slice::<VmDefinition>(&bytes).map_err(Into::into)
                }) {
                Ok(def) => defs.push(def),
                Err(err) => log::warn!("ignoring {}: {}", path.display(), err),
            }
        }
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(defs)
    }

    fn delete(&self, name: &str) -> Result<()> {
        let path = self.definition_path(name)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn cleanup_runtime(&self, name: &str) -> Result<()> {
        for path in [self.qmp_socket(name)?, self.pid_file(name)?] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QmpStatus {
    pub running: bool,
    pub status: String,
}

pub struct QmpClient {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl QmpClient {
    pub fn connect(path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .with_context(|| format!("cannot connect to QMP socket {}", path.display()))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        let writer = stream.try_clone()?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
        };

        let greeting = client.read_json()?;
        if greeting.get("QMP").is_none() {
            bail!("invalid QMP greeting");
        }
        client.execute("qmp_capabilities")?;
        Ok(client)
    }

    pub fn query_status(&mut self) -> Result<QmpStatus> {
        serde_json::from_value(self.execute("query-status")?)
            .context("invalid query-status response")
    }

    fn powerdown(&mut self) -> Result<()> {
        self.execute("system_powerdown")?;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.execute("system_reset")?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.execute("stop")?;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.execute("cont")?;
        Ok(())
    }

    fn quit(&mut self) -> Result<()> {
        match self.execute("quit") {
            Ok(_) => Ok(()),
            Err(err) if err.to_string().contains("QMP EOF") => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn execute(&mut self, command: &str) -> Result<Value> {
        serde_json::to_writer(&mut self.writer, &json!({"execute": command}))?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        loop {
            let response = self.read_json()?;
            if response.get("event").is_some() {
                continue;
            }
            if let Some(error) = response.get("error") {
                let class = error
                    .get("class")
                    .and_then(Value::as_str)
                    .unwrap_or("QmpError");
                let desc = error
                    .get("desc")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown QMP error");
                bail!("{class}: {desc}");
            }
            if let Some(value) = response.get("return") {
                return Ok(value.clone());
            }
        }
    }

    fn read_json(&mut self) -> Result<Value> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line)?;
        if n == 0 {
            bail!("QMP EOF");
        }
        serde_json::from_str(line.trim()).context("invalid JSON from QMP")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmState {
    Running,
    Paused,
    Shutoff,
    Starting,
    Unknown,
}

impl std::fmt::Display for VmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Shutoff => write!(f, "shutoff"),
            Self::Starting => write!(f, "starting"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmPlan {
    pub definition: VmDefinition,
    pub boot_score: f64,
    pub confidence: f64,
    pub ready: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub reasons: Vec<String>,
    pub qemu_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmListEntry {
    pub name: String,
    pub state: VmState,
    pub image: PathBuf,
    pub memory_mb: u64,
    pub vcpus: u16,
}

#[derive(Debug, Clone)]
struct DefineOptions {
    name: String,
    image: PathBuf,
    memory_mb: u64,
    vcpus: u16,
    disk_bus: DiskBus,
    network: NetworkConfig,
    readonly: bool,
    vsock_cid: Option<u32>,
    uefi: Option<UefiConfig>,
    min_boot_score: f64,
}

pub struct VmManager {
    store: VmStore,
}

impl VmManager {
    pub fn system() -> Self {
        Self {
            store: VmStore::system(),
        }
    }

    pub fn new(store: VmStore) -> Self {
        Self { store }
    }

    fn define(&self, options: DefineOptions, verbose: bool) -> Result<VmPlan> {
        let (evidence, boot) = collect_assurance_data(&options.image, BootTarget::Kvm, verbose)
            .context("GuestKit could not inspect the VM image")?;
        let architecture = Architecture::from_guestkit(&evidence.os.architecture)?;
        let uefi_required =
            evidence.boot.efi_present || evidence.boot.firmware.eq_ignore_ascii_case("uefi");

        let def = VmDefinition {
            schema_version: VM_DEFINITION_SCHEMA,
            name: options.name,
            image_format: ImageFormat::infer(&options.image)?,
            image: options.image,
            architecture,
            memory_mb: options.memory_mb,
            vcpus: options.vcpus,
            disk_bus: options.disk_bus,
            network: options.network,
            readonly: options.readonly,
            vsock_cid: options.vsock_cid,
            uefi_required,
            uefi: options.uefi,
            min_boot_score: options.min_boot_score,
        };

        self.store.save(&def)?;
        self.plan_from_boot(def, &boot)
    }

    pub fn plan(&self, name: &str, verbose: bool) -> Result<VmPlan> {
        let def = self.store.load(name)?;
        let (_evidence, boot) = collect_assurance_data(&def.image, BootTarget::Kvm, verbose)
            .context("GuestKit could not inspect the VM image")?;
        self.plan_from_boot(def, &boot)
    }

    pub fn start(&self, name: &str, force: bool, verbose: bool) -> Result<VmPlan> {
        if matches!(
            self.state(name)?,
            VmState::Running | VmState::Paused | VmState::Starting
        ) {
            bail!("VM {name:?} is already active");
        }

        let plan = self.plan(name, verbose)?;
        if !plan.ready && !force {
            bail!(
                "GuestKit blocked VM start:\n{}",
                plan.reasons
                    .iter()
                    .map(|v| format!(" - {v}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        plan.definition.validate_for_start()?;
        self.store.ensure_dirs()?;
        self.store.cleanup_runtime(name)?;
        let qmp = self.store.qmp_socket(name)?;
        let pid = self.store.pid_file(name)?;
        start_qemu(&plan.definition, &qmp, &pid)?;
        wait_for_qmp(&qmp, Duration::from_secs(5))?;
        Ok(plan)
    }

    pub fn list(&self) -> Result<Vec<VmListEntry>> {
        let mut out = Vec::new();
        for def in self.store.list()? {
            let state = self.state(&def.name).unwrap_or(VmState::Unknown);
            out.push(VmListEntry {
                name: def.name,
                state,
                image: def.image,
                memory_mb: def.memory_mb,
                vcpus: def.vcpus,
            });
        }
        Ok(out)
    }

    pub fn state(&self, name: &str) -> Result<VmState> {
        let _ = self.store.load(name)?;
        let pid_path = self.store.pid_file(name)?;
        let qmp_path = self.store.qmp_socket(name)?;
        let Some(pid) = read_pid(&pid_path)? else {
            return Ok(VmState::Shutoff);
        };

        if !process_alive(pid) {
            let _ = self.store.cleanup_runtime(name);
            return Ok(VmState::Shutoff);
        }
        if !qmp_path.exists() {
            return Ok(VmState::Starting);
        }

        match QmpClient::connect(&qmp_path).and_then(|mut q| q.query_status()) {
            Ok(status) if status.status == "running" || status.running => Ok(VmState::Running),
            Ok(status) if status.status == "paused" => Ok(VmState::Paused),
            Ok(status) if status.status == "shutdown" => Ok(VmState::Shutoff),
            Ok(_) => Ok(VmState::Unknown),
            Err(_) => Ok(VmState::Unknown),
        }
    }

    pub fn shutdown(&self, name: &str) -> Result<()> {
        self.with_qmp(name, |q| q.powerdown())
    }

    pub fn reboot(&self, name: &str) -> Result<()> {
        self.with_qmp(name, |q| q.reset())
    }

    pub fn pause(&self, name: &str) -> Result<()> {
        self.with_qmp(name, |q| q.pause())
    }

    pub fn resume(&self, name: &str) -> Result<()> {
        self.with_qmp(name, |q| q.resume())
    }

    pub fn destroy(&self, name: &str) -> Result<()> {
        let pid = read_pid(&self.store.pid_file(name)?)?;
        if self.with_qmp(name, |q| q.quit()).is_err() {
            if let Some(pid) = pid {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
        }
        wait_until_dead(pid, Duration::from_secs(3));
        self.store.cleanup_runtime(name)?;
        Ok(())
    }

    pub fn undefine(&self, name: &str, force: bool) -> Result<()> {
        let state = self.state(name).unwrap_or(VmState::Shutoff);
        if matches!(
            state,
            VmState::Running | VmState::Paused | VmState::Starting
        ) {
            if !force {
                bail!("VM {name:?} is active; stop it first or use --force");
            }
            self.destroy(name)?;
        }
        self.store.delete(name)
    }

    pub fn definition(&self, name: &str) -> Result<VmDefinition> {
        self.store.load(name)
    }

    fn with_qmp<T>(&self, name: &str, f: impl FnOnce(&mut QmpClient) -> Result<T>) -> Result<T> {
        let _ = self.store.load(name)?;
        let mut qmp = QmpClient::connect(&self.store.qmp_socket(name)?)?;
        f(&mut qmp)
    }

    fn plan_from_boot(&self, def: VmDefinition, boot: &BootabilityReport) -> Result<VmPlan> {
        let blockers = boot
            .blockers
            .iter()
            .map(|f| format!("{}: {}", f.title, f.message))
            .collect::<Vec<_>>();
        let warnings = boot
            .warnings
            .iter()
            .map(|f| format!("{}: {}", f.title, f.message))
            .collect::<Vec<_>>();
        let mut reasons = Vec::new();

        if !blockers.is_empty() {
            reasons.push(format!("{} GuestKit boot blocker(s)", blockers.len()));
        }
        if boot.score < def.min_boot_score {
            reasons.push(format!(
                "boot score {:.0} is below required {:.0}",
                boot.score, def.min_boot_score
            ));
        }
        if def.uefi_required && def.uefi.is_none() {
            reasons.push("UEFI guest requires --uefi-code and --uefi-vars".into());
        }
        if let Some(uefi) = &def.uefi {
            if !uefi.code.exists() {
                reasons.push(format!("missing UEFI code file {}", uefi.code.display()));
            }
            if !uefi.vars.exists() {
                reasons.push(format!("missing UEFI vars file {}", uefi.vars.display()));
            }
        }

        let qmp = self.store.qmp_socket(&def.name)?;
        let pid = self.store.pid_file(&def.name)?;
        let qemu_command = printable_qemu_command(&def, &qmp, &pid)
            .unwrap_or_else(|err| format!("<unavailable: {err}>"));

        Ok(VmPlan {
            definition: def,
            boot_score: boot.score,
            confidence: boot.confidence,
            ready: reasons.is_empty(),
            blockers,
            warnings,
            reasons,
            qemu_command,
        })
    }
}

pub fn build_qemu_args(
    def: &VmDefinition,
    qmp_socket: &Path,
    pid_file: &Path,
    daemonize: bool,
) -> Result<Vec<String>> {
    def.validate_for_start()?;
    let mut args = Vec::new();

    args.extend(["-name".into(), def.name.clone()]);
    args.extend([
        "-machine".into(),
        format!("{},accel=kvm", def.architecture.machine()),
    ]);
    args.extend(["-cpu".into(), "host".into()]);
    args.extend(["-smp".into(), def.vcpus.to_string()]);
    args.extend(["-m".into(), def.memory_mb.to_string()]);
    args.extend([
        "-qmp".into(),
        format!("unix:{},server=on,wait=off", qemu_path(qmp_socket)?),
    ]);
    args.extend(["-pidfile".into(), qemu_path(pid_file)?]);
    args.extend(["-display".into(), "none".into()]);
    if daemonize {
        args.push("-daemonize".into());
    }

    if let Some(uefi) = &def.uefi {
        args.extend([
            "-drive".into(),
            format!(
                "if=pflash,format=raw,readonly=on,file={}",
                qemu_path(&uefi.code)?
            ),
        ]);
        args.extend([
            "-drive".into(),
            format!("if=pflash,format=raw,file={}", qemu_path(&uefi.vars)?),
        ]);
    }

    args.extend([
        "-drive".into(),
        format!(
            "id=gk_disk0,file={},format={},if=none,cache=none{}",
            qemu_path(&def.image)?,
            def.image_format.qemu_name(),
            if def.readonly { ",readonly=on" } else { "" }
        ),
    ]);

    match def.disk_bus {
        DiskBus::VirtioBlk => args.extend([
            "-device".into(),
            format!(
                "{},drive=gk_disk0,id=gk_vblk0",
                def.architecture.virtio_blk()
            ),
        ]),
        DiskBus::VirtioScsi => {
            args.extend([
                "-device".into(),
                format!("{},id=gk_scsi0", def.architecture.virtio_scsi()),
            ]);
            args.extend([
                "-device".into(),
                "scsi-hd,drive=gk_disk0,bus=gk_scsi0.0,id=gk_scsi_disk0".into(),
            ]);
        }
    }

    match &def.network {
        NetworkConfig::None => {}
        NetworkConfig::User { ssh_port } => {
            let mut backend = "user,id=gk_net0".to_string();
            if let Some(port) = ssh_port {
                if *port == 0 {
                    bail!("ssh_port must be greater than zero");
                }
                backend.push_str(&format!(",hostfwd=tcp:127.0.0.1:{port}-:22"));
            }
            args.extend(["-netdev".into(), backend]);
            args.extend([
                "-device".into(),
                format!(
                    "{},netdev=gk_net0,id=gk_nic0",
                    def.architecture.virtio_net()
                ),
            ]);
        }
        NetworkConfig::Tap { ifname, vhost } => {
            args.extend([
                "-netdev".into(),
                format!(
                    "tap,id=gk_net0,ifname={ifname},script=no,downscript=no,vhost={}",
                    if *vhost { "on" } else { "off" }
                ),
            ]);
            args.extend([
                "-device".into(),
                format!(
                    "{},netdev=gk_net0,id=gk_nic0",
                    def.architecture.virtio_net()
                ),
            ]);
        }
    }

    args.extend([
        "-device".into(),
        format!("{},id=gk_balloon0", def.architecture.virtio_balloon()),
    ]);
    args.extend([
        "-object".into(),
        "rng-random,id=gk_rng_backend0,filename=/dev/urandom".into(),
    ]);
    args.extend([
        "-device".into(),
        format!(
            "{},rng=gk_rng_backend0,id=gk_rng0",
            def.architecture.virtio_rng()
        ),
    ]);

    if let Some(cid) = def.vsock_cid {
        args.extend([
            "-device".into(),
            format!(
                "{},guest-cid={cid},id=gk_vsock0",
                def.architecture.virtio_vsock()
            ),
        ]);
    }

    Ok(args)
}

fn start_qemu(def: &VmDefinition, qmp_socket: &Path, pid_file: &Path) -> Result<()> {
    let output = Command::new(def.architecture.qemu_binary())
        .args(build_qemu_args(def, qmp_socket, pid_file, true)?)
        .output()
        .with_context(|| format!("failed to execute {}", def.architecture.qemu_binary()))?;
    if !output.status.success() {
        bail!(
            "QEMU failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn printable_qemu_command(
    def: &VmDefinition,
    qmp_socket: &Path,
    pid_file: &Path,
) -> Result<String> {
    let mut parts = vec![shell_quote(def.architecture.qemu_binary())];
    parts.extend(
        build_qemu_args(def, qmp_socket, pid_file, true)?
            .iter()
            .map(|v| shell_quote(v)),
    );
    Ok(parts.join(" "))
}

fn wait_for_qmp(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() && QmpClient::connect(path).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("QMP did not become ready at {}", path.display())
}

fn read_pid(path: &Path) -> Result<Option<i32>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            Ok(Some(text.trim().parse::<i32>().with_context(|| {
                format!("invalid PID file {}", path.display())
            })?))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn wait_until_dead(pid: Option<i32>, timeout: Duration) {
    let Some(pid) = pid else {
        return;
    };
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_alive(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if process_alive(pid) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        bail!("VM name must contain 1..=64 characters");
    }
    if !name
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        bail!("VM name may only contain letters, digits, '.', '-' and '_'");
    }
    Ok(())
}

fn validate_token(label: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }
    if !value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':'))
    {
        bail!("{label} contains unsupported characters");
    }
    Ok(())
}

fn qemu_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"-_./:=,".contains(&c))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[derive(Subcommand, Debug)]
pub enum VmAction {
    /// Define a VM from an existing image. GuestKit inspects it first.
    Define {
        name: String,
        image: PathBuf,
        #[arg(long, default_value_t = 4096)]
        memory_mb: u64,
        #[arg(long, default_value_t = 2)]
        vcpus: u16,
        #[arg(long, default_value = "virtio-blk", value_parser = ["virtio-blk", "virtio-scsi"])]
        disk_bus: String,
        #[arg(long)]
        no_network: bool,
        #[arg(long, conflicts_with = "no_network")]
        tap: Option<String>,
        #[arg(long, conflicts_with_all = ["no_network", "tap"])]
        ssh_port: Option<u16>,
        #[arg(long)]
        readonly: bool,
        #[arg(long)]
        vsock_cid: Option<u32>,
        #[arg(long, requires = "uefi_vars")]
        uefi_code: Option<PathBuf>,
        #[arg(long, requires = "uefi_code")]
        uefi_vars: Option<PathBuf>,
        #[arg(long, default_value_t = 70.0)]
        min_boot_score: f64,
    },
    /// Re-run GuestKit assurance and print the QEMU plan.
    Plan {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Start after GuestKit assurance.
    Start {
        name: String,
        #[arg(long)]
        force: bool,
    },
    /// List all defined VMs.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show runtime state.
    Status {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// ACPI-style powerdown request.
    Shutdown { name: String },
    /// Direct VM reset through QMP.
    Reboot { name: String },
    /// Pause vCPUs.
    Pause { name: String },
    /// Resume vCPUs.
    Resume { name: String },
    /// Terminate QEMU immediately (virsh destroy equivalent).
    Destroy { name: String },
    /// Delete only the GuestKit definition, never the VM disk.
    Undefine {
        name: String,
        #[arg(long)]
        force: bool,
    },
    /// Show stored GuestKit VM JSON.
    Show { name: String },
}

pub fn run_cli(action: VmAction, verbose: bool) -> Result<()> {
    let manager = VmManager::system();

    match action {
        VmAction::Define {
            name,
            image,
            memory_mb,
            vcpus,
            disk_bus,
            no_network,
            tap,
            ssh_port,
            readonly,
            vsock_cid,
            uefi_code,
            uefi_vars,
            min_boot_score,
        } => {
            let disk_bus = if disk_bus == "virtio-scsi" {
                DiskBus::VirtioScsi
            } else {
                DiskBus::VirtioBlk
            };
            let network = if no_network {
                NetworkConfig::None
            } else if let Some(ifname) = tap {
                NetworkConfig::Tap {
                    ifname,
                    vhost: true,
                }
            } else {
                NetworkConfig::User { ssh_port }
            };
            let uefi = match (uefi_code, uefi_vars) {
                (Some(code), Some(vars)) => Some(UefiConfig { code, vars }),
                (None, None) => None,
                _ => bail!("both --uefi-code and --uefi-vars are required"),
            };
            let plan = manager.define(
                DefineOptions {
                    name,
                    image,
                    memory_mb,
                    vcpus,
                    disk_bus,
                    network,
                    readonly,
                    vsock_cid,
                    uefi,
                    min_boot_score,
                },
                verbose,
            )?;
            println!("defined {}", plan.definition.name);
            print_plan(&plan);
        }
        VmAction::Plan { name, json } => {
            let plan = manager.plan(&name, verbose)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_plan(&plan);
            }
        }
        VmAction::Start { name, force } => {
            let plan = manager.start(&name, force, verbose)?;
            println!(
                "{} started; GuestKit boot score {:.0}",
                plan.definition.name, plan.boot_score
            );
        }
        VmAction::List { json } => {
            let rows = manager.list()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!(
                    "{:<24} {:<10} {:>6} {:>8}  IMAGE",
                    "NAME", "STATE", "VCPUS", "MEM(MB)"
                );
                for row in rows {
                    println!(
                        "{:<24} {:<10} {:>6} {:>8}  {}",
                        row.name,
                        row.state,
                        row.vcpus,
                        row.memory_mb,
                        row.image.display()
                    );
                }
            }
        }
        VmAction::Status { name, json } => {
            let state = manager.state(&name)?;
            if json {
                println!("{}", json!({"name": name, "state": state}));
            } else {
                println!("{state}");
            }
        }
        VmAction::Shutdown { name } => {
            manager.shutdown(&name)?;
            println!("shutdown requested for {name}");
        }
        VmAction::Reboot { name } => {
            manager.reboot(&name)?;
            println!("reset requested for {name}");
        }
        VmAction::Pause { name } => {
            manager.pause(&name)?;
            println!("paused {name}");
        }
        VmAction::Resume { name } => {
            manager.resume(&name)?;
            println!("resumed {name}");
        }
        VmAction::Destroy { name } => {
            manager.destroy(&name)?;
            println!("destroyed {name}");
        }
        VmAction::Undefine { name, force } => {
            manager.undefine(&name, force)?;
            println!("undefined {name}");
        }
        VmAction::Show { name } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&manager.definition(&name)?)?
            );
        }
    }
    Ok(())
}

fn print_plan(plan: &VmPlan) {
    println!("GuestKit VM plan");
    println!("  ready:      {}", plan.ready);
    println!("  boot score: {:.0}", plan.boot_score);
    println!("  confidence: {:.0}%", plan.confidence * 100.0);
    for reason in &plan.reasons {
        println!("  BLOCK: {reason}");
    }
    for warning in &plan.warnings {
        println!("  WARN:  {warning}");
    }
    println!("  qemu: {}", plan.qemu_command);
}
