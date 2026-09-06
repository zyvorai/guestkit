// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Agent I/O transports.

pub mod named_pipe;
pub mod stdio;
#[cfg(unix)]
pub mod unix_listen;
pub mod virtio;
#[cfg(not(target_os = "windows"))]
pub mod vsock;
#[cfg(not(target_os = "windows"))]
pub mod vsock_host;
pub mod zeus_push;

use anyhow::Result;
use guestkit_agent_protocol::{
    read_frame, read_line, write_frame, write_line, JsonRpcNotification,
};
use std::io::{BufReader, Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Virtio,
    Vsock,
    Stdio,
}

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub kind: ChannelKind,
    pub device_path: String,
    pub vsock_cid: Option<u32>,
    pub vsock_port: Option<u32>,
}

/// Bidirectional framed transport.
pub struct FramedTransport {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    line_framing: bool,
}

impl FramedTransport {
    /// Assemble a transport from raw halves. Used by loopback tests and
    /// embedders; production channels go through [`FramedTransport::open`].
    pub fn from_parts(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        line_framing: bool,
    ) -> Self {
        Self {
            reader,
            writer,
            line_framing,
        }
    }

    pub fn open(config: &TransportConfig) -> Result<Self> {
        let mut transport = match config.kind {
            ChannelKind::Virtio => virtio::open(&config.device_path)?,
            #[cfg(not(target_os = "windows"))]
            ChannelKind::Vsock => vsock::open(config.vsock_cid, config.vsock_port)?,
            #[cfg(target_os = "windows")]
            ChannelKind::Vsock => anyhow::bail!("AF_VSOCK transport is not available on Windows"),
            ChannelKind::Stdio => stdio::open()?,
        };
        transport.line_framing = config.kind == ChannelKind::Virtio;
        Ok(transport)
    }

    pub fn read_message(&mut self) -> Result<Vec<u8>> {
        if self.line_framing {
            let mut buf = BufReader::new(&mut self.reader);
            read_line(&mut buf).map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            read_frame(&mut self.reader).map_err(|e| anyhow::anyhow!("{e}"))
        }
    }

    pub fn write_message(&mut self, payload: &[u8], delimited: bool) -> Result<()> {
        if self.line_framing {
            if delimited {
                guestkit_agent_protocol::write_delimited_line(&mut self.writer, payload)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            } else {
                write_line(&mut self.writer, payload).map_err(|e| anyhow::anyhow!("{e}"))
            }
        } else if delimited {
            guestkit_agent_protocol::write_delimited_line(&mut self.writer, payload)
                .map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            write_frame(&mut self.writer, payload).map_err(|e| anyhow::anyhow!("{e}"))
        }
    }

    pub fn read_frame(&mut self) -> Result<Vec<u8>> {
        self.read_message()
    }

    pub fn write_frame(&mut self, payload: &[u8]) -> Result<()> {
        self.write_message(payload, false)
    }

    pub fn write_delimited_frame(&mut self, payload: &[u8]) -> Result<()> {
        self.write_message(payload, true)
    }

    /// Split into an owned reader half and a clonable, thread-safe writer
    /// half so a background task (heartbeat push) can share the write side
    /// with the request/response loop.
    pub fn split(self) -> (TransportReader, SharedWriter) {
        (
            TransportReader {
                reader: self.reader,
                line_framing: self.line_framing,
            },
            SharedWriter {
                inner: Arc::new(Mutex::new(self.writer)),
                line_framing: self.line_framing,
            },
        )
    }
}

/// Owned read half of a split [`FramedTransport`].
pub struct TransportReader {
    reader: Box<dyn Read + Send>,
    line_framing: bool,
}

impl TransportReader {
    pub fn read_message(&mut self) -> Result<Vec<u8>> {
        if self.line_framing {
            let mut buf = BufReader::new(&mut self.reader);
            read_line(&mut buf).map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            read_frame(&mut self.reader).map_err(|e| anyhow::anyhow!("{e}"))
        }
    }
}

/// Clonable, mutex-guarded write half of a split [`FramedTransport`].
/// Frames are written whole under the lock, so responses and pushed
/// notifications never interleave mid-frame.
#[derive(Clone)]
pub struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
    line_framing: bool,
}

impl SharedWriter {
    pub fn write_message(&self, payload: &[u8], delimited: bool) -> Result<()> {
        let mut writer = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if self.line_framing {
            if delimited {
                guestkit_agent_protocol::write_delimited_line(&mut *writer, payload)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            } else {
                write_line(&mut *writer, payload).map_err(|e| anyhow::anyhow!("{e}"))
            }
        } else if delimited {
            guestkit_agent_protocol::write_delimited_line(&mut *writer, payload)
                .map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            write_frame(&mut *writer, payload).map_err(|e| anyhow::anyhow!("{e}"))
        }
    }

    pub fn send_notification(&self, n: &JsonRpcNotification) -> Result<()> {
        let payload = serde_json::to_vec(n)?;
        self.write_message(&payload, false)
    }
}
