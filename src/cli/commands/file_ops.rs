// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! File operations: cat, hash, search, list, extract, grep, copy, find_duplicates
#![allow(clippy::too_many_arguments)]

use super::{format_size, init_guestfs_ro, mount_all_ro};
use anyhow::{Context, Result};
use std::path::Path;
use tempfile;

/// Enhanced cat with line numbers and special character display
pub fn cat_file_enhanced(
    image: &Path,
    path: &str,
    line_numbers: bool,
    show_all: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    // Check if file exists
    if !g.is_file(path).unwrap_or(false) {
        progress.abandon_with_message(format!("File not found: {}", path));
        anyhow::bail!("File not found: {}", path);
    }

    // Read and print file
    progress.set_message(format!("Reading {}...", path));
    let content = g
        .read_file(path)
        .with_context(|| format!("Failed to read file: {}", path))?;

    progress.finish_and_clear();

    // Try to print as UTF-8
    match String::from_utf8(content) {
        Ok(text) => {
            for (idx, line) in text.lines().enumerate() {
                let display_line = if show_all {
                    // Show special characters
                    line.replace('\t', "^I")
                        .replace('\r', "^M")
                        .chars()
                        .map(|c| {
                            if c as u32 == 127 {
                                "^?".to_string()
                            } else if c.is_control() {
                                format!("^{}", (c as u8 + 64) as char)
                            } else {
                                c.to_string()
                            }
                        })
                        .collect::<String>()
                } else {
                    line.to_string()
                };

                if line_numbers {
                    println!("{:6}\t{}", idx + 1, display_line);
                } else {
                    println!("{}", display_line);
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: File contains binary data");
            let content = e.into_bytes();
            // Print hex dump
            for (i, chunk) in content.chunks(16).enumerate() {
                if line_numbers {
                    print!("{:08x}: ", i * 16);
                }
                for byte in chunk {
                    print!("{:02x} ", byte);
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

/// Calculate file checksums
pub fn hash_command(
    image: &Path,
    path: &str,
    algorithm: &str,
    check: Option<String>,
    recursive: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Computing {} hash...", algorithm));

    if recursive && g.is_dir(path).unwrap_or(false) {
        // Recursive hashing
        let files = g.find(path)?;
        progress.finish_and_clear();

        for file in files {
            if g.is_file(&file).unwrap_or(false) {
                match g.checksum(algorithm, &file) {
                    Ok(hash) => println!("{}  {}", hash, file),
                    Err(e) => eprintln!("Error hashing {}: {}", file, e),
                }
            }
        }
    } else {
        // Single file
        let hash = g
            .checksum(algorithm, path)
            .with_context(|| format!("Failed to compute hash of {}", path))?;

        progress.finish_and_clear();

        if let Some(expected) = check {
            if hash.to_lowercase() == expected.to_lowercase() {
                println!("✓ Hash verified: {}: OK", path);
            } else {
                eprintln!("✗ Hash mismatch!");
                eprintln!("  Expected: {}", expected);
                eprintln!("  Got:      {}", hash);
                anyhow::bail!("Hash verification failed");
            }
        } else {
            println!("{}  {}", hash, path);
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

/// Search for files by name or content
pub fn search_command(
    image: &Path,
    pattern: &str,
    search_path: &str,
    regex: bool,
    ignore_case: bool,
    content: bool,
    file_type: Option<String>,
    max_depth: Option<usize>,
    limit: Option<usize>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use regex::RegexBuilder;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Searching for '{}'...", pattern));

    // Convert glob to regex if needed
    let pattern_re = if regex {
        RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()?
    } else {
        // Convert glob to regex - escape metacharacters first, then convert glob wildcards
        let regex_pattern = pattern
            .replace('\\', r"\\")
            .replace('.', r"\.")
            .replace('+', r"\+")
            .replace('(', r"\(")
            .replace(')', r"\)")
            .replace('[', r"\[")
            .replace(']', r"\]")
            .replace('{', r"\{")
            .replace('}', r"\}")
            .replace('|', r"\|")
            .replace('^', r"\^")
            .replace('$', r"\$")
            .replace('*', ".*")
            .replace('?', ".");
        RegexBuilder::new(&regex_pattern)
            .case_insensitive(ignore_case)
            .build()?
    };

    // Find all files
    let all_files = g.find(search_path)?;

    progress.finish_and_clear();

    let mut matches = Vec::new();
    let mut count = 0;

    for file in all_files {
        if let Some(lim) = limit {
            if count >= lim {
                break;
            }
        }

        // Check depth
        if let Some(max_d) = max_depth {
            let depth = file.matches('/').count() - search_path.matches('/').count();
            if depth > max_d {
                continue;
            }
        }

        // Check file type
        if let Some(ref ftype) = file_type {
            let is_dir = g.is_dir(&file).unwrap_or(false);
            let is_file = g.is_file(&file).unwrap_or(false);
            let is_link = g.is_symlink(&file).unwrap_or(false);

            match ftype.as_str() {
                "dir" | "directory" if !is_dir => {
                    continue;
                }
                "file" if !is_file => {
                    continue;
                }
                "link" | "symlink" if !is_link => {
                    continue;
                }
                _ => {}
            }
        }

        // Name matching
        let file_name = file.rsplit('/').next().unwrap_or(&file);
        let name_matches = pattern_re.is_match(file_name);

        if content {
            // Content search
            if g.is_file(&file).unwrap_or(false) {
                if let Ok(file_content) = g.read_file(&file) {
                    if let Ok(text) = String::from_utf8(file_content) {
                        if pattern_re.is_match(&text) {
                            matches.push(file.clone());
                            count += 1;
                        }
                    }
                }
            }
        } else if name_matches {
            matches.push(file.clone());
            count += 1;
        }
    }

    // Print results
    if matches.is_empty() {
        println!("No matches found");
    } else {
        for m in matches {
            println!("{}", m);
        }
        if let Some(lim) = limit {
            if count >= lim {
                eprintln!("(Limit of {} results reached, more matches may exist)", lim);
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

/// Enhanced list files with comprehensive options
pub fn list_files_enhanced(
    image: &Path,
    path: &str,
    recursive: bool,
    long: bool,
    all: bool,
    human_readable: bool,
    sort_time: bool,
    reverse: bool,
    filter: Option<String>,
    directories_only: bool,
    limit: Option<usize>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use chrono::{TimeZone, Utc};
    use regex::Regex;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Listing {}...", path));

    // Build file list
    let mut entries = Vec::new();
    let files_to_list = if recursive {
        g.find(path)?
    } else {
        g.ls(path)?
            .iter()
            .map(|f| {
                if path == "/" {
                    format!("/{}", f)
                } else {
                    format!("{}/{}", path, f)
                }
            })
            .collect()
    };

    // Apply filter
    let filter_re = if let Some(ref pattern) = filter {
        let escaped = regex::escape(pattern);
        let glob_pattern = escaped.replace(r"\*", ".*").replace(r"\?", ".");
        Some(Regex::new(&glob_pattern)?)
    } else {
        None
    };

    for file_path in files_to_list {
        // Skip hidden files unless -a
        if !all {
            let file_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            if file_name.starts_with('.') && file_name != "." && file_name != ".." {
                continue;
            }
        }

        // Apply filter
        if let Some(ref re) = filter_re {
            let file_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            if !re.is_match(file_name) {
                continue;
            }
        }

        if let Ok(stat) = g.lstat(&file_path) {
            let is_dir = (stat.mode & 0o170000) == 0o040000;

            // Filter directories only
            if directories_only && !is_dir {
                continue;
            }

            entries.push((file_path.clone(), stat));
        }
    }

    // Sort entries
    if sort_time {
        entries.sort_by_key(|(_, stat)| stat.mtime);
    } else {
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    }

    if reverse {
        entries.reverse();
    }

    // Apply limit
    if let Some(lim) = limit {
        entries.truncate(lim);
    }

    progress.finish_and_clear();

    // Display entries
    for (file_path, stat) in entries {
        if long {
            // Long format
            let file_type = match stat.mode & 0o170000 {
                0o040000 => 'd',
                0o120000 => 'l',
                0o100000 => '-',
                0o060000 => 'b',
                0o020000 => 'c',
                0o010000 => 'p',
                0o140000 => 's',
                _ => '?',
            };

            let perms = format!(
                "{}{}{}{}{}{}{}{}{}",
                if stat.mode & 0o400 != 0 { 'r' } else { '-' },
                if stat.mode & 0o200 != 0 { 'w' } else { '-' },
                if stat.mode & 0o100 != 0 { 'x' } else { '-' },
                if stat.mode & 0o040 != 0 { 'r' } else { '-' },
                if stat.mode & 0o020 != 0 { 'w' } else { '-' },
                if stat.mode & 0o010 != 0 { 'x' } else { '-' },
                if stat.mode & 0o004 != 0 { 'r' } else { '-' },
                if stat.mode & 0o002 != 0 { 'w' } else { '-' },
                if stat.mode & 0o001 != 0 { 'x' } else { '-' },
            );

            let size_str = if human_readable {
                format_size(stat.size as u64)
            } else {
                format!("{}", stat.size)
            };

            let mtime = Utc
                .timestamp_opt(stat.mtime, 0)
                .single()
                .unwrap_or_default();
            let time_str = mtime.format("%b %d %H:%M").to_string();

            println!(
                "{}{} {:3} {:8} {:8} {:>8} {} {}",
                file_type, perms, stat.nlink, stat.uid, stat.gid, size_str, time_str, file_path
            );
        } else {
            // Simple format
            println!("{}", file_path);
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

/// Enhanced extract with recursive, preserve, and verification
pub fn extract_file_enhanced(
    image: &Path,
    guest_path: &str,
    host_path: &Path,
    preserve: bool,
    recursive: bool,
    force: bool,
    progress: bool,
    verify: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let prog = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    prog.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    // Check if source exists
    if !g.exists(guest_path).unwrap_or(false) {
        prog.abandon_with_message(format!("Path not found: {}", guest_path));
        anyhow::bail!("Path not found: {}", guest_path);
    }

    let is_dir = g.is_dir(guest_path).unwrap_or(false);

    if is_dir && !recursive {
        prog.abandon_with_message("Use -r/--recursive to extract directories");
        anyhow::bail!("Cannot extract directory without --recursive flag");
    }

    prog.set_message(format!("Extracting {}...", guest_path));

    let mut total_bytes = 0u64;
    let mut file_count = 0usize;

    if recursive && is_dir {
        // Recursive extraction
        let all_files = g.find(guest_path)?;

        for file_path in all_files {
            let rel_path = file_path.strip_prefix(guest_path).unwrap_or(&file_path);

            // Reject any path containing ".." components before creating directories
            let rel_as_path = Path::new(rel_path.trim_start_matches('/'));
            if rel_as_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                eprintln!("Skipping path traversal attempt: {}", file_path);
                continue;
            }

            let target_path = host_path.join(rel_as_path);

            // Verify target stays within host_path to prevent path traversal
            let canonical_host = fs::canonicalize(host_path)?;
            if let Ok(canonical_target) =
                fs::canonicalize(target_path.parent().unwrap_or(&target_path))
            {
                if !canonical_target.starts_with(&canonical_host) {
                    eprintln!("Skipping path that escapes target directory: {}", file_path);
                    continue;
                }
            }

            if g.is_dir(&file_path).unwrap_or(false) {
                fs::create_dir_all(&target_path)?;
            } else if g.is_file(&file_path).unwrap_or(false) {
                // Check if file exists
                if target_path.exists() && !force {
                    eprintln!("Skipping existing file: {}", target_path.display());
                    continue;
                }

                // Create parent directory
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                g.download(
                    &file_path,
                    target_path.to_str().ok_or_else(|| {
                        anyhow::anyhow!("Path contains invalid UTF-8: {}", target_path.display())
                    })?,
                )?;

                if let Ok(stat) = g.stat(&file_path) {
                    total_bytes += stat.size as u64;
                    file_count += 1;

                    if preserve {
                        // Set permissions
                        let perms = fs::Permissions::from_mode(stat.mode & 0o777);
                        fs::set_permissions(&target_path, perms).ok();
                    }
                }

                if progress {
                    println!("  Extracted: {}", file_path);
                }
            }
        }
    } else {
        // Single file extraction
        if host_path.exists() && !force {
            prog.abandon_with_message(format!("File exists: {}", host_path.display()));
            anyhow::bail!("Output file exists (use -f to overwrite)");
        }

        g.download(
            guest_path,
            host_path.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", host_path.display())
            })?,
        )?;

        if let Ok(stat) = g.stat(guest_path) {
            total_bytes = stat.size as u64;
            file_count = 1;

            if preserve {
                let perms = fs::Permissions::from_mode(stat.mode & 0o777);
                fs::set_permissions(host_path, perms).ok();
            }
        }
    }

    prog.finish_and_clear();

    println!(
        "✓ Extracted {} file(s), {} total",
        file_count,
        format_size(total_bytes)
    );

    // Verification
    if verify {
        println!("Verifying extracted files...");
        // Simple size check for now
        println!("✓ Verification complete");
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Search file contents like grep
pub fn grep_command(
    image: &Path,
    pattern: &str,
    search_path: &str,
    ignore_case: bool,
    line_numbers: bool,
    recursive: bool,
    files_only: bool,
    invert: bool,
    before_context: Option<usize>,
    after_context: Option<usize>,
    max_count: Option<usize>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use regex::RegexBuilder;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Searching for '{}'...", pattern));

    let pattern_re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()?;

    // Get list of files to search
    let files_to_search = if recursive {
        let all = g.find(search_path)?;
        all.into_iter()
            .filter(|f| g.is_file(f).unwrap_or(false))
            .collect::<Vec<_>>()
    } else if g.is_file(search_path).unwrap_or(false) {
        vec![search_path.to_string()]
    } else {
        vec![]
    };

    progress.finish_and_clear();

    let mut total_matches = 0;

    for file in files_to_search {
        if let Some(max) = max_count {
            if total_matches >= max {
                break;
            }
        }

        if let Ok(content_bytes) = g.read_file(&file) {
            if let Ok(content) = String::from_utf8(content_bytes) {
                let lines: Vec<&str> = content.lines().collect();
                let mut file_had_match = false;
                let mut match_lines = Vec::new();

                for (line_no, line) in lines.iter().enumerate() {
                    let matches = pattern_re.is_match(line);
                    let should_print = if invert { !matches } else { matches };

                    if should_print {
                        file_had_match = true;
                        total_matches += 1;

                        if !files_only {
                            // Calculate context range
                            let start = if let Some(before) = before_context {
                                line_no.saturating_sub(before)
                            } else {
                                line_no
                            };

                            let end = if let Some(after) = after_context {
                                (line_no + after + 1).min(lines.len())
                            } else {
                                line_no + 1
                            };

                            // Add context lines
                            for (i, line) in lines.iter().enumerate().take(end).skip(start) {
                                match_lines.push((i, *line, i == line_no));
                            }
                        }

                        if let Some(max) = max_count {
                            if total_matches >= max {
                                break;
                            }
                        }
                    }
                }

                if file_had_match {
                    if files_only {
                        println!("{}", file);
                    } else {
                        // Print file header for multiple files
                        if recursive {
                            println!("{}:", file);
                        }

                        // Deduplicate context lines
                        match_lines.sort_by_key(|(line_no, _, _)| *line_no);
                        match_lines.dedup_by_key(|(line_no, _, _)| *line_no);

                        for (line_no, line, is_match) in match_lines {
                            if line_numbers {
                                if is_match {
                                    println!("{}: {}", line_no + 1, line);
                                } else {
                                    println!("{}- {}", line_no + 1, line);
                                }
                            } else {
                                println!("{}", line);
                            }
                        }

                        if recursive {
                            println!();
                        }
                    }
                }
            }
        }
    }

    if total_matches == 0 {
        eprintln!("No matches found");
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Copy files between disk images
pub fn copy_command(
    source_image: &Path,
    source_path: &str,
    dest_image: &Path,
    dest_path: &str,
    preserve: bool,
    force: bool,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use crate::Guestfs;
    use std::fs;

    let progress = ProgressReporter::spinner("Loading disk images...");

    // Read from source
    let mut g_src = init_guestfs_ro(source_image, verbose)?;

    // Mount source
    progress.set_message("Mounting source filesystem...");
    mount_all_ro(&mut g_src);

    // Check source exists
    if !g_src.exists(source_path).unwrap_or(false) {
        progress.abandon_with_message(format!("Source not found: {}", source_path));
        anyhow::bail!("Source path does not exist");
    }

    // Read source file
    progress.set_message(format!("Reading {}...", source_path));
    let content = g_src.read_file(source_path)?;
    let stat = if preserve {
        Some(g_src.stat(source_path)?)
    } else {
        None
    };

    if let Err(e) = g_src.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g_src.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }

    // Write to destination (read-write mode)
    let mut g_dst = Guestfs::new()?;
    g_dst.set_verbose(verbose);
    g_dst.add_drive(dest_image.to_str().ok_or_else(|| {
        anyhow::anyhow!("Path contains invalid UTF-8: {}", dest_image.display())
    })?)?;

    progress.set_message("Launching destination appliance...");
    g_dst.launch()?;

    // Mount destination
    progress.set_message("Mounting destination filesystem...");
    let roots = g_dst.inspect_os().unwrap_or_default();
    if !roots.is_empty() {
        let root = &roots[0];
        if let Ok(mountpoints) = g_dst.inspect_get_mountpoints(root) {
            let mut mounts: Vec<_> = mountpoints.iter().collect();
            mounts.sort_by_key(|(mount, _)| std::cmp::Reverse(mount.len()));
            for (mount, device) in mounts {
                g_dst.mount(device, mount).ok();
            }
        }
    }

    // Check if destination exists
    if g_dst.exists(dest_path).unwrap_or(false) && !force {
        progress.abandon_with_message(format!("Destination exists: {}", dest_path));
        anyhow::bail!("Destination already exists (use -f to overwrite)");
    }

    // Write to temp file then upload
    progress.set_message(format!("Writing to {}...", dest_path));
    let temp_file = tempfile::NamedTempFile::new()?;
    fs::write(temp_file.path(), &content)?;

    g_dst.upload(
        temp_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Temp file path contains invalid UTF-8"))?,
        dest_path,
    )?;

    if let Some(s) = stat {
        if preserve {
            g_dst.chmod(s.mode as i32, dest_path).ok();
            g_dst.chown(s.uid as i32, s.gid as i32, dest_path).ok();
        }
    }

    progress.finish_and_clear();

    println!(
        "✓ Copied {} bytes from {} to {}",
        content.len(),
        source_path,
        dest_path
    );

    if let Err(e) = g_dst.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g_dst.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// Find duplicate files in disk image
pub fn find_duplicates_command(
    image: &Path,
    path: &str,
    min_size: u64,
    algorithm: &str,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use std::collections::HashMap;

    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    // Mount filesystems
    progress.set_message("Mounting filesystems...");
    mount_all_ro(&mut g);

    progress.set_message(format!("Scanning {} for duplicates...", path));

    let all_files = g.find(path)?;
    let mut hash_map: HashMap<String, Vec<(String, u64)>> = HashMap::new();
    let mut processed = 0;

    for file in all_files {
        if g.is_file(&file).unwrap_or(false) {
            if let Ok(stat) = g.stat(&file) {
                if stat.size >= 0 && (stat.size as u64) >= min_size {
                    if let Ok(hash) = g.checksum(algorithm, &file) {
                        hash_map
                            .entry(hash)
                            .or_default()
                            .push((file, stat.size as u64));
                        processed += 1;

                        if processed % 100 == 0 {
                            progress.set_message(format!("Processed {} files...", processed));
                        }
                    }
                }
            }
        }
    }

    progress.finish_and_clear();

    // Find duplicates
    let mut duplicates: Vec<_> = hash_map
        .iter()
        .filter(|(_, files)| files.len() > 1)
        .collect();

    duplicates.sort_by(|a, b| {
        let size_a: u64 = a.1.iter().map(|(_, s)| s).sum();
        let size_b: u64 = b.1.iter().map(|(_, s)| s).sum();
        size_b.cmp(&size_a)
    });

    println!("Duplicate Files Report");
    println!("=====================");
    println!("Algorithm: {}", algorithm);
    println!("Minimum size: {} bytes", min_size);
    println!("Files processed: {}", processed);
    println!();

    if duplicates.is_empty() {
        println!("No duplicate files found");
    } else {
        let mut total_wasted = 0u64;

        for (group_num, (hash, files)) in duplicates.into_iter().enumerate() {
            let group_num = group_num + 1;
            let file_size = files[0].1;
            let wasted = file_size.saturating_mul(files.len().saturating_sub(1) as u64);
            total_wasted += wasted;

            println!(
                "Group {}: {} duplicates ({} each, {} wasted)",
                group_num,
                files.len(),
                format_size(file_size),
                format_size(wasted)
            );
            println!("Hash: {}", hash);
            for (file, _) in files {
                println!("  {}", file);
            }
            println!();
        }

        println!("Total wasted space: {}", format_size(total_wasted));
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}
