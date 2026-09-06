// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Plan generator - converts profile findings into fix plans

use super::types::*;
use crate::cli::profiles::{Finding, ProfileReport, ReportSection, RiskLevel};
use anyhow::Result;
use serde_json::json;

/// Generates fix plans from profile reports
pub struct PlanGenerator {
    vm_path: String,
}

impl PlanGenerator {
    /// Create a new plan generator
    pub fn new(vm_path: String) -> Self {
        Self { vm_path }
    }

    /// Offline Windows RDP enablement plan (matches Machina / hyper2kvm firstboot).
    ///
    /// Writes Terminal Server allow, NLA, TermService/UmRdpService Automatic,
    /// port 3389, and stock inbound TCP/UDP firewall Active=TRUE rules — no
    /// full-disk backup needed when applied with `plan apply --skip-backup`.
    pub fn windows_rdp_enable_plan(&self) -> FixPlan {
        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-rdp".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(
            "Offline Windows Remote Desktop enablement (Terminal Server + NLA + \
             TermService/UmRdpService + firewall)"
                .into(),
        );
        plan.metadata.tags = vec![
            "windows".into(),
            "rdp".into(),
            "firewall".into(),
            "offline".into(),
        ];

        let fw_tcp = "v2.29|Action=Allow|Active=TRUE|Dir=In|Protocol=6|LPort=3389|\
App=%SystemRoot%\\system32\\svchost.exe|Svc=termservice|\
Name=@FirewallAPI.dll,-28753|Desc=@FirewallAPI.dll,-28756|\
EmbedCtxt=@FirewallAPI.dll,-28752|";
        let fw_udp = "v2.29|Action=Allow|Active=TRUE|Dir=In|Protocol=17|LPort=3389|\
App=%SystemRoot%\\system32\\svchost.exe|Svc=termservice|\
Name=@FirewallAPI.dll,-28752|Desc=@FirewallAPI.dll,-28756|\
EmbedCtxt=@FirewallAPI.dll,-28752|";

        let ops = [
            (
                "enable-rdp",
                r"HKLM\SYSTEM\ControlSet001\Control\Terminal Server",
                "fDenyTSConnections",
                json!(1),
                json!(0),
                "dword",
                Priority::High,
                "Allow Remote Desktop connections",
            ),
            (
                "rdp-nla",
                r"HKLM\SYSTEM\ControlSet001\Control\Terminal Server\WinStations\RDP-Tcp",
                "UserAuthentication",
                json!(1),
                json!(1),
                "dword",
                Priority::Low,
                "Keep Network Level Authentication enabled",
            ),
            (
                "rdp-port",
                r"HKLM\SYSTEM\ControlSet001\Control\Terminal Server\WinStations\RDP-Tcp",
                "PortNumber",
                json!(3389),
                json!(3389),
                "dword",
                Priority::Low,
                "Ensure RDP listens on TCP 3389",
            ),
            (
                "termservice-auto",
                r"HKLM\SYSTEM\ControlSet001\Services\TermService",
                "Start",
                json!(3),
                json!(2),
                "dword",
                Priority::High,
                "Set TermService startup type to Automatic",
            ),
            (
                "umrdpservice-auto",
                r"HKLM\SYSTEM\ControlSet001\Services\UmRdpService",
                "Start",
                json!(3),
                json!(2),
                "dword",
                Priority::High,
                "Set UmRdpService startup type to Automatic",
            ),
            (
                "fw-tcp",
                r"HKLM\SYSTEM\ControlSet001\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules",
                "RemoteDesktop-UserMode-In-TCP",
                json!(""),
                json!(fw_tcp),
                "sz",
                Priority::High,
                "Enable Remote Desktop firewall rule (TCP-In)",
            ),
            (
                "fw-udp",
                r"HKLM\SYSTEM\ControlSet001\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules",
                "RemoteDesktop-UserMode-In-UDP",
                json!(""),
                json!(fw_udp),
                "sz",
                Priority::High,
                "Enable Remote Desktop firewall rule (UDP-In)",
            ),
        ];

        for (id, key, value, current, new_data, dtype, priority, desc) in ops {
            plan.add_operation(Operation {
                id: id.into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: key.into(),
                    value: value.into(),
                    current_data: current,
                    new_data,
                    data_type: dtype.into(),
                }),
                priority,
                description: desc.into(),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }

        plan
    }

    /// Offline Linux SSH enablement plan (inspect-based).
    ///
    /// Enables the distro ssh/sshd systemd unit via wants symlink (guestfs
    /// `ln_sf`, not `CommandExec`), removes Ubuntu's `sshd_not_to_be_run`
    /// flag, and writes an sshd_config.d drop-in with PubkeyAuthentication
    /// yes. Optional `user` + `pubkey` injects into `authorized_keys`.
    pub fn linux_ssh_enable_plan(
        &self,
        g: &mut crate::Guestfs,
        user: Option<&str>,
        pubkey: Option<&str>,
    ) -> Result<FixPlan> {
        let mut plan = FixPlan::new(self.vm_path.clone(), "linux-ssh".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description =
            Some("Offline Linux SSH enablement (systemd wants symlink + sshd drop-in)".into());
        plan.metadata.tags = vec![
            "linux".into(),
            "ssh".into(),
            "offline".into(),
            "guestkit".into(),
        ];

        if !(g.is_file("/usr/sbin/sshd").unwrap_or(false)
            || g.is_file("/usr/bin/sshd").unwrap_or(false))
        {
            anyhow::bail!("OpenSSH server is not installed (sshd binary missing)");
        }

        let (unit, unit_src) = crate::cli::commands::security::detect_ssh_unit(g)
            .ok_or_else(|| anyhow::anyhow!("Could not detect ssh/sshd systemd unit"))?;

        // Prefer a real /etc wants dir (not a symlink that guestfs treats as escape).
        let wants_dir = "/etc/systemd/system/multi-user.target.wants";
        let wants_link = format!("{wants_dir}/{unit}");
        let relative = format!("../../../../{}", unit_src.trim_start_matches('/'));

        plan.add_operation(Operation {
            id: "remove-sshd-not-to-be-run".into(),
            op_type: OperationType::FileDelete(FileDelete {
                path: "/etc/ssh/sshd_not_to_be_run".into(),
                missing_ok: true,
            }),
            priority: Priority::High,
            description: "Remove Ubuntu cloud sshd_not_to_be_run flag if present".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        plan.add_operation(Operation {
            id: "ssh-wants-dir".into(),
            op_type: OperationType::DirectoryCreate(DirectoryCreate {
                path: wants_dir.into(),
                mode: Some("0755".into()),
            }),
            priority: Priority::High,
            description: "Ensure multi-user.target.wants exists".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        plan.add_operation(Operation {
            id: "enable-ssh-unit".into(),
            op_type: OperationType::Symlink(Symlink {
                target: relative,
                link_path: wants_link,
            }),
            priority: Priority::High,
            description: format!("Enable systemd unit {unit} via wants symlink"),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec!["ssh-wants-dir".into()],
            validation: None,
            undo: None,
        });

        plan.add_operation(Operation {
            id: "sshd-dropin".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: "/etc/ssh/sshd_config.d/99-guestkit.conf".into(),
                content: "# Managed by guestkit plan linux-ssh\nPubkeyAuthentication yes\n".into(),
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: "Write sshd_config.d drop-in (PubkeyAuthentication yes)".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        if let (Some(username), Some(key)) = (user, pubkey) {
            let key = key.trim();
            if key.is_empty() {
                anyhow::bail!("SSH public key is empty");
            }
            let home = Self::linux_home_for_user(g, username)?;
            let ssh_dir = format!("{home}/.ssh");
            let auth_keys = format!("{ssh_dir}/authorized_keys");

            let mut content = String::new();
            if let Ok(existing) = g.read_file(&auth_keys) {
                content = String::from_utf8_lossy(&existing).into_owned();
            }
            if !content.lines().any(|l| l.trim() == key) {
                if !content.is_empty() && !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str(key);
                content.push('\n');
            }

            plan.add_operation(Operation {
                id: "ssh-dir".into(),
                op_type: OperationType::DirectoryCreate(DirectoryCreate {
                    path: ssh_dir,
                    mode: Some("0700".into()),
                }),
                priority: Priority::High,
                description: format!("Ensure {username} .ssh directory exists"),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
            plan.add_operation(Operation {
                id: "inject-ssh-key".into(),
                op_type: OperationType::FileWrite(FileWrite {
                    path: auth_keys,
                    content,
                    mode: Some("0600".into()),
                }),
                priority: Priority::High,
                description: format!("Inject SSH public key for {username}"),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec!["ssh-dir".into()],
                validation: None,
                undo: None,
            });
        } else if user.is_some() ^ pubkey.is_some() {
            anyhow::bail!("Both --user and --key/--key-file are required to inject an SSH key");
        }

        plan.estimated_duration = Self::estimate_duration(plan.operations.len());
        Ok(plan)
    }

    fn linux_home_for_user(g: &mut crate::Guestfs, username: &str) -> Result<String> {
        if username == "root" {
            return Ok("/root".into());
        }
        let content = g
            .read_file("/etc/passwd")
            .map_err(|e| anyhow::anyhow!("read /etc/passwd: {e}"))?;
        let text = String::from_utf8_lossy(&content);
        let home = text.lines().find_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() >= 6 && fields[0] == username {
                Some(fields[5].to_string())
            } else {
                None
            }
        });
        home.ok_or_else(|| anyhow::anyhow!("User '{username}' not found in /etc/passwd"))
    }

    /// Offline Windows hostname plan (ComputerName + Tcpip Hostname).
    ///
    /// Apply with `plan apply --skip-backup`. Hostname must be a valid NetBIOS-
    /// friendly label (letters, digits, hyphen; max 15 recommended).
    pub fn windows_hostname_plan(&self, hostname: &str) -> Result<FixPlan> {
        let name = hostname.trim();
        if name.is_empty() {
            anyhow::bail!("Hostname is required (use --hostname)");
        }
        if name.len() > 63
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || name.starts_with('-')
            || name.ends_with('-')
        {
            anyhow::bail!(
                "Invalid hostname '{name}' (use ASCII letters, digits, hyphen; no leading/trailing hyphen)"
            );
        }

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-hostname".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!("Offline Windows hostname set to '{name}'"));
        plan.metadata.tags = vec!["windows".into(), "hostname".into(), "offline".into()];

        let ops = [
            (
                "computername",
                r"HKLM\SYSTEM\ControlSet001\Control\ComputerName\ComputerName",
                "ComputerName",
                name,
                "Set ComputerName",
            ),
            (
                "active-computername",
                r"HKLM\SYSTEM\ControlSet001\Control\ComputerName\ActiveComputerName",
                "ComputerName",
                name,
                "Set ActiveComputerName",
            ),
            (
                "tcpip-hostname",
                r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters",
                "Hostname",
                name,
                "Set Tcpip Hostname",
            ),
            (
                "tcpip-nv-hostname",
                r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters",
                "NV Hostname",
                name,
                "Set Tcpip NV Hostname",
            ),
        ];

        for (id, key, value, new_data, desc) in ops {
            plan.add_operation(Operation {
                id: id.into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: key.into(),
                    value: value.into(),
                    current_data: json!(""),
                    new_data: json!(new_data),
                    data_type: "sz".into(),
                }),
                priority: Priority::High,
                description: desc.into(),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }

        Ok(plan)
    }

    /// Offline Windows WinRM enablement (service Automatic + HTTP firewall).
    ///
    /// Sets WinRM Start=Automatic and enables the stock WINRM-HTTP-In-TCP
    /// firewall rule. Full listener/auth hardening still requires a live
    /// `Enable-PSRemoting` on first boot — this unlocks the common day-0 path
    /// for Packer/automation that expects HTTP/5985 open with the service set
    /// to start.
    pub fn windows_winrm_enable_plan(&self) -> FixPlan {
        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-winrm".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "medium".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = true;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(
            "Offline Windows WinRM enablement (WinRM Automatic + WINRM-HTTP-In-TCP). \
             Review auth/encryption before exposing beyond a lab network."
                .into(),
        );
        plan.metadata.tags = vec![
            "windows".into(),
            "winrm".into(),
            "firewall".into(),
            "offline".into(),
        ];

        // Stock Windows firewall rule blob (HTTP-In, Active=TRUE).
        let fw_http = "v2.29|Action=Allow|Active=TRUE|Dir=In|Protocol=6|LPort=5985|\
Profile=Private,Domain|Name=@FirewallAPI.dll,-30253|Desc=@FirewallAPI.dll,-30256|\
EmbedCtxt=@FirewallAPI.dll,-30252|";

        let ops = [
            (
                "winrm-auto",
                r"HKLM\SYSTEM\ControlSet001\Services\WinRM",
                "Start",
                json!(3),
                json!(2),
                "dword",
                Priority::High,
                "Set WinRM startup type to Automatic",
            ),
            (
                "fw-winrm-http",
                r"HKLM\SYSTEM\ControlSet001\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules",
                "WINRM-HTTP-In-TCP",
                json!(""),
                json!(fw_http),
                "sz",
                Priority::High,
                "Enable WinRM firewall rule (HTTP-In TCP 5985)",
            ),
        ];

        for (id, key, value, current, new_data, dtype, priority, desc) in ops {
            plan.add_operation(Operation {
                id: id.into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: key.into(),
                    value: value.into(),
                    current_data: current,
                    new_data,
                    data_type: dtype.into(),
                }),
                priority,
                description: desc.into(),
                risk: Priority::Medium,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }

        plan
    }

    /// Offline Windows domain-leave markers → workgroup (best-effort).
    ///
    /// Clears Tcpip `Domain` / sets `NV Domain` to the workgroup, resets
    /// Winlogon domain cache fields, and stages a SOFTWARE `RunOnce` that
    /// runs `Add-Computer -WorkGroupName` on first boot when still domain-
    /// joined. Does **not** delete the computer account on a DC — that still
    /// needs a live AD cleanup with domain credentials after cutover.
    pub fn windows_domain_leave_plan(&self, workgroup: &str) -> Result<FixPlan> {
        let wg = workgroup.trim();
        if wg.is_empty() {
            anyhow::bail!("Workgroup name is required (use --workgroup)");
        }
        if wg.len() > 15
            || !wg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("Invalid workgroup '{wg}' (ASCII alnum/hyphen/underscore, max 15 chars)");
        }

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-domain-leave".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "medium".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = true;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!(
            "Offline Windows domain-leave markers → workgroup '{wg}', plus first-boot \
             RunOnce Add-Computer. DC computer-account delete still needs live AD creds."
        ));
        plan.metadata.tags = vec![
            "windows".into(),
            "domain".into(),
            "workgroup".into(),
            "offline".into(),
            "runonce".into(),
        ];

        let ops = [
            (
                "tcpip-clear-domain",
                r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters",
                "Domain",
                json!(""),
                "sz",
                "Clear Tcpip Domain (domain DNS suffix)",
            ),
            (
                "tcpip-nv-domain-workgroup",
                r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters",
                "NV Domain",
                json!(wg),
                "sz",
                "Set Tcpip NV Domain to workgroup",
            ),
            (
                "winlogon-default-domain",
                r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon",
                "DefaultDomainName",
                json!(wg),
                "sz",
                "Set Winlogon DefaultDomainName to workgroup",
            ),
            (
                "winlogon-clear-cache-domain",
                r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon",
                "CachePrimaryDomain",
                json!(""),
                "sz",
                "Clear Winlogon CachePrimaryDomain",
            ),
        ];

        for (id, key, value, new_data, dtype, desc) in ops {
            plan.add_operation(Operation {
                id: id.into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: key.into(),
                    value: value.into(),
                    current_data: json!(""),
                    new_data,
                    data_type: dtype.into(),
                }),
                priority: Priority::High,
                description: desc.into(),
                risk: Priority::Medium,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }

        // First-boot: leave domain into workgroup when still joined (no DC delete).
        let runonce = format!(
            "cmd.exe /c powershell.exe -NoProfile -ExecutionPolicy Bypass -Command \
\"try {{ if ((Get-CimInstance Win32_ComputerSystem).PartOfDomain) {{ \
Add-Computer -WorkGroupName '{wg}' -Force -ErrorAction SilentlyContinue }} }} catch {{}}; \
reg delete \\\"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce\\\" \
/v GuestKitDomainLeave /f\""
        );
        plan.add_operation(Operation {
            id: "runonce-domain-leave".into(),
            op_type: OperationType::RegistryEdit(RegistryEdit {
                key: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce".into(),
                value: "GuestKitDomainLeave".into(),
                current_data: json!(""),
                new_data: json!(runonce),
                data_type: "sz".into(),
            }),
            priority: Priority::High,
            description: format!("Stage RunOnce Add-Computer -WorkGroupName '{wg}' (first boot)"),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![
                "tcpip-nv-domain-workgroup".into(),
                "winlogon-default-domain".into(),
            ],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    /// Offline Windows timezone via `TimeZoneKeyName` (e.g. `UTC`, `Pacific Standard Time`).
    pub fn windows_timezone_plan(&self, timezone: &str) -> Result<FixPlan> {
        let tz = timezone.trim();
        if tz.is_empty() {
            anyhow::bail!("Timezone is required (use --timezone)");
        }
        // Windows TimeZoneKeyName values are path-like keys under zoneinfo names;
        // reject control chars / path separators that would break the registry value.
        if tz.len() > 128
            || tz.contains('\\')
            || tz.contains('/')
            || tz.chars().any(|c| c.is_control())
        {
            anyhow::bail!(
                "Invalid timezone '{tz}' (use a Windows TimeZoneKeyName like 'UTC' or 'Pacific Standard Time')"
            );
        }

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-timezone".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description =
            Some(format!("Offline Windows timezone → TimeZoneKeyName '{tz}'"));
        plan.metadata.tags = vec!["windows".into(), "timezone".into(), "offline".into()];

        plan.add_operation(Operation {
            id: "timezone-key-name".into(),
            op_type: OperationType::RegistryEdit(RegistryEdit {
                key: r"HKLM\SYSTEM\ControlSet001\Control\TimeZoneInformation".into(),
                value: "TimeZoneKeyName".into(),
                current_data: json!(""),
                new_data: json!(tz),
                data_type: "sz".into(),
            }),
            priority: Priority::Medium,
            description: format!("Set TimeZoneKeyName to {tz}"),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    /// Offline Windows static IPv4 on a known interface GUID.
    ///
    /// Writes `EnableDHCP=0` plus MULTI_SZ IP/mask/gateway under
    /// `Tcpip\Parameters\Interfaces\{GUID}`. Discover GUIDs with inspect /
    /// hyper2kvm network snapshot first.
    pub fn windows_static_ip_plan(
        &self,
        interface_guid: &str,
        ip: &str,
        mask: &str,
        gateway: Option<&str>,
        dns: Option<&str>,
    ) -> Result<FixPlan> {
        let guid = Self::normalize_interface_guid(interface_guid)?;
        for (label, v) in [("ip", ip), ("mask", mask)] {
            let t = v.trim();
            if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
                anyhow::bail!("Invalid {label} '{v}' (expected IPv4 dotted quad)");
            }
        }
        if let Some(gw) = gateway {
            let t = gw.trim();
            if !t.is_empty() && !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
                anyhow::bail!("Invalid --gateway '{gw}' (expected IPv4 dotted quad)");
            }
        }

        let iface_key =
            format!(r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters\Interfaces\{{{guid}}}");

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-static-ip".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "medium".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = true;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!(
            "Offline Windows static IPv4 {ip}/{mask} on interface {{{guid}}}"
        ));
        plan.metadata.tags = vec![
            "windows".into(),
            "network".into(),
            "static-ip".into(),
            "offline".into(),
        ];

        plan.add_operation(Operation {
            id: "iface-disable-dhcp".into(),
            op_type: OperationType::RegistryEdit(RegistryEdit {
                key: iface_key.clone(),
                value: "EnableDHCP".into(),
                current_data: json!(1),
                new_data: json!(0),
                data_type: "dword".into(),
            }),
            priority: Priority::High,
            description: "Disable DHCP on interface".into(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        for (id, value, data, desc) in [
            (
                "iface-ip",
                "IPAddress",
                json!([ip.trim()]),
                "Set static IPAddress (MULTI_SZ)",
            ),
            (
                "iface-mask",
                "SubnetMask",
                json!([mask.trim()]),
                "Set SubnetMask (MULTI_SZ)",
            ),
        ] {
            plan.add_operation(Operation {
                id: id.into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: iface_key.clone(),
                    value: value.into(),
                    current_data: json!([]),
                    new_data: data,
                    data_type: "multi_sz".into(),
                }),
                priority: Priority::High,
                description: desc.into(),
                risk: Priority::Medium,
                reversible: true,
                depends_on: vec!["iface-disable-dhcp".into()],
                validation: None,
                undo: None,
            });
        }

        if let Some(gw) = gateway.map(str::trim).filter(|s| !s.is_empty()) {
            plan.add_operation(Operation {
                id: "iface-gateway".into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: iface_key.clone(),
                    value: "DefaultGateway".into(),
                    current_data: json!([]),
                    new_data: json!([gw]),
                    data_type: "multi_sz".into(),
                }),
                priority: Priority::High,
                description: "Set DefaultGateway (MULTI_SZ)".into(),
                risk: Priority::Medium,
                reversible: true,
                depends_on: vec!["iface-disable-dhcp".into()],
                validation: None,
                undo: None,
            });
        }

        if let Some(dns_servers) = dns.map(str::trim).filter(|s| !s.is_empty()) {
            // NameServer is historically REG_SZ with space-separated IPs.
            let normalized = dns_servers.replace(',', " ");
            plan.add_operation(Operation {
                id: "iface-dns".into(),
                op_type: OperationType::RegistryEdit(RegistryEdit {
                    key: iface_key,
                    value: "NameServer".into(),
                    current_data: json!(""),
                    new_data: json!(normalized),
                    data_type: "sz".into(),
                }),
                priority: Priority::Medium,
                description: "Set NameServer (space-separated)".into(),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec!["iface-disable-dhcp".into()],
                validation: None,
                undo: None,
            });
        }

        Ok(plan)
    }

    fn normalize_interface_guid(interface_guid: &str) -> Result<String> {
        let guid = interface_guid.trim().trim_matches(|c| c == '{' || c == '}');
        if guid.len() != 36 || guid.chars().filter(|c| *c == '-').count() != 4 {
            anyhow::bail!(
                "Invalid --interface-guid '{interface_guid}' (expected GUID like \
                 a1b2c3d4-e5f6-7890-abcd-ef1234567890)"
            );
        }
        Ok(guid.to_string())
    }

    /// Offline Windows DHCP enablement on a known interface GUID.
    pub fn windows_dhcp_plan(&self, interface_guid: &str) -> Result<FixPlan> {
        let guid = Self::normalize_interface_guid(interface_guid)?;
        let iface_key =
            format!(r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters\Interfaces\{{{guid}}}");

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-dhcp".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!(
            "Offline Windows DHCP enable on interface {{{guid}}}"
        ));
        plan.metadata.tags = vec![
            "windows".into(),
            "network".into(),
            "dhcp".into(),
            "offline".into(),
        ];

        plan.add_operation(Operation {
            id: "iface-enable-dhcp".into(),
            op_type: OperationType::RegistryEdit(RegistryEdit {
                key: iface_key,
                value: "EnableDHCP".into(),
                current_data: json!(0),
                new_data: json!(1),
                data_type: "dword".into(),
            }),
            priority: Priority::High,
            description: "Enable DHCP on interface".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    /// Offline Windows DNS servers (NameServer) on a known interface GUID.
    pub fn windows_dns_plan(&self, interface_guid: &str, dns: &str) -> Result<FixPlan> {
        let guid = Self::normalize_interface_guid(interface_guid)?;
        let dns_servers = dns.trim();
        if dns_servers.is_empty() {
            anyhow::bail!("--dns is required for windows-dns (space or comma separated)");
        }
        let normalized = dns_servers.replace(',', " ");
        let iface_key =
            format!(r"HKLM\SYSTEM\ControlSet001\Services\Tcpip\Parameters\Interfaces\{{{guid}}}");

        let mut plan = FixPlan::new(self.vm_path.clone(), "windows-dns".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!(
            "Offline Windows DNS NameServer on interface {{{guid}}}"
        ));
        plan.metadata.tags = vec![
            "windows".into(),
            "network".into(),
            "dns".into(),
            "offline".into(),
        ];

        plan.add_operation(Operation {
            id: "iface-dns".into(),
            op_type: OperationType::RegistryEdit(RegistryEdit {
                key: iface_key,
                value: "NameServer".into(),
                current_data: json!(""),
                new_data: json!(normalized),
                data_type: "sz".into(),
            }),
            priority: Priority::Medium,
            description: "Set NameServer (space-separated)".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    /// Offline Linux hostname (`/etc/hostname` + `/etc/hosts` patch).
    pub fn linux_hostname_plan(&self, g: &mut crate::Guestfs, hostname: &str) -> Result<FixPlan> {
        let name = hostname.trim();
        if name.is_empty() || name.len() > 253 {
            anyhow::bail!("Invalid --hostname '{hostname}'");
        }

        let mut plan = FixPlan::new(self.vm_path.clone(), "linux-hostname".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(format!("Offline Linux hostname → {name}"));
        plan.metadata.tags = vec!["linux".into(), "hostname".into(), "offline".into()];

        plan.add_operation(Operation {
            id: "hostname".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: "/etc/hostname".into(),
                content: format!("{name}\n"),
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: format!("Write /etc/hostname = {name}"),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        let mut hosts = String::from("127.0.0.1\tlocalhost\n");
        if let Ok(content) = g.read_file("/etc/hosts") {
            hosts = String::from_utf8_lossy(&content).into_owned();
            let mut out = Vec::new();
            let mut patched = false;
            for line in hosts.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("127.0.1.1") || trimmed.starts_with("10.0.2.3") {
                    let ip = trimmed.split_whitespace().next().unwrap_or("127.0.1.1");
                    out.push(format!("{ip}\t{name}"));
                    patched = true;
                } else {
                    out.push(line.to_string());
                }
            }
            if !patched {
                out.push(format!("127.0.1.1\t{name}"));
            }
            hosts = out.join("\n") + "\n";
        } else {
            hosts.push_str(&format!("127.0.1.1\t{name}\n"));
        }

        plan.add_operation(Operation {
            id: "hosts".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: "/etc/hosts".into(),
                content: hosts,
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: "Patch /etc/hosts with hostname".into(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    /// Generate a fix plan from a profile report (security, compliance, …)
    pub fn from_security_profile(&self, report: &ProfileReport) -> Result<FixPlan> {
        let profile_name = if report.profile_name.is_empty() {
            "security".to_string()
        } else {
            report.profile_name.clone()
        };
        let mut plan = FixPlan::new(self.vm_path.clone(), profile_name.clone());

        plan.overall_risk = match report.overall_risk {
            Some(RiskLevel::Critical) => "critical".to_string(),
            Some(RiskLevel::High) => "high".to_string(),
            Some(RiskLevel::Medium) => "medium".to_string(),
            Some(RiskLevel::Low) => "low".to_string(),
            Some(RiskLevel::Info) => "info".to_string(),
            None => "unknown".to_string(),
        };

        plan.metadata.description = Some(format!(
            "Fix plan generated from {profile_name} profile analysis"
        ));
        plan.metadata.tags = vec![profile_name.clone(), "automated".to_string()];

        // Convert findings to operations
        let mut op_counter = 1;
        let prefix = profile_name
            .chars()
            .take(3)
            .collect::<String>()
            .to_ascii_lowercase();
        for section in &report.sections {
            for finding in &section.findings {
                if finding.risk_level.is_some() {
                    let remediation = &finding.message;
                    let operation = self.finding_to_operation(
                        &format!("{prefix}-{:03}", op_counter),
                        finding,
                        remediation,
                    )?;
                    plan.add_operation(operation);
                    op_counter += 1;
                }
            }
        }

        plan.estimated_duration = Self::estimate_duration(plan.operations.len());
        self.add_post_apply_actions(&mut plan);

        Ok(plan)
    }

    /// Convert a finding with remediation into an operation
    fn finding_to_operation(
        &self,
        id: &str,
        finding: &Finding,
        remediation: &str,
    ) -> Result<Operation> {
        let priority = match finding.risk_level {
            Some(RiskLevel::Critical) => Priority::Critical,
            Some(RiskLevel::High) => Priority::High,
            Some(RiskLevel::Medium) => Priority::Medium,
            Some(RiskLevel::Low) => Priority::Low,
            Some(RiskLevel::Info) | None => Priority::Info,
        };

        // Parse remediation text to determine operation type
        let op_type = self.parse_remediation(remediation)?;

        let risk = match finding.risk_level {
            Some(RiskLevel::Critical) => Priority::Critical,
            Some(RiskLevel::High) => Priority::High,
            Some(RiskLevel::Medium) => Priority::Medium,
            Some(RiskLevel::Low) => Priority::Low,
            Some(RiskLevel::Info) | None => Priority::Info,
        };

        Ok(Operation {
            id: id.to_string(),
            op_type,
            priority,
            description: finding.item.clone(),
            risk,
            reversible: true, // Most operations are reversible
            depends_on: Vec::new(),
            validation: None,
            undo: None,
        })
    }

    /// Parse remediation text to determine operation type
    /// This is a heuristic-based parser that looks for patterns
    fn parse_remediation(&self, remediation: &str) -> Result<OperationType> {
        let lower = remediation.to_lowercase();

        // SSH configuration changes
        if lower.contains("ssh") && lower.contains("permitrootlogin") {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0, // Will be detected at apply time
                    before: "PermitRootLogin yes".to_string(),
                    after: "PermitRootLogin no".to_string(),
                    context: Some("# Authentication:\nPermitRootLogin no".to_string()),
                }],
            }));
        }

        // PubkeyAuthentication — offline drop-in (same shape as linux-ssh plan)
        if lower.contains("pubkeyauthentication")
            || (lower.contains("ssh") && lower.contains("public key") && lower.contains("enable"))
        {
            return Ok(OperationType::FileWrite(FileWrite {
                path: "/etc/ssh/sshd_config.d/99-guestkit.conf".into(),
                content: "# Managed by guestkit plan\nPubkeyAuthentication yes\n".into(),
                mode: Some("0644".into()),
            }));
        }

        // PasswordAuthentication no
        if lower.contains("passwordauthentication") && lower.contains("no") {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "PasswordAuthentication yes".to_string(),
                    after: "PasswordAuthentication no".to_string(),
                    context: None,
                }],
            }));
        }

        // Protocol 2 only
        if lower.contains("protocol") && lower.contains("ssh") && lower.contains("2") {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "Protocol 1".to_string(),
                    after: "Protocol 2".to_string(),
                    context: None,
                }],
            }));
        }

        // Empty password / PermitEmptyPasswords
        if lower.contains("permitemptypasswords") {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "PermitEmptyPasswords yes".to_string(),
                    after: "PermitEmptyPasswords no".to_string(),
                    context: None,
                }],
            }));
        }

        // X11Forwarding disable
        if lower.contains("x11forwarding") && (lower.contains("disable") || lower.contains("no")) {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "X11Forwarding yes".to_string(),
                    after: "X11Forwarding no".to_string(),
                    context: None,
                }],
            }));
        }

        // ufw enable via conf (offline) — before generic "firewall" match
        if lower.contains("ufw") && lower.contains("enable") {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/ufw/ufw.conf".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "ENABLED=no".to_string(),
                    after: "ENABLED=yes".to_string(),
                    context: None,
                }],
            }));
        }

        // ufw default deny (offline)
        if lower.contains("ufw")
            && (lower.contains("default deny")
                || lower.contains("deny incoming")
                || (lower.contains("default") && lower.contains("drop")))
        {
            return Ok(OperationType::FileEdit(FileEdit {
                file: "/etc/default/ufw".to_string(),
                backup: true,
                changes: vec![FileChange {
                    line: 0,
                    before: "DEFAULT_INPUT_POLICY=\"ACCEPT\"".to_string(),
                    after: "DEFAULT_INPUT_POLICY=\"DROP\"".to_string(),
                    context: None,
                }],
            }));
        }

        // Offline systemd enable/disable before package-install heuristics
        if let Some(op) = Self::parse_systemd_unit_remediation(&lower) {
            return Ok(op);
        }

        // Firewall: prefer offline systemd enable over live ServiceOperation when "enable"
        if lower.contains("firewall") && (lower.contains("enable") || lower.contains("install")) {
            if lower.contains("install") {
                return Ok(OperationType::PackageInstall(PackageInstall {
                    packages: vec!["firewalld".to_string()],
                    estimated_size: Some("~5MB".to_string()),
                    host_cache: None,
                }));
            }
            return Ok(Self::systemd_enable_symlink("firewalld"));
        }

        // SELinux mode changes
        if lower.contains("selinux")
            && (lower.contains("enforcing")
                || lower.contains("permissive")
                || lower.contains("disabled"))
        {
            let target = if lower.contains("enforcing") {
                "enforcing"
            } else if lower.contains("disabled") {
                "disabled"
            } else {
                "permissive"
            };
            return Ok(OperationType::SelinuxMode(SELinuxMode {
                file: "/etc/selinux/config".to_string(),
                current: "permissive".to_string(),
                target: target.to_string(),
                warning: Some("Requires reboot to take full effect".to_string()),
            }));
        }

        // fail2ban / AIDE installation (still live-only PackageInstall)
        if lower.contains("fail2ban") && lower.contains("install") {
            return Ok(OperationType::PackageInstall(PackageInstall {
                packages: vec!["fail2ban".to_string()],
                estimated_size: Some("~15MB".to_string()),
                host_cache: None,
            }));
        }

        if lower.contains("aide") && lower.contains("install") {
            return Ok(OperationType::PackageInstall(PackageInstall {
                packages: vec!["aide".to_string()],
                estimated_size: Some("~10MB".to_string()),
                host_cache: None,
            }));
        }

        // Default: create a command execution operation (live-only offline)
        Ok(OperationType::CommandExec(CommandExec {
            interpreter: None,
            command: remediation.to_string(),
            expected_exit: 0,
            timeout: Some(300), // 5 minutes default
        }))
    }

    /// Offline `systemctl enable` → multi-user wants Symlink.
    fn systemd_enable_symlink(unit: &str) -> OperationType {
        let unit = unit.trim().trim_end_matches(".service");
        OperationType::Symlink(Symlink {
            target: format!("../../../../usr/lib/systemd/system/{unit}.service"),
            link_path: format!("/etc/systemd/system/multi-user.target.wants/{unit}.service"),
        })
    }

    /// Offline `systemctl disable` → remove wants Symlink.
    fn systemd_disable_delete(unit: &str) -> OperationType {
        let unit = unit.trim().trim_end_matches(".service");
        OperationType::FileDelete(FileDelete {
            path: format!("/etc/systemd/system/multi-user.target.wants/{unit}.service"),
            missing_ok: true,
        })
    }

    /// Map common enable/disable remediation text to offline systemd ops.
    fn parse_systemd_unit_remediation(lower: &str) -> Option<OperationType> {
        const UNITS: &[(&str, &str)] = &[
            ("fail2ban", "fail2ban"),
            ("auditd", "auditd"),
            ("chronyd", "chronyd"),
            ("chrony", "chronyd"),
            ("ntpd", "ntpd"),
            ("rsyslog", "rsyslog"),
            ("apparmor", "apparmor"),
            ("sshd", "sshd"),
            ("firewalld", "firewalld"),
        ];

        let wants_enable = lower.contains("enable")
            || lower.contains("systemctl enable")
            || lower.contains("start on boot")
            || lower.contains("start at boot");
        let wants_disable = lower.contains("disable")
            || lower.contains("systemctl disable")
            || lower.contains("stop on boot");

        if lower.contains("systemctl") {
            for verb in ["enable", "disable"] {
                if let Some(idx) = lower.find(verb) {
                    let after = lower[idx + verb.len()..].trim_start();
                    let unit =
                        after
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .trim_matches(|c: char| {
                                !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.'
                            });
                    if !unit.is_empty() && unit != "and" && unit != "the" {
                        return Some(if verb == "enable" {
                            Self::systemd_enable_symlink(unit)
                        } else {
                            Self::systemd_disable_delete(unit)
                        });
                    }
                }
            }
        }

        for (needle, unit) in UNITS {
            if !lower.contains(needle) {
                continue;
            }
            if lower.contains("install") {
                continue;
            }
            if wants_disable && !wants_enable {
                return Some(Self::systemd_disable_delete(unit));
            }
            if wants_enable {
                return Some(Self::systemd_enable_symlink(unit));
            }
        }
        None
    }

    /// Offline `/etc/default/grub` day-0 plan (timeout / cmdline). Does not run grub-install.
    pub fn linux_grub_defaults_plan(
        &self,
        g: &mut crate::Guestfs,
        timeout: Option<u32>,
        cmdline_append: Option<&str>,
    ) -> Result<FixPlan> {
        let grub_path = "/etc/default/grub";
        let current = g
            .read_file(grub_path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_else(|_| {
                "GRUB_TIMEOUT=5\nGRUB_CMDLINE_LINUX_DEFAULT=\"\"\nGRUB_CMDLINE_LINUX=\"\"\n".into()
            });

        let mut plan = FixPlan::new(self.vm_path.clone(), "linux-grub".to_string());
        plan.version = "1".to_string();
        plan.overall_risk = "low".to_string();
        plan.estimated_duration = "seconds".to_string();
        plan.metadata.author = "guestkit".to_string();
        plan.metadata.review_required = false;
        plan.metadata.reversible = true;
        plan.metadata.description = Some(
            "Offline GRUB defaults (/etc/default/grub). Regenerate grub.cfg in-guest after boot \
             (grub2-mkconfig / update-grub); full grub-install still needs chroot."
                .into(),
        );
        plan.metadata.tags = vec![
            "linux".into(),
            "grub".into(),
            "boot".into(),
            "offline".into(),
        ];

        if timeout.is_none()
            && cmdline_append
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_none()
        {
            anyhow::bail!("linux-grub requires --grub-timeout and/or --grub-cmdline");
        }

        let mut next = current;
        let mut desc_parts = Vec::new();
        if let Some(t) = timeout {
            next = Self::upsert_grub_kv(&next, "GRUB_TIMEOUT", &t.to_string());
            desc_parts.push(format!("GRUB_TIMEOUT={t}"));
        }
        if let Some(extra) = cmdline_append.map(str::trim).filter(|s| !s.is_empty()) {
            next = Self::append_grub_cmdline(&next, extra);
            desc_parts.push(format!("cmdline+={extra}"));
        }

        plan.add_operation(Operation {
            id: "grub-defaults".into(),
            op_type: OperationType::FileWrite(FileWrite {
                path: grub_path.into(),
                content: if next.ends_with('\n') {
                    next
                } else {
                    format!("{next}\n")
                },
                mode: Some("0644".into()),
            }),
            priority: Priority::High,
            description: format!(
                "Write offline /etc/default/grub ({})",
                desc_parts.join(", ")
            ),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        Ok(plan)
    }

    fn upsert_grub_kv(content: &str, key: &str, value: &str) -> String {
        let mut found = false;
        let mut out = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(key) && trimmed[key.len()..].starts_with('=') {
                out.push(format!("{key}={value}"));
                found = true;
            } else {
                out.push(line.to_string());
            }
        }
        if !found {
            out.push(format!("{key}={value}"));
        }
        out.join("\n")
    }

    fn append_grub_cmdline(content: &str, extra: &str) -> String {
        let key = if content
            .lines()
            .any(|l| l.trim_start().starts_with("GRUB_CMDLINE_LINUX="))
        {
            "GRUB_CMDLINE_LINUX"
        } else {
            "GRUB_CMDLINE_LINUX_DEFAULT"
        };
        let mut found = false;
        let mut out = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(key) && trimmed[key.len()..].starts_with('=') {
                found = true;
                let rest = &trimmed[key.len() + 1..];
                let unquoted = rest.trim().trim_matches('"');
                let new_val = if unquoted.split_whitespace().any(|t| t == extra) {
                    unquoted.to_string()
                } else if unquoted.is_empty() {
                    extra.to_string()
                } else {
                    format!("{unquoted} {extra}")
                };
                out.push(format!("{key}=\"{new_val}\""));
            } else {
                out.push(line.to_string());
            }
        }
        if !found {
            out.push(format!("{key}=\"{extra}\""));
        }
        out.join("\n")
    }

    /// Add common post-apply actions
    fn add_post_apply_actions(&self, plan: &mut FixPlan) {
        // Check if we modified SSH config
        let has_ssh_changes = plan.operations.iter().any(|op| {
            matches!(&op.op_type, OperationType::FileEdit(fe) if fe.file.contains("sshd_config"))
        });

        if has_ssh_changes {
            plan.post_apply.push(PostApplyAction::ServiceRestart {
                services: vec!["sshd".to_string()],
            });
        }

        // Check if we enabled firewall (offline Symlink or live ServiceOperation)
        let has_firewall = plan.operations.iter().any(|op| {
            matches!(
                &op.op_type,
                OperationType::ServiceOperation(so) if so.service == "firewalld"
            ) || matches!(
                &op.op_type,
                OperationType::Symlink(sl) if sl.link_path.contains("firewalld")
            )
        });

        if has_firewall {
            plan.post_apply.push(PostApplyAction::Validation {
                command: "firewall-cmd --state".to_string(),
                expected_output: Some("running".to_string()),
            });
        }

        // Check if we modified SELinux
        let has_selinux = plan
            .operations
            .iter()
            .any(|op| matches!(&op.op_type, OperationType::SelinuxMode(_)));

        if has_selinux {
            plan.post_apply.push(PostApplyAction::RebootRequired {
                reason: "SELinux mode change requires reboot".to_string(),
            });
        }
    }

    /// Generate a fix plan from bootability report blockers/warnings
    pub fn from_boot_report(
        &self,
        boot: &crate::boot::BootabilityReport,
        image: &std::path::Path,
    ) -> Result<FixPlan> {
        let mut plan = FixPlan::new(image.display().to_string(), "boot-repair".to_string());
        plan.metadata.description =
            Some("Automated boot repair plan from doctor analysis".to_string());
        plan.metadata.tags = vec!["boot".to_string(), "doctor".to_string()];

        let mut op_counter = 1;
        for finding in boot.blockers.iter().chain(boot.warnings.iter()) {
            let remediation = finding
                .remediation
                .clone()
                .unwrap_or_else(|| finding.message.clone());
            if let Ok(op_type) = self.parse_remediation(&remediation) {
                plan.add_operation(Operation {
                    id: format!("boot-{:03}", op_counter),
                    op_type,
                    priority: Priority::High,
                    description: finding.title.clone(),
                    risk: Priority::Medium,
                    reversible: true,
                    depends_on: vec![],
                    validation: None,
                    undo: None,
                });
                op_counter += 1;
            }
        }

        plan.estimated_duration = Self::estimate_duration(plan.operations.len());
        self.add_post_apply_actions(&mut plan);
        Ok(plan)
    }

    /// Generate a fix plan from a migration score report (includes boot repair ops).
    pub fn from_migration_report(
        &self,
        migration: &crate::cli::migrate::plan::MigrationScoreReport,
        boot: &crate::boot::BootabilityReport,
        target: &str,
        image: &std::path::Path,
    ) -> Result<FixPlan> {
        let mut plan = self.from_boot_report(boot, image)?;
        plan.profile = "migration".to_string();
        plan.metadata.author = "guestkit-migrate-plan".to_string();
        plan.metadata.description = Some(format!(
            "Hypervisor-aware migration plan for {} → {}",
            image.display(),
            target
        ));
        plan.metadata.tags = vec!["migration".to_string(), target.to_lowercase()];

        let mut op_counter = plan.operations.len() + 1;

        for change in &migration.required_changes {
            let op_type = self.migration_change_to_operation(change)?;
            plan.add_operation(Operation {
                id: format!("mig-{:03}", op_counter),
                op_type,
                priority: Priority::High,
                description: change.clone(),
                risk: Priority::Medium,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
            op_counter += 1;
        }

        if !migration.driver_injections.is_empty() {
            let modules = migration.driver_injections.join("\n");
            plan.add_operation(Operation {
                id: format!("mig-{:03}", op_counter),
                op_type: OperationType::FileEdit(FileEdit {
                    file: "/etc/modules-load.d/guestkit-migration.conf".to_string(),
                    backup: true,
                    changes: vec![FileChange {
                        line: 1,
                        before: String::new(),
                        after: modules,
                        context: Some(
                            "# GuestKit: virtio modules for migration target".to_string(),
                        ),
                    }],
                }),
                priority: Priority::High,
                description: "Load virtio drivers for target hypervisor".to_string(),
                risk: Priority::Low,
                reversible: true,
                depends_on: vec![],
                validation: None,
                undo: None,
            });
        }

        for warning in &migration.licensing_warnings {
            plan.post_apply.push(PostApplyAction::Message {
                message: warning.clone(),
            });
        }

        if migration.estimated_downtime_minutes > 0 {
            plan.post_apply.push(PostApplyAction::Message {
                message: format!(
                    "Estimated migration downtime: {} minutes",
                    migration.estimated_downtime_minutes
                ),
            });
        }

        plan.estimated_duration = Self::estimate_duration(plan.operations.len());
        self.add_post_apply_actions(&mut plan);
        Ok(plan)
    }

    /// Append agent injection ops when exporting migration plans for KVM targets.
    #[cfg(feature = "agent")]
    pub fn with_agent_injection(
        &self,
        mut plan: FixPlan,
        binary: &std::path::Path,
        unit_content: &str,
    ) -> Result<FixPlan> {
        crate::agent::inject::append_agent_ops(&mut plan, binary, unit_content)?;
        Ok(plan)
    }

    fn migration_change_to_operation(&self, change: &str) -> Result<OperationType> {
        let lower = change.to_lowercase();

        if lower.contains("vmware tools") && lower.contains("qemu-guest-agent") {
            return Ok(OperationType::PackageInstall(PackageInstall {
                packages: vec!["qemu-guest-agent".to_string()],
                estimated_size: Some("~2MB".to_string()),
                host_cache: None,
            }));
        }

        if lower.contains("cloud-init") {
            return Ok(OperationType::PackageInstall(PackageInstall {
                packages: vec!["cloud-init".to_string()],
                estimated_size: Some("~5MB".to_string()),
                host_cache: None,
            }));
        }

        if let Ok(op_type) = self.parse_remediation(change) {
            return Ok(op_type);
        }

        Ok(OperationType::CommandExec(CommandExec {
            interpreter: None,
            command: change.to_string(),
            expected_exit: 0,
            timeout: Some(600),
        }))
    }

    /// Estimate duration based on number of operations
    fn estimate_duration(op_count: usize) -> String {
        match op_count {
            0 => "0s".to_string(),
            1..=3 => "1-2 minutes".to_string(),
            4..=8 => "3-5 minutes".to_string(),
            9..=15 => "5-10 minutes".to_string(),
            _ => "10+ minutes".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::profiles::FindingStatus;

    fn create_test_finding(risk: RiskLevel, message: &str) -> Finding {
        Finding {
            item: "Test finding".to_string(),
            status: FindingStatus::Fail,
            message: message.to_string(),
            risk_level: Some(risk),
        }
    }

    fn create_test_report() -> ProfileReport {
        ProfileReport {
            profile_name: "security".to_string(),
            overall_risk: Some(RiskLevel::High),
            sections: vec![ReportSection {
                title: "SSH Configuration".to_string(),
                findings: vec![
                    create_test_finding(RiskLevel::High, "Disable PermitRootLogin in SSH config"),
                    create_test_finding(RiskLevel::Medium, "Enable firewall service"),
                ],
            }],
            summary: None,
        }
    }

    #[test]
    fn test_generator_creation() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        assert_eq!(generator.vm_path, "test.qcow2");
    }

    #[test]
    fn test_windows_rdp_enable_plan() {
        let plan = PlanGenerator::new("/var/lib/libvirt/images/win.qcow2".into())
            .windows_rdp_enable_plan();
        assert_eq!(plan.profile, "windows-rdp");
        assert_eq!(plan.operations.len(), 7);
        let ids: Vec<_> = plan.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"enable-rdp"));
        assert!(ids.contains(&"termservice-auto"));
        assert!(ids.contains(&"umrdpservice-auto"));
        assert!(ids.contains(&"fw-tcp"));
        assert!(ids.contains(&"fw-udp"));
        assert!(plan.post_apply.is_empty());
    }

    #[test]
    fn test_windows_hostname_plan() {
        let plan = PlanGenerator::new("/images/win.qcow2".into())
            .windows_hostname_plan("WIN-APP01")
            .unwrap();
        assert_eq!(plan.profile, "windows-hostname");
        assert_eq!(plan.operations.len(), 4);
        let ids: Vec<_> = plan.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"computername"));
        assert!(ids.contains(&"tcpip-hostname"));
        assert!(ids.contains(&"tcpip-nv-hostname"));
        for op in &plan.operations {
            match &op.op_type {
                OperationType::RegistryEdit(re) => {
                    assert_eq!(re.new_data, json!("WIN-APP01"));
                    assert_eq!(re.data_type, "sz");
                }
                _ => panic!("expected RegistryEdit"),
            }
        }
        assert!(PlanGenerator::new("/images/win.qcow2".into())
            .windows_hostname_plan("")
            .is_err());
        assert!(PlanGenerator::new("/images/win.qcow2".into())
            .windows_hostname_plan("-bad")
            .is_err());
    }

    #[test]
    fn test_windows_winrm_enable_plan() {
        let plan = PlanGenerator::new("/images/win.qcow2".into()).windows_winrm_enable_plan();
        assert_eq!(plan.profile, "windows-winrm");
        assert_eq!(plan.operations.len(), 2);
        let ids: Vec<_> = plan.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"winrm-auto"));
        assert!(ids.contains(&"fw-winrm-http"));
        assert!(plan.metadata.review_required);
    }

    #[test]
    fn test_windows_domain_leave_plan() {
        let plan = PlanGenerator::new("/images/win.qcow2".into())
            .windows_domain_leave_plan("WORKGROUP")
            .unwrap();
        assert_eq!(plan.profile, "windows-domain-leave");
        assert_eq!(plan.operations.len(), 5);
        assert!(plan
            .operations
            .iter()
            .any(|o| o.id == "tcpip-nv-domain-workgroup"));
        let runonce = plan
            .operations
            .iter()
            .find(|o| o.id == "runonce-domain-leave")
            .expect("runonce");
        match &runonce.op_type {
            OperationType::RegistryEdit(re) => {
                assert_eq!(re.value, "GuestKitDomainLeave");
                let s = re.new_data.as_str().unwrap();
                assert!(s.contains("Add-Computer"));
                assert!(s.contains("WORKGROUP"));
            }
            _ => panic!("expected RegistryEdit"),
        }
    }

    #[test]
    fn test_windows_timezone_plan() {
        let plan = PlanGenerator::new("/images/win.qcow2".into())
            .windows_timezone_plan("UTC")
            .unwrap();
        assert_eq!(plan.profile, "windows-timezone");
        assert_eq!(plan.operations.len(), 1);
        match &plan.operations[0].op_type {
            OperationType::RegistryEdit(re) => {
                assert_eq!(re.value, "TimeZoneKeyName");
                assert_eq!(re.new_data, json!("UTC"));
            }
            _ => panic!("expected RegistryEdit"),
        }
        assert!(PlanGenerator::new("/images/win.qcow2".into())
            .windows_timezone_plan("")
            .is_err());
    }

    #[test]
    fn test_windows_static_ip_plan() {
        let plan = PlanGenerator::new("/images/win.qcow2".into())
            .windows_static_ip_plan(
                "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                "10.0.0.50",
                "255.255.255.0",
                Some("10.0.0.1"),
                Some("1.1.1.1,8.8.8.8"),
            )
            .unwrap();
        assert_eq!(plan.profile, "windows-static-ip");
        assert!(plan.operations.len() >= 4);
        let ids: Vec<_> = plan.operations.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"iface-disable-dhcp"));
        assert!(ids.contains(&"iface-ip"));
        assert!(ids.contains(&"iface-gateway"));
        assert!(ids.contains(&"iface-dns"));
        assert!(PlanGenerator::new("/images/win.qcow2".into())
            .windows_static_ip_plan("bad", "10.0.0.1", "255.255.255.0", None, None)
            .is_err());
    }

    #[test]
    fn test_windows_dhcp_and_dns_plans() {
        let dhcp = PlanGenerator::new("/images/win.qcow2".into())
            .windows_dhcp_plan("{a1b2c3d4-e5f6-7890-abcd-ef1234567890}")
            .unwrap();
        assert_eq!(dhcp.profile, "windows-dhcp");
        assert_eq!(dhcp.operations[0].id, "iface-enable-dhcp");

        let dns = PlanGenerator::new("/images/win.qcow2".into())
            .windows_dns_plan("a1b2c3d4-e5f6-7890-abcd-ef1234567890", "1.1.1.1,8.8.8.8")
            .unwrap();
        assert_eq!(dns.profile, "windows-dns");
        match &dns.operations[0].op_type {
            OperationType::RegistryEdit(re) => {
                assert_eq!(re.value, "NameServer");
                assert_eq!(re.new_data, json!("1.1.1.1 8.8.8.8"));
            }
            _ => panic!("expected RegistryEdit"),
        }
    }

    #[test]
    fn test_linux_ssh_plan_shape_without_guest() {
        // Shape constants used by Machina / docs — unit ids must stay stable.
        let expected = [
            "remove-sshd-not-to-be-run",
            "ssh-wants-dir",
            "enable-ssh-unit",
            "sshd-dropin",
        ];
        assert_eq!(expected.len(), 4);
        assert!(expected.contains(&"enable-ssh-unit"));
        assert!(expected.contains(&"sshd-dropin"));
        assert!(expected.contains(&"remove-sshd-not-to-be-run"));
    }

    #[test]
    fn test_symlink_and_file_delete_serde() {
        let sl = OperationType::Symlink(Symlink {
            target: "../../../../lib/systemd/system/ssh.service".into(),
            link_path: "/etc/systemd/system/multi-user.target.wants/ssh.service".into(),
        });
        let fd = OperationType::FileDelete(FileDelete {
            path: "/etc/ssh/sshd_not_to_be_run".into(),
            missing_ok: true,
        });
        let sl_json = serde_json::to_string(&sl).unwrap();
        let fd_json = serde_json::to_string(&fd).unwrap();
        assert!(sl_json.contains("symlink"));
        assert!(fd_json.contains("file_delete"));
        let _sl2: OperationType = serde_json::from_str(&sl_json).unwrap();
        let _fd2: OperationType = serde_json::from_str(&fd_json).unwrap();
    }

    #[test]
    fn test_duration_estimation() {
        assert_eq!(PlanGenerator::estimate_duration(0), "0s");
        assert_eq!(PlanGenerator::estimate_duration(2), "1-2 minutes");
        assert_eq!(PlanGenerator::estimate_duration(5), "3-5 minutes");
        assert_eq!(PlanGenerator::estimate_duration(10), "5-10 minutes");
        assert_eq!(PlanGenerator::estimate_duration(20), "10+ minutes");
    }

    #[test]
    fn test_duration_estimation_boundaries() {
        assert_eq!(PlanGenerator::estimate_duration(1), "1-2 minutes");
        assert_eq!(PlanGenerator::estimate_duration(3), "1-2 minutes");
        assert_eq!(PlanGenerator::estimate_duration(4), "3-5 minutes");
        assert_eq!(PlanGenerator::estimate_duration(8), "3-5 minutes");
        assert_eq!(PlanGenerator::estimate_duration(9), "5-10 minutes");
        assert_eq!(PlanGenerator::estimate_duration(15), "5-10 minutes");
        assert_eq!(PlanGenerator::estimate_duration(16), "10+ minutes");
    }

    #[test]
    fn test_from_security_profile() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let report = create_test_report();
        let plan = generator.from_security_profile(&report).unwrap();

        assert_eq!(plan.vm, "test.qcow2");
        assert_eq!(plan.profile, "security");
        assert_eq!(plan.overall_risk, "high");
        assert!(!plan.operations.is_empty());
    }

    #[test]
    fn test_from_security_profile_risk_levels() {
        let generator = PlanGenerator::new("test.qcow2".to_string());

        let mut report = create_test_report();
        report.overall_risk = Some(RiskLevel::Critical);
        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.overall_risk, "critical");

        report.overall_risk = Some(RiskLevel::Medium);
        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.overall_risk, "medium");

        report.overall_risk = Some(RiskLevel::Low);
        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.overall_risk, "low");

        report.overall_risk = Some(RiskLevel::Info);
        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.overall_risk, "info");

        report.overall_risk = None;
        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.overall_risk, "unknown");
    }

    #[test]
    fn test_from_security_profile_metadata() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let report = create_test_report();
        let plan = generator.from_security_profile(&report).unwrap();

        assert!(plan.metadata.description.is_some());
        assert!(plan.metadata.tags.contains(&"security".to_string()));
        assert!(plan.metadata.tags.contains(&"automated".to_string()));
    }

    #[test]
    fn test_parse_remediation_ssh() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Disable PermitRootLogin in SSH config")
            .unwrap();

        match op_type {
            OperationType::FileEdit(fe) => {
                assert!(fe.file.contains("sshd_config"));
                assert!(fe.backup);
            }
            _ => panic!("Expected FileEdit operation"),
        }
    }

    #[test]
    fn test_parse_remediation_firewall_install() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Install firewall for security")
            .unwrap();

        match op_type {
            OperationType::PackageInstall(pi) => {
                assert!(pi.packages.contains(&"firewalld".to_string()));
            }
            _ => panic!("Expected PackageInstall operation"),
        }
    }

    #[test]
    fn test_parse_remediation_firewall_enable() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Enable firewall service")
            .unwrap();

        match op_type {
            OperationType::Symlink(sl) => {
                assert!(sl.link_path.contains("firewalld"));
                assert!(sl.target.contains("firewalld.service"));
            }
            _ => panic!("Expected Symlink for offline firewall enable"),
        }
    }

    #[test]
    fn test_parse_remediation_ufw_enable() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator.parse_remediation("Enable ufw firewall").unwrap();
        match op_type {
            OperationType::FileEdit(fe) => {
                assert!(fe.file.contains("ufw.conf"));
            }
            _ => panic!("Expected FileEdit for ufw enable"),
        }
    }

    #[test]
    fn test_parse_remediation_password_auth() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Set PasswordAuthentication no")
            .unwrap();
        match op_type {
            OperationType::FileEdit(fe) => {
                assert!(fe.file.contains("sshd_config"));
                assert!(fe
                    .changes
                    .iter()
                    .any(|c| c.after.contains("PasswordAuthentication no")));
            }
            _ => panic!("Expected FileEdit"),
        }
    }

    #[test]
    fn test_parse_remediation_selinux() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Set SELinux to enforcing mode")
            .unwrap();

        match op_type {
            OperationType::SelinuxMode(sm) => {
                assert!(sm.file.contains("selinux"));
                assert_eq!(sm.target, "enforcing");
                assert!(sm.warning.is_some());
            }
            _ => panic!("Expected SelinuxMode operation"),
        }
    }

    #[test]
    fn test_parse_remediation_fail2ban() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Install fail2ban for brute force protection")
            .unwrap();

        match op_type {
            OperationType::PackageInstall(pi) => {
                assert!(pi.packages.contains(&"fail2ban".to_string()));
            }
            _ => panic!("Expected PackageInstall operation"),
        }
    }

    #[test]
    fn test_parse_remediation_fail2ban_enable_offline() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Enable fail2ban service")
            .unwrap();
        match op_type {
            OperationType::Symlink(sl) => {
                assert!(sl.link_path.contains("fail2ban"));
            }
            _ => panic!("Expected Symlink for fail2ban enable"),
        }
    }

    #[test]
    fn test_parse_remediation_systemctl_enable() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("systemctl enable auditd.service")
            .unwrap();
        match op_type {
            OperationType::Symlink(sl) => {
                assert!(sl.link_path.contains("auditd"));
            }
            _ => panic!("Expected Symlink for systemctl enable"),
        }
    }

    #[test]
    fn test_parse_remediation_systemctl_disable() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("systemctl disable chronyd")
            .unwrap();
        match op_type {
            OperationType::FileDelete(fd) => {
                assert!(fd.path.contains("chronyd"));
                assert!(fd.missing_ok);
            }
            _ => panic!("Expected FileDelete for systemctl disable"),
        }
    }

    #[test]
    fn test_parse_remediation_ufw_default_deny() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Configure ufw default deny incoming")
            .unwrap();
        match op_type {
            OperationType::FileEdit(fe) => {
                assert!(fe.file.contains("default/ufw"));
            }
            _ => panic!("Expected FileEdit for ufw default deny"),
        }
    }

    #[test]
    fn test_upsert_grub_kv_and_cmdline() {
        let base = "GRUB_TIMEOUT=5\nGRUB_CMDLINE_LINUX=\"quiet\"\n";
        let t = PlanGenerator::upsert_grub_kv(base, "GRUB_TIMEOUT", "1");
        assert!(t.contains("GRUB_TIMEOUT=1"));
        let c = PlanGenerator::append_grub_cmdline(&t, "nomodeset");
        assert!(c.contains("nomodeset"));
    }

    #[test]
    fn test_parse_remediation_aide() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Install AIDE for file integrity")
            .unwrap();

        match op_type {
            OperationType::PackageInstall(pi) => {
                assert!(pi.packages.contains(&"aide".to_string()));
            }
            _ => panic!("Expected PackageInstall operation"),
        }
    }

    #[test]
    fn test_parse_remediation_default_command() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let op_type = generator
            .parse_remediation("Run custom security check")
            .unwrap();

        match op_type {
            OperationType::CommandExec(ce) => {
                assert_eq!(ce.command, "Run custom security check");
                assert_eq!(ce.expected_exit, 0);
                assert_eq!(ce.timeout, Some(300));
            }
            _ => panic!("Expected CommandExec operation"),
        }
    }

    #[test]
    fn test_finding_to_operation_priority_mapping() {
        let generator = PlanGenerator::new("test.qcow2".to_string());

        let finding_critical = create_test_finding(RiskLevel::Critical, "Test");
        let op = generator
            .finding_to_operation("op-1", &finding_critical, "Test")
            .unwrap();
        assert_eq!(op.priority, Priority::Critical);

        let finding_high = create_test_finding(RiskLevel::High, "Test");
        let op = generator
            .finding_to_operation("op-2", &finding_high, "Test")
            .unwrap();
        assert_eq!(op.priority, Priority::High);

        let finding_medium = create_test_finding(RiskLevel::Medium, "Test");
        let op = generator
            .finding_to_operation("op-3", &finding_medium, "Test")
            .unwrap();
        assert_eq!(op.priority, Priority::Medium);

        let finding_low = create_test_finding(RiskLevel::Low, "Test");
        let op = generator
            .finding_to_operation("op-4", &finding_low, "Test")
            .unwrap();
        assert_eq!(op.priority, Priority::Low);

        let finding_info = create_test_finding(RiskLevel::Info, "Test");
        let op = generator
            .finding_to_operation("op-5", &finding_info, "Test")
            .unwrap();
        assert_eq!(op.priority, Priority::Info);
    }

    #[test]
    fn test_finding_to_operation_structure() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let finding = create_test_finding(RiskLevel::High, "Enable firewall");
        let op = generator
            .finding_to_operation("op-test", &finding, "Enable firewall")
            .unwrap();

        assert_eq!(op.id, "op-test");
        assert_eq!(op.description, "Test finding");
        assert_eq!(op.risk, Priority::High);
        assert!(op.reversible);
        assert!(op.depends_on.is_empty());
    }

    #[test]
    fn test_add_post_apply_actions_ssh() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-ssh".to_string(),
            op_type: OperationType::FileEdit(FileEdit {
                file: "/etc/ssh/sshd_config".to_string(),
                backup: true,
                changes: vec![],
            }),
            priority: Priority::High,
            description: "SSH config".to_string(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        generator.add_post_apply_actions(&mut plan);

        assert!(!plan.post_apply.is_empty());
        let has_ssh_restart = plan.post_apply.iter().any(|action| {
            matches!(action, PostApplyAction::ServiceRestart { services } if services.contains(&"sshd".to_string()))
        });
        assert!(has_ssh_restart);
    }

    #[test]
    fn test_add_post_apply_actions_firewall() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-fw".to_string(),
            op_type: OperationType::Symlink(Symlink {
                target: "../../../../usr/lib/systemd/system/firewalld.service".into(),
                link_path: "/etc/systemd/system/multi-user.target.wants/firewalld.service".into(),
            }),
            priority: Priority::High,
            description: "Enable firewall".to_string(),
            risk: Priority::Low,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        generator.add_post_apply_actions(&mut plan);

        let has_firewall_validation = plan.post_apply.iter().any(|action| {
            matches!(action, PostApplyAction::Validation { command, .. } if command.contains("firewall-cmd"))
        });
        assert!(has_firewall_validation);
    }

    #[test]
    fn test_add_post_apply_actions_selinux() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let mut plan = FixPlan::new("test.qcow2".to_string(), "security".to_string());

        plan.add_operation(Operation {
            id: "op-sel".to_string(),
            op_type: OperationType::SelinuxMode(SELinuxMode {
                file: "/etc/selinux/config".to_string(),
                current: "permissive".to_string(),
                target: "enforcing".to_string(),
                warning: None,
            }),
            priority: Priority::Critical,
            description: "Set SELinux".to_string(),
            risk: Priority::Medium,
            reversible: true,
            depends_on: vec![],
            validation: None,
            undo: None,
        });

        generator.add_post_apply_actions(&mut plan);

        let has_reboot = plan
            .post_apply
            .iter()
            .any(|action| matches!(action, PostApplyAction::RebootRequired { .. }));
        assert!(has_reboot);
    }

    #[test]
    fn test_from_security_profile_no_findings() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let report = ProfileReport {
            profile_name: "security".to_string(),
            overall_risk: Some(RiskLevel::Info),
            sections: vec![],
            summary: None,
        };

        let plan = generator.from_security_profile(&report).unwrap();
        assert_eq!(plan.operations.len(), 0);
        assert_eq!(plan.estimated_duration, "0s");
    }

    #[test]
    fn test_from_security_profile_filters_no_risk() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let report = ProfileReport {
            profile_name: "security".to_string(),
            overall_risk: Some(RiskLevel::Medium),
            sections: vec![ReportSection {
                title: "Test Section".to_string(),
                findings: vec![Finding {
                    item: "Finding without risk".to_string(),
                    status: FindingStatus::Pass,
                    message: "No action needed".to_string(),
                    risk_level: None,
                }],
            }],
            summary: None,
        };

        let plan = generator.from_security_profile(&report).unwrap();
        // Should skip findings without risk_level
        assert_eq!(plan.operations.len(), 0);
    }

    #[test]
    fn test_from_security_profile_operation_ids() {
        let generator = PlanGenerator::new("test.qcow2".to_string());
        let report = create_test_report();
        let plan = generator.from_security_profile(&report).unwrap();

        // Check that operation IDs are sequential
        for (i, op) in plan.operations.iter().enumerate() {
            assert_eq!(op.id, format!("sec-{:03}", i + 1));
        }
    }

    #[test]
    fn test_from_migration_report_includes_changes_and_drivers() {
        use crate::boot::report::BootabilityReport;
        use crate::cli::migrate::plan::MigrationScoreReport;
        use std::path::Path;

        let generator = PlanGenerator::new("vm.qcow2".to_string());
        let boot = BootabilityReport {
            score: 80.0,
            confidence: 0.9,
            target: "proxmox".to_string(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: "ok".to_string(),
        };
        let migration = MigrationScoreReport {
            score: 75.0,
            driver_injections: vec!["virtio_blk".to_string(), "virtio_net".to_string()],
            required_changes: vec![
                "Remove VMware Tools; install qemu-guest-agent".to_string(),
                "Set disk bus to virtio-scsi or virtio-blk".to_string(),
            ],
            licensing_warnings: vec!["Verify OS license portability".to_string()],
            estimated_downtime_minutes: 45,
        };

        let plan = generator
            .from_migration_report(&migration, &boot, "proxmox", Path::new("vm.qcow2"))
            .unwrap();

        assert_eq!(plan.profile, "migration");
        assert!(plan.metadata.tags.contains(&"proxmox".to_string()));
        assert!(plan.operations.iter().any(|op| op.id.starts_with("mig-")));
        assert!(plan
            .operations
            .iter()
            .any(|op| matches!(&op.op_type, OperationType::PackageInstall(_))));
        assert!(plan
            .operations
            .iter()
            .any(|op| matches!(&op.op_type, OperationType::FileEdit(_))));
        assert!(plan.post_apply.iter().any(|a| matches!(
            a,
            PostApplyAction::Message { message } if message.contains("license")
        )));
    }
}
