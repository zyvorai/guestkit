// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! aya loader: one object, cgroup_skb programs attached per container
//! cgroup, LSM hooks attached once (they scope themselves by cgroup id).

use std::collections::{HashMap as StdHashMap, VecDeque};
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use aya::{
    include_bytes_aligned,
    maps::{Array, HashMap, MapData, PerCpuHashMap, RingBuf},
    programs::{
        cgroup_skb::CgroupSkbLinkId, lsm::LsmLinkId, CgroupAttachMode, CgroupSkb, CgroupSkbAttachType, Lsm,
    },
    Ebpf, EbpfLoader, Pod,
};
use guestkit_ebpf_common::*;

use crate::{decode_event, Event};

static OBJECT: &[u8] = include_bytes_aligned!(concat!(env!("OUT_DIR"), "/guestkit-ebpf.o"));

const LSM_HOOKS: &[(&str, &str)] =
    &[("gk_lsm_exec", "bprm_check_security"), ("gk_lsm_mprotect", "file_mprotect"), ("gk_lsm_open", "file_open")];
const LSM_LIST: &str = "/sys/kernel/security/lsm";
const EVENT_CAP: usize = 512;

/// False when the agent was built without nightly + bpf-linker.
pub fn programs_compiled() -> bool {
    !OBJECT.is_empty()
}

/// CLOCK_MONOTONIC in ns (same clock as `bpf_ktime_get_ns`).
pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub fn lsm_list() -> String {
    std::fs::read_to_string(LSM_LIST).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// BPF-LSM hooks only run when `bpf` is in the kernel's active LSM list.
pub fn lsm_active() -> bool {
    lsm_list().split(',').any(|l| l.trim() == "bpf")
}

fn bump_memlock() {
    let rlim = libc::rlimit { rlim_cur: libc::RLIM_INFINITY, rlim_max: libc::RLIM_INFINITY };
    unsafe {
        libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim);
    }
}

fn lsm_layout() -> Option<LsmCfg> {
    let b = crate::btf::Btf::from_sys_fs().ok()?;
    Some(LsmCfg {
        off_bprm_file: b.offset("linux_binprm", "file")?,
        off_file_inode: b.offset("file", "f_inode")?,
        off_file_mode: b.offset("file", "f_mode")?,
        off_inode_ino: b.offset("inode", "i_ino")?,
        off_inode_sb: b.offset("inode", "i_sb")?,
        off_sb_dev: b.offset("super_block", "s_dev")?,
        off_inode_mode: b.offset("inode", "i_mode")?,
        off_inode_rdev: b.offset("inode", "i_rdev")?,
        off_vma_flags: b.offset("vm_area_struct", "vm_flags")?,
        ready: 1,
    })
}

pub struct GuestEbpf {
    ebpf: Ebpf,
    np_loaded: bool,
    np_links: StdHashMap<u64, (CgroupSkbLinkId, CgroupSkbLinkId)>,
    lsm_links: Vec<(&'static str, LsmLinkId)>,
    events: Arc<Mutex<VecDeque<Event>>>,
    stop: Arc<AtomicBool>,
    drain: Option<std::thread::JoinHandle<()>>,
}

impl Drop for GuestEbpf {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.drain.take() {
            let _ = h.join();
        }
    }
}

impl GuestEbpf {
    pub fn load() -> Result<Self> {
        if !programs_compiled() {
            bail!("guestkit was built without its eBPF programs (needs nightly + bpf-linker at build time)");
        }
        bump_memlock();
        let mut ebpf = EbpfLoader::new().load(OBJECT).context("load guestkit-ebpf object")?;
        let rb = RingBuf::try_from(ebpf.take_map("GK_EVENTS").ok_or_else(|| anyhow!("map GK_EVENTS missing"))?)?;
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let drain = {
            let (events, stop) = (events.clone(), stop.clone());
            std::thread::Builder::new()
                .name("guestkit-ebpf-events".into())
                .spawn(move || drain_events(rb, events, stop))?
        };
        Ok(Self {
            ebpf,
            np_loaded: false,
            np_links: StdHashMap::new(),
            lsm_links: Vec::new(),
            events,
            stop,
            drain: Some(drain),
        })
    }

    fn hash<K: Pod, V: Pod>(&mut self, name: &str) -> Result<HashMap<&mut MapData, K, V>> {
        let m = self.ebpf.map_mut(name).ok_or_else(|| anyhow!("map {name} missing"))?;
        Ok(HashMap::try_from(m)?)
    }

    fn remove_where<K: Pod, V: Pod>(&mut self, name: &str, pred: impl Fn(&K) -> bool) -> Result<()> {
        let mut m = self.hash::<K, V>(name)?;
        let keys: Vec<K> = m.keys().filter_map(|r| r.ok()).filter(|k| pred(k)).collect();
        for k in keys {
            let _ = m.remove(&k);
        }
        Ok(())
    }

    fn percpu_get<K: Pod, V: Pod + Default>(&mut self, name: &str, k: &K, add: impl Fn(&mut V, &V)) -> V {
        let mut s = V::default();
        let Some(m) = self.ebpf.map_mut(name) else { return s };
        let Ok(m) = PerCpuHashMap::<&mut MapData, K, V>::try_from(m) else { return s };
        if let Ok(per_cpu) = m.get(k, 0) {
            for c in per_cpu.iter() {
                add(&mut s, c);
            }
        }
        s
    }

    fn percpu_remove<K: Pod, V: Pod>(&mut self, name: &str, k: &K) {
        if let Some(m) = self.ebpf.map_mut(name) {
            if let Ok(mut m) = PerCpuHashMap::<&mut MapData, K, V>::try_from(m) {
                let _ = m.remove(k);
            }
        }
    }

    // ---- network policy -------------------------------------------------

    fn np_load(&mut self) -> Result<()> {
        if self.np_loaded {
            return Ok(());
        }
        for name in ["gk_np_egress", "gk_np_ingress"] {
            let p: &mut CgroupSkb =
                self.ebpf.program_mut(name).ok_or_else(|| anyhow!("program {name} missing"))?.try_into()?;
            p.load().with_context(|| format!("verifier rejected {name}"))?;
        }
        self.np_loaded = true;
        Ok(())
    }

    fn np_attach(&mut self, cgroup_id: u64, cgroup: &Path) -> Result<()> {
        if self.np_links.contains_key(&cgroup_id) {
            return Ok(());
        }
        self.np_load()?;
        let mut ids = Vec::new();
        for (name, ty) in [("gk_np_egress", CgroupSkbAttachType::Egress), ("gk_np_ingress", CgroupSkbAttachType::Ingress)] {
            let f = File::open(cgroup).with_context(|| format!("open {}", cgroup.display()))?;
            let p: &mut CgroupSkb = self.ebpf.program_mut(name).expect("loaded").try_into()?;
            // bpf_link attachments always coexist with other programs on the
            // cgroup; link_create rejects the legacy ALLOW_MULTI flag.
            match p.attach(f, ty, CgroupAttachMode::default()) {
                Ok(id) => ids.push(id),
                Err(e) => {
                    if let Some(id) = ids.pop() {
                        let p: &mut CgroupSkb = self.ebpf.program_mut("gk_np_egress").expect("loaded").try_into()?;
                        let _ = p.detach(id);
                    }
                    return Err(anyhow!("attach {name} to {}: {e}", cgroup.display()));
                }
            }
        }
        let ingress = ids.pop().expect("two links");
        let egress = ids.pop().expect("two links");
        self.np_links.insert(cgroup_id, (egress, ingress));
        Ok(())
    }

    /// Install (or replace) a container's policy and attach the programs to
    /// its cgroup. `ids` are the cgroup ids of the container cgroup and its
    /// descendants (the programs look up the socket's own cgroup), the first
    /// being the container cgroup itself. Rules are written before the
    /// policy so the program never sees a policy pointing at missing slots.
    pub fn np_apply(&mut self, cgroup: &Path, ids: &[u64], policy: NpPolicy, rules: &[NpRule]) -> Result<()> {
        if rules.len() > NP_MAX_RULES as usize {
            bail!("at most {NP_MAX_RULES} rules per container");
        }
        let Some(&top) = ids.first() else { bail!("no cgroup ids") };
        self.np_attach(top, cgroup)?;
        let n = rules.len() as u32;
        for &cgroup_id in ids {
            {
                let mut m = self.hash::<NpRuleKey, NpRule>("NP_RULES")?;
                for (slot, r) in rules.iter().enumerate() {
                    m.insert(NpRuleKey { cgroup_id, slot: slot as u32, _pad: 0 }, *r, 0)?;
                }
            }
            self.remove_where::<NpRuleKey, NpRule>("NP_RULES", |k| k.cgroup_id == cgroup_id && k.slot >= n)?;
            self.hash::<u64, NpPolicy>("NP_POLICIES")?.insert(cgroup_id, NpPolicy { nrules: n, ..policy }, 0)?;
        }
        Ok(())
    }

    /// Update only the policy flags/deadline (lease expiry → audit).
    pub fn np_set_policy(&mut self, ids: &[u64], policy: NpPolicy) -> Result<()> {
        let mut m = self.hash::<u64, NpPolicy>("NP_POLICIES")?;
        for id in ids {
            m.insert(*id, policy, 0)?;
        }
        Ok(())
    }

    pub fn np_remove(&mut self, ids: &[u64]) -> Result<()> {
        for &cgroup_id in ids {
            let _ = self.hash::<u64, NpPolicy>("NP_POLICIES")?.remove(&cgroup_id);
            self.remove_where::<NpRuleKey, NpRule>("NP_RULES", |k| k.cgroup_id == cgroup_id)?;
            self.percpu_remove::<u64, NpCounters>("NP_COUNTERS", &cgroup_id);
        }
        let Some(&top) = ids.first() else { return Ok(()) };
        if let Some((eg, ing)) = self.np_links.remove(&top) {
            let p: &mut CgroupSkb = self.ebpf.program_mut("gk_np_egress").expect("loaded").try_into()?;
            let _ = p.detach(eg);
            let p: &mut CgroupSkb = self.ebpf.program_mut("gk_np_ingress").expect("loaded").try_into()?;
            let _ = p.detach(ing);
        }
        Ok(())
    }

    pub fn np_policies(&mut self) -> Result<Vec<(u64, NpPolicy)>> {
        let m = self.hash::<u64, NpPolicy>("NP_POLICIES")?;
        Ok(m.iter().filter_map(|r| r.ok()).collect())
    }

    pub fn np_attached(&self, top_cgroup_id: u64) -> bool {
        self.np_links.contains_key(&top_cgroup_id)
    }

    pub fn np_counters(&mut self, ids: &[u64]) -> NpCounters {
        let mut t = NpCounters::default();
        for id in ids {
            let c = self.percpu_get::<u64, NpCounters>("NP_COUNTERS", id, |s, c| {
                s.allowed += c.allowed;
                s.audited += c.audited;
                s.denied += c.denied;
            });
            t.allowed += c.allowed;
            t.audited += c.audited;
            t.denied += c.denied;
        }
        t
    }

    // ---- LSM ------------------------------------------------------------

    pub fn lsm_attached(&self) -> bool {
        self.lsm_links.len() == LSM_HOOKS.len()
    }

    /// Load + attach the three hooks once and publish the kernel layout.
    pub fn lsm_attach(&mut self) -> Result<()> {
        if self.lsm_attached() {
            return Ok(());
        }
        let cfg = lsm_layout().ok_or_else(|| anyhow!("kernel BTF lacks the file/inode layout the LSM hooks need"))?;
        {
            let m = self.ebpf.map_mut("LSM_CFG").ok_or_else(|| anyhow!("map LSM_CFG missing"))?;
            Array::<&mut MapData, LsmCfg>::try_from(m)?.set(0, cfg, 0)?;
        }
        let btf = aya::Btf::from_sys_fs().context("kernel BTF (/sys/kernel/btf/vmlinux)")?;
        for (prog, hook) in LSM_HOOKS {
            if self.lsm_links.iter().any(|(p, _)| p == prog) {
                continue;
            }
            let p: &mut Lsm = self.ebpf.program_mut(prog).ok_or_else(|| anyhow!("program {prog} missing"))?.try_into()?;
            if p.fd().is_err() {
                p.load(hook, &btf).with_context(|| format!("verifier rejected {prog}"))?;
            }
            let id = p.attach().with_context(|| format!("attach {prog} to lsm/{hook}"))?;
            self.lsm_links.push((prog, id));
        }
        Ok(())
    }

    /// Detach the hooks (when no container has an LSM policy left).
    pub fn lsm_detach(&mut self) {
        for (prog, id) in std::mem::take(&mut self.lsm_links) {
            if let Some(p) = self.ebpf.program_mut(prog) {
                if let Ok(p) = <&mut Lsm>::try_from(p) {
                    let _ = p.detach(id);
                }
            }
        }
    }

    /// Replace a container's MAC policy and allowlists on every cgroup id
    /// of the container (its cgroup and descendants).
    pub fn lsm_apply(
        &mut self,
        ids: &[u64],
        policy: LsmPolicy,
        files: &[(u64, u32)],
        devices: &[u64],
        writable: &[u32],
    ) -> Result<()> {
        self.lsm_attach()?;
        for &cgroup_id in ids {
            self.lsm_clear_lists(cgroup_id)?;
            {
                let mut m = self.hash::<LsmFileKey, u8>("LSM_FILES")?;
                for (ino, dev) in files {
                    m.insert(LsmFileKey { cgroup_id, ino: *ino, dev: *dev, _pad: 0 }, 1, 0)?;
                }
            }
            {
                let mut m = self.hash::<LsmDevKey, u8>("LSM_DEVICES")?;
                for dev in devices {
                    m.insert(LsmDevKey { cgroup_id, dev: *dev }, 1, 0)?;
                }
            }
            {
                let mut m = self.hash::<LsmFsKey, u8>("LSM_WRITABLE")?;
                for dev in writable {
                    m.insert(LsmFsKey { cgroup_id, dev: *dev, _pad: 0 }, 1, 0)?;
                }
            }
            self.hash::<u64, LsmPolicy>("LSM_POLICIES")?.insert(cgroup_id, policy, 0)?;
        }
        Ok(())
    }

    pub fn lsm_set_policy(&mut self, ids: &[u64], policy: LsmPolicy) -> Result<()> {
        let mut m = self.hash::<u64, LsmPolicy>("LSM_POLICIES")?;
        for id in ids {
            m.insert(*id, policy, 0)?;
        }
        Ok(())
    }

    fn lsm_clear_lists(&mut self, cgroup_id: u64) -> Result<()> {
        self.remove_where::<LsmFileKey, u8>("LSM_FILES", |k| k.cgroup_id == cgroup_id)?;
        self.remove_where::<LsmDevKey, u8>("LSM_DEVICES", |k| k.cgroup_id == cgroup_id)?;
        self.remove_where::<LsmFsKey, u8>("LSM_WRITABLE", |k| k.cgroup_id == cgroup_id)
    }

    pub fn lsm_remove(&mut self, ids: &[u64]) -> Result<()> {
        for &cgroup_id in ids {
            let _ = self.hash::<u64, LsmPolicy>("LSM_POLICIES")?.remove(&cgroup_id);
            self.lsm_clear_lists(cgroup_id)?;
        }
        let m = self.ebpf.map_mut("LSM_COUNTERS").ok_or_else(|| anyhow!("map LSM_COUNTERS missing"))?;
        let mut m = PerCpuHashMap::<&mut MapData, LsmCounterKey, u64>::try_from(m)?;
        let keys: Vec<LsmCounterKey> =
            m.keys().filter_map(|r| r.ok()).filter(|k| ids.contains(&k.cgroup_id)).collect();
        for k in keys {
            let _ = m.remove(&k);
        }
        Ok(())
    }

    pub fn lsm_policies(&mut self) -> Result<Vec<(u64, LsmPolicy)>> {
        let m = self.hash::<u64, LsmPolicy>("LSM_POLICIES")?;
        Ok(m.iter().filter_map(|r| r.ok()).collect())
    }

    /// (action, denied, count) for one container.
    pub fn lsm_counters(&mut self, ids: &[u64]) -> Vec<(u8, bool, u64)> {
        let mut out = Vec::new();
        for action in [LSM_ACT_EXEC, LSM_ACT_WX, LSM_ACT_DEVICE, LSM_ACT_WRITE] {
            for denied in [false, true] {
                let mut n = 0;
                for &cgroup_id in ids {
                    let k = LsmCounterKey { cgroup_id, action: action as u32, denied: denied as u32 };
                    n += self.percpu_get::<LsmCounterKey, u64>("LSM_COUNTERS", &k, |s, c| *s += *c);
                }
                if n > 0 {
                    out.push((action, denied, n));
                }
            }
        }
        out
    }

    /// Most recent events (oldest first), optionally of one kind.
    pub fn events(&self, kind: Option<&str>, limit: usize) -> Vec<Event> {
        let q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<Event> = q.iter().filter(|e| kind.is_none_or(|k| e.kind == k)).cloned().collect();
        let skip = v.len().saturating_sub(limit);
        v.drain(..skip);
        v
    }
}

fn drain_events(mut rb: RingBuf<MapData>, events: Arc<Mutex<VecDeque<Event>>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        while let Some(item) = rb.next() {
            if item.len() < core::mem::size_of::<GkEvent>() {
                continue;
            }
            let raw: GkEvent = unsafe { core::ptr::read_unaligned(item.as_ptr().cast()) };
            let mut q = events.lock().unwrap_or_else(|e| e.into_inner());
            if q.len() >= EVENT_CAP {
                q.pop_front();
            }
            q.push_back(decode_event(&raw));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_kernel_has_lsm_layout() {
        if crate::btf::Btf::from_sys_fs().is_err() {
            return;
        }
        let c = lsm_layout().expect("vmlinux BTF has the file/inode/vma members");
        assert_eq!(c.ready, 1);
        assert!(c.off_file_inode > 0 || c.off_bprm_file > 0);
    }
}
