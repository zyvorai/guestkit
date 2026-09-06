// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Declarative QEMU configuration and argument generation.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QemuError {
    #[error("invalid QEMU VM configuration: {0}")]
    InvalidConfig(String),

    #[error("unsupported guest architecture: {0}")]
    UnsupportedArchitecture(String),

    #[error("unable to infer QEMU disk format from {0}; specify the format explicitly")]
    UnknownDiskFormat(String),

    #[error("QEMU process failed to start: {0}")]
    Spawn(#[source] std::io::Error),
}

pub type Result<T> = std::result::Result<T, QemuError>;

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
            other => Err(QemuError::UnsupportedArchitecture(other.to_string())),
        }
    }

    pub const fn default_machine(self) -> MachineType {
        match self {
            Self::X86_64 => MachineType::Q35,
            Self::Aarch64 => MachineType::Virt,
        }
    }

    pub const fn qemu_binary(self) -> &'static str {
        match self {
            Self::X86_64 => "qemu-system-x86_64",
            Self::Aarch64 => "qemu-system-aarch64",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineType {
    Q35,
    Pc,
    Virt,
}

impl fmt::Display for MachineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Q35 => f.write_str("q35"),
            Self::Pc => f.write_str("pc"),
            Self::Virt => f.write_str("virt"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Acceleration {
    Kvm,
    Tcg,
}

impl fmt::Display for Acceleration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kvm => f.write_str("kvm"),
            Self::Tcg => f.write_str("tcg"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuConfig {
    pub sockets: u16,
    pub cores: u16,
    pub threads: u16,
    pub model: CpuModel,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            sockets: 1,
            cores: 2,
            threads: 1,
            model: CpuModel::Host,
        }
    }
}

impl CpuConfig {
    pub fn vcpus(&self) -> u32 {
        u32::from(self.sockets) * u32::from(self.cores) * u32::from(self.threads)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuModel {
    Host,
    Max,
    Qemu64,
    Custom(String),
}

impl fmt::Display for CpuModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host => f.write_str("host"),
            Self::Max => f.write_str("max"),
            Self::Qemu64 => f.write_str("qemu64"),
            Self::Custom(value) => f.write_str(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub size_mb: u64,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    pub prealloc: bool,
    #[serde(default = "default_hugepage_path")]
    pub hugepage_path: PathBuf,
}

fn default_hugepage_path() -> PathBuf {
    PathBuf::from("/dev/hugepages")
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            size_mb: 4096,
            hugepages: false,
            prealloc: false,
            hugepage_path: default_hugepage_path(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskFormat {
    Raw,
    Qcow2,
    Vmdk,
    Vdi,
    Vhd,
}

impl DiskFormat {
    pub fn infer(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        match ext.as_str() {
            "raw" | "img" => Ok(Self::Raw),
            "qcow" | "qcow2" => Ok(Self::Qcow2),
            "vmdk" => Ok(Self::Vmdk),
            "vdi" => Ok(Self::Vdi),
            "vhd" | "vpc" => Ok(Self::Vhd),
            _ => Err(QemuError::UnknownDiskFormat(path.display().to_string())),
        }
    }

    pub const fn qemu_name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Qcow2 => "qcow2",
            Self::Vmdk => "vmdk",
            Self::Vdi => "vdi",
            Self::Vhd => "vpc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskInterface {
    VirtioBlk,
    VirtioScsi,
    Nvme,
    Sata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    None,
    Writeback,
    Writethrough,
    Directsync,
    Unsafe,
}

impl fmt::Display for CacheMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::Writeback => f.write_str("writeback"),
            Self::Writethrough => f.write_str("writethrough"),
            Self::Directsync => f.write_str("directsync"),
            Self::Unsafe => f.write_str("unsafe"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    pub id: String,
    pub path: PathBuf,
    pub format: DiskFormat,
    pub interface: DiskInterface,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default = "default_cache_mode")]
    pub cache: CacheMode,
    #[serde(default)]
    pub discard: bool,
}

fn default_cache_mode() -> CacheMode {
    CacheMode::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkModel {
    VirtioNet,
    E1000e,
    Vmxnet3,
}

impl NetworkModel {
    const fn qemu_name(self, architecture: Architecture) -> &'static str {
        match (self, architecture) {
            (Self::VirtioNet, Architecture::Aarch64) => "virtio-net-device",
            (Self::VirtioNet, Architecture::X86_64) => "virtio-net-pci",
            (Self::E1000e, _) => "e1000e",
            (Self::Vmxnet3, _) => "vmxnet3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardProtocol {
    Tcp,
    Udp,
}

impl fmt::Display for ForwardProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp => f.write_str("tcp"),
            Self::Udp => f.write_str("udp"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostForward {
    pub protocol: ForwardProtocol,
    #[serde(default)]
    pub host_addr: Option<String>,
    pub host_port: u16,
    #[serde(default)]
    pub guest_addr: Option<String>,
    pub guest_port: u16,
}

impl HostForward {
    fn qemu_value(&self) -> String {
        format!(
            "hostfwd={}:{}:{}-{}:{}",
            self.protocol,
            self.host_addr.as_deref().unwrap_or_default(),
            self.host_port,
            self.guest_addr.as_deref().unwrap_or_default(),
            self.guest_port
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkBackend {
    User {
        #[serde(default)]
        forwards: Vec<HostForward>,
    },
    Tap {
        interface: String,
        #[serde(default)]
        vhost: bool,
    },
    Bridge {
        bridge: String,
    },
}

impl Default for NetworkBackend {
    fn default() -> Self {
        Self::User { forwards: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub id: String,
    pub backend: NetworkBackend,
    #[serde(default = "default_network_model")]
    pub model: NetworkModel,
    #[serde(default)]
    pub mac: Option<String>,
}

fn default_network_model() -> NetworkModel {
    NetworkModel::VirtioNet
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VirtioDevice {
    Balloon {
        id: String,
    },
    Rng {
        id: String,
        source: PathBuf,
    },
    Vsock {
        id: String,
        cid: u32,
    },
    Gpu {
        id: String,
    },
    SerialPort {
        id: String,
        name: String,
        socket: PathBuf,
    },
}

impl VirtioDevice {
    fn id(&self) -> &str {
        match self {
            Self::Balloon { id }
            | Self::Rng { id, .. }
            | Self::Vsock { id, .. }
            | Self::Gpu { id }
            | Self::SerialPort { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Firmware {
    pub code: PathBuf,
    #[serde(default)]
    pub vars: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Console {
    None,
    #[default]
    Serial,
    Vnc {
        display: u16,
    },
    Spice {
        port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QmpEndpoint {
    pub socket: PathBuf,
    #[serde(default = "default_true")]
    pub server: bool,
    #[serde(default)]
    pub wait: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuVm {
    pub name: String,
    pub architecture: Architecture,
    pub machine: MachineType,
    pub acceleration: Acceleration,
    pub cpu: CpuConfig,
    pub memory: MemoryConfig,
    #[serde(default)]
    pub disks: Vec<Disk>,
    #[serde(default)]
    pub networks: Vec<NetworkInterface>,
    #[serde(default)]
    pub devices: Vec<VirtioDevice>,
    #[serde(default)]
    pub firmware: Option<Firmware>,
    #[serde(default)]
    pub console: Console,
    #[serde(default)]
    pub qmp: Option<QmpEndpoint>,
    #[serde(default)]
    pub pidfile: Option<PathBuf>,
    #[serde(default)]
    pub daemonize: bool,
    #[serde(default)]
    pub binary_override: Option<PathBuf>,
}

impl QemuVm {
    pub fn new(name: impl Into<String>, architecture: Architecture) -> Self {
        let machine = architecture.default_machine();
        Self {
            name: name.into(),
            architecture,
            machine,
            acceleration: Acceleration::Kvm,
            cpu: CpuConfig::default(),
            memory: MemoryConfig::default(),
            disks: vec![],
            networks: vec![],
            devices: vec![],
            firmware: None,
            console: Console::default(),
            qmp: None,
            pidfile: None,
            daemonize: false,
            binary_override: None,
        }
    }

    pub fn disk(mut self, disk: Disk) -> Self {
        self.disks.push(disk);
        self
    }

    pub fn network(mut self, network: NetworkInterface) -> Self {
        self.networks.push(network);
        self
    }

    pub fn device(mut self, device: VirtioDevice) -> Self {
        self.devices.push(device);
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(QemuError::InvalidConfig("VM name cannot be empty".into()));
        }
        if self.cpu.sockets == 0 || self.cpu.cores == 0 || self.cpu.threads == 0 {
            return Err(QemuError::InvalidConfig(
                "CPU sockets, cores, and threads must all be greater than zero".into(),
            ));
        }
        if self.memory.size_mb < 128 {
            return Err(QemuError::InvalidConfig(
                "memory must be at least 128 MiB".into(),
            ));
        }
        if self.acceleration == Acceleration::Tcg && self.cpu.model == CpuModel::Host {
            return Err(QemuError::InvalidConfig(
                "CPU model 'host' requires hardware acceleration; use max or another emulated CPU with TCG".into(),
            ));
        }
        match (self.architecture, self.machine) {
            (Architecture::X86_64, MachineType::Virt) => {
                return Err(QemuError::InvalidConfig(
                    "the virt machine is intended for aarch64; use q35 or pc on x86_64".into(),
                ));
            }
            (Architecture::Aarch64, MachineType::Q35 | MachineType::Pc) => {
                return Err(QemuError::InvalidConfig(
                    "q35/pc are x86 machine types; use virt on aarch64".into(),
                ));
            }
            _ => {}
        }

        let mut ids = HashSet::new();
        for disk in &self.disks {
            validate_id(&disk.id, "disk")?;
            if !ids.insert(disk.id.as_str()) {
                return Err(QemuError::InvalidConfig(format!(
                    "duplicate QEMU object id: {}",
                    disk.id
                )));
            }
            if disk.path.as_os_str().is_empty() {
                return Err(QemuError::InvalidConfig(format!(
                    "disk {} has an empty path",
                    disk.id
                )));
            }
        }
        for network in &self.networks {
            validate_id(&network.id, "network")?;
            if !ids.insert(network.id.as_str()) {
                return Err(QemuError::InvalidConfig(format!(
                    "duplicate QEMU object id: {}",
                    network.id
                )));
            }
            if let Some(mac) = &network.mac {
                validate_mac(mac)?;
            }
            if self.architecture == Architecture::Aarch64
                && network.model != NetworkModel::VirtioNet
            {
                return Err(QemuError::InvalidConfig(
                    "aarch64 plans currently support virtio-net only".into(),
                ));
            }
            match &network.backend {
                NetworkBackend::User { forwards } => {
                    for forward in forwards {
                        if forward.host_port == 0 || forward.guest_port == 0 {
                            return Err(QemuError::InvalidConfig(
                                "host-forward ports must be between 1 and 65535".into(),
                            ));
                        }
                    }
                }
                NetworkBackend::Tap { interface, .. } if interface.trim().is_empty() => {
                    return Err(QemuError::InvalidConfig(
                        "tap interface cannot be empty".into(),
                    ));
                }
                NetworkBackend::Bridge { bridge } if bridge.trim().is_empty() => {
                    return Err(QemuError::InvalidConfig(
                        "bridge name cannot be empty".into(),
                    ));
                }
                _ => {}
            }
        }
        for device in &self.devices {
            validate_id(device.id(), "device")?;
            if !ids.insert(device.id()) {
                return Err(QemuError::InvalidConfig(format!(
                    "duplicate QEMU object id: {}",
                    device.id()
                )));
            }
            if let VirtioDevice::Vsock { cid, .. } = device {
                if *cid < 3 {
                    return Err(QemuError::InvalidConfig(
                        "vsock guest CID must be >= 3 (0, 1, and 2 are reserved)".into(),
                    ));
                }
            }
        }
        if let Console::Spice { port } = &self.console {
            if *port == 0 {
                return Err(QemuError::InvalidConfig(
                    "SPICE port must be between 1 and 65535".into(),
                ));
            }
        }
        if self.daemonize && matches!(&self.console, Console::Serial) {
            return Err(QemuError::InvalidConfig(
                "daemonized QEMU cannot use the foreground serial console; choose none, VNC, or SPICE"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn command_spec(&self) -> Result<QemuCommand> {
        self.validate()?;
        let program = self
            .binary_override
            .clone()
            .unwrap_or_else(|| PathBuf::from(self.architecture.qemu_binary()));
        let mut args = Vec::<OsString>::new();

        push_pair(&mut args, "-name", OsString::from(self.name.as_str()));
        push_pair(
            &mut args,
            "-machine",
            OsString::from(format!("{},accel={}", self.machine, self.acceleration)),
        );
        push_pair(
            &mut args,
            "-cpu",
            OsString::from(self.cpu.model.to_string()),
        );
        push_pair(
            &mut args,
            "-smp",
            OsString::from(format!(
                "cpus={},sockets={},cores={},threads={}",
                self.cpu.vcpus(),
                self.cpu.sockets,
                self.cpu.cores,
                self.cpu.threads
            )),
        );
        push_pair(
            &mut args,
            "-m",
            OsString::from(self.memory.size_mb.to_string()),
        );

        if self.memory.hugepages {
            push_pair(
                &mut args,
                "-mem-path",
                self.memory.hugepage_path.as_os_str().to_owned(),
            );
        }
        if self.memory.prealloc {
            args.push(OsString::from("-mem-prealloc"));
        }

        if let Some(firmware) = &self.firmware {
            let mut code = OsString::from("if=pflash,format=raw,readonly=on,file=");
            code.push(firmware.code.as_os_str());
            push_pair(&mut args, "-drive", code);
            if let Some(vars) = &firmware.vars {
                let mut value = OsString::from("if=pflash,format=raw,file=");
                value.push(vars.as_os_str());
                push_pair(&mut args, "-drive", value);
            }
        }

        let needs_scsi = self
            .disks
            .iter()
            .any(|disk| disk.interface == DiskInterface::VirtioScsi);
        if needs_scsi {
            push_pair(
                &mut args,
                "-device",
                OsString::from(match self.architecture {
                    Architecture::X86_64 => "virtio-scsi-pci,id=scsi0",
                    Architecture::Aarch64 => "virtio-scsi-device,id=scsi0",
                }),
            );
        }

        for (index, disk) in self.disks.iter().enumerate() {
            let mut drive = OsString::from(format!(
                "id={},format={},if=none,cache={}",
                disk.id,
                disk.format.qemu_name(),
                disk.cache
            ));
            if disk.readonly {
                drive.push(",readonly=on");
            }
            if disk.discard {
                drive.push(",discard=unmap");
            }
            drive.push(",file=");
            drive.push(disk.path.as_os_str());
            push_pair(&mut args, "-drive", drive);

            match disk.interface {
                DiskInterface::VirtioBlk => {
                    let device_name = match self.architecture {
                        Architecture::X86_64 => "virtio-blk-pci",
                        Architecture::Aarch64 => "virtio-blk-device",
                    };
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("{device_name},drive={},id={}", disk.id, disk.id)),
                    );
                }
                DiskInterface::VirtioScsi => {
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!(
                            "scsi-hd,drive={},bus=scsi0.0,id={}",
                            disk.id, disk.id
                        )),
                    );
                }
                DiskInterface::Nvme => {
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!(
                            "nvme,drive={},serial=guestkit-nvme-{index},id={}",
                            disk.id, disk.id
                        )),
                    );
                }
                DiskInterface::Sata => {
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("ide-hd,drive={},id={}", disk.id, disk.id)),
                    );
                }
            }
        }

        for network in &self.networks {
            let netdev = match &network.backend {
                NetworkBackend::User { forwards } => {
                    let mut value = format!("user,id={}", network.id);
                    for forward in forwards {
                        value.push(',');
                        value.push_str(&forward.qemu_value());
                    }
                    value
                }
                NetworkBackend::Tap { interface, vhost } => format!(
                    "tap,id={},ifname={},script=no,downscript=no,vhost={}",
                    network.id,
                    interface,
                    if *vhost { "on" } else { "off" }
                ),
                NetworkBackend::Bridge { bridge } => {
                    format!("bridge,id={},br={}", network.id, bridge)
                }
            };
            push_pair(&mut args, "-netdev", OsString::from(netdev));

            let mut device = format!(
                "{},netdev={},id={}",
                network.model.qemu_name(self.architecture),
                network.id,
                network.id
            );
            if let Some(mac) = &network.mac {
                device.push_str(",mac=");
                device.push_str(mac);
            }
            push_pair(&mut args, "-device", OsString::from(device));
        }

        let has_serial_port = self
            .devices
            .iter()
            .any(|device| matches!(device, VirtioDevice::SerialPort { .. }));
        if has_serial_port {
            push_pair(
                &mut args,
                "-device",
                OsString::from(match self.architecture {
                    Architecture::X86_64 => "virtio-serial-pci,id=virtio-serial0",
                    Architecture::Aarch64 => "virtio-serial-device,id=virtio-serial0",
                }),
            );
        }

        for device in &self.devices {
            match device {
                VirtioDevice::Balloon { id } => {
                    let model = match self.architecture {
                        Architecture::X86_64 => "virtio-balloon-pci",
                        Architecture::Aarch64 => "virtio-balloon-device",
                    };
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("{model},id={id}")),
                    );
                }
                VirtioDevice::Rng { id, source } => {
                    let object_id = format!("{id}-backend");
                    let mut object = OsString::from(format!("rng-random,id={object_id},filename="));
                    object.push(source.as_os_str());
                    push_pair(&mut args, "-object", object);
                    let model = match self.architecture {
                        Architecture::X86_64 => "virtio-rng-pci",
                        Architecture::Aarch64 => "virtio-rng-device",
                    };
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("{model},rng={object_id},id={id}")),
                    );
                }
                VirtioDevice::Vsock { id, cid } => {
                    let model = match self.architecture {
                        Architecture::X86_64 => "vhost-vsock-pci",
                        Architecture::Aarch64 => "vhost-vsock-device",
                    };
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("{model},id={id},guest-cid={cid}")),
                    );
                }
                VirtioDevice::Gpu { id } => {
                    let model = match self.architecture {
                        Architecture::X86_64 => "virtio-gpu-pci",
                        Architecture::Aarch64 => "virtio-gpu-device",
                    };
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!("{model},id={id}")),
                    );
                }
                VirtioDevice::SerialPort { id, name, socket } => {
                    let mut chardev = OsString::from(format!("socket,id={id},path="));
                    chardev.push(socket.as_os_str());
                    chardev.push(",server=on,wait=off");
                    push_pair(&mut args, "-chardev", chardev);
                    push_pair(
                        &mut args,
                        "-device",
                        OsString::from(format!(
                            "virtserialport,bus=virtio-serial0.0,chardev={id},name={name},id={id}"
                        )),
                    );
                }
            }
        }

        match &self.console {
            Console::None => {
                push_pair(&mut args, "-display", OsString::from("none"));
            }
            Console::Serial => {
                args.push(OsString::from("-nographic"));
            }
            Console::Vnc { display } => {
                push_pair(&mut args, "-vnc", OsString::from(format!(":{display}")));
            }
            Console::Spice { port } => {
                push_pair(
                    &mut args,
                    "-spice",
                    OsString::from(format!("port={port},disable-ticketing=on")),
                );
            }
        }

        if let Some(qmp) = &self.qmp {
            let mut value = OsString::from("unix:");
            value.push(qmp.socket.as_os_str());
            let suffix = format!(
                ",server={},wait={}",
                if qmp.server { "on" } else { "off" },
                if qmp.wait { "on" } else { "off" }
            );
            value.push(OsStr::new(&suffix));
            push_pair(&mut args, "-qmp", value);
        }

        if let Some(pidfile) = &self.pidfile {
            push_pair(&mut args, "-pidfile", pidfile.as_os_str().to_owned());
        }
        if self.daemonize {
            args.push(OsString::from("-daemonize"));
        }

        Ok(QemuCommand { program, args })
    }

    pub fn spawn(&self) -> Result<Child> {
        let spec = self.command_spec()?;
        let mut command = spec.to_command();
        if matches!(&self.console, Console::Serial) && !self.daemonize {
            command.stdin(Stdio::inherit());
        } else {
            command.stdin(Stdio::null());
        }
        command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(QemuError::Spawn)
    }
}

fn validate_id(id: &str, kind: &str) -> Result<()> {
    if id.is_empty() {
        return Err(QemuError::InvalidConfig(format!(
            "{kind} id cannot be empty"
        )));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(QemuError::InvalidConfig(format!(
            "{kind} id {id:?} contains unsupported characters"
        )));
    }
    Ok(())
}

fn validate_mac(mac: &str) -> Result<()> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6
        || parts
            .iter()
            .any(|part| part.len() != 2 || u8::from_str_radix(part, 16).is_err())
    {
        return Err(QemuError::InvalidConfig(format!(
            "invalid MAC address: {mac}"
        )));
    }
    Ok(())
}

fn push_pair(args: &mut Vec<OsString>, flag: &str, value: OsString) {
    args.push(OsString::from(flag));
    args.push(value);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl QemuCommand {
    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command
    }

    /// Human-readable shell representation for logs/copy-paste only.
    /// Execution always uses `Command` directly; no shell is involved.
    pub fn render_shell(&self) -> String {
        std::iter::once(self.program.as_os_str())
            .chain(self.args.iter().map(OsString::as_os_str))
            .map(shell_quote)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn shell_quote(value: &OsStr) -> String {
    let text = value.to_string_lossy();
    if !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&b))
    {
        return text.into_owned();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(path: &str) -> Disk {
        Disk {
            id: "root".into(),
            path: path.into(),
            format: DiskFormat::Qcow2,
            interface: DiskInterface::VirtioBlk,
            readonly: false,
            cache: CacheMode::None,
            discard: true,
        }
    }

    #[test]
    fn rejects_zero_cpu_topology() {
        let mut vm = QemuVm::new("bad", Architecture::X86_64);
        vm.cpu.cores = 0;
        assert!(matches!(vm.validate(), Err(QemuError::InvalidConfig(_))));
    }

    #[test]
    fn path_with_spaces_remains_one_argument() {
        let vm = QemuVm::new("demo", Architecture::X86_64).disk(disk("/vm images/root disk.qcow2"));
        let command = vm.command_spec().unwrap();
        let drive = command
            .args
            .iter()
            .find(|arg| arg.to_string_lossy().contains("root disk.qcow2"))
            .unwrap();
        assert!(drive
            .to_string_lossy()
            .contains("file=/vm images/root disk.qcow2"));
        assert!(command.render_shell().contains("'id=root,format=qcow2"));
    }

    #[test]
    fn creates_one_scsi_controller_for_multiple_disks() {
        let mk = |id: &str| Disk {
            id: id.into(),
            path: format!("/{id}.qcow2").into(),
            format: DiskFormat::Qcow2,
            interface: DiskInterface::VirtioScsi,
            readonly: false,
            cache: CacheMode::None,
            discard: false,
        };
        let mut vm = QemuVm::new("scsi", Architecture::X86_64);
        vm.disks = vec![mk("root"), mk("data")];
        let command = vm.command_spec().unwrap();
        let controllers = command
            .args
            .iter()
            .filter(|arg| arg.to_string_lossy().contains("virtio-scsi-pci,id=scsi0"))
            .count();
        assert_eq!(controllers, 1);
    }

    #[test]
    fn renders_user_network_forward() {
        let mut vm = QemuVm::new("net", Architecture::X86_64);
        vm.networks.push(NetworkInterface {
            id: "net0".into(),
            backend: NetworkBackend::User {
                forwards: vec![HostForward {
                    protocol: ForwardProtocol::Tcp,
                    host_addr: Some("127.0.0.1".into()),
                    host_port: 2222,
                    guest_addr: None,
                    guest_port: 22,
                }],
            },
            model: NetworkModel::VirtioNet,
            mac: Some("52:54:00:12:34:56".into()),
        });
        let command = vm.command_spec().unwrap();
        assert!(command.args.iter().any(|arg| {
            arg.to_string_lossy()
                .contains("hostfwd=tcp:127.0.0.1:2222-:22")
        }));
    }

    #[test]
    fn infers_common_disk_formats() {
        assert_eq!(
            DiskFormat::infer(Path::new("vm.qcow2")).unwrap(),
            DiskFormat::Qcow2
        );
        assert_eq!(
            DiskFormat::infer(Path::new("vm.vhd")).unwrap(),
            DiskFormat::Vhd
        );
        assert!(DiskFormat::infer(Path::new("vm.unknown")).is_err());
    }
}
