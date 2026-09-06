// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit inspect handler - VM disk inspection

use async_trait::async_trait;
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{OperationHandler, HandlerContext, HandlerResult};

/// Inspect operation payload
#[derive(Debug, Clone, Deserialize, Serialize)]
struct InspectPayload {
    image: ImageSpec,
    #[serde(default)]
    options: InspectOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<OutputSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ImageSpec {
    path: String,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    checksum: Option<String>,
    #[serde(default = "default_true")]
    read_only: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct InspectOptions {
    #[serde(default)]
    deep_scan: bool,
    #[serde(default = "default_true")]
    include_packages: bool,
    #[serde(default = "default_true")]
    include_services: bool,
    #[serde(default = "default_true")]
    include_users: bool,
    #[serde(default = "default_true")]
    include_network: bool,
    #[serde(default = "default_true")]
    include_security: bool,
    #[serde(default = "default_true")]
    include_storage: bool,
    #[serde(default)]
    include_databases: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OutputSpec {
    format: String,
    destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compression: Option<String>,
}

/// Trim heavy arrays for API/UI while keeping full data in output_file.
fn summarize_inspection(full: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::json!({
        "source": "guestkit.inspect",
    });
    if let Some(os) = full.get("operating_system") {
        out["operating_system"] = os.clone();
    }
    if let Some(packages) = full.get("packages") {
        let sample: Vec<serde_json::Value> = packages
            .get("packages")
            .and_then(|p| p.as_array())
            .map(|arr| arr.iter().take(30).cloned().collect())
            .unwrap_or_default();
        out["packages"] = serde_json::json!({
            "count": packages.get("count").cloned().unwrap_or(serde_json::json!(0)),
            "manager": packages.get("manager").cloned().unwrap_or(serde_json::Value::Null),
            "sample": sample,
        });
    }
    if let Some(services) = full.get("services") {
        let sample: Vec<serde_json::Value> = services
            .get("enabled_services")
            .and_then(|p| p.as_array())
            .map(|arr| arr.iter().take(20).cloned().collect())
            .unwrap_or_default();
        out["services"] = serde_json::json!({
            "count": services.get("count").cloned().unwrap_or(serde_json::json!(0)),
            "sample": sample,
        });
    }
    if let Some(network) = full.get("network") {
        out["network"] = network.clone();
    }
    if let Some(security) = full.get("security") {
        out["security"] = security.clone();
    }
    if let Some(mounts) = full.get("mountpoints").and_then(|m| m.as_array()) {
        out["mountpoints"] = serde_json::json!({
            "count": mounts.len(),
        });
    }
    // Deep-inspection panels — forwarded for the TUI-style report.
    if let Some(storage) = full.get("storage") {
        out["storage"] = storage.clone();
    }
    if let Some(kernels) = full.get("kernels") {
        out["kernels"] = kernels.clone();
    }
    if let Some(modules) = full.get("kernel_modules") {
        out["kernel_modules"] = modules.clone();
    }
    if let Some(units) = full.get("systemd_units") {
        let sample: Vec<serde_json::Value> = units
            .get("units")
            .and_then(|u| u.as_array())
            .map(|arr| arr.iter().take(60).cloned().collect())
            .unwrap_or_default();
        out["systemd_units"] = serde_json::json!({
            "count": units.get("count").cloned().unwrap_or(serde_json::json!(0)),
            "sample": sample,
        });
    }
    if let Some(users) = full.get("users") {
        out["users"] = users.clone();
    }
    for key in ["machine_id", "cloud_init", "vm_tools", "firewall", "ssh"] {
        if let Some(v) = full.get(key) {
            out[key] = v.clone();
        }
    }
    out
}

/// Guestkit inspect handler
pub struct InspectHandler {
    /// Temporary directory for operations
    temp_dir: PathBuf,
}

impl InspectHandler {
    /// Create a new inspect handler
    pub fn new() -> Self {
        Self {
            temp_dir: std::env::temp_dir().join("guestkit-inspect"),
        }
    }

    /// Verify image checksum if provided
    /// Supports format: "sha256:hexhash" or just "hexhash" (defaults to SHA256)
    async fn verify_checksum(&self, path: &str, expected: &str) -> WorkerResult<bool> {
        use sha2::{Sha256, Digest};
        use std::io::Read;

        log::info!("Verifying checksum for image: {}", path);

        // Parse checksum format
        let (algorithm, expected_hash) = if expected.contains(':') {
            let parts: Vec<&str> = expected.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(WorkerError::ExecutionError(
                    format!("Invalid checksum format: {}", expected)
                ));
            }
            (parts[0].to_lowercase(), parts[1].to_lowercase())
        } else {
            // Default to SHA256 if no algorithm specified
            ("sha256".to_string(), expected.to_lowercase())
        };

        // Only SHA256 is supported for now
        if algorithm != "sha256" {
            return Err(WorkerError::ExecutionError(
                format!("Unsupported checksum algorithm: {}. Only 'sha256' is supported.", algorithm)
            ));
        }

        // Open file and compute SHA256
        let mut file = std::fs::File::open(path)
            .map_err(|e| WorkerError::ExecutionError(
                format!("Failed to open image for checksum verification: {}", e)
            ))?;

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192]; // 8KB buffer for reading

        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| WorkerError::ExecutionError(
                    format!("Failed to read image during checksum verification: {}", e)
                ))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        let computed_hash = format!("{:x}", hasher.finalize());

        log::debug!("Checksum verification - Expected: {}, Computed: {}", expected_hash, computed_hash);

        if computed_hash != expected_hash {
            log::error!("Checksum mismatch! Expected: {}, Got: {}", expected_hash, computed_hash);
            return Ok(false);
        }

        log::info!("Checksum verification successful");
        Ok(true)
    }

    /// Perform VM disk inspection
    async fn inspect_vm(
        &self,
        context: &HandlerContext,
        payload: &InspectPayload,
    ) -> WorkerResult<serde_json::Value> {
        context.report_progress("validation", Some(5), "Validating image").await?;

        // Verify image exists
        let image_path = std::path::Path::new(&payload.image.path);
        if !image_path.exists() {
            return Err(WorkerError::ExecutionError(
                format!("Image not found: {}", payload.image.path)
            ));
        }

        // Verify checksum if provided
        if let Some(ref checksum) = payload.image.checksum {
            context.report_progress("validation", Some(10), "Verifying image checksum").await?;
            if !self.verify_checksum(&payload.image.path, checksum).await? {
                context.record_checksum_verification("failure");
                return Err(WorkerError::ExecutionError(
                    format!("Image checksum verification failed for {}. The image may be corrupted or tampered with.", payload.image.path)
                ));
            }
            context.record_checksum_verification("success");
        } else {
            context.record_checksum_verification("skipped");
        }

        context.report_progress("inspection", Some(20), "Starting VM inspection").await?;

        // Perform real inspection using guestkit library
        let inspection_result = self.real_inspection(&payload).await?;

        context.report_progress("analysis", Some(80), "Analyzing results").await?;

        // Generate output
        let output_path = if let Some(ref output) = payload.output {
            context.report_progress("export", Some(90), "Writing output file").await?;
            self.write_output(&inspection_result, output).await?
        } else {
            tokio::fs::create_dir_all(&context.work_dir).await?;
            let temp_file = context.work_dir.join(format!("{}-result.json", context.job_id));
            tokio::fs::write(
                &temp_file,
                serde_json::to_string_pretty(&inspection_result)?
            ).await?;
            temp_file.to_string_lossy().to_string()
        };

        context.report_progress("complete", Some(100), "Inspection complete").await?;

        Ok(serde_json::json!({
            "status": "success",
            "output_file": output_path,
            "summary": {
                "image": payload.image.path,
                "format": payload.image.format,
                "inspection_time": chrono::Utc::now().to_rfc3339(),
            },
            "inspect": summarize_inspection(&inspection_result),
        }))
    }

    /// Real inspection using guestkit library
    async fn real_inspection(&self, payload: &InspectPayload) -> WorkerResult<serde_json::Value> {
        // Run blocking guestkit operations in a separate thread
        let payload_clone = payload.clone();

        tokio::task::spawn_blocking(move || -> WorkerResult<serde_json::Value> {
            use guestkit::Guestfs;

            // Create guestfs handle
            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs handle: {}", e)))?;

            // Add drive in read-only mode
            g.add_drive_ro(&payload_clone.image.path)
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {}", e)))?;

            // Launch the VM
            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {}", e)))?;

            // Inspect the OS
            let inspected_oses = g.inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect OS: {}", e)))?;

            if inspected_oses.is_empty() {
                return Err(WorkerError::ExecutionError("No operating system found in image".to_string()));
            }

            // Use the first OS found
            let os_info = &inspected_oses[0];

            // Build result structure
            let mut result = serde_json::json!({
                "version": "1.0",
                "image": {
                    "path": payload_clone.image.path,
                    "format": payload_clone.image.format,
                },
                "operating_system": {
                    "type": os_info.os_type,
                    "distribution": os_info.distro,
                    "product_name": os_info.product_name,
                    "version": format!("{}.{}", os_info.major_version, os_info.minor_version),
                    "major_version": os_info.major_version,
                    "minor_version": os_info.minor_version,
                    "hostname": os_info.hostname,
                    "arch": os_info.arch,
                    "package_format": os_info.package_format,
                },
                "mountpoints": os_info.mountpoints,
            });

            // Mount the root filesystem
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount root: {}", e)))?;

            // Collect packages if requested
            if payload_clone.options.include_packages {
                let packages = match os_info.package_format.as_str() {
                    "deb" => g.dpkg_list().ok(),
                    "rpm" => g.rpm_list().ok(),
                    _ => None,
                };

                if let Some(pkg_list) = packages {
                    result["packages"] = serde_json::json!({
                        "count": pkg_list.len(),
                        "manager": os_info.package_format,
                        "packages": pkg_list,
                    });
                }
            }

            // Collect services if requested
            if payload_clone.options.include_services {
                if let Ok(services) = g.list_enabled_services() {
                    result["services"] = serde_json::json!({
                        "count": services.len(),
                        "enabled_services": services,
                    });
                }
            }

            // Collect network interfaces if requested
            if payload_clone.options.include_network {
                if let Ok(interfaces) = g.list_network_interfaces() {
                    result["network"] = serde_json::json!({
                        "interfaces": interfaces,
                    });
                }

                // Get hostname
                if let Ok(hostname) = g.get_hostname() {
                    if let Some(net) = result.get_mut("network") {
                        net["hostname"] = serde_json::Value::String(hostname);
                    }
                }
            }

            // Collect security information if requested
            if payload_clone.options.include_security {
                let mut security = serde_json::json!({});

                // SELinux status
                if let Ok(selinux_status) = g.getcon() {
                    security["selinux"] = serde_json::json!({
                        "status": selinux_status,
                        "enabled": selinux_status != "disabled",
                    });
                }

                // Check for AppArmor
                if g.exists("/sys/kernel/security/apparmor").unwrap_or(false) {
                    security["apparmor"] = serde_json::json!({
                        "enabled": true,
                    });
                } else {
                    security["apparmor"] = serde_json::json!({
                        "enabled": false,
                    });
                }

                result["security"] = security;
            }

            // Collect storage / partition inventory (devices, fstypes, UUIDs, fstab)
            if payload_clone.options.include_storage {
                let mut partitions = Vec::new();
                if let Ok(fs_map) = g.list_filesystems() {
                    let mut devices: Vec<(String, String)> = fs_map.into_iter().collect();
                    devices.sort_by(|a, b| a.0.cmp(&b.0));
                    for (device, fstype) in devices {
                        let uuid = g.vfs_uuid(&device).ok();
                        partitions.push(serde_json::json!({
                            "device": device,
                            "fstype": fstype,
                            "uuid": uuid,
                        }));
                    }
                }
                let fstab = g.list_fstab().ok();
                if !partitions.is_empty() || fstab.is_some() {
                    result["storage"] = serde_json::json!({
                        "count": partitions.len(),
                        "partitions": partitions,
                        "fstab": fstab,
                    });
                }
            }

            // Collect installed kernels
            if let Ok(kernels) = g.list_kernels() {
                if !kernels.is_empty() {
                    let default = g.get_default_kernel().ok();
                    result["kernels"] = serde_json::json!({
                        "count": kernels.len(),
                        "installed": kernels,
                        "default": default,
                    });
                }
            }

            // Collect kernel modules configured to load at boot (drivers)
            {
                let mut modules: Vec<String> = Vec::new();
                if let Ok(content) = g.cat("/etc/modules") {
                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            modules.push(line.to_string());
                        }
                    }
                }
                if let Ok(files) = g.ls("/etc/modules-load.d") {
                    for file in files.iter().filter(|f| f.ends_with(".conf")) {
                        if let Ok(content) = g.cat(&format!("/etc/modules-load.d/{}", file)) {
                            for line in content.lines() {
                                let line = line.trim().to_string();
                                if !line.is_empty() && !line.starts_with('#') && !modules.contains(&line) {
                                    modules.push(line);
                                }
                            }
                        }
                    }
                }
                if !modules.is_empty() {
                    result["kernel_modules"] = serde_json::json!({
                        "count": modules.len(),
                        "modules": modules,
                    });
                }
            }

            // Collect systemd unit files (complements enabled_services)
            if let Ok(units) = g.list_systemd_units() {
                if !units.is_empty() {
                    result["systemd_units"] = serde_json::json!({
                        "count": units.len(),
                        "units": units,
                    });
                }
            }

            // Collect user accounts (parse /etc/passwd for uid/shell/login capability)
            if payload_clone.options.include_users {
                if let Ok(passwd) = g.cat("/etc/passwd") {
                    let mut accounts = Vec::new();
                    for line in passwd.lines() {
                        let fields: Vec<&str> = line.split(':').collect();
                        if fields.len() >= 7 {
                            let uid: u64 = fields[2].parse().unwrap_or(0);
                            let shell = fields[6];
                            let login = !(shell.ends_with("/nologin")
                                || shell.ends_with("/false")
                                || shell.is_empty());
                            accounts.push(serde_json::json!({
                                "name": fields[0],
                                "uid": uid,
                                "home": fields[5],
                                "shell": shell,
                                "login": login,
                                "system": uid < 1000,
                            }));
                        }
                    }
                    if !accounts.is_empty() {
                        let login_count = accounts
                            .iter()
                            .filter(|a| a["login"].as_bool().unwrap_or(false))
                            .count();
                        result["users"] = serde_json::json!({
                            "count": accounts.len(),
                            "login_count": login_count,
                            "accounts": accounts,
                        });
                    }
                }
            }

            // Extended network detail (DNS, gateway) merged into network object
            if payload_clone.options.include_network {
                if let Ok(dns) = g.get_dns() {
                    if !dns.is_empty() {
                        if !result["network"].is_object() { result["network"] = serde_json::json!({}); }
                        result["network"]["dns_servers"] = serde_json::json!(dns);
                    }
                }
                // Best-effort default gateway from netplan / ifupdown config
                let mut gateway: Option<String> = None;
                if let Ok(files) = g.ls("/etc/netplan") {
                    for f in files.iter().filter(|f| f.ends_with(".yaml") || f.ends_with(".yml")) {
                        if let Ok(content) = g.cat(&format!("/etc/netplan/{}", f)) {
                            for line in content.lines() {
                                let l = line.trim();
                                if let Some(rest) = l.strip_prefix("gateway4:").or_else(|| l.strip_prefix("via:")) {
                                    gateway = Some(rest.trim().trim_matches('"').to_string());
                                    break;
                                }
                            }
                        }
                        if gateway.is_some() { break; }
                    }
                }
                if gateway.is_none() {
                    if let Ok(content) = g.cat("/etc/network/interfaces") {
                        for line in content.lines() {
                            if let Some(rest) = line.trim().strip_prefix("gateway ") {
                                gateway = Some(rest.trim().to_string());
                                break;
                            }
                        }
                    }
                }
                if let Some(gw) = gateway {
                    if !result["network"].is_object() { result["network"] = serde_json::json!({}); }
                    result["network"]["gateway"] = serde_json::Value::String(gw);
                }
            }

            // Machine ID (identity)
            if let Ok(mid) = g.get_machine_id() {
                if !mid.is_empty() {
                    result["machine_id"] = serde_json::Value::String(mid);
                }
            }

            // cloud-init presence
            {
                let present = g.exists("/etc/cloud/cloud.cfg").unwrap_or(false)
                    || g.exists("/etc/cloud").unwrap_or(false);
                let disabled = g.exists("/etc/cloud/cloud-init.disabled").unwrap_or(false);
                if present {
                    result["cloud_init"] = serde_json::json!({
                        "present": true,
                        "enabled": !disabled,
                    });
                }
            }

            // VM guest tooling detection (offline, by well-known paths)
            {
                let probes: [(&str, &[&str]); 5] = [
                    ("open-vm-tools", &["/usr/bin/vmtoolsd", "/usr/bin/vmware-toolbox-cmd"]),
                    ("vmware-tools", &["/etc/vmware-tools", "/usr/lib/vmware-tools"]),
                    ("virtualbox-guest", &["/usr/bin/VBoxControl", "/opt/VBoxGuestAdditions"]),
                    ("hyperv", &["/usr/sbin/hv_kvp_daemon", "/usr/lib/hyperv-daemons"]),
                    ("qemu-guest-agent", &["/usr/sbin/qemu-ga", "/usr/bin/qemu-ga"]),
                ];
                let mut tools = Vec::new();
                for (name, paths) in probes.iter() {
                    if paths.iter().any(|p| g.exists(p).unwrap_or(false)) {
                        tools.push((*name).to_string());
                    }
                }
                if !tools.is_empty() {
                    result["vm_tools"] = serde_json::json!({ "detected": tools });
                }
            }

            // Firewall configuration (offline)
            {
                let mut fw = serde_json::Map::new();
                if let Ok(conf) = g.cat("/etc/ufw/ufw.conf") {
                    let enabled = conf.lines().any(|l| l.trim().eq_ignore_ascii_case("ENABLED=yes"));
                    fw.insert("ufw".into(), serde_json::json!({ "enabled": enabled }));
                }
                if g.exists("/etc/firewalld").unwrap_or(false) {
                    fw.insert("firewalld".into(), serde_json::json!(true));
                }
                if g.exists("/etc/iptables/rules.v4").unwrap_or(false)
                    || g.exists("/etc/sysconfig/iptables").unwrap_or(false) {
                    fw.insert("iptables".into(), serde_json::json!(true));
                }
                if !fw.is_empty() {
                    result["firewall"] = serde_json::Value::Object(fw);
                }
            }

            // SSH server policy (offline, from sshd_config)
            if let Ok(sshd) = g.get_sshd_config() {
                let mut root_login: Option<String> = None;
                let mut password_auth: Option<bool> = None;
                for line in sshd.lines() {
                    let l = line.trim();
                    if l.starts_with('#') { continue; }
                    let lower = l.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("permitrootlogin") {
                        root_login = Some(v.trim().to_string());
                    } else if let Some(v) = lower.strip_prefix("passwordauthentication") {
                        password_auth = Some(v.trim() == "yes");
                    }
                }
                if root_login.is_some() || password_auth.is_some() {
                    result["ssh"] = serde_json::json!({
                        "permit_root_login": root_login,
                        "password_auth": password_auth,
                    });
                }
            }

            // Unmount and cleanup
            let _ = g.umount_all();
            let _ = g.shutdown();

            Ok(result)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {}", e)))?
    }

    /// Write output to specified destination
    async fn write_output(
        &self,
        data: &serde_json::Value,
        output: &OutputSpec,
    ) -> WorkerResult<String> {
        let content = match output.format.as_str() {
            "json" => serde_json::to_string_pretty(data)?,
            "yaml" => serde_yaml::to_string(data)
                .map_err(|e| WorkerError::ExecutionError(format!("YAML serialization failed: {}", e)))?,
            _ => {
                return Err(WorkerError::ExecutionError(
                    format!("Unsupported output format: {}", output.format)
                ));
            }
        };

        // Ensure parent directory exists
        let output_path = std::path::Path::new(&output.destination);
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&output.destination, content).await?;

        Ok(output.destination.clone())
    }
}

impl Default for InspectHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OperationHandler for InspectHandler {
    fn name(&self) -> &str {
        "guestkit-inspect"
    }

    fn operations(&self) -> Vec<String> {
        vec!["guestkit.inspect".to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        // Parse and validate payload
        let inspect_payload: InspectPayload = serde_json::from_value(payload.data.clone())
            .map_err(|e| WorkerError::ExecutionError(
                format!("Invalid inspect payload: {}", e)
            ))?;

        // Validate image path
        if inspect_payload.image.path.is_empty() {
            return Err(WorkerError::ExecutionError(
                "Image path cannot be empty".to_string()
            ));
        }

        // Validate format
        let supported_formats = ["qcow2", "vmdk", "vdi", "vhdx", "raw", "img", "ova"];
        if !supported_formats.contains(&inspect_payload.image.format.as_str()) {
            return Err(WorkerError::ExecutionError(
                format!("Unsupported image format: {}", inspect_payload.image.format)
            ));
        }

        Ok(())
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        log::info!("Starting VM inspection for job {}", context.job_id);

        // Parse payload
        let inspect_payload: InspectPayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(
                format!("Failed to parse inspect payload: {}", e)
            ))?;

        // Perform inspection
        let result_data = self.inspect_vm(&context, &inspect_payload).await?;

        // Extract output path
        let output_file = result_data
            .get("output_file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(HandlerResult::new()
            .with_output(output_file.unwrap_or_default())
            .with_data(result_data))
    }

    async fn cleanup(&self, context: &HandlerContext) -> WorkerResult<()> {
        // Clean up any temporary files
        log::debug!("Cleanup for job {}", context.job_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressTracker;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_inspect_handler_validation() {
        let handler = InspectHandler::new();

        // Valid payload
        let valid_payload = Payload {
            payload_type: "guestkit.inspect.v1".to_string(),
            data: serde_json::json!({
                "image": {
                    "path": "/vms/test.qcow2",
                    "format": "qcow2"
                }
            }),
        };

        assert!(handler.validate(&valid_payload).await.is_ok());

        // Invalid format
        let invalid_payload = Payload {
            payload_type: "guestkit.inspect.v1".to_string(),
            data: serde_json::json!({
                "image": {
                    "path": "/vms/test.invalid",
                    "format": "invalid"
                }
            }),
        };

        assert!(handler.validate(&invalid_payload).await.is_err());
    }

    #[tokio::test]
    async fn test_inspect_handler_operations() {
        let handler = InspectHandler::new();
        assert_eq!(handler.operations(), vec!["guestkit.inspect"]);
        assert_eq!(handler.name(), "guestkit-inspect");
    }

    #[tokio::test]
    async fn test_checksum_verification_valid() {
        use std::io::Write;
        let handler = InspectHandler::new();

        // Create a temporary file with known content
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.img");
        let mut file = std::fs::File::create(&test_file).unwrap();
        file.write_all(b"test content for checksum").unwrap();
        drop(file);

        // Compute the expected SHA256
        // echo -n "test content for checksum" | sha256sum
        // Expected: c8ce4e97a404b12b1d8f0e245f04ff607be1048b16d973c2f23bab86655c808b
        let expected_hash = "c8ce4e97a404b12b1d8f0e245f04ff607be1048b16d973c2f23bab86655c808b";

        // Test with sha256: prefix
        let result = handler.verify_checksum(
            test_file.to_str().unwrap(),
            &format!("sha256:{}", expected_hash)
        ).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test without prefix (defaults to SHA256)
        let result = handler.verify_checksum(
            test_file.to_str().unwrap(),
            expected_hash
        ).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_checksum_verification_invalid() {
        use std::io::Write;
        let handler = InspectHandler::new();

        // Create a temporary file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.img");
        let mut file = std::fs::File::create(&test_file).unwrap();
        file.write_all(b"test content").unwrap();
        drop(file);

        // Use an incorrect checksum
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = handler.verify_checksum(
            test_file.to_str().unwrap(),
            &format!("sha256:{}", wrong_hash)
        ).await;

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for mismatch
    }

    #[tokio::test]
    async fn test_checksum_verification_unsupported_algorithm() {
        use std::io::Write;
        let handler = InspectHandler::new();

        // Create a temporary file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.img");
        let mut file = std::fs::File::create(&test_file).unwrap();
        file.write_all(b"test content").unwrap();
        drop(file);

        // Try with unsupported algorithm
        let result = handler.verify_checksum(
            test_file.to_str().unwrap(),
            "md5:abcdef123456"
        ).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unsupported checksum algorithm"));
    }

    #[tokio::test]
    async fn test_checksum_verification_nonexistent_file() {
        let handler = InspectHandler::new();

        let result = handler.verify_checksum(
            "/nonexistent/path/to/file.img",
            "sha256:1234567890abcdef"
        ).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to open image"));
    }
}
