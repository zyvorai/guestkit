// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! QEMU/VirtIO runtime definitions backed by GuestKit migration evidence.
//!
//! The runtime is deliberately split into three layers:
//! - [`config`] is pure declarative QEMU configuration and safe argv generation.
//! - [`guestkit`] derives a QEMU plan from GuestKit evidence + boot assurance.
//! - [`qmp`] provides a small Unix QMP client for day-2 VM control.

mod config;
mod guestkit;
pub mod img;

#[cfg(unix)]
pub mod qmp;

pub use config::{
    Acceleration, Architecture, CacheMode, Console, CpuConfig, CpuModel, Disk, DiskFormat,
    DiskInterface, Firmware, ForwardProtocol, HostForward, MachineType, MemoryConfig,
    NetworkBackend, NetworkInterface, NetworkModel, QemuCommand, QemuError, QemuVm, QmpEndpoint,
    Result, VirtioDevice,
};
pub use guestkit::{GuestKitQemuOptions, GuestKitQemuPlan};
pub use img::{ImgCheckReport, QemuImg};
