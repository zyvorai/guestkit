// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! JSON-RPC request dispatch for the in-guest agent.

use crate::agent::state::{AgentRuntime, ChannelHandle};
use crate::VERSION;
use guestkit_agent_protocol::{
    AgentCapabilities, AgentError, JsonRpcRequest, JsonRpcResponse, RpcErrorCode, RpcMethod,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct RequestHandler {
    capabilities: AgentCapabilities,
    runtime: Arc<AgentRuntime>,
}

fn parse_tier(params: &Value) -> Option<guestkit_agent_protocol::PerfTier> {
    params
        .get("tier")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

impl Default for RequestHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestHandler {
    pub fn new() -> Self {
        Self::with_runtime(AgentRuntime::global())
    }

    pub fn with_runtime(runtime: Arc<AgentRuntime>) -> Self {
        Self {
            capabilities: AgentCapabilities::standard(VERSION),
            runtime,
        }
    }

    /// Dispatch QGA (`execute`) or GuestKit JSON-RPC (`method`) on one virtio channel.
    pub fn handle_frame(&self, bytes: &[u8]) -> Vec<u8> {
        self.handle_frame_on(bytes, None)
    }

    /// Like [`handle_frame`], with the originating channel attached so
    /// channel-scoped methods (event subscription) can act on it.
    pub fn handle_frame_on(&self, bytes: &[u8], chan: Option<&Arc<ChannelHandle>>) -> Vec<u8> {
        if crate::agent::qga::is_qga_request(bytes) {
            // Generic passthrough: `guestkit-rpc` carries a full JSON-RPC request,
            // routed through the complete dispatch so EVERY agent method is
            // reachable over the QGA channel — not only the bespoke `guestkit-*`
            // shims. Everything else falls back to the QGA compat handler.
            if let Some(inner) = crate::agent::qga::extract_rpc_passthrough(bytes) {
                let resp = self.handle_on(&inner, chan);
                return crate::agent::qga::wrap_qga_response(&resp);
            }
            return crate::agent::qga::handle(bytes);
        }
        serde_json::to_vec(&self.handle_on(bytes, chan)).unwrap_or_else(|e| {
            serde_json::to_vec(&JsonRpcResponse::error(
                None,
                RpcErrorCode::InternalError,
                format!("serialize: {e}"),
            ))
            .unwrap_or_default()
        })
    }

    pub fn handle(&self, bytes: &[u8]) -> JsonRpcResponse {
        self.handle_on(bytes, None)
    }

    pub fn handle_on(&self, bytes: &[u8], chan: Option<&Arc<ChannelHandle>>) -> JsonRpcResponse {
        let req = match JsonRpcRequest::parse(bytes) {
            Ok(r) => r,
            Err(e) => return JsonRpcResponse::from_agent_error(None, e),
        };

        if let Err(e) = req.validate() {
            return JsonRpcResponse::from_agent_error(req.id, e);
        }

        // Security choke point. Policy authorization applies to everything
        // except capability negotiation; expiry, replay protection, and
        // idempotent response caching additionally apply to mutating methods.
        let method = req.method();
        let negotiation = matches!(
            method,
            RpcMethod::Ping | RpcMethod::GetVersion | RpcMethod::GetCapabilities
        );
        if !negotiation {
            let policy = crate::agent::policy::AgentPolicy::load();
            if let Err(e) = policy.authorize(&method, &req.method) {
                crate::agent::audit::audit(&req.method, "policy_denied", "");
                return JsonRpcResponse::from_agent_error(req.id, e);
            }
        }
        if method.is_mutating() {
            match self.enforce_security(&req, &method) {
                Ok(Some(cached)) => return cached,
                Ok(None) => {}
                Err(resp) => return *resp,
            }
            let resp = self.dispatch(&req, chan);
            if let Some(key) = req.idempotency_key.as_deref() {
                if resp.error.is_none() {
                    self.runtime.idempotent_store(key, resp.clone());
                }
            }
            crate::agent::audit::audit(
                &req.method,
                if resp.error.is_none() { "ok" } else { "error" },
                req.idempotency_key.as_deref().unwrap_or(""),
            );
            return resp;
        }
        self.dispatch(&req, chan)
    }

    /// Returns Ok(Some(response)) for an idempotent replay hit,
    /// Ok(None) to proceed, Err(response) on a security rejection.
    fn enforce_security(
        &self,
        req: &JsonRpcRequest,
        _method: &RpcMethod,
    ) -> Result<Option<JsonRpcResponse>, Box<JsonRpcResponse>> {
        let policy = crate::agent::policy::AgentPolicy::load();
        let sec = &policy.security;
        match (&req.ts, req.ttl_ms) {
            (Some(ts), ttl) => {
                let issued = chrono::DateTime::parse_from_rfc3339(ts).map_err(|e| {
                    Box::new(JsonRpcResponse::from_agent_error(
                        req.id.clone(),
                        AgentError::InvalidRequest(format!("bad ts: {e}")),
                    ))
                })?;
                let ttl_ms = ttl.unwrap_or(sec.max_ttl_ms).min(sec.max_ttl_ms);
                let deadline = issued + chrono::Duration::milliseconds(ttl_ms as i64);
                if chrono::Utc::now() > deadline {
                    crate::agent::audit::audit(&req.method, "expired", "");
                    return Err(Box::new(JsonRpcResponse::from_agent_error(
                        req.id.clone(),
                        AgentError::RequestExpired(format!("issued {ts}, ttl {ttl_ms}ms")),
                    )));
                }
            }
            (None, _) if sec.require_request_expiry => {
                return Err(Box::new(JsonRpcResponse::from_agent_error(
                    req.id.clone(),
                    AgentError::RequestExpired(
                        "policy requires ts/ttl_ms on mutating requests".into(),
                    ),
                )));
            }
            _ => {}
        }

        if let Some(nonce) = req.nonce.as_deref() {
            if !self.runtime.nonce_fresh(nonce, sec.nonce_cache_size) {
                crate::agent::audit::audit(&req.method, "replay", "");
                return Err(Box::new(JsonRpcResponse::from_agent_error(
                    req.id.clone(),
                    AgentError::ReplayDetected(format!("nonce {nonce} already used")),
                )));
            }
        }

        if let Some(key) = req.idempotency_key.as_deref() {
            if let Some(mut cached) = self.runtime.idempotent_get(key) {
                cached.id = req.id.clone();
                return Ok(Some(cached));
            }
        }
        Ok(None)
    }

    fn dispatch(&self, req: &JsonRpcRequest, chan: Option<&Arc<ChannelHandle>>) -> JsonRpcResponse {
        let req = req.clone();
        match req.method() {
            RpcMethod::Ping => Self::ping(req.id),
            RpcMethod::GetVersion => Self::get_version(req.id),
            RpcMethod::GetCapabilities => self.get_capabilities(req.id),
            RpcMethod::GetEvidence => self.get_evidence(req.id),
            RpcMethod::GetStatus => self.get_status(req.id),
            RpcMethod::Doctor => self.doctor(req.id, &req.params),
            RpcMethod::MigrateScore => self.migrate_score(req.id, &req.params),
            RpcMethod::GetMetrics => self.get_metrics(req.id),
            RpcMethod::GetFilesystem => self.get_filesystem(req.id),
            RpcMethod::Exec => self.exec(req.id, &req.params),
            RpcMethod::EnableRdp => self.enable_rdp(req.id),
            RpcMethod::DisableRdp => self.disable_rdp(req.id),
            RpcMethod::RunFixPlan => self.run_fix_plan(req.id, &req.params),
            RpcMethod::RunFixPlanRollback => self.run_fix_plan_rollback(req.id, &req.params),
            RpcMethod::GetGuestHealth => self.get_guest_health(req.id),
            RpcMethod::GetSystemdUnits => self.get_systemd_units(req.id),
            RpcMethod::GetFailedUnits => self.get_failed_units(req.id),
            RpcMethod::GetBootAnalysis => self.get_boot_analysis(req.id),
            RpcMethod::GetJournalSlice => self.get_journal_slice(req.id, &req.params),
            RpcMethod::GetLoginState => self.get_login_state(req.id),
            RpcMethod::GetDnsState => self.get_dns_state(req.id),
            RpcMethod::GetTimedateState => self.get_timedate_state(req.id),
            RpcMethod::GetSnapshotReadiness => self.get_snapshot_readiness(req.id),
            RpcMethod::FreezeFilesystem => self.freeze_filesystem(req.id),
            RpcMethod::ThawFilesystem => self.thaw_filesystem(req.id),
            RpcMethod::RestartUnit => self.restart_unit(req.id, &req.params),
            RpcMethod::ExecuteRemediationPlan => {
                self.execute_remediation_plan(req.id, &req.params)
            }
            RpcMethod::CollectSupportBundle => self.collect_support_bundle(req.id),
            RpcMethod::GetGuestInfo => self.get_guest_info(req.id),
            RpcMethod::GetSystemdUnit => self.get_systemd_unit(req.id, &req.params),
            RpcMethod::GetSystemdEvents => self.get_systemd_events(req.id, &req.params),
            RpcMethod::GetProcesses => self.get_processes(req.id),
            RpcMethod::GetAgentHealth => self.get_agent_health(req.id),
            RpcMethod::SubscribeEvents => self.subscribe_events(req.id, &req.params, chan),
            RpcMethod::UnsubscribeEvents => self.unsubscribe_events(req.id, chan),
            RpcMethod::GetCpuStats => self.get_cpu_stats(req.id),
            RpcMethod::GetMemoryStats => self.get_memory_stats(req.id),
            RpcMethod::GetPerformanceSummary => self.get_performance_summary(req.id, &req.params),
            RpcMethod::GetPerformanceHistory => self.get_performance_history(req.id, &req.params),
            RpcMethod::SnapshotPrepare => self.snapshot_prepare(req.id, &req.params),
            RpcMethod::SnapshotComplete => self.snapshot_complete(req.id),
            RpcMethod::MigrationAssess => self.migration_assess(req.id, &req.params),
            RpcMethod::MigrationPlan => self.migration_plan(req.id, &req.params),
            RpcMethod::MigrationRepair => self.migration_repair(req.id, &req.params),
            RpcMethod::MigrationPreCheck => self.migration_pre_check(req.id, &req.params),
            RpcMethod::MigrationCutoverEnter => self.cutover_enter(req.id, &req.params),
            RpcMethod::MigrationCutoverExit => self.cutover_exit(req.id),
            RpcMethod::MigrationValidate => self.migration_validate(req.id, &req.params),
            RpcMethod::BaselineCapture => self.baseline_capture(req.id, &req.params),
            RpcMethod::BaselineDiff => self.baseline_diff(req.id, &req.params),
            RpcMethod::StartUnit => self.control_unit(req.id, &req.params, "start"),
            RpcMethod::StopUnit => self.control_unit(req.id, &req.params, "stop"),
            RpcMethod::FileRead => Self::file_op(req.id, &req.params, crate::agent::file_ops::read),
            RpcMethod::FileWrite => {
                Self::file_op(req.id, &req.params, crate::agent::file_ops::write)
            }
            RpcMethod::FileStat => Self::file_op(req.id, &req.params, crate::agent::file_ops::stat),
            RpcMethod::FileList => Self::file_op(req.id, &req.params, crate::agent::file_ops::list),
            RpcMethod::FileChecksum => {
                Self::file_op(req.id, &req.params, crate::agent::file_ops::checksum)
            }
            RpcMethod::StorageRescan => {
                Self::json_result(req.id, crate::agent::storage_ops::rescan())
            }
            RpcMethod::StorageTrim => {
                Self::json_result(req.id, crate::agent::storage_ops::trim(&req.params))
            }
            RpcMethod::StorageExpand => {
                Self::json_result(req.id, crate::agent::storage_ops::expand(&req.params))
            }
            RpcMethod::GetProcess => self.get_process(req.id, &req.params),
            RpcMethod::NetworkTest => self.network_test(req.id, &req.params),
            RpcMethod::SecurityPosture => {
                let report = crate::agent::posture::collect();
                match serde_json::to_value(&report) {
                    Ok(v) => JsonRpcResponse::success(req.id, v),
                    Err(e) => JsonRpcResponse::error(
                        req.id,
                        RpcErrorCode::InternalError,
                        e.to_string(),
                    ),
                }
            }
            RpcMethod::NetworkConnections => {
                let intel = crate::agent::netintel::collect();
                match serde_json::to_value(&intel) {
                    Ok(v) => JsonRpcResponse::success(req.id, v),
                    Err(e) => JsonRpcResponse::error(
                        req.id,
                        RpcErrorCode::InternalError,
                        e.to_string(),
                    ),
                }
            }
            RpcMethod::TimeSyncNow => self.time_sync_now(req.id),
            RpcMethod::Reboot => self.power_action(req.id, &req.params, "reboot"),
            RpcMethod::Shutdown => self.power_action(req.id, &req.params, "shutdown"),
            RpcMethod::ContainersInventory => {
                JsonRpcResponse::success(req.id, crate::agent::containers::inventory())
            }
            RpcMethod::IntegrityBaseline => {
                Self::json_result(req.id, crate::agent::integrity::write_baseline())
            }
            RpcMethod::IntegrityCheck => {
                JsonRpcResponse::success(req.id, crate::agent::integrity::check())
            }
            RpcMethod::InventoryCacheWrite => Self::json_result(
                req.id,
                crate::agent::inventory_cache::write_cache(&self.runtime)
                    .map(|_| json!({ "written": crate::agent::inventory_cache::cache_path().display().to_string() })),
            ),
            RpcMethod::PackagesInventory => {
                JsonRpcResponse::success(req.id, crate::agent::packages::inventory())
            }
            RpcMethod::PackagesUpdates => {
                JsonRpcResponse::success(req.id, crate::agent::packages::updates())
            }
            RpcMethod::PackagesInstall => {
                Self::json_result(req.id, crate::agent::packages::install(&req.params))
            }
            RpcMethod::CertificatesInventory => {
                JsonRpcResponse::success(req.id, crate::agent::certificates::inventory())
            }
            RpcMethod::UsersInventory => {
                JsonRpcResponse::success(req.id, crate::agent::users::inventory())
            }
            RpcMethod::SetHostname => {
                Self::json_result(req.id, crate::agent::customization::set_hostname(&req.params))
            }
            RpcMethod::SetTimezone => {
                Self::json_result(req.id, crate::agent::customization::set_timezone(&req.params))
            }
            RpcMethod::SetDns => {
                Self::json_result(req.id, crate::agent::customization::set_dns(&req.params))
            }
            RpcMethod::Unknown(name) => JsonRpcResponse::error(
                req.id,
                RpcErrorCode::MethodNotFound,
                format!("unknown method: {name}"),
            ),
            // Protocol 1.3 methods whose handlers have not landed yet. They
            // are deliberately absent from AgentCapabilities::standard() so
            // hosts negotiating capabilities never see them advertised.
            other => JsonRpcResponse::error(
                req.id,
                RpcErrorCode::NotImplemented,
                format!("method not implemented yet: {other:?}"),
            ),
        }
    }

    fn assess_live(
        target: &str,
    ) -> Result<
        (
            crate::evidence::EvidenceSnapshot,
            crate::migration::MigrationAssessment,
        ),
        String,
    > {
        let evidence = crate::evidence::build_evidence_live().map_err(|e| e.to_string())?;
        let boot_target = crate::boot::BootTarget::parse(target);
        let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
        let assessment = crate::migration::assess_migration(&evidence, &boot_report, target, true);
        Ok((evidence, assessment))
    }

    fn migration_assess(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");
        match Self::assess_live(target) {
            Ok((_, assessment)) => {
                JsonRpcResponse::success(id, serde_json::to_value(assessment).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("migration assess failed: {e}"),
            ),
        }
    }

    fn migration_plan(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");
        let include_destructive = params
            .get("include_destructive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        match Self::assess_live(target) {
            Ok((evidence, assessment)) => {
                let (plan, notes) = crate::migration::MigrationRepairPlanner::from_assessment(
                    &assessment,
                    &evidence,
                    &crate::migration::RepairOptions {
                        include_destructive,
                        virtio_win_dir: None,
                    },
                );
                JsonRpcResponse::success(
                    id,
                    json!({
                        "plan": serde_json::to_value(&plan).unwrap_or(json!({})),
                        "planner_notes": notes,
                        "score": assessment.overall_score,
                        "readiness": assessment.readiness,
                        "blockers": assessment.critical_blockers,
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("migration plan failed: {e}"),
            ),
        }
    }

    /// Apply (or dry-run) a migration repair plan. Safe by construction:
    /// dry_run defaults to true and a real apply requires `confirm: true`;
    /// destructive plans additionally require the
    /// `migration.repair_destructive` policy flag.
    fn migration_repair(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let policy = crate::agent::policy::AgentPolicy::load();
        let dry_run = params
            .get("dry_run")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let confirmed = params
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let include_destructive = params
            .get("include_destructive")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if !dry_run && !confirmed {
            return JsonRpcResponse::from_agent_error(
                id,
                AgentError::PolicyDenied(
                    "applying repairs requires {\"dry_run\": false, \"confirm\": true}".into(),
                ),
            );
        }
        if include_destructive && !policy.actions.migration.repair_destructive {
            return JsonRpcResponse::from_agent_error(
                id,
                AgentError::PolicyDenied(
                    "destructive repairs disabled by local policy (migration.repair_destructive)"
                        .into(),
                ),
            );
        }

        // Either apply a supplied plan or generate one from a fresh assessment.
        let plan: crate::cli::plan::FixPlan = if let Some(plan_value) = params.get("plan") {
            match serde_json::from_value(plan_value.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        RpcErrorCode::InvalidParams,
                        format!("invalid plan: {e}"),
                    )
                }
            }
        } else {
            let target = params
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("kvm");
            match Self::assess_live(target) {
                Ok((evidence, assessment)) => {
                    crate::migration::MigrationRepairPlanner::from_assessment(
                        &assessment,
                        &evidence,
                        &crate::migration::RepairOptions {
                            include_destructive,
                            virtio_win_dir: None,
                        },
                    )
                    .0
                }
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        RpcErrorCode::InternalError,
                        format!("assessment for repair failed: {e}"),
                    )
                }
            }
        };

        let executor = crate::cli::plan::executor_live::LivePlanExecutor::for_migration(dry_run);
        match executor.apply(&plan) {
            Ok(result) => {
                JsonRpcResponse::success(id, serde_json::to_value(result).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::PlanApplyFailed,
                format!("repair apply failed: {e}"),
            ),
        }
    }

    fn snapshot_prepare(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let watchdog_secs = params.get("watchdog_secs").and_then(Value::as_u64);
        match crate::agent::snapshot::prepare(watchdog_secs) {
            Ok(report) => {
                JsonRpcResponse::success(id, serde_json::to_value(report).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn snapshot_complete(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::snapshot::complete() {
            Ok(report) => {
                JsonRpcResponse::success(id, serde_json::to_value(report).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn migration_pre_check(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");
        match crate::migration::workflow::pre_migration_check(target) {
            Ok(token) => {
                JsonRpcResponse::success(id, serde_json::to_value(token).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn cutover_enter(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        // Cutover requires a valid, unexpired readiness token.
        let token: Option<crate::migration::workflow::ReadinessToken> = params
            .get("token")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        match token {
            Some(token) => {
                if let Err(e) = crate::migration::workflow::verify_token(&token) {
                    return JsonRpcResponse::from_agent_error(
                        id,
                        AgentError::PolicyDenied(format!("invalid readiness token: {e}")),
                    );
                }
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InvalidParams,
                    "missing readiness token (run migration.preCheck first)",
                )
            }
        }
        let cutover_params: crate::migration::workflow::CutoverParams =
            serde_json::from_value(params.clone()).unwrap_or_default();
        match crate::migration::workflow::enter_cutover(&cutover_params) {
            Ok(state) => {
                JsonRpcResponse::success(id, serde_json::to_value(state).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn cutover_exit(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::migration::workflow::exit_cutover() {
            Ok(state) => {
                JsonRpcResponse::success(id, serde_json::to_value(state).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn migration_validate(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let baseline_id = params.get("baseline_id").and_then(|v| v.as_str());
        match crate::migration::workflow::post_migration_validate(baseline_id) {
            Ok(report) => {
                JsonRpcResponse::success(id, serde_json::to_value(report).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn baseline_capture(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let phase = match params.get("phase").and_then(|v| v.as_str()) {
            Some("post_migration") => crate::migration::baseline::BaselinePhase::PostMigration,
            _ => crate::migration::baseline::BaselinePhase::PreMigration,
        };
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");
        match crate::migration::baseline::capture_baseline(phase, target) {
            Ok(baseline) => JsonRpcResponse::success(
                id,
                json!({ "baseline_id": baseline.id, "captured_at": baseline.captured_at }),
            ),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn baseline_diff(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let Some(before_id) = params.get("before_id").and_then(|v| v.as_str()) else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                "missing required param: before_id",
            );
        };
        let before = match crate::migration::baseline::load_baseline(before_id) {
            Ok(b) => b,
            Err(e) => {
                return JsonRpcResponse::error(id, RpcErrorCode::InvalidParams, e.to_string())
            }
        };
        let after = match params.get("after_id").and_then(|v| v.as_str()) {
            Some(after_id) => match crate::migration::baseline::load_baseline(after_id) {
                Ok(b) => b.evidence,
                Err(e) => {
                    return JsonRpcResponse::error(id, RpcErrorCode::InvalidParams, e.to_string())
                }
            },
            None => match crate::evidence::build_evidence_live() {
                Ok(ev) => ev,
                Err(e) => {
                    return JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string())
                }
            },
        };
        let drift = crate::migration::baseline::diff_baselines(&before, &after);
        JsonRpcResponse::success(id, serde_json::to_value(drift).unwrap_or(json!({})))
    }

    fn file_op(
        id: Option<Value>,
        params: &Value,
        f: fn(&crate::agent::policy::FileOpsPolicy, &Value) -> anyhow::Result<Value>,
    ) -> JsonRpcResponse {
        // Category enablement already checked at the choke point; the
        // per-path allowlist and size cap are enforced here.
        let policy = crate::agent::policy::AgentPolicy::load();
        match f(&policy.capabilities.file_ops, params) {
            Ok(result) => JsonRpcResponse::success(id, result),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::CapabilityDenied, e.to_string()),
        }
    }

    fn json_result(id: Option<Value>, result: anyhow::Result<Value>) -> JsonRpcResponse {
        match result {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn control_unit(&self, id: Option<Value>, params: &Value, op: &str) -> JsonRpcResponse {
        let Some(unit) = params.get("unit").and_then(|v| v.as_str()) else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                "missing required param: unit",
            );
        };
        let executor = crate::agent::executor::Executor::new();
        match executor.control_unit(op, unit) {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_process(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let Some(pid) = params.get("pid").and_then(Value::as_u64) else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                "missing required param: pid",
            );
        };
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let mut sys = System::new();
        let target = Pid::from_u32(pid as u32);
        sys.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
        let Some(proc_) = sys.process(target) else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                format!("no such process: {pid}"),
            );
        };
        // Mutated only in the Linux cgroup-enrichment block below.
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut info = json!({
            "pid": pid,
            "name": proc_.name().to_string_lossy(),
            "exe": proc_.exe().map(|p| p.display().to_string()),
            "cmd": proc_.cmd().iter().map(|c| c.to_string_lossy().to_string()).collect::<Vec<_>>(),
            "cwd": proc_.cwd().map(|p| p.display().to_string()),
            "parent_pid": proc_.parent().map(|p| p.as_u32()),
            "user_id": proc_.user_id().map(|u| u.to_string()),
            "status": proc_.status().to_string(),
            "memory_bytes": proc_.memory(),
            "virtual_memory_bytes": proc_.virtual_memory(),
            "cpu_usage_percent": proc_.cpu_usage(),
            "run_time_secs": proc_.run_time(),
        });
        #[cfg(target_os = "linux")]
        if let Ok(cgroup) = std::fs::read_to_string(format!("/proc/{pid}/cgroup")) {
            if let Some(unit) = cgroup
                .lines()
                .find_map(|l| l.rsplit('/').next())
                .filter(|u| u.ends_with(".service") || u.ends_with(".scope"))
            {
                info["systemd_unit"] = json!(unit);
            }
        }
        JsonRpcResponse::success(id, info)
    }

    fn network_test(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let report = crate::agent::nettest::run(params);
        match serde_json::to_value(&report) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn time_sync_now(&self, id: Option<Value>) -> JsonRpcResponse {
        if crate::agent::executor_ipc::executor_available() {
            if let Ok(result) = crate::agent::executor_ipc::call_executor("time_sync", json!({})) {
                return JsonRpcResponse::success(id, json!({ "message": result }));
            }
        }
        match crate::agent::executor_ipc::run_time_sync() {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn power_action(&self, id: Option<Value>, params: &Value, action: &str) -> JsonRpcResponse {
        let policy = crate::agent::policy::AgentPolicy::load();
        let confirmed = params
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if policy.actions.reboot_vm.require_approval && !confirmed {
            return JsonRpcResponse::from_agent_error(
                id,
                AgentError::PolicyDenied(format!(
                    "{action} requires explicit approval: pass {{\"confirm\": true}}"
                )),
            );
        }
        let delay_secs = params
            .get("delay_secs")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        if crate::agent::executor_ipc::executor_available() {
            if let Ok(result) = crate::agent::executor_ipc::call_executor(
                "power_action",
                json!({ "action": action, "delay_secs": delay_secs }),
            ) {
                return JsonRpcResponse::success(id, json!({ "message": result }));
            }
        }
        match crate::agent::executor_ipc::run_power_action(action, delay_secs) {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_cpu_stats(&self, id: Option<Value>) -> JsonRpcResponse {
        let latest = self.runtime.telemetry.latest();
        let summary = self
            .runtime
            .telemetry
            .summary(guestkit_agent_protocol::PerfTier::Fine, 60);
        JsonRpcResponse::success(
            id,
            json!({
                "cpu_pct": latest.map(|s| s.cpu_pct),
                "load1": latest.map(|s| s.load1),
                "psi_cpu": latest.map(|s| s.psi_cpu),
                "procs": latest.map(|s| s.procs),
                "last_60s": summary.metrics.get("cpu_pct"),
            }),
        )
    }

    fn get_memory_stats(&self, id: Option<Value>) -> JsonRpcResponse {
        let latest = self.runtime.telemetry.latest();
        JsonRpcResponse::success(
            id,
            json!({
                "mem_used": latest.map(|s| s.mem_used),
                "mem_avail": latest.map(|s| s.mem_avail),
                "swap_used": latest.map(|s| s.swap_used),
                "mem_pct": latest.map(|s| s.mem_pct),
                "psi_mem": latest.map(|s| s.psi_mem),
            }),
        )
    }

    fn get_performance_summary(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let tier = parse_tier(params).unwrap_or(guestkit_agent_protocol::PerfTier::Fine);
        let window_secs = params
            .get("window_secs")
            .and_then(Value::as_u64)
            .unwrap_or(900);
        let summary = self.runtime.telemetry.summary(tier, window_secs);
        match serde_json::to_value(&summary) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_performance_history(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let Some(tier) = parse_tier(params) else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                "tier must be one of \"fine\", \"medium\", \"coarse\"",
            );
        };
        let from_ts = params.get("from_ts").and_then(Value::as_u64);
        let to_ts = params.get("to_ts").and_then(Value::as_u64);
        let metrics: Option<Vec<String>> = params
            .get("metrics")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let series = self
            .runtime
            .telemetry
            .series(tier, from_ts, to_ts, metrics.as_deref());
        match serde_json::to_value(&series) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_agent_health(&self, id: Option<Value>) -> JsonRpcResponse {
        let hb = crate::agent::heartbeat::build_heartbeat(&self.runtime);
        match serde_json::to_value(&hb) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn subscribe_events(
        &self,
        id: Option<Value>,
        params: &Value,
        chan: Option<&Arc<ChannelHandle>>,
    ) -> JsonRpcResponse {
        let Some(chan) = chan else {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::CapabilityDenied,
                "event subscription is only available on daemon channels",
            );
        };
        if !chan.push_capable {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::CapabilityDenied,
                "push is not allowed on the QGA-shared channel; connect via the dedicated GuestKit channel",
            );
        }
        let requested: Vec<String> = params
            .get("events")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| vec!["heartbeat".to_string()]);
        if !requested.iter().any(|e| e == "heartbeat") {
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::InvalidParams,
                "only the \"heartbeat\" event is supported",
            );
        }
        chan.subscribed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        JsonRpcResponse::success(
            id,
            json!({
                "subscribed": ["heartbeat"],
                "interval_secs": crate::agent::heartbeat::DEFAULT_INTERVAL_SECS,
                "channel": chan.name,
            }),
        )
    }

    fn unsubscribe_events(
        &self,
        id: Option<Value>,
        chan: Option<&Arc<ChannelHandle>>,
    ) -> JsonRpcResponse {
        if let Some(chan) = chan {
            chan.subscribed
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
        JsonRpcResponse::success(id, json!({ "subscribed": [] }))
    }

    fn ping(id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(id, json!({ "pong": true }))
    }

    fn get_version(id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "version": VERSION,
                "protocol": guestkit_agent_protocol::PROTOCOL_VERSION,
            }),
        )
    }

    fn get_capabilities(&self, id: Option<Value>) -> JsonRpcResponse {
        let mut caps = self.capabilities.clone();
        caps.categories = crate::agent::policy::AgentPolicy::load().enabled_categories();
        JsonRpcResponse::success(id, serde_json::to_value(&caps).unwrap_or(json!({})))
    }

    fn get_evidence(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let health = crate::health::build_guest_health(&evidence);
                match serde_json::to_value(evidence) {
                    Ok(mut v) => {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "guest_health".to_string(),
                                serde_json::to_value(health).unwrap_or(json!({})),
                            );
                        }
                        JsonRpcResponse::success(id, v)
                    }
                    Err(e) => JsonRpcResponse::error(
                        id,
                        RpcErrorCode::InternalError,
                        format!("serialize evidence: {e}"),
                    ),
                }
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("collect evidence: {e}"),
            ),
        }
    }

    fn get_status(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_agent_status_live() {
            Ok(status) => match serde_json::to_value(status) {
                Ok(v) => JsonRpcResponse::success(id, v),
                Err(e) => JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InternalError,
                    format!("serialize status: {e}"),
                ),
            },
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("collect status: {e}"),
            ),
        }
    }

    fn doctor(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");

        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let boot_target = crate::boot::BootTarget::parse(target);
                let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
                let semantic = crate::ai::semantic::analyze_semantic(&evidence);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "evidence": evidence,
                        "boot_report": boot_report,
                        "semantic": semantic,
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("doctor failed: {e}"),
            ),
        }
    }

    fn migrate_score(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("kvm");

        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let boot_target = crate::boot::BootTarget::parse(target);
                let boot_report = crate::boot::analyze_bootability(&evidence, boot_target);
                let report = crate::cli::migrate::plan::compute_migration_score(
                    &evidence,
                    &boot_report,
                    target,
                );
                JsonRpcResponse::success(id, serde_json::to_value(report).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::InternalError,
                format!("migrate score failed: {e}"),
            ),
        }
    }

    fn get_metrics(&self, id: Option<Value>) -> JsonRpcResponse {
        let metrics = crate::metrics::collect_metrics_live();
        JsonRpcResponse::success(id, serde_json::to_value(metrics).unwrap_or(json!({})))
    }

    fn get_filesystem(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::qga::filesystem_mounts_normalized() {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn exec(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let policy = crate::agent::policy::AgentPolicy::load();
        if !policy.shell_enabled() {
            return JsonRpcResponse::from_agent_error(
                id,
                AgentError::CapabilityDenied("shell exec disabled by policy".into()),
            );
        }
        match crate::agent::exec::exec_sync(params) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn enable_rdp(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::rdp::enable_rdp() {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn disable_rdp(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::rdp::disable_rdp() {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn run_fix_plan(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        if !self.capabilities.fix_apply {
            return JsonRpcResponse::from_agent_error(
                id,
                AgentError::CapabilityDenied("fix_apply not enabled".into()),
            );
        }

        let plan_value = match params.get("plan") {
            Some(v) => v.clone(),
            None => {
                return JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InvalidParams,
                    "missing required param: plan",
                )
            }
        };

        let plan: crate::cli::plan::FixPlan = match serde_json::from_value(plan_value) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InvalidParams,
                    format!("invalid plan: {e}"),
                )
            }
        };

        let dry_run = params
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let executor = crate::cli::plan::LivePlanExecutor::new(dry_run);
        match executor.apply(&plan) {
            Ok(result) => {
                let plan_id = format!(
                    "{}-{}",
                    plan.vm.replace('/', "_"),
                    plan.generated.timestamp()
                );
                JsonRpcResponse::success(
                    id,
                    json!({
                        "plan_id": plan_id,
                        "dry_run": dry_run,
                        "result": result,
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::PlanApplyFailed,
                format!("apply failed: {e}"),
            ),
        }
    }

    fn run_fix_plan_rollback(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let plan_id = match params.get("plan_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InvalidParams,
                    "missing required param: plan_id",
                )
            }
        };

        let executor = crate::cli::plan::LivePlanExecutor::new(false);
        match executor.rollback(plan_id) {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(
                id,
                RpcErrorCode::PlanApplyFailed,
                format!("rollback failed: {e}"),
            ),
        }
    }

    fn get_guest_health(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::health::build_guest_health_live() {
            Ok(health) => {
                JsonRpcResponse::success(id, serde_json::to_value(health).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_systemd_units(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let units = evidence
                    .systemd
                    .as_ref()
                    .and_then(|s| s.runtime.as_ref())
                    .map(|r| r.units.clone())
                    .unwrap_or_default();
                JsonRpcResponse::success(id, serde_json::to_value(units).unwrap_or(json!([])))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_failed_units(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let failed = crate::health::list_failed_units_from_evidence(&evidence);
                JsonRpcResponse::success(id, serde_json::to_value(failed).unwrap_or(json!([])))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_boot_analysis(&self, id: Option<Value>) -> JsonRpcResponse {
        let analysis = crate::boot::live::collect_boot_analysis();
        JsonRpcResponse::success(id, serde_json::to_value(analysis).unwrap_or(json!({})))
    }

    fn get_journal_slice(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let unit = params.get("unit").and_then(|v| v.as_str()).unwrap_or("");
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(200) as usize;
        let boot = params
            .get("boot")
            .and_then(|v| v.as_str())
            .unwrap_or("current");
        let slice = crate::journal::live::collect_journal_slice_boot(unit, limit, boot);
        JsonRpcResponse::success(id, serde_json::to_value(slice).unwrap_or(json!({})))
    }

    fn get_guest_info(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let info = crate::health::build_guest_info(&evidence);
                JsonRpcResponse::success(id, serde_json::to_value(info).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_systemd_unit(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let unit = params.get("unit").and_then(|v| v.as_str()).unwrap_or("");
        if unit.is_empty() {
            return JsonRpcResponse::error(id, RpcErrorCode::InvalidParams, "unit required");
        }
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                // NOTE: must stay .or_else (lazy) — the fallback makes a live D-Bus
                // call (get_unit_detail); .or() would run it on every request even
                // when build_service_health already found the unit from evidence.
                #[allow(clippy::unnecessary_lazy_evaluations)]
                let detail = crate::health::build_service_health(unit, &evidence).or_else(|| {
                    #[cfg(target_os = "linux")]
                    {
                        crate::collectors::dbus::get_unit_detail(unit).map(|u| {
                            guestkit_agent_protocol::ServiceHealth {
                                name: u.name,
                                state: u.active_state,
                                sub_state: u.sub_state,
                                main_pid: u.main_pid,
                                exit_code: u.exec_main_status,
                                restart_count: u.n_restarts,
                                last_failure: None,
                                journal_cursor: None,
                                actions: vec!["view_logs".into(), "restart_unit".into()],
                            }
                        })
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        None
                    }
                });
                JsonRpcResponse::success(id, serde_json::to_value(detail).unwrap_or(json!(null)))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_systemd_events(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        #[cfg(target_os = "linux")]
        {
            let cursor = params.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0);
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            let (next_cursor, events) = if cursor > 0 {
                crate::collectors::dbus::systemd_events::get_events_since(cursor)
            } else {
                let events = crate::collectors::dbus::systemd_events::recent_events(limit);
                let (c, _) = crate::collectors::dbus::systemd_events::get_events_since(0);
                (c, events)
            };
            JsonRpcResponse::success(
                id,
                json!({
                    "cursor": next_cursor,
                    "events": events,
                }),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = params;
            JsonRpcResponse::success(id, json!({ "cursor": 0, "events": [] }))
        }
    }

    fn get_processes(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::evidence::build_evidence_live() {
            Ok(evidence) => {
                let process = evidence
                    .process
                    .clone()
                    .unwrap_or_else(crate::collectors::process::collect_process_evidence);
                JsonRpcResponse::success(id, serde_json::to_value(process).unwrap_or(json!({})))
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn get_login_state(&self, id: Option<Value>) -> JsonRpcResponse {
        let state = crate::collectors::dbus::collect_login_state_safe();
        JsonRpcResponse::success(id, serde_json::to_value(state).unwrap_or(json!({})))
    }

    fn get_dns_state(&self, id: Option<Value>) -> JsonRpcResponse {
        let dns = crate::collectors::dbus::collect_dns_health_safe();
        JsonRpcResponse::success(id, serde_json::to_value(dns).unwrap_or(json!({})))
    }

    fn get_timedate_state(&self, id: Option<Value>) -> JsonRpcResponse {
        let timedate = crate::collectors::dbus::collect_timedate_health_safe();
        JsonRpcResponse::success(id, serde_json::to_value(timedate).unwrap_or(json!({})))
    }

    fn get_snapshot_readiness(&self, id: Option<Value>) -> JsonRpcResponse {
        let report = crate::agent::snapshot_hooks::build_snapshot_readiness_report();
        JsonRpcResponse::success(id, serde_json::to_value(report).unwrap_or(json!({})))
    }

    fn freeze_filesystem(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::snapshot_hooks::freeze_filesystems() {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn thaw_filesystem(&self, id: Option<Value>) -> JsonRpcResponse {
        match crate::agent::snapshot_hooks::thaw_filesystems() {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e),
        }
    }

    fn restart_unit(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let unit = match params.get("unit").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return JsonRpcResponse::error(
                    id,
                    RpcErrorCode::InvalidParams,
                    "missing required param: unit",
                )
            }
        };
        let executor = crate::agent::executor::Executor::new();
        match executor.restart_unit(unit) {
            Ok(msg) => JsonRpcResponse::success(id, json!({ "message": msg })),
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }

    fn execute_remediation_plan(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let plan_id = params
            .get("plan_id")
            .and_then(|v| v.as_str())
            .unwrap_or("local");
        let actions: Vec<crate::agent::executor::RemediationActionSpec> = params
            .get("actions")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let executor = crate::agent::executor::Executor::new();
        let result = executor.execute_remediation_plan(plan_id, &actions);
        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap_or(json!({})))
    }

    fn collect_support_bundle(&self, id: Option<Value>) -> JsonRpcResponse {
        let executor = crate::agent::executor::Executor::new();
        match executor.collect_support_bundle() {
            Ok(bytes) => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                JsonRpcResponse::success(
                    id,
                    json!({
                        "format": "tar.zst",
                        "encoding": "base64",
                        "data": STANDARD.encode(bytes),
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(id, RpcErrorCode::InternalError, e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_round_trip() {
        let handler = RequestHandler::new();
        let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.ping","id":1}"#);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn qga_guest_ping_round_trip() {
        let handler = RequestHandler::new();
        let raw = handler.handle_frame(br#"{"execute":"guest-ping"}"#);
        let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert!(v.get("return").is_some());
    }
}
