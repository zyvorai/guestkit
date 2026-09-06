// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Length-prefixed frame encoding/decoding.

use crate::error::AgentError;
use std::io::{BufRead, Read, Write};

/// Maximum frame size (16 MiB) to prevent memory exhaustion.
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Read one length-prefixed frame from `reader`, skipping QGA 0xFF sync sentinel bytes.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, AgentError> {
    loop {
        let mut first = [0u8; 1];
        reader.read_exact(&mut first).map_err(AgentError::Io)?;
        if first[0] == 0xFF {
            continue;
        }
        let mut len_buf = [first[0], 0, 0, 0];
        reader
            .read_exact(&mut len_buf[1..])
            .map_err(AgentError::Io)?;
        let len = u32::from_be_bytes(len_buf);
        if len == 0 {
            return Err(AgentError::InvalidRequest("empty frame".into()));
        }
        if len > MAX_FRAME_SIZE {
            return Err(AgentError::InvalidRequest(format!(
                "frame too large: {len} bytes (max {MAX_FRAME_SIZE})"
            )));
        }
        let mut buf = vec![0u8; len as usize];
        reader.read_exact(&mut buf).map_err(AgentError::Io)?;
        return Ok(buf);
    }
}

/// Read one newline-terminated JSON line, skipping leading QGA 0xFF sync bytes.
pub fn read_line<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, AgentError> {
    loop {
        let mut line = Vec::new();
        reader
            .read_until(b'\n', &mut line)
            .map_err(AgentError::Io)?;
        if line.is_empty() {
            return Err(AgentError::Io(std::io::Error::from(
                std::io::ErrorKind::UnexpectedEof,
            )));
        }
        while line.first() == Some(&0xFF) {
            line.remove(0);
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if !line.is_empty() {
            return Ok(line);
        }
    }
}

/// Write one newline-terminated JSON line (QGA/libvirt virtio framing).
pub fn write_line<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), AgentError> {
    if payload.is_empty() {
        return Err(AgentError::InvalidRequest("empty payload".into()));
    }
    writer.write_all(payload).map_err(AgentError::Io)?;
    if !payload.ends_with(b"\n") {
        writer.write_all(b"\n").map_err(AgentError::Io)?;
    }
    writer.flush().map_err(AgentError::Io)?;
    Ok(())
}

/// Write a QGA `guest-sync-delimited` response line with a leading 0xFF sentinel.
pub fn write_delimited_line<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), AgentError> {
    writer.write_all(&[0xFF]).map_err(AgentError::Io)?;
    write_line(writer, payload)
}

/// Write one length-prefixed frame to `writer`.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), AgentError> {
    if payload.is_empty() {
        return Err(AgentError::InvalidRequest("empty payload".into()));
    }
    if payload.len() as u32 > MAX_FRAME_SIZE {
        return Err(AgentError::InvalidRequest(format!(
            "payload too large: {} bytes",
            payload.len()
        )));
    }
    let len = (payload.len() as u32).to_be_bytes();
    writer.write_all(&len).map_err(AgentError::Io)?;
    writer.write_all(payload).map_err(AgentError::Io)?;
    writer.flush().map_err(AgentError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_frame() {
        let payload = br#"{"jsonrpc":"2.0","method":"guestkit.ping"}"#;
        let mut buf = Vec::new();
        write_frame(&mut buf, payload).unwrap();
        let mut cursor = Cursor::new(buf);
        let decoded = read_frame(&mut cursor).unwrap();
        assert_eq!(decoded, payload);
    }
}
