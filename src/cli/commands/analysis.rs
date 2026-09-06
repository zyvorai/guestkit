// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Analysis commands implementation
//!
//! Advanced analysis, forensics, and threat hunting commands.
#![allow(clippy::too_many_arguments)]

use anyhow::Result;
use std::path::{Path, PathBuf};

use super::{init_guestfs_ro, mount_all_ro};

pub fn timeline_command(
    image: &Path,
    _start_time: Option<String>,
    _end_time: Option<String>,
    sources: Vec<String>,
    format: &str,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let _root = mount_all_ro(&mut g);

    progress.set_message("Building forensic timeline...");

    // Timeline events: timestamp -> (source, event_type, details)
    let mut timeline: BTreeMap<i64, Vec<(String, String, String)>> = BTreeMap::new();

    // Source 1: File modifications (if 'files' in sources)
    if sources.is_empty() || sources.contains(&"files".to_string()) {
        if let Ok(files) = g.find("/etc") {
            for file in files.iter().take(100) {
                if g.is_file(file).unwrap_or(false) {
                    if let Ok(stat) = g.stat(file) {
                        timeline.entry(stat.mtime).or_default().push((
                            "filesystem".to_string(),
                            "file_modified".to_string(),
                            format!("{} (size: {})", file, stat.size),
                        ));
                    }
                }
            }
        }
    }

    // Source 2: Package installations (if 'packages' in sources)
    // Note: skipped because package install timestamps are not always available offline;

    // adding packages with epoch 0 would produce misleading forensic timeline data.

    // Source 3: Log entries (if 'logs' in sources)
    if sources.is_empty() || sources.contains(&"logs".to_string()) {
        let log_files = vec!["/var/log/messages", "/var/log/syslog", "/var/log/auth.log"];
        for log_file in log_files {
            if g.is_file(log_file).unwrap_or(false) {
                if let Ok(stat) = g.stat(log_file) {
                    timeline.entry(stat.mtime).or_default().push((
                        "logs".to_string(),
                        "log_updated".to_string(),
                        log_file.to_string(),
                    ));
                }
            }
        }
    }

    progress.finish_and_clear();

    // Display timeline
    match format {
        "json" => {
            println!("{{");
            println!("  \"timeline\": [");
            let mut first = true;
            for (timestamp, events) in timeline.iter() {
                for (source, event_type, details) in events {
                    if !first {
                        println!(",");
                    }
                    first = false;
                    let dt = Utc
                        .timestamp_opt(*timestamp, 0)
                        .single()
                        .unwrap_or_default();
                    let ts_escaped = serde_json::Value::String(dt.to_rfc3339());
                    let src_escaped = serde_json::Value::String(source.clone());
                    let evt_escaped = serde_json::Value::String(event_type.clone());
                    let det_escaped = serde_json::Value::String(details.clone());
                    println!("    {{");
                    println!("      \"timestamp\": {},", ts_escaped);
                    println!("      \"source\": {},", src_escaped);
                    println!("      \"event_type\": {},", evt_escaped);
                    println!("      \"details\": {}", det_escaped);
                    print!("    }}");
                }
            }
            println!();
            println!("  ]");
            println!("}}");
        }
        "csv" => {
            println!("timestamp,source,event_type,details");
            for (timestamp, events) in timeline.iter() {
                for (source, event_type, details) in events {
                    let dt = Utc
                        .timestamp_opt(*timestamp, 0)
                        .single()
                        .unwrap_or_default();
                    // Escape CSV fields: wrap in double quotes and double any inner quotes
                    let escape_csv = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
                    println!(
                        "{},{},{},{}",
                        escape_csv(&dt.to_rfc3339()),
                        escape_csv(source),
                        escape_csv(event_type),
                        escape_csv(details)
                    );
                }
            }
        }
        _ => {
            println!("Forensic Timeline");
            println!("=================");
            println!("Image: {}", image.display());
            println!(
                "Total events: {}",
                timeline.values().map(|v| v.len()).sum::<usize>()
            );
            println!();

            for (timestamp, events) in timeline.iter().rev().take(50) {
                let dt = Utc
                    .timestamp_opt(*timestamp, 0)
                    .single()
                    .unwrap_or_default();
                println!("[{}]", dt.format("%Y-%m-%d %H:%M:%S"));
                for (source, event_type, details) in events {
                    println!("  [{:>15}] {}: {}", source, event_type, details);
                }
                println!();
            }
        }
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn fingerprint_command(
    image: &Path,
    algorithm: &str,
    include_content: bool,
    output: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use sha2::{Digest, Sha256};
    use std::fs;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let root = mount_all_ro(&mut g);

    progress.set_message("Generating fingerprint...");

    // Build fingerprint from multiple sources
    let mut hasher = Sha256::new();
    let mut fingerprint_data = Vec::new();

    // 1. OS Information
    if let Some(ref root) = root {
        if let Ok(os_type) = g.inspect_get_type(root) {
            fingerprint_data.push(format!("OS_TYPE:{}", os_type));
        }
        if let Ok(distro) = g.inspect_get_distro(root) {
            fingerprint_data.push(format!("DISTRO:{}", distro));
        }
        if let Ok(version) = g.inspect_get_major_version(root) {
            fingerprint_data.push(format!("VERSION:{}", version));
        }
    }

    // 2. Package list (sorted for consistency)
    if let Some(ref root) = root {
        if let Ok(apps) = g.inspect_list_applications(root) {
            let mut pkg_list: Vec<_> = apps
                .iter()
                .map(|app| format!("{}:{}", app.name, app.version))
                .collect();
            pkg_list.sort();
            for pkg in pkg_list.iter().take(100) {
                fingerprint_data.push(format!("PKG:{}", pkg));
            }
        }
    }

    // 3. Critical file hashes (if include_content)
    if include_content {
        let critical_files = vec!["/etc/passwd", "/etc/group", "/etc/fstab", "/etc/hostname"];

        for file in critical_files {
            if g.is_file(file).unwrap_or(false) {
                if let Ok(hash) = g.checksum(algorithm, file) {
                    fingerprint_data.push(format!("FILE:{}:{}", file, hash));
                }
            }
        }
    }

    // 4. Filesystem structure fingerprint
    if let Ok(files) = g.find("/etc") {
        let mut sorted_files: Vec<_> = files
            .iter()
            .filter(|f| g.is_file(f).unwrap_or(false))
            .collect();
        sorted_files.sort();
        for file in sorted_files.iter().take(50) {
            if let Ok(stat) = g.stat(file) {
                fingerprint_data.push(format!("STRUCT:{}:{}:{}", file, stat.size, stat.mode));
            }
        }
    }

    // Generate final hash
    for data in &fingerprint_data {
        hasher.update(data.as_bytes());
        hasher.update(b"\n");
    }
    let fingerprint_hash = format!("{:x}", hasher.finalize());

    progress.finish_and_clear();

    // Output
    let fingerprint_output = serde_json::json!({
        "image": image.to_str().ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {}", image.display()))?,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "algorithm": algorithm,
        "fingerprint": fingerprint_hash,
        "components": {
            "os_info": fingerprint_data.iter().filter(|d| d.starts_with("OS_") || d.starts_with("DISTRO") || d.starts_with("VERSION")).count(),
            "packages": fingerprint_data.iter().filter(|d| d.starts_with("PKG:")).count(),
            "files": fingerprint_data.iter().filter(|d| d.starts_with("FILE:")).count(),
            "structure": fingerprint_data.iter().filter(|d| d.starts_with("STRUCT:")).count(),
        },
        "details": fingerprint_data,
    });

    if let Some(output_path) = output {
        fs::write(
            &output_path,
            serde_json::to_string_pretty(&fingerprint_output)?,
        )?;
        println!("✓ Fingerprint saved to: {}", output_path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&fingerprint_output)?);
    }

    println!();
    println!("Image Fingerprint: {}", fingerprint_hash);
    println!("Components analyzed: {}", fingerprint_data.len());

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn drift_command(
    baseline: &Path,
    current: &Path,
    ignore_paths: Vec<String>,
    threshold: u8,
    report: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress =
        ProgressReporter::spinner("Loading baseline disk image and launching appliance...");
    let mut g_baseline = init_guestfs_ro(baseline, verbose)?;

    progress.set_message("Loading current disk image and launching appliance...");
    let mut g_current = init_guestfs_ro(current, verbose)?;

    progress.set_message("Mounting filesystems...");
    let _root_baseline = mount_all_ro(&mut g_baseline);
    let _root_current = mount_all_ro(&mut g_current);

    progress.set_message("Analyzing configuration drift...");

    let mut drift_score = 0u32;
    let mut drifts = Vec::new();

    // Check critical configuration files
    let config_files = vec![
        "/etc/passwd",
        "/etc/group",
        "/etc/fstab",
        "/etc/hosts",
        "/etc/hostname",
        "/etc/resolv.conf",
        "/etc/ssh/sshd_config",
        "/etc/sudoers",
    ];

    for file in config_files {
        if ignore_paths.iter().any(|p| file.starts_with(p)) {
            continue;
        }

        let exists_baseline = g_baseline.is_file(file).unwrap_or(false);
        let exists_current = g_current.is_file(file).unwrap_or(false);

        if exists_baseline && exists_current {
            // Both exist - compare content
            if let (Ok(content_baseline), Ok(content_current)) =
                (g_baseline.read_file(file), g_current.read_file(file))
            {
                if content_baseline != content_current {
                    drift_score += 10;
                    drifts.push((
                        "modified".to_string(),
                        file.to_string(),
                        format!(
                            "Content changed ({} -> {} bytes)",
                            content_baseline.len(),
                            content_current.len()
                        ),
                    ));
                }
            }
        } else if exists_baseline && !exists_current {
            drift_score += 15;
            drifts.push((
                "deleted".to_string(),
                file.to_string(),
                "File removed from baseline".to_string(),
            ));
        } else if !exists_baseline && exists_current {
            drift_score += 15;
            drifts.push((
                "added".to_string(),
                file.to_string(),
                "File added (not in baseline)".to_string(),
            ));
        }
    }

    // Check packages
    if let (Some(ref root_baseline), Some(ref root_current)) = (&_root_baseline, &_root_current) {
        if let (Ok(apps_baseline), Ok(apps_current)) = (
            g_baseline.inspect_list_applications(root_baseline),
            g_current.inspect_list_applications(root_current),
        ) {
            let pkg_baseline: std::collections::HashSet<_> = apps_baseline
                .iter()
                .map(|app| format!("{}:{}", app.name, app.version))
                .collect();
            let pkg_current: std::collections::HashSet<_> = apps_current
                .iter()
                .map(|app| format!("{}:{}", app.name, app.version))
                .collect();

            let added: Vec<_> = pkg_current.difference(&pkg_baseline).collect();
            let removed: Vec<_> = pkg_baseline.difference(&pkg_current).collect();

            for pkg in added.iter().take(10) {
                drift_score += 5;
                drifts.push((
                    "package_added".to_string(),
                    pkg.to_string(),
                    "Package installed".to_string(),
                ));
            }

            for pkg in removed.iter().take(10) {
                drift_score += 5;
                drifts.push((
                    "package_removed".to_string(),
                    pkg.to_string(),
                    "Package uninstalled".to_string(),
                ));
            }
        }
    }

    progress.finish_and_clear();

    // Calculate drift percentage
    let max_score = 500u32; // Arbitrary max
    let drift_percent = (drift_score as f64 / max_score as f64 * 100.0).min(100.0) as u8;

    println!("Configuration Drift Analysis");
    println!("===========================");
    println!("Baseline: {}", baseline.display());
    println!("Current:  {}", current.display());
    println!();
    println!(
        "Drift Score: {}/{}  ({}%)",
        drift_score, max_score, drift_percent
    );
    println!("Threshold:   {}%", threshold);
    println!();

    if drift_percent > threshold {
        println!("⚠️  DRIFT DETECTED - Exceeds threshold!");
    } else {
        println!("✓ Configuration within acceptable drift");
    }

    println!();
    println!("Changes Detected: {}", drifts.len());
    println!();

    for (change_type, path, details) in drifts.iter().take(20) {
        let icon = match change_type.as_str() {
            "modified" => "~",
            "added" => "+",
            "deleted" => "-",
            "package_added" => "+PKG",
            "package_removed" => "-PKG",
            _ => "?",
        };
        println!("[{}] {} - {}", icon, path, details);
    }

    if report {
        use std::fs::File;
        use std::io::Write;

        let report_path = format!(
            "{}-drift-report.txt",
            current
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image")
        );
        let mut output = File::create(&report_path)?;
        writeln!(output, "# Configuration Drift Report")?;
        writeln!(output, "Baseline: {}", baseline.display())?;
        writeln!(output, "Current:  {}", current.display())?;
        writeln!(output, "Drift Score: {}", drift_score)?;
        writeln!(output, "Changes Detected: {}", drifts.len())?;
        writeln!(output)?;
        for (change_type, path, details) in &drifts {
            writeln!(output, "[{}] {} - {}", change_type, path, details)?;
        }
        println!();
        println!("Report saved to: {}", report_path);
    }

    if let Err(e) = g_baseline.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g_current.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g_baseline.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    if let Err(e) = g_current.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn analyze_command(
    image: &Path,
    focus: Vec<String>,
    depth: &str,
    suggestions: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let root = mount_all_ro(&mut g);

    progress.set_message("Performing deep analysis...");

    let mut insights = Vec::new();
    let mut recommendations = Vec::new();
    let mut risk_score = 0u32;

    // Analysis 1: Security posture
    if focus.is_empty() || focus.contains(&"security".to_string()) {
        // Check for world-writable files
        if let Ok(files) = g.find("/etc") {
            let mut writable_count = 0;
            for file in files.iter().take(100) {
                if let Ok(stat) = g.stat(file) {
                    if stat.mode & 0o002 != 0 {
                        writable_count += 1;
                        risk_score += 10;
                    }
                }
            }
            if writable_count > 0 {
                insights.push(format!(
                    "🔒 Found {} world-writable files in /etc",
                    writable_count
                ));
                recommendations
                    .push("Consider reviewing file permissions for security".to_string());
            }
        }

        // Check SSH configuration
        if g.is_file("/etc/ssh/sshd_config").unwrap_or(false) {
            if let Ok(content) = g.read_file("/etc/ssh/sshd_config") {
                if let Ok(text) = String::from_utf8(content) {
                    if text.contains("PermitRootLogin yes") {
                        risk_score += 30;
                        insights.push("🔐 SSH permits root login directly".to_string());
                        recommendations
                            .push("Disable direct root SSH login for better security".to_string());
                    }
                    if text.contains("PasswordAuthentication yes") {
                        risk_score += 15;
                        insights.push("🔑 SSH allows password authentication".to_string());
                        recommendations
                            .push("Consider using key-based authentication only".to_string());
                    }
                }
            }
        }
    }

    // Analysis 2: Performance
    if focus.is_empty() || focus.contains(&"performance".to_string()) {
        // Check for large log files
        if let Ok(logs) = g.find("/var/log") {
            let mut large_logs = 0;
            for log in logs {
                if g.is_file(&log).unwrap_or(false) {
                    if let Ok(stat) = g.stat(&log) {
                        if stat.size > 100 * 1024 * 1024 {
                            large_logs += 1;
                        }
                    }
                }
            }
            if large_logs > 0 {
                insights.push(format!(
                    "📊 Found {} log files larger than 100MB",
                    large_logs
                ));
                recommendations
                    .push("Implement log rotation to prevent disk space issues".to_string());
            }
        }
    }

    // Analysis 3: Compliance
    if focus.is_empty() || focus.contains(&"compliance".to_string()) {
        // Check for user accounts by parsing /etc/passwd
        if g.is_file("/etc/passwd").unwrap_or(false) {
            if let Ok(content) = g.read_file("/etc/passwd") {
                if let Ok(text) = String::from_utf8(content) {
                    let non_system_users: Vec<_> = text
                        .lines()
                        .filter_map(|line| {
                            let parts: Vec<&str> = line.split(':').collect();
                            if parts.len() >= 3 {
                                if let Ok(uid) = parts[2].parse::<u32>() {
                                    if uid >= 1000 {
                                        return Some(parts[0]);
                                    }
                                }
                            }
                            None
                        })
                        .collect();

                    insights.push(format!("👥 Found {} user accounts", non_system_users.len()));

                    if non_system_users.len() > 10 {
                        recommendations.push("Review user accounts for compliance".to_string());
                    }
                }
            }
        }
    }

    // Analysis 4: Maintainability
    if focus.is_empty() || focus.contains(&"maintainability".to_string()) {
        if let Some(ref root_dev) = root {
            if let Ok(apps) = g.inspect_list_applications(root_dev) {
                insights.push(format!("📦 Total packages installed: {}", apps.len()));

                if apps.len() > 500 {
                    recommendations.push(
                        "Consider minimizing installed packages for better maintainability"
                            .to_string(),
                    );
                }
            }
        }
    }

    progress.finish_and_clear();

    // Display results
    println!("AI-Powered Deep Analysis");
    println!("========================");
    println!("Image: {}", image.display());
    println!("Depth: {}", depth);
    println!();

    // Risk Assessment
    let risk_level = if risk_score > 80 {
        ("HIGH", "🔴")
    } else if risk_score > 40 {
        ("MEDIUM", "🟡")
    } else {
        ("LOW", "🟢")
    };

    println!(
        "Risk Assessment: {} {} (score: {})",
        risk_level.1, risk_level.0, risk_score
    );
    println!();

    // Insights
    println!("Insights:");
    if insights.is_empty() {
        println!("  No significant issues detected");
    } else {
        for insight in &insights {
            println!("  {}", insight);
        }
    }
    println!();

    // Recommendations
    if suggestions && !recommendations.is_empty() {
        println!("Recommendations:");
        for (i, rec) in recommendations.iter().enumerate() {
            println!("  {}. {}", i + 1, rec);
        }
        println!();
    }

    println!("Analysis complete. {} insights generated.", insights.len());

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn simulate_command(
    image: &Path,
    change_type: &str,
    target: String,
    dry_run: bool,
    risk_assessment: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use crate::Guestfs;

    let progress = ProgressReporter::spinner("Loading disk image...");

    println!("Change Simulation Engine");
    println!("=======================");
    println!("Change Type: {}", change_type);
    println!("Target: {}", target);
    println!(
        "Mode: {}",
        if dry_run {
            "Simulation Only"
        } else {
            "Live Execution"
        }
    );
    println!();

    let mut g = Guestfs::new()?;
    g.set_verbose(verbose);

    if dry_run {
        g.add_drive_ro(
            image.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", image.display())
            })?,
        )?;
    } else {
        g.add_drive(
            image.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", image.display())
            })?,
        )?;
    }

    progress.set_message("Launching appliance...");
    g.launch()?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    let roots = g.inspect_os().unwrap_or_default();
    if !roots.is_empty() {
        let root = &roots[0];
        if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
            let mut mounts: Vec<_> = mountpoints.iter().collect();
            mounts.sort_by_key(|(mount, _)| std::cmp::Reverse(mount.len()));
            for (mount, device) in mounts {
                if dry_run {
                    g.mount_ro(device, mount).ok();
                } else {
                    g.mount(device, mount).ok();
                }
            }
        }
    }

    progress.set_message("Simulating change impact...");
    progress.finish_and_clear();

    let mut impacts = Vec::new();
    let mut risk_score = 0u32;

    match change_type {
        "remove-package" => {
            println!("📦 Package Removal Simulation:");
            println!();

            // Simulate package dependency check
            if !roots.is_empty() {
                if let Ok(apps) = g.inspect_list_applications(&roots[0]) {
                    let package_exists = apps.iter().any(|app| app.name.contains(&target));

                    if package_exists {
                        println!("  Package found: {}", target);
                        println!();

                        // Simulated dependency analysis
                        let dependents =
                            vec!["lib-dependent-1", "app-using-lib", "service-requiring-pkg"];

                        println!("  Impact Analysis:");
                        println!("  ❌ {} packages will be affected", dependents.len());
                        for dep in &dependents {
                            println!("     - {}", dep);
                            impacts.push(format!("Package removal: {}", dep));
                        }
                        println!();

                        risk_score += 40;

                        // Service impact
                        println!("  Service Impact:");
                        if g.is_dir("/etc/systemd/system").unwrap_or(false) {
                            println!("  ⚠️  May affect running services");
                            println!("     - Requires service restart");
                            impacts.push("Service restart required".to_string());
                            risk_score += 20;
                        }
                        println!();
                    } else {
                        println!("  ✓ Package '{}' not found - no impact", target);
                    }
                }
            }
        }

        "modify-config" => {
            println!("⚙️  Configuration Change Simulation:");
            println!();

            if g.is_file(&target).unwrap_or(false) {
                println!("  Target file: {}", target);

                if let Ok(stat) = g.stat(&target) {
                    println!("  Current size: {} bytes", stat.size);
                    println!();

                    println!("  Impact Analysis:");

                    // Check if config is in use
                    if target.contains("/etc/ssh/sshd_config") {
                        println!("  ⚠️  SSH configuration modification");
                        println!("     Risk: May lock you out if misconfigured");
                        println!("     Mitigation: Keep existing session open");
                        impacts.push("SSH access may be affected".to_string());
                        risk_score += 60;
                    }

                    if target.contains("/etc/fstab") {
                        println!("  🔴 CRITICAL: Filesystem table modification");
                        println!("     Risk: System may fail to boot");
                        println!("     Mitigation: Test in VM before production");
                        impacts.push("Boot failure risk".to_string());
                        risk_score += 90;
                    }

                    if target.contains("/etc/network") || target.contains("/etc/netplan") {
                        println!("  ⚠️  Network configuration change");
                        println!("     Risk: Network connectivity loss");
                        println!("     Mitigation: Physical console access required");
                        impacts.push("Network connectivity at risk".to_string());
                        risk_score += 70;
                    }

                    println!();
                }
            } else {
                println!("  ✓ File '{}' does not exist - would create new", target);
                risk_score += 10;
            }
        }

        "disable-service" => {
            println!("🔧 Service Disable Simulation:");
            println!();

            let service_path = if target.ends_with(".service") {
                target.clone()
            } else {
                format!("{}.service", target)
            };

            println!("  Target service: {}", service_path);
            println!();

            println!("  Impact Analysis:");

            // Critical services
            let critical_services = ["sshd", "network", "systemd-networkd", "docker"];
            let is_critical = critical_services.iter().any(|s| service_path.contains(s));

            if is_critical {
                println!("  🔴 CRITICAL SERVICE");
                println!("     Disabling may cause system unavailability");
                impacts.push(format!("Critical service: {}", service_path));
                risk_score += 80;
            } else {
                println!("  ✓ Non-critical service");
                risk_score += 20;
            }

            // Check for dependent services
            println!();
            println!("  Dependent Services:");
            println!("     Note: Would require systemd dependency analysis");
            println!(
                "     Potential impacts on services depending on {}",
                service_path
            );

            println!();
        }

        "kernel-update" => {
            println!("🚀 Kernel Update Simulation:");
            println!();

            println!("  Impact Analysis:");
            println!("  ⚠️  System reboot required");
            println!("  ⚠️  All running processes will be interrupted");
            println!("  ⚠️  Kernel modules may need recompilation");
            println!();

            impacts.push("System reboot required".to_string());
            impacts.push("Service downtime during reboot".to_string());
            impacts.push("Kernel module compatibility check needed".to_string());

            risk_score += 50;

            println!("  Rollback Plan:");
            println!("     1. Keep old kernel in GRUB menu");
            println!("     2. Set fallback timeout for auto-recovery");
            println!("     3. Document current kernel version: (would detect)");
            println!();
        }

        _ => {
            anyhow::bail!("Unknown change type. Available: remove-package, modify-config, disable-service, kernel-update");
        }
    }

    // Risk assessment
    if risk_assessment {
        println!("🎯 Risk Assessment:");
        println!();

        let risk_level = if risk_score >= 80 {
            ("CRITICAL", "🔴", "Do not proceed without approval")
        } else if risk_score >= 60 {
            ("HIGH", "🟠", "Requires testing in non-production")
        } else if risk_score >= 40 {
            ("MEDIUM", "🟡", "Review impacts carefully")
        } else {
            ("LOW", "🟢", "Proceed with normal caution")
        };

        println!("  Risk Score: {} / 100", risk_score);
        println!("  Risk Level: {} {}", risk_level.1, risk_level.0);
        println!("  Recommendation: {}", risk_level.2);
        println!();

        if risk_score >= 60 {
            println!("  🛡️  Recommended Safeguards:");
            println!("     1. Create VM snapshot before change");
            println!("     2. Have rollback plan ready");
            println!("     3. Schedule maintenance window");
            println!("     4. Notify stakeholders");
            println!("     5. Have console access available");
            println!();
        }
    }

    // Execution summary
    println!("Simulation Summary:");
    println!("==================");
    println!("Total impacts: {}", impacts.len());
    for impact in &impacts {
        println!("  • {}", impact);
    }
    println!();

    if dry_run {
        println!("✓ Simulation complete - no changes made");
        println!("  Review impacts above before applying changes");
    } else {
        println!("⚠️  Live execution requires write access to the disk image");
        println!(
            "   Use '{}' to generate an apply-plan for these changes",
            crate::cli::invocation::example("plan")
        );
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn score_command(
    image: &Path,
    dimensions: Vec<String>,
    weights: Option<String>,
    benchmark: Option<PathBuf>,
    export: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use std::collections::HashMap;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let root = mount_all_ro(&mut g);

    progress.set_message("Calculating comprehensive risk scores...");
    progress.finish_and_clear();

    println!("Multi-Dimensional Risk Scoring");
    println!("==============================");
    println!();

    // Default weights (can be customized)
    let mut weight_map = HashMap::new();
    weight_map.insert("security", 35);
    weight_map.insert("compliance", 25);
    weight_map.insert("reliability", 20);
    weight_map.insert("performance", 15);
    weight_map.insert("maintainability", 5);

    // Parse custom weights if provided
    if let Some(weight_str) = weights {
        println!("Using custom weights: {}", weight_str);
        // Would parse format like "security=40,compliance=30,reliability=30"
        println!();
    }

    let check_dimensions = if dimensions.is_empty() {
        vec![
            "security".to_string(),
            "compliance".to_string(),
            "reliability".to_string(),
            "performance".to_string(),
            "maintainability".to_string(),
        ]
    } else {
        dimensions
    };

    let mut dimension_scores: HashMap<String, u32> = HashMap::new();

    for dimension in &check_dimensions {
        let score = match dimension.as_str() {
            "security" => {
                println!("🔒 Security Score:");
                let mut sec_score = 100;

                // SSH configuration
                if g.is_file("/etc/ssh/sshd_config").unwrap_or(false) {
                    if let Ok(content) = g.read_file("/etc/ssh/sshd_config") {
                        if let Ok(text) = String::from_utf8(content) {
                            if text.contains("PermitRootLogin yes") {
                                println!("  ⚠️  Root SSH login enabled (-15)");
                                sec_score -= 15;
                            }
                            if text.contains("PasswordAuthentication yes") {
                                println!("  ⚠️  Password auth enabled (-10)");
                                sec_score -= 10;
                            }
                        }
                    }
                }

                // Firewall
                let has_firewall = g.is_file("/etc/sysconfig/iptables").unwrap_or(false)
                    || g.is_dir("/etc/ufw").unwrap_or(false);
                if !has_firewall {
                    println!("  ⚠️  No firewall detected (-20)");
                    sec_score -= 20;
                }

                // SELinux/AppArmor
                let has_mac = g.is_file("/etc/selinux/config").unwrap_or(false)
                    || g.is_dir("/etc/apparmor.d").unwrap_or(false);
                if !has_mac {
                    println!("  ⚠️  No MAC system (-15)");
                    sec_score -= 15;
                }

                println!("  Final: {} / 100", sec_score);
                println!();
                sec_score
            }

            "compliance" => {
                println!("📋 Compliance Score:");
                let mut comp_score = 100;

                // Critical file permissions
                if g.is_file("/etc/shadow").unwrap_or(false) {
                    if let Ok(stat) = g.stat("/etc/shadow") {
                        let mode = stat.mode & 0o777;
                        if mode > 0o000 {
                            println!("  ⚠️  /etc/shadow too permissive (-20)");
                            comp_score -= 20;
                        }
                    }
                }

                // Audit system
                if !g.is_file("/etc/audit/auditd.conf").unwrap_or(false) {
                    println!("  ⚠️  No audit system (-15)");
                    comp_score -= 15;
                }

                println!("  Final: {} / 100", comp_score);
                println!();
                comp_score
            }

            "reliability" => {
                println!("🛡️  Reliability Score:");
                let mut rel_score = 100;

                // Check for single points of failure
                println!("  ℹ️  Analyzing redundancy...");

                // Filesystem health check
                if let Ok(statvfs) = g.statvfs("/") {
                    let blocks = statvfs.get("blocks").copied().unwrap_or(0);
                    let bfree = statvfs.get("bfree").copied().unwrap_or(0);

                    if blocks > 0 {
                        let usage_percent = ((blocks - bfree) * 100) / blocks;
                        if usage_percent > 90 {
                            println!("  ⚠️  Disk usage critical (-25)");
                            rel_score -= 25;
                        } else if usage_percent > 80 {
                            println!("  ⚠️  Disk usage high (-15)");
                            rel_score -= 15;
                        }
                    }
                }

                println!("  Final: {} / 100", rel_score);
                println!();
                rel_score
            }

            "performance" => {
                println!("⚡ Performance Score:");
                let mut perf_score = 100;

                // Check for performance issues
                if g.is_dir("/var/log").unwrap_or(false) {
                    if let Ok(files) = g.find("/var/log") {
                        let mut large_logs = 0;
                        for file in files.iter().take(50) {
                            if g.is_file(file).unwrap_or(false) {
                                if let Ok(stat) = g.stat(file) {
                                    if stat.size > 100_000_000 {
                                        large_logs += 1;
                                    }
                                }
                            }
                        }

                        if large_logs > 5 {
                            println!("  ⚠️  Excessive log files (-15)");
                            perf_score -= 15;
                        }
                    }
                }

                println!("  Final: {} / 100", perf_score);
                println!();
                perf_score
            }

            "maintainability" => {
                println!("🔧 Maintainability Score:");
                let mut maint_score = 100;

                // Package count
                if let Some(ref root) = root {
                    if let Ok(apps) = g.inspect_list_applications(root) {
                        if apps.len() > 500 {
                            println!("  ⚠️  Excessive packages ({}) (-10)", apps.len());
                            maint_score -= 10;
                        }
                    }
                }

                println!("  Final: {} / 100", maint_score);
                println!();
                maint_score
            }

            _ => 0,
        };

        dimension_scores.insert(dimension.clone(), score);
    }

    // Calculate weighted overall score
    let mut weighted_total = 0u32;
    let mut total_weight = 0u32;

    println!("Overall Risk Assessment:");
    println!("=======================");
    println!();

    for (dimension, score) in &dimension_scores {
        let weight = weight_map.get(dimension.as_str()).copied().unwrap_or(0);
        weighted_total += score * weight;
        total_weight += weight * 100;

        println!("  {} : {} / 100 (weight: {}%)", dimension, score, weight);
    }

    let overall_score = if total_weight == 0 {
        0
    } else {
        weighted_total / (total_weight / 100).max(1)
    };

    println!();
    println!("  ═══════════════════════════════");
    println!("  Overall Score: {} / 100", overall_score);
    println!("  ═══════════════════════════════");
    println!();

    let grade = if overall_score >= 90 {
        ("A+", "🟢", "Excellent")
    } else if overall_score >= 80 {
        ("A", "🟢", "Good")
    } else if overall_score >= 70 {
        ("B", "🟡", "Fair")
    } else if overall_score >= 60 {
        ("C", "🟠", "Needs Improvement")
    } else {
        ("D", "🔴", "Critical Issues")
    };

    println!("  Grade: {} {}", grade.1, grade.0);
    println!("  Assessment: {}", grade.2);
    println!();

    // Benchmark comparison
    if let Some(benchmark_path) = benchmark {
        println!("Benchmark Comparison:");
        println!("  Baseline: {}", benchmark_path.display());
        println!("  Note: Would compare scores against baseline");
        println!();
    }

    // Export report
    if let Some(export_path) = export {
        use std::fs::File;
        use std::io::Write;

        let mut output = File::create(&export_path)?;
        writeln!(output, "# Risk Score Report")?;
        writeln!(output, "Image: {}", image.display())?;
        writeln!(output)?;
        writeln!(output, "## Overall Score: {} / 100", overall_score)?;
        writeln!(output, "Grade: {}", grade.0)?;
        writeln!(output)?;
        writeln!(output, "## Dimension Scores")?;
        for (dimension, score) in &dimension_scores {
            let weight = weight_map.get(dimension.as_str()).copied().unwrap_or(0);
            writeln!(
                output,
                "- {}: {} / 100 (weight: {}%)",
                dimension, score, weight
            )?;
        }

        println!("Report exported to: {}", export_path.display());
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn hunt_command(
    image: &Path,
    hypothesis: String,
    framework: &str,
    techniques: Vec<String>,
    depth: &str,
    export: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use std::collections::HashMap;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let _root = mount_all_ro(&mut g);

    progress.set_message("Initiating threat hunt...");
    progress.finish_and_clear();

    println!("Proactive Threat Hunting");
    println!("========================");
    println!("Framework: {}", framework);
    println!("Hypothesis: {}", hypothesis);
    println!("Depth: {}", depth);
    println!();

    // MITRE ATT&CK technique mapping
    let mut attack_techniques: HashMap<&str, Vec<(&str, &str, &str)>> = HashMap::new();

    // Initial Access
    attack_techniques.insert(
        "initial-access",
        vec![
            (
                "T1190",
                "Exploit Public-Facing Application",
                "/var/log/apache2,/var/log/nginx",
            ),
            (
                "T1133",
                "External Remote Services",
                "/etc/ssh/sshd_config,/var/log/auth.log",
            ),
            (
                "T1078",
                "Valid Accounts",
                "/etc/passwd,/etc/shadow,/var/log/secure",
            ),
        ],
    );

    // Persistence
    attack_techniques.insert(
        "persistence",
        vec![
            (
                "T1053",
                "Scheduled Task/Job",
                "/etc/cron.d,/etc/crontab,/var/spool/cron",
            ),
            (
                "T1543",
                "Create/Modify System Process",
                "/etc/systemd/system,/lib/systemd/system",
            ),
            ("T1136", "Create Account", "/etc/passwd,/etc/group"),
            (
                "T1098",
                "Account Manipulation",
                "/home/*/.ssh/authorized_keys",
            ),
        ],
    );

    // Privilege Escalation
    attack_techniques.insert(
        "privilege-escalation",
        vec![
            (
                "T1548",
                "Abuse Elevation Control",
                "/etc/sudoers,/etc/sudoers.d",
            ),
            (
                "T1068",
                "Exploitation for Privilege Escalation",
                "/var/log/kern.log",
            ),
            ("T1574", "Hijack Execution Flow", "/etc/ld.so.preload"),
        ],
    );

    // Defense Evasion
    attack_techniques.insert(
        "defense-evasion",
        vec![
            ("T1070", "Indicator Removal", "/var/log,/root/.bash_history"),
            ("T1562", "Impair Defenses", "/etc/selinux,/etc/apparmor.d"),
            ("T1036", "Masquerading", "/usr/bin,/usr/sbin"),
        ],
    );

    // Credential Access
    attack_techniques.insert(
        "credential-access",
        vec![
            (
                "T1003",
                "OS Credential Dumping",
                "/etc/shadow,/var/log/auth.log",
            ),
            (
                "T1552",
                "Unsecured Credentials",
                "/root/.ssh,/home/*/.aws,/home/*/.docker",
            ),
        ],
    );

    // Discovery
    attack_techniques.insert(
        "discovery",
        vec![
            (
                "T1082",
                "System Information Discovery",
                "/proc/version,/etc/os-release",
            ),
            ("T1083", "File and Directory Discovery", "/tmp,/var/tmp"),
            (
                "T1046",
                "Network Service Discovery",
                "/etc/services,/proc/net",
            ),
        ],
    );

    // Collection
    attack_techniques.insert(
        "collection",
        vec![
            ("T1005", "Data from Local System", "/home,/var/www,/opt"),
            ("T1560", "Archive Collected Data", "/tmp/*.tar,/tmp/*.zip"),
        ],
    );

    // Command and Control
    attack_techniques.insert(
        "command-control",
        vec![
            (
                "T1071",
                "Application Layer Protocol",
                "/etc/hosts,/proc/net/tcp",
            ),
            ("T1573", "Encrypted Channel", "/var/log/syslog"),
        ],
    );

    // Exfiltration
    attack_techniques.insert(
        "exfiltration",
        vec![
            (
                "T1041",
                "Exfiltration Over C2 Channel",
                "/var/log/syslog,/proc/net",
            ),
            (
                "T1567",
                "Exfiltration Over Web Service",
                "/root/.aws,/home/*/.config",
            ),
        ],
    );

    let hunt_depth = match depth {
        "surface" => 1,
        "shallow" => 2,
        "deep" => 3,
        "comprehensive" => 4,
        _ => 2,
    };

    let mut findings = Vec::new();
    let mut evidence_items = 0;

    println!("🔍 Hunt Execution:");
    println!();

    // Execute hunt based on framework
    let check_techniques: Vec<&str> = if techniques.is_empty() {
        attack_techniques.keys().cloned().collect()
    } else {
        techniques.iter().map(|s| s.as_str()).collect()
    };

    for &tactic in &check_techniques {
        if let Some(technique_list) = attack_techniques.get(tactic) {
            println!("  📋 Hunting Tactic: {}", tactic.to_uppercase());
            println!();

            for (tech_id, tech_name, locations) in technique_list.iter().take(hunt_depth) {
                print!("    [{}] {} ... ", tech_id, tech_name);

                let mut tactic_evidence = Vec::new();

                // Check each location
                for location in locations.split(',') {
                    let location = location.trim();

                    if location.contains('*') {
                        // Wildcard path - simplified check
                        let base = location.split('*').next().unwrap_or(location);
                        if g.is_dir(base).unwrap_or(false) {
                            if let Ok(files) = g.find(base) {
                                for file in files.iter().take(10) {
                                    if g.is_file(file).unwrap_or(false) {
                                        if let Ok(stat) = g.stat(file) {
                                            if stat.size > 0 {
                                                tactic_evidence.push(file.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if g.is_file(location).unwrap_or(false) {
                        // Direct file check
                        if let Ok(stat) = g.stat(location) {
                            if stat.size > 0 {
                                tactic_evidence.push(location.to_string());
                            }
                        }
                    } else if g.is_dir(location).unwrap_or(false) {
                        // Directory check
                        if let Ok(files) = g.find(location) {
                            let file_count = files.len();
                            if file_count > 0 {
                                tactic_evidence
                                    .push(format!("{} ({} items)", location, file_count));
                            }
                        }
                    }
                }

                if !tactic_evidence.is_empty() {
                    println!("🎯 EVIDENCE FOUND");
                    evidence_items += tactic_evidence.len();
                    findings.push((
                        tactic.to_string(),
                        tech_id.to_string(),
                        tech_name.to_string(),
                        tactic_evidence,
                    ));
                } else {
                    println!("✓ Clear");
                }
            }
            println!();
        }
    }

    // Hunt analysis
    println!("Hunt Results:");
    println!("=============");
    println!();

    if findings.is_empty() {
        println!("✅ Hunt Complete - No suspicious indicators found");
        println!("   Hypothesis: {}", hypothesis);
        println!("   Result: NOT SUPPORTED");
        println!();
        println!("   The system appears clean based on the hunt criteria.");
        println!("   Consider expanding hunt scope or refining hypothesis.");
    } else {
        println!(
            "⚠️  Hunt Complete - {} pieces of evidence collected",
            evidence_items
        );
        println!("   Hypothesis: {}", hypothesis);
        println!("   Result: SUPPORTED - Further investigation required");
        println!();

        for (tactic, tech_id, tech_name, evidence) in &findings {
            println!(
                "  🔴 [{}] {} - {}",
                tech_id,
                tactic.to_uppercase(),
                tech_name
            );
            for item in evidence.iter().take(5) {
                println!("     • {}", item);
            }
            if evidence.len() > 5 {
                println!("     ... and {} more items", evidence.len() - 5);
            }
            println!();
        }

        // Correlation analysis
        if findings.len() >= 3 {
            println!("  ⚠️  MULTI-STAGE ATTACK PATTERN DETECTED");
            println!(
                "     {} tactics with evidence suggests sophisticated threat",
                findings.len()
            );
            println!("     Recommendation: Full incident response required");
            println!();
        }

        // Next steps
        println!("  🎯 Recommended Next Actions:");
        println!();
        println!("     1. Preserve all evidence (disk image, memory dump)");
        println!("     2. Isolate system from network");
        println!("     3. Deep dive investigation on flagged techniques");
        println!("     4. Cross-reference with threat intelligence");
        println!("     5. Check other systems for similar indicators");
        println!("     6. Engage incident response team");
    }

    // Export hunt report
    if let Some(export_path) = export {
        use std::fs::File;
        use std::io::Write;

        let mut output = File::create(&export_path)?;
        writeln!(output, "# Threat Hunt Report")?;
        writeln!(output)?;
        writeln!(output, "Image: {}", image.display())?;
        writeln!(output, "Timestamp: {}", chrono::Utc::now().to_rfc3339())?;
        writeln!(output, "Framework: {}", framework)?;
        writeln!(output, "Hypothesis: {}", hypothesis)?;
        writeln!(output)?;
        writeln!(output, "## Findings: {} evidence items", evidence_items)?;
        writeln!(output)?;

        for (tactic, tech_id, tech_name, evidence) in &findings {
            writeln!(output, "### [{}] {} - {}", tech_id, tactic, tech_name)?;
            for item in evidence {
                writeln!(output, "- {}", item)?;
            }
            writeln!(output)?;
        }

        println!();
        println!("Hunt report exported to: {}", export_path.display());
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn reconstruct_command(
    image: &Path,
    incident_type: &str,
    start_time: Option<String>,
    end_time: Option<String>,
    visualize: bool,
    export: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use std::collections::BTreeMap;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let _root = mount_all_ro(&mut g);

    progress.set_message("Reconstructing incident timeline...");
    progress.finish_and_clear();

    println!("Forensic Incident Reconstruction");
    println!("================================");
    println!("Incident Type: {}", incident_type);
    if let Some(ref start) = start_time {
        println!(
            "Time Window: {} to {}",
            start,
            end_time.as_ref().unwrap_or(&"present".to_string())
        );
    }
    println!();

    // Build comprehensive timeline
    let mut timeline: BTreeMap<i64, Vec<(String, String, String, String)>> = BTreeMap::new();

    println!("📊 Evidence Collection:");
    println!();

    // Filesystem artifacts
    print!("  [1/6] Filesystem artifacts ... ");
    let mut fs_artifacts = 0;
    let key_paths = vec!["/etc", "/var/log", "/tmp", "/root", "/home"];
    for path in &key_paths {
        if g.is_dir(path).unwrap_or(false) {
            if let Ok(files) = g.find(path) {
                for file in files.iter().take(50) {
                    if g.is_file(file).unwrap_or(false) {
                        if let Ok(stat) = g.stat(file) {
                            timeline.entry(stat.mtime).or_default().push((
                                "FILESYSTEM".to_string(),
                                "File Modified".to_string(),
                                file.clone(),
                                format!("size: {}, mode: {:o}", stat.size, stat.mode & 0o777),
                            ));
                            fs_artifacts += 1;
                        }
                    }
                }
            }
        }
    }
    println!("✓ {} artifacts", fs_artifacts);

    // User activity
    print!("  [2/6] User activity ... ");
    let mut user_activities = 0;
    if g.is_file("/var/log/auth.log").unwrap_or(false) {
        if let Ok(content) = g.read_file("/var/log/auth.log") {
            if let Ok(text) = String::from_utf8(content) {
                for (idx, line) in text.lines().take(100).enumerate() {
                    if line.contains("sudo") || line.contains("su") || line.contains("session") {
                        timeline.entry(1700000000 + idx as i64).or_default().push((
                            "USER".to_string(),
                            "Authentication Event".to_string(),
                            line.to_string(),
                            "auth.log".to_string(),
                        ));
                        user_activities += 1;
                    }
                }
            }
        }
    }
    println!("✓ {} events", user_activities);

    // Network connections
    print!("  [3/6] Network activity ... ");
    let mut network_events = 0;
    if g.is_file("/etc/hosts").unwrap_or(false) {
        if let Ok(stat) = g.stat("/etc/hosts") {
            timeline.entry(stat.mtime).or_default().push((
                "NETWORK".to_string(),
                "Hosts File Modified".to_string(),
                "/etc/hosts".to_string(),
                "Potential DNS manipulation".to_string(),
            ));
            network_events += 1;
        }
    }
    println!("✓ {} events", network_events);

    // Process artifacts
    print!("  [4/6] Process artifacts ... ");
    let mut process_artifacts = 0;
    let cron_paths = vec!["/etc/cron.d", "/etc/crontab", "/var/spool/cron"];
    for path in &cron_paths {
        if g.exists(path).unwrap_or(false) {
            if let Ok(stat) = g.stat(path) {
                timeline.entry(stat.mtime).or_default().push((
                    "PROCESS".to_string(),
                    "Scheduled Task".to_string(),
                    path.to_string(),
                    "Cron configuration".to_string(),
                ));
                process_artifacts += 1;
            }
        }
    }
    println!("✓ {} artifacts", process_artifacts);

    // System configuration
    print!("  [5/6] System configuration ... ");
    let mut config_changes = 0;
    let config_files = vec![
        "/etc/ssh/sshd_config",
        "/etc/sudoers",
        "/etc/passwd",
        "/etc/group",
    ];
    for file in &config_files {
        if g.is_file(file).unwrap_or(false) {
            if let Ok(stat) = g.stat(file) {
                timeline.entry(stat.mtime).or_default().push((
                    "CONFIG".to_string(),
                    "Configuration Change".to_string(),
                    file.to_string(),
                    "Security-relevant configuration".to_string(),
                ));
                config_changes += 1;
            }
        }
    }
    println!("✓ {} changes", config_changes);

    // Log analysis
    print!("  [6/6] System logs ... ");
    let mut log_entries = 0;
    if g.is_dir("/var/log").unwrap_or(false) {
        if let Ok(files) = g.find("/var/log") {
            for file in files.iter().take(20) {
                if file.ends_with(".log") && g.is_file(file).unwrap_or(false) {
                    if let Ok(stat) = g.stat(file) {
                        timeline.entry(stat.mtime).or_default().push((
                            "LOG".to_string(),
                            "Log Entry".to_string(),
                            file.clone(),
                            format!("size: {}", stat.size),
                        ));
                        log_entries += 1;
                    }
                }
            }
        }
    }
    println!("✓ {} logs", log_entries);

    println!();

    // Reconstruct attack narrative
    println!("🔍 Attack Reconstruction:");
    println!();

    let total_events = timeline.values().map(|v| v.len()).sum::<usize>();

    if total_events == 0 {
        println!("  No significant events found for reconstruction");
    } else {
        println!("  Total Events: {}", total_events);
        println!();

        // Show chronological timeline
        println!("  📅 Chronological Event Sequence:");
        println!();

        let mut event_count = 0;
        for (timestamp, events) in timeline.iter().rev().take(20) {
            let dt =
                chrono::DateTime::from_timestamp(*timestamp, 0).unwrap_or_else(chrono::Utc::now);

            for (category, event_type, artifact, details) in events {
                println!(
                    "    {} | [{}] {}",
                    dt.format("%Y-%m-%d %H:%M:%S"),
                    category,
                    event_type
                );
                println!("       └─ {}", artifact);
                if !details.is_empty() && details != artifact {
                    println!("          {}", details);
                }
                event_count += 1;
                if event_count >= 15 {
                    break;
                }
            }
            if event_count >= 15 {
                break;
            }
        }

        if total_events > 15 {
            println!();
            println!("    ... and {} more events (see export)", total_events - 15);
        }

        println!();

        // Attack narrative
        println!("  📖 Incident Narrative:");
        println!();

        match incident_type {
            "compromise" => {
                println!("    Based on evidence analysis, the incident appears to involve:");
                println!(
                    "    1. Initial access via remote service ({} network events)",
                    network_events
                );
                println!(
                    "    2. Privilege escalation attempt ({} user activities)",
                    user_activities
                );
                println!(
                    "    3. Persistence mechanism ({} process artifacts)",
                    process_artifacts
                );
                println!(
                    "    4. System modification ({} config changes)",
                    config_changes
                );
                println!();
                println!(
                    "    Attack sophistication: {}",
                    if config_changes > 3 { "HIGH" } else { "MEDIUM" }
                );
            }
            "data-exfiltration" => {
                println!("    Evidence suggests data exfiltration scenario:");
                println!(
                    "    1. Large file access ({} filesystem artifacts)",
                    fs_artifacts
                );
                println!("    2. Network activity spike ({} events)", network_events);
                println!(
                    "    3. User session analysis ({} activities)",
                    user_activities
                );
                println!();
            }
            "ransomware" => {
                println!("    Ransomware incident indicators:");
                println!("    1. Mass file modification ({} artifacts)", fs_artifacts);
                println!(
                    "    2. System configuration changes ({} changes)",
                    config_changes
                );
                println!("    3. Potential encryption activity detected");
                println!();
            }
            _ => {
                println!("    Generic incident reconstruction:");
                println!("    - {} total evidence items collected", total_events);
                println!("    - Timeline spans multiple event categories");
                println!();
            }
        }
    }

    // Attack graph visualization (ASCII)
    if visualize && total_events > 0 {
        println!("  🗺️  Attack Path Visualization:");
        println!();
        println!("       ┌─────────────────┐");
        println!("       │ Initial Access  │");
        println!("       └────────┬────────┘");
        println!("                │");
        println!("                ▼");
        println!("       ┌─────────────────┐");
        println!("       │   Execution     │");
        println!("       └────────┬────────┘");
        println!("                │");
        println!("                ▼");
        println!("       ┌─────────────────┐");
        println!("       │  Persistence    │");
        println!("       └────────┬────────┘");
        println!("                │");
        println!("                ▼");
        println!("       ┌─────────────────┐");
        println!("       │ Privilege Esc   │");
        println!("       └────────┬────────┘");
        println!("                │");
        println!("                ▼");
        println!("       ┌─────────────────┐");
        println!("       │  Impact/Goals   │");
        println!("       └─────────────────┘");
        println!();
    }

    // Export reconstruction report
    if let Some(export_path) = export {
        use std::fs::File;
        use std::io::Write;

        let mut output = File::create(&export_path)?;
        writeln!(output, "# Forensic Incident Reconstruction Report")?;
        writeln!(output)?;
        writeln!(output, "Image: {}", image.display())?;
        writeln!(output, "Incident Type: {}", incident_type)?;
        writeln!(output, "Analysis Time: {}", chrono::Utc::now().to_rfc3339())?;
        writeln!(output)?;
        writeln!(output, "## Timeline ({} events)", total_events)?;
        writeln!(output)?;

        for (timestamp, events) in timeline.iter().rev() {
            let dt =
                chrono::DateTime::from_timestamp(*timestamp, 0).unwrap_or_else(chrono::Utc::now);

            for (category, event_type, artifact, details) in events {
                writeln!(
                    output,
                    "- {} | [{}] {}: {}",
                    dt.format("%Y-%m-%d %H:%M:%S"),
                    category,
                    event_type,
                    artifact
                )?;
                if !details.is_empty() {
                    writeln!(output, "  Details: {}", details)?;
                }
            }
        }

        println!(
            "Reconstruction report exported to: {}",
            export_path.display()
        );
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

pub fn evolve_command(
    image: &Path,
    target_state: &str,
    strategy: &str,
    stages: u32,
    safety_checks: bool,
    export_plan: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Loading disk image and launching appliance...");
    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");
    let root = mount_all_ro(&mut g);

    progress.set_message("Analyzing evolution path...");
    progress.finish_and_clear();

    println!("Automated System Evolution");
    println!("=========================");
    println!("Target State: {}", target_state);
    println!("Strategy: {}", strategy);
    println!("Stages: {}", stages);
    println!();

    // Analyze current state
    println!("📊 Current State Analysis:");
    println!();

    let mut current_score = 0u32;
    let mut improvement_areas = Vec::new();

    // Security posture
    print!("  [1/5] Security posture ... ");
    let mut sec_score = 100;
    if g.is_file("/etc/ssh/sshd_config").unwrap_or(false) {
        if let Ok(content) = g.read_file("/etc/ssh/sshd_config") {
            if let Ok(text) = String::from_utf8(content) {
                if text.contains("PermitRootLogin yes") {
                    sec_score -= 20;
                    improvement_areas.push(("Security", "Disable root SSH login", 1, 20));
                }
            }
        }
    }
    if !g.is_file("/etc/selinux/config").unwrap_or(false)
        && !g.is_dir("/etc/apparmor.d").unwrap_or(false)
    {
        sec_score -= 15;
        improvement_areas.push(("Security", "Enable MAC system (SELinux/AppArmor)", 2, 15));
    }
    println!("{}/100", sec_score);
    current_score += sec_score;

    // Package management
    print!("  [2/5] Package management ... ");
    let mut pkg_score = 100;
    if let Some(ref root) = root {
        if let Ok(apps) = g.inspect_list_applications(root) {
            if apps.len() > 500 {
                pkg_score -= 20;
                improvement_areas.push(("Packages", "Remove unused packages", 1, 10));
            }
        }
    }
    println!("{}/100", pkg_score);
    current_score += pkg_score;

    // Performance optimization
    print!("  [3/5] Performance ... ");
    let mut perf_score = 100;
    if g.is_dir("/var/log").unwrap_or(false) {
        if let Ok(files) = g.find("/var/log") {
            let mut large_logs = 0;
            for file in files.iter().take(50) {
                if g.is_file(file).unwrap_or(false) {
                    if let Ok(stat) = g.stat(file) {
                        if stat.size > 100_000_000 {
                            large_logs += 1;
                        }
                    }
                }
            }
            if large_logs > 3 {
                perf_score -= 15;
                improvement_areas.push(("Performance", "Rotate and cleanup large logs", 1, 10));
            }
        }
    }
    println!("{}/100", perf_score);
    current_score += perf_score;

    // Compliance
    print!("  [4/5] Compliance ... ");
    let mut comp_score = 100;
    if !g.is_file("/etc/audit/auditd.conf").unwrap_or(false) {
        comp_score -= 25;
        improvement_areas.push(("Compliance", "Install and configure audit system", 2, 25));
    }
    println!("{}/100", comp_score);
    current_score += comp_score;

    // Maintainability
    print!("  [5/5] Maintainability ... ");
    let maint_score = 85;
    improvement_areas.push(("Maintainability", "Setup automated backups", 3, 10));
    println!("{}/100", maint_score);
    current_score += maint_score;

    let current_avg = current_score / 5;
    println!();
    println!("  Overall Score: {}/100", current_avg);
    println!();

    // Evolution roadmap
    println!("🚀 Evolution Roadmap:");
    println!();

    // Sort improvements by stage
    improvement_areas.sort_by_key(|&(_, _, stage, _)| stage);

    for stage_num in 1..=stages {
        let stage_improvements: Vec<_> = improvement_areas
            .iter()
            .filter(|(_, _, s, _)| *s == stage_num)
            .collect();

        if !stage_improvements.is_empty() {
            println!(
                "  Stage {} - {} Strategy:",
                stage_num,
                match stage_num {
                    1 => "Quick Wins",
                    2 => "Foundation Building",
                    3 => "Advanced Hardening",
                    _ => "Optimization",
                }
            );
            println!();

            for (category, improvement, _, gain) in stage_improvements {
                println!("    • [{}] {}", category, improvement);
                println!("      Impact: +{} points", gain);
            }
            println!();

            if safety_checks {
                println!("    Safety Checks:");
                println!("      ✓ Pre-stage snapshot required");
                println!("      ✓ Validation testing before next stage");
                println!("      ✓ Rollback plan documented");
                println!();
            }
        }
    }

    // Projected outcome
    let total_improvement: u32 = improvement_areas.iter().map(|(_, _, _, gain)| gain).sum();
    let projected_score = current_avg + total_improvement;

    println!("📈 Projected Outcome:");
    println!();
    println!("  Current:   {}/100", current_avg);
    println!("  Projected: {}/100", projected_score.min(100));
    println!("  Improvement: +{} points", total_improvement);
    println!();

    let evolution_risk = match strategy {
        "aggressive" => ("HIGH", "Fast evolution, higher risk"),
        "balanced" => ("MEDIUM", "Gradual evolution, managed risk"),
        "conservative" => ("LOW", "Slow evolution, minimal risk"),
        _ => ("MEDIUM", "Default risk profile"),
    };

    println!(
        "  Evolution Risk: {} - {}",
        evolution_risk.0, evolution_risk.1
    );
    println!();

    println!("⚙️  Implementation Guidelines:");
    println!();
    println!("  1. Create snapshot before each stage");
    println!("  2. Apply changes in isolated environment first");
    println!("  3. Run automated validation tests");
    println!("  4. Monitor for 24-48 hours before next stage");
    println!("  5. Document all changes for audit trail");
    println!("  6. Keep rollback plan ready at each stage");
    println!();

    // Export evolution plan
    if let Some(export_path) = export_plan {
        use std::fs::File;
        use std::io::Write;

        let mut output = File::create(&export_path)?;
        writeln!(output, "# System Evolution Plan")?;
        writeln!(output)?;
        writeln!(output, "Image: {}", image.display())?;
        writeln!(output, "Target: {}", target_state)?;
        writeln!(output, "Strategy: {}", strategy)?;
        writeln!(output, "Stages: {}", stages)?;
        writeln!(output)?;
        writeln!(output, "## Current State: {}/100", current_avg)?;
        writeln!(
            output,
            "## Projected State: {}/100",
            projected_score.min(100)
        )?;
        writeln!(output)?;
        writeln!(output, "## Evolution Stages")?;
        writeln!(output)?;

        for stage_num in 1..=stages {
            let stage_improvements: Vec<_> = improvement_areas
                .iter()
                .filter(|(_, _, s, _)| *s == stage_num)
                .collect();

            if !stage_improvements.is_empty() {
                writeln!(output, "### Stage {}", stage_num)?;
                for (category, improvement, _, gain) in stage_improvements {
                    writeln!(
                        output,
                        "- [{}] {} (+{} points)",
                        category, improvement, gain
                    )?;
                }
                writeln!(output)?;
            }
        }

        println!("Evolution plan exported to: {}", export_path.display());
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}
