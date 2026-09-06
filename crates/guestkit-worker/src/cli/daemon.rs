// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Daemon command handler

use anyhow::Result;
use std::sync::Arc;
use crate::{
    Worker, WorkerConfig, HandlerRegistry,
    handlers::{
        AgentCallHandler, AgentDoctorHandler, AgentEvidenceHandler, AgentFixHandler, ConvertHandler, DoctorHandler,
        EchoHandler, InspectHandler,
        MigratePlanHandler, PassportHandler, ProfileHandler, RepairHandler,
    },
    transport::file::{FileTransport, FileTransportConfig},
    transport::http::{HttpTransport, HttpTransportConfig},
    transport::redis::{RedisTransport, RedisTransportConfig},
    capabilities::Capabilities,
    metrics::MetricsRegistry,
    metrics_server::{MetricsServer, MetricsServerConfig},
    api::server::{ApiServer, ApiServerConfig},
    api::handlers::ApiState,
};
use super::commands::DaemonArgs;

pub async fn run_daemon(args: DaemonArgs) -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
    ).init();

    log::info!("Starting guestkit worker daemon");

    // Worker configuration
    let config = WorkerConfig {
        worker_id: args.worker_id.clone().unwrap_or_else(|| format!("worker-{}", ulid::Ulid::new())),
        worker_pool: Some(args.pool.clone()),
        work_dir: args.work_dir.clone(),
        result_dir: args.results_dir.clone(),
        max_concurrent_jobs: args.max_concurrent,
        shutdown_timeout_secs: 30,
    };

    log::info!("Worker ID: {}", config.worker_id);
    log::info!("Working directory: {}", config.work_dir.display());
    log::info!("Results directory: {}", config.result_dir.display());

    std::fs::create_dir_all(&config.work_dir)
        .map_err(|e| anyhow::anyhow!("create work_dir {}: {e}", config.work_dir.display()))?;
    std::fs::create_dir_all(&config.result_dir)
        .map_err(|e| anyhow::anyhow!("create result_dir {}: {e}", config.result_dir.display()))?;

    // Setup handler registry
    let mut registry = HandlerRegistry::new();

    // Register built-in handlers
    registry.register(Arc::new(EchoHandler::new()));

    // Register guestkit operation handlers
    registry.register(Arc::new(InspectHandler::new()));
    registry.register(Arc::new(ProfileHandler::new()));
    registry.register(Arc::new(DoctorHandler));
    registry.register(Arc::new(MigratePlanHandler));
    registry.register(Arc::new(PassportHandler));
    registry.register(Arc::new(RepairHandler));
    registry.register(Arc::new(ConvertHandler));
    registry.register(Arc::new(AgentEvidenceHandler));
    registry.register(Arc::new(AgentDoctorHandler));
    registry.register(Arc::new(AgentCallHandler));
    registry.register(Arc::new(AgentFixHandler));

    log::info!("Registered {} operation handlers", registry.len());
    log::info!("Supported operations: {:?}", registry.operations());

    // Worker capabilities
    let capabilities = Capabilities::new()
        .with_operation("system.echo")
        .with_operation("test.echo")
        .with_operation("guestkit.inspect")
        .with_operation("guestkit.profile")
        .with_operation("guestkit.doctor")
        .with_operation("guestkit.migrate-plan")
        .with_operation("guestkit.passport")
        .with_operation("guestkit.repair")
        .with_operation("guestkit.agent.evidence")
        .with_operation("guestkit.agent.doctor")
        .with_operation("guestkit.agent.call")
        .with_operation("guestkit.agent.fix")
        .with_feature("rust")
        .with_feature("lvm")
        .with_feature("nbd")
        .with_disk_format("qcow2")
        .with_disk_format("vmdk")
        .with_disk_format("vdi")
        .with_disk_format("vhdx")
        .with_disk_format("raw");

    // Create metrics registry
    let metrics = Arc::new(MetricsRegistry::new());

    // Start metrics server if enabled
    let _metrics_handle = if args.metrics_enabled {
        let metrics_config = MetricsServerConfig {
            bind_addr: args.metrics_addr.parse()
                .expect("Invalid metrics address"),
        };

        let server = MetricsServer::new(metrics_config.clone(), Arc::clone(&metrics));
        let handle = server.start().await?;

        log::info!("Metrics server started on {}", metrics_config.bind_addr);
        log::info!("Metrics endpoint: http://{}/metrics", metrics_config.bind_addr);
        log::info!("Health endpoint: http://{}/health", metrics_config.bind_addr);

        Some(handle)
    } else {
        log::info!("Metrics server disabled");
        None
    };

    // Setup transport and API server based on mode
    match args.transport.as_str() {
        "redis" => {
            log::info!("Using Redis Streams transport");

            let redis_config = RedisTransportConfig {
                redis_url: args.redis_url.clone(),
                consumer_name: config.worker_id.clone(),
                result_dir: args.results_dir.clone(),
                block_ms: 2000,
            };

            let redis_transport = RedisTransport::new(redis_config).await?;

            let mut worker = Worker::new(
                config,
                capabilities,
                registry,
                Box::new(redis_transport),
            )?;

            worker.with_metrics(metrics);
            log::info!("Worker ready, waiting for jobs on Redis stream...");
            worker.run().await?;
        }
        "http" => {
            log::info!("Using HTTP transport with REST API");

            let http_transport = HttpTransport::new(HttpTransportConfig::default());

            // Start API server if enabled
            let _api_handle = if args.api_enabled {
                let api_config = ApiServerConfig {
                    bind_addr: args.api_addr.parse()
                        .expect("Invalid API address"),
                };

                let api_state = ApiState {
                    worker_id: config.worker_id.clone(),
                    capabilities: capabilities.clone(),
                    job_submitter: http_transport.get_submitter(),
                    job_status_lookup: http_transport.get_status_lookup(),
                };

                let server = ApiServer::new(api_config.clone(), api_state);
                let handle = server.start().await?;

                log::info!("REST API server started on {}", api_config.bind_addr);
                log::info!("API endpoints:");
                log::info!("  POST   http://{}/api/v1/jobs", api_config.bind_addr);
                log::info!("  GET    http://{}/api/v1/jobs", api_config.bind_addr);
                log::info!("  GET    http://{}/api/v1/jobs/:id", api_config.bind_addr);
                log::info!("  GET    http://{}/api/v1/jobs/:id/result", api_config.bind_addr);
                log::info!("  GET    http://{}/api/v1/capabilities", api_config.bind_addr);
                log::info!("  GET    http://{}/api/v1/health", api_config.bind_addr);

                Some(handle)
            } else {
                log::warn!("HTTP transport selected but API server disabled");
                None
            };

            // Create and run worker with HTTP transport
            let mut worker = Worker::new(
                config,
                capabilities,
                registry,
                Box::new(http_transport),
            )?;

            worker.with_metrics(metrics);

            log::info!("Worker ready, waiting for jobs...");
            worker.run().await?;
        },
        "file" | _ => {
            log::info!("Using file transport");

            let transport_config = FileTransportConfig {
                watch_dir: args.jobs_dir.clone(),
                done_dir: args.jobs_dir.join("done"),
                failed_dir: args.jobs_dir.join("failed"),
                poll_interval_secs: 2,
            };

            log::info!("Watching for jobs in: {}", transport_config.watch_dir.display());

            let file_transport = FileTransport::new(transport_config).await?;

            // Create and run worker with file transport
            let mut worker = Worker::new(
                config,
                capabilities,
                registry,
                Box::new(file_transport),
            )?;

            worker.with_metrics(metrics);

            log::info!("Worker ready, waiting for jobs...");
            worker.run().await?;
        }
    }

    log::info!("Worker shut down cleanly");

    Ok(())
}
