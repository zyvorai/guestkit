// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Backend diagnostics for callers that want to log which engine and attach
//! mode handled a disk, and how long launch took.

use crate::core::Result;
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Diagnostic information about this GuestKit engine instance: which
    /// implementation is handling the disk, its version, whether it attached
    /// via NBD or a loop device, and whether the drive was opened read-only.
    pub fn get_backend_info(&self) -> Result<Vec<(String, String)>> {
        let attach_mode = if self.loop_device.is_some() {
            "loop"
        } else if self.nbd_device.is_some() {
            "nbd"
        } else {
            "none"
        };

        Ok(vec![
            ("implementation".to_string(), "guestkit".to_string()),
            ("version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
            ("attach_mode".to_string(), attach_mode.to_string()),
            ("readonly".to_string(), self.readonly.to_string()),
        ])
    }

    /// Measured performance counters for the most recent `launch()` call, in
    /// seconds. Empty until `launch()` has completed at least once.
    pub fn get_performance_metrics(&self) -> Result<Vec<(String, f64)>> {
        let mut metrics = Vec::new();
        if let Some(duration) = self.launch_duration {
            metrics.push(("launch_seconds".to_string(), duration.as_secs_f64()));
        }
        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_info_before_launch() {
        let g = Guestfs::new().unwrap();
        let info: std::collections::HashMap<_, _> =
            g.get_backend_info().unwrap().into_iter().collect();
        assert_eq!(
            info.get("implementation").map(String::as_str),
            Some("guestkit")
        );
        assert_eq!(info.get("attach_mode").map(String::as_str), Some("none"));
    }

    #[test]
    fn test_performance_metrics_empty_before_launch() {
        let g = Guestfs::new().unwrap();
        assert!(g.get_performance_metrics().unwrap().is_empty());
    }
}
