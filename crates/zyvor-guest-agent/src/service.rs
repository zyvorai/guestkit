// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use std::ffi::OsString;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "GuestKitAgent";
/// Pre-rebrand service name, removed by the MSI on upgrade.
#[allow(dead_code)]
const SERVICE_NAME_LEGACY: &str = "ZyvorGuestAgent";

define_windows_service!(ffi_service_main, service_main);

pub fn run_service() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_impl() {
        log::error!("service failed: {e:#}");
    }
}

fn run_service_impl() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        // Answer the QEMU guest-agent virtio-serial channel by default, so the
        // Windows service is reachable by host integrations (virsh
        // qemu-agent-command / KubeVirt) exactly like the Linux agent. Override
        // with GUESTKIT_AGENT_CHANNEL=stdio|virtio for other transports.
        let channel = match std::env::var("GUESTKIT_AGENT_CHANNEL").ok().as_deref() {
            Some("stdio") => guestkit::agent::AgentChannel::Stdio,
            _ => guestkit::agent::AgentChannel::Virtio,
        };
        if let Err(e) = rt.block_on(guestkit::agent::run_agent(guestkit::agent::AgentArgs {
            channel,
            device: None,
            socket: None,
            user: None,
        })) {
            log::error!("agent daemon: {e:#}");
        }
    });

    let _ = shutdown_rx.recv();
    Ok(())
}
