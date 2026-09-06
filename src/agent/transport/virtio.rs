// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Virtio-serial character device transport.

use super::FramedTransport;
use anyhow::{Context, Result};
#[cfg(not(target_os = "windows"))]
use guestkit_agent_protocol::VIRTIO_DEVICE_PATH;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

/// Default virtio-serial port for the QGA channel. On Linux this is the
/// `/dev/virtio-ports/...` char device; on Windows the virtio-serial driver
/// (virtio-win) exposes the same port under the `\\.\Global\` namespace.
#[cfg(not(target_os = "windows"))]
pub const DEFAULT_DEVICE: &str = VIRTIO_DEVICE_PATH;
#[cfg(target_os = "windows")]
pub const DEFAULT_DEVICE: &str = r"\\.\Global\org.qemu.guest_agent.0";

struct DeviceIo {
    inner: std::fs::File,
}

impl Read for DeviceIo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for DeviceIo {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub fn open(device_path: &str) -> Result<FramedTransport> {
    let path = if device_path.is_empty() {
        DEFAULT_DEVICE
    } else {
        device_path
    };
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new(path))
        .with_context(|| format!("failed to open virtio channel {path}"))?;
    Ok(FramedTransport {
        reader: Box::new(DeviceIo {
            inner: file.try_clone()?,
        }),
        writer: Box::new(DeviceIo { inner: file }),
        line_framing: false,
    })
}
