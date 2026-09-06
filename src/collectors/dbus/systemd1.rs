// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! org.freedesktop.systemd1 D-Bus collector.

use crate::evidence::snapshot::{
    SystemdBootTimestamps, SystemdJob, SystemdManagerInfo, SystemdRuntimeInfo, SystemdRuntimeUnit,
};
use anyhow::{Context, Result};
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub fn collect_systemd_runtime() -> Option<SystemdRuntimeInfo> {
    collect_systemd_runtime_inner().ok()
}

fn collect_systemd_runtime_inner() -> Result<SystemdRuntimeInfo> {
    let conn = Connection::system().context("connect to system dbus")?;
    let proxy = manager_proxy(&conn)?;

    let manager = collect_manager_info(&proxy)?;
    let units = collect_list_units(&conn)?;

    let jobs_raw: Vec<OwnedValue> = proxy.call("ListJobs", &()).unwrap_or_default();
    let jobs = parse_jobs(&jobs_raw);

    let failed_count = units
        .iter()
        .filter(|u| u.active_state == "failed" || u.sub_state == "failed")
        .count();

    let manager_state = if manager.system_state.is_empty() {
        if failed_count > 0 {
            "degraded".to_string()
        } else {
            "running".to_string()
        }
    } else {
        manager.system_state.clone()
    };

    Ok(SystemdRuntimeInfo {
        manager_state,
        failed_unit_count: failed_count,
        manager: Some(manager),
        units,
        jobs,
    })
}

pub fn get_unit_detail(name: &str) -> Option<SystemdRuntimeUnit> {
    let conn = Connection::system().ok()?;
    let mut unit = SystemdRuntimeUnit {
        name: name.to_string(),
        ..Default::default()
    };
    enrich_unit(&conn, name, &mut unit);
    Some(unit)
}

pub fn get_unit_by_pid(pid: u32) -> Option<String> {
    let conn = Connection::system().ok()?;
    let proxy = manager_proxy(&conn).ok()?;
    proxy
        .call("GetUnitByPID", &(pid,))
        .ok()
        .and_then(|path: OwnedObjectPath| unit_name_from_path(&conn, path.as_str()))
}

fn collect_manager_info(proxy: &zbus::blocking::Proxy<'_>) -> Result<SystemdManagerInfo> {
    let system_state = get_prop_string(proxy, "SystemState");
    let n_failed_units = get_prop_u32(proxy, "NFailedUnits");
    let n_jobs = get_prop_u32(proxy, "NJobs");
    let architecture = get_prop_string(proxy, "Architecture");
    let virtualization = get_prop_string(proxy, "Virtualization");

    let boot_timestamps = SystemdBootTimestamps {
        firmware_us: get_timestamp_us(proxy, "FirmwareTimestamp"),
        loader_us: get_timestamp_us(proxy, "LoaderTimestamp"),
        kernel_us: get_timestamp_us(proxy, "KernelTimestamp"),
        initrd_us: get_timestamp_us(proxy, "InitRDTimestamp"),
        userspace_us: get_timestamp_us(proxy, "UserspaceTimestamp"),
        finish_us: get_timestamp_us(proxy, "FinishTimestamp"),
        security_start_us: get_timestamp_us(proxy, "SecurityStartTimestamp"),
        security_finish_us: get_timestamp_us(proxy, "SecurityFinishTimestamp"),
        units_load_start_us: get_timestamp_us(proxy, "UnitsLoadStartTimestamp"),
        units_load_finish_us: get_timestamp_us(proxy, "UnitsLoadFinishTimestamp"),
    };

    Ok(SystemdManagerInfo {
        system_state,
        n_failed_units,
        n_jobs,
        architecture,
        virtualization,
        boot_timestamps,
    })
}

fn collect_list_units(conn: &Connection) -> Result<Vec<SystemdRuntimeUnit>> {
    let proxy = manager_proxy(conn)?;
    let units_raw: Vec<OwnedValue> = proxy.call("ListUnits", &())?;
    parse_unit_list(conn, &units_raw)
}

#[allow(dead_code)]
fn list_units_by_patterns(conn: &Connection, patterns: &[&str]) -> Result<Vec<SystemdRuntimeUnit>> {
    let proxy = manager_proxy(conn)?;
    let patterns: Vec<&str> = patterns.to_vec();
    let units_raw: Vec<OwnedValue> =
        proxy.call("ListUnitsByPatterns", &(Vec::<&str>::new(), patterns))?;
    parse_unit_list(conn, &units_raw)
}

fn parse_unit_list(conn: &Connection, units_raw: &[OwnedValue]) -> Result<Vec<SystemdRuntimeUnit>> {
    let mut units = Vec::new();
    for entry in units_raw {
        if let Ok(row) = entry.downcast_ref::<zbus::zvariant::Structure>() {
            let fields = row.fields();
            if fields.len() < 5 {
                continue;
            }
            let name = value_to_string(&fields[0]);
            let description = value_to_string(&fields[1]);
            let load_state = value_to_string(&fields[2]);
            let active_state = value_to_string(&fields[3]);
            let sub_state = value_to_string(&fields[4]);

            let mut unit = SystemdRuntimeUnit {
                name: name.clone(),
                description,
                load_state,
                active_state,
                sub_state,
                ..Default::default()
            };

            enrich_unit(conn, &name, &mut unit);
            units.push(unit);
        }
    }
    Ok(units)
}

fn enrich_unit(conn: &Connection, name: &str, unit: &mut SystemdRuntimeUnit) {
    let Ok(manager) = manager_proxy(conn) else {
        return;
    };
    let unit_path = match manager.call::<_, _, OwnedObjectPath>("GetUnit", &(name,)) {
        Ok(path) => path.to_string(),
        Err(_) => return,
    };

    {
        let unit_proxy = match zbus::blocking::Proxy::new(
            conn,
            "org.freedesktop.systemd1",
            unit_path.as_str(),
            "org.freedesktop.systemd1.Unit",
        ) {
            Ok(proxy) => proxy,
            Err(_) => return,
        };

        unit.main_pid = get_prop_u32(&unit_proxy, "MainPID");
        unit.n_restarts = get_prop_u32(&unit_proxy, "NRestarts");
        unit.fragment_path = get_prop_string(&unit_proxy, "FragmentPath");
        unit.unit_file_state = get_prop_string(&unit_proxy, "UnitFileState");
        unit.exec_main_status = unit_proxy.get_property("ExecMainStatus").ok();
        unit.cgroup_path = unit_proxy.get_property("ControlGroup").ok();
        unit.following = unit_proxy.get_property("Following").ok();
        unit.source_path = get_prop_string_opt(&unit_proxy, "SourcePath");
        unit.drop_in_paths = unit_proxy
            .get_property::<Vec<String>>("DropInPaths")
            .unwrap_or_default();
        unit.need_daemon_reload = unit_proxy.get_property("NeedDaemonReload").unwrap_or(false);
        unit.can_start = unit_proxy.get_property("CanStart").unwrap_or(false);
        unit.can_stop = unit_proxy.get_property("CanStop").unwrap_or(false);
        unit.can_reload = unit_proxy.get_property("CanReload").unwrap_or(false);

        if let Ok(ts) = unit_proxy.get_property::<u64>("ExecMainStartTimestamp") {
            if ts > 0 {
                unit.exec_main_start_timestamp = Some(format!("{ts}"));
            }
        }
        if let Ok(ts) = unit_proxy.get_property::<u64>("ExecMainExitTimestamp") {
            if ts > 0 {
                unit.exec_main_exit_timestamp = Some(format!("{ts}"));
            }
        }
    }

    if name.ends_with(".service") {
        enrich_service_unit(conn, unit_path.as_str(), unit);
    }
}

fn enrich_service_unit(conn: &Connection, unit_path: &str, unit: &mut SystemdRuntimeUnit) {
    if let Ok(svc_proxy) = zbus::blocking::Proxy::new(
        conn,
        "org.freedesktop.systemd1",
        unit_path,
        "org.freedesktop.systemd1.Service",
    ) {
        unit.control_pid = Some(get_prop_u32(&svc_proxy, "ControlPID"));
        unit.exec_main_code = svc_proxy.get_property("ExecMainCode").ok();
        unit.restart = get_prop_string_opt(&svc_proxy, "Restart");
        unit.restart_usec = svc_proxy.get_property("RestartUSec").ok();
        unit.timeout_start_usec = svc_proxy.get_property("TimeoutStartUSec").ok();
        unit.timeout_stop_usec = svc_proxy.get_property("TimeoutStopUSec").ok();
        unit.watchdog_usec = svc_proxy.get_property("WatchdogUSec").ok();
        unit.oom_policy = get_prop_string_opt(&svc_proxy, "OOMPolicy");
        unit.result = get_prop_string_opt(&svc_proxy, "Result");
        unit.reload_result = get_prop_string_opt(&svc_proxy, "ReloadResult");
        unit.clean_result = get_prop_string_opt(&svc_proxy, "CleanResult");
    }
}

fn manager_proxy(conn: &Connection) -> Result<zbus::blocking::Proxy<'_>> {
    Ok(zbus::blocking::Proxy::new(
        conn,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )?)
}

fn get_prop_string(proxy: &zbus::blocking::Proxy<'_>, name: &str) -> String {
    proxy.get_property::<String>(name).unwrap_or_default()
}

fn get_prop_string_opt(proxy: &zbus::blocking::Proxy<'_>, name: &str) -> Option<String> {
    proxy.get_property(name).ok()
}

fn get_prop_u32(proxy: &zbus::blocking::Proxy<'_>, name: &str) -> u32 {
    proxy.get_property::<u32>(name).unwrap_or(0)
}

fn get_timestamp_us(proxy: &zbus::blocking::Proxy<'_>, name: &str) -> Option<u64> {
    let raw: OwnedValue = proxy.get_property(name).ok()?;
    if let Ok(ts) = raw.downcast_ref::<u64>() {
        return Some(ts);
    }
    if let Ok(row) = raw.downcast_ref::<zbus::zvariant::Structure>() {
        let fields = row.fields();
        if fields.len() >= 2 {
            let secs = fields[0].downcast_ref::<u64>().unwrap_or(0);
            let usecs = fields[1].downcast_ref::<u64>().unwrap_or(0);
            return Some(secs * 1_000_000 + usecs);
        }
    }
    None
}

fn unit_name_from_path(conn: &Connection, path: &str) -> Option<String> {
    let unit_proxy = zbus::blocking::Proxy::new(
        conn,
        "org.freedesktop.systemd1",
        path,
        "org.freedesktop.systemd1.Unit",
    )
    .ok()?;
    let id = get_prop_string(&unit_proxy, "Id");
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn parse_jobs(raw: &[OwnedValue]) -> Vec<SystemdJob> {
    let mut out = Vec::new();
    for entry in raw {
        if let Ok(row) = entry.downcast_ref::<zbus::zvariant::Structure>() {
            let fields = row.fields();
            if fields.len() < 5 {
                continue;
            }
            out.push(SystemdJob {
                id: value_to_u32(&fields[0]),
                unit: value_to_string(&fields[1]),
                job_type: value_to_string(&fields[2]),
                state: value_to_string(&fields[4]),
            });
        }
    }
    out
}

fn value_to_string(v: &Value<'_>) -> String {
    v.downcast_ref::<String>()
        .unwrap_or_else(|_| format!("{v:?}"))
}

fn value_to_u32(v: &Value<'_>) -> u32 {
    v.downcast_ref::<u32>().unwrap_or(0)
}

/// List units matching failed state.
pub fn list_failed_units(runtime: &SystemdRuntimeInfo) -> Vec<SystemdRuntimeUnit> {
    runtime
        .units
        .iter()
        .filter(|u| u.active_state == "failed" || u.sub_state == "failed")
        .cloned()
        .collect()
}

/// Restart a unit via D-Bus (privileged).
pub fn restart_unit(name: &str) -> Result<String> {
    let conn = Connection::system().context("connect to system dbus")?;
    let proxy = manager_proxy(&conn)?;
    let _job: u32 = proxy
        .call("RestartUnit", &(name, "replace"))
        .context("RestartUnit D-Bus call")?;
    Ok(format!("restarted {name}"))
}
