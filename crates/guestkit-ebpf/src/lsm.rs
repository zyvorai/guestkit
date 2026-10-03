//! Per-container MAC on BPF-LSM hooks, keyed by the calling task's cgroup id
//! (LSM_POLICIES): exec outside the container's binary allowlist, W+X
//! mappings, device opens outside the device allowlist and write-opens of
//! regular files on filesystems not marked writable. Violations are counted
//! and reported, and denied only in enforce mode before the lease deadline.

use aya_ebpf::{
    helpers::generated::{bpf_get_current_cgroup_id, bpf_get_current_comm, bpf_get_current_pid_tgid, bpf_probe_read_kernel},
    macros::{lsm, map},
    maps::{Array, HashMap, PerCpuHashMap},
    programs::LsmContext,
};
use guestkit_ebpf_common::*;

use crate::{now_ns, GK_EVENTS};

#[map]
pub static LSM_CFG: Array<LsmCfg> = Array::with_max_entries(1, 0);
#[map]
pub static LSM_POLICIES: HashMap<u64, LsmPolicy> = HashMap::with_max_entries(1024, 0);
#[map]
pub static LSM_FILES: HashMap<LsmFileKey, u8> = HashMap::with_max_entries(16384, 0);
#[map]
pub static LSM_DEVICES: HashMap<LsmDevKey, u8> = HashMap::with_max_entries(16384, 0);
#[map]
pub static LSM_WRITABLE: HashMap<LsmFsKey, u8> = HashMap::with_max_entries(4096, 0);
#[map]
pub static LSM_COUNTERS: PerCpuHashMap<LsmCounterKey, u64> = PerCpuHashMap::with_max_entries(4096, 0);

const EPERM: i32 = 1;
const PROT_WRITE: u64 = 2;
const PROT_EXEC: u64 = 4;
const VM_WRITE: u64 = 2;
const FMODE_WRITE: u32 = 2;
const S_IFMT: u16 = 0o170000;
const S_IFREG: u16 = 0o100000;
const S_IFCHR: u16 = 0o020000;
const S_IFBLK: u16 = 0o060000;

#[inline(always)]
fn rd<T: Copy + Default>(p: u64, off: u32) -> Option<T> {
    let mut v = T::default();
    let r = unsafe {
        bpf_probe_read_kernel((&mut v as *mut T).cast(), core::mem::size_of::<T>() as u32, (p + off as u64) as *const _)
    };
    (r == 0).then_some(v)
}

/// (cgroup id, policy, cfg) when the current task's cgroup has `flag` set.
#[inline(always)]
fn scope(flag: u32) -> Option<(u64, &'static LsmPolicy, &'static LsmCfg)> {
    let c = LSM_CFG.get(0)?;
    if c.ready == 0 {
        return None;
    }
    let cg = unsafe { bpf_get_current_cgroup_id() };
    let p = unsafe { LSM_POLICIES.get(&cg) }?;
    (p.flags & flag != 0).then_some((cg, p, c))
}

#[inline(always)]
fn count(cg: u64, action: u8, denied: bool) {
    let key = LsmCounterKey { cgroup_id: cg, action: action as u32, denied: denied as u32 };
    if let Some(v) = LSM_COUNTERS.get_ptr_mut(&key) {
        unsafe { *v += 1 };
    } else {
        let _ = LSM_COUNTERS.insert(&key, &1u64, 0);
    }
}

/// Record a violation; returns the hook's verdict.
#[inline(always)]
fn violation(cg: u64, p: &LsmPolicy, action: u8, a: u32, b: u64) -> i32 {
    let deny = p.flags & LSM_ENFORCE != 0 && now_ns() < p.deadline_ns;
    record(cg, action as u32 | (deny as u32) << 8, a, b);
    if deny { -EPERM } else { 0 }
}

/// `meta`: action | denied << 8 (BPF calls take at most five arguments).
#[inline(never)]
fn record(cg: u64, meta: u32, a: u32, b: u64) {
    let (action, deny) = (meta as u8, (meta >> 8) & 1 != 0);
    count(cg, action, deny);
    if let Some(mut e) = GK_EVENTS.reserve::<GkEvent>(0) {
        let ev = e.as_mut_ptr();
        unsafe {
            (*ev).ts_ns = now_ns();
            (*ev).cgroup_id = cg;
            (*ev).a = a;
            (*ev).b = b;
            (*ev).pid = (bpf_get_current_pid_tgid() >> 32) as u32;
            (*ev).kind = EV_KIND_LSM;
            (*ev).action = action;
            (*ev).denied = deny as u8;
            (*ev).family = 0;
            (*ev).port = 0;
            (*ev).proto = 0;
            (*ev)._pad = 0;
            bpf_get_current_comm((*ev).comm.as_mut_ptr().cast(), 16);
        }
        e.submit(0);
    }
}

/// (kernel dev_t of the inode's superblock, inode number).
#[inline(always)]
fn inode_id(c: &LsmCfg, inode: u64) -> (u32, u64) {
    let ino = rd::<u64>(inode, c.off_inode_ino).unwrap_or(0);
    let sb = rd::<u64>(inode, c.off_inode_sb).unwrap_or(0);
    let dev = if sb != 0 { rd::<u32>(sb, c.off_sb_dev).unwrap_or(0) } else { 0 };
    (dev, ino)
}

#[lsm(hook = "bprm_check_security")]
pub fn gk_lsm_exec(ctx: LsmContext) -> i32 {
    let prev: i32 = unsafe { ctx.arg(1) };
    if prev != 0 {
        return prev;
    }
    let Some((cg, p, c)) = scope(LSM_DENY_EXEC) else { return 0 };
    let bprm: u64 = unsafe { ctx.arg(0) };
    let Some(file) = rd::<u64>(bprm, c.off_bprm_file) else { return 0 };
    let Some(inode) = rd::<u64>(file, c.off_file_inode) else { return 0 };
    let (dev, ino) = inode_id(c, inode);
    let key = LsmFileKey { cgroup_id: cg, ino, dev, _pad: 0 };
    if unsafe { LSM_FILES.get(&key) }.is_some() {
        return 0;
    }
    violation(cg, p, LSM_ACT_EXEC, dev, ino)
}

#[lsm(hook = "file_mprotect")]
pub fn gk_lsm_mprotect(ctx: LsmContext) -> i32 {
    let prev: i32 = unsafe { ctx.arg(3) };
    if prev != 0 {
        return prev;
    }
    let Some((cg, p, c)) = scope(LSM_DENY_WX) else { return 0 };
    let prot: u64 = unsafe { ctx.arg(2) };
    if prot & PROT_EXEC == 0 {
        return 0;
    }
    let vma: u64 = unsafe { ctx.arg(0) };
    let flags = rd::<u64>(vma, c.off_vma_flags).unwrap_or(0);
    if prot & PROT_WRITE == 0 && flags & VM_WRITE == 0 {
        return 0;
    }
    violation(cg, p, LSM_ACT_WX, prot as u32, flags)
}

#[lsm(hook = "file_open")]
pub fn gk_lsm_open(ctx: LsmContext) -> i32 {
    let prev: i32 = unsafe { ctx.arg(1) };
    if prev != 0 {
        return prev;
    }
    let Some((cg, p, c)) = scope(LSM_RESTRICT_DEVICES | LSM_RESTRICT_WRITES) else { return 0 };
    let file: u64 = unsafe { ctx.arg(0) };
    let Some(inode) = rd::<u64>(file, c.off_file_inode) else { return 0 };
    let ty = rd::<u16>(inode, c.off_inode_mode).unwrap_or(0) & S_IFMT;
    if p.flags & LSM_RESTRICT_DEVICES != 0 && (ty == S_IFCHR || ty == S_IFBLK) {
        let rdev = rd::<u32>(inode, c.off_inode_rdev).unwrap_or(0);
        let (major, minor) = (rdev >> 20, rdev & 0xfffff);
        let kind = if ty == S_IFCHR { DEV_KIND_CHAR } else { DEV_KIND_BLOCK };
        let exact = LsmDevKey { cgroup_id: cg, dev: dev_key(kind, major, minor) };
        let any = LsmDevKey { cgroup_id: cg, dev: dev_key(kind, major, DEV_MINOR_ANY) };
        if unsafe { LSM_DEVICES.get(&exact) }.is_none() && unsafe { LSM_DEVICES.get(&any) }.is_none() {
            return violation(cg, p, LSM_ACT_DEVICE, major, minor as u64 | kind << 62);
        }
        return 0;
    }
    if p.flags & LSM_RESTRICT_WRITES != 0 && ty == S_IFREG {
        let fmode = rd::<u32>(file, c.off_file_mode).unwrap_or(0);
        if fmode & FMODE_WRITE == 0 {
            return 0;
        }
        let (dev, ino) = inode_id(c, inode);
        let key = LsmFsKey { cgroup_id: cg, dev, _pad: 0 };
        if unsafe { LSM_WRITABLE.get(&key) }.is_none() {
            return violation(cg, p, LSM_ACT_WRITE, dev, ino);
        }
    }
    0
}
