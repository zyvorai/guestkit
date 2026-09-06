// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration assurance CLI commands (doctor, policy, fleet, migrate-plan, forensic diff).

use crate::assurance::{
    run_doctor, run_migrate_plan, run_repair_plan, MigratePlanOptions, RepairOptions,
};
use crate::fleet::{analyze_fleet, plan_waves, quarantine_fleet};
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};

use super::{init_guestfs_ro, mount_all_ro, validate_command};

pub use crate::assurance::collect_assurance_data;

/// Bootability prediction: `guestkit doctor`
pub fn doctor_command(
    image: &Path,
    target: &str,
    explain: bool,
    ai: bool,
    output_format: &str,
    fail_below: Option<u8>,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Analyzing bootability...");
    let result = run_doctor(image, target, explain, verbose)?;
    progress.finish_and_clear();

    let boot_report = &result.bootability;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!("{}", "Bootability Assessment".bold().cyan());
        println!("{}", "═".repeat(50));
        println!();
        println!("  {}", boot_report.assurance_score_message().green().bold());
        println!();

        if !boot_report.blockers.is_empty() {
            println!("{}", "Blockers:".red().bold());
            for b in &boot_report.blockers {
                println!("  {} {} — {}", "✗".red(), b.title.bold(), b.message);
                if let Some(r) = &b.remediation {
                    println!("    → {}", r.dimmed());
                }
            }
            println!();
        }

        if !boot_report.warnings.is_empty() {
            println!("{}", "Warnings:".yellow().bold());
            for w in &boot_report.warnings {
                println!("  {} {} — {}", "⚠".yellow(), w.title, w.message);
            }
            println!();
        }

        println!("{}", "Checks:".bold());
        for check in &boot_report.checks {
            if check.weight <= 0.0 {
                continue;
            }
            let icon = if check.passed {
                "✓".green()
            } else {
                "✗".red()
            };
            println!("  {} [{}] {}", icon, check.id, check.message);
        }

        if let Some(root_cause) = &result.root_cause {
            println!();
            println!("{}", "Root Cause Analysis".bold().cyan());
            println!("  {}", root_cause.summary.yellow());
            for step in &root_cause.chain {
                println!("  {}. {}", step.step, step.description);
            }
        }

        if explain || ai {
            print_guest_intelligence(image, target, verbose, ai, result.copilot.clone())?;
        }
    }

    enforce_doctor_score_gate(boot_report.score, fail_below)?;
    Ok(())
}

/// Fail CI pipelines when the boot score is below `--fail-below`.
pub fn enforce_doctor_score_gate(score: f64, fail_below: Option<u8>) -> Result<()> {
    if let Some(threshold) = fail_below {
        if score < f64::from(threshold) {
            anyhow::bail!(
                "boot score {:.0} is below --fail-below threshold {threshold}",
                score
            );
        }
    }
    Ok(())
}

fn print_guest_intelligence(
    image: &Path,
    target: &str,
    verbose: bool,
    ai: bool,
    copilot: Option<crate::assurance::MigrationBriefing>,
) -> Result<()> {
    use crate::ai::build_intelligence;
    use crate::assurance::{boot_target_from_str, collect_assurance_data};
    use colored::Colorize;

    let (evidence, boot) = collect_assurance_data(image, boot_target_from_str(target), verbose)?;
    let intel = build_intelligence(&evidence, Some(&boot), copilot);

    println!();
    println!("{}", "Guest Intelligence".bold().cyan());
    println!("{}", intel.narrative.executive_summary);

    if !intel.recommendations.is_empty() {
        println!();
        println!("{}", "Top recommendations:".bold());
        for rec in intel.recommendations.iter().take(5) {
            println!(
                "  • [{category:?}] {title} — {citation}",
                category = rec.category,
                title = rec.title,
                citation = rec.citation
            );
        }
    }

    println!("  CIS-lite score: {}/100", intel.security_profile.score);

    #[cfg(feature = "ai")]
    if ai {
        use crate::ai::{run_agent_on_evidence, AgentConfig};
        let rt = tokio::runtime::Runtime::new()?;
        let config = AgentConfig {
            boot_target: target.to_string(),
            ..Default::default()
        };
        match rt.block_on(run_agent_on_evidence(
            &evidence,
            "Summarize migration readiness, cite evidence, and list the top 3 actions.",
            &config,
        )) {
            Ok(agent) => {
                println!();
                println!("{}", "AI analysis".bold().magenta());
                println!("{}", agent.answer);
            }
            Err(e) => eprintln!("AI agent skipped: {e:#}"),
        }
    }

    #[cfg(not(feature = "ai"))]
    if ai {
        eprintln!("AI requires rebuild with --features ai");
    }

    Ok(())
}

/// Policy check alias wrapper
pub fn policy_check_command(
    image: &Path,
    policy: Option<&Path>,
    benchmark: Option<String>,
    example_policy: bool,
    format: &str,
    output: Option<&Path>,
    strict: bool,
    verbose: bool,
) -> Result<()> {
    validate_command(
        image,
        policy,
        benchmark,
        example_policy,
        format,
        output,
        strict,
        verbose,
    )
}

/// Fleet analysis: `guestkit fleet analyze`
pub fn fleet_analyze_command(
    dir: &Path,
    output_format: &str,
    recursive: bool,
    jobs: usize,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;

    let images = collect_fleet_disk_images(dir, recursive)?;

    if images.is_empty() {
        anyhow::bail!("No disk images found in {}", dir.display());
    }

    let jobs = jobs.max(1);
    let msg = format!("Analyzing {} VMs (jobs={jobs})...", images.len());
    let progress = ProgressReporter::spinner(&msg);

    let (snapshots, failed_vms) = collect_fleet_snapshots(&images, jobs, verbose)?;

    progress.finish_and_clear();

    let mut report = analyze_fleet(&snapshots);
    report.total_vms = images.len();
    report.failed_vms = failed_vms;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!();
        println!("{}", "Fleet Analysis".bold().cyan());
        println!("  Total VMs: {}", report.total_vms);
        println!("  Analyzed: {}", snapshots.len());
        println!("  Jobs: {jobs}");
        if !report.failed_vms.is_empty() {
            println!("  Failed: {}", report.failed_vms.len());
        }
        println!();

        for cluster in &report.clusters {
            if cluster.count > 1 {
                println!(
                    "  {} {} identical {} nodes",
                    "●".green(),
                    cluster.count,
                    cluster.os
                );
                for m in &cluster.members {
                    println!("      - {}", m);
                }
            }
        }

        if !report.snowflakes.is_empty() {
            println!();
            println!("{}", "Anomalous VMs:".yellow().bold());
            for s in &report.snowflakes {
                println!("  {} {} — {}", "◆".yellow(), s.image, s.reason);
            }
        }

        if !report.migration_blockers.is_empty() {
            println!();
            println!("{}", "Migration blockers:".red().bold());
            for b in &report.migration_blockers {
                println!(
                    "  {} {} (assurance score: {:.0}%)",
                    "✗".red(),
                    b.image,
                    b.boot_score
                );
            }
        }

        if !report.golden_image_candidates.is_empty() {
            println!();
            println!("{}", "Golden image candidates:".green().bold());
            for g in &report.golden_image_candidates {
                println!("  {} {}", "★".green(), g);
            }
        }

        if !report.failed_vms.is_empty() {
            println!();
            println!("{}", "Failed analyses:".red().bold());
            for f in &report.failed_vms {
                println!("  {} {} — {}", "✗".red(), f.image, f.error);
            }
        }
    }

    if !report.failed_vms.is_empty() {
        anyhow::bail!(
            "{} of {} VM(s) failed fleet analysis",
            report.failed_vms.len(),
            report.total_vms
        );
    }

    Ok(())
}

/// Order a fleet's disk images into dependency-aware migration waves:
/// `guestkit fleet wave-plan`
pub fn fleet_wave_plan_command(
    dir: &Path,
    output_format: &str,
    recursive: bool,
    jobs: usize,
    verbose: bool,
) -> Result<()> {
    use crate::core::ProgressReporter;
    use crate::fleet::VmRole;

    let images = collect_fleet_disk_images(dir, recursive)?;

    if images.is_empty() {
        anyhow::bail!("No disk images found in {}", dir.display());
    }

    let jobs = jobs.max(1);
    let msg = format!(
        "Planning migration waves for {} VMs (jobs={jobs})...",
        images.len()
    );
    let progress = ProgressReporter::spinner(&msg);

    let (snapshots, failed_vms) = collect_fleet_snapshots(&images, jobs, verbose)?;

    progress.finish_and_clear();

    let plan = plan_waves(&snapshots);

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!();
        println!("{}", "Migration Wave Plan".bold().cyan());
        println!("  VMs planned: {}", plan.members.len());
        if !failed_vms.is_empty() {
            println!("  Failed: {}", failed_vms.len());
        }
        println!();

        for wave in &plan.waves {
            println!("{}", format!("Wave {}:", wave.wave).bold());
            for image in &wave.images {
                let role = plan
                    .members
                    .iter()
                    .find(|m| &m.image == image)
                    .map(|m| m.role)
                    .unwrap_or(VmRole::Standard);
                let marker = if role == VmRole::Database {
                    format!(" {}", "(database)".yellow())
                } else {
                    String::new()
                };
                println!("  {} {}{}", "→".cyan(), image, marker);
            }
            println!();
        }

        if !plan.cycles.is_empty() {
            println!("{}", "Unresolved dependency cycles:".red().bold());
            for cycle in &plan.cycles {
                println!("  {} {}", "⟲".red(), cycle.join(" → "));
            }
            println!();
        }

        if !failed_vms.is_empty() {
            println!("{}", "Failed analyses:".red().bold());
            for f in &failed_vms {
                println!("  {} {} — {}", "✗".red(), f.image, f.error);
            }
        }
    }

    if !failed_vms.is_empty() {
        anyhow::bail!(
            "{} VM(s) failed evidence collection and were excluded from wave planning",
            failed_vms.len()
        );
    }

    Ok(())
}

/// Scheduled drift check against each VM's stored golden baseline:
/// `guestkit fleet watch`
#[allow(clippy::too_many_arguments)]
pub fn fleet_watch_command(
    dir: &Path,
    output_format: &str,
    recursive: bool,
    jobs: usize,
    reset_baseline: bool,
    fail_on_drift: bool,
    verbose: bool,
) -> Result<()> {
    use crate::ai::explain_fleet_drift;
    use crate::core::ProgressReporter;
    use crate::fleet::baseline;
    use serde::Serialize;

    #[derive(Serialize)]
    struct VmDriftStatus {
        image: String,
        hostname: String,
        status: &'static str, // "baseline_established" | "baseline_reset" | "no_drift" | "drift"
        drift: Option<crate::ai::FleetDriftReport>,
    }

    let images = collect_fleet_disk_images(dir, recursive)?;

    if images.is_empty() {
        anyhow::bail!("No disk images found in {}", dir.display());
    }

    let jobs = jobs.max(1);
    let msg = format!("Checking drift for {} VMs (jobs={jobs})...", images.len());
    let progress = ProgressReporter::spinner(&msg);

    let (snapshots, failed_vms) = collect_fleet_snapshots(&images, jobs, verbose)?;

    progress.finish_and_clear();

    let mut statuses = Vec::with_capacity(snapshots.len());
    let mut drifted = 0usize;
    for (image, evidence, _score) in &snapshots {
        let image_path = Path::new(image);
        let prior = if reset_baseline {
            None
        } else {
            baseline::load(image_path)
        };

        let status = match prior {
            None => {
                baseline::store(image_path, &evidence.os.hostname, evidence)?;
                VmDriftStatus {
                    image: image.clone(),
                    hostname: evidence.os.hostname.clone(),
                    status: if reset_baseline {
                        "baseline_reset"
                    } else {
                        "baseline_established"
                    },
                    drift: None,
                }
            }
            Some(base) => {
                let report = explain_fleet_drift(&base.evidence, evidence);
                let has_drift = !report.os_drift.is_empty()
                    || !report.systemd_drift.is_empty()
                    || !report.security_drift.is_empty()
                    || !report.package_drift.is_empty();
                if has_drift {
                    drifted += 1;
                }
                VmDriftStatus {
                    image: image.clone(),
                    hostname: evidence.os.hostname.clone(),
                    status: if has_drift { "drift" } else { "no_drift" },
                    drift: Some(report),
                }
            }
        };
        statuses.push(status);
    }

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
    } else {
        println!();
        println!("{}", "Fleet Drift Watch".bold().cyan());
        println!("  VMs checked: {}", statuses.len());
        if drifted > 0 {
            println!("  Drifted: {drifted}");
        }
        if !failed_vms.is_empty() {
            println!("  Failed: {}", failed_vms.len());
        }
        println!();

        for s in &statuses {
            match s.status {
                "baseline_established" => {
                    println!("  {} {} — baseline established", "●".cyan(), s.image);
                }
                "baseline_reset" => {
                    println!("  {} {} — baseline reset", "●".cyan(), s.image);
                }
                "no_drift" => {
                    println!("  {} {} — no drift", "✓".green(), s.image);
                }
                "drift" => {
                    println!("  {} {} — drifted", "✗".red().bold(), s.image);
                    if let Some(report) = &s.drift {
                        for line in report
                            .os_drift
                            .iter()
                            .chain(&report.systemd_drift)
                            .chain(&report.security_drift)
                            .chain(&report.package_drift)
                        {
                            println!("      - {line}");
                        }
                    }
                }
                _ => {}
            }
        }

        if !failed_vms.is_empty() {
            println!();
            println!("{}", "Failed analyses:".red().bold());
            for f in &failed_vms {
                println!("  {} {} — {}", "✗".red(), f.image, f.error);
            }
        }
    }

    if !failed_vms.is_empty() {
        anyhow::bail!(
            "{} VM(s) failed evidence collection and were excluded from the drift check",
            failed_vms.len()
        );
    }

    if fail_on_drift && drifted > 0 {
        anyhow::bail!(
            "{drifted} of {} VM(s) drifted from baseline",
            statuses.len()
        );
    }

    Ok(())
}

/// Fleet quarantine: `guestkit fleet quarantine`
pub fn fleet_quarantine_command(
    dir: &Path,
    output_format: &str,
    recursive: bool,
    jobs: usize,
    threshold: f64,
    fail: bool,
    verbose: bool,
) -> Result<()> {
    let images = collect_fleet_disk_images(dir, recursive)?;
    if images.is_empty() {
        anyhow::bail!("No disk images found in {}", dir.display());
    }
    let (snapshots, failed_vms) = collect_fleet_snapshots(&images, jobs.max(1), verbose)?;
    let mut analysis = analyze_fleet(&snapshots);
    analysis.total_vms = images.len();
    analysis.failed_vms = failed_vms.clone();
    let scored: Vec<(String, f64)> = snapshots.iter().map(|(p, _, s)| (p.clone(), *s)).collect();
    let report = quarantine_fleet(&scored, &analysis, &failed_vms, threshold);

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!();
        println!("{}", "Fleet quarantine".bold().cyan());
        println!("  threshold: {:.0}", report.threshold);
        println!("  allowed:   {}", report.allowed.len());
        println!("  quarantine: {}", report.quarantined.len());
        for a in &report.allowed {
            println!("  {} {a}", "✓".green());
        }
        for q in &report.quarantined {
            println!(
                "  {} {} ({})",
                "✗".red(),
                q.image,
                q.detail.as_deref().unwrap_or("quarantined")
            );
        }
    }

    if fail && !report.all_clear() {
        anyhow::bail!(
            "{} VM(s) quarantined — do not hand to h2kvmctl",
            report.quarantined.len()
        );
    }
    Ok(())
}

/// Passport → h2kvmctl job: `guestkit passport handoff`
pub fn passport_handoff_command(
    passport_path: &Path,
    output: Option<&Path>,
    fail_below: Option<f64>,
    require_signature: bool,
    public_key: Option<&str>,
    trust_keys: Option<&Path>,
    max_age_hours: Option<u64>,
    fail: bool,
) -> Result<()> {
    use crate::assurance::{
        build_handoff, handoff_default_output, load_and_gate, write_handoff, PassportVerifyOptions,
    };

    let (passport, allowed, extra) = load_and_gate(
        passport_path,
        &PassportVerifyOptions {
            fail_below,
            require_signature,
            public_key: public_key.map(|s| s.to_string()),
            trust_keys_file: trust_keys.map(|p| p.to_path_buf()),
            max_age_hours,
        },
    )?;
    let doc = build_handoff(passport_path, &passport, allowed, &extra);
    let dest = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| handoff_default_output(passport_path));
    write_handoff(&doc, &dest)?;
    if doc.allowed {
        println!(
            "{} handoff {} (boot={:.0} migration={:.0})",
            "✓".green(),
            dest.display(),
            doc.scores.boot,
            doc.scores.migration
        );
        println!("  {}", doc.h2kvmctl);
    } else {
        println!("{} handoff refused → {}", "✗".red(), dest.display());
        for b in &doc.blockers {
            println!("    - {b}");
        }
        if fail {
            anyhow::bail!("passport handoff not allowed");
        }
    }
    Ok(())
}

type FleetSnapshots = Vec<(String, crate::evidence::EvidenceSnapshot, f64)>;

/// Parallel evidence collection shared by `fleet analyze` and `fleet wave-plan`.
fn collect_fleet_snapshots(
    images: &[PathBuf],
    jobs: usize,
    verbose: bool,
) -> Result<(FleetSnapshots, Vec<crate::fleet::report::FleetFailedVm>)> {
    use crate::boot::{analyze_bootability, BootTarget};
    use crate::cli::cache::EvidenceCache;
    use crate::fleet::report::FleetFailedVm;
    use rayon::prelude::*;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .context("build fleet thread pool")?;

    let results: Vec<Result<(String, crate::evidence::EvidenceSnapshot, f64), FleetFailedVm>> =
        pool.install(|| {
            images
                .par_iter()
                .map(|image| {
                    if verbose {
                        eprintln!("  → {}", image.display());
                    }
                    // Fast path: reuse evidence cache → boot score without remount.
                    if let Ok(cache) = EvidenceCache::new() {
                        if let Ok(Some(evidence)) = cache.get(image) {
                            let boot = analyze_bootability(&evidence, BootTarget::Kvm);
                            return Ok((image.display().to_string(), evidence, boot.score));
                        }
                    }
                    match collect_assurance_data(image, BootTarget::Kvm, false) {
                        Ok((evidence, boot)) => {
                            if let Ok(cache) = EvidenceCache::new() {
                                let _ = cache.store(image, &evidence);
                            }
                            Ok((image.display().to_string(), evidence, boot.score))
                        }
                        Err(e) => Err(FleetFailedVm {
                            image: image.display().to_string(),
                            error: e.to_string(),
                        }),
                    }
                })
                .collect()
        });

    let mut snapshots = Vec::new();
    let mut failed_vms = Vec::new();
    for r in results {
        match r {
            Ok(tuple) => snapshots.push(tuple),
            Err(f) => failed_vms.push(f),
        }
    }

    Ok((snapshots, failed_vms))
}

fn is_fleet_disk_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "qcow2" | "vmdk" | "raw" | "img" | "vhd" | "vdi"
            )
        })
}

fn collect_fleet_disk_images(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    if recursive {
        collect_fleet_disk_images_recursive(dir, &mut images)?;
    } else {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_fleet_disk_image(&path) {
                images.push(path);
            }
        }
    }
    images.sort();
    Ok(images)
}

fn collect_fleet_disk_images_recursive(dir: &Path, images: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_fleet_disk_images_recursive(&path, images)?;
        } else if is_fleet_disk_image(&path) {
            images.push(path);
        }
    }
    Ok(())
}

/// Migration readiness assessment: `guestkit migrate-assess`
pub fn migrate_assess_command(
    image: &Path,
    target: &str,
    output_format: &str,
    fail_below: Option<f64>,
    verbose: bool,
) -> Result<()> {
    let (_evidence, assessment) = crate::assurance::run_migrate_assess(image, target, verbose)?;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&assessment)?);
    } else {
        println!(
            "Migration Score: {:.0}/100  ({:?})",
            assessment.overall_score, assessment.readiness
        );
        println!();
        let s = &assessment.sub_scores;
        println!("  Boot readiness:        {:>3.0}", s.boot);
        println!("  Storage readiness:     {:>3.0}", s.storage);
        println!("  Network readiness:     {:>3.0}", s.network);
        println!("  Driver readiness:      {:>3.0}", s.driver);
        println!("  Application readiness: {:>3.0}", s.application);
        println!("  Security readiness:    {:>3.0}", s.security);
        if !assessment.critical_blockers.is_empty() {
            println!("\nCritical blockers:");
            for b in &assessment.critical_blockers {
                println!("  - [{}] {}: {}", b.check_id, b.title, b.message);
            }
        }
        if !assessment.recommended_actions.is_empty() {
            println!("\nRecommended actions:");
            for (i, action) in assessment.recommended_actions.iter().enumerate() {
                println!("  {}. {}", i + 1, action);
            }
        }
        // §31 combined assessment: last-known running state from the guest's
        // on-disk cache (only present when a live agent wrote one).
        if let Some(oc) = &assessment.online_correlation {
            println!("\nLast-known running state (from in-guest cache, §31):");
            if let Some(ts) = oc.get("written_at").and_then(|v| v.as_str()) {
                println!("  captured (live): {ts}");
            }
            if let Some(p) = oc.get("payload") {
                if let Some(net) = p.get("network") {
                    println!(
                        "  listeners: {}  established: {}",
                        net.get("listening").unwrap_or(&serde_json::json!("?")),
                        net.get("established").unwrap_or(&serde_json::json!("?"))
                    );
                }
                if let Some(c) = p.get("containers").and_then(|c| c.get("count")) {
                    println!("  containers running: {c}");
                }
                if let Some(hb) = p.get("heartbeat").and_then(|h| h.get("agent_state")) {
                    println!("  last agent state: {hb}");
                }
            }
        }
    }

    if let Some(threshold) = fail_below {
        if assessment.overall_score < threshold {
            anyhow::bail!(
                "migration score {:.0} below threshold {threshold}",
                assessment.overall_score
            );
        }
    }
    Ok(())
}

/// Migration repair plan (offline): `guestkit migrate-repair`
pub fn migrate_repair_command(
    image: &Path,
    target: &str,
    apply: bool,
    include_destructive: bool,
    virtio_win: Option<&Path>,
    export: Option<&Path>,
    verbose: bool,
) -> Result<()> {
    let (evidence, assessment) = crate::assurance::run_migrate_assess(image, target, verbose)?;
    let (plan, notes) = crate::migration::MigrationRepairPlanner::from_assessment(
        &assessment,
        &evidence,
        &crate::migration::RepairOptions {
            include_destructive,
            virtio_win_dir: virtio_win.map(|p| p.to_path_buf()),
        },
    );

    for note in &notes {
        eprintln!("note: {note}");
    }
    if plan.operations.is_empty() {
        println!(
            "No automated repairs required (score {:.0}).",
            assessment.overall_score
        );
        return Ok(());
    }

    crate::cli::plan::PlanPreview::display(&plan);

    if let Some(path) = export {
        std::fs::write(path, serde_json::to_string_pretty(&plan)?)?;
        println!("Plan exported to {}", path.display());
    }

    if apply {
        // Offline apply refuses to run without a successful full-image backup.
        let applicator = crate::cli::plan::PlanApplicator::new(image.display().to_string(), false);
        let result = applicator.apply(&plan)?;
        println!("{}", result.message);
        if !result.success {
            anyhow::bail!("repair plan apply failed");
        }
    } else {
        println!(
            "\nDry run only — pass --apply to execute against the image (a backup is taken first)."
        );
    }
    Ok(())
}

/// Emit Cutover Passport: `guestkit passport emit`
pub fn passport_emit_command(
    image: &Path,
    target: &str,
    output: &Path,
    bundle: bool,
    content_hash: bool,
    virtio_win: Option<&Path>,
    live_url: Option<&str>,
    sign_key: Option<&Path>,
    issuer: Option<&str>,
    expires_hours: Option<u64>,
    verbose: bool,
) -> Result<()> {
    use crate::assurance::{emit_passport, write_passport_outputs, PassportEmitOptions};
    use crate::core::ProgressReporter;

    let progress = ProgressReporter::spinner("Emitting Cutover Passport...");
    let opts = PassportEmitOptions {
        verbose,
        content_hash,
        virtio_win_dir: virtio_win.map(|p| p.to_path_buf()),
        live_url: live_url.map(|s| s.to_string()),
        sign_key: sign_key.map(|p| p.to_path_buf()),
        issuer: issuer.map(|s| s.to_string()),
        expires_hours,
    };
    let (passport, plan) = emit_passport(image, target, &opts)?;
    write_passport_outputs(&passport, &plan, output, bundle)?;
    progress.finish_and_clear();

    println!("{}", "Cutover Passport".bold().cyan());
    println!(
        "  boot={:.0}  migration={:.0}  readiness={:?}  hard_blocked={}",
        passport.scores.boot,
        passport.scores.migration,
        passport.scores.readiness,
        passport.hard_blocked
    );
    if let Some(issuer) = &passport.issuer {
        println!("  issuer={}", issuer);
    }
    if let Some(exp) = &passport.expires_at {
        println!("  expires_at={}", exp);
    }
    if passport.windows.is_windows {
        println!(
            "  windows_offline_ready={}  bitlocker_blocker={}  virtio_drivers={}",
            passport.windows.windows_offline_ready,
            passport.windows.bitlocker_blocker,
            passport.windows.virtio_driver_count
        );
    }
    if let Some(live) = &passport.live_attestation {
        println!(
            "  live_attestation={} score={:?}",
            live.source, live.readiness_score
        );
    }
    println!("  wrote {}", output.display());
    println!(
        "  suite: {} → {}",
        passport.suite.assurance, passport.suite.next_step
    );
    Ok(())
}

/// Generate Ed25519 keypair: `guestkit passport keygen`
pub fn passport_keygen_command(seed: &Path, public: &Path) -> Result<()> {
    use crate::assurance::generate_passport_signing_key;

    let pub_hex = generate_passport_signing_key(seed, public)?;
    println!("{}", "Passport signing key".bold().cyan());
    println!("  seed={}", seed.display());
    println!("  public={}", public.display());
    println!("  public_key_hex={}", pub_hex);
    Ok(())
}

/// Verify Cutover Passport: `guestkit passport verify`
pub fn passport_verify_command(
    passport_path: &Path,
    fail_below: Option<f64>,
    require_signature: bool,
    public_key: Option<&str>,
    trust_keys: Option<&Path>,
    max_age_hours: Option<u64>,
) -> Result<()> {
    use crate::assurance::{verify_passport, CutoverPassport, PassportVerifyOptions};

    let raw = std::fs::read_to_string(passport_path)
        .with_context(|| format!("read passport {}", passport_path.display()))?;
    let passport: CutoverPassport =
        serde_json::from_str(&raw).context("parse Cutover Passport JSON")?;
    verify_passport(
        &passport,
        &PassportVerifyOptions {
            fail_below,
            require_signature,
            public_key: public_key.map(|s| s.to_string()),
            trust_keys_file: trust_keys.map(|p| p.to_path_buf()),
            max_age_hours,
        },
    )?;
    println!(
        "{} passport OK (boot={:.0} migration={:.0})",
        "✓".green(),
        passport.scores.boot,
        passport.scores.migration
    );
    Ok(())
}

/// Migration plan: `guestkit migrate-plan`
#[cfg(feature = "agent")]
pub fn migrate_plan_command(
    image: &Path,
    target: &str,
    explain: bool,
    ai: bool,
    output_format: &str,
    export: Option<&Path>,
    verbose: bool,
    inject_agent: bool,
    agent_binary: Option<&Path>,
    agent_unit: Option<&Path>,
) -> Result<()> {
    migrate_plan_command_impl(
        image,
        target,
        explain,
        ai,
        output_format,
        export,
        verbose,
        inject_agent,
        Some((agent_binary, agent_unit)),
    )
}

#[cfg(not(feature = "agent"))]
pub fn migrate_plan_command(
    image: &Path,
    target: &str,
    explain: bool,
    ai: bool,
    output_format: &str,
    export: Option<&Path>,
    verbose: bool,
    inject_agent: bool,
) -> Result<()> {
    migrate_plan_command_impl(
        image,
        target,
        explain,
        ai,
        output_format,
        export,
        verbose,
        inject_agent,
        None,
    )
}

fn migrate_plan_command_impl(
    image: &Path,
    target: &str,
    explain: bool,
    ai: bool,
    output_format: &str,
    export: Option<&Path>,
    verbose: bool,
    inject_agent: bool,
    #[cfg(feature = "agent")] agent_paths: Option<(Option<&Path>, Option<&Path>)>,
    #[cfg(not(feature = "agent"))] _agent_paths: Option<()>,
) -> Result<()> {
    let options = {
        #[cfg(feature = "agent")]
        let (agent_binary, agent_unit) = agent_paths
            .map(|(b, u)| (b.map(|p| p.to_path_buf()), u.map(|p| p.to_path_buf())))
            .unwrap_or((None, None));
        MigratePlanOptions {
            explain,
            verbose,
            export_fix_plan: export.is_some(),
            inject_agent,
            #[cfg(feature = "agent")]
            agent_binary,
            #[cfg(feature = "agent")]
            agent_unit,
        }
    };

    let result = run_migrate_plan(image, target, &options)?;
    let migration_score = &result.migration_score;
    let boot_report = &result.bootability;

    if let Some(export_path) = export {
        use crate::cli::plan::PlanExporter;
        use std::fs;

        let plan = result.fix_plan.as_ref().context("fix plan not generated")?;

        let content = if export_path.extension().is_some_and(|e| e == "json") {
            PlanExporter::to_json(plan)?
        } else {
            PlanExporter::to_yaml(plan)?
        };
        fs::write(export_path, content)
            .with_context(|| format!("Failed to write fix plan to {}", export_path.display()))?;
        println!(
            "{}",
            format!(
                "Fix plan written to {} ({} operations)",
                export_path.display(),
                plan.operations.len()
            )
            .green()
        );
    }

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!();
    println!("{}", format!("Migration Plan → {}", target).bold().cyan());
    println!("  Migration score: {:.0}%", migration_score.score);
    println!("  Boot score: {:.0}%", boot_report.score);
    println!(
        "  Est. downtime: {} min",
        migration_score.estimated_downtime_minutes
    );
    println!();

    if !migration_score.driver_injections.is_empty() {
        println!("{}", "Driver injections required:".yellow());
        for d in &migration_score.driver_injections {
            println!("  • {}", d);
        }
        println!();
    }

    if !migration_score.required_changes.is_empty() {
        println!("{}", "Required changes:".bold());
        for c in &migration_score.required_changes {
            println!("  • {}", c);
        }
        println!();
    }

    if !migration_score.licensing_warnings.is_empty() {
        println!("{}", "Licensing warnings:".red());
        for w in &migration_score.licensing_warnings {
            println!("  • {}", w);
        }
    }

    if let Some(rc) = &result.root_cause {
        println!();
        println!("  {}", rc.summary);
    }

    if explain || ai {
        print_guest_intelligence(image, target, verbose, ai, result.copilot.clone())?;
    }

    Ok(())
}

/// Forensic diff: `guestkit forensic-diff`
pub fn forensic_diff_command(
    old_image: &Path,
    new_image: &Path,
    output_format: &str,
    verbose: bool,
    sbom_old: Option<&Path>,
    sbom_new: Option<&Path>,
) -> Result<()> {
    use super::collect_inspection_data;
    use crate::cli::forensic_diff::{attach_sbom, compute_forensic_diff};

    let mut g1 = init_guestfs_ro(old_image, verbose)?;
    let root1 = mount_all_ro(&mut g1).context("No OS in old image")?;
    let report1 = collect_inspection_data(&mut g1, &root1, verbose)?;
    let _ = g1.shutdown();

    let mut g2 = init_guestfs_ro(new_image, verbose)?;
    let root2 = mount_all_ro(&mut g2).context("No OS in new image")?;
    let report2 = collect_inspection_data(&mut g2, &root2, verbose)?;
    let _ = g2.shutdown();

    let mut forensic = compute_forensic_diff(&report1, &report2);
    if let (Some(a), Some(b)) = (sbom_old, sbom_new) {
        attach_sbom(&mut forensic, crate::cli::sbom_diff::diff_files(a, b)?);
    }

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&forensic)?);
        return Ok(());
    }

    println!();
    println!("{}", "Forensic Diff Report".bold().cyan());
    println!("  {}", forensic.summary);
    println!(
        "  Security drift score: {:.0}%",
        forensic.security_drift_score
    );
    println!("  Config drift items: {}", forensic.config_drift_count);

    if !forensic.suspicious_persistence.is_empty() {
        println!();
        println!("{}", "Suspicious persistence:".yellow());
        for s in &forensic.suspicious_persistence {
            println!("  • {}", s);
        }
    }

    if !forensic.ransomware_indicators.is_empty() {
        println!();
        println!("{}", "Ransomware indicators:".red().bold());
        for r in &forensic.ransomware_indicators {
            println!("  • {}", r);
        }
    }

    if let Some(sbom) = &forensic.sbom {
        println!();
        println!("{}", "SBOM package drift:".cyan());
        crate::cli::sbom_diff::print_text(sbom);
    }

    Ok(())
}

/// Transactional boot repair: `guestkit repair --fix boot`
#[cfg(feature = "agent")]
pub fn repair_boot_command(
    image: &Path,
    dry_run: bool,
    verbose: bool,
    inject_agent: bool,
    agent_binary: Option<&Path>,
    agent_unit: Option<&Path>,
) -> Result<()> {
    repair_boot_command_impl(
        image,
        dry_run,
        verbose,
        inject_agent,
        Some((agent_binary, agent_unit)),
    )
}

#[cfg(not(feature = "agent"))]
pub fn repair_boot_command(
    image: &Path,
    dry_run: bool,
    verbose: bool,
    inject_agent: bool,
) -> Result<()> {
    repair_boot_command_impl(image, dry_run, verbose, inject_agent, None)
}

fn repair_boot_command_impl(
    image: &Path,
    dry_run: bool,
    verbose: bool,
    inject_agent: bool,
    #[cfg(feature = "agent")] agent_paths: Option<(Option<&Path>, Option<&Path>)>,
    #[cfg(not(feature = "agent"))] _agent_paths: Option<()>,
) -> Result<()> {
    let options = {
        #[cfg(feature = "agent")]
        let (agent_binary, agent_unit) = agent_paths
            .map(|(b, u)| (b.map(|p| p.to_path_buf()), u.map(|p| p.to_path_buf())))
            .unwrap_or((None, None));
        RepairOptions {
            dry_run,
            verbose,
            inject_agent,
            inject_qga: false,
            enable_systemd: true,
            fix_cloud_init_network: false,
            validate_fstab: false,
            #[cfg(feature = "agent")]
            agent_binary,
            #[cfg(feature = "agent")]
            agent_unit,
        }
    };

    let result = run_repair_plan(image, &options)?;

    if !result.applied && result.fix_plan.operations.is_empty() {
        println!("{}", result.message.green());
        return Ok(());
    }

    if dry_run {
        println!("{}", "Boot Repair Plan".bold().cyan());
        for op in &result.fix_plan.operations {
            println!("  → {}: {}", op.id, op.description);
        }
        println!();
        println!("{}", result.message.yellow());
        return Ok(());
    }

    if result.applied {
        println!("{}", result.message.green());
        if let (before, Some(after)) = (result.before_score, result.after_score) {
            if after < before {
                println!(
                    "{}",
                    "Warning: boot score decreased after repair — review changes".yellow()
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::enforce_doctor_score_gate;

    #[test]
    fn doctor_score_gate_passes_when_above_threshold() {
        enforce_doctor_score_gate(85.0, Some(80)).expect("pass");
    }

    #[test]
    fn doctor_score_gate_passes_when_equal_to_threshold() {
        enforce_doctor_score_gate(80.0, Some(80)).expect("pass");
    }

    #[test]
    fn doctor_score_gate_fails_when_below_threshold() {
        let err = enforce_doctor_score_gate(79.0, Some(80)).unwrap_err();
        assert!(err.to_string().contains("below --fail-below threshold 80"));
    }

    #[test]
    fn doctor_score_gate_skipped_when_unset() {
        enforce_doctor_score_gate(0.0, None).expect("pass");
    }

    #[test]
    fn fleet_disk_image_extensions() {
        use super::{collect_fleet_disk_images, is_fleet_disk_image};
        use std::path::Path;

        assert!(is_fleet_disk_image(Path::new("/vms/a.qcow2")));
        assert!(!is_fleet_disk_image(Path::new("/vms/readme.txt")));

        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("one.raw"), b"x").expect("write");
        std::fs::create_dir(dir.path().join("nested")).expect("mkdir");
        std::fs::write(dir.path().join("nested/two.vmdk"), b"y").expect("write nested");

        let flat = collect_fleet_disk_images(dir.path(), false).expect("flat");
        assert_eq!(flat.len(), 1);

        let deep = collect_fleet_disk_images(dir.path(), true).expect("recursive");
        assert_eq!(deep.len(), 2);
    }
}
