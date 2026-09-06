// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! org.freedesktop.timedate1 D-Bus collector.

use anyhow::{Context, Result};
use guestkit_agent_protocol::TimedateHealth;
use zbus::blocking::Connection;

pub fn collect_timedate_health() -> Result<TimedateHealth> {
    let conn = Connection::system().context("connect to system dbus")?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.timedate1",
        "/org/freedesktop/timedate1",
        "org.freedesktop.timedate1",
    )?;

    let timezone: String = proxy.get_property("Timezone").unwrap_or_default();
    let ntp_enabled: bool = proxy.get_property("NTP").unwrap_or(false);
    let ntp_synchronized: bool = proxy.get_property("NTPSynchronized").unwrap_or(false);
    let rtc_in_local_time: bool = proxy.get_property("LocalRTC").unwrap_or(false);

    Ok(TimedateHealth {
        timezone,
        ntp_enabled,
        ntp_synchronized,
        rtc_in_local_time,
        drift_secs: None,
    })
}
