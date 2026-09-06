// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Host-side AF_VSOCK listener — guests connect outward to the hypervisor.

use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};

pub use super::vsock::DEFAULT_VSOCK_PORT;

/// Listen on any guest CID (standard hypervisor bind).
pub const VMADDR_CID_ANY: u32 = 0xFFFF_FFFF;

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

/// Connected guest stream plus the guest's vsock CID.
pub struct VsockGuestStream {
    pub guest_cid: u32,
    io: VsockIo,
}

impl VsockGuestStream {
    pub fn read_write_pair(self) -> (Box<dyn Read + Send>, Box<dyn Write + Send>) {
        let fd = self.io.fd;
        let reader = Box::new(VsockIo { fd });
        let writer_fd = unsafe {
            let dup = libc::dup(fd);
            if dup < 0 {
                libc::close(fd);
                return (Box::new(VsockIo { fd: -1 }), Box::new(VsockIo { fd: -1 }));
            }
            dup
        };
        (reader, Box::new(VsockIo { fd: writer_fd }))
    }

    pub fn into_raw_fd(self) -> RawFd {
        let fd = self.io.fd;
        std::mem::forget(self);
        fd
    }
}

pub struct VsockListener {
    fd: RawFd,
}

impl Drop for VsockListener {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(target_os = "linux")]
fn bind_vsock(port: u32) -> Result<RawFd> {
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
        svm_cid: VMADDR_CID_ANY,
        svm_flags: 0,
        svm_zero: [0; 3],
    };

    let bind_ret = unsafe {
        libc::bind(
            fd,
            &addr as *const SockAddrVm as *const libc::sockaddr,
            mem::size_of::<SockAddrVm>() as libc::socklen_t,
        )
    };
    if bind_ret != 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        bail!("vsock bind port={port}: {err}");
    }

    if unsafe { libc::listen(fd, 8) } != 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        bail!("vsock listen port={port}: {err}");
    }

    Ok(fd)
}

#[cfg(not(target_os = "linux"))]
fn bind_vsock(_port: u32) -> Result<RawFd> {
    bail!("vsock host listener requires Linux (AF_VSOCK)")
}

pub fn listen(port: Option<u32>) -> Result<VsockListener> {
    let port = port.unwrap_or(DEFAULT_VSOCK_PORT);
    let fd = bind_vsock(port).with_context(|| format!("listen vsock://*:{port}"))?;
    Ok(VsockListener { fd })
}

impl VsockListener {
    pub fn accept(&self) -> Result<VsockGuestStream> {
        let client_fd =
            unsafe { libc::accept(self.fd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if client_fd < 0 {
            bail!("vsock accept: {}", std::io::Error::last_os_error());
        }

        let guest_cid = peer_cid(client_fd).unwrap_or(0);
        Ok(VsockGuestStream {
            guest_cid,
            io: VsockIo { fd: client_fd },
        })
    }

    pub fn accept_blocking_loop<F>(&self, mut on_connect: F) -> Result<()>
    where
        F: FnMut(VsockGuestStream) -> Result<()>,
    {
        loop {
            let stream = self.accept()?;
            on_connect(stream)?;
        }
    }
}

#[cfg(target_os = "linux")]
fn peer_cid(fd: RawFd) -> Result<u32> {
    use std::mem;

    #[repr(C)]
    struct SockAddrVm {
        svm_family: u16,
        svm_reserved1: u16,
        svm_port: u32,
        svm_cid: u32,
        svm_flags: u8,
        svm_zero: [u8; 3],
    }

    let mut addr = SockAddrVm {
        svm_family: 0,
        svm_reserved1: 0,
        svm_port: 0,
        svm_cid: 0,
        svm_flags: 0,
        svm_zero: [0; 3],
    };
    let mut len = mem::size_of::<SockAddrVm>() as libc::socklen_t;
    let ret = unsafe {
        libc::getpeername(
            fd,
            &mut addr as *mut SockAddrVm as *mut libc::sockaddr,
            &mut len,
        )
    };
    if ret != 0 {
        bail!("getpeername: {}", std::io::Error::last_os_error());
    }
    Ok(addr.svm_cid)
}

#[cfg(not(target_os = "linux"))]
fn peer_cid(_fd: RawFd) -> Result<u32> {
    Ok(0)
}

/// Framed RPC over a single accepted vsock connection (blocking).
pub fn serve_connection(fd: RawFd, handler: &crate::agent::handler::RequestHandler) -> Result<()> {
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut reader = file.try_clone()?;
    let mut writer = file;
    loop {
        let frame = guestkit_agent_protocol::read_frame(&mut reader)
            .map_err(|e| anyhow::anyhow!("read frame: {e}"))?;
        let response = handler.handle_frame(&frame);
        guestkit_agent_protocol::write_frame(&mut writer, &response)
            .map_err(|e| anyhow::anyhow!("write frame: {e}"))?;
    }
}
