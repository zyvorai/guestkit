// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Network and hostname operations for disk image manipulation
//!
//! This implementation provides network configuration access.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get hostname
    ///
    pub fn get_hostname(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_hostname");
        }

        // Try to read /etc/hostname
        if let Ok(hostname) = self.cat("/etc/hostname") {
            return Ok(hostname.trim().to_string());
        }

        // Fallback: try /etc/sysconfig/network (RHEL/CentOS)
        if let Ok(content) = self.cat("/etc/sysconfig/network") {
            for line in content.lines() {
                if line.starts_with("HOSTNAME=") {
                    let hostname = line.trim_start_matches("HOSTNAME=").trim();
                    return Ok(hostname.trim_matches('"').to_string());
                }
            }
        }

        Err(Error::NotFound("Could not determine hostname".to_string()))
    }

    /// Set hostname
    ///
    pub fn set_hostname(&mut self, hostname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_hostname {}", hostname);
        }

        // Write to /etc/hostname
        self.write("/etc/hostname", format!("{}\n", hostname).as_bytes())?;

        // Also update /etc/hosts if it exists
        if self.exists("/etc/hosts")? {
            let hosts_content = self.cat("/etc/hosts").unwrap_or_default();
            let mut new_hosts = String::new();
            let mut found_127 = false;

            for line in hosts_content.lines() {
                if line.trim().starts_with("127.0.1.1") || line.trim().starts_with("127.0.0.1\t") {
                    if !found_127 {
                        new_hosts.push_str(&format!("127.0.1.1\t{}\n", hostname));
                        found_127 = true;
                    }
                } else {
                    new_hosts.push_str(line);
                    new_hosts.push('\n');
                }
            }

            if !found_127 {
                new_hosts.push_str(&format!("127.0.1.1\t{}\n", hostname));
            }

            self.write("/etc/hosts", new_hosts.as_bytes())?;
        }

        Ok(())
    }

    /// Get network interfaces
    ///
    pub fn list_network_interfaces(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_network_interfaces");
        }

        let mut interfaces = Vec::new();

        // List /sys/class/net
        if self.exists("/sys/class/net")? {
            if let Ok(entries) = self.ls("/sys/class/net") {
                for entry in entries {
                    // Skip loopback
                    if entry != "lo" {
                        interfaces.push(entry);
                    }
                }
            }
        }

        Ok(interfaces)
    }

    /// Ping host
    ///
    pub fn ping_daemon(&self) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ping_daemon");
        }

        // Always return true since we're running
        Ok(true)
    }

    /// Get network configuration for interface
    ///
    pub fn get_network_config(&mut self, interface: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_network_config {}", interface);
        }

        let mut config = String::new();

        // Try Debian/Ubuntu style
        let debian_config = format!("/etc/network/interfaces.d/{}", interface);
        if self.exists(&debian_config)? {
            config = self.cat(&debian_config)?;
        } else if self.exists("/etc/network/interfaces")? {
            let interfaces = self.cat("/etc/network/interfaces")?;
            let mut in_interface = false;
            for line in interfaces.lines() {
                if line.contains(&format!("iface {}", interface)) {
                    in_interface = true;
                }
                if in_interface {
                    config.push_str(line);
                    config.push('\n');
                    if line.trim().is_empty() {
                        break;
                    }
                }
            }
        }

        // Try RHEL/CentOS style
        let rhel_config = format!("/etc/sysconfig/network-scripts/ifcfg-{}", interface);
        if config.is_empty() && self.exists(&rhel_config)? {
            config = self.cat(&rhel_config)?;
        }

        if config.is_empty() {
            return Err(Error::NotFound(format!(
                "No configuration found for interface {}",
                interface
            )));
        }

        Ok(config)
    }

    /// Read /etc/hosts file
    ///
    pub fn read_etc_hosts(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: read_etc_hosts");
        }

        self.cat("/etc/hosts")
    }

    /// Get DNS servers
    ///
    pub fn get_dns(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_dns");
        }

        let mut dns_servers = Vec::new();

        if self.exists("/etc/resolv.conf")? {
            let resolv = self.cat("/etc/resolv.conf")?;
            for line in resolv.lines() {
                let line = line.trim();
                if line.starts_with("nameserver") {
                    if let Some(server) = line.split_whitespace().nth(1) {
                        dns_servers.push(server.to_string());
                    }
                }
            }
        }

        Ok(dns_servers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
