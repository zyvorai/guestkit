// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Map layouts shared by the in-guest eBPF programs (`guestkit-ebpf`) and the
//! agent-side loader (`guestkit-ebpf-runtime`). Every struct is `repr(C)`
//! with explicit padding: map keys are compared byte for byte.

#![no_std]

// ---------------------------------------------------------------------------
// Per-container network policy (cgroup_skb ingress/egress)
// ---------------------------------------------------------------------------

/// Egress from the container is policed (non-matching peers are violations).
pub const NP_EGRESS_ISOLATED: u32 = 1 << 0;
/// Ingress to the container is policed.
pub const NP_INGRESS_ISOLATED: u32 = 1 << 1;
/// Violations are dropped (only while `deadline_ns` is in the future).
pub const NP_ENFORCE: u32 = 1 << 2;

pub const NP_DIR_EGRESS: u8 = 1;
pub const NP_DIR_INGRESS: u8 = 2;
pub const NP_FAMILY_V4: u8 = 4;
pub const NP_FAMILY_V6: u8 = 6;
pub const NP_MAX_RULES: u32 = 32;

/// NP_POLICIES value, keyed by cgroup id (the cgroup directory's inode).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpPolicy {
    pub flags: u32,
    pub nrules: u32,
    /// CLOCK_MONOTONIC ns; enforcement reverts to audit by itself past this.
    pub deadline_ns: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NpRuleKey {
    pub cgroup_id: u64,
    pub slot: u32,
    pub _pad: u32,
}

/// One allow rule: peer CIDR (`net`/`mask` are the address bytes read as
/// native-endian words, so the program compares raw packet loads), plus an
/// optional IP protocol and destination port.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpRule {
    pub dir: u8,
    pub family: u8,
    /// 0 = any.
    pub proto: u8,
    pub _pad: u8,
    /// Destination port, host order; 0 = any.
    pub port: u16,
    pub _pad2: u16,
    pub net: [u32; 4],
    pub mask: [u32; 4],
}

/// Per-cgroup packet counters (PERCPU_HASH value).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpCounters {
    pub allowed: u64,
    pub audited: u64,
    pub denied: u64,
}

// ---------------------------------------------------------------------------
// Per-container MAC (BPF-LSM)
// ---------------------------------------------------------------------------

/// Deny exec of binaries outside LSM_FILES.
pub const LSM_DENY_EXEC: u32 = 1 << 0;
/// Deny writable+executable mappings.
pub const LSM_DENY_WX: u32 = 1 << 1;
/// Deny opens of char/block devices outside LSM_DEVICES.
pub const LSM_RESTRICT_DEVICES: u32 = 1 << 2;
/// Deny write-opens of regular files on filesystems outside LSM_WRITABLE.
pub const LSM_RESTRICT_WRITES: u32 = 1 << 3;
pub const LSM_ENFORCE: u32 = 1 << 4;

pub const LSM_ACT_EXEC: u8 = 1;
pub const LSM_ACT_WX: u8 = 2;
pub const LSM_ACT_DEVICE: u8 = 3;
pub const LSM_ACT_WRITE: u8 = 4;

/// LSM_DEVICES minor wildcard.
pub const DEV_MINOR_ANY: u32 = 0xffff_ffff;
pub const DEV_KIND_CHAR: u64 = 1;
pub const DEV_KIND_BLOCK: u64 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LsmPolicy {
    pub flags: u32,
    pub _pad: u32,
    pub deadline_ns: u64,
}

/// Running kernel's struct member offsets (from BTF; no CO-RE in Rust eBPF).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LsmCfg {
    pub off_bprm_file: u32,
    pub off_file_inode: u32,
    pub off_file_mode: u32,
    pub off_inode_ino: u32,
    pub off_inode_sb: u32,
    pub off_sb_dev: u32,
    pub off_inode_mode: u32,
    pub off_inode_rdev: u32,
    pub off_vma_flags: u32,
    pub ready: u32,
}

/// LSM_FILES key: an executable a container may exec (kernel dev_t + inode).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LsmFileKey {
    pub cgroup_id: u64,
    pub ino: u64,
    pub dev: u32,
    pub _pad: u32,
}

/// LSM_DEVICES key: `dev` = kind << 62 | major << 32 | minor (DEV_MINOR_ANY).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LsmDevKey {
    pub cgroup_id: u64,
    pub dev: u64,
}

/// LSM_WRITABLE key: a filesystem (kernel dev_t of its superblock).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LsmFsKey {
    pub cgroup_id: u64,
    pub dev: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LsmCounterKey {
    pub cgroup_id: u64,
    pub action: u32,
    pub denied: u32,
}

#[inline(always)]
pub const fn dev_key(kind: u64, major: u32, minor: u32) -> u64 {
    kind << 62 | (major as u64) << 32 | minor as u64
}

// ---------------------------------------------------------------------------
// Violation events (shared ring buffer)
// ---------------------------------------------------------------------------

pub const EV_KIND_NET: u8 = 1;
pub const EV_KIND_LSM: u8 = 2;

/// Net: `a` = first address word of the peer, `b` = full v4 peer or the
/// first 8 v6 bytes; `action` = direction. LSM: `a`/`b` = action-specific
/// object (exec: dev/ino, mprotect: prot/vm_flags, open: major/minor or dev/ino).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GkEvent {
    pub ts_ns: u64,
    pub cgroup_id: u64,
    pub b: u64,
    pub a: u32,
    pub pid: u32,
    pub kind: u8,
    pub action: u8,
    pub denied: u8,
    pub family: u8,
    pub port: u16,
    pub proto: u8,
    pub _pad: u8,
    pub comm: [u8; 16],
}

#[cfg(all(feature = "user", target_os = "linux"))]
mod pod {
    use super::*;
    macro_rules! pod {
        ($($t:ty),*) => { $(unsafe impl aya::Pod for $t {})* };
    }
    pod!(
        NpPolicy, NpRuleKey, NpRule, NpCounters, LsmPolicy, LsmCfg, LsmFileKey, LsmDevKey, LsmFsKey,
        LsmCounterKey, GkEvent
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn layouts_have_no_implicit_padding() {
        assert_eq!(size_of::<NpPolicy>(), 16);
        assert_eq!(size_of::<NpRuleKey>(), 16);
        assert_eq!(size_of::<NpRule>(), 40);
        assert_eq!(size_of::<NpCounters>(), 24);
        assert_eq!(size_of::<LsmPolicy>(), 16);
        assert_eq!(size_of::<LsmCfg>(), 40);
        assert_eq!(size_of::<LsmFileKey>(), 24);
        assert_eq!(size_of::<LsmDevKey>(), 16);
        assert_eq!(size_of::<LsmFsKey>(), 16);
        assert_eq!(size_of::<LsmCounterKey>(), 16);
        assert_eq!(size_of::<GkEvent>(), 56);
    }

    #[test]
    fn dev_key_packs_kind_major_minor() {
        let k = dev_key(DEV_KIND_CHAR, 1, 3);
        assert_eq!(k >> 62, DEV_KIND_CHAR);
        assert_eq!((k >> 32) as u32 & 0x3fff_ffff, 1);
        assert_eq!(k as u32, 3);
        assert_ne!(dev_key(DEV_KIND_BLOCK, 1, 3), k);
    }
}
