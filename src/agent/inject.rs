// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline agent injection into disk images via guestfs.

use crate::cli::plan::types::{
    CommandExec, FileCopy, FixPlan, Operation, OperationType, PostApplyAction, Priority,
};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub const GUEST_BINARY_DEST: &str = "/usr/local/bin/zyvor-guest-agent";
pub const GUEST_UNIT_DEST: &str = "/etc/systemd/system/zyvor-guest-agent.service";
const DEFAULT_UNIT: &str = include_str!("../../templates/agent/zyvor-guest-agent.service");

const KUBEVIRT_CHANNEL_HINT: &str = r#"Add virtio-serial channel (QGA-compatible) to VMI/libvirt domain:
spec:
  domain:
    devices:
      channels:
      - name: org.qemu.guest_agent.0
        target:
          type: virtio
          name: org.qemu.guest_agent.0
Guest device: /dev/virtio-ports/org.qemu.guest_agent.0"#;

/// Append agent install operations to a fix plan (for export / preview).
pub fn append_agent_ops(plan: &mut FixPlan, binary: &Path, unit_content: &str) -> Result<()> {
    plan.metadata.tags.push("inject-agent".to_string());
    let op_base = plan.operations.len() + 1;

    plan.operations.push(Operation {
        id: format!("agent-{op_base:03}"),
        op_type: OperationType::FileCopy(FileCopy {
            source: binary.display().to_string(),
            destination: GUEST_BINARY_DEST.to_string(),
            backup: false,
        }),
        priority: Priority::Medium,
        description: "Install guestkit binary for in-guest agent".to_string(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![],
        validation: None,
        undo: None,
    });

    let unit_staging =
        std::env::temp_dir().join(format!("zyvor-guest-agent-{}.service", std::process::id()));
    fs::write(&unit_staging, unit_content)?;

    plan.operations.push(Operation {
        id: format!("agent-{:03}", op_base + 1),
        op_type: OperationType::FileCopy(FileCopy {
            source: unit_staging.display().to_string(),
            destination: GUEST_UNIT_DEST.to_string(),
            backup: false,
        }),
        priority: Priority::Medium,
        description: "Install guestkit-agent systemd unit".to_string(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![format!("agent-{op_base:03}")],
        validation: None,
        undo: None,
    });

    plan.operations.push(Operation {
        id: format!("agent-{:03}", op_base + 2),
        op_type: OperationType::CommandExec(CommandExec {
            interpreter: None,
            command: "chmod 0755 /usr/local/bin/zyvor-guest-agent".to_string(),
            expected_exit: 0,
            timeout: Some(30),
        }),
        priority: Priority::Low,
        description: "Make zyvor-guest-agent binary executable".to_string(),
        risk: Priority::Low,
        reversible: false,
        depends_on: vec![format!("agent-{op_base:03}")],
        validation: None,
        undo: None,
    });

    plan.operations.push(Operation {
        id: format!("agent-{:03}", op_base + 3),
        op_type: OperationType::CommandExec(CommandExec {
            interpreter: None,
            command: "systemctl enable zyvor-guest-agent || true".to_string(),
            expected_exit: 0,
            timeout: Some(60),
        }),
        priority: Priority::Medium,
        description: "Enable zyvor-guest-agent on boot".to_string(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![format!("agent-{:03}", op_base + 1)],
        validation: None,
        undo: None,
    });

    plan.post_apply.push(PostApplyAction::Message {
        message: KUBEVIRT_CHANNEL_HINT.to_string(),
    });

    Ok(())
}

/// Inject agent binary and systemd unit into an offline disk image.
pub fn inject_agent_into_image(
    image: &Path,
    binary: &Path,
    unit_content: &str,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    if !binary.exists() {
        anyhow::bail!("agent binary not found: {}", binary.display());
    }
    if dry_run {
        println!("Dry run — would inject agent:");
        println!("  binary: {} → {}", binary.display(), GUEST_BINARY_DEST);
        println!("  unit → {}", GUEST_UNIT_DEST);
        return Ok(());
    }

    if verbose {
        println!("Injecting GuestKit agent into {}", image.display());
    }

    let mut g = crate::Guestfs::new().context("create guestfs")?;
    g.add_drive(
        image
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid image path"))?,
    )?;
    g.launch()?;

    let roots = g.inspect_os()?;
    if roots.is_empty() {
        anyhow::bail!("no operating system found in disk image");
    }
    let root = &roots[0];
    if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
        let mut mounts: Vec<_> = mountpoints.into_iter().collect();
        mounts.sort_by_key(|(m, _)| m.len());
        for (mount, device) in &mounts {
            let _ = g.mount(device, mount);
        }
    }

    g.upload(
        binary
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("binary path"))?,
        GUEST_BINARY_DEST,
    )?;
    g.command(&["chmod", "0755", GUEST_BINARY_DEST])?;

    let unit_tmp = tempfile::NamedTempFile::new()?;
    fs::write(unit_tmp.path(), unit_content)?;
    g.upload(
        unit_tmp
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("temp path"))?,
        GUEST_UNIT_DEST,
    )?;

    let _ = g.command(&["systemctl", "enable", "zyvor-guest-agent"]);
    let _ = g.umount_all();
    g.shutdown()?;

    if verbose {
        println!("Agent injected successfully");
    }
    Ok(())
}

/// Windows install directory for the agent (guest path, forward-slash form).
pub const WIN_AGENT_DIR: &str = "/guestkit";
/// Windows agent binary destination (guest path).
pub const WIN_AGENT_DEST: &str = "/guestkit/guestkitd.exe";
/// Windows service name registered for the agent (matches the service crate).
pub const WIN_SERVICE_NAME: &str = "GuestKitAgent";

/// Offline-install the GuestKit agent into a **Windows** disk image, entirely
/// through guestkit's own machinery: copy `guestkitd.exe` into `C:\guestkit\`
/// and register the `GuestKitAgent` Windows service (auto-start, LocalSystem,
/// `--service`) by writing the offline `SYSTEM` hive with guestkit's hivex
/// registry-write. The injected service answers the QEMU guest-agent
/// virtio-serial channel at boot, so the host can drive it via
/// `virsh qemu-agent-command … guestkit-rpc` exactly like the Linux agent.
///
/// This replaces hand-rolled `virt-win-reg` service registration and, per the
/// standing rule, keeps offline Windows provisioning inside guestkit (which
/// also exercises the hivex path on real disks).
#[cfg(feature = "registry-write")]
pub fn inject_windows_agent(
    image: &Path,
    binary: &Path,
    virtio_serial_driver: Option<&Path>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    use serde_json::json;

    if !binary.exists() {
        anyhow::bail!("agent binary not found: {}", binary.display());
    }
    if dry_run {
        println!("Dry run — would inject Windows agent:");
        println!(
            "  binary: {} → C:\\guestkit\\guestkitd.exe",
            binary.display()
        );
        println!("  register service {WIN_SERVICE_NAME} (auto-start, LocalSystem, --service)");
        if let Some(d) = virtio_serial_driver {
            println!("  preinstall virtio-serial driver from {}", d.display());
        }
        return Ok(());
    }

    if verbose {
        println!("Injecting GuestKit Windows agent into {}", image.display());
    }

    let mut g = crate::Guestfs::new().context("create guestfs")?;
    g.add_drive(
        image
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid image path"))?,
    )?;
    g.launch()?;

    let roots = g.inspect_os()?;
    let root = roots
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no operating system found in disk image"))?;
    if g.inspect_get_type(&root)? != "windows" {
        anyhow::bail!(
            "inject_windows_agent: image root {root} is not Windows (use inject_agent_into_image)"
        );
    }

    // Do **not** call inspect_get_mountpoints here: it mount_ro()'s the Windows
    // root to hunt for /etc/fstab and leaves it mounted. The follow-up mount()
    // then hits remount_rw, which is fragile on NTFS (dirty volumes / kernel
    // ntfs3) and used to leave C: read-only — mkdir_p was ignored and upload
    // failed with ENOENT for /guestkit/guestkitd.exe. Mount RW directly.
    let _ = g.umount_all();
    if g.vfs_type(&root).ok().as_deref() == Some("ntfs") {
        if let Err(e) = g.ntfsfix_opts(&root, false, true) {
            if verbose {
                println!("  ntfsfix {root}: {e} (continuing)");
            }
        }
    }
    g.mount(&root, "/")
        .with_context(|| format!("mount {root} read-write at /"))?;

    // 1. Copy the agent binary into C:\guestkit\.
    g.mkdir_p(WIN_AGENT_DIR)
        .context("creating C:\\guestkit on the Windows volume")?;
    g.upload(
        binary
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("binary path"))?,
        WIN_AGENT_DEST,
    )?;
    if verbose {
        println!("  copied agent → C:\\guestkit\\guestkitd.exe");
    }

    // 2. Register the service by writing the offline SYSTEM hive with hivex.
    let hive_guest_path = g.inspect_get_windows_system_hive(&root)?;
    let hive_tmp = tempfile::NamedTempFile::new()?;
    let hive_host = hive_tmp
        .path()
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("temp hive path"))?;
    g.download_hive(&hive_guest_path, hive_host)?;

    // ControlSet001 is the on-disk control set; the running system aliases it as
    // CurrentControlSet. Registering there makes the service present at boot.
    let subpath: Vec<String> = ["ControlSet001", "Services", WIN_SERVICE_NAME]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // SERVICE_WIN32_OWN_PROCESS (0x10), SERVICE_AUTO_START (2), errorctl NORMAL (1).
    let values: &[(&str, &str, serde_json::Value)] = &[
        ("Type", "dword", json!(0x10)),
        ("Start", "dword", json!(2)),
        ("ErrorControl", "dword", json!(1)),
        (
            "ImagePath",
            "expand_sz",
            json!("C:\\guestkit\\guestkitd.exe --service"),
        ),
        ("DisplayName", "sz", json!("Zyvor GuestKit Agent")),
        ("ObjectName", "sz", json!("LocalSystem")),
        (
            "Description",
            "sz",
            json!("GuestKit in-guest agent (QGA virtio-serial channel)."),
        ),
    ];
    for (name, ty, data) in values {
        crate::guestfs::hivex_ffi::set_registry_value(hive_tmp.path(), &subpath, name, ty, data)
            .map_err(|e| anyhow::anyhow!("set {name}: {e}"))?;
    }
    // The GuestKit agent owns the QGA virtio-serial channel; a stock
    // qemu-guest-agent would contend for the same port. Disable known stock QGA
    // services (Start=4 = SERVICE_DISABLED) so GuestKit answers it uncontended.
    // GuestKit implements the QGA compat commands, so KubeVirt/libvirt stays
    // connected. Creating the key when the service is absent is harmless.
    for svc in ["QEMU-GA", "qemu-ga", "QEMUGuestAgent"] {
        let svc_path: Vec<String> = ["ControlSet001", "Services", svc]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let _ = crate::guestfs::hivex_ffi::set_registry_value(
            hive_tmp.path(),
            &svc_path,
            "Start",
            "dword",
            &json!(4),
        );
    }
    g.upload_hive(hive_host, &hive_guest_path)?;
    if verbose {
        println!("  registered service {WIN_SERVICE_NAME}; disabled stock qemu-ga in SYSTEM hive");
    }

    // 3. Preinstall the virtio-serial driver so the QGA channel device exists.
    if let Some(driver_dir) = virtio_serial_driver {
        inject_windows_driver_dir(&mut g, &root, driver_dir, "VirtioSerial", verbose)?;
    }

    let _ = g.umount_all();
    g.shutdown()?;

    if verbose {
        println!(
            "Windows agent injected; the {WIN_SERVICE_NAME} service answers the QGA virtio channel at boot."
        );
    }
    Ok(())
}

/// Offline-preinstall a Windows driver package so PnP installs it on next boot:
/// copy the driver files into `C:\Windows\Drivers\<dest_name>\` (and the `.sys`
/// also into `System32\drivers\`) and add that directory to the SOFTWARE-hive
/// `DevicePath` so Windows PnP searches it when it enumerates the matching
/// device. This is how the virtio-serial (`vioser`) driver — required for the
/// QGA channel the agent answers — gets installed on a guest that shipped
/// without it (`--inject-virtio-win` only lands the boot-critical block driver).
#[cfg(feature = "registry-write")]
pub fn inject_windows_driver_dir(
    g: &mut crate::Guestfs,
    root: &str,
    driver_dir: &Path,
    dest_name: &str,
    verbose: bool,
) -> Result<()> {
    let guest_dir = format!("/Windows/Drivers/{dest_name}");
    let _ = g.mkdir_p("/Windows/Drivers");
    let _ = g.mkdir_p(&guest_dir);

    let mut copied = 0usize;
    for entry in fs::read_dir(driver_dir)
        .with_context(|| format!("read driver dir {}", driver_dir.display()))?
    {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // The files PnP needs to install and load the driver.
        if !matches!(ext.as_str(), "inf" | "sys" | "cat" | "dll") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("driver filename"))?;
        let src = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("driver path"))?;
        g.upload(src, &format!("{guest_dir}/{name}"))?;
        if ext == "sys" {
            // Kernel driver binary is loaded from System32\drivers.
            let _ = g.upload(src, &format!("/Windows/System32/drivers/{name}"));
        }
        copied += 1;
        if verbose {
            println!("  driver file → C:\\Windows\\Drivers\\{dest_name}\\{name}");
        }
    }
    if copied == 0 {
        anyhow::bail!(
            "no .inf/.sys/.cat driver files found in {}",
            driver_dir.display()
        );
    }

    // Add the driver directory to DevicePath so PnP finds the INF on next boot.
    // The Windows 10 default is `%SystemRoot%\inf`; keep it and append ours.
    let sw_hive = g.inspect_get_windows_software_hive(root)?;
    let hive_tmp = tempfile::NamedTempFile::new()?;
    let hive_host = hive_tmp
        .path()
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("temp hive path"))?;
    g.download_hive(&sw_hive, hive_host)?;
    let subpath: Vec<String> = ["Microsoft", "Windows", "CurrentVersion"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    crate::guestfs::hivex_ffi::set_registry_value(
        hive_tmp.path(),
        &subpath,
        "DevicePath",
        "expand_sz",
        &serde_json::json!(format!(
            "%SystemRoot%\\inf;%SystemRoot%\\Drivers\\{dest_name}"
        )),
    )
    .map_err(|e| anyhow::anyhow!("set DevicePath: {e}"))?;
    g.upload_hive(hive_host, &sw_hive)?;
    if verbose {
        println!("  DevicePath now includes C:\\Windows\\Drivers\\{dest_name}");
    }

    // DevicePath alone does not re-install a device Windows already enumerated
    // as "no driver" on a prior boot (nothing appears in setupapi.dev.log).
    // Force the binding via the CriticalDeviceDatabase + a driver service key,
    // parsed from the INF, so Windows loads the driver for the device HWIDs at
    // boot regardless of PnP install state. This is the reliable offline path.
    if let Some(inf) = find_inf(driver_dir)? {
        let meta =
            parse_driver_inf(&inf).with_context(|| format!("parse INF {}", inf.display()))?;
        bind_driver_system_hive(g, root, &meta, verbose)?;
    } else if verbose {
        println!("  no .inf found; skipped CriticalDeviceDatabase binding");
    }
    Ok(())
}

/// Minimal driver metadata parsed from a PnP INF, enough to force-bind the
/// driver to its device HWIDs offline via the CriticalDeviceDatabase.
#[cfg(feature = "registry-write")]
#[derive(Debug, Default)]
struct DriverInf {
    class_guid: String,
    service: String,
    sys_file: String,
    service_type: u32,
    start_type: u32,
    error_control: u32,
    kmdf_version: Option<String>,
    display_name: Option<String>,
    /// Device hardware IDs, e.g. "PCI\\VEN_1AF4&DEV_1043".
    hardware_ids: Vec<String>,
}

#[cfg(feature = "registry-write")]
fn find_inf(dir: &Path) -> Result<Option<PathBuf>> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let p = entry?.path();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("inf"))
            == Some(true)
        {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// Parse the handful of INF directives needed for CDDB binding. This is not a
/// general INF parser — it targets the standard virtio device-install layout
/// (Class/ClassGuid, AddService, the *_Service_Inst section, and the hardware
/// IDs listed in the model section).
#[cfg(feature = "registry-write")]
fn parse_driver_inf(inf: &Path) -> Result<DriverInf> {
    let text = fs::read_to_string(inf).with_context(|| format!("read {}", inf.display()))?;
    let mut m = DriverInf {
        service_type: 1,
        start_type: 3,
        error_control: 1,
        ..Default::default()
    };
    // ClassGuid, KmdfLibraryVersion, AddService, and ServiceType/StartType are
    // simple `key = value` lines; scan the whole file for them.
    for raw in text.lines() {
        let line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(v) = kv(line, "classguid") {
            m.class_guid = v.trim().to_string();
        } else if let Some(v) = kv(line, "kmdflibraryversion") {
            m.kmdf_version = Some(v.trim().to_string());
        } else if lower.starts_with("addservice") {
            // AddService = <ServiceName>, <flags>, <install-section>
            if let Some(rhs) = line.split('=').nth(1) {
                if let Some(first) = rhs.split(',').next() {
                    m.service = first.trim().to_string();
                }
            }
        } else if let Some(v) = kv(line, "servicetype") {
            m.service_type = parse_int(v).unwrap_or(1);
        } else if let Some(v) = kv(line, "starttype") {
            m.start_type = parse_int(v).unwrap_or(3);
        } else if let Some(v) = kv(line, "errorcontrol") {
            m.error_control = parse_int(v).unwrap_or(1);
        } else if let Some(v) = kv(line, "servicebinary") {
            // %12%\vioser.sys -> vioser.sys
            let f = v.rsplit(['\\', '/']).next().unwrap_or("").trim();
            if !f.is_empty() {
                m.sys_file = f.to_string();
            }
        }
        // Hardware IDs: any token like PCI\VEN_xxxx&DEV_xxxx (with or without
        // SUBSYS/REV). Collect both the specific and the generic forms.
        for tok in line.split([',', '=', ' ', '\t']) {
            let t = tok.trim();
            if t.to_ascii_uppercase().starts_with("PCI\\VEN_") && t.contains("&DEV_") {
                let up = t.to_ascii_uppercase();
                if !m.hardware_ids.contains(&up) {
                    m.hardware_ids.push(up);
                }
            }
        }
    }
    if m.service.is_empty() || m.class_guid.is_empty() || m.hardware_ids.is_empty() {
        anyhow::bail!(
            "INF missing required fields (service={:?} class={:?} hwids={})",
            m.service,
            m.class_guid,
            m.hardware_ids.len()
        );
    }
    if m.sys_file.is_empty() {
        // Fall back to <service>.sys.
        m.sys_file = format!("{}.sys", m.service.to_ascii_lowercase());
    }
    Ok(m)
}

#[cfg(feature = "registry-write")]
fn kv<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (k, v) = line.split_once('=')?;
    if k.trim().eq_ignore_ascii_case(key) {
        Some(v)
    } else {
        None
    }
}

#[cfg(feature = "registry-write")]
fn parse_int(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// Write the SYSTEM-hive entries that force Windows to load `meta`'s driver for
/// its device HWIDs at boot: the driver service key (+ KMDF binding) and a
/// CriticalDeviceDatabase entry per hardware ID.
#[cfg(feature = "registry-write")]
fn bind_driver_system_hive(
    g: &mut crate::Guestfs,
    root: &str,
    meta: &DriverInf,
    verbose: bool,
) -> Result<()> {
    use crate::guestfs::hivex_ffi::set_registry_value;
    use serde_json::json;

    let sys_hive = g.inspect_get_windows_system_hive(root)?;
    let hive_tmp = tempfile::NamedTempFile::new()?;
    let hive_path = hive_tmp.path();
    let hive_host = hive_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("temp hive path"))?;
    g.download_hive(&sys_hive, hive_host)?;

    // 1. Driver service key: HKLM\SYSTEM\ControlSet001\Services\<service>.
    let svc: Vec<String> = ["ControlSet001", "Services", &meta.service]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let image_path = format!("System32\\drivers\\{}", meta.sys_file);
    let svc_vals: Vec<(&str, &str, serde_json::Value)> = vec![
        ("Type", "dword", json!(meta.service_type)),
        ("Start", "dword", json!(meta.start_type)),
        ("ErrorControl", "dword", json!(meta.error_control)),
        ("ImagePath", "expand_sz", json!(image_path)),
        (
            "DisplayName",
            "sz",
            json!(meta
                .display_name
                .clone()
                .unwrap_or_else(|| meta.service.clone())),
        ),
    ];
    for (n, t, d) in &svc_vals {
        set_registry_value(hive_path, &svc, n, t, d)
            .map_err(|e| anyhow::anyhow!("service {n}: {e}"))?;
    }
    // KMDF binding so a WDF driver actually starts (Parameters\Wdf).
    if let Some(ver) = &meta.kmdf_version {
        let wdf: Vec<String> = [
            "ControlSet001",
            "Services",
            &meta.service,
            "Parameters",
            "Wdf",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        set_registry_value(hive_path, &wdf, "KmdfLibraryVersion", "sz", &json!(ver))
            .map_err(|e| anyhow::anyhow!("KmdfLibraryVersion: {e}"))?;
    }

    // 2. CriticalDeviceDatabase entries: one per hardware ID, key name is the
    //    HWID lowercased with '\' -> '#'. Windows binds the device to the named
    //    service very early, bypassing interactive PnP install.
    let mut cddb_count = 0;
    for hwid in &meta.hardware_ids {
        let key = hwid.to_ascii_lowercase().replace('\\', "#");
        let cddb: Vec<String> = [
            "ControlSet001".to_string(),
            "Control".to_string(),
            "CriticalDeviceDatabase".to_string(),
            key,
        ]
        .to_vec();
        set_registry_value(hive_path, &cddb, "Service", "sz", &json!(meta.service))
            .map_err(|e| anyhow::anyhow!("CDDB Service: {e}"))?;
        set_registry_value(hive_path, &cddb, "ClassGUID", "sz", &json!(meta.class_guid))
            .map_err(|e| anyhow::anyhow!("CDDB ClassGUID: {e}"))?;
        cddb_count += 1;
    }

    // 3. Force a real driver (re)install on next boot for the device. A device
    //    Windows already enumerated as "no driver" is not re-searched just
    //    because a driver became available; setting CONFIGFLAG_REINSTALL on its
    //    devnode makes PnP run a full INF install (which, unlike CDDB alone,
    //    correctly completes a KMDF driver's WDF binding).
    let needles: Vec<String> = meta
        .hardware_ids
        .iter()
        .filter_map(|h| h.strip_prefix("PCI\\").or(Some(h.as_str())))
        .map(|h| {
            // Keep the "VEN_xxxx&DEV_xxxx" prefix so it matches the enum key
            // regardless of SUBSYS/REV suffix.
            let parts: Vec<&str> = h.split('&').collect();
            if parts.len() >= 2 {
                format!("{}&{}", parts[0], parts[1])
            } else {
                h.to_string()
            }
        })
        .collect();
    let needle_refs: Vec<&str> = needles.iter().map(|s| s.as_str()).collect();
    // Deleting the cached devnode is the strongest forcing function: the PCI bus
    // re-detects the device as brand-new next boot and runs a full INF install.
    // (Additive ConfigFlags on an existing "no driver" devnode was not honored on
    // converted images — setupapi logged no install attempt.)
    let deleted =
        crate::guestfs::hivex_ffi::delete_device_nodes(hive_path, "ControlSet001", &needle_refs)
            .map_err(|e| anyhow::anyhow!("delete device nodes: {e}"))?;
    // If nothing was deleted (device key absent), fall back to the reinstall flag.
    let reinstalled = if deleted == 0 {
        crate::guestfs::hivex_ffi::set_configflags_reinstall(
            hive_path,
            "ControlSet001",
            &needle_refs,
        )
        .map_err(|e| anyhow::anyhow!("set ConfigFlags reinstall: {e}"))?
    } else {
        0
    };

    g.upload_hive(hive_host, &sys_hive)?;
    if verbose {
        println!(
            "  bound driver service {} for {} HWID(s) via CriticalDeviceDatabase",
            meta.service, cddb_count
        );
        if deleted > 0 {
            println!("  deleted {deleted} cached devnode(s) to force fresh driver install");
        } else {
            println!("  flagged {reinstalled} device instance(s) for driver reinstall");
        }
    }
    Ok(())
}

/// Resolve agent binary path (explicit or current executable).
pub fn resolve_agent_binary(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    std::env::current_exe().context("resolve guestkit binary for injection")
}

/// Load systemd unit content from path or built-in template.
pub fn resolve_agent_unit(explicit: Option<&Path>) -> Result<String> {
    if let Some(p) = explicit {
        return fs::read_to_string(p).with_context(|| format!("read unit file {}", p.display()));
    }
    Ok(DEFAULT_UNIT.to_string())
}
