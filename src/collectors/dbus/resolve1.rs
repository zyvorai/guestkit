// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! org.freedesktop.resolve1 D-Bus collector.

use anyhow::{Context, Result};
use guestkit_agent_protocol::DnsHealth;
use zbus::blocking::Connection;

pub fn collect_dns_health() -> Result<DnsHealth> {
    let conn = Connection::system().context("connect to system dbus")?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.resolve1",
        "/org/freedesktop/resolve1",
        "org.freedesktop.resolve1.Manager",
    )?;

    let dns_servers: Vec<String> = proxy.get_property("DNS").unwrap_or_default();
    let search_domains: Vec<String> = proxy.get_property("Domains").unwrap_or_default();
    let dnssec: String = proxy.get_property("DNSSEC").unwrap_or_default();
    let llmnr: String = proxy.get_property("LLMNR").unwrap_or_default();
    let mdns: String = proxy.get_property("MulticastDNS").unwrap_or_default();

    let mut errors = Vec::new();
    if dns_servers.is_empty() {
        errors.push("no DNS servers configured in systemd-resolved".into());
    }

    Ok(DnsHealth {
        dns_servers,
        search_domains,
        dnssec,
        llmnr,
        mdns,
        errors,
    })
}
