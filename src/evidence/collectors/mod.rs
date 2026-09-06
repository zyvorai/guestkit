// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Evidence collectors — populate typed slices of [`EvidenceSnapshot`].

pub mod systemd;
#[cfg(not(target_os = "windows"))]
pub mod windows;

#[cfg(feature = "agent")]
pub mod cloud_init_live;
#[cfg(feature = "agent")]
pub mod kubevirt_live;
#[cfg(feature = "agent")]
pub mod network_probes_live;
#[cfg(feature = "agent")]
pub mod snapshot_live;
#[cfg(feature = "agent")]
pub mod windows_live;

#[cfg(not(target_os = "windows"))]
pub use systemd::collect_systemd_guest;
pub use systemd::collect_systemd_live;
#[cfg(not(target_os = "windows"))]
pub use windows::collect_windows_details;

#[cfg(feature = "agent")]
pub use cloud_init_live::collect_cloud_init_live;
#[cfg(feature = "agent")]
pub use kubevirt_live::collect_kubevirt_live;
#[cfg(feature = "agent")]
pub use network_probes_live::collect_network_probes_live;
#[cfg(feature = "agent")]
pub use snapshot_live::collect_snapshot_readiness_live;
#[cfg(feature = "agent")]
pub use windows_live::collect_windows_live;
