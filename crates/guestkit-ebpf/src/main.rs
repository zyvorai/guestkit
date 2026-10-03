// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestKit in-guest eBPF programs (one object, loaded by guestkitd):
//! per-container network policy (cgroup_skb) and per-container MAC (BPF-LSM).
//! Both key every map by cgroup id, so one loaded instance serves every
//! container and cgroups without a policy entry are never touched.

#![no_std]
#![no_main]

mod lsm;
mod netpolicy;

use aya_ebpf::{helpers::generated::bpf_ktime_get_ns, macros::map, maps::RingBuf};

/// Violation events from both programs.
#[map]
pub static GK_EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

#[inline(always)]
pub fn now_ns() -> u64 {
    unsafe { bpf_ktime_get_ns() }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// GPL-compatible license is required for the LSM attach type and
// bpf_probe_read_kernel.
#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 4] = *b"GPL\0";
