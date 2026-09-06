// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! org.freedesktop.login1 D-Bus collector.

use anyhow::{Context, Result};
use guestkit_agent_protocol::{LoggedInUser, LoginState, ShutdownInhibitor};
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

fn field_string(fields: &[zbus::zvariant::Value<'_>], idx: usize) -> String {
    fields
        .get(idx)
        .and_then(|v| v.downcast_ref::<String>().ok())
        .unwrap_or_default()
}

pub fn collect_login_state() -> Result<LoginState> {
    let conn = Connection::system().context("connect to system dbus")?;
    let manager = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;

    let idle_hint: bool = manager.get_property("IdleHint").unwrap_or(false);

    let sessions_raw: Vec<OwnedValue> = manager.call("ListSessions", &()).unwrap_or_default();
    let mut logged_in_users = Vec::new();
    for entry in sessions_raw {
        if let Ok(row) = entry.downcast_ref::<zbus::zvariant::Structure>() {
            let fields = row.fields();
            if fields.len() < 4 {
                continue;
            }
            logged_in_users.push(LoggedInUser {
                name: field_string(fields, 2),
                seat: field_string(fields, 3),
                session_type: "unknown".into(),
                active: true,
            });
        }
    }

    let inhibitors_raw: Vec<OwnedValue> = manager.call("ListInhibitors", &()).unwrap_or_default();
    let mut inhibitors = Vec::new();
    for entry in inhibitors_raw {
        if let Ok(row) = entry.downcast_ref::<zbus::zvariant::Structure>() {
            let fields = row.fields();
            if fields.len() < 6 {
                continue;
            }
            inhibitors.push(ShutdownInhibitor {
                what: field_string(fields, 2),
                who: field_string(fields, 3),
                why: field_string(fields, 4),
                mode: field_string(fields, 5),
            });
        }
    }

    Ok(LoginState {
        logged_in_users,
        inhibitors,
        idle_hint,
    })
}
