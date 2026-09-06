// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Enhanced inspection operations for comprehensive guest analysis
use crate::core::Result;
use crate::guestfs::Guestfs;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::str::FromStr;

// Common file paths
const RESOLV_CONF: &str = "/etc/resolv.conf";
const PASSWD: &str = "/etc/passwd";
const SSHD_CONFIG: &str = "/etc/ssh/sshd_config";
const SELINUX_CONFIG: &str = "/etc/selinux/config";
const TIMEZONE: &str = "/etc/timezone";
const LOCALTIME: &str = "/etc/localtime";
const LOCALE_CONF: &str = "/etc/locale.conf";
const DEFAULT_LOCALE: &str = "/etc/default/locale";
const SYSTEMD_SERVICES_DIR: &str = "/etc/systemd/system/multi-user.target.wants";
const SYSTEMD_TIMERS_DIR: &str = "/etc/systemd/system/timers.target.wants";
const FSTAB: &str = "/etc/fstab";
const _HOSTNAME: &str = "/etc/hostname";
const HOSTS: &str = "/etc/hosts";
const SYSCTL_CONF: &str = "/etc/sysctl.conf";
const CRONTAB: &str = "/etc/crontab";

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_address: Vec<String>,
    pub mac_address: String,
    pub dhcp: bool,
    pub dns_servers: Vec<String>,
}

/// User account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub username: String,
    pub uid: String,
    pub gid: String,
    pub home: String,
    pub shell: String,
}

/// System service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemService {
    pub name: String,
    pub enabled: bool,
    pub state: String,
}

/// LVM Volume Group information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeGroup {
    pub name: String,
    pub pv_count: usize,
    pub lv_count: usize,
    pub size: String,
}

/// LVM Logical Volume information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalVolume {
    pub name: String,
    pub vg_name: String,
    pub size: String,
    pub path: String,
}

/// LVM information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LVMInfo {
    pub physical_volumes: Vec<String>,
    pub volume_groups: Vec<VolumeGroup>,
    pub logical_volumes: Vec<LogicalVolume>,
}

/// RAID array information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAIDArray {
    pub device: String,
    pub level: String,
    pub status: String,
    pub devices: Vec<String>,
    pub active_devices: usize,
    pub total_devices: usize,
}

/// Boot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootConfig {
    pub bootloader: String,
    pub default_entry: String,
    pub timeout: String,
    pub kernel_cmdline: String,
}

/// Certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub path: String,
    pub subject: String,
    pub issuer: String,
    pub expiry: String,
}

impl Guestfs {
    /// Execute a function with automatic mount/unmount handling
    ///
    /// This helper ensures the root filesystem is mounted before executing the function
    /// and properly unmounts it afterwards if it wasn't already mounted.
    ///
    /// # Arguments
    /// * `root` - Root device to mount (e.g., "/dev/sda1")
    /// * `f` - Function to execute while filesystem is mounted
    ///
    /// # Returns
    /// Result from the function execution
    pub fn with_mount<F, T>(&mut self, root: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted {
            self.mount_ro(root, "/")?;
        }

        let result = f(self);

        if !was_mounted {
            let _ = self.umount("/");
        }

        result
    }

    /// Inspect network configuration
    ///
    /// Detects network interfaces from multiple configuration formats:
    /// - Debian/Ubuntu: /etc/network/interfaces
    /// - RHEL/CentOS/Fedora: /etc/sysconfig/network-scripts/ifcfg-*
    /// - Ubuntu 17.10+: /etc/netplan/*.yaml
    /// - NetworkManager: /etc/NetworkManager/system-connections/*.nmconnection
    /// - systemd-networkd: /etc/systemd/network/*.network
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Vector of NetworkInterface with name, IPs, MAC, DHCP, and DNS servers
    pub fn inspect_network(&mut self, root: &str) -> Result<Vec<NetworkInterface>> {
        self.with_mount(root, |guestfs| {
            let mut interfaces = Vec::new();

            // 1. Check /etc/network/interfaces (Debian/Ubuntu)
            if let Ok(content) = guestfs.cat("/etc/network/interfaces") {
                interfaces.extend(guestfs.parse_debian_interfaces(&content));
            }

            // 2. Check /etc/sysconfig/network-scripts/ (RHEL/CentOS/Fedora)
            if let Ok(files) = guestfs.ls("/etc/sysconfig/network-scripts") {
                for file in files.iter().filter(|f| f.starts_with("ifcfg-")) {
                    let path = format!("/etc/sysconfig/network-scripts/{}", file);
                    if let Ok(content) = guestfs.cat(&path) {
                        if let Some(iface) = guestfs.parse_rhel_interface(&content, file) {
                            interfaces.push(iface);
                        }
                    }
                }
            }

            // 3. Check netplan (Ubuntu 17.10+)
            if guestfs.is_dir("/etc/netplan").unwrap_or(false) {
                if let Ok(files) = guestfs.ls("/etc/netplan") {
                    for file in files
                        .iter()
                        .filter(|f| f.ends_with(".yaml") || f.ends_with(".yml"))
                    {
                        let path = format!("/etc/netplan/{}", file);
                        if let Ok(content) = guestfs.cat(&path) {
                            interfaces.extend(guestfs.parse_netplan(&content));
                        }
                    }
                }
            }

            // 4. Check NetworkManager (modern distros)
            if guestfs
                .is_dir("/etc/NetworkManager/system-connections")
                .unwrap_or(false)
            {
                if let Ok(files) = guestfs.ls("/etc/NetworkManager/system-connections") {
                    for file in files.iter().filter(|f| f.ends_with(".nmconnection")) {
                        let path = format!("/etc/NetworkManager/system-connections/{}", file);
                        if let Ok(content) = guestfs.cat(&path) {
                            if let Some(iface) = guestfs.parse_networkmanager(&content, file) {
                                interfaces.push(iface);
                            }
                        }
                    }
                }
            }

            // 5. Check systemd-networkd
            if guestfs.is_dir("/etc/systemd/network").unwrap_or(false) {
                if let Ok(files) = guestfs.ls("/etc/systemd/network") {
                    for file in files.iter().filter(|f| f.ends_with(".network")) {
                        let path = format!("/etc/systemd/network/{}", file);
                        if let Ok(content) = guestfs.cat(&path) {
                            interfaces.extend(guestfs.parse_systemd_network(&content, file));
                        }
                    }
                }
            }

            Ok(interfaces)
        })
    }

    fn parse_debian_interfaces(&self, content: &str) -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        let mut current_iface: Option<NetworkInterface> = None;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("iface ") {
                if let Some(iface) = current_iface.take() {
                    interfaces.push(iface);
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let dhcp = match parts[3] {
                        "dhcp" => true,
                        "static" | "manual" => false,
                        _ => false,
                    };
                    current_iface = Some(NetworkInterface {
                        name: parts[1].to_string(),
                        ip_address: Vec::new(),
                        mac_address: String::new(),
                        dhcp,
                        dns_servers: Vec::new(),
                    });
                }
            } else if let Some(ref mut iface) = current_iface {
                if line.starts_with("address ") {
                    if let Some(addr) = line.split_whitespace().nth(1) {
                        if !addr.is_empty() {
                            iface.ip_address.push(addr.to_string());
                        }
                    }
                } else if line.starts_with("hwaddress ") || line.starts_with("hwaddr ") {
                    if let Some(mac) = line.split_whitespace().nth(1) {
                        if !mac.is_empty() {
                            iface.mac_address = mac.to_string();
                        }
                    }
                } else if line.starts_with("dns-nameservers ") {
                    for server in line.split_whitespace().skip(1) {
                        iface.dns_servers.push(server.to_string());
                    }
                }
            }
        }
        if let Some(iface) = current_iface {
            interfaces.push(iface);
        }
        interfaces
    }

    fn parse_rhel_interface(&self, content: &str, filename: &str) -> Option<NetworkInterface> {
        let mut iface = NetworkInterface {
            name: filename
                .strip_prefix("ifcfg-")
                .unwrap_or(filename)
                .to_string(),
            ip_address: Vec::new(),
            mac_address: String::new(),
            dhcp: false,
            dns_servers: Vec::new(),
        };
        let mut index = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim_matches('"').trim();
                match key {
                    "BOOTPROTO" => {
                        iface.dhcp = matches!(value.to_lowercase().as_str(), "dhcp" | "bootp")
                    }
                    "IPADDR" => iface.ip_address.push(value.to_string()),
                    k if k.starts_with("IPADDR") => {
                        if let Some(suffix) = k.get(6..) {
                            if let Ok(num) = usize::from_str(suffix) {
                                if num >= index && num < 256 {
                                    while iface.ip_address.len() <= num {
                                        iface.ip_address.push(String::new());
                                    }
                                    iface.ip_address[num] = value.to_string();
                                    index = num.saturating_add(1);
                                }
                            } else {
                                iface.ip_address.push(value.to_string());
                            }
                        }
                    }
                    "HWADDR" | "MACADDR" => iface.mac_address = value.to_string(),
                    k if k.starts_with("DNS") => {
                        if let Some(suffix) = k.get(3..) {
                            if suffix.parse::<usize>().is_ok() {
                                iface.dns_servers.push(value.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(iface)
    }

    fn parse_netplan(&self, content: &str) -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        if let Ok(yaml) = serde_yaml::from_str::<Value>(content) {
            if let Some(network) = yaml.get("network") {
                if let Some(ethernets) = network.get("ethernets") {
                    if let Some(mapping) = ethernets.as_mapping() {
                        for (name, iface_val) in mapping {
                            if let Some(name_str) = name.as_str() {
                                let mut intf = NetworkInterface {
                                    name: name_str.to_string(),
                                    ip_address: Vec::new(),
                                    mac_address: String::new(),
                                    dhcp: false,
                                    dns_servers: Vec::new(),
                                };
                                if let Some(iface) = iface_val.as_mapping() {
                                    if let Some(dhcp4) = iface.get("dhcp4") {
                                        intf.dhcp = dhcp4.as_bool().unwrap_or(false);
                                    } else if let Some(dhcp) = iface.get("dhcp") {
                                        intf.dhcp = dhcp.as_bool().unwrap_or(false);
                                    }
                                    if let Some(addresses) = iface.get("addresses") {
                                        if let Some(addrs) = addresses.as_sequence() {
                                            for addr in addrs {
                                                if let Some(s) = addr.as_str() {
                                                    intf.ip_address.push(s.to_string());
                                                }
                                            }
                                        }
                                    }
                                    if let Some(match_obj) = iface.get("match") {
                                        if let Some(mac) = match_obj.get("macaddress") {
                                            intf.mac_address =
                                                mac.as_str().unwrap_or("").to_string();
                                        }
                                    } else if let Some(mac) = iface.get("macaddress") {
                                        intf.mac_address = mac.as_str().unwrap_or("").to_string();
                                    }
                                    if let Some(nameservers) = iface.get("nameservers") {
                                        if let Some(addresses) = nameservers.get("addresses") {
                                            if let Some(addrs) = addresses.as_sequence() {
                                                for addr in addrs {
                                                    if let Some(s) = addr.as_str() {
                                                        intf.dns_servers.push(s.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                interfaces.push(intf);
                            }
                        }
                    }
                }
            }
        }
        interfaces
    }

    fn parse_networkmanager(&self, content: &str, filename: &str) -> Option<NetworkInterface> {
        let mut iface = NetworkInterface {
            name: filename
                .strip_suffix(".nmconnection")
                .unwrap_or(filename)
                .to_string(),
            ip_address: Vec::new(),
            mac_address: String::new(),
            dhcp: false,
            dns_servers: Vec::new(),
        };
        let mut current_section = "";
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim();
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match current_section {
                    "connection" if key == "interface-name" => {
                        iface.name = value.to_string();
                    }
                    "ethernet" | "wifi" if key == "mac-address" => {
                        iface.mac_address = value.to_string();
                    }
                    "ipv4" => {
                        if key == "method" {
                            iface.dhcp = value == "auto";
                        } else if key.starts_with("address") {
                            let parts: Vec<&str> = value.split(',').collect();
                            if !parts.is_empty() {
                                let ip = parts[0].split('/').next().unwrap_or("").to_string();
                                if !ip.is_empty() {
                                    iface.ip_address.push(ip);
                                }
                            }
                        } else if key == "dns" {
                            for s in value.split(';') {
                                let s = s.trim();
                                if !s.is_empty() {
                                    iface.dns_servers.push(s.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(iface)
    }

    fn parse_systemd_network(&self, content: &str, filename: &str) -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        let mut current_section = "";
        let mut name = String::new();
        let mut mac_address = String::new();
        let mut dhcp = false;
        let mut ip_address = Vec::new();
        let mut dns_servers = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                // If we have data from previous sections, create interface
                if !name.is_empty() {
                    interfaces.push(NetworkInterface {
                        name: name.clone(),
                        ip_address: ip_address.clone(),
                        mac_address: mac_address.clone(),
                        dhcp,
                        dns_servers: dns_servers.clone(),
                    });
                    // Reset for next potential interface
                    name.clear();
                    mac_address.clear();
                    dhcp = false;
                    ip_address.clear();
                    dns_servers.clear();
                }
                current_section = &line[1..line.len() - 1];
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match current_section {
                    "Match" => {
                        if key == "Name" {
                            name = value.to_string();
                        } else if key == "MACAddress" {
                            mac_address = value.to_string();
                        }
                    }
                    "Network" => {
                        if key == "DHCP" {
                            dhcp = matches!(value, "yes" | "ipv4" | "true");
                        } else if key == "Address" {
                            let ip = value.split('/').next().unwrap_or("").to_string();
                            if !ip.is_empty() {
                                ip_address.push(ip);
                            }
                        } else if key == "DNS" {
                            for server in value.split_whitespace() {
                                dns_servers.push(server.to_string());
                            }
                        }
                    }
                    "Address" if key == "Address" => {
                        let ip = value.split('/').next().unwrap_or("").to_string();
                        if !ip.is_empty() {
                            ip_address.push(ip);
                        }
                    }
                    _ => {}
                }
            }
        }
        // After loop, add if data present
        if !name.is_empty() {
            interfaces.push(NetworkInterface {
                name,
                ip_address,
                mac_address,
                dhcp,
                dns_servers,
            });
        } else if interfaces.is_empty() {
            // If no interfaces parsed, create one with filename as fallback
            let name = filename
                .strip_suffix(".network")
                .unwrap_or(filename)
                .to_string();
            if !name.is_empty() {
                interfaces.push(NetworkInterface {
                    name,
                    ip_address: Vec::new(),
                    mac_address: String::new(),
                    dhcp: false,
                    dns_servers: Vec::new(),
                });
            }
        }
        interfaces
    }

    /// Get DNS configuration
    ///
    /// Extracts DNS servers from multiple sources:
    /// - /etc/resolv.conf (standard)
    /// - systemd-resolved configuration (when resolv.conf points to loopback)
    /// - Network interface configurations (fallback)
    /// - NetworkManager global DNS settings
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Deduplicated and sorted list of DNS server addresses
    pub fn inspect_dns(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut dns_servers = Vec::new();

            // Parse /etc/resolv.conf
            let mut is_loopback_only = true;
            let mut has_resolv_conf = false;
            if let Ok(content) = guestfs.cat(RESOLV_CONF) {
                has_resolv_conf = true;
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("nameserver ") {
                        if let Some(server) = line.split_whitespace().nth(1) {
                            dns_servers.push(server.to_string());
                            if !server.starts_with("127.") && server != "::1" {
                                is_loopback_only = false;
                            }
                        }
                    }
                }
            }

            // If resolv.conf points to loopback (e.g., 127.0.0.53 for systemd-resolved), parse resolved.conf
            if has_resolv_conf
                && is_loopback_only
                && guestfs
                    .exists("/etc/systemd/resolved.conf")
                    .unwrap_or(false)
            {
                // Parse main resolved.conf
                if let Ok(content) = guestfs.cat("/etc/systemd/resolved.conf") {
                    let mut parsed = guestfs.parse_systemd_resolved_content(&content);
                    dns_servers.append(&mut parsed.dns);
                    dns_servers.append(&mut parsed.fallback_dns);
                }

                // Parse drop-ins in /etc/systemd/resolved.conf.d/
                if guestfs
                    .is_dir("/etc/systemd/resolved.conf.d")
                    .unwrap_or(false)
                {
                    if let Ok(files) = guestfs.ls("/etc/systemd/resolved.conf.d") {
                        let mut conf_files: Vec<String> =
                            files.into_iter().filter(|f| f.ends_with(".conf")).collect();
                        conf_files.sort();
                        for file in conf_files {
                            let path = format!("/etc/systemd/resolved.conf.d/{}", file);
                            if let Ok(content) = guestfs.cat(&path) {
                                let mut parsed = guestfs.parse_systemd_resolved_content(&content);
                                dns_servers.append(&mut parsed.dns);
                                dns_servers.append(&mut parsed.fallback_dns);
                            }
                        }
                    }
                }
            } else if !has_resolv_conf || dns_servers.is_empty() {
                // If no resolv.conf or empty, collect from network configs
                // Note: We need to temporarily unmount to call inspect_network
                let was_mounted = guestfs.mounted.contains_key("/");
                if was_mounted {
                    let _ = guestfs.umount("/");
                }

                if let Ok(interfaces) = guestfs.inspect_network(root) {
                    for iface in interfaces {
                        dns_servers.extend(iface.dns_servers);
                    }
                }

                // Re-mount if we unmounted
                if was_mounted {
                    guestfs.mount_ro(root, "/")?;
                }

                // For NetworkManager global DNS
                if guestfs
                    .exists("/etc/NetworkManager/NetworkManager.conf")
                    .unwrap_or(false)
                {
                    if let Ok(content) = guestfs.cat("/etc/NetworkManager/NetworkManager.conf") {
                        let mut in_dns_section = false;
                        for line in content.lines() {
                            let line = line.trim();
                            if line == "[dns]" {
                                in_dns_section = true;
                            } else if in_dns_section && line.starts_with('[') {
                                in_dns_section = false;
                            } else if in_dns_section && line.starts_with("dns=") {
                                let value =
                                    line.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
                                for server in value.split(';') {
                                    let server = server.trim();
                                    if !server.is_empty() {
                                        dns_servers.push(server.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            dns_servers.sort();
            dns_servers.dedup();
            Ok(dns_servers)
        })
    }

    fn parse_systemd_resolved_content(&self, content: &str) -> ParsedResolved {
        let mut parsed = ParsedResolved {
            dns: Vec::new(),
            fallback_dns: Vec::new(),
        };
        let mut current_section = "";
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = &line[1..line.len() - 1];
                continue;
            }
            if current_section == "Resolve" {
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "DNS" => {
                            for server in value.split_whitespace() {
                                // Extract just the IP address, ignoring port, interface, SNI
                                let ip = server.split([':', '%', '#']).next().unwrap_or("");
                                if !ip.is_empty() {
                                    parsed.dns.push(ip.to_string());
                                }
                            }
                        }
                        "FallbackDNS" => {
                            for server in value.split_whitespace() {
                                let ip = server.split([':', '%', '#']).next().unwrap_or("");
                                if !ip.is_empty() {
                                    parsed.fallback_dns.push(ip.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        parsed
    }

    /// List user accounts
    pub fn inspect_users(&mut self, root: &str) -> Result<Vec<UserAccount>> {
        self.with_mount(root, |guestfs| {
            let mut users = Vec::new();
            if let Ok(content) = guestfs.cat(PASSWD) {
                for line in content.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 7 {
                        users.push(UserAccount {
                            username: parts[0].to_string(),
                            uid: parts[2].to_string(),
                            gid: parts[3].to_string(),
                            home: parts[5].to_string(),
                            shell: parts[6].to_string(),
                        });
                    }
                }
            }
            Ok(users)
        })
    }

    /// Get SSH configuration
    pub fn inspect_ssh_config(&mut self, root: &str) -> Result<HashMap<String, String>> {
        self.with_mount(root, |guestfs| {
            let mut config = HashMap::new();
            if let Ok(content) = guestfs.cat(SSHD_CONFIG) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once(char::is_whitespace) {
                        config.insert(key.to_string(), value.trim().to_string());
                    }
                }
            }
            Ok(config)
        })
    }

    /// Check SELinux status
    pub fn inspect_selinux(&mut self, root: &str) -> Result<String> {
        self.with_mount(root, |guestfs| {
            if let Ok(content) = guestfs.cat(SELINUX_CONFIG) {
                for line in content.lines() {
                    if line.starts_with("SELINUX=") {
                        return Ok(line.split('=').nth(1).unwrap_or("unknown").to_string());
                    }
                }
                Ok("unknown".to_string())
            } else {
                Ok("disabled".to_string())
            }
        })
    }

    /// List systemd services
    pub fn inspect_systemd_services(&mut self, root: &str) -> Result<Vec<SystemService>> {
        self.with_mount(root, |guestfs| {
            let mut services = Vec::new();
            // Check for enabled services in /etc/systemd/system
            if let Ok(links) = guestfs.ls(SYSTEMD_SERVICES_DIR) {
                for link in links {
                    services.push(SystemService {
                        name: link.strip_suffix(".service").unwrap_or(&link).to_string(),
                        enabled: true,
                        state: "enabled".to_string(),
                    });
                }
            }
            Ok(services)
        })
    }

    /// Get timezone information
    pub fn inspect_timezone(&mut self, root: &str) -> Result<String> {
        self.with_mount(root, |guestfs| {
            if let Ok(content) = guestfs.cat(TIMEZONE) {
                Ok(content.trim().to_string())
            } else if guestfs.is_symlink(LOCALTIME).unwrap_or(false) {
                if let Ok(target) = guestfs.readlink(LOCALTIME) {
                    // Extract timezone from path like /usr/share/zoneinfo/America/New_York
                    Ok(target
                        .strip_prefix("/usr/share/zoneinfo/")
                        .unwrap_or(&target)
                        .to_string())
                } else {
                    Ok("unknown".to_string())
                }
            } else {
                Ok("unknown".to_string())
            }
        })
    }

    /// Get locale information
    pub fn inspect_locale(&mut self, root: &str) -> Result<String> {
        self.with_mount(root, |guestfs| {
            if let Ok(content) = guestfs.cat(LOCALE_CONF) {
                for line in content.lines() {
                    if line.starts_with("LANG=") {
                        return Ok(line.split('=').nth(1).unwrap_or("unknown").to_string());
                    }
                }
                Ok("unknown".to_string())
            } else if let Ok(content) = guestfs.cat(DEFAULT_LOCALE) {
                for line in content.lines() {
                    if line.starts_with("LANG=") {
                        return Ok(line
                            .split('=')
                            .nth(1)
                            .unwrap_or("unknown")
                            .trim_matches('"')
                            .to_string());
                    }
                }
                Ok("unknown".to_string())
            } else {
                Ok("unknown".to_string())
            }
        })
    }

    /// Detect LVM configuration
    pub fn inspect_lvm(&mut self, _root: &str) -> Result<LVMInfo> {
        let mut lvm_info = LVMInfo {
            physical_volumes: Vec::new(),
            volume_groups: Vec::new(),
            logical_volumes: Vec::new(),
        };

        // Get physical volumes
        if let Ok(pvs) = self.pvs() {
            lvm_info.physical_volumes = pvs;
        }

        // Get volume groups with basic info
        if let Ok(vgs) = self.vgs() {
            for vg in vgs {
                let vg_info = VolumeGroup {
                    name: vg,
                    pv_count: 0, // Will be counted from PVs
                    lv_count: 0, // Will be counted from LVs
                    size: "Unknown".to_string(),
                };
                lvm_info.volume_groups.push(vg_info);
            }
        }

        // Get logical volumes with basic info
        if let Ok(lvs) = self.lvs() {
            for lv_path in lvs {
                // Parse LV path to get VG and LV names
                // Format is usually /dev/mapper/vgname-lvname or /dev/vgname/lvname
                let lv_name = lv_path
                    .split('/')
                    .next_back()
                    .unwrap_or(&lv_path)
                    .to_string();

                // Try to get VG name from LV
                let vg_name = if lv_path.contains("/mapper/") {
                    // For /dev/mapper/vg-lv format
                    lv_name.split('-').next().unwrap_or("unknown").to_string()
                } else if lv_path.starts_with("/dev/") {
                    // For /dev/vg/lv format
                    lv_path.split('/').nth(2).unwrap_or("unknown").to_string()
                } else {
                    "unknown".to_string()
                };

                let lv_info = LogicalVolume {
                    name: lv_name,
                    vg_name: vg_name.clone(),
                    size: "Unknown".to_string(),
                    path: lv_path,
                };
                lvm_info.logical_volumes.push(lv_info);

                // Update VG counts
                if let Some(vg) = lvm_info
                    .volume_groups
                    .iter_mut()
                    .find(|v| v.name == vg_name)
                {
                    vg.lv_count += 1;
                }
            }
        }

        Ok(lvm_info)
    }

    /// Inspect RAID arrays
    pub fn inspect_raid(&mut self, root: &str) -> Result<Vec<RAIDArray>> {
        let mut raids = Vec::new();

        self.with_mount(root, |guestfs| {
            // Read /proc/mdstat to get RAID information
            if let Ok(content) = guestfs.read_file("/proc/mdstat") {
                let content_str = String::from_utf8_lossy(&content);
                let mut current_array: Option<RAIDArray> = None;

                for line in content_str.lines() {
                    let line = line.trim();

                    // Skip header and empty lines
                    if line.is_empty()
                        || line.starts_with("Personalities")
                        || line.starts_with("unused devices")
                    {
                        continue;
                    }

                    // New array line (e.g., "md0 : active raid1 sda1[0] sdb1[1]")
                    if line.starts_with("md") {
                        // Save previous array if any
                        if let Some(array) = current_array.take() {
                            raids.push(array);
                        }

                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            let device = format!("/dev/{}", parts[0]);
                            let status = parts[2].to_string(); // active/inactive
                            let level = parts[3].to_string(); // raid0, raid1, raid5, etc.

                            // Extract device list
                            let devices: Vec<String> = parts
                                .iter()
                                .skip(4)
                                .filter_map(|s| {
                                    // Format: sda1[0] or sdb1[1](F) for failed
                                    let dev = s.split('[').next()?.to_string();
                                    Some(format!("/dev/{}", dev))
                                })
                                .collect();

                            let dev_count = devices.len();
                            current_array = Some(RAIDArray {
                                device,
                                level,
                                status,
                                devices,
                                active_devices: dev_count,
                                total_devices: dev_count,
                            });
                        }
                    }
                    // Status line (e.g., "      123456 blocks [2/2] [UU]")
                    else if let Some(ref mut array) = current_array {
                        if line.contains("[") && line.contains("]") {
                            // Extract [n/m] status
                            if let Some(start) = line.rfind('[') {
                                if let Some(end) = line.rfind(']') {
                                    let status_part = &line[start + 1..end];
                                    if status_part.contains('/') {
                                        let nums: Vec<&str> = status_part.split('/').collect();
                                        if nums.len() == 2 {
                                            array.active_devices = nums[0].parse().unwrap_or(0);
                                            array.total_devices = nums[1].parse().unwrap_or(0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Save last array
                if let Some(array) = current_array {
                    raids.push(array);
                }
            }

            Ok(raids)
        })
    }

    /// Detect cloud-init
    pub fn inspect_cloud_init(&mut self, root: &str) -> Result<bool> {
        self.with_mount(root, |guestfs| {
            Ok(guestfs.exists("/etc/cloud/cloud.cfg").unwrap_or(false)
                || guestfs.exists("/usr/bin/cloud-init").unwrap_or(false))
        })
    }

    /// Get language runtime versions
    pub fn inspect_runtimes(&mut self, root: &str) -> Result<HashMap<String, String>> {
        self.with_mount(root, |guestfs| {
            let mut runtimes = HashMap::new();
            let installed = "installed".to_string();
            // Python
            for py in &["python3", "python", "python2"] {
                let path = format!("/usr/bin/{}", py);
                if guestfs.exists(&path).unwrap_or(false) {
                    runtimes.insert(py.to_string(), installed.clone());
                }
            }
            // Check other runtimes
            let checks = [
                ("nodejs", "/usr/bin/node"),
                ("ruby", "/usr/bin/ruby"),
                ("java", "/usr/bin/java"),
                ("go", "/usr/bin/go"),
                ("perl", "/usr/bin/perl"),
            ];
            for (name, path) in &checks {
                if guestfs.exists(path).unwrap_or(false) {
                    runtimes.insert(name.to_string(), installed.clone());
                }
            }
            Ok(runtimes)
        })
    }

    /// Detect container runtimes
    pub fn inspect_container_runtimes(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut runtimes = Vec::new();
            if guestfs.exists("/usr/bin/docker").unwrap_or(false) {
                runtimes.push("docker".to_string());
            }
            if guestfs.exists("/usr/bin/podman").unwrap_or(false) {
                runtimes.push("podman".to_string());
            }
            if guestfs.exists("/usr/bin/containerd").unwrap_or(false) {
                runtimes.push("containerd".to_string());
            }
            if guestfs.exists("/usr/bin/crio").unwrap_or(false)
                || guestfs.exists("/usr/bin/cri-o").unwrap_or(false)
            {
                runtimes.push("cri-o".to_string());
            }
            Ok(runtimes)
        })
    }

    /// List cron jobs
    pub fn inspect_cron(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut cron_jobs = Vec::new();
            // System crontab
            if let Ok(content) = guestfs.cat(CRONTAB) {
                for line in content.lines() {
                    let line = line.trim();
                    if !line.is_empty() && !line.starts_with('#') {
                        cron_jobs.push(line.to_string());
                    }
                }
            }
            // Cron directories
            for dir in &[
                "/etc/cron.d",
                "/etc/cron.daily",
                "/etc/cron.hourly",
                "/etc/cron.weekly",
                "/etc/cron.monthly",
            ] {
                if let Ok(files) = guestfs.ls(dir) {
                    for file in files {
                        cron_jobs.push(format!(
                            "{}/{}",
                            dir.strip_prefix("/etc/").unwrap_or(dir),
                            file
                        ));
                    }
                }
            }
            Ok(cron_jobs)
        })
    }

    /// List systemd timers
    pub fn inspect_systemd_timers(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut timers = Vec::new();
            if let Ok(links) = guestfs.ls(SYSTEMD_TIMERS_DIR) {
                timers = links
                    .into_iter()
                    .filter(|f| f.ends_with(".timer"))
                    .collect();
            }
            Ok(timers)
        })
    }

    /// List SSL certificates
    pub fn inspect_certificates(&mut self, root: &str) -> Result<Vec<Certificate>> {
        self.with_mount(root, |guestfs| {
            let mut certs = Vec::new();
            // Common certificate locations
            let mut paths = Vec::new();
            for dir in &["/etc/ssl/certs", "/etc/pki/tls/certs", "/etc/pki/ca-trust"] {
                if let Ok(files) = guestfs.ls(dir) {
                    for file in files
                        .iter()
                        .filter(|f| f.ends_with(".crt") || f.ends_with(".pem"))
                    {
                        paths.push(format!("{}/{}", dir, file));
                    }
                }
            }
            // Parse certificates if openssl is available in the guest
            let has_openssl = guestfs.exists("/usr/bin/openssl").unwrap_or(false);
            for path in paths {
                let mut subject = "Unknown".to_string();
                let mut issuer = "Unknown".to_string();
                let mut expiry = "Unknown".to_string();
                if has_openssl {
                    if let Ok(output) = guestfs.command(&[
                        "openssl", "x509", "-in", &path, "-noout", "-subject", "-issuer",
                        "-enddate",
                    ]) {
                        for line in output.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("subject=") {
                                subject = trimmed
                                    .strip_prefix("subject=")
                                    .unwrap_or(&subject)
                                    .to_string();
                            } else if trimmed.starts_with("issuer=") {
                                issuer = trimmed
                                    .strip_prefix("issuer=")
                                    .unwrap_or(&issuer)
                                    .to_string();
                            } else if trimmed.starts_with("notAfter=") {
                                expiry = trimmed
                                    .strip_prefix("notAfter=")
                                    .unwrap_or(&expiry)
                                    .to_string();
                            }
                        }
                    }
                }
                certs.push(Certificate {
                    path: path.clone(),
                    subject,
                    issuer,
                    expiry,
                });
            }
            Ok(certs)
        })
    }

    /// Get kernel parameters
    pub fn inspect_kernel_params(&mut self, root: &str) -> Result<HashMap<String, String>> {
        self.with_mount(root, |guestfs| {
            let mut params = HashMap::new();
            if let Ok(content) = guestfs.cat(SYSCTL_CONF) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        params.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
            }
            Ok(params)
        })
    }

    /// Detect virtualization guest tools
    pub fn inspect_vm_tools(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut tools = Vec::new();
            // open-vm-tools (OSS) ships vmware-toolbox-cmd too. Prefer the OSS
            // markers so Ubuntu cloud images are not flagged as proprietary
            // VMware Tools remnants (BOOT-006 / BOOT-010 false positives).
            let has_open_vm = guestfs.exists("/usr/bin/vmtoolsd").unwrap_or(false)
                || guestfs
                    .exists("/lib/systemd/system/open-vm-tools.service")
                    .unwrap_or(false)
                || guestfs
                    .exists("/usr/lib/systemd/system/open-vm-tools.service")
                    .unwrap_or(false);
            if has_open_vm {
                tools.push("open-vm-tools".to_string());
            } else if guestfs
                .exists("/usr/bin/vmware-toolbox-cmd")
                .unwrap_or(false)
                || guestfs.exists("/etc/vmware-tools").unwrap_or(false)
            {
                tools.push("vmware-tools".to_string());
            }
            // QEMU Guest Agent
            if guestfs.exists("/usr/bin/qemu-ga").unwrap_or(false) {
                tools.push("qemu-guest-agent".to_string());
            }
            // VirtualBox Guest Additions
            if guestfs.exists("/usr/sbin/VBoxService").unwrap_or(false)
                || guestfs.exists("/opt/VBoxGuestAdditions").unwrap_or(false)
            {
                tools.push("virtualbox-guest-additions".to_string());
            }
            // Hyper-V tools
            if guestfs.exists("/usr/sbin/hv_kvp_daemon").unwrap_or(false) {
                tools.push("hyper-v-tools".to_string());
            }
            Ok(tools)
        })
    }

    /// Get boot configuration
    pub fn inspect_boot_config(&mut self, root: &str) -> Result<BootConfig> {
        self.with_mount(root, |guestfs| {
            let mut config = BootConfig {
                bootloader: "unknown".to_string(),
                default_entry: "unknown".to_string(),
                timeout: "unknown".to_string(),
                kernel_cmdline: String::new(),
            };
            // Check GRUB2
            let grub_paths = vec![
                "/boot/grub2/grub.cfg",
                "/boot/grub/grub.cfg",
                "/etc/grub2.cfg",
            ];
            for grub_cfg in grub_paths {
                if guestfs.exists(grub_cfg).unwrap_or(false) {
                    config.bootloader = "GRUB2".to_string();
                    if let Ok(content) = guestfs.cat(grub_cfg) {
                        // Parse basic GRUB config
                        let mut default_index: Option<usize> = None;
                        let mut current_entry = 0;
                        let mut in_menuentry = false;
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("set timeout=") {
                                config.timeout = trimmed
                                    .split('=')
                                    .nth(1)
                                    .unwrap_or("unknown")
                                    .trim_matches('"')
                                    .to_string();
                            } else if trimmed.starts_with("set default=") {
                                let default_str =
                                    trimmed.split('=').nth(1).unwrap_or("0").trim_matches('"');
                                default_index = default_str.parse::<usize>().ok();
                            } else if trimmed.starts_with("menuentry ")
                                || trimmed.starts_with("submenu ")
                            {
                                if in_menuentry {
                                    current_entry += 1;
                                }
                                in_menuentry = true;
                                let title_parts: Vec<&str> = trimmed.splitn(3, '\'').collect();
                                let current_title = if title_parts.len() >= 2 {
                                    title_parts[1].to_string()
                                } else {
                                    "unknown".to_string()
                                };
                                if Some(current_entry) == default_index {
                                    config.default_entry = current_title;
                                }
                            } else if in_menuentry
                                && (trimmed.starts_with("linux")
                                    || trimmed.starts_with("linux16")
                                    || trimmed.starts_with("linuxefi"))
                            {
                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                if parts.len() > 1 && Some(current_entry) == default_index {
                                    config.kernel_cmdline = parts[1..].join(" ");
                                }
                            } else if trimmed == "}" && in_menuentry {
                                in_menuentry = false;
                                current_entry += 1;
                            }
                        }
                    }
                    break;
                }
            }
            Ok(config)
        })
    }

    /// Get swap information
    pub fn inspect_swap(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut swap_devices = Vec::new();
            if let Ok(content) = guestfs.cat(FSTAB) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 && parts[2] == "swap" {
                        swap_devices.push(parts[0].to_string());
                    }
                }
            }
            Ok(swap_devices)
        })
    }

    /// Get fstab mounts
    pub fn inspect_fstab(&mut self, root: &str) -> Result<Vec<(String, String, String)>> {
        self.with_mount(root, |guestfs| {
            let mut mounts = Vec::new();
            if let Ok(content) = guestfs.cat(FSTAB) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        mounts.push((
                            parts[0].to_string(), // device
                            parts[1].to_string(), // mountpoint
                            parts[2].to_string(), // fstype
                        ));
                    }
                }
            }
            Ok(mounts)
        })
    }

    /// Detect installed package manager and list packages
    ///
    /// Supports rpm (RHEL, Fedora, CentOS, openSUSE), dpkg (Debian, Ubuntu),
    /// pacman (Arch Linux), and apk (Alpine Linux).
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// `PackageInfo` struct containing manager type, count, and package list
    pub fn inspect_packages(&mut self, root: &str) -> Result<PackageInfo> {
        self.with_mount(root, |guestfs| {
            let mut pkg_info = PackageInfo {
                manager: "unknown".to_string(),
                package_count: 0,
                packages: Vec::new(),
            };

            // RPM-based (RHEL, Fedora, CentOS, openSUSE)
            if guestfs.exists("/usr/bin/rpm").unwrap_or(false) {
                pkg_info.manager = "rpm".to_string();
                if let Ok(output) =
                    guestfs.command(&["rpm", "-qa", "--qf", "%{NAME}|%{VERSION}|%{RELEASE}\\n"])
                {
                    for line in output.lines() {
                        let parts: Vec<&str> = line.split('|').collect();
                        if parts.len() >= 3 {
                            pkg_info.packages.push(Package {
                                name: parts[0].to_string(),
                                version: format!("{}-{}", parts[1], parts[2]),
                                manager: "rpm".to_string(),
                            });
                        }
                    }
                }
            }
            // DEB-based (Debian, Ubuntu)
            else if guestfs.exists("/usr/bin/dpkg").unwrap_or(false) {
                pkg_info.manager = "dpkg".to_string();
                if let Ok(output) =
                    guestfs.command(&["dpkg-query", "-W", "-f=${Package}|${Version}\\n"])
                {
                    for line in output.lines() {
                        if let Some((name, version)) = line.split_once('|') {
                            pkg_info.packages.push(Package {
                                name: name.to_string(),
                                version: version.to_string(),
                                manager: "dpkg".to_string(),
                            });
                        }
                    }
                }
            }
            // Arch Linux
            else if guestfs.exists("/usr/bin/pacman").unwrap_or(false) {
                pkg_info.manager = "pacman".to_string();
                if let Ok(output) = guestfs.command(&["pacman", "-Q"]) {
                    for line in output.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            pkg_info.packages.push(Package {
                                name: parts[0].to_string(),
                                version: parts[1].to_string(),
                                manager: "pacman".to_string(),
                            });
                        }
                    }
                }
            }
            // Alpine Linux
            else if guestfs.exists("/sbin/apk").unwrap_or(false) {
                pkg_info.manager = "apk".to_string();
                if let Ok(output) = guestfs.command(&["apk", "info", "-v"]) {
                    for line in output.lines() {
                        // Format: package-version
                        if let Some(pos) = line.rfind('-') {
                            pkg_info.packages.push(Package {
                                name: line[..pos].to_string(),
                                version: line[pos + 1..].to_string(),
                                manager: "apk".to_string(),
                            });
                        }
                    }
                }
            }

            pkg_info.package_count = pkg_info.packages.len();
            Ok(pkg_info)
        })
    }

    /// Detect firewall configuration
    ///
    /// Supports firewalld (RHEL/Fedora/CentOS), ufw (Ubuntu/Debian), and iptables.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// `FirewallInfo` struct with type, enabled status, rule count, and zones
    pub fn inspect_firewall(&mut self, root: &str) -> Result<FirewallInfo> {
        self.with_mount(root, |guestfs| {
            let mut fw_info = FirewallInfo {
                firewall_type: "none".to_string(),
                enabled: false,
                rules_count: 0,
                zones: Vec::new(),
            };

            // firewalld (RHEL/Fedora/CentOS)
            if guestfs.exists("/usr/sbin/firewalld").unwrap_or(false) {
                fw_info.firewall_type = "firewalld".to_string();

                // Check if enabled
                if let Ok(links) = guestfs.ls(SYSTEMD_SERVICES_DIR) {
                    fw_info.enabled = links.iter().any(|l| l.contains("firewalld"));
                }

                // Get active zones
                if let Ok(zones) = guestfs.ls("/etc/firewalld/zones") {
                    fw_info.zones = zones
                        .into_iter()
                        .filter(|z| z.ends_with(".xml"))
                        .map(|z| z.strip_suffix(".xml").unwrap_or(&z).to_string())
                        .collect();
                }
            }
            // ufw (Ubuntu/Debian)
            else if guestfs.exists("/usr/sbin/ufw").unwrap_or(false) {
                fw_info.firewall_type = "ufw".to_string();

                if let Ok(content) = guestfs.cat("/etc/ufw/ufw.conf") {
                    for line in content.lines() {
                        if line.starts_with("ENABLED=") {
                            fw_info.enabled = line.contains("yes");
                        }
                    }
                }

                // Count rules
                if let Ok(rules) = guestfs.cat("/etc/ufw/user.rules") {
                    fw_info.rules_count = rules
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                        .count();
                }
            }
            // iptables (legacy)
            else if guestfs.exists("/usr/sbin/iptables").unwrap_or(false) {
                fw_info.firewall_type = "iptables".to_string();

                // Check for rules files
                if let Ok(rules) = guestfs
                    .cat("/etc/sysconfig/iptables")
                    .or_else(|_| guestfs.cat("/etc/iptables/rules.v4"))
                {
                    fw_info.rules_count = rules
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                        .count();
                    fw_info.enabled = fw_info.rules_count > 0;
                }
            }

            Ok(fw_info)
        })
    }

    /// Detect init system
    ///
    /// Detects systemd, upstart, OpenRC, runit, or SysVinit.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// String identifying the init system
    pub fn inspect_init_system(&mut self, root: &str) -> Result<String> {
        self.with_mount(root, |guestfs| {
            // systemd
            if guestfs.is_dir("/run/systemd/system").unwrap_or(false)
                || guestfs.exists("/usr/lib/systemd/systemd").unwrap_or(false)
            {
                return Ok("systemd".to_string());
            }

            // upstart
            if guestfs.exists("/sbin/initctl").unwrap_or(false)
                && guestfs.is_dir("/etc/init").unwrap_or(false)
            {
                return Ok("upstart".to_string());
            }

            // OpenRC
            if guestfs.exists("/sbin/openrc").unwrap_or(false)
                || guestfs.exists("/sbin/rc-service").unwrap_or(false)
            {
                return Ok("openrc".to_string());
            }

            // runit
            if guestfs.is_dir("/etc/runit").unwrap_or(false) {
                return Ok("runit".to_string());
            }

            // SysVinit (fallback)
            if guestfs.exists("/sbin/init").unwrap_or(false) {
                return Ok("sysvinit".to_string());
            }

            Ok("unknown".to_string())
        })
    }

    /// Detect installed web servers
    ///
    /// Detects Apache, Nginx, and Lighttpd with configuration paths and enabled status.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Vector of `WebServer` structs
    pub fn inspect_web_servers(&mut self, root: &str) -> Result<Vec<WebServer>> {
        self.with_mount(root, |guestfs| {
            let mut servers = Vec::new();

            // Apache
            if guestfs.exists("/usr/sbin/httpd").unwrap_or(false)
                || guestfs.exists("/usr/sbin/apache2").unwrap_or(false)
            {
                let mut apache = WebServer {
                    name: "apache".to_string(),
                    version: "unknown".to_string(),
                    config_path: String::new(),
                    enabled: false,
                };

                // Detect config location
                if guestfs.is_dir("/etc/httpd").unwrap_or(false) {
                    apache.config_path = "/etc/httpd/conf/httpd.conf".to_string();
                } else if guestfs.is_dir("/etc/apache2").unwrap_or(false) {
                    apache.config_path = "/etc/apache2/apache2.conf".to_string();
                }

                // Check if enabled
                if let Ok(links) = guestfs.ls(SYSTEMD_SERVICES_DIR) {
                    apache.enabled = links
                        .iter()
                        .any(|l| l.contains("httpd") || l.contains("apache"));
                }

                servers.push(apache);
            }

            // Nginx
            if guestfs.exists("/usr/sbin/nginx").unwrap_or(false) {
                let mut nginx = WebServer {
                    name: "nginx".to_string(),
                    version: "unknown".to_string(),
                    config_path: "/etc/nginx/nginx.conf".to_string(),
                    enabled: false,
                };

                if let Ok(links) = guestfs.ls(SYSTEMD_SERVICES_DIR) {
                    nginx.enabled = links.iter().any(|l| l.contains("nginx"));
                }

                servers.push(nginx);
            }

            // Lighttpd
            if guestfs.exists("/usr/sbin/lighttpd").unwrap_or(false) {
                servers.push(WebServer {
                    name: "lighttpd".to_string(),
                    version: "unknown".to_string(),
                    config_path: "/etc/lighttpd/lighttpd.conf".to_string(),
                    enabled: false,
                });
            }

            Ok(servers)
        })
    }

    /// Detect installed databases
    ///
    /// Detects PostgreSQL, MySQL/MariaDB, MongoDB, and Redis with data directories
    /// and configuration paths.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Vector of `Database` structs
    pub fn inspect_databases(&mut self, root: &str) -> Result<Vec<Database>> {
        self.with_mount(root, |guestfs| {
            let mut databases = Vec::new();

            // PostgreSQL
            if guestfs.exists("/usr/bin/postgres").unwrap_or(false)
                || guestfs.exists("/usr/lib/postgresql").unwrap_or(false)
            {
                databases.push(Database {
                    name: "postgresql".to_string(),
                    data_dir: "/var/lib/pgsql/data".to_string(),
                    config_path: "/var/lib/pgsql/data/postgresql.conf".to_string(),
                });
            }

            // MySQL/MariaDB
            if guestfs.exists("/usr/bin/mysqld").unwrap_or(false)
                || guestfs.exists("/usr/sbin/mysqld").unwrap_or(false)
            {
                let name = if guestfs.exists("/usr/bin/mariadb").unwrap_or(false) {
                    "mariadb"
                } else {
                    "mysql"
                };

                databases.push(Database {
                    name: name.to_string(),
                    data_dir: "/var/lib/mysql".to_string(),
                    config_path: "/etc/my.cnf".to_string(),
                });
            }

            // MongoDB
            if guestfs.exists("/usr/bin/mongod").unwrap_or(false) {
                databases.push(Database {
                    name: "mongodb".to_string(),
                    data_dir: "/var/lib/mongo".to_string(),
                    config_path: "/etc/mongod.conf".to_string(),
                });
            }

            // Redis
            if guestfs.exists("/usr/bin/redis-server").unwrap_or(false) {
                databases.push(Database {
                    name: "redis".to_string(),
                    data_dir: "/var/lib/redis".to_string(),
                    config_path: "/etc/redis.conf".to_string(),
                });
            }

            Ok(databases)
        })
    }

    /// Detect security tools and hardening
    ///
    /// Checks for SELinux, AppArmor, fail2ban, AIDE, auditd, and SSH authorized keys.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// `SecurityInfo` struct with security tool status
    pub fn inspect_security(&mut self, root: &str) -> Result<SecurityInfo> {
        self.with_mount(root, |guestfs| {
            let mut sec_info = SecurityInfo {
                selinux: "unknown".to_string(),
                apparmor: false,
                fail2ban: false,
                aide: false,
                auditd: false,
                ssh_keys: Vec::new(),
            };

            // SELinux
            if let Ok(content) = guestfs.cat(SELINUX_CONFIG) {
                for line in content.lines() {
                    if line.starts_with("SELINUX=") {
                        sec_info.selinux = line.split('=').nth(1).unwrap_or("unknown").to_string();
                    }
                }
            }

            // AppArmor
            sec_info.apparmor = guestfs.exists("/etc/apparmor.d").unwrap_or(false)
                || guestfs
                    .exists("/sys/kernel/security/apparmor")
                    .unwrap_or(false);

            // fail2ban
            sec_info.fail2ban = guestfs.exists("/etc/fail2ban").unwrap_or(false);

            // AIDE
            sec_info.aide = guestfs.exists("/usr/sbin/aide").unwrap_or(false);

            // auditd
            sec_info.auditd = guestfs.exists("/sbin/auditd").unwrap_or(false);

            // SSH authorized keys
            if let Ok(users) = guestfs.ls("/home") {
                for user in users {
                    let key_path = format!("/home/{}/.ssh/authorized_keys", user);
                    if let Ok(content) = guestfs.cat(&key_path) {
                        let key_count = content
                            .lines()
                            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                            .count();
                        if key_count > 0 {
                            sec_info.ssh_keys.push((user, key_count));
                        }
                    }
                }
            }

            Ok(sec_info)
        })
    }

    /// List kernel modules configured to load at boot
    ///
    /// Reads from /etc/modules and /etc/modules-load.d/ directories.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Vector of module names
    pub fn inspect_kernel_modules(&mut self, root: &str) -> Result<Vec<String>> {
        self.with_mount(root, |guestfs| {
            let mut modules = Vec::new();

            // Get list of modules to load at boot
            if let Ok(content) = guestfs.cat("/etc/modules") {
                for line in content.lines() {
                    let line = line.trim();
                    if !line.is_empty() && !line.starts_with('#') {
                        modules.push(line.to_string());
                    }
                }
            }

            // RHEL/Fedora/systemd style
            if let Ok(files) = guestfs.ls("/etc/modules-load.d") {
                for file in files.iter().filter(|f| f.ends_with(".conf")) {
                    let path = format!("/etc/modules-load.d/{}", file);
                    if let Ok(content) = guestfs.cat(&path) {
                        for line in content.lines() {
                            let line = line.trim();
                            if !line.is_empty() && !line.starts_with('#') {
                                modules.push(line.to_string());
                            }
                        }
                    }
                }
            }

            Ok(modules)
        })
    }

    /// Parse /etc/hosts file
    ///
    /// Extracts IP addresses and their associated hostnames.
    ///
    /// # Arguments
    /// * `root` - Root device (e.g., "/dev/sda1")
    ///
    /// # Returns
    /// Vector of `HostEntry` structs
    pub fn inspect_hosts(&mut self, root: &str) -> Result<Vec<HostEntry>> {
        self.with_mount(root, |guestfs| {
            let mut entries = Vec::new();

            if let Ok(content) = guestfs.cat(HOSTS) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }

                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        entries.push(HostEntry {
                            ip: parts[0].to_string(),
                            hostnames: parts[1..].iter().map(|s| s.to_string()).collect(),
                        });
                    }
                }
            }

            Ok(entries)
        })
    }

    // ==================== Windows-Specific Inspection ====================
    /// Inspect Windows software from registry
    pub fn inspect_windows_software(&mut self, root: &str) -> Result<Vec<WindowsApplication>> {
        let mut applications = Vec::new();
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted && self.mount_ro(root, "/").is_err() {
            return Ok(applications);
        }
        // Get SOFTWARE registry hive path
        let systemroot = self
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let software_path = format!("{}/System32/config/SOFTWARE", systemroot);
        // Resolve to host path for direct file access
        let host_path = self.resolve_guest_path(&software_path)?;
        // Parse using nt_hive2
        if let Ok(apps) = super::windows_registry::parse_installed_software(host_path.as_path()) {
            for app in apps {
                applications.push(WindowsApplication {
                    name: app.name,
                    version: app.version,
                    publisher: app.publisher,
                    install_date: "Unknown".to_string(),
                });
            }
        }
        if !was_mounted {
            self.umount("/").ok();
        }
        Ok(applications)
    }

    /// Inspect Windows services from registry
    pub fn inspect_windows_services(&mut self, root: &str) -> Result<Vec<WindowsService>> {
        let mut services = Vec::new();
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted && self.mount_ro(root, "/").is_err() {
            return Ok(services);
        }
        let systemroot = self
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        // Parse from SYSTEM registry hive using nt_hive2
        let system_path = format!("{}/System32/config/SYSTEM", systemroot);
        let host_path = self.resolve_guest_path(&system_path)?;
        if let Ok(svcs) = super::windows_registry::parse_windows_services(host_path.as_path()) {
            for svc in svcs {
                services.push(WindowsService {
                    name: svc.name,
                    display_name: svc.display_name,
                    start_type: svc.start_type,
                    status: "Unknown".to_string(), // Status requires runtime info
                });
            }
        }
        if !was_mounted {
            self.umount("/").ok();
        }
        Ok(services)
    }

    /// Inspect Windows network configuration
    pub fn inspect_windows_network(&mut self, root: &str) -> Result<Vec<WindowsNetworkAdapter>> {
        let mut adapters = Vec::new();
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted && self.mount_ro(root, "/").is_err() {
            return Ok(adapters);
        }
        let systemroot = self
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        // Parse SYSTEM registry for network configuration using nt_hive2
        let system_path = format!("{}/System32/config/SYSTEM", systemroot);
        let host_path = self.resolve_guest_path(&system_path)?;
        if let Ok(net_adapters) =
            super::windows_registry::parse_network_adapters(host_path.as_path())
        {
            for adapter in net_adapters {
                adapters.push(WindowsNetworkAdapter {
                    name: adapter.name,
                    description: adapter.description,
                    dhcp_enabled: adapter.dhcp_enabled,
                    ip_address: adapter.ip_address,
                    mac_address: adapter.mac_address,
                    dns_servers: adapter.dns_servers,
                });
            }
        }
        if !was_mounted {
            self.umount("/").ok();
        }
        Ok(adapters)
    }

    /// Inspect Windows updates and patches
    pub fn inspect_windows_updates(&mut self, root: &str) -> Result<Vec<WindowsUpdate>> {
        let mut updates = Vec::new();
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted && self.mount_ro(root, "/").is_err() {
            return Ok(updates);
        }
        let systemroot = self
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        // Parse installed updates from registry
        let software_path = format!("{}/System32/config/SOFTWARE", systemroot);
        let software_host = self.resolve_guest_path(&software_path)?;
        if let Ok(reg_updates) =
            super::windows_registry::parse_installed_updates(software_host.as_path())
        {
            for upd in reg_updates {
                updates.push(WindowsUpdate {
                    kb: upd.kb_number,
                    title: upd.title,
                    installed_date: upd.installed_date,
                    update_type: upd.update_type,
                });
            }
        }
        // Parse CBS.log for component updates
        let cbs_log_path = format!("{}/Logs/CBS/CBS.log", systemroot);
        if let Ok(content) = self.cat(&cbs_log_path) {
            let cbs_updates = super::windows_registry::parse_cbs_log(&content);
            for upd in cbs_updates {
                updates.push(WindowsUpdate {
                    kb: upd.kb_number,
                    title: upd.title,
                    installed_date: upd.installed_date,
                    update_type: upd.update_type,
                });
            }
        }
        // Check Windows Update DataStore
        let update_db_path = format!(
            "{}/SoftwareDistribution/DataStore/DataStore.edb",
            systemroot
        );
        let db_host = self.resolve_guest_path(&update_db_path).ok();
        if let Some(db_path) = db_host {
            if let Ok(db_updates) =
                super::windows_registry::parse_update_datastore(db_path.as_path())
            {
                for upd in db_updates {
                    updates.push(WindowsUpdate {
                        kb: upd.kb_number,
                        title: upd.title,
                        installed_date: upd.installed_date,
                        update_type: upd.update_type,
                    });
                }
            }
        }
        // Detect hotfixes from filesystem
        let systemroot_host = self.resolve_guest_path(&systemroot)?;
        if let Ok(hotfixes) =
            super::windows_registry::detect_hotfixes_from_filesystem(systemroot_host.as_path())
        {
            for upd in hotfixes {
                updates.push(WindowsUpdate {
                    kb: upd.kb_number,
                    title: upd.title,
                    installed_date: upd.installed_date,
                    update_type: upd.update_type,
                });
            }
        }
        if !was_mounted {
            self.umount("/").ok();
        }
        Ok(updates)
    }

    /// Inspect Windows event logs
    pub fn inspect_windows_events(
        &mut self,
        root: &str,
        log_name: &str,
        limit: usize,
    ) -> Result<Vec<WindowsEventLogEntry>> {
        let mut events = Vec::new();
        let was_mounted = self.mounted.contains_key("/");
        if !was_mounted && self.mount_ro(root, "/").is_err() {
            return Ok(events);
        }
        let systemroot = self
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let event_log_path = format!("{}/System32/winevt/Logs/{}.evtx", systemroot, log_name);
        // Resolve to host path for direct file access
        if let Ok(evtx_host) = self.resolve_guest_path(&event_log_path) {
            // Parse EVTX file using evtx crate
            if let Ok(parsed_events) =
                super::windows_registry::parse_evtx_file(evtx_host.as_path(), limit)
            {
                for evt in parsed_events {
                    events.push(WindowsEventLogEntry {
                        event_id: evt.event_id as i32,
                        level: evt.level,
                        source: evt.source,
                        message: evt.message,
                        time_created: evt.time_created,
                    });
                }
            }
        }
        if !was_mounted {
            self.umount("/").ok();
        }
        Ok(events)
    }
}

/// Windows application information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsApplication {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_date: String,
}

/// Windows service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsService {
    pub name: String,
    pub display_name: String,
    pub start_type: String,
    pub status: String,
}

/// Windows network adapter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsNetworkAdapter {
    pub name: String,
    pub description: String,
    pub mac_address: String,
    pub ip_address: Vec<String>,
    pub dns_servers: Vec<String>,
    pub dhcp_enabled: bool,
}

/// Windows update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsUpdate {
    pub kb: String,
    pub title: String,
    pub installed_date: String,
    pub update_type: String,
}

/// Windows event log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsEventLogEntry {
    pub event_id: i32,
    pub level: String,
    pub source: String,
    pub message: String,
    pub time_created: String,
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub manager: String,
    pub package_count: usize,
    pub packages: Vec<Package>,
}

/// Individual package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub manager: String,
}

/// Firewall information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallInfo {
    pub firewall_type: String,
    pub enabled: bool,
    pub rules_count: usize,
    pub zones: Vec<String>,
}

/// Web server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebServer {
    pub name: String,
    pub version: String,
    pub config_path: String,
    pub enabled: bool,
}

/// Database information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub name: String,
    pub data_dir: String,
    pub config_path: String,
}

/// Security information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub selinux: String,
    pub apparmor: bool,
    pub fail2ban: bool,
    pub aide: bool,
    pub auditd: bool,
    pub ssh_keys: Vec<(String, usize)>,
}

/// Host entry from /etc/hosts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub ip: String,
    pub hostnames: Vec<String>,
}

/// Parsed systemd-resolved configuration
struct ParsedResolved {
    dns: Vec<String>,
    fallback_dns: Vec<String>,
}
