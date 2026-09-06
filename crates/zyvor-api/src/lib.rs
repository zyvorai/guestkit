// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Zyvor VM Services API library

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod guest_agent_ca;
pub mod guest_agent_mtls;
pub mod guest_agent_vm;
pub mod guest_remediation_auth;
pub mod guest_actions;
pub mod guest_action_policy;
pub mod guest_control;
pub mod kubevirt_guest_pull;
pub mod kubevirt_apply;
pub mod kubevirt_boot_inspect;
pub mod kubevirt_copilot;
pub mod kubevirt_export;
pub mod kubevirt_inspect;
pub mod kubevirt_guest_agent;
pub mod kubevirt_guest_cr;
pub mod kubevirt_guest_intel;
pub mod kubevirt_qga;
pub mod qga_socket;
pub mod kubevirt_lifecycle;
pub mod kubevirt_vmtools_ops;
pub mod packetwolf_correlate;
pub mod packetwolf_fleet;
pub mod vmtools_bundle;
pub mod jobs;
pub mod models;
pub mod routes;
pub mod state;

use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use auth::middleware::require_auth;
use config::Config;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub async fn serve(config: Config) -> Result<()> {
    config.emit_startup_warnings();

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    db::migrate(&pool).await?;

    if let Err(e) = auth::store::bootstrap_from_env(&pool, &config).await {
        tracing::warn!("auth settings bootstrap skipped: {e}");
    }

    let redis = redis::Client::open(config.redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis).await?;

    tokio::fs::create_dir_all(&config.storage_path).await?;

    let kube = match kube::Client::try_default().await {
        Ok(client) => {
            tracing::info!("kubernetes client initialized for KubeVirt discovery");
            Some(client)
        }
        Err(e) => {
            tracing::warn!("kubernetes client unavailable: {e}");
            None
        }
    };

    let state = AppState {
        pool,
        redis: redis_conn,
        config: config.clone(),
        kube,
    };

    let max_upload = config.max_upload_bytes;

    let app = routes::api_router()
        .layer(from_fn_with_state(state.clone(), require_auth))
        .layer(DefaultBodyLimit::max(max_upload))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    if let Some(mtls_bind) = config.agent_mtls_bind_addr.clone() {
        let mtls_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = guest_agent_mtls::serve(&mtls_bind, mtls_state).await {
                tracing::error!("guest-agent mTLS listener stopped: {e}");
            }
        });
    }

    packetwolf_fleet::spawn_fleet_worker(
        config.clone(),
        state.kube.clone(),
        state.redis.clone(),
    );

    guest_control::polling::spawn_airgap_poll_worker(state.clone());

    let addr: SocketAddr = config.bind_addr.parse()?;
    tracing::info!("zyvor-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
