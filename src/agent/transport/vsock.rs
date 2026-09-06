// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Linux AF_VSOCK client transport for the in-guest agent.

use super::FramedTransport;
use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

/// Default vsock port for GuestKit agent (host listens, guest connects).
pub const DEFAULT_VSOCK_PORT: u32 = 1234;

/// Host CID when guest connects outward (VM → hypervisor).
pub const VMADDR_CID_HOST: u32 = 2;

struct VsockIo {
    fd: RawFd,
}

impl Read for VsockIo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
}

impl Write for VsockIo {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = unsafe { libc::write(self.fd, buf.as_ptr() as *const _, buf.len()) };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for VsockIo {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(target_os = "linux")]
fn connect_vsock(cid: u32, port: u32) -> Result<RawFd> {
    use std::mem;

    const AF_VSOCK: i32 = 40;

    let fd = unsafe { libc::socket(AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        bail!("vsock socket: {}", std::io::Error::last_os_error());
    }

    #[repr(C)]
    struct SockAddrVm {
        svm_family: u16,
        svm_reserved1: u16,
        svm_port: u32,
        svm_cid: u32,
        svm_flags: u8,
        svm_zero: [u8; 3],
    }

    let addr = SockAddrVm {
        svm_family: AF_VSOCK as u16,
        svm_reserved1: 0,
        svm_port: port,
        svm_cid: cid,
        svm_flags: 0,
        svm_zero: [0; 3],
    };

    let ret = unsafe {
        libc::connect(
            fd,
            &addr as *const SockAddrVm as *const libc::sockaddr,
            mem::size_of::<SockAddrVm>() as libc::socklen_t,
        )
    };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        bail!("vsock connect cid={cid} port={port}: {err}");
    }
    Ok(fd)
}

#[cfg(not(target_os = "linux"))]
fn connect_vsock(_cid: u32, _port: u32) -> Result<RawFd> {
    bail!("vsock transport requires Linux (AF_VSOCK)")
}

pub fn open(cid: Option<u32>, port: Option<u32>) -> Result<FramedTransport> {
    let cid = cid.unwrap_or(VMADDR_CID_HOST);
    let port = port.unwrap_or(DEFAULT_VSOCK_PORT);
    let fd = connect_vsock(cid, port).with_context(|| format!("open vsock://{cid}:{port}"))?;
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let reader_fd = file.try_clone()?.into_raw_fd();
    let writer_fd = file.into_raw_fd();
    Ok(FramedTransport {
        reader: Box::new(VsockIo { fd: reader_fd }),
        writer: Box::new(VsockIo { fd: writer_fd }),
        line_framing: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        assert_eq!(DEFAULT_VSOCK_PORT, 1234);
        assert_eq!(VMADDR_CID_HOST, 2);
    }
}
