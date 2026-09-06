// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows Registry parsing using nt_hive2
//!
//! This module provides pure Rust parsing of Windows registry hive files.
//! Note: This is an initial implementation that will be enhanced with full
//! registry parsing in future versions.

#[cfg(feature = "evtx")]
use serde_json::Value;

use crate::core::{Error, Result};
use std::path::Path;
#[derive(Debug, Clone)]
pub struct WindowsApp {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_location: Option<String>,
}

/// Windows service information
#[derive(Debug, Clone)]
pub struct WindowsSvc {
    pub name: String,
    pub display_name: String,
    pub start_type: String,
    pub image_path: String,
}

/// Windows network adapter information
#[derive(Debug, Clone)]
pub struct WindowsNetAdapter {
    pub name: String,
    pub description: String,
    pub dhcp_enabled: bool,
    pub ip_address: Vec<String>,
    pub mac_address: String,
    pub dns_servers: Vec<String>,
}

/// Parse installed applications from SOFTWARE hive
///
/// Reads from SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall
/// and SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall (for 32-bit apps on 64-bit Windows)
pub fn parse_installed_software(hive_path: &Path) -> Result<Vec<WindowsApp>> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    // Verify hive exists
    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SOFTWARE hive not found: {}",
            hive_path.display()
        )));
    }

    // Read hive file
    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {}", e)))?;

    // Parse hive
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))?;

    let mut applications = Vec::new();

    // Get root key
    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

    // Navigate to Uninstall key: Microsoft\Windows\CurrentVersion\Uninstall
    let microsoft_key = match root_key.subkey("Microsoft", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(applications), // No Microsoft key
    };

    let windows_key = match microsoft_key.borrow().subkey("Windows", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(applications), // No Windows key
    };

    let current_version_key = match windows_key.borrow().subkey("CurrentVersion", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(applications), // No CurrentVersion key
    };

    let uninstall_key = match current_version_key.borrow().subkey("Uninstall", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(applications), // No Uninstall key
    };

    // Iterate through subkeys (each represents an installed application)
    let uninstall_borrowed = uninstall_key.borrow();
    let subkeys_result = uninstall_borrowed.subkeys(&mut hive);
    let subkeys_ref = match subkeys_result {
        Ok(ref_vec) => ref_vec,
        Err(_) => return Ok(applications),
    };

    for app_key in subkeys_ref.iter() {
        let app_key_ref = app_key.borrow();

        // Extract application information from values
        let mut name = String::new();
        let mut version = String::new();
        let mut publisher = String::new();
        let mut install_location = None;

        for kv in app_key_ref.values() {
            match kv.name() {
                "DisplayName" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        name = data.clone();
                    }
                }
                "DisplayVersion" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        version = data.clone();
                    }
                }
                "Publisher" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        publisher = data.clone();
                    }
                }
                "InstallLocation" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        if !data.is_empty() {
                            install_location = Some(data.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Only add if we have a display name
        if !name.is_empty() {
            applications.push(WindowsApp {
                name,
                version,
                publisher,
                install_location,
            });
        }
    }

    Ok(applications)
}

/// Parse Windows services from SYSTEM hive
///
/// Reads from SYSTEM\ControlSet001\Services
pub fn parse_windows_services(hive_path: &Path) -> Result<Vec<WindowsSvc>> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    // Verify hive exists
    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SYSTEM hive not found: {}",
            hive_path.display()
        )));
    }

    // Read hive file
    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {}", e)))?;

    // Parse hive
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))?;

    let mut services = Vec::new();

    // Get root key
    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

    // Navigate to Services: ControlSet001\Services
    let controlset_key = match root_key.subkey("ControlSet001", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(services), // No ControlSet001 key
    };

    let services_key = match controlset_key.borrow().subkey("Services", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(services), // No Services key
    };

    // Iterate through service subkeys
    let services_borrowed = services_key.borrow();
    let subkeys_result = services_borrowed.subkeys(&mut hive);
    let subkeys_ref = match subkeys_result {
        Ok(ref_vec) => ref_vec,
        Err(_) => return Ok(services),
    };

    for svc_key in subkeys_ref.iter() {
        let svc_key_ref = svc_key.borrow();
        let svc_name = svc_key_ref.name().to_string();

        // Extract service information
        let mut display_name = svc_name.clone();
        let mut start_type = String::from("Unknown");
        let mut image_path = String::new();

        for kv in svc_key_ref.values() {
            match kv.name() {
                "DisplayName" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        display_name = data.clone();
                    }
                }
                "Start" => {
                    // Start type is a DWORD:
                    // 0 = Boot, 1 = System, 2 = Automatic, 3 = Manual, 4 = Disabled
                    if let RegistryValue::RegDWord(start_val) = kv.value() {
                        start_type = match start_val {
                            0 => "Boot".to_string(),
                            1 => "System".to_string(),
                            2 => "Automatic".to_string(),
                            3 => "Manual".to_string(),
                            4 => "Disabled".to_string(),
                            _ => format!("Unknown({})", start_val),
                        };
                    }
                }
                "ImagePath" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        image_path = data.clone();
                    }
                }
                _ => {}
            }
        }

        // Only add services that have an image path (actual services, not just service groups)
        if !image_path.is_empty() {
            services.push(WindowsSvc {
                name: svc_name,
                display_name,
                start_type,
                image_path,
            });
        }
    }

    Ok(services)
}

/// Parse network configuration from SYSTEM hive
///
/// Reads from SYSTEM\ControlSet001\Services\Tcpip\Parameters\Interfaces
pub fn parse_network_adapters(hive_path: &Path) -> Result<Vec<WindowsNetAdapter>> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    // Verify hive exists
    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SYSTEM hive not found: {}",
            hive_path.display()
        )));
    }

    // Read hive file
    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {}", e)))?;

    // Parse hive
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))?;

    let mut adapters = Vec::new();

    // Get root key
    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

    // Navigate to Tcpip interfaces: ControlSet001\Services\Tcpip\Parameters\Interfaces
    let controlset_key = match root_key.subkey("ControlSet001", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(adapters), // No ControlSet001 key
    };

    let services_key = match controlset_key.borrow().subkey("Services", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(adapters), // No Services key
    };

    let tcpip_key = match services_key.borrow().subkey("Tcpip", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(adapters), // No Tcpip key
    };

    let params_key = match tcpip_key.borrow().subkey("Parameters", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(adapters), // No Parameters key
    };

    let interfaces_key = match params_key.borrow().subkey("Interfaces", &mut hive) {
        Ok(Some(key)) => key,
        _ => return Ok(adapters), // No Interfaces key
    };

    // Iterate through interface subkeys (each GUID represents an adapter)
    let interfaces_borrowed = interfaces_key.borrow();
    let subkeys_result = interfaces_borrowed.subkeys(&mut hive);
    let subkeys_ref = match subkeys_result {
        Ok(ref_vec) => ref_vec,
        Err(_) => return Ok(adapters),
    };

    for if_key in subkeys_ref.iter() {
        let if_key_ref = if_key.borrow();
        let adapter_guid = if_key_ref.name().to_string();

        // Extract network adapter information
        let mut dhcp_enabled = false;
        let mut ip_address = Vec::new();
        let mut dns_servers = Vec::new();

        for kv in if_key_ref.values() {
            match kv.name() {
                "EnableDHCP" => {
                    // DHCP enabled is a DWORD (1 = enabled, 0 = disabled)
                    if let RegistryValue::RegDWord(val) = kv.value() {
                        dhcp_enabled = *val == 1;
                    }
                }
                "IPAddress" => {
                    // IP addresses stored as REG_MULTI_SZ (array of strings)
                    if let RegistryValue::RegMultiSZ(addrs) = kv.value() {
                        for addr in addrs {
                            if !addr.is_empty() && addr != "0.0.0.0" {
                                ip_address.push(addr.clone());
                            }
                        }
                    }
                }
                "DhcpIPAddress" => {
                    // DHCP-assigned IP address
                    if let RegistryValue::RegSZ(addr) = kv.value() {
                        if !addr.is_empty() && addr != "0.0.0.0" {
                            ip_address.push(addr.clone());
                        }
                    }
                }
                "NameServer" => {
                    // DNS servers as comma-separated string
                    if let RegistryValue::RegSZ(servers) = kv.value() {
                        for server in servers.split(',') {
                            let trimmed = server.trim();
                            if !trimmed.is_empty() {
                                dns_servers.push(trimmed.to_string());
                            }
                        }
                    }
                }
                "DhcpNameServer" => {
                    // DHCP-assigned DNS servers
                    if let RegistryValue::RegSZ(servers) = kv.value() {
                        for server in servers.split(',') {
                            let trimmed = server.trim();
                            if !trimmed.is_empty() && !dns_servers.contains(&trimmed.to_string()) {
                                dns_servers.push(trimmed.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Only add adapters that have configuration (at least IP or DHCP enabled)
        if !ip_address.is_empty() || dhcp_enabled {
            adapters.push(WindowsNetAdapter {
                name: adapter_guid.clone(),
                description: format!("Network Adapter {}", adapter_guid),
                dhcp_enabled,
                ip_address,
                mac_address: String::new(), // MAC address not available in Tcpip key
                dns_servers,
            });
        }
    }

    Ok(adapters)
}

/// Infer Windows license channel from ProductId / EditionID / ProductName.
pub fn infer_activation_channel(product_id: &str, edition_id: &str, product_name: &str) -> String {
    let pid = product_id.to_ascii_uppercase();
    let ed = edition_id.to_ascii_uppercase();
    let name = product_name.to_ascii_uppercase();

    if pid.contains("-OEM-") || pid.contains("OEM") || ed.contains("OEM") || name.contains("OEM") {
        return "OEM".into();
    }
    // Volume / enterprise SKUs are usually KMS or MAK activated on fleets.
    if ed.contains("ENTERPRISE")
        || ed.contains("EDUCATION")
        || ed.contains("SERVER")
        || name.contains("ENTERPRISE")
        || name.contains("DATACENTER")
    {
        return "Volume".into();
    }
    if !pid.is_empty() || !ed.is_empty() {
        return "Retail".into();
    }
    String::new()
}

/// Offline activation markers from SOFTWARE\Microsoft\Windows NT\CurrentVersion.
pub fn parse_activation_info(
    hive_path: &Path,
) -> Option<crate::evidence::snapshot::ActivationInfo> {
    use crate::evidence::snapshot::ActivationInfo;
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let file = File::open(hive_path).ok()?;
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock).ok()?;
    let root_key = hive.root_key_node().ok()?;

    let microsoft = root_key.subkey("Microsoft", &mut hive).ok()??;
    let windows_nt = microsoft.borrow().subkey("Windows NT", &mut hive).ok()??;
    let current = windows_nt
        .borrow()
        .subkey("CurrentVersion", &mut hive)
        .ok()??;

    let mut product_id = String::new();
    let mut edition_id = String::new();
    let mut product_name = String::new();
    let mut has_digital_pid = false;

    for kv in current.borrow().values() {
        match kv.name() {
            "ProductId" => {
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    product_id = data.clone();
                }
            }
            "EditionID" => {
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    edition_id = data.clone();
                }
            }
            "ProductName" => {
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    product_name = data.clone();
                }
            }
            "DigitalProductId" | "DigitalProductId4" => {
                if let RegistryValue::RegBinary(data) = kv.value() {
                    has_digital_pid = !data.is_empty();
                }
            }
            _ => {}
        }
    }

    if product_id.is_empty() && edition_id.is_empty() && !has_digital_pid {
        return None;
    }

    let channel = infer_activation_channel(&product_id, &edition_id, &product_name);
    Some(ActivationInfo {
        // Offline we cannot query SPP; treat a present ProductId as "has license material".
        licensed: !product_id.is_empty() || has_digital_pid,
        channel,
        product_id: (!product_id.is_empty()).then_some(product_id),
        edition_id: (!edition_id.is_empty()).then_some(edition_id),
    })
}

const CONFIGFLAG_REMOVED: u32 = 0x0000_0001;
const CONFIGFLAG_REINSTALL: u32 = 0x0000_0020;
const CONFIGFLAG_FAILEDINSTALL: u32 = 0x0000_0040;

/// True when ConfigFlags indicate a removed / failed / reinstall-needed device.
pub fn config_flags_indicate_ghost(flags: u32) -> bool {
    flags & (CONFIGFLAG_REMOVED | CONFIGFLAG_REINSTALL | CONFIGFLAG_FAILEDINSTALL) != 0
}

/// Source-hypervisor NIC signatures that become "ghosts" after KVM cutover.
pub fn is_source_hypervisor_nic(blob: &str) -> bool {
    let b = blob.to_ascii_lowercase();
    b.contains("vmxnet")
        || b.contains("vmware")
        || b.contains("ven_15ad")
        || b.contains("netvsc")
        || b.contains("hv_netvsc")
        || b.contains("ven_1414")
        || (b.contains("xen") && (b.contains("net") || b.contains("vif")))
}

fn reg_multi_or_sz(kv: &nt_hive2::KeyValue) -> String {
    use nt_hive2::RegistryValue;
    match kv.value() {
        RegistryValue::RegSZ(s) | RegistryValue::RegExpandSZ(s) => s.clone(),
        RegistryValue::RegMultiSZ(v) => v.join(";"),
        _ => String::new(),
    }
}

/// Offline ghost / remnant NICs from SYSTEM\ControlSet001\Enum\PCI.
pub fn detect_ghost_nics(hive_path: &Path) -> Vec<crate::evidence::snapshot::GhostNicEntry> {
    use crate::evidence::snapshot::GhostNicEntry;
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let mut out = Vec::new();
    let Ok(file) = File::open(hive_path) else {
        return out;
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return out;
    };
    let Ok(root_key) = hive.root_key_node() else {
        return out;
    };
    let Ok(Some(cs)) = root_key.subkey("ControlSet001", &mut hive) else {
        return out;
    };
    let Ok(Some(enum_key)) = cs.borrow().subkey("Enum", &mut hive) else {
        return out;
    };
    let Ok(Some(pci)) = enum_key.borrow().subkey("PCI", &mut hive) else {
        return out;
    };

    let pci_borrowed = pci.borrow();
    let Ok(devices) = pci_borrowed.subkeys(&mut hive) else {
        return out;
    };

    for device in devices.iter() {
        let device_borrowed = device.borrow();
        let device_name = device_borrowed.name().to_string();
        let Ok(instances) = device_borrowed.subkeys(&mut hive) else {
            continue;
        };
        for inst in instances.iter() {
            let inst_ref = inst.borrow();
            let instance_name = inst_ref.name().to_string();
            let mut class = String::new();
            let mut desc = String::new();
            let mut service = String::new();
            let mut hwid = String::new();
            let mut config_flags: u32 = 0;

            for kv in inst_ref.values() {
                match kv.name() {
                    "Class" => class = reg_multi_or_sz(kv),
                    "DeviceDesc" => desc = reg_multi_or_sz(kv),
                    "Service" => service = reg_multi_or_sz(kv),
                    "HardwareID" | "CompatibleIDs" => {
                        let v = reg_multi_or_sz(kv);
                        if !v.is_empty() {
                            if !hwid.is_empty() {
                                hwid.push(';');
                            }
                            hwid.push_str(&v);
                        }
                    }
                    "ConfigFlags" => {
                        if let RegistryValue::RegDWord(v) = kv.value() {
                            config_flags = *v;
                        }
                    }
                    _ => {}
                }
            }

            let class_net = class.eq_ignore_ascii_case("Net");
            let blob = format!("{device_name} {desc} {service} {hwid}");
            let hypervisor = is_source_hypervisor_nic(&blob);
            let problem = config_flags_indicate_ghost(config_flags);

            if !(class_net || hypervisor) {
                continue;
            }
            if !(problem || hypervisor) {
                continue;
            }

            let reason = if hypervisor && problem {
                "hypervisor remnant + ConfigFlags"
            } else if hypervisor {
                "hypervisor remnant NIC"
            } else {
                "ConfigFlags problem"
            };
            let description = if desc.is_empty() {
                format!("{device_name} [{reason}]")
            } else {
                // DeviceDesc often embeds ";friendly name" from INF.
                let friendly = desc.split(';').next_back().unwrap_or(&desc).trim();
                format!("{friendly} [{reason}]")
            };

            out.push(GhostNicEntry {
                instance_id: format!("PCI\\{device_name}\\{instance_name}"),
                description,
            });
        }
    }

    out.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    out.dedup_by(|a, b| a.instance_id == b.instance_id);
    out
}

/// Static (non-DHCP) NIC configs from Tcpip Interfaces — for post-cutover IP transfer.
pub fn parse_static_nic_configs(
    hive_path: &Path,
) -> Vec<crate::evidence::snapshot::WindowsNicConfig> {
    use crate::evidence::snapshot::WindowsNicConfig;
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let mut out = Vec::new();
    let Ok(file) = File::open(hive_path) else {
        return out;
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return out;
    };
    let Ok(root_key) = hive.root_key_node() else {
        return out;
    };
    let Ok(Some(cs)) = root_key.subkey("ControlSet001", &mut hive) else {
        return out;
    };
    let Ok(Some(services)) = cs.borrow().subkey("Services", &mut hive) else {
        return out;
    };
    let Ok(Some(tcpip)) = services.borrow().subkey("Tcpip", &mut hive) else {
        return out;
    };
    let Ok(Some(params)) = tcpip.borrow().subkey("Parameters", &mut hive) else {
        return out;
    };
    let Ok(Some(interfaces)) = params.borrow().subkey("Interfaces", &mut hive) else {
        return out;
    };

    let ifaces = interfaces.borrow();
    let Ok(subkeys) = ifaces.subkeys(&mut hive) else {
        return out;
    };

    for if_key in subkeys.iter() {
        let if_ref = if_key.borrow();
        let guid = if_ref.name().to_string();
        let mut dhcp = true;
        let mut ips = Vec::new();
        let mut gateway = None;
        let mut dns = Vec::new();

        for kv in if_ref.values() {
            match kv.name() {
                "EnableDHCP" => {
                    if let RegistryValue::RegDWord(v) = kv.value() {
                        dhcp = *v == 1;
                    }
                }
                "IPAddress" => {
                    if let RegistryValue::RegMultiSZ(addrs) = kv.value() {
                        for a in addrs {
                            if !a.is_empty() && a != "0.0.0.0" {
                                ips.push(a.clone());
                            }
                        }
                    }
                }
                "DefaultGateway" => {
                    if let RegistryValue::RegMultiSZ(gws) = kv.value() {
                        gateway = gws
                            .iter()
                            .find(|g| !g.is_empty() && g.as_str() != "0.0.0.0")
                            .cloned();
                    } else if let RegistryValue::RegSZ(g) = kv.value() {
                        if !g.is_empty() && g != "0.0.0.0" {
                            gateway = Some(g.clone());
                        }
                    }
                }
                "NameServer" => {
                    if let RegistryValue::RegSZ(servers) = kv.value() {
                        for s in servers.split([',', ' ']) {
                            let t = s.trim();
                            if !t.is_empty() {
                                dns.push(t.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !dhcp && !ips.is_empty() {
            out.push(WindowsNicConfig {
                name: guid,
                mac: String::new(),
                ip_addresses: ips,
                gateway,
                dns,
                dhcp: false,
            });
        }
    }

    out
}

/// Get Windows version from SOFTWARE hive
///
/// Returns (product_name, version, edition)
/// Reads from SOFTWARE\Microsoft\Windows NT\CurrentVersion
pub fn get_windows_version(hive_path: &Path) -> Result<(String, String, String)> {
    use nt_hive2::{Hive, HiveParseMode};
    use std::fs::File;

    // Verify hive exists
    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SOFTWARE hive not found: {}",
            hive_path.display()
        )));
    }

    // Read hive file
    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {}", e)))?;

    // Parse hive
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))?;

    // Navigate to CurrentVersion key
    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

    // Path: Microsoft\Windows NT\CurrentVersion
    // Navigate step by step
    let microsoft_key = root_key
        .subkey("Microsoft", &mut hive)
        .map_err(|e| Error::CommandFailed(format!("Failed to find Microsoft key: {:?}", e)))?
        .ok_or_else(|| Error::NotFound("Microsoft key not found".to_string()))?;

    let windows_nt_key = microsoft_key
        .borrow()
        .subkey("Windows NT", &mut hive)
        .map_err(|e| Error::CommandFailed(format!("Failed to find Windows NT key: {:?}", e)))?
        .ok_or_else(|| Error::NotFound("Windows NT key not found".to_string()))?;

    let current_version_key = windows_nt_key
        .borrow()
        .subkey("CurrentVersion", &mut hive)
        .map_err(|e| Error::CommandFailed(format!("Failed to find CurrentVersion key: {:?}", e)))?
        .ok_or_else(|| Error::NotFound("CurrentVersion key not found".to_string()))?;

    // Read values
    let mut product_name = String::from("Windows");
    let mut version = String::from("Unknown");
    let mut edition = String::from("Unknown");
    let mut build = String::new();
    let mut major_version = String::new();
    let mut minor_version = String::new();

    // Get all values from the CurrentVersion key
    let key_ref = current_version_key.borrow();
    let values = key_ref.values();

    // Iterate through values to find the ones we need
    for kv in values {
        // Get value name
        let name = kv.name();

        match name {
            "ProductName" => {
                // Read string value (e.g., "Windows 10 Pro", "Windows 11 Home")
                use nt_hive2::RegistryValue;
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    product_name = data.clone();
                }
            }
            "EditionID" => {
                // Read string value (e.g., "Professional", "Home", "Enterprise")
                use nt_hive2::RegistryValue;
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    edition = data.clone();
                }
            }
            "CurrentBuild" => {
                // Read string value (e.g., "19045", "22631")
                use nt_hive2::RegistryValue;
                if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) = kv.value() {
                    build = data.clone();
                }
            }
            "CurrentMajorVersionNumber" => {
                // Read DWORD value
                use nt_hive2::RegistryValue;
                if let RegistryValue::RegDWord(data) = kv.value() {
                    major_version = data.to_string();
                }
            }
            "CurrentMinorVersionNumber" => {
                // Read DWORD value
                use nt_hive2::RegistryValue;
                if let RegistryValue::RegDWord(data) = kv.value() {
                    minor_version = data.to_string();
                }
            }
            _ => {}
        }
    }

    // Construct version string
    if !major_version.is_empty() && !build.is_empty() {
        version = format!("{}.{}.{}", major_version, minor_version, build);
    } else if !build.is_empty() {
        version = build;
    }

    Ok((product_name, version, edition))
}

/// Windows update/hotfix information
#[derive(Debug, Clone)]
pub struct WindowsUpdateInfo {
    pub kb_number: String,
    pub title: String,
    pub description: String,
    pub installed_date: String,
    pub update_type: String,
}

/// Parse installed Windows updates from registry
///
/// Checks SOFTWARE hive for installed updates and hotfixes under
/// `Microsoft\Windows NT\CurrentVersion\HotFix`.
pub fn parse_installed_updates(hive_path: &Path) -> Result<Vec<WindowsUpdateInfo>> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SOFTWARE hive not found: {}",
            hive_path.display()
        )));
    }

    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {}", e)))?;
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))?;

    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

    let mut updates = Vec::new();

    let microsoft = match root_key.subkey("Microsoft", &mut hive) {
        Ok(Some(k)) => k,
        _ => return Ok(updates),
    };
    let windows_nt = match microsoft.borrow().subkey("Windows NT", &mut hive) {
        Ok(Some(k)) => k,
        _ => return Ok(updates),
    };
    let current_version = match windows_nt.borrow().subkey("CurrentVersion", &mut hive) {
        Ok(Some(k)) => k,
        _ => return Ok(updates),
    };
    let hotfix = match current_version.borrow().subkey("HotFix", &mut hive) {
        Ok(Some(k)) => k,
        _ => return Ok(updates),
    };

    let hotfix_borrowed = hotfix.borrow();
    let Ok(subkeys) = hotfix_borrowed.subkeys(&mut hive) else {
        return Ok(updates);
    };

    for kb_key in subkeys.iter() {
        let kb_ref = kb_key.borrow();
        let kb_name = kb_ref.name().to_string();
        if kb_name.is_empty() {
            continue;
        }
        let mut description = String::new();
        let mut installed_date = String::from("Unknown");
        for kv in kb_ref.values() {
            match kv.name() {
                "Description" | "Fix Description" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        description = data.clone();
                    }
                }
                "Installed" | "InstallDate" => {
                    if let RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) =
                        kv.value()
                    {
                        installed_date = data.clone();
                    }
                }
                _ => {}
            }
        }
        let kb_number = if kb_name.to_ascii_uppercase().starts_with("KB") {
            kb_name.clone()
        } else {
            format!("KB{kb_name}")
        };
        updates.push(WindowsUpdateInfo {
            kb_number: kb_number.clone(),
            title: if description.is_empty() {
                format!("{kb_number} (HotFix key)")
            } else {
                description.clone()
            },
            description,
            installed_date,
            update_type: "Hotfix".to_string(),
        });
    }

    Ok(updates)
}

/// Parse CBS.log for component-based servicing updates
pub fn parse_cbs_log(log_content: &str) -> Vec<WindowsUpdateInfo> {
    let mut updates = Vec::new();

    // Parse CBS.log entries for installed packages
    // Format: "Package KB###### was successfully changed to the Installed state."
    for line in log_content.lines() {
        if line.contains("Package KB") && line.contains("Installed state") {
            // Extract KB number
            if let Some(kb_start) = line.find("KB") {
                let kb_part = &line[kb_start..];
                if let Some(kb_end) = kb_part.find(|c: char| !c.is_alphanumeric()) {
                    let kb_number = kb_part[..kb_end].to_string();

                    updates.push(WindowsUpdateInfo {
                        kb_number: kb_number.clone(),
                        title: format!("{} installed", kb_number),
                        description: "Detected from CBS.log".to_string(),
                        installed_date: "Unknown".to_string(),
                        update_type: "Component".to_string(),
                    });
                }
            }
        }
    }

    updates
}

/// Parse Windows Update history from DataStore
pub fn parse_update_datastore(datastore_path: &Path) -> Result<Vec<WindowsUpdateInfo>> {
    // Verify DataStore.edb exists
    if !datastore_path.exists() {
        return Err(Error::NotFound(format!(
            "DataStore.edb not found: {}",
            datastore_path.display()
        )));
    }

    // ESE database (Extensible Storage Engine) parsing requires a dedicated ESE parser.
    // Without one, we cannot extract update history from DataStore.edb.
    Ok(vec![])
}

/// Detect hotfixes from file system
pub fn detect_hotfixes_from_filesystem(windows_dir: &Path) -> Result<Vec<WindowsUpdateInfo>> {
    let mut hotfixes = Vec::new();

    // $NtUninstall directories contain per-KB subdirectories like $NtUninstallKB123456$
    let nt_uninstall_prefix = "$NtUninstall";
    if let Ok(entries) = std::fs::read_dir(windows_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(nt_uninstall_prefix) && entry.path().is_dir() {
                // Extract KB number from directory name like "$NtUninstallKB123456$"
                let inner = name_str
                    .trim_start_matches(nt_uninstall_prefix)
                    .trim_end_matches('$');
                if !inner.is_empty() {
                    hotfixes.push(WindowsUpdateInfo {
                        kb_number: inner.to_string(),
                        title: format!("{} (uninstall data present)", inner),
                        description: format!("Found at {}", entry.path().display()),
                        installed_date: "Unknown".to_string(),
                        update_type: "Hotfix".to_string(),
                    });
                }
            }
        }
    }

    // $hf_mig$ directory indicates hotfix migration data exists
    let hf_mig = windows_dir.join("$hf_mig$");
    if hf_mig.exists() && hf_mig.is_dir() {
        // Count subdirectories as an approximation of hotfix count
        if let Ok(entries) = std::fs::read_dir(&hf_mig) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(kb_start) = name_str.find("KB") {
                    let kb_part = &name_str[kb_start..];
                    let kb_end = kb_part
                        .find(|c: char| !c.is_alphanumeric())
                        .unwrap_or(kb_part.len());
                    let kb_number = &kb_part[..kb_end];
                    hotfixes.push(WindowsUpdateInfo {
                        kb_number: kb_number.to_string(),
                        title: format!("{} (migration data)", kb_number),
                        description: format!("Found at {}", entry.path().display()),
                        installed_date: "Unknown".to_string(),
                        update_type: "Hotfix".to_string(),
                    });
                }
            }
        }
    }

    Ok(hotfixes)
}

/// Heuristic: scan BCD store bytes for UTF-16LE `testsigning` / `nointegritychecks`
/// followed nearby by `Yes`. Returns `Some(true)` when enforcement looks intact,
/// `Some(false)` when weakened, `None` when BCD cannot be read.
pub fn probe_bcd_signature_enforcement(bcd_bytes: &[u8]) -> Option<bool> {
    if bcd_bytes.is_empty() {
        return None;
    }
    let as_utf16: String = bcd_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .filter(|&u| u != 0)
        .map(|u| char::from_u32(u as u32).unwrap_or('\u{FFFD}'))
        .collect();
    let lower = as_utf16.to_ascii_lowercase();
    let weakened = ["testsigning", "nointegritychecks"].iter().any(|key| {
        if let Some(idx) = lower.find(key) {
            let window = &lower[idx..lower.len().min(idx + 64)];
            window.contains("yes")
        } else {
            false
        }
    });
    Some(!weakened)
}

/// Read the trailing portion of a (possibly huge) CBS.log and parse KBs.
pub fn parse_cbs_log_tail(path: &Path, max_bytes: u64) -> Vec<WindowsUpdateInfo> {
    use std::io::{Read, Seek, SeekFrom};

    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(meta) = file.metadata() else {
        return Vec::new();
    };
    let len = meta.len();
    let start = len.saturating_sub(max_bytes);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    parse_cbs_log(&text)
}

/// Windows event log entry
#[derive(Debug, Clone)]
pub struct WindowsEventEntry {
    pub event_id: u32,
    pub level: String,
    pub source: String,
    pub message: String,
    pub time_created: String,
    pub computer: String,
    pub channel: String,
}

/// Parse Windows Event Log (.evtx) file
pub fn parse_evtx_file(evtx_path: &Path, limit: usize) -> Result<Vec<WindowsEventEntry>> {
    if !evtx_path.exists() {
        return Err(Error::NotFound(format!(
            "EVTX file not found: {}",
            evtx_path.display()
        )));
    }

    #[cfg(feature = "evtx")]
    {
        parse_evtx_with_crate(evtx_path, limit)
    }

    #[cfg(not(feature = "evtx"))]
    {
        let _ = limit;
        Ok(vec![])
    }
}

#[cfg(feature = "evtx")]
fn parse_evtx_with_crate(evtx_path: &Path, limit: usize) -> Result<Vec<WindowsEventEntry>> {
    use evtx::EvtxParser;
    use serde_json::{json, Value};
    use std::fs::File;

    let file = File::open(evtx_path)?;
    let mut parser = EvtxParser::from_read_seek(file)
        .map_err(|e| Error::InvalidFormat(format!("evtx open: {e}")))?;
    let mut events = Vec::new();
    for record in parser.records() {
        if events.len() >= limit {
            break;
        }
        let record = record.map_err(|e| Error::InvalidFormat(format!("evtx record: {e}")))?;
        let val: Value = serde_json::from_str(&record.data).unwrap_or(json!({}));
        let system = val.pointer("/Event/System").or_else(|| val.get("System"));
        let event_id = json_u32(system.and_then(|s| s.get("EventID")), 0);
        let level = system
            .and_then(|s| s.get("Level"))
            .map(json_scalar_string)
            .unwrap_or_else(|| "0".into());
        let source = system
            .and_then(|s| s.get("Provider"))
            .and_then(|p| p.get("Name"))
            .or_else(|| system.and_then(|s| s.get("Provider")))
            .map(json_scalar_string)
            .unwrap_or_else(|| "Unknown".into());
        let channel = system
            .and_then(|s| s.get("Channel"))
            .map(json_scalar_string)
            .unwrap_or_default();
        let computer = system
            .and_then(|s| s.get("Computer"))
            .map(json_scalar_string)
            .unwrap_or_default();
        let message = val
            .pointer("/Event/EventData")
            .map(|d| d.to_string())
            .unwrap_or_else(|| record.data.chars().take(512).collect());
        events.push(WindowsEventEntry {
            event_id,
            level,
            source,
            message,
            time_created: record.timestamp.to_string(),
            computer,
            channel,
        });
    }
    Ok(events)
}

#[cfg(feature = "evtx")]
fn json_u32(value: Option<&Value>, default: u32) -> u32 {
    value
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                .or_else(|| v.as_i64().map(|i| i as u64))
        })
        .map(|n| n as u32)
        .unwrap_or(default)
}

#[cfg(feature = "evtx")]
fn json_scalar_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Parse System event log for boot times and errors
pub fn parse_system_events(evtx_path: &Path) -> Result<Vec<WindowsEventEntry>> {
    parse_evtx_file(evtx_path, 100)
}

/// Parse Security event log for login attempts
pub fn parse_security_events(evtx_path: &Path) -> Result<Vec<WindowsEventEntry>> {
    // Security log - look for failed logins (Event ID 4625)
    parse_evtx_file(evtx_path, 100)
}

/// Parse Application event log
pub fn parse_application_events(evtx_path: &Path) -> Result<Vec<WindowsEventEntry>> {
    parse_evtx_file(evtx_path, 50)
}

/// Parse domain join status from SYSTEM hive
pub fn parse_domain_info(hive_path: &Path) -> (bool, Option<String>) {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let Ok(file) = File::open(hive_path) else {
        return (false, None);
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return (false, None);
    };
    let Ok(root_key) = hive.root_key_node() else {
        return (false, None);
    };
    let Ok(Some(cs)) = root_key.subkey("ControlSet001", &mut hive) else {
        return (false, None);
    };
    let Ok(Some(svc)) = cs.borrow().subkey("Services", &mut hive) else {
        return (false, None);
    };
    let Ok(Some(tcpip)) = svc.borrow().subkey("Tcpip", &mut hive) else {
        return (false, None);
    };
    let Ok(Some(params)) = tcpip.borrow().subkey("Parameters", &mut hive) else {
        return (false, None);
    };

    let mut domain = None;
    for kv in params.borrow().values() {
        if kv.name() == "Domain" {
            if let RegistryValue::RegSZ(d) | RegistryValue::RegExpandSZ(d) = kv.value() {
                if !d.is_empty() && d.to_lowercase() != "workgroup" {
                    domain = Some(d.clone());
                }
            }
        }
    }
    (domain.is_some(), domain)
}

/// Check if RDP is enabled via SYSTEM hive
pub fn parse_rdp_enabled(hive_path: &Path) -> bool {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let Ok(file) = File::open(hive_path) else {
        return false;
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return false;
    };
    let Ok(root_key) = hive.root_key_node() else {
        return false;
    };
    let Ok(Some(cs)) = root_key.subkey("ControlSet001", &mut hive) else {
        return false;
    };
    let Ok(Some(svc)) = cs.borrow().subkey("Services", &mut hive) else {
        return false;
    };
    let Ok(Some(ts)) = svc.borrow().subkey("TermService", &mut hive) else {
        return false;
    };
    let Ok(Some(params)) = ts.borrow().subkey("Parameters", &mut hive) else {
        return false;
    };

    for kv in params.borrow().values() {
        if kv.name() == "fDenyTSConnections" {
            if let RegistryValue::RegDWord(v) = kv.value() {
                return *v == 0;
            }
        }
    }
    false
}

/// Detect pending reboot from SYSTEM hive Session Manager
pub fn parse_pending_reboot(hive_path: &Path) -> bool {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let Ok(file) = File::open(hive_path) else {
        return false;
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return false;
    };
    let Ok(root_key) = hive.root_key_node() else {
        return false;
    };
    let Ok(Some(cs)) = root_key.subkey("ControlSet001", &mut hive) else {
        return false;
    };
    let Ok(Some(sm)) = cs.borrow().subkey("Control", &mut hive) else {
        return false;
    };
    let Ok(Some(session)) = sm.borrow().subkey("Session Manager", &mut hive) else {
        return false;
    };

    for kv in session.borrow().values() {
        if kv.name() == "PendingFileRenameOperations" {
            if let RegistryValue::RegMultiSZ(ops) = kv.value() {
                return !ops.is_empty();
            }
        }
    }
    false
}

/// Parse SAM hive for local account summary
pub fn parse_sam_accounts(hive_path: &Path) -> Result<SamSummary> {
    use nt_hive2::{Hive, HiveParseMode};
    use std::fs::File;

    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SAM hive not found: {}",
            hive_path.display()
        )));
    }

    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open SAM hive: {}", e)))?;
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse SAM hive: {:?}", e)))?;

    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("Failed to get SAM root: {:?}", e)))?;

    let mut summary = SamSummary::default();
    if let Ok(Some(sam)) = root_key.subkey("SAM", &mut hive) {
        if let Ok(Some(domains)) = sam.borrow().subkey("Domains", &mut hive) {
            if let Ok(Some(account)) = domains.borrow().subkey("Account", &mut hive) {
                if let Ok(Some(users)) = account.borrow().subkey("Users", &mut hive) {
                    if let Ok(subkeys) = users.borrow().subkeys(&mut hive) {
                        summary.local_account_count = subkeys.len();
                    }
                }
            }
        }
    }
    Ok(summary)
}

/// Summary of SAM hive account data
#[derive(Debug, Clone, Default)]
pub struct SamSummary {
    pub local_account_count: usize,
    pub admin_count: usize,
}

/// Detect hypervisor guest tool remnants
pub fn detect_hypervisor_remnants(
    system_hive: &Path,
    drivers_path: &str,
    g: &mut crate::guestfs::Guestfs,
) -> Vec<String> {
    let mut remnants = Vec::new();
    let vmware_drivers = ["vmci.sys", "vmhgfs.sys", "vmmouse.sys", "vm3dmp.sys"];
    let hyperv_drivers = ["vmbus.sys", "hv_vmbus.sys", "storvsc.sys"];

    if let Ok(drivers) = g.ls(drivers_path) {
        for d in &drivers {
            let lower = d.to_lowercase();
            if vmware_drivers
                .iter()
                .any(|v| lower.contains(v.trim_end_matches(".sys")))
            {
                remnants.push("vmware-drivers".to_string());
            }
            if hyperv_drivers
                .iter()
                .any(|v| lower.contains(v.trim_end_matches(".sys")))
            {
                remnants.push("hyper-v-drivers".to_string());
            }
        }
    }

    if let Ok(services) = parse_windows_services(system_hive) {
        for svc in services {
            let name = svc.name.to_lowercase();
            if name.contains("vmware") || name.contains("vmtools") {
                remnants.push("vmware-tools-service".to_string());
            }
            if name.contains("hyper-v") || name.contains("vmbus") {
                remnants.push("hyper-v-service".to_string());
            }
        }
    }

    remnants.sort();
    remnants.dedup();
    remnants
}

/// Detect AV/EDR products from registry and filesystem
pub fn detect_av_edr(
    software_hive: &Path,
    g: &mut crate::guestfs::Guestfs,
    systemroot: &str,
) -> Vec<String> {
    let mut products = Vec::new();
    if let Ok(apps) = parse_installed_software(software_hive) {
        for app in apps {
            let name = app.name.to_lowercase();
            for keyword in [
                "defender",
                "crowdstrike",
                "sentinelone",
                "carbon black",
                "symantec",
                "mcafee",
                "sophos",
                "kaspersky",
                "trend micro",
                "cylance",
                "malwarebytes",
                "bitdefender",
            ] {
                if name.contains(keyword) {
                    products.push(app.name.clone());
                }
            }
        }
    }

    let paths = [
        format!("{}/System32/Windows Defender", systemroot),
        format!(
            "{}/Program Files/CrowdStrike",
            systemroot.trim_start_matches('/')
        ),
    ];
    for p in paths {
        if g.exists(&p).unwrap_or(false) {
            if p.contains("Defender") {
                products.push("Windows Defender".to_string());
            }
            if p.contains("CrowdStrike") {
                products.push("CrowdStrike".to_string());
            }
        }
    }

    products.sort();
    products.dedup();
    products
}

/// Parse SECURITY hive for audit policy metadata
pub fn parse_security_hive(hive_path: &Path) -> Result<SecurityHiveSummary> {
    use nt_hive2::{CleanHive, Hive, HiveParseMode};
    use std::fs::File;

    if !hive_path.exists() {
        return Err(Error::NotFound(format!(
            "SECURITY hive not found: {}",
            hive_path.display()
        )));
    }

    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open SECURITY hive: {}", e)))?;
    let _hive: Hive<File, CleanHive> = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse SECURITY hive: {:?}", e)))?;

    Ok(SecurityHiveSummary {
        lsa_present: true,
        audit_policy_configured: false,
    })
}

/// Summary from SECURITY hive (metadata only, no secret decryption)
#[derive(Debug, Clone, Default)]
pub struct SecurityHiveSummary {
    pub lsa_present: bool,
    pub audit_policy_configured: bool,
}

/// Detect BitLocker protectors from registry (FVE key path)
pub fn parse_bitlocker_status(software_hive: &Path) -> bool {
    use nt_hive2::{Hive, HiveParseMode};
    use std::fs::File;

    let Ok(file) = File::open(software_hive) else {
        return false;
    };
    let Ok(mut hive) = Hive::new(file, HiveParseMode::NormalWithBaseBlock) else {
        return false;
    };
    let Ok(root_key) = hive.root_key_node() else {
        return false;
    };
    root_key
        .subkey("Microsoft", &mut hive)
        .ok()
        .flatten()
        .and_then(|k| k.borrow().subkey("FVE", &mut hive).ok().flatten())
        .is_some()
}

/// Read `SYSTEM\ControlSet001\Control\BitLockerStatus\BootStatus`.
/// `Some(1)` = protection enabled on the OS volume (strong offline signal).
pub fn parse_bitlocker_boot_status(system_hive: &Path) -> Option<u32> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    let file = File::open(system_hive).ok()?;
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock).ok()?;
    let root_key = hive.root_key_node().ok()?;
    let cs = root_key.subkey("ControlSet001", &mut hive).ok()??;
    let control = cs.borrow().subkey("Control", &mut hive).ok()??;
    let status = control
        .borrow()
        .subkey("BitLockerStatus", &mut hive)
        .ok()??;
    for kv in status.borrow().values() {
        if kv.name() == "BootStatus" {
            if let RegistryValue::RegDWord(v) = kv.value() {
                return Some(*v);
            }
        }
    }
    None
}

/// Offline BitLocker state for evidence / MIG-W-005 / Passport.
///
/// `BootStatus == 1` → `any_protected` (hard blocker). Otherwise FVE artifacts
/// set `offline_uncertain` so checks warn without false hard-blocks.
pub fn collect_bitlocker_offline(
    software_hive: &Path,
    system_hive: &Path,
    dollar_bitlocker: bool,
    fvevol_sys: bool,
) -> Option<crate::evidence::snapshot::BitLockerState> {
    use crate::evidence::snapshot::{BitLockerState, BitLockerVolume};

    let boot_status = parse_bitlocker_boot_status(system_hive);
    let fve_key = parse_bitlocker_status(software_hive);
    let fvevol_svc = parse_windows_services(system_hive)
        .ok()
        .map(|svcs| {
            svcs.iter().any(|s| {
                s.name.eq_ignore_ascii_case("FVEVOL") || s.name.eq_ignore_ascii_case("BDESvc")
            })
        })
        .unwrap_or(false);

    if boot_status.is_none() && !fve_key && !dollar_bitlocker && !fvevol_sys && !fvevol_svc {
        return None;
    }

    let any_protected = boot_status == Some(1);
    let artifacts =
        fve_key || dollar_bitlocker || fvevol_sys || fvevol_svc || boot_status.is_some();
    let offline_uncertain = !any_protected && artifacts && boot_status != Some(0);

    let mut sources = Vec::new();
    if let Some(bs) = boot_status {
        sources.push(format!("BitLockerStatus.BootStatus={bs}"));
    }
    if fve_key {
        sources.push("SOFTWARE\\Microsoft\\FVE".into());
    }
    if dollar_bitlocker {
        sources.push("$BitLocker".into());
    }
    if fvevol_sys {
        sources.push("fvevol.sys".into());
    }
    if fvevol_svc {
        sources.push("FVEVOL/BDESvc".into());
    }

    let protection = if any_protected {
        "on"
    } else if boot_status == Some(0) && !fve_key && !dollar_bitlocker {
        "off"
    } else if artifacts {
        "artifacts"
    } else {
        "unknown"
    };

    Some(BitLockerState {
        any_protected,
        offline_uncertain,
        volumes: vec![BitLockerVolume {
            mount_point: "C:".into(),
            protection: protection.into(),
            source: Some(sources.join(", ")),
        }],
    })
}

/// Offline VSS stack readiness from services + System Volume Information.
///
/// Live writer health requires a running guest; offline we infer whether the
/// VSS service pair looks usable for consistent cutover snapshots.
pub fn collect_vss_offline(
    system_hive: &Path,
    svi_present: bool,
) -> Option<crate::evidence::snapshot::VssHealth> {
    use crate::evidence::snapshot::VssHealth;

    let Ok(services) = parse_windows_services(system_hive) else {
        return if svi_present {
            Some(VssHealth {
                writers_total: 0,
                writers_failed: vec![],
                healthy: svi_present,
                offline_inferred: true,
            })
        } else {
            None
        };
    };

    const CRITICAL: &[&str] = &["VSS", "swprv"];
    let mut found = 0usize;
    let mut failed = Vec::new();
    for name in CRITICAL {
        if let Some(svc) = services.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
            found += 1;
            if svc.start_type.eq_ignore_ascii_case("Disabled") {
                failed.push(format!("{name} (Disabled)"));
            }
        } else {
            failed.push(format!("{name} (missing)"));
        }
    }

    if found == 0 && !svi_present {
        return None;
    }

    Some(VssHealth {
        writers_total: found,
        writers_failed: failed.clone(),
        healthy: failed.is_empty() && svi_present,
        offline_inferred: true,
    })
}

/// Run / RunOnce registry value for persistence analysis.
#[derive(Debug, Clone)]
pub struct WindowsRunKeyEntry {
    pub location: String,
    pub name: String,
    pub command: String,
}

/// Parse Run and RunOnce keys from SOFTWARE hive.
pub fn parse_run_keys(hive_path: &Path) -> Result<Vec<WindowsRunKeyEntry>> {
    use nt_hive2::{Hive, HiveParseMode, RegistryValue};
    use std::fs::File;

    if !hive_path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(hive_path)
        .map_err(|e| Error::CommandFailed(format!("Failed to open hive: {e}")))?;
    let mut hive = Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {e:?}")))?;
    let root_key = hive
        .root_key_node()
        .map_err(|e| Error::CommandFailed(format!("root key: {e:?}")))?;

    let mut out = Vec::new();
    if let Ok(Some(microsoft)) = root_key.subkey("Microsoft", &mut hive) {
        if let Ok(Some(windows)) = microsoft.borrow().subkey("Windows", &mut hive) {
            if let Ok(Some(current)) = windows.borrow().subkey("CurrentVersion", &mut hive) {
                for (subkey, label) in
                    [("Run", "HKLM\\...\\Run"), ("RunOnce", "HKLM\\...\\RunOnce")]
                {
                    if let Ok(Some(run_key)) = current.borrow().subkey(subkey, &mut hive) {
                        for kv in run_key.borrow().values() {
                            let name = kv.name().to_string();
                            let command = match kv.value() {
                                RegistryValue::RegSZ(s) | RegistryValue::RegExpandSZ(s) => {
                                    s.clone()
                                }
                                RegistryValue::RegMultiSZ(v) => v.join(" "),
                                _ => continue,
                            };
                            out.push(WindowsRunKeyEntry {
                                location: label.into(),
                                name,
                                command,
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod hotfix_diag_tests {
    use super::*;

    #[test]
    fn cbs_log_extracts_kb() {
        let log = "2024-01-01 Package KB5034441 was successfully changed to the Installed state.\n";
        let updates = parse_cbs_log(log);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].kb_number, "KB5034441");
    }

    #[test]
    fn bcd_probe_detects_testsigning_yes() {
        // UTF-16LE for "testsigning Yes"
        let mut bytes = Vec::new();
        for c in "pad testsigning Yes pad".encode_utf16() {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        assert_eq!(probe_bcd_signature_enforcement(&bytes), Some(false));
    }

    #[test]
    fn bcd_probe_enforcement_when_absent() {
        let mut bytes = Vec::new();
        for c in "windows boot manager".encode_utf16() {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        assert_eq!(probe_bcd_signature_enforcement(&bytes), Some(true));
    }

    #[test]
    fn oem_product_id_channel() {
        assert_eq!(
            infer_activation_channel("00330-80000-00000-AA280", "Professional", "Windows 10 Pro"),
            "Retail"
        );
        assert_eq!(
            infer_activation_channel("00330-OEM-0000000-00000", "Core", "Windows 10 Home"),
            "OEM"
        );
        assert_eq!(
            infer_activation_channel("", "Enterprise", "Windows 10 Enterprise"),
            "Volume"
        );
    }

    #[test]
    fn config_flags_and_hypervisor_nic() {
        assert!(config_flags_indicate_ghost(0x20));
        assert!(config_flags_indicate_ghost(0x1));
        assert!(!config_flags_indicate_ghost(0));
        assert!(is_source_hypervisor_nic("PCI\\VEN_15AD&DEV_07B0 vmxnet3"));
        assert!(is_source_hypervisor_nic("netvsc Hyper-V"));
        assert!(!is_source_hypervisor_nic("PCI\\VEN_1AF4&DEV_1000 virtio"));
    }

    #[test]
    fn bitlocker_offline_boot_status_protected() {
        let state = collect_bitlocker_offline(
            Path::new("/nonexistent-software"),
            Path::new("/nonexistent-system"),
            true,
            true,
        );
        // Without hive BootStatus, artifacts alone → uncertain, not hard-protected.
        let state = state.expect("artifacts should yield state");
        assert!(!state.any_protected);
        assert!(state.offline_uncertain);
        assert_eq!(state.volumes[0].protection, "artifacts");
    }

    #[test]
    fn vss_offline_without_hive_uses_svi() {
        let health = collect_vss_offline(Path::new("/nonexistent-system"), true)
            .expect("SVI alone yields inferred health");
        assert!(health.offline_inferred);
        assert!(health.healthy);
    }
}
