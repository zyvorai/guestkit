// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Privileged executor sidecar (policy-gated remediation).

use anyhow::Result;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Zyvor guest agent executor starting");
    guestkit::agent::executor_ipc::spawn_executor_server()?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
