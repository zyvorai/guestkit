// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! QEMU guest-agent protocol compatibility (`virsh qemu-agent-command` / libvirt).

use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

static EXEC_JOBS: LazyLock<Mutex<HashMap<u64, ExecJob>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_PID: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(1000));
static FS_FROZEN: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));
// Marker shadow created by a Windows guest-fsfreeze-freeze, torn down by the
// matching thaw. See guest_fsfreeze_freeze/thaw below.
static VSS_SHADOW_ID: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));
use std::cell::Cell;

thread_local! {
    static QGA_DELIMITED_RESPONSE: Cell<bool> = const { Cell::new(false) };
}

/// True when the last QGA response must be prefixed with the 0xFF sync sentinel.
pub fn take_delimited_response() -> bool {
    QGA_DELIMITED_RESPONSE.replace(false)
}

#[derive(Debug)]
struct ExecJob {
    out: Vec<u8>,
    err: Vec<u8>,
    exited: bool,
    exitcode: i64,
}

/// True if payload is a QGA `{"execute":...}` command (not JSON-RPC).
pub fn is_qga_request(bytes: &[u8]) -> bool {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return false;
    };
    v.get("execute").and_then(|e| e.as_str()).is_some()
}

/// Handle one QGA request frame; returns full JSON response bytes.
pub fn handle(bytes: &[u8]) -> Vec<u8> {
    let req: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return qga_error(None, &format!("invalid JSON: {e}")),
    };
    let execute = match req.get("execute").and_then(|e| e.as_str()) {
        Some(c) => c,
        None => return qga_error(req.get("id").cloned(), "missing execute"),
    };
    let args = req.get("arguments").cloned().unwrap_or(json!({}));
    let id = req.get("id").cloned();

    let result = match execute {
        "guest-ping" => Ok(json!({})),
        "guest-sync" => guest_sync(&args),
        "guest-sync-delimited" => guest_sync_delimited(&args),
        "guest-info" => Ok(guest_info()),
        "guest-get-osinfo" => guest_get_osinfo(),
        "guest-get-fsinfo" => guest_get_fsinfo(),
        "guest-get-users" => guest_get_users(),
        "guest-get-timezone" => guest_get_timezone(),
        "guest-get-time" => guest_get_time(),
        "guest-set-time" => guest_set_time(&args),
        "guest-fstrim" => guest_fstrim(),
        "guest-fsfreeze-status" => guest_fsfreeze_status(),
        "guest-fsfreeze-freeze" => guest_fsfreeze_freeze(),
        "guest-fsfreeze-thaw" => guest_fsfreeze_thaw(),
        "guest-network-get-interfaces" => guest_network_get_interfaces(),
        "guest-get-host-name" => guest_get_host_name(),
        "guest-exec" => guest_exec(&args),
        "guest-exec-status" => guest_exec_status(&args),
        "guest-shutdown" => guest_shutdown(&args),
        "guest-reboot" => guest_reboot(),
        "guestkit-get-evidence" => guestkit_get_evidence(),
        "guestkit-doctor" => guestkit_doctor(&args),
        "guestkit-get-capabilities" => guestkit_get_capabilities(),
        "guestkit-get-version" => guestkit_get_version(),
        "guestkit-run-fix-plan" => guestkit_run_fix_plan(&args),
        "guestkit-migrate-score" => guestkit_migrate_score(&args),
        "guestkit-get-metrics" => guestkit_get_metrics(),
        "guestkit-get-filesystem" => guestkit_get_filesystem(),
        "guestkit-get-guest-health" => guestkit_get_guest_health(),
        "guestkit-get-guest-info" => guestkit_get_guest_info(),
        "guestkit-get-systemd-units" => guestkit_get_systemd_units(),
        "guestkit-get-systemd-unit" => guestkit_get_systemd_unit(&args),
        "guestkit-get-systemd-events" => guestkit_get_systemd_events(&args),
        "guestkit-get-processes" => guestkit_get_processes(),
        "guestkit-get-journal-slice" => guestkit_get_journal_slice(&args),
        "guestkit-get-failed-units" => guestkit_get_failed_units(),
        "guestkit-get-boot-analysis" => guestkit_get_boot_analysis(),
        "guestkit-get-login-state" => guestkit_get_login_state(),
        "guestkit-get-dns-state" => guestkit_get_dns_state(),
        "guestkit-get-snapshot-readiness" => guestkit_get_snapshot_readiness(),
        "guestkit-exec" => guestkit_exec(&args),
        "guestkit-enable-rdp" => guestkit_enable_rdp(),
        "guestkit-disable-rdp" => guestkit_disable_rdp(),
        "guest-suspend-disk" | "guest-suspend-ram" | "guest-suspend-hybrid" => Ok(json!({})),
        other => Err(format!("unsupported QGA command: {other}")),
    };

    match result {
        Ok(ret) => qga_ok(id, ret),
        Err(msg) => qga_error(id, &msg),
    }
}

/// If `bytes` is a QGA `guestkit-rpc` passthrough envelope, return the inner
/// JSON-RPC request bytes. This lets a QGA client reach ANY agent RPC method
/// (all of `AgentCapabilities::standard`), not just the bespoke `guestkit-*`
/// shims — the request is routed through the full [`RequestHandler`] dispatch.
///
/// Envelope: `{"execute":"guestkit-rpc","arguments":{"method":"<m>","params":{…},"id":<id>}}`.
pub fn extract_rpc_passthrough(bytes: &[u8]) -> Option<Vec<u8>> {
    let req: Value = serde_json::from_slice(bytes).ok()?;
    if req.get("execute").and_then(|e| e.as_str()) != Some("guestkit-rpc") {
        return None;
    }
    let args = req.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let method = args.get("method").and_then(|m| m.as_str())?;
    let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
    let id = args.get("id").cloned().unwrap_or_else(|| json!(1));
    let inner = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    serde_json::to_vec(&inner).ok()
}

/// Wrap a JSON-RPC response as a QGA reply (`{"return":…}` or `{"error":…}`).
pub fn wrap_qga_response(resp: &guestkit_agent_protocol::JsonRpcResponse) -> Vec<u8> {
    match &resp.error {
        Some(err) => qga_error(resp.id.clone(), &err.message),
        None => qga_ok(resp.id.clone(), resp.result.clone().unwrap_or(Value::Null)),
    }
}

fn qga_ok(id: Option<Value>, ret: Value) -> Vec<u8> {
    let mut out = json!({ "return": ret });
    if let Some(i) = id {
        out["id"] = i;
    }
    serde_json::to_vec(&out).unwrap_or_else(|_| br#"{"return":{}}"#.to_vec())
}

fn qga_error(id: Option<Value>, desc: &str) -> Vec<u8> {
    let mut out = json!({
        "error": {
            "class": "GenericError",
            "desc": desc
        }
    });
    if let Some(i) = id {
        out["id"] = i;
    }
    serde_json::to_vec(&out)
        .unwrap_or_else(|_| br#"{"error":{"class":"GenericError","desc":"error"}}"#.to_vec())
}

fn guest_sync(args: &Value) -> Result<Value, String> {
    Ok(json!(args.get("id").and_then(|v| v.as_i64()).unwrap_or(0)))
}

fn guest_sync_delimited(args: &Value) -> Result<Value, String> {
    QGA_DELIMITED_RESPONSE.set(true);
    Ok(json!(args.get("id").and_then(|v| v.as_i64()).unwrap_or(0)))
}

fn guest_info() -> Value {
    // KubeVirt probes guest-info.version and rejects non-semver strings; report a current
    // qemu-ga release while GuestKit identity lives in guest-get-osinfo / JSON-RPC metadata.
    json!({
        "version": "6.2.0",
        "supported_commands": [
            "guest-ping",
            "guest-sync",
            "guest-sync-delimited",
            "guest-info",
            "guest-get-osinfo",
            "guest-get-fsinfo",
            "guest-get-users",
            "guest-get-timezone",
            "guest-get-time",
            "guest-set-time",
            "guest-fstrim",
            "guest-fsfreeze-status",
            "guest-fsfreeze-freeze",
            "guest-fsfreeze-thaw",
            "guest-network-get-interfaces",
            "guest-get-host-name",
            "guest-exec",
            "guest-exec-status",
            "guest-shutdown",
            "guest-reboot",
            "guestkit-rpc",
            "guestkit-get-evidence",
            "guestkit-doctor",
            "guestkit-get-capabilities",
            "guestkit-get-version",
            "guestkit-run-fix-plan",
            "guestkit-migrate-score",
            "guestkit-get-metrics",
            "guestkit-get-filesystem",
            "guestkit-get-guest-health",
            "guestkit-get-guest-info",
            "guestkit-get-systemd-units",
            "guestkit-get-systemd-unit",
            "guestkit-get-systemd-events",
            "guestkit-get-processes",
            "guestkit-get-journal-slice",
            "guestkit-get-failed-units",
            "guestkit-get-boot-analysis",
            "guestkit-exec",
            "guestkit-enable-rdp",
            "guestkit-disable-rdp"
        ],
        "supported_events": []
    })
}

fn guest_get_osinfo() -> Result<Value, String> {
    let mut id = String::new();
    let mut name = String::new();
    let mut pretty = String::new();
    let mut version = String::new();
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim_matches('"');
            match k {
                "ID" => id = v.to_string(),
                "NAME" => name = v.to_string(),
                "PRETTY_NAME" => pretty = v.to_string(),
                "VERSION_ID" => version = v.to_string(),
                _ => {}
            }
        }
    }
    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let machine = Command::new("uname")
        .arg("-m")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    Ok(json!({
        "id": id,
        "name": name,
        "pretty-name": if pretty.is_empty() { &name } else { &pretty },
        "version": version,
        "kernel-release": kernel,
        "machine": machine
    }))
}

fn guest_get_fsinfo() -> Result<Value, String> {
    let mut mounts = Vec::new();
    let Ok(content) = std::fs::read_to_string("/proc/mounts") else {
        return Ok(json!([]));
    };
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let mountpoint = parts[1];
        if mountpoint == "/proc"
            || mountpoint == "/sys"
            || mountpoint == "/dev"
            || mountpoint.starts_with("/proc/")
            || mountpoint.starts_with("/sys/")
        {
            continue;
        }
        let fstype = parts[2];
        if matches!(
            fstype,
            "proc" | "sysfs" | "devtmpfs" | "tmpfs" | "cgroup2" | "cgroup"
        ) {
            continue;
        }
        let (total_bytes, used_bytes, avail_bytes, inodes_total, inodes_free) =
            statvfs_usage(mountpoint);
        let use_percent = if total_bytes > 0 {
            (used_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };
        let inode_use_percent = if inodes_total > 0 {
            ((inodes_total - inodes_free) as f64 / inodes_total as f64) * 100.0
        } else {
            0.0
        };
        let disk_serial = disk_serial_for_device(parts[0]);
        mounts.push(json!({
            "name": parts[0],
            "mountpoint": mountpoint,
            "type": fstype,
            "total-bytes": total_bytes,
            "used-bytes": used_bytes,
            "avail-bytes": avail_bytes,
            "use-percent": use_percent,
            "inodes-total": inodes_total,
            "inodes-free": inodes_free,
            "inode-use-percent": inode_use_percent,
            "disk-serial": disk_serial,
            "disk": [{
                "bus-type": "virtio",
                "serial": disk_serial,
                "dev": parts[0],
                "total-bytes": total_bytes,
                "used-bytes": used_bytes,
                "avail-bytes": avail_bytes
            }]
        }));
    }
    Ok(json!(mounts))
}

#[cfg(target_os = "linux")]
fn statvfs_usage(path: &str) -> (u64, u64, u64, u64, u64) {
    use nix::sys::statvfs::statvfs;
    use std::path::Path;
    let Ok(st) = statvfs(Path::new(path)) else {
        return (0, 0, 0, 0, 0);
    };
    let total = st.blocks() * st.fragment_size();
    let free = st.blocks_free() * st.fragment_size();
    let avail = st.blocks_available() * st.fragment_size();
    let used = total.saturating_sub(free);
    let inodes_total = st.files();
    let inodes_free = st.files_free();
    (total, used, avail, inodes_total, inodes_free)
}

#[cfg(not(target_os = "linux"))]
fn statvfs_usage(_path: &str) -> (u64, u64, u64, u64, u64) {
    (0, 0, 0, 0, 0)
}

fn disk_serial_for_device(dev_path: &str) -> String {
    let name = dev_path.trim_start_matches("/dev/");
    let base = name
        .trim_end_matches(char::is_numeric)
        .trim_end_matches("p");
    std::fs::read_to_string(format!("/sys/block/{base}/serial"))
        .or_else(|_| std::fs::read_to_string(format!("/sys/block/{name}/serial")))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Normalized filesystem mounts for VMRogue `/guest-filesystem`.
pub fn filesystem_mounts_normalized() -> Result<Value, String> {
    let raw = guest_get_fsinfo()?;
    let mut mounts = Vec::new();
    if let Some(arr) = raw.as_array() {
        for entry in arr {
            let mountpoint = entry
                .get("mountpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("/")
                .to_string();
            let total = entry
                .get("total-bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let used = entry
                .get("used-bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let avail = entry
                .get("avail-bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(total.saturating_sub(used));
            let use_percent = entry
                .get("use-percent")
                .and_then(|v| v.as_f64())
                .unwrap_or_else(|| {
                    if total > 0 {
                        (used as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    }
                });
            mounts.push(json!({
                "mount": mountpoint,
                "filesystem": entry.get("name").and_then(|v| v.as_str()),
                "fstype": entry.get("type").and_then(|v| v.as_str()),
                "size_bytes": total,
                "used_bytes": used,
                "avail_bytes": avail,
                "use_percent": use_percent,
                "inodes_total": entry.get("inodes-total").and_then(|v| v.as_u64()).unwrap_or(0),
                "inodes_free": entry.get("inodes-free").and_then(|v| v.as_u64()).unwrap_or(0),
                "disk_serial": entry.get("disk-serial").and_then(|v| v.as_str()).unwrap_or(""),
            }));
        }
    }
    Ok(json!({ "mounts": mounts, "source": "guestkit-get-filesystem" }))
}

fn guest_get_users() -> Result<Value, String> {
    let out = Command::new("who")
        .output()
        .map_err(|e| format!("who: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let users: Vec<Value> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let user = parts.next()?;
            let _tty = parts.next()?;
            let _time = parts.next()?;
            Some(json!({
                "user": user,
                "login-time": now,
                "domain": ""
            }))
        })
        .collect();
    Ok(json!(users))
}

fn guest_get_timezone() -> Result<Value, String> {
    let tz = std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| {
            p.to_string_lossy()
                .strip_prefix("/usr/share/zoneinfo/")
                .map(|s| s.to_string())
        })
        .or_else(|| {
            std::fs::read_to_string("/etc/timezone")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "UTC".into());
    Ok(json!({ "timezone": tz }))
}

fn guest_get_time() -> Result<Value, String> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    Ok(json!({ "time": secs }))
}

fn guest_set_time(args: &Value) -> Result<Value, String> {
    let secs = args
        .get("time")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "missing time".to_string())?;
    let nanos = args
        .get("nanoseconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cmd = format!("date -u --set=@{}.{:09}", secs, nanos % 1_000_000_000);
    let status = Command::new("sh").arg("-c").arg(&cmd).status();
    match status {
        Ok(s) if s.success() => Ok(json!({})),
        Ok(s) => Err(format!("date failed: {s}")),
        Err(e) => Err(format!("set time: {e}")),
    }
}

// QGA `guest-fsfreeze-status` returns the GuestFsfreezeStatus enum as a bare
// STRING ("thawed" | "frozen"), not an object. KubeVirt's snapshot controller
// parses the string; an object reply ({"return":{"frozen":0}}) makes it fail with
// "failed to strip FSFreeze status" and wedges every online snapshot InProgress.
fn guest_fsfreeze_status() -> Result<Value, String> {
    let frozen = FS_FROZEN.lock().map_err(|e| e.to_string())?;
    Ok(json!(if *frozen { "frozen" } else { "thawed" }))
}

// QGA `guest-fsfreeze-freeze` returns the NUMBER of frozen filesystems as a bare
// integer (wrapped by the caller as {"return": N}).
//
// Regression: this unconditionally shelled out to the Linux `fsfreeze`
// binary, which doesn't exist on Windows — confirmed live, every quiesced
// snapshot of a Windows guest failed with "fsfreeze: program not found".
// Windows has no filesystem-level freeze syscall; the platform-correct
// equivalent is a VSS shadow copy, which already exists for guestkit's own
// orchestrated snapshot flow (snapshot::vss) — reuse it here instead of
// re-implementing COM VSS from scratch.
fn guest_fsfreeze_freeze() -> Result<Value, String> {
    if cfg!(target_os = "windows") {
        let result = crate::agent::snapshot::vss::create_marker_shadow("C:")
            .map_err(|e| format!("VSS freeze failed: {e}"))?;
        *VSS_SHADOW_ID.lock().map_err(|e| e.to_string())? = result.shadow_id;
        *FS_FROZEN.lock().map_err(|e| e.to_string())? = true;
        return Ok(json!(1));
    }
    let status = Command::new("fsfreeze")
        .arg("-f")
        .arg("/")
        .status()
        .map_err(|e| format!("fsfreeze: {e}"))?;
    if !status.success() {
        return Err(format!("fsfreeze -f / failed: {status}"));
    }
    *FS_FROZEN.lock().map_err(|e| e.to_string())? = true;
    Ok(json!(1))
}

// QGA `guest-fsfreeze-thaw` returns the NUMBER of thawed filesystems as a bare
// integer. Mirrors guest_fsfreeze_freeze's Windows/Linux split.
fn guest_fsfreeze_thaw() -> Result<Value, String> {
    if cfg!(target_os = "windows") {
        let shadow_id = VSS_SHADOW_ID.lock().map_err(|e| e.to_string())?.take();
        if let Some(id) = shadow_id {
            crate::agent::snapshot::vss::delete_marker_shadow(&id)
                .map_err(|e| format!("VSS cleanup: {e}"))?;
        }
        *FS_FROZEN.lock().map_err(|e| e.to_string())? = false;
        return Ok(json!(1));
    }
    let status = Command::new("fsfreeze")
        .arg("-u")
        .arg("/")
        .status()
        .map_err(|e| format!("fsfreeze: {e}"))?;
    if !status.success() {
        return Err(format!("fsfreeze -u / failed: {status}"));
    }
    *FS_FROZEN.lock().map_err(|e| e.to_string())? = false;
    Ok(json!(1))
}

pub fn freeze_fs() -> Result<(), String> {
    guest_fsfreeze_freeze().map(|_| ())
}

pub fn thaw_fs() -> Result<(), String> {
    guest_fsfreeze_thaw().map(|_| ())
}

/// Current freeze state as tracked by the QGA fsfreeze path.
pub fn fs_frozen() -> bool {
    FS_FROZEN.lock().map(|f| *f).unwrap_or(false)
}

fn guest_get_host_name() -> Result<Value, String> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "localhost".into());
    Ok(json!({ "host-name": hostname }))
}

fn guest_network_get_interfaces() -> Result<Value, String> {
    if let Ok(out) = Command::new("ip").args(["-j", "addr", "show"]).output() {
        if out.status.success() {
            if let Ok(interfaces) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(arr) = interfaces.as_array() {
                    let mapped: Vec<Value> = arr.iter().filter_map(map_ip_json_interface).collect();
                    if !mapped.is_empty() {
                        return Ok(json!(mapped));
                    }
                }
            }
        }
    }
    Ok(json!(collect_network_from_sysfs()))
}

fn map_ip_json_interface(iface: &Value) -> Option<Value> {
    let name = iface.get("ifname")?.as_str()?;
    if name == "lo" {
        return None;
    }
    let mac = iface
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut addrs = Vec::new();
    if let Some(addr_info) = iface.get("addr_info").and_then(|v| v.as_array()) {
        for info in addr_info {
            let family = info.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let ip_type = match family {
                "inet" => "ipv4",
                "inet6" => "ipv6",
                _ => continue,
            };
            if let Some(local) = info.get("local").and_then(|v| v.as_str()) {
                let prefix = info.get("prefixlen").and_then(|v| v.as_u64()).unwrap_or(0);
                addrs.push(json!({
                    "ip-address": local,
                    "ip-address-type": ip_type,
                    "prefix": prefix
                }));
            }
        }
    }
    Some(json!({
        "name": name,
        "hardware-address": mac,
        "ip-addresses": addrs
    }))
}

fn collect_network_from_sysfs() -> Vec<Value> {
    let mut interfaces = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return interfaces;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let mac = std::fs::read_to_string(format!("/sys/class/net/{name}/address"))
            .unwrap_or_default()
            .trim()
            .to_string();
        interfaces.push(json!({
            "name": name,
            "hardware-address": mac,
            "ip-addresses": []
        }));
    }
    interfaces
}

fn guest_fstrim() -> Result<Value, String> {
    let out = Command::new("fstrim")
        .arg("-a")
        .arg("-v")
        .output()
        .map_err(|e| format!("fstrim: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut results = Vec::new();
    for line in text.lines() {
        if line.contains(':') {
            let mount = line.split(':').next().unwrap_or("/").trim();
            results.push(json!({
                "path": mount,
                "minimum": 0,
                "error": if out.status.success() { 0 } else { 1 }
            }));
        }
    }
    if results.is_empty() && out.status.success() {
        results.push(json!({ "path": "/", "minimum": 0, "error": 0 }));
    }
    Ok(json!(results))
}

fn guest_exec(args: &Value) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing path".to_string())?;
    let capture = args
        .get("capture-output")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut cmd = Command::new(path);
    if let Some(arr) = args.get("arg").and_then(|v| v.as_array()) {
        for a in arr {
            if let Some(s) = a.as_str() {
                cmd.arg(s);
            }
        }
    }
    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut pid_guard = NEXT_PID.lock().map_err(|e| e.to_string())?;
    let pid = *pid_guard;
    *pid_guard += 1;
    drop(pid_guard);

    if capture {
        let output = cmd.output().map_err(|e| format!("exec: {e}"))?;
        let job = ExecJob {
            out: output.stdout,
            err: output.stderr,
            exited: true,
            exitcode: output.status.code().unwrap_or(1) as i64,
        };
        EXEC_JOBS
            .lock()
            .map_err(|e| e.to_string())?
            .insert(pid, job);
    } else {
        cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
        EXEC_JOBS.lock().map_err(|e| e.to_string())?.insert(
            pid,
            ExecJob {
                out: Vec::new(),
                err: Vec::new(),
                exited: true,
                exitcode: 0,
            },
        );
    }
    Ok(json!({ "pid": pid }))
}

fn guest_exec_status(args: &Value) -> Result<Value, String> {
    let pid = args
        .get("pid")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "missing pid".to_string())?;
    let jobs = EXEC_JOBS.lock().map_err(|e| e.to_string())?;
    let job = jobs.get(&pid).ok_or_else(|| format!("unknown pid {pid}"))?;
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut ret = json!({
        "exited": job.exited,
        "exitcode": job.exitcode,
    });
    if !job.out.is_empty() {
        ret["out-data"] = Value::String(STANDARD.encode(&job.out));
    }
    if !job.err.is_empty() {
        ret["err-data"] = Value::String(STANDARD.encode(&job.err));
    }
    Ok(ret)
}

fn guest_shutdown(args: &Value) -> Result<Value, String> {
    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("powerdown");
    let cmd = match mode {
        "halt" => "systemctl halt",
        _ => "systemctl poweroff",
    };
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn()
        .map_err(|e| format!("shutdown: {e}"))?;
    Ok(json!({}))
}

fn guest_reboot() -> Result<Value, String> {
    Command::new("systemctl")
        .args(["reboot"])
        .spawn()
        .or_else(|_| Command::new("reboot").spawn())
        .map_err(|e| format!("reboot: {e}"))?;
    Ok(json!({}))
}

fn guestkit_get_evidence() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    serde_json::to_value(evidence).map_err(|e| format!("serialize evidence: {e}"))
}

fn guestkit_doctor(args: &Value) -> Result<Value, String> {
    let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("kvm");
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let boot_target = crate::boot::BootTarget::parse(target);
    let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
    Ok(json!({
        "evidence": serde_json::to_value(&evidence).map_err(|e| e.to_string())?,
        "boot_report": serde_json::to_value(&boot_report).map_err(|e| e.to_string())?,
    }))
}

fn guestkit_get_capabilities() -> Result<Value, String> {
    let caps = guestkit_agent_protocol::AgentCapabilities::standard(crate::VERSION);
    serde_json::to_value(caps).map_err(|e| format!("serialize capabilities: {e}"))
}

fn guestkit_get_version() -> Result<Value, String> {
    Ok(json!({
        "version": crate::VERSION,
        "protocol": guestkit_agent_protocol::PROTOCOL_VERSION,
    }))
}

fn guestkit_run_fix_plan(args: &Value) -> Result<Value, String> {
    let plan_value = args
        .get("plan")
        .cloned()
        .ok_or_else(|| "missing required argument: plan".to_string())?;
    let plan: crate::cli::plan::FixPlan =
        serde_json::from_value(plan_value).map_err(|e| format!("invalid plan: {e}"))?;
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let executor = crate::cli::plan::LivePlanExecutor::new(dry_run);
    let result = executor.apply(&plan).map_err(|e| e.to_string())?;
    Ok(json!({
        "plan_id": format!("{}-{}", plan.vm.replace('/', "_"), plan.generated.timestamp()),
        "dry_run": dry_run,
        "result": result,
    }))
}

fn guestkit_migrate_score(args: &Value) -> Result<Value, String> {
    let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("kvm");
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let boot_target = crate::boot::BootTarget::parse(target);
    let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
    let report =
        crate::cli::migrate::plan::compute_migration_score(&evidence, &boot_report, target);
    serde_json::to_value(report).map_err(|e| e.to_string())
}

fn guestkit_get_metrics() -> Result<Value, String> {
    let metrics = crate::metrics::collect_metrics_live();
    serde_json::to_value(metrics).map_err(|e| e.to_string())
}

fn guestkit_get_filesystem() -> Result<Value, String> {
    filesystem_mounts_normalized()
}

fn guestkit_get_guest_health() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let health = crate::health::build_guest_health(&evidence);
    serde_json::to_value(health).map_err(|e| e.to_string())
}

fn guestkit_get_guest_info() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let info = crate::health::build_guest_info(&evidence);
    serde_json::to_value(info).map_err(|e| e.to_string())
}

fn guestkit_get_systemd_units() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let units = evidence
        .systemd
        .as_ref()
        .and_then(|s| s.runtime.as_ref())
        .map(|r| r.units.clone())
        .unwrap_or_default();
    serde_json::to_value(units).map_err(|e| e.to_string())
}

fn guestkit_get_systemd_unit(args: &Value) -> Result<Value, String> {
    let unit = args
        .get("unit")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing unit".to_string())?;
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    // NOTE: must stay .or_else (lazy) — the fallback makes a live D-Bus call
    // (get_unit_detail); .or() would run it on every request even when
    // build_service_health already found the unit from evidence.
    #[allow(clippy::unnecessary_lazy_evaluations)]
    let detail = crate::health::build_service_health(unit, &evidence).or_else(|| {
        #[cfg(target_os = "linux")]
        {
            crate::collectors::dbus::get_unit_detail(unit).map(|u| {
                guestkit_agent_protocol::ServiceHealth {
                    name: u.name,
                    state: u.active_state,
                    sub_state: u.sub_state,
                    main_pid: u.main_pid,
                    exit_code: u.exec_main_status,
                    restart_count: u.n_restarts,
                    last_failure: None,
                    journal_cursor: None,
                    actions: vec!["view_logs".into(), "restart_unit".into()],
                }
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    });
    serde_json::to_value(detail).map_err(|e| e.to_string())
}

fn guestkit_get_systemd_events(args: &Value) -> Result<Value, String> {
    #[cfg(target_os = "linux")]
    {
        let cursor = args.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0);
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
        let (next_cursor, events) = if cursor > 0 {
            crate::collectors::dbus::systemd_events::get_events_since(cursor)
        } else {
            let events = crate::collectors::dbus::systemd_events::recent_events(limit);
            let (c, _) = crate::collectors::dbus::systemd_events::get_events_since(0);
            (c, events)
        };
        Ok(json!({ "cursor": next_cursor, "events": events }))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        Ok(json!({ "cursor": 0, "events": [] }))
    }
}

fn guestkit_get_processes() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let process = evidence
        .process
        .clone()
        .unwrap_or_else(crate::collectors::process::collect_process_evidence);
    serde_json::to_value(process).map_err(|e| e.to_string())
}

fn guestkit_get_journal_slice(args: &Value) -> Result<Value, String> {
    let unit = args.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(200) as usize;
    let boot = args
        .get("boot")
        .and_then(|v| v.as_str())
        .unwrap_or("current");
    let slice = crate::journal::live::collect_journal_slice_boot(unit, limit, boot);
    serde_json::to_value(slice).map_err(|e| e.to_string())
}

fn guestkit_get_failed_units() -> Result<Value, String> {
    let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
    let failed = crate::health::list_failed_units_from_evidence(&evidence);
    serde_json::to_value(failed).map_err(|e| e.to_string())
}

fn guestkit_get_boot_analysis() -> Result<Value, String> {
    let analysis = crate::boot::live::collect_boot_analysis();
    serde_json::to_value(analysis).map_err(|e| e.to_string())
}

fn guestkit_get_login_state() -> Result<Value, String> {
    let state = crate::collectors::dbus::collect_login_state_safe();
    serde_json::to_value(state).map_err(|e| e.to_string())
}

fn guestkit_get_dns_state() -> Result<Value, String> {
    let dns = crate::collectors::dbus::collect_dns_health_safe();
    serde_json::to_value(dns).map_err(|e| e.to_string())
}

fn guestkit_get_snapshot_readiness() -> Result<Value, String> {
    let report = crate::agent::snapshot_hooks::build_snapshot_readiness_report();
    serde_json::to_value(report).map_err(|e| e.to_string())
}

fn guestkit_exec(args: &Value) -> Result<Value, String> {
    crate::agent::exec::exec_sync_qga(args)
}

fn guestkit_enable_rdp() -> Result<Value, String> {
    crate::agent::rdp::enable_rdp()
}

fn guestkit_disable_rdp() -> Result<Value, String> {
    crate::agent::rdp::disable_rdp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_ping_round_trip() {
        let resp = handle(br#"{"execute":"guest-ping"}"#);
        let v: Value = serde_json::from_slice(&resp).unwrap();
        assert!(v.get("return").is_some());
    }

    #[test]
    fn detects_qga_vs_jsonrpc() {
        assert!(is_qga_request(br#"{"execute":"guest-ping"}"#));
        assert!(!is_qga_request(
            br#"{"jsonrpc":"2.0","method":"guestkit.ping","id":1}"#
        ));
    }

    #[test]
    fn rpc_passthrough_extracts_inner_request() {
        let env = br#"{"execute":"guestkit-rpc","arguments":{"method":"guestkit.getVersion","params":{}}}"#;
        let inner = extract_rpc_passthrough(env).expect("passthrough envelope");
        let v: Value = serde_json::from_slice(&inner).unwrap();
        assert_eq!(v["method"], "guestkit.getVersion");
        assert_eq!(v["jsonrpc"], "2.0");
        // A plain QGA command is not a passthrough.
        assert!(extract_rpc_passthrough(br#"{"execute":"guest-ping"}"#).is_none());
    }

    #[test]
    fn rpc_passthrough_round_trips_through_full_handler() {
        // Any method reachable through the full handler is now reachable via QGA.
        let handler = crate::agent::handler::RequestHandler::new();
        let env = br#"{"execute":"guestkit-rpc","arguments":{"method":"guestkit.getCapabilities","params":{},"id":7}}"#;
        let out = handler.handle_frame(env);
        let v: Value = serde_json::from_slice(&out).unwrap();
        let ret = v.get("return").expect("QGA return");
        let methods = ret
            .get("methods")
            .and_then(|m| m.as_array())
            .expect("methods array");
        assert!(
            methods.len() > 50,
            "capabilities advertise the full method set"
        );
    }

    #[test]
    fn fsfreeze_status_is_qga_string_enum_not_object() {
        // KubeVirt's snapshot controller parses guest-fsfreeze-status as a bare
        // enum STRING ("thawed"|"frozen"); an object reply wedges online snapshots.
        let resp = handle(br#"{"execute":"guest-fsfreeze-status"}"#);
        let v: Value = serde_json::from_slice(&resp).unwrap();
        let ret = v.get("return").expect("has return");
        assert!(
            ret.is_string(),
            "guest-fsfreeze-status return must be a string, got {ret}"
        );
        assert_eq!(ret.as_str().unwrap(), "thawed");
    }
}
