// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Disk forensics and analysis
//!
//! This example demonstrates forensic analysis of a disk image:
//! - File search and pattern matching
//! - Deleted file recovery concepts
//! - Checksum verification
//! - Timeline analysis
//! - Suspicious file detection

use guestkit::Guestfs;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Disk Forensics Example ===\n");

    // For this example, we'd need a disk image
    // You can use one of the other examples to create one first

    let disk_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/path/to/disk.img".to_string());

    let mut g = Guestfs::new()?;
    g.set_verbose(false);

    println!("Opening disk image: {}", disk_path);
    g.add_drive_ro(&disk_path)?;
    g.launch()?;

    // Collect forensic evidence
    let mut evidence = ForensicEvidence::new();

    // 1. Scan filesystems
    println!("\n[1] Scanning filesystems...");
    let filesystems = g.list_filesystems()?;
    for (device, fstype) in &filesystems {
        println!("  Found: {} ({})", device, fstype);
        evidence.filesystems.push((device.clone(), fstype.clone()));
    }

    // 2. Detect operating system
    println!("\n[2] Detecting operating system...");
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        println!("  No OS detected");
    } else {
        for root in &roots {
            let ostype = g.inspect_get_type(root)?;
            let distro = g.inspect_get_distro(root).unwrap_or_default();
            let hostname = g.inspect_get_hostname(root).unwrap_or_default();

            println!("  OS Root: {}", root);
            println!("    Type: {}", ostype);
            println!("    Distribution: {}", distro);
            println!("    Hostname: {}", hostname);

            evidence.os_info = Some(OsInfo {
                root: root.clone(),
                ostype,
                distro,
                hostname,
            });
        }
    }

    // Mount filesystem for analysis
    if let Some(os_info) = &evidence.os_info {
        let mountpoints = g.inspect_get_mountpoints(&os_info.root)?;
        for (mp, device) in mountpoints {
            g.mount(&device, &mp)?;
        }

        // 3. Search for sensitive files
        println!("\n[3] Searching for sensitive files...");
        search_sensitive_files(&mut g, &mut evidence)?;

        // 4. Analyze user activity
        println!("\n[4] Analyzing user activity...");
        analyze_user_activity(&mut g, &mut evidence)?;

        // 5. Check for suspicious files
        println!("\n[5] Checking for suspicious files...");
        check_suspicious_files(&mut g, &mut evidence)?;

        // 6. Calculate checksums of important files
        println!("\n[6] Calculating checksums...");
        calculate_checksums(&mut g, &mut evidence)?;

        g.umount_all()?;
    }

    // 7. Generate report
    println!("\n{}", "=".repeat(70));
    println!("FORENSIC ANALYSIS REPORT");
    println!("{}", "=".repeat(70));

    evidence.print_report();

    g.shutdown()?;

    Ok(())
}

struct ForensicEvidence {
    filesystems: Vec<(String, String)>,
    os_info: Option<OsInfo>,
    sensitive_files: Vec<String>,
    user_files: HashMap<String, Vec<String>>,
    suspicious_files: Vec<SuspiciousFile>,
    checksums: HashMap<String, String>,
}

impl ForensicEvidence {
    fn new() -> Self {
        Self {
            filesystems: Vec::new(),
            os_info: None,
            sensitive_files: Vec::new(),
            user_files: HashMap::new(),
            suspicious_files: Vec::new(),
            checksums: HashMap::new(),
        }
    }

    fn print_report(&self) {
        println!("\n## Filesystems");
        for (dev, fs) in &self.filesystems {
            println!("  {} - {}", dev, fs);
        }

        if let Some(os) = &self.os_info {
            println!("\n## Operating System");
            println!("  Type: {}", os.ostype);
            println!("  Distribution: {}", os.distro);
            println!("  Hostname: {}", os.hostname);
        }

        println!("\n## Sensitive Files Found: {}", self.sensitive_files.len());
        for file in self.sensitive_files.iter().take(10) {
            println!("  {}", file);
        }
        if self.sensitive_files.len() > 10 {
            println!("  ... and {} more", self.sensitive_files.len() - 10);
        }

        println!("\n## User Activity");
        for (user, files) in &self.user_files {
            println!("  {}: {} files", user, files.len());
        }

        println!("\n## Suspicious Files: {}", self.suspicious_files.len());
        for file in &self.suspicious_files {
            println!("  {} - {}", file.path, file.reason);
        }

        println!("\n## Important File Checksums: {}", self.checksums.len());
        for (file, checksum) in self.checksums.iter().take(5) {
            println!("  {} : {}", file, checksum);
        }
    }
}

struct OsInfo {
    root: String,
    ostype: String,
    distro: String,
    hostname: String,
}

struct SuspiciousFile {
    path: String,
    reason: String,
}

fn search_sensitive_files(
    g: &mut Guestfs,
    evidence: &mut ForensicEvidence,
) -> Result<(), Box<dyn std::error::Error>> {
    let sensitive_patterns = vec![
        "/etc/passwd",
        "/etc/shadow",
        "/etc/ssh/",
        "/root/.ssh/",
        "/home/*/.ssh/",
    ];

    for pattern in sensitive_patterns {
        if let Ok(files) = g.glob_expand(pattern) {
            for file in files {
                if g.exists(&file).unwrap_or(false) {
                    println!("  Found: {}", file);
                    evidence.sensitive_files.push(file);
                }
            }
        }
    }

    Ok(())
}

fn analyze_user_activity(
    g: &mut Guestfs,
    evidence: &mut ForensicEvidence,
) -> Result<(), Box<dyn std::error::Error>> {
    // Try to read /etc/passwd to get users
    if g.exists("/etc/passwd")? {
        let passwd = g.cat("/etc/passwd")?;
        for line in passwd.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 6 {
                let username = parts[0];
                let home_dir = parts[5];

                // Check user's home directory
                if g.is_dir(home_dir).unwrap_or(false) {
                    if let Ok(files) = g.find(home_dir) {
                        println!("  User {}: {} files in home", username, files.len());
                        evidence
                            .user_files
                            .insert(username.to_string(), files.clone());
                    }
                }
            }
        }
    }

    Ok(())
}

fn check_suspicious_files(
    g: &mut Guestfs,
    evidence: &mut ForensicEvidence,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check for files in unusual locations
    let unusual_locations = vec!["/tmp", "/var/tmp", "/dev/shm"];

    for location in unusual_locations {
        if g.is_dir(location).unwrap_or(false) {
            if let Ok(files) = g.ls(location) {
                for file in files {
                    let full_path = format!("{}/{}", location, file);
                    if g.is_file(&full_path).unwrap_or(false) {
                        let size = g.filesize(&full_path).unwrap_or(0);
                        if size > 1024 * 1024 {
                            // Files > 1MB in tmp
                            println!("  Large file in temp: {} ({} bytes)", full_path, size);
                            evidence.suspicious_files.push(SuspiciousFile {
                                path: full_path,
                                reason: format!(
                                    "Large file in temporary directory ({} bytes)",
                                    size
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for hidden files in root
    if let Ok(files) = g.ls("/") {
        for file in files {
            if file.starts_with('.') && file != "." && file != ".." {
                let full_path = format!("/{}", file);
                println!("  Hidden file in root: {}", full_path);
                evidence.suspicious_files.push(SuspiciousFile {
                    path: full_path,
                    reason: "Hidden file in root directory".to_string(),
                });
            }
        }
    }

    Ok(())
}

fn calculate_checksums(
    g: &mut Guestfs,
    evidence: &mut ForensicEvidence,
) -> Result<(), Box<dyn std::error::Error>> {
    let important_files = vec![
        "/etc/passwd",
        "/etc/shadow",
        "/etc/group",
        "/boot/vmlinuz",
        "/bin/bash",
        "/bin/sh",
    ];

    for file in important_files {
        if g.exists(file).unwrap_or(false) && g.is_file(file).unwrap_or(false) {
            if let Ok(checksum) = g.checksum("sha256", file) {
                println!("  {}: {}", file, checksum);
                evidence.checksums.insert(file.to_string(), checksum);
            }
        }
    }

    Ok(())
}
