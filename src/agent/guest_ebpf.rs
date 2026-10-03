// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Per-container eBPF policy inside the guest (`guestkit.netpolicy.*`,
//! `guestkit.lsm.*`): network allow rules on the container's cgroup
//! (cgroup_skb) and a MAC policy on BPF-LSM hooks scoped by cgroup id.
//!
//! Audit by default. Enforcement needs a lease (1..=3600 s) whose deadline
//! is written into the policy map, so the kernel programs revert to audit by
//! themselves when it passes, even if the agent is gone. Nothing is
//! persisted: the programs detach when guestkitd exits.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
pub const MAX_LEASE_SECS: u64 = 3600;
const EVENT_LIMIT: usize = 100;

/// Char devices every container needs (null, zero, full, random, urandom,
/// tty, console, ptmx, pts).
const DEFAULT_DEVICES: &[&str] = &[
    "c 1:3", "c 1:5", "c 1:7", "c 1:8", "c 1:9", "c 5:0", "c 5:1", "c 5:2", "c 136:*",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    /// Container name or id (Docker, Podman or CRI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// cgroup v2 directory (absolute under /sys/fs/cgroup, or relative to it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroup: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetRuleSpec {
    pub direction: String,
    pub cidr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

fn default_mode() -> String {
    "audit".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetpolicyApply {
    #[serde(flatten)]
    pub target: Target,
    /// off | audit | enforce
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub lease_secs: Option<u64>,
    #[serde(default = "default_true")]
    pub egress: bool,
    #[serde(default)]
    pub ingress: bool,
    #[serde(default)]
    pub rules: Vec<NetRuleSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LsmSpec {
    #[serde(default)]
    pub deny_exec: bool,
    /// Executables (paths inside the container) still allowed to exec.
    #[serde(default)]
    pub allow_exec: Vec<String>,
    #[serde(default)]
    pub deny_wx: bool,
    #[serde(default)]
    pub restrict_devices: bool,
    /// `c|b MAJOR:MINOR|*` on top of the default tty/null/random set.
    #[serde(default)]
    pub allow_devices: Vec<String>,
    #[serde(default)]
    pub restrict_writes: bool,
    /// Paths inside the container whose filesystems stay writable.
    #[serde(default)]
    pub writable_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LsmApply {
    #[serde(flatten)]
    pub target: Target,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub lease_secs: Option<u64>,
    #[serde(flatten)]
    pub spec: LsmSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    Audit,
    Enforce { lease_secs: u64 },
}

pub fn parse_mode(mode: &str, lease_secs: Option<u64>) -> Result<Mode, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "off" | "remove" => Ok(Mode::Off),
        "audit" | "observe" => Ok(Mode::Audit),
        "enforce" => {
            let s = lease_secs.ok_or_else(|| format!("enforce needs lease_secs (1..={MAX_LEASE_SECS})"))?;
            if !(1..=MAX_LEASE_SECS).contains(&s) {
                return Err(format!("lease_secs must be 1..={MAX_LEASE_SECS}"));
            }
            Ok(Mode::Enforce { lease_secs: s })
        }
        m => Err(format!("mode must be off, audit or enforce (got `{m}`)")),
    }
}

/// `c 1:3`, `c 136:*`, `b 8:0` → (kind, major, minor | DEV_MINOR_ANY).
pub fn parse_device(s: &str) -> Result<(u64, u32, u32), String> {
    let bad = || format!("device `{s}`: expected `c|b MAJOR:MINOR` (MINOR may be *)");
    let mut it = s.split_whitespace();
    let kind = match it.next() {
        Some("c") => 1,
        Some("b") => 2,
        _ => return Err(bad()),
    };
    let (ma, mi) = it.next().and_then(|d| d.split_once(':')).ok_or_else(bad)?;
    if it.next().is_some() {
        return Err(bad());
    }
    let major = ma.parse::<u32>().map_err(|_| bad())?;
    let minor = if mi == "*" { u32::MAX } else { mi.parse::<u32>().map_err(|_| bad())? };
    if major >= 1 << 12 || (minor != u32::MAX && minor >= 1 << 20) {
        return Err(bad());
    }
    Ok((kind, major, minor))
}

/// The unified-hierarchy path from `/proc/<pid>/cgroup` content.
pub fn parse_cgroup_v2(content: &str) -> Option<String> {
    content.lines().find_map(|l| l.strip_prefix("0::").map(|p| p.trim().to_string()))
}

/// A cgroup path the caller may name: inside /sys/fs/cgroup, no `..`.
pub fn cgroup_dir(spec: &str) -> Result<PathBuf, String> {
    if spec.split('/').any(|c| c == "..") {
        return Err(format!("cgroup `{spec}`: `..` not allowed"));
    }
    let rel = spec.strip_prefix(CGROUP_ROOT).unwrap_or(spec).trim_start_matches('/');
    if rel.is_empty() {
        return Err("refusing to police the root cgroup".into());
    }
    Ok(Path::new(CGROUP_ROOT).join(rel))
}

/// Unique key for a target in agent state and responses.
pub fn target_key(t: &Target) -> Result<String, String> {
    match (&t.container, &t.cgroup) {
        (Some(c), None) if !c.trim().is_empty() => Ok(format!("container:{}", c.trim())),
        (None, Some(g)) if !g.trim().is_empty() => Ok(format!("cgroup:{}", cgroup_dir(g.trim())?.display())),
        (Some(_), Some(_)) => Err("pass either container or cgroup, not both".into()),
        _ => Err("missing target: pass container (name/id) or cgroup".into()),
    }
}

#[cfg(all(target_os = "linux", feature = "ebpf"))]
mod imp {
    use super::*;
    use guestkit_ebpf_runtime::{self as rt, common::*, GuestEbpf};
    use std::collections::BTreeMap;
    use std::os::unix::fs::MetadataExt;
    use std::process::Command;
    use std::sync::Mutex;

    struct Resolved {
        cgroup: PathBuf,
        /// `/proc/<pid>/root` for containers, so in-container paths resolve.
        root: Option<PathBuf>,
        ids: Vec<u64>,
    }

    struct Entry {
        cgroup: PathBuf,
        ids: Vec<u64>,
        enforce: bool,
        deadline: u64,
        lease_expired: bool,
        spec: Value,
    }

    struct State {
        ebpf: GuestEbpf,
        np: BTreeMap<String, (Entry, NpPolicy)>,
        lsm: BTreeMap<String, (Entry, LsmPolicy)>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn with_state<T>(f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
        let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            let ebpf = GuestEbpf::load().map_err(|e| format!("ebpf_unavailable: {e:#}"))?;
            *g = Some(State { ebpf, np: BTreeMap::new(), lsm: BTreeMap::new() });
        }
        let s = g.as_mut().expect("initialized");
        sweep(s);
        f(s)
    }

    /// Mirror the in-kernel lease expiry in agent state (the programs have
    /// already stopped denying by then).
    fn sweep(s: &mut State) {
        let now = rt::monotonic_ns();
        for (e, p) in s.np.values_mut() {
            if e.enforce && now >= e.deadline {
                p.flags &= !NP_ENFORCE;
                p.deadline_ns = 0;
                let _ = s.ebpf.np_set_policy(&e.ids, *p);
                e.enforce = false;
                e.lease_expired = true;
            }
        }
        for (e, p) in s.lsm.values_mut() {
            if e.enforce && now >= e.deadline {
                p.flags &= !LSM_ENFORCE;
                p.deadline_ns = 0;
                let _ = s.ebpf.lsm_set_policy(&e.ids, *p);
                e.enforce = false;
                e.lease_expired = true;
            }
        }
    }

    fn run(cmd: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(cmd).args(args).stderr(std::process::Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn container_pid(name: &str) -> Option<u32> {
        for rt in ["docker", "podman"] {
            if let Some(pid) = run(rt, &["inspect", "--format", "{{.State.Pid}}", name]).and_then(|s| s.parse().ok()) {
                if pid > 0 {
                    return Some(pid);
                }
            }
        }
        let info = run("crictl", &["inspect", name])?;
        let v: Value = serde_json::from_str(&info).ok()?;
        v.pointer("/info/pid").and_then(Value::as_u64).map(|p| p as u32).filter(|p| *p > 0)
    }

    /// The cgroup directory and its descendants' inode numbers (== cgroup ids).
    fn subtree_ids(dir: &Path) -> Vec<u64> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(md) = std::fs::metadata(&d) else { continue };
            out.push(md.ino());
            if let Ok(rd) = std::fs::read_dir(&d) {
                stack.extend(rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()));
            }
        }
        out
    }

    fn resolve(t: &Target) -> Result<Resolved, String> {
        let (cgroup, root) = if let Some(name) = &t.container {
            let name = name.trim();
            let pid = container_pid(name).ok_or_else(|| format!("container `{name}` not found or not running"))?;
            let cg = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
                .ok()
                .and_then(|c| parse_cgroup_v2(&c))
                .ok_or_else(|| format!("container `{name}`: no cgroup v2 path for pid {pid}"))?;
            (cgroup_dir(&cg)?, Some(PathBuf::from(format!("/proc/{pid}/root"))))
        } else {
            (cgroup_dir(t.cgroup.as_deref().unwrap_or_default().trim())?, None)
        };
        if !cgroup.is_dir() {
            return Err(format!("cgroup {} not found", cgroup.display()));
        }
        let ids = subtree_ids(&cgroup);
        Ok(Resolved { cgroup, root, ids })
    }

    fn inside(root: &Option<PathBuf>, p: &str) -> PathBuf {
        match root {
            Some(r) => r.join(p.trim_start_matches('/')),
            None => PathBuf::from(p),
        }
    }

    fn kdev(st_dev: u64) -> u32 {
        let d = st_dev as libc::dev_t;
        rt::kdev(libc::major(d), libc::minor(d))
    }

    fn lease_deadline(mode: Mode) -> (bool, u64) {
        match mode {
            Mode::Enforce { lease_secs } => (true, rt::monotonic_ns() + lease_secs * 1_000_000_000),
            _ => (false, 0),
        }
    }

    fn mode_json(e: &Entry) -> Value {
        let now = rt::monotonic_ns();
        json!({
            "mode": if e.enforce { "enforce" } else { "audit" },
            "lease_remaining_secs": if e.enforce { Some(e.deadline.saturating_sub(now) / 1_000_000_000) } else { None },
            "lease_expired": e.lease_expired,
        })
    }

    fn merge(mut a: Value, b: Value) -> Value {
        if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
            a.extend(b);
        }
        a
    }

    fn events_for(s: &State, kind: &str, keys: &BTreeMap<u64, String>) -> Vec<Value> {
        s.ebpf
            .events(Some(kind), EVENT_LIMIT)
            .into_iter()
            .map(|e| {
                let target = keys.get(&e.cgroup_id).cloned();
                merge(serde_json::to_value(&e).unwrap_or_default(), json!({ "target": target }))
            })
            .collect()
    }

    // ---- netpolicy ------------------------------------------------------

    pub fn netpolicy_apply(params: &Value) -> Result<Value, String> {
        let req: NetpolicyApply = serde_json::from_value(params.clone()).map_err(|e| format!("invalid params: {e}"))?;
        let key = target_key(&req.target)?;
        let mode = parse_mode(&req.mode, req.lease_secs)?;
        let mut rules = Vec::new();
        for r in &req.rules {
            let dir = rt::parse_direction(&r.direction)?;
            let (addr, prefix) = rt::parse_cidr(&r.cidr)?;
            let proto = rt::parse_proto(r.proto.as_deref().unwrap_or("any"))?;
            rules.push(rt::compile_rule(dir, addr, prefix, proto, r.port.unwrap_or(0))?);
        }
        if rules.len() > NP_MAX_RULES as usize {
            return Err(format!("at most {NP_MAX_RULES} rules"));
        }
        if mode != Mode::Off && !req.egress && !req.ingress {
            return Err("police at least one of egress / ingress".into());
        }
        with_state(|s| {
            if mode == Mode::Off {
                if let Some((e, _)) = s.np.remove(&key) {
                    s.ebpf.np_remove(&e.ids).map_err(|e| format!("{e:#}"))?;
                }
                return Ok(json!({ "target": key, "removed": true }));
            }
            let r = resolve(&req.target)?;
            if let Some((old, _)) = s.np.get(&key) {
                if old.cgroup != r.cgroup {
                    let ids = old.ids.clone();
                    s.ebpf.np_remove(&ids).map_err(|e| format!("{e:#}"))?;
                    s.np.remove(&key);
                }
            }
            let (enforce, deadline) = lease_deadline(mode);
            let policy = rt::np_policy(req.egress, req.ingress, enforce, rules.len() as u32, deadline);
            s.ebpf.np_apply(&r.cgroup, &r.ids, policy, &rules).map_err(|e| format!("{e:#}"))?;
            let spec = json!({ "egress": req.egress, "ingress": req.ingress, "rules": req.rules });
            let entry = Entry { cgroup: r.cgroup, ids: r.ids, enforce, deadline, lease_expired: false, spec };
            s.np.insert(key.clone(), (entry, NpPolicy { nrules: rules.len() as u32, ..policy }));
            Ok(np_target_json(s, &key))
        })
    }

    fn np_target_json(s: &mut State, key: &str) -> Value {
        let Some((e, _)) = s.np.get(key) else { return Value::Null };
        let (ids, top) = (e.ids.clone(), e.ids.first().copied().unwrap_or(0));
        let base = merge(
            json!({
                "target": key,
                "cgroup": e.cgroup.display().to_string(),
                "cgroup_ids": e.ids,
            }),
            merge(mode_json(e), e.spec.clone()),
        );
        let c = s.ebpf.np_counters(&ids);
        merge(
            base,
            json!({
                "attached": s.ebpf.np_attached(top),
                "counters": { "allowed": c.allowed, "audited": c.audited, "denied": c.denied },
            }),
        )
    }

    pub fn netpolicy_status(params: &Value) -> Result<Value, String> {
        let only = filter_key(params)?;
        let res = with_state(|s| {
            let keys: Vec<String> = s.np.keys().filter(|k| only.as_ref().is_none_or(|o| o == *k)).cloned().collect();
            let targets: Vec<Value> = keys.iter().map(|k| np_target_json(s, k)).collect();
            let by_id: BTreeMap<u64, String> =
                s.np.iter().flat_map(|(k, (e, _))| e.ids.iter().map(move |i| (*i, k.clone()))).collect();
            Ok(json!({
                "available": true,
                "programs_compiled": true,
                "targets": targets,
                "events": events_for(s, "net", &by_id),
            }))
        });
        Ok(res.unwrap_or_else(unavailable))
    }

    // ---- LSM ------------------------------------------------------------

    pub fn lsm_apply(params: &Value) -> Result<Value, String> {
        let req: LsmApply = serde_json::from_value(params.clone()).map_err(|e| format!("invalid params: {e}"))?;
        let key = target_key(&req.target)?;
        let mode = parse_mode(&req.mode, req.lease_secs)?;
        let sp = &req.spec;
        if mode != Mode::Off && !(sp.deny_exec || sp.deny_wx || sp.restrict_devices || sp.restrict_writes) {
            return Err("enable at least one of deny_exec, deny_wx, restrict_devices, restrict_writes".into());
        }
        let active = rt::lsm_active();
        if matches!(mode, Mode::Enforce { .. }) && !active {
            return Err(format!(
                "lsm_inactive: refusing to enforce while `bpf` is not an active LSM ({}); add it to the \
                 kernel command line (lsm=...,bpf) and reboot",
                rt::lsm_list()
            ));
        }
        let mut devices = Vec::new();
        for d in DEFAULT_DEVICES.iter().copied().chain(sp.allow_devices.iter().map(String::as_str)) {
            let (kind, ma, mi) = parse_device(d)?;
            devices.push(dev_key(kind, ma, mi));
        }
        with_state(|s| {
            if mode == Mode::Off {
                if let Some((e, _)) = s.lsm.remove(&key) {
                    s.ebpf.lsm_remove(&e.ids).map_err(|e| format!("{e:#}"))?;
                }
                if s.lsm.is_empty() {
                    s.ebpf.lsm_detach();
                }
                return Ok(json!({ "target": key, "removed": true }));
            }
            let r = resolve(&req.target)?;
            let mut files = Vec::new();
            for p in &sp.allow_exec {
                let md = std::fs::metadata(inside(&r.root, p)).map_err(|e| format!("allow_exec `{p}`: {e}"))?;
                if !md.is_file() {
                    return Err(format!("allow_exec `{p}`: not a file"));
                }
                files.push((md.ino(), kdev(md.dev())));
            }
            let mut writable = Vec::new();
            for p in &sp.writable_paths {
                let md = std::fs::metadata(inside(&r.root, p)).map_err(|e| format!("writable_paths `{p}`: {e}"))?;
                writable.push(kdev(md.dev()));
            }
            writable.sort_unstable();
            writable.dedup();
            if let Some((old, _)) = s.lsm.get(&key) {
                if old.cgroup != r.cgroup {
                    let ids = old.ids.clone();
                    s.ebpf.lsm_remove(&ids).map_err(|e| format!("{e:#}"))?;
                    s.lsm.remove(&key);
                }
            }
            let (enforce, deadline) = lease_deadline(mode);
            let mut flags = 0;
            for (on, f) in [
                (sp.deny_exec, LSM_DENY_EXEC),
                (sp.deny_wx, LSM_DENY_WX),
                (sp.restrict_devices, LSM_RESTRICT_DEVICES),
                (sp.restrict_writes, LSM_RESTRICT_WRITES),
                (enforce, LSM_ENFORCE),
            ] {
                if on {
                    flags |= f;
                }
            }
            let policy = LsmPolicy { flags, _pad: 0, deadline_ns: deadline };
            s.ebpf.lsm_apply(&r.ids, policy, &files, &devices, &writable).map_err(|e| format!("{e:#}"))?;
            let spec = serde_json::to_value(sp).unwrap_or_default();
            let entry = Entry { cgroup: r.cgroup, ids: r.ids, enforce, deadline, lease_expired: false, spec };
            s.lsm.insert(key.clone(), (entry, policy));
            let mut v = lsm_target_json(s, &key);
            if !active {
                v = merge(v, json!({ "note": lsm_inactive_note() }));
            }
            Ok(v)
        })
    }

    fn lsm_inactive_note() -> String {
        format!(
            "lsm_inactive: hooks are attached but `bpf` is not an active LSM ({}), so they never run",
            rt::lsm_list()
        )
    }

    fn lsm_target_json(s: &mut State, key: &str) -> Value {
        let Some((e, _)) = s.lsm.get(key) else { return Value::Null };
        let ids = e.ids.clone();
        let base = merge(
            json!({
                "target": key,
                "cgroup": e.cgroup.display().to_string(),
                "cgroup_ids": e.ids,
            }),
            merge(mode_json(e), e.spec.clone()),
        );
        let counters: Vec<Value> = s
            .ebpf
            .lsm_counters(&ids)
            .into_iter()
            .map(|(a, denied, n)| json!({ "action": rt::lsm_action_name(a), "denied": denied, "count": n }))
            .collect();
        merge(base, json!({ "counters": counters }))
    }

    pub fn lsm_status(params: &Value) -> Result<Value, String> {
        let only = filter_key(params)?;
        let active = rt::lsm_active();
        let res = with_state(|s| {
            let keys: Vec<String> = s.lsm.keys().filter(|k| only.as_ref().is_none_or(|o| o == *k)).cloned().collect();
            let targets: Vec<Value> = keys.iter().map(|k| lsm_target_json(s, k)).collect();
            let by_id: BTreeMap<u64, String> =
                s.lsm.iter().flat_map(|(k, (e, _))| e.ids.iter().map(move |i| (*i, k.clone()))).collect();
            Ok(json!({
                "available": true,
                "programs_compiled": true,
                "lsm_active": active,
                "lsm_list": rt::lsm_list(),
                "hooks_attached": s.ebpf.lsm_attached(),
                "note": (!active).then(lsm_inactive_note),
                "targets": targets,
                "events": events_for(s, "lsm", &by_id),
            }))
        });
        Ok(res.unwrap_or_else(|e| merge(unavailable(e), json!({ "lsm_active": active, "lsm_list": rt::lsm_list() }))))
    }

    fn filter_key(params: &Value) -> Result<Option<String>, String> {
        let t: Target = serde_json::from_value(params.clone()).unwrap_or_default();
        if t.container.is_none() && t.cgroup.is_none() {
            return Ok(None);
        }
        target_key(&t).map(Some)
    }

    fn unavailable(reason: String) -> Value {
        json!({
            "available": false,
            "programs_compiled": rt::programs_compiled(),
            "reason": reason,
            "targets": [],
            "events": [],
        })
    }
}

#[cfg(not(all(target_os = "linux", feature = "ebpf")))]
mod imp {
    use super::*;

    const WHY: &str = "ebpf_unavailable: this guestkit build has no eBPF support (Linux + `ebpf` feature)";

    pub fn netpolicy_apply(_: &Value) -> Result<Value, String> {
        Err(WHY.into())
    }
    pub fn lsm_apply(_: &Value) -> Result<Value, String> {
        Err(WHY.into())
    }
    pub fn netpolicy_status(_: &Value) -> Result<Value, String> {
        Ok(json!({ "available": false, "programs_compiled": false, "reason": WHY, "targets": [], "events": [] }))
    }
    pub fn lsm_status(params: &Value) -> Result<Value, String> {
        netpolicy_status(params)
    }
}

pub use imp::{lsm_apply, lsm_status, netpolicy_apply, netpolicy_status};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_need_a_bounded_lease_to_enforce() {
        assert_eq!(parse_mode("audit", None).unwrap(), Mode::Audit);
        assert_eq!(parse_mode("off", None).unwrap(), Mode::Off);
        assert!(parse_mode("enforce", None).is_err());
        assert!(parse_mode("enforce", Some(0)).is_err());
        assert!(parse_mode("enforce", Some(MAX_LEASE_SECS + 1)).is_err());
        assert_eq!(parse_mode("enforce", Some(60)).unwrap(), Mode::Enforce { lease_secs: 60 });
        assert!(parse_mode("block", None).is_err());
    }

    #[test]
    fn device_rules() {
        assert_eq!(parse_device("c 1:3").unwrap(), (1, 1, 3));
        assert_eq!(parse_device("c 136:*").unwrap(), (1, 136, u32::MAX));
        assert_eq!(parse_device("b 8:0").unwrap(), (2, 8, 0));
        assert!(parse_device("x 1:3").is_err());
        assert!(parse_device("c 1").is_err());
        assert!(parse_device("c 1:3 extra").is_err());
        assert!(parse_device("c 5000:1").is_err());
        for d in DEFAULT_DEVICES {
            parse_device(d).unwrap();
        }
    }

    #[test]
    fn cgroup_v2_line() {
        let c = "12:pids:/legacy\n0::/system.slice/docker-abc.scope\n";
        assert_eq!(parse_cgroup_v2(c).as_deref(), Some("/system.slice/docker-abc.scope"));
        assert_eq!(parse_cgroup_v2("3:cpu:/x\n"), None);
    }

    #[test]
    fn cgroup_paths_stay_under_root() {
        assert_eq!(cgroup_dir("/sys/fs/cgroup/a/b").unwrap(), PathBuf::from("/sys/fs/cgroup/a/b"));
        assert_eq!(cgroup_dir("/system.slice/x.scope").unwrap(), PathBuf::from("/sys/fs/cgroup/system.slice/x.scope"));
        assert_eq!(cgroup_dir("a").unwrap(), PathBuf::from("/sys/fs/cgroup/a"));
        assert!(cgroup_dir("/sys/fs/cgroup/../etc").is_err());
        assert!(cgroup_dir("/").is_err());
        assert!(cgroup_dir("/sys/fs/cgroup").is_err());
    }

    #[test]
    fn target_keys() {
        let t = Target { container: Some("web".into()), cgroup: None };
        assert_eq!(target_key(&t).unwrap(), "container:web");
        let t = Target { container: None, cgroup: Some("gk/test".into()) };
        assert_eq!(target_key(&t).unwrap(), "cgroup:/sys/fs/cgroup/gk/test");
        assert!(target_key(&Target::default()).is_err());
        let both = Target { container: Some("a".into()), cgroup: Some("b".into()) };
        assert!(target_key(&both).is_err());
    }

    #[test]
    fn apply_params_default_to_audit_egress() {
        let p: NetpolicyApply = serde_json::from_value(json!({
            "container": "web",
            "rules": [{ "direction": "egress", "cidr": "10.0.0.0/8", "proto": "tcp", "port": 443 }]
        }))
        .unwrap();
        assert_eq!(p.mode, "audit");
        assert!(p.egress && !p.ingress);
        assert_eq!(p.rules[0].port, Some(443));
        let l: LsmApply = serde_json::from_value(json!({ "cgroup": "x", "deny_wx": true })).unwrap();
        assert_eq!(l.mode, "audit");
        assert!(l.spec.deny_wx && !l.spec.deny_exec);
    }
}
