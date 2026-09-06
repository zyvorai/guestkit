// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Service and daemon management operations for disk image manipulation
//!
//! This implementation provides service management access.

use crate::core::Result;
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Check if service is enabled
    ///
    pub fn is_service_enabled(&mut self, service: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_service_enabled {}", service);
        }

        // Check systemd services
        let systemd_path = format!(
            "/etc/systemd/system/multi-user.target.wants/{}.service",
            service
        );
        if self.exists(&systemd_path)? {
            return Ok(true);
        }

        // Check systemd system path
        let system_path = format!("/lib/systemd/system/{}.service", service);
        if self.exists(&system_path)? {
            // Check if linked in wants directory
            if self.exists(&systemd_path)? {
                return Ok(true);
            }
        }

        // Check sysvinit rc?.d links (Debian/Ubuntu)
        for runlevel in &["2", "3", "4", "5"] {
            let rc_path = format!("/etc/rc{}.d/S??{}", runlevel, service);
            if self.exists(&rc_path)? {
                return Ok(true);
            }
        }

        // Check chkconfig (RHEL/CentOS)
        let chkconfig_path = format!("/etc/rc.d/rc3.d/S??{}", service);
        if self.exists(&chkconfig_path)? {
            return Ok(true);
        }

        Ok(false)
    }

    /// List enabled services
    ///
    pub fn list_enabled_services(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_enabled_services");
        }

        let mut services = Vec::new();

        // List systemd enabled services
        if self.exists("/etc/systemd/system/multi-user.target.wants")? {
            let entries = self.ls("/etc/systemd/system/multi-user.target.wants")?;
            for entry in entries {
                if entry.ends_with(".service") {
                    let service_name = entry.trim_end_matches(".service");
                    services.push(service_name.to_string());
                }
            }
        }

        // List sysvinit services (runlevel 3)
        if self.exists("/etc/rc3.d")? {
            let entries = self.ls("/etc/rc3.d")?;
            for entry in entries {
                if entry.starts_with('S') {
                    // Extract service name from S??servicename
                    if let Some(name) = entry.get(3..) {
                        if !services.contains(&name.to_string()) {
                            services.push(name.to_string());
                        }
                    }
                }
            }
        }

        services.sort();
        Ok(services)
    }

    /// List disabled services
    ///
    pub fn list_disabled_services(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_disabled_services");
        }

        let mut all_services = Vec::new();
        let mut disabled = Vec::new();

        // Get all available services
        if self.exists("/lib/systemd/system")? {
            let entries = self.ls("/lib/systemd/system")?;
            for entry in entries {
                if entry.ends_with(".service") {
                    let service_name = entry.trim_end_matches(".service");
                    all_services.push(service_name.to_string());
                }
            }
        }

        // Check which are enabled
        let enabled = self.list_enabled_services()?;

        // Find disabled services
        for service in all_services {
            if !enabled.contains(&service) {
                disabled.push(service);
            }
        }

        disabled.sort();
        Ok(disabled)
    }

    /// Get service status
    ///
    pub fn get_service_status(&mut self, service: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_service_status {}", service);
        }

        // For offline VM, we can only check if enabled
        if self.is_service_enabled(service)? {
            Ok("enabled".to_string())
        } else {
            Ok("disabled".to_string())
        }
    }

    /// List running services
    ///
    pub fn list_services(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_services");
        }

        // For offline VM, return enabled services
        self.list_enabled_services()
    }

    /// Get init system type
    ///
    pub fn get_init_system(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_init_system");
        }

        // Check for systemd
        if self.exists("/etc/systemd/system")? || self.exists("/lib/systemd/system")? {
            return Ok("systemd".to_string());
        }

        // Check for upstart
        if self.exists("/etc/init")? {
            return Ok("upstart".to_string());
        }

        // Check for sysvinit
        if self.exists("/etc/rc.d")? || self.exists("/etc/rc3.d")? {
            return Ok("sysvinit".to_string());
        }

        Ok("unknown".to_string())
    }

    /// List cron jobs
    ///
    pub fn list_cron_jobs(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_cron_jobs");
        }

        let mut jobs = Vec::new();

        // Check /etc/crontab
        if self.exists("/etc/crontab")? {
            let crontab = self.cat("/etc/crontab")?;
            for line in crontab.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    jobs.push(format!("system: {}", line));
                }
            }
        }

        // Check /etc/cron.d
        if self.exists("/etc/cron.d")? {
            let entries = self.ls("/etc/cron.d")?;
            for entry in entries {
                let path = format!("/etc/cron.d/{}", entry);
                if let Ok(content) = self.cat(&path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            jobs.push(format!("{}: {}", entry, line));
                        }
                    }
                }
            }
        }

        // Check user crontabs
        if self.exists("/var/spool/cron/crontabs")? {
            let users = self.ls("/var/spool/cron/crontabs")?;
            for user in users {
                let path = format!("/var/spool/cron/crontabs/{}", user);
                if let Ok(content) = self.cat(&path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            jobs.push(format!("{}: {}", user, line));
                        }
                    }
                }
            }
        }

        Ok(jobs)
    }

    /// Get process list snapshot
    ///
    pub fn list_processes(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_processes");
        }

        // For offline VM, we can't list running processes
        // Return empty list
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
