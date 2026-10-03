// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Loader for the GuestKit in-guest eBPF object: per-container network
//! policy (cgroup_skb) and per-container MAC (BPF-LSM). The rule compiler and
//! event decoder are portable; loading is Linux-only.

use guestkit_ebpf_common::*;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[cfg(target_os = "linux")]
mod btf;
#[cfg(target_os = "linux")]
mod loader;
#[cfg(target_os = "linux")]
pub use loader::*;

pub use guestkit_ebpf_common as common;

/// `addr/prefix` (a bare address is a host route).
pub fn parse_cidr(s: &str) -> Result<(IpAddr, u8), String> {
    let (a, p) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let addr: IpAddr = a.trim().parse().map_err(|_| format!("`{s}`: not an IP address"))?;
    let max = if addr.is_ipv4() { 32 } else { 128 };
    let prefix = match p {
        Some(p) => p.trim().parse::<u8>().map_err(|_| format!("`{s}`: bad prefix length"))?,
        None => max,
    };
    if prefix > max {
        return Err(format!("`{s}`: prefix longer than /{max}"));
    }
    Ok((addr, prefix))
}

/// `tcp` / `udp` / `sctp` / `icmp` / `any` or a protocol number.
pub fn parse_proto(s: &str) -> Result<u8, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "any" | "all" => 0,
        "tcp" => 6,
        "udp" => 17,
        "sctp" => 132,
        "icmp" => 1,
        "icmpv6" | "ipv6-icmp" => 58,
        n => n.parse().map_err(|_| format!("unknown protocol `{s}`"))?,
    })
}

pub fn parse_direction(s: &str) -> Result<u8, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "egress" | "out" => Ok(NP_DIR_EGRESS),
        "ingress" | "in" => Ok(NP_DIR_INGRESS),
        _ => Err(format!("direction must be egress or ingress (got `{s}`)")),
    }
}

fn mask_bytes<const N: usize>(prefix: u8) -> [u8; N] {
    let mut m = [0u8; N];
    for (i, b) in m.iter_mut().enumerate() {
        let bits = (prefix as usize).saturating_sub(i * 8).min(8);
        *b = if bits == 0 { 0 } else { 0xffu8 << (8 - bits) };
    }
    m
}

fn words(b: &[u8]) -> [u32; 4] {
    let mut w = [0u32; 4];
    for (i, c) in b.chunks(4).enumerate().take(4) {
        w[i] = u32::from_ne_bytes([c[0], c[1], c[2], c[3]]);
    }
    w
}

/// One allow rule in the program's layout. A port only makes sense for
/// TCP/UDP/SCTP (or "any" protocol, where it matches those).
pub fn compile_rule(dir: u8, addr: IpAddr, prefix: u8, proto: u8, port: u16) -> Result<NpRule, String> {
    if port != 0 && !matches!(proto, 0 | 6 | 17 | 132) {
        return Err(format!("port {port} needs tcp, udp, sctp or any protocol"));
    }
    let (family, net, mask) = match addr {
        IpAddr::V4(a) => {
            let m = mask_bytes::<4>(prefix.min(32));
            let n: Vec<u8> = a.octets().iter().zip(m).map(|(x, y)| x & y).collect();
            (NP_FAMILY_V4, words(&n), words(&m))
        }
        IpAddr::V6(a) => {
            let m = mask_bytes::<16>(prefix.min(128));
            let n: Vec<u8> = a.octets().iter().zip(m).map(|(x, y)| x & y).collect();
            (NP_FAMILY_V6, words(&n), words(&m))
        }
    };
    Ok(NpRule { dir, family, proto, _pad: 0, port, _pad2: 0, net, mask })
}

/// Policy as the program sees it. `deadline_ns` only matters when enforcing.
pub fn np_policy(egress: bool, ingress: bool, enforce: bool, nrules: u32, deadline_ns: u64) -> NpPolicy {
    let mut flags = 0;
    if egress {
        flags |= NP_EGRESS_ISOLATED;
    }
    if ingress {
        flags |= NP_INGRESS_ISOLATED;
    }
    if enforce {
        flags |= NP_ENFORCE;
    }
    NpPolicy { flags, nrules, deadline_ns: if enforce { deadline_ns } else { 0 } }
}

/// Kernel-internal dev_t (MKDEV: major << 20 | minor) from major/minor.
pub fn kdev(major: u32, minor: u32) -> u32 {
    (major << 20) | (minor & 0xfffff)
}

pub fn lsm_action_name(a: u8) -> &'static str {
    match a {
        LSM_ACT_EXEC => "exec",
        LSM_ACT_WX => "wx_mapping",
        LSM_ACT_DEVICE => "device_open",
        LSM_ACT_WRITE => "write_open",
        _ => "unknown",
    }
}

/// A decoded violation event.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Event {
    /// CLOCK_MONOTONIC ns.
    pub ts_ns: u64,
    pub cgroup_id: u64,
    pub kind: &'static str,
    pub action: &'static str,
    pub denied: bool,
    pub pid: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proto: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

pub fn decode_event(e: &GkEvent) -> Event {
    let comm_end = e.comm.iter().position(|c| *c == 0).unwrap_or(e.comm.len());
    let comm = String::from_utf8_lossy(&e.comm[..comm_end]).into_owned();
    let mut ev = Event {
        ts_ns: e.ts_ns,
        cgroup_id: e.cgroup_id,
        kind: "lsm",
        action: lsm_action_name(e.action),
        denied: e.denied != 0,
        pid: e.pid,
        comm,
        peer: None,
        proto: None,
        port: None,
        detail: String::new(),
    };
    if e.kind == EV_KIND_NET {
        ev.kind = "net";
        ev.action = if e.action == NP_DIR_INGRESS { "ingress" } else { "egress" };
        let b = e.b.to_ne_bytes();
        // Native-endian words of the address bytes: word 0 is in the low half.
        let (w0, w1) = (u32::from_ne_bytes([b[0], b[1], b[2], b[3]]), u32::from_ne_bytes([b[4], b[5], b[6], b[7]]));
        let (lo, hi) = if cfg!(target_endian = "little") { (w0, w1) } else { (w1, w0) };
        ev.peer = Some(if e.family == NP_FAMILY_V4 {
            Ipv4Addr::from(lo.to_ne_bytes()).to_string()
        } else {
            let mut o = [0u8; 16];
            o[..4].copy_from_slice(&lo.to_ne_bytes());
            o[4..8].copy_from_slice(&hi.to_ne_bytes());
            format!("{}/64", Ipv6Addr::from(o))
        });
        ev.proto = Some(e.proto);
        ev.port = (e.port != 0).then_some(e.port);
        return ev;
    }
    ev.detail = match e.action {
        LSM_ACT_EXEC | LSM_ACT_WRITE => format!("dev {}:{} ino {}", e.a >> 20, e.a & 0xfffff, e.b),
        LSM_ACT_WX => format!("prot {:#x} vm_flags {:#x}", e.a, e.b),
        LSM_ACT_DEVICE => {
            let kind = if e.b >> 62 == DEV_KIND_BLOCK { 'b' } else { 'c' };
            format!("{kind} {}:{}", e.a, e.b & 0xffff_ffff)
        }
        _ => String::new(),
    };
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ne(b: [u8; 4]) -> u32 {
        u32::from_ne_bytes(b)
    }

    #[test]
    fn cidr_parsing() {
        assert_eq!(parse_cidr("10.0.0.0/8").unwrap(), ("10.0.0.0".parse().unwrap(), 8));
        assert_eq!(parse_cidr("10.1.2.3").unwrap().1, 32);
        assert_eq!(parse_cidr("2001:db8::/32").unwrap().1, 32);
        assert_eq!(parse_cidr("::1").unwrap().1, 128);
        assert!(parse_cidr("10.0.0.0/33").is_err());
        assert!(parse_cidr("nope/8").is_err());
    }

    #[test]
    fn proto_and_direction() {
        assert_eq!(parse_proto("TCP").unwrap(), 6);
        assert_eq!(parse_proto("any").unwrap(), 0);
        assert_eq!(parse_proto("47").unwrap(), 47);
        assert!(parse_proto("bogus").is_err());
        assert_eq!(parse_direction("egress").unwrap(), NP_DIR_EGRESS);
        assert_eq!(parse_direction("in").unwrap(), NP_DIR_INGRESS);
        assert!(parse_direction("sideways").is_err());
    }

    #[test]
    fn v4_rule_masks_network_bytes() {
        let r = compile_rule(NP_DIR_EGRESS, "10.1.2.3".parse().unwrap(), 16, 6, 443).unwrap();
        assert_eq!(r.family, NP_FAMILY_V4);
        assert_eq!(r.net, [ne([10, 1, 0, 0]), 0, 0, 0]);
        assert_eq!(r.mask, [ne([255, 255, 0, 0]), 0, 0, 0]);
        assert_eq!((r.proto, r.port), (6, 443));
        // A packet word from 10.1.200.7 matches the way the program checks it.
        let pkt = ne([10, 1, 200, 7]);
        assert_eq!(pkt & r.mask[0], r.net[0]);
        assert_ne!(ne([10, 2, 0, 1]) & r.mask[0], r.net[0]);
    }

    #[test]
    fn v6_rule_partial_byte_prefix() {
        let r = compile_rule(NP_DIR_INGRESS, "2001:db8:abcd::1".parse().unwrap(), 36, 0, 0).unwrap();
        assert_eq!(r.family, NP_FAMILY_V6);
        assert_eq!(r.mask[0], ne([0xff, 0xff, 0xff, 0xff]));
        assert_eq!(r.mask[1], ne([0xf0, 0, 0, 0]));
        assert_eq!(r.mask[2], 0);
        assert_eq!(r.net[1], ne([0xa0, 0, 0, 0]));
    }

    #[test]
    fn port_needs_l4_protocol() {
        assert!(compile_rule(NP_DIR_EGRESS, "1.1.1.1".parse().unwrap(), 32, 1, 53).is_err());
        assert!(compile_rule(NP_DIR_EGRESS, "1.1.1.1".parse().unwrap(), 32, 0, 53).is_ok());
    }

    #[test]
    fn zero_prefix_matches_everything() {
        let r = compile_rule(NP_DIR_EGRESS, "0.0.0.0".parse().unwrap(), 0, 0, 0).unwrap();
        assert_eq!(r.mask, [0; 4]);
        assert_eq!(r.net, [0; 4]);
    }

    #[test]
    fn policy_flags_and_lease() {
        let p = np_policy(true, false, false, 3, 99);
        assert_eq!(p.flags, NP_EGRESS_ISOLATED);
        assert_eq!(p.deadline_ns, 0, "audit mode carries no deadline");
        let p = np_policy(true, true, true, 0, 99);
        assert_eq!(p.flags, NP_EGRESS_ISOLATED | NP_INGRESS_ISOLATED | NP_ENFORCE);
        assert_eq!(p.deadline_ns, 99);
    }

    #[test]
    fn decodes_events() {
        let mut raw = GkEvent {
            ts_ns: 5,
            cgroup_id: 77,
            b: 0,
            a: 0,
            pid: 10,
            kind: EV_KIND_NET,
            action: NP_DIR_EGRESS,
            denied: 1,
            family: NP_FAMILY_V4,
            port: 443,
            proto: 6,
            _pad: 0,
            comm: [0; 16],
        };
        let w = ne([93, 184, 216, 34]);
        raw.a = w;
        raw.b = w as u64;
        let e = decode_event(&raw);
        assert_eq!((e.kind, e.action, e.denied), ("net", "egress", true));
        assert_eq!(e.peer.as_deref(), Some("93.184.216.34"));
        assert_eq!(e.port, Some(443));

        raw.kind = EV_KIND_LSM;
        raw.action = LSM_ACT_EXEC;
        raw.a = kdev(8, 1);
        raw.b = 1234;
        raw.comm[..4].copy_from_slice(b"bash");
        let e = decode_event(&raw);
        assert_eq!((e.kind, e.action), ("lsm", "exec"));
        assert_eq!(e.comm, "bash");
        assert_eq!(e.detail, "dev 8:1 ino 1234");
    }
}
