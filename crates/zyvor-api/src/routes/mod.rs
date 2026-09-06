// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

mod agent;
mod auth;
mod config;
mod copilot;
pub mod guest_agent;
mod health;
mod jobs;
mod packetwolf;
pub(crate) mod kubevirt;
mod settings;
mod storage;
mod system;
mod vmtools;
mod vms;

use axum::routing::{get, post, put};
use axum::Router;
use crate::state::AppState;

/// Guest-agent push endpoints (also served on the optional mTLS listener).
pub fn guest_agent_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/guest-agents/register",
            post(guest_agent::register_guest_agent),
        )
        .route(
            "/api/v1/guest-agents/bootstrap-info",
            get(guest_agent::guest_agent_bootstrap_info),
        )
        .route(
            "/api/v1/guest-agents/bootstrap",
            post(guest_agent::guest_agent_bootstrap_cert),
        )
        .route(
            "/api/v1/guest-agents/:agent_id/heartbeat",
            post(guest_agent::guest_agent_heartbeat),
        )
        .route(
            "/api/v1/guest-agents/:agent_id/report",
            post(guest_agent::guest_agent_report).get(guest_agent::get_guest_agent_report),
        )
}

/// mTLS-only push routes (heartbeat + report); registration/bootstrap stay on the main API port.
pub fn guest_agent_mtls_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/guest-agents/:agent_id/heartbeat",
            post(guest_agent::guest_agent_heartbeat),
        )
        .route(
            "/api/v1/guest-agents/:agent_id/report",
            post(guest_agent::guest_agent_report),
        )
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/health", get(health::health))
        .route("/api/v1/config", get(config::get_config))
        .route("/api/v1/auth/config", get(auth::public_config))
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/auth/logout", axum::routing::post(auth::logout))
        .route("/api/v1/auth/local", axum::routing::post(auth::local_login))
        .route("/api/v1/auth/oidc/login", get(auth::oidc_login))
        .route("/api/v1/auth/oidc/callback", get(auth::oidc_callback))
        .route("/api/v1/auth/saml/login", get(auth::saml_login))
        .route("/api/v1/auth/saml/acs", axum::routing::post(auth::saml_acs))
        .route("/api/v1/settings/identity", get(settings::get_identity).put(settings::put_identity))
        .route("/api/v1/settings/sso", get(settings::get_sso).put(settings::put_sso))
        .route(
            "/api/v1/settings/sso/saml/metadata",
            get(settings::saml_metadata),
        )
        .route("/api/v1/system/status", get(system::get_system_status))
        .route("/api/v1/storage/roots", get(storage::list_storage_roots))
        .route("/api/v1/storage/browse", get(storage::browse_storage))
        .route("/api/v1/vms/import-from-storage", post(storage::import_from_storage))
        .route("/api/v1/vmtools/bundle", get(vmtools::get_bundle))
        .route(
            "/api/v1/packetwolf/fleet-snapshot",
            get(packetwolf::get_fleet_snapshot),
        )
        .route(
            "/api/v1/packetwolf/fleet-correlate",
            post(packetwolf::trigger_fleet_correlation),
        )
        .route("/api/v1/vmtools/coverage", get(vmtools::get_coverage))
        .route("/api/v1/vmtools/policy", get(vmtools::get_policy).put(vmtools::put_policy))
        .route(
            "/api/v1/vmtools/policy/reconcile",
            post(vmtools::reconcile_policy),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools",
            get(vmtools::get_vm_vmtools),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/install",
            post(vmtools::install_vm_vmtools),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/diagnostics",
            post(vmtools::run_vm_diagnostics),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/quiesce",
            post(crate::kubevirt_vmtools_ops::quiesce_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/unquiesce",
            post(crate::kubevirt_vmtools_ops::unquiesce_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/reboot",
            post(crate::kubevirt_vmtools_ops::reboot_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/shutdown",
            post(crate::kubevirt_vmtools_ops::shutdown_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/vmtools/exec",
            post(crate::kubevirt_vmtools_ops::exec_vm_handler),
        )
        .route("/api/v1/vms/import", post(vms::import_vm))
        .route("/api/v1/vms/import-from-url", post(vms::import_from_url))
        .route("/api/v1/vms/import-from-s3", post(vms::import_from_s3))
        .route("/api/v1/vms/import-from-nfs", post(vms::import_from_nfs))
        .route("/api/v1/vms/compare", post(vms::compare_vms))
        .route("/api/v1/vms/cleanup-shadows", post(vms::cleanup_shadow_vms))
        .route("/api/v1/vms", get(vms::list_vms))
        .route("/api/v1/vms/:id", axum::routing::delete(vms::delete_vm))
        .route("/api/v1/vms/:id/jobs", get(vms::list_vm_jobs))
        .route("/api/v1/vms/:id/inspect", post(vms::inspect_vm))
        .route("/api/v1/vms/:id/doctor", post(vms::doctor_vm))
        .route("/api/v1/vms/:id/migration-plan", post(vms::migration_plan_vm))
        .route("/api/v1/vms/:id/passport", post(vms::passport_vm))
        .route("/api/v1/vms/:id/repair-plan", post(vms::repair_plan_vm))
        .route("/api/v1/vms/:id/convert", post(vms::convert_vm))
        .route("/api/v1/vms/:id/readiness-report", post(vms::readiness_report))
        .route("/api/v1/vms/:id/provision", post(vms::provision_vm))
        .route("/api/v1/jobs/:id", get(jobs::get_job))
        .route("/api/v1/vms/:id/copilot/ask", post(copilot::ask_copilot))
        .route("/api/v1/vms/:id/copilot/briefing", get(copilot::get_vm_briefing))
        .route("/api/v1/vms/:id/copilot/launch-advice", post(copilot::launch_advice))
        .route("/api/v1/vms/:id/copilot/explain-check", post(copilot::explain_check))
        .route("/api/v1/vms/compare/copilot", post(copilot::compare_copilot))
        .route("/api/v1/copilot/fleet-overview", post(copilot::fleet_overview))
        .route("/api/v1/vms/:id/agent/ping", post(agent::ping_agent))
        .route("/api/v1/vms/:id/agent/evidence", post(agent::agent_evidence))
        .route("/api/v1/vms/:id/agent/doctor", post(agent::agent_doctor))
        .route("/api/v1/vms/:id/agent/rpc", post(agent::agent_rpc))
        .route("/api/v1/vms/:id/agent/fix", post(agent::agent_fix))
        .merge(guest_agent_router())
        .route(
            "/api/v1/guest-actions/pending",
            get(crate::guest_actions::list_pending_guest_actions),
        )
        .route(
            "/api/v1/guest-actions/audit",
            get(crate::guest_actions::list_guest_action_audit),
        )
        .route(
            "/api/v1/guest-actions/:id/approve",
            post(crate::guest_actions::approve_guest_action),
        )
        .route(
            "/api/v1/guest-actions/:id/reject",
            post(crate::guest_actions::reject_guest_action),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/info",
            get(crate::kubevirt_guest_intel::get_guest_info),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/health",
            get(crate::kubevirt_guest_intel::get_guest_health),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/systemd",
            get(crate::kubevirt_guest_intel::get_guest_systemd),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/systemd/units/:unit",
            get(crate::kubevirt_guest_intel::get_guest_systemd_unit),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/journal",
            get(crate::kubevirt_guest_intel::get_guest_journal),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/evidence",
            get(crate::kubevirt_guest_intel::get_guest_evidence),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/network",
            get(crate::kubevirt_guest_intel::get_guest_network),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/processes",
            get(crate::kubevirt_guest_intel::get_guest_processes),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/systemd/events",
            get(crate::kubevirt_guest_intel::get_guest_systemd_events),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/actions/restart-unit",
            post(crate::kubevirt_guest_intel::post_restart_unit),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/logs",
            get(crate::kubevirt_guest_intel::get_guest_logs),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/actions/collect-support-bundle",
            post(crate::kubevirt_guest_intel::post_collect_support_bundle),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/actions/pre-snapshot-freeze",
            post(crate::kubevirt_guest_intel::post_pre_snapshot_freeze),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/actions/post-snapshot-thaw",
            post(crate::kubevirt_guest_intel::post_post_snapshot_thaw),
        )
        .route("/api/v1/kubevirt/namespaces", get(kubevirt::list_kubevirt_namespaces))
        .route("/api/v1/kubevirt/vms", get(kubevirt::list_kubevirt_vms))
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/status",
            get(crate::guest_control::routes::get_guest_status),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/capabilities",
            get(crate::guest_control::routes::get_guest_capabilities),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/doctor",
            get(crate::guest_control::routes::get_guest_doctor)
                .post(crate::guest_control::routes::post_guest_doctor),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/readiness",
            get(crate::guest_control::routes::get_guest_readiness),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/install-agent",
            post(crate::guest_control::routes::post_install_agent),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/repair-plan",
            post(crate::guest_control::routes::post_guest_repair_plan),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/file/read",
            post(crate::guest_control::routes::post_guest_file_read),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/file/write",
            post(crate::guest_control::routes::post_guest_file_write),
        )
        .route(
            "/api/v1/kubevirt/guest/poll-reconcile",
            post(crate::guest_control::routes::post_guest_poll_reconcile),
        )
        .route(
            "/api/v1/kubevirt/guest/poll-telemetry",
            get(crate::guest_control::routes::get_guest_poll_fleet_telemetry),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest/poll-telemetry",
            get(crate::guest_control::routes::get_guest_poll_telemetry),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest-agent",
            get(kubevirt::get_guest_agent_info),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/guest-agent/install",
            post(kubevirt::install_guest_agent),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/boot-inspect",
            get(crate::kubevirt_boot_inspect::get_boot_inspect)
                .post(crate::kubevirt_boot_inspect::post_boot_inspect_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/inspect/boot",
            get(crate::kubevirt_boot_inspect::get_boot_inspect)
                .post(crate::kubevirt_boot_inspect::post_boot_inspect_vm),
        )
        .route(
            "/api/v1/kubevirt/boot-inspect",
            post(crate::kubevirt_boot_inspect::post_boot_inspect),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/restart",
            put(crate::kubevirt_lifecycle::restart_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/start",
            put(crate::kubevirt_lifecycle::start_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/stop",
            put(crate::kubevirt_lifecycle::stop_vm_handler),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/inspect",
            post(crate::kubevirt_inspect::post_inspect_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/doctor",
            post(crate::kubevirt_inspect::post_doctor_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/migration-plan",
            post(crate::kubevirt_inspect::post_migration_plan_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/repair-plan",
            post(crate::kubevirt_inspect::post_repair_plan_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/provision",
            post(crate::kubevirt_inspect::post_provision_vm),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/export-disk",
            post(crate::kubevirt_export::export_vm_disk),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/copilot/briefing",
            post(crate::kubevirt_copilot::cluster_briefing),
        )
        .route(
            "/api/v1/kubevirt/vms/:namespace/:name/copilot/ask",
            post(crate::kubevirt_copilot::cluster_ask),
        )
        .route(
            "/api/v1/kubevirt/apply",
            post(crate::kubevirt_apply::apply_yaml_handler),
        )
}
