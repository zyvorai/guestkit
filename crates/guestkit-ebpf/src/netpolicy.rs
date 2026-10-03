//! Per-container network policy on the container's cgroup (cgroup_skb).
//! The cgroup id is the packet's socket cgroup (`bpf_skb_cgroup_id`), which
//! is right for ingress too (softirq runs under an unrelated task). A policed
//! direction allows loopback and peers matching an allow rule; anything else
//! is counted and reported, and dropped only while the policy is in enforce
//! mode and its lease deadline has not passed.

use aya_ebpf::{
    helpers::generated::{bpf_get_current_pid_tgid, bpf_skb_cgroup_id},
    macros::{cgroup_skb, map},
    maps::{HashMap, PerCpuHashMap},
    programs::SkBuffContext,
};
use guestkit_ebpf_common::*;

use crate::{now_ns, GK_EVENTS};

#[map]
pub static NP_POLICIES: HashMap<u64, NpPolicy> = HashMap::with_max_entries(1024, 0);
#[map]
pub static NP_RULES: HashMap<NpRuleKey, NpRule> = HashMap::with_max_entries(1024 * NP_MAX_RULES, 0);
#[map]
pub static NP_COUNTERS: PerCpuHashMap<u64, NpCounters> = PerCpuHashMap::with_max_entries(1024, 0);

const AF_INET: u32 = 2;
const AF_INET6: u32 = 10;
const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;
const IPPROTO_SCTP: u8 = 132;

struct Pkt {
    family: u8,
    proto: u8,
    port: u16,
    addr: [u32; 4],
}

const ALLOWED: u8 = 0;
const AUDITED: u8 = 1;
const DENIED: u8 = 2;

#[inline(always)]
fn count(cg: u64, which: u8) {
    if let Some(c) = NP_COUNTERS.get_ptr_mut(&cg) {
        unsafe {
            match which {
                ALLOWED => (*c).allowed += 1,
                AUDITED => (*c).audited += 1,
                _ => (*c).denied += 1,
            }
        }
        return;
    }
    let mut c = NpCounters::default();
    match which {
        ALLOWED => c.allowed = 1,
        AUDITED => c.audited = 1,
        _ => c.denied = 1,
    }
    let _ = NP_COUNTERS.insert(&cg, &c, 0);
}

#[inline(always)]
fn parse(ctx: &SkBuffContext, ingress: bool) -> Option<Pkt> {
    let family = unsafe { (*ctx.skb.skb).family };
    let mut p = Pkt { family: 0, proto: 0, port: 0, addr: [0; 4] };
    let l4 = if family == AF_INET {
        let vihl: u8 = ctx.load(0).ok()?;
        p.family = NP_FAMILY_V4;
        p.proto = ctx.load(9).ok()?;
        p.addr[0] = ctx.load(if ingress { 12 } else { 16 }).ok()?;
        ((vihl & 0x0f) as usize) * 4
    } else if family == AF_INET6 {
        p.family = NP_FAMILY_V6;
        p.proto = ctx.load(6).ok()?;
        let off = if ingress { 8 } else { 24 };
        p.addr[0] = ctx.load(off).ok()?;
        p.addr[1] = ctx.load(off + 4).ok()?;
        p.addr[2] = ctx.load(off + 8).ok()?;
        p.addr[3] = ctx.load(off + 12).ok()?;
        40
    } else {
        return None;
    };
    if matches!(p.proto, IPPROTO_TCP | IPPROTO_UDP | IPPROTO_SCTP) {
        let be: u16 = ctx.load(l4 + 2).unwrap_or(0);
        p.port = u16::from_be(be);
    }
    Some(p)
}

#[inline(always)]
fn loopback(p: &Pkt) -> bool {
    if p.family == NP_FAMILY_V4 {
        return p.addr[0] & u32::from_ne_bytes([0xff, 0, 0, 0]) == u32::from_ne_bytes([127, 0, 0, 0]);
    }
    p.addr[0] == 0 && p.addr[1] == 0 && p.addr[2] == 0 && p.addr[3] == u32::from_ne_bytes([0, 0, 0, 1])
}

#[inline(always)]
fn rule_matches(r: &NpRule, dir: u8, p: &Pkt) -> bool {
    r.dir == dir
        && r.family == p.family
        && (r.proto == 0 || r.proto == p.proto)
        && (r.port == 0 || r.port == p.port)
        && p.addr[0] & r.mask[0] == r.net[0]
        && p.addr[1] & r.mask[1] == r.net[1]
        && p.addr[2] & r.mask[2] == r.net[2]
        && p.addr[3] & r.mask[3] == r.net[3]
}

#[inline(never)]
fn report(cg: u64, dir: u8, p: &Pkt, denied: bool) {
    let Some(mut e) = GK_EVENTS.reserve::<GkEvent>(0) else { return };
    let ev = e.as_mut_ptr();
    unsafe {
        (*ev).ts_ns = now_ns();
        (*ev).cgroup_id = cg;
        (*ev).a = p.addr[0];
        (*ev).b = (p.addr[1] as u64) << 32 | p.addr[0] as u64;
        (*ev).pid = (bpf_get_current_pid_tgid() >> 32) as u32;
        (*ev).kind = EV_KIND_NET;
        (*ev).action = dir;
        (*ev).denied = denied as u8;
        (*ev).family = p.family;
        (*ev).port = p.port;
        (*ev).proto = p.proto;
        (*ev)._pad = 0;
        (*ev).comm = [0; 16];
    }
    e.submit(0);
}

#[inline(always)]
fn verdict(ctx: &SkBuffContext, dir: u8) -> i32 {
    let cg = unsafe { bpf_skb_cgroup_id(ctx.skb.skb) };
    let Some(pol) = (unsafe { NP_POLICIES.get(&cg) }) else { return 1 };
    let iso = if dir == NP_DIR_EGRESS { NP_EGRESS_ISOLATED } else { NP_INGRESS_ISOLATED };
    if pol.flags & iso == 0 {
        return 1;
    }
    let (flags, nrules, deadline) = (pol.flags, pol.nrules, pol.deadline_ns);
    let Some(p) = parse(ctx, dir == NP_DIR_INGRESS) else { return 1 };
    if loopback(&p) {
        return 1;
    }
    let mut slot = 0u32;
    while slot < NP_MAX_RULES && slot < nrules {
        let key = NpRuleKey { cgroup_id: cg, slot, _pad: 0 };
        if let Some(r) = unsafe { NP_RULES.get(&key) } {
            if rule_matches(r, dir, &p) {
                count(cg, ALLOWED);
                return 1;
            }
        }
        slot += 1;
    }
    let deny = flags & NP_ENFORCE != 0 && now_ns() < deadline;
    count(cg, if deny { DENIED } else { AUDITED });
    report(cg, dir, &p, deny);
    if deny { 0 } else { 1 }
}

#[cgroup_skb]
pub fn gk_np_egress(ctx: SkBuffContext) -> i32 {
    verdict(&ctx, NP_DIR_EGRESS)
}

#[cgroup_skb]
pub fn gk_np_ingress(ctx: SkBuffContext) -> i32 {
    verdict(&ctx, NP_DIR_INGRESS)
}
