// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::{Path, State};
use axum::Json;
use guestkit::assurance::{answer_copilot_question, CopilotInsight, MigrationBriefing};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::jobs::{get_job_result, hydrate_job_record};
use crate::models::{ApiResponse, JobRecord};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CopilotAskRequest {
    pub question: String,
    #[serde(default)]
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct CopilotAskResponse {
    pub insight: CopilotInsight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub briefing: Option<MigrationBriefing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BriefingResponse {
    pub briefing: MigrationBriefing,
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CompareCopilotRequest {
    pub before_name: String,
    pub after_name: String,
    pub diff: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CompareCopilotResponse {
    pub headline: String,
    pub summary: String,
    pub recommendation: String,
    pub insights: Vec<CopilotInsight>,
}

#[derive(Debug, Deserialize)]
pub struct ExplainCheckRequest {
    pub check_id: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FleetOverviewRequest {
    pub disks: Vec<FleetDiskSummary>,
}

#[derive(Debug, Deserialize)]
pub struct FleetDiskSummary {
    pub name: String,
    #[serde(default)]
    pub boot_score: Option<f64>,
    #[serde(default)]
    pub blockers: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    pub readiness: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FleetOverviewResponse {
    pub headline: String,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub priority_disk: Option<String>,
}

fn extract_briefing(job_result: &serde_json::Value) -> Option<MigrationBriefing> {
    if let Some(inner) = job_result.get("data") {
        if let Some(copilot) = inner.get("copilot") {
            return serde_json::from_value(copilot.clone()).ok();
        }
    }
    job_result
        .get("copilot")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

async fn latest_briefing_for_vm(
    state: &AppState,
    vm_id: Uuid,
) -> Option<(Uuid, MigrationBriefing)> {
    let mut rows = sqlx::query_as::<_, JobRecord>(
        "SELECT id, vm_id, operation, status, worker_id, submitted_at, completed_at
         FROM jobs
         WHERE vm_id = $1
         AND operation IN ('guestkit.doctor', 'guestkit.migration-plan', 'guestkit.migrate-plan')
         ORDER BY submitted_at DESC
         LIMIT 12",
    )
    .bind(vm_id)
    .fetch_all(&state.pool)
    .await
    .ok()?;

    for row in rows.iter_mut() {
        hydrate_job_record(state, row, false).await;
        if row.status != "completed" {
            continue;
        }
        let mut redis = state.redis.clone();
        let Some(result) = get_job_result(&mut redis, &row.id.to_string()).await else {
            continue;
        };
        if let Some(b) = extract_briefing(&result) {
            return Some((row.id, b));
        }
    }
    None
}

pub async fn get_vm_briefing(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<BriefingResponse>>> {
    let Some((job_id, briefing)) = latest_briefing_for_vm(&state, id).await else {
        return Err(ApiError::bad_request(
            "No copilot briefing yet — run Doctor with explain=true first",
        ));
    };
    Ok(Json(ApiResponse::ok(BriefingResponse {
        briefing,
        job_id: Some(job_id),
    })))
}

pub async fn ask_copilot(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<CopilotAskRequest>,
) -> ApiResult<Json<ApiResponse<CopilotAskResponse>>> {
    if body.question.trim().is_empty() {
        return Err(ApiError::bad_request("question is required"));
    }

    let (job_id, briefing) = if let Some(job_id) = body.job_id {
        let mut redis = state.redis.clone();
        let result = get_job_result(&mut redis, &job_id.to_string())
            .await
            .ok_or_else(|| ApiError::not_found(format!("No result for job {job_id}")))?;
        let briefing = extract_briefing(&result).ok_or_else(|| {
            ApiError::bad_request(
                "Job result has no copilot briefing — run doctor with explain=true first",
            )
        })?;
        (Some(job_id), briefing)
    } else if let Some(found) = latest_briefing_for_vm(&state, id).await {
        (Some(found.0), found.1)
    } else {
        return Err(ApiError::bad_request(
            "No briefing available — run Doctor or Migration Plan with explain=true first",
        ));
    };

    let insight = answer_copilot_question(&body.question, &briefing);

    Ok(Json(ApiResponse::ok(CopilotAskResponse {
        insight,
        briefing: Some(briefing),
        job_id,
    })))
}

pub async fn launch_advice(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<CopilotInsight>>> {
    let Some((_, briefing)) = latest_briefing_for_vm(&state, id).await else {
        return Err(ApiError::bad_request("Run Doctor with explain=true first"));
    };
    let insight = if briefing.blocker_count > 0 {
        CopilotInsight {
            id: "launch_blocked".into(),
            question: "Can I launch this VM now?".into(),
            answer: format!(
                "Not yet — {} blocker(s) remain. Boot score {:.0}. Fix the top recommended action ({}) before applying YAML to the cluster.",
                briefing.blocker_count,
                briefing.boot_score,
                briefing
                    .recommended_actions
                    .first()
                    .map(|a| a.title.as_str())
                    .unwrap_or("repair-plan")
            ),
        }
    } else if briefing.boot_score >= 75.0 {
        CopilotInsight {
            id: "launch_ready".into(),
            question: "Can I launch this VM now?".into(),
            answer: format!(
                "Yes — readiness is {} with boot score {:.0}. Generate YAML, review CPU/memory in Launch Preview, then apply to KubeVirt. Next workflow: {}.",
                briefing.readiness,
                briefing.boot_score,
                briefing.next_workflow
            ),
        }
    } else {
        CopilotInsight {
            id: "launch_caution".into(),
            question: "Can I launch this VM now?".into(),
            answer: format!(
                "Proceed with caution — boot score {:.0} and {} warning(s). Consider repair-plan or migration-plan before launch. {}",
                briefing.boot_score,
                briefing.warning_count,
                briefing.summary
            ),
        }
    };
    Ok(Json(ApiResponse::ok(insight)))
}

pub async fn explain_check(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ExplainCheckRequest>,
) -> ApiResult<Json<ApiResponse<CopilotInsight>>> {
    let Some((_, briefing)) = latest_briefing_for_vm(&state, id).await else {
        return Err(ApiError::bad_request("Run Doctor with explain=true first"));
    };
    let q = format!(
        "Explain boot check {} — {}",
        body.check_id,
        body.message.as_deref().unwrap_or("what does this mean for migration?")
    );
    let mut insight = answer_copilot_question(&q, &briefing);
    insight.id = format!("check:{}", body.check_id);
    insight.question = q;
    if insight.answer == briefing.summary {
        insight.answer = format!(
            "Check `{}` affects boot confidence on this disk. Boot score {:.0}, {} blocker(s). {} Review repair-plan for automated fixes.",
            body.check_id,
            briefing.boot_score,
            briefing.blocker_count,
            body.message.as_deref().unwrap_or("")
        );
    }
    Ok(Json(ApiResponse::ok(insight)))
}

pub async fn compare_copilot(
    Json(body): Json<CompareCopilotRequest>,
) -> ApiResult<Json<ApiResponse<CompareCopilotResponse>>> {
    let before = body.diff.get("before_boot_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let after = body.diff.get("after_boot_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let delta = body
        .diff
        .get("boot_score_delta")
        .and_then(|v| v.as_f64())
        .unwrap_or(after - before);
    let bb = body
        .diff
        .get("before_blockers")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ab = body
        .diff
        .get("after_blockers")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let headline = if delta > 5.0 {
        format!("{} improved boot readiness vs {}", body.after_name, body.before_name)
    } else if delta < -5.0 {
        format!("{} regressed — investigate before launch", body.after_name)
    } else {
        "Disks are broadly similar for migration".into()
    };

    let summary = format!(
        "Boot score moved {:.0} → {:.0} (Δ {:+.0}). Blockers {} → {}. {}",
        before,
        after,
        delta,
        bb,
        ab,
        if delta > 0.0 {
            "The after disk is safer for KubeVirt first boot."
        } else if delta < 0.0 {
            "The after disk introduces new boot risks — run Doctor again."
        } else {
            "Scores are close; compare OS and driver gaps in the split view."
        }
    );

    let recommendation: String = if ab == 0 && after >= 75.0 {
        "Use the after disk for Launch Preview and provision.".into()
    } else if bb > ab {
        "Revert to the before disk or run repair-plan on the after image.".into()
    } else {
        "Run migration-plan on the stronger disk, then repair-plan if blockers remain.".into()
    };

    let insights = vec![
        CopilotInsight {
            id: "compare_delta".into(),
            question: "What changed between disks?".into(),
            answer: summary.clone(),
        },
        CopilotInsight {
            id: "compare_pick".into(),
            question: "Which disk should I launch?".into(),
            answer: recommendation.clone(),
        },
    ];

    Ok(Json(ApiResponse::ok(CompareCopilotResponse {
        headline,
        summary,
        recommendation,
        insights,
    })))
}

pub async fn fleet_overview(
    Json(body): Json<FleetOverviewRequest>,
) -> ApiResult<Json<ApiResponse<FleetOverviewResponse>>> {
    if body.disks.is_empty() {
        return Ok(Json(ApiResponse::ok(FleetOverviewResponse {
            headline: "Vault empty".into(),
            summary: "Upload or import a disk to start AI-assisted migration.".into(),
            recommendations: vec!["Upload a cloud image (Ubuntu, Cirros) via Intake Portal".into()],
            priority_disk: None,
        })));
    }

    let scored: Vec<_> = body
        .disks
        .iter()
        .filter(|d| d.boot_score.is_some())
        .collect();
    let blocked: Vec<_> = body
        .disks
        .iter()
        .filter(|d| d.blockers.unwrap_or(0) > 0)
        .collect();
    let best = body
        .disks
        .iter()
        .max_by(|a, b| {
            a.boot_score
                .unwrap_or(-1.0)
                .partial_cmp(&b.boot_score.unwrap_or(-1.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    let avg = if scored.is_empty() {
        None
    } else {
        Some(scored.iter().map(|d| d.boot_score.unwrap_or(0.0)).sum::<f64>() / scored.len() as f64)
    };

    let headline = format!(
        "{} disk(s) in vault — {} scanned, {} blocked",
        body.disks.len(),
        scored.len(),
        blocked.len()
    );

    let summary = match avg {
        Some(a) if a >= 80.0 => format!(
            "Fleet average boot score {:.0} — most disks look launch-ready. Prioritize unblocked images for migration.",
            a
        ),
        Some(a) if a >= 50.0 => format!(
            "Fleet average boot score {:.0} — mixed readiness. Run Doctor on unscored disks and repair blockers first.",
            a
        ),
        Some(a) => format!(
            "Fleet average boot score {:.0} — high risk. Focus repair-plan on blockers before any launch.",
            a
        ),
        None => "No Doctor scans yet — run Quick scan (Q) on your primary disk.".into(),
    };

    let mut recommendations = Vec::new();
    if let Some(d) = best {
        recommendations.push(format!(
            "Best candidate: {} (boot {:.0})",
            d.name,
            d.boot_score.unwrap_or(0.0)
        ));
    }
    if !blocked.is_empty() {
        recommendations.push(format!(
            "Repair {} blocked disk(s) before fleet-wide launch",
            blocked.len()
        ));
    }
    if scored.len() < body.disks.len() {
        recommendations.push("Run Doctor with explain on unscored disks for AI briefings".into());
    }

    Ok(Json(ApiResponse::ok(FleetOverviewResponse {
        headline,
        summary,
        recommendations,
        priority_disk: best.map(|d| d.name.clone()),
    })))
}
