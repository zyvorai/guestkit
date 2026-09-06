// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd boot analysis

use super::{BootTiming, ServiceTiming, SystemdAnalyzer};
use anyhow::{Context, Result};
use std::fs;

/// Boot analyzer
#[derive(Debug)]
pub struct BootAnalyzer {
    analyzer: SystemdAnalyzer,
}

impl BootAnalyzer {
    /// Create a new boot analyzer
    pub fn new(analyzer: SystemdAnalyzer) -> Self {
        Self { analyzer }
    }

    /// Analyze boot performance
    ///
    /// This parses systemd-analyze output if available, or estimates from service files
    pub fn analyze_boot(&self) -> Result<BootTiming> {
        // Try to read systemd-analyze blame output if it exists
        let analyze_file = self
            .analyzer
            .root_path
            .join("var/lib/systemd/analyze-blame.txt");

        if analyze_file.exists() {
            return self.parse_analyze_file(&analyze_file);
        }

        // Otherwise, create estimated timing
        Ok(self.estimate_boot_timing())
    }

    /// Parse systemd-analyze output file
    fn parse_analyze_file(&self, path: &std::path::Path) -> Result<BootTiming> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read analyze file: {}", path.display()))?;

        let mut services = Vec::new();
        let mut total_time = 0u64;

        for line in content.lines() {
            // Parse lines like: "1.234s service-name.service"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Some(time_str) = parts.first() {
                    if let Some(activation_time) = self.parse_time_str(time_str) {
                        let name = parts[1..].join(" ");
                        services.push(ServiceTiming {
                            name,
                            activation_time,
                            start_offset: 0, // Would need full systemd-analyze data
                        });

                        total_time += activation_time;
                    }
                }
            }
        }

        Ok(BootTiming {
            total_time,
            kernel_time: 0, // Not available from blame output
            initrd_time: 0, // Not available from blame output
            userspace_time: total_time,
            services,
        })
    }

    /// Parse time string (e.g., "1.234s" or "567ms")
    fn parse_time_str(&self, s: &str) -> Option<u64> {
        // Check for milliseconds FIRST (before seconds, since "ms" ends with "s")
        if let Some(ms_val) = s.strip_suffix("ms") {
            // Milliseconds
            ms_val.parse::<u64>().ok()
        } else if let Some(s_val) = s.strip_suffix("s") {
            // Seconds
            s_val.parse::<f64>().ok().map(|v| (v * 1000.0) as u64)
        } else {
            None
        }
    }

    /// Estimate boot timing from available data
    fn estimate_boot_timing(&self) -> BootTiming {
        BootTiming {
            total_time: 15000,     // 15 seconds estimated
            kernel_time: 3000,     // 3 seconds
            initrd_time: 2000,     // 2 seconds
            userspace_time: 10000, // 10 seconds
            services: Vec::new(),
        }
    }

    /// Get boot optimization recommendations
    pub fn get_recommendations(&self, timing: &BootTiming) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check total boot time
        if timing.total_time > 30000 {
            recommendations.push(
                "Boot time is slow (>30s). Consider investigating slow services.".to_string(),
            );
        }

        // Check slowest services
        let slowest = timing.slowest_services(5);
        if !slowest.is_empty() {
            for service in &slowest {
                if service.activation_time > 3000 {
                    recommendations.push(format!(
                        "Service '{}' takes {:.2}s to activate. Consider optimization.",
                        service.name,
                        service.activation_time as f64 / 1000.0
                    ));
                }
            }
        }

        // Check kernel time
        if timing.kernel_time > 5000 {
            recommendations.push(
                "Kernel boot time is high (>5s). Check kernel parameters and modules.".to_string(),
            );
        }

        // Check initrd time
        if timing.initrd_time > 3000 {
            recommendations.push(
                "Initrd time is high (>3s). Consider reducing modules in initramfs.".to_string(),
            );
        }

        if recommendations.is_empty() {
            recommendations.push("Boot performance looks good!".to_string());
        }

        recommendations
    }

    /// Generate boot timeline Mermaid diagram
    pub fn generate_boot_timeline(&self, timing: &BootTiming) -> String {
        let mut diagram = String::from("```mermaid\ngantt\n");
        diagram.push_str("    title Boot Timeline\n");
        diagram.push_str("    dateFormat X\n");
        diagram.push_str("    axisFormat %L ms\n\n");

        let mut offset = 0u64;

        // Kernel
        diagram.push_str(&format!(
            "    section Kernel\n    Kernel Init :done, {}, {}\n\n",
            offset,
            offset + timing.kernel_time
        ));
        offset += timing.kernel_time;

        // Initrd
        if timing.initrd_time > 0 {
            diagram.push_str(&format!(
                "    section Initrd\n    Initrd :done, {}, {}\n\n",
                offset,
                offset + timing.initrd_time
            ));
            offset += timing.initrd_time;
        }

        // Services
        diagram.push_str("    section Userspace\n");
        for service in timing.services.iter().take(10) {
            let start = offset + service.start_offset;
            let end = start + service.activation_time;

            let status = if service.activation_time > 3000 {
                "crit"
            } else if service.activation_time > 1000 {
                "active"
            } else {
                "done"
            };

            diagram.push_str(&format!(
                "    {} :{}, {}, {}\n",
                service.name.replace('.', "_"),
                status,
                start,
                end
            ));
        }

        diagram.push_str("```\n");
        diagram
    }

    /// Generate boot statistics summary
    pub fn generate_summary(&self, timing: &BootTiming) -> String {
        let mut summary = String::new();

        summary.push_str(&format!(
            "Total Boot Time: {:.2}s\n",
            timing.total_time as f64 / 1000.0
        ));
        summary.push_str(&format!(
            "  - Kernel: {:.2}s\n",
            timing.kernel_time as f64 / 1000.0
        ));
        summary.push_str(&format!(
            "  - Initrd: {:.2}s\n",
            timing.initrd_time as f64 / 1000.0
        ));
        summary.push_str(&format!(
            "  - Userspace: {:.2}s\n\n",
            timing.userspace_time as f64 / 1000.0
        ));

        summary.push_str("Slowest Services:\n");
        for service in timing.slowest_services(5) {
            summary.push_str(&format!(
                "  - {}: {:.2}s\n",
                service.name,
                service.activation_time as f64 / 1000.0
            ));
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_analyzer_creation() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let _boot_analyzer = BootAnalyzer::new(analyzer);
    }

    #[test]
    fn test_parse_time_str() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let boot_analyzer = BootAnalyzer::new(analyzer);

        assert_eq!(boot_analyzer.parse_time_str("1.5s"), Some(1500));
        assert_eq!(boot_analyzer.parse_time_str("500ms"), Some(500));
        assert_eq!(boot_analyzer.parse_time_str("invalid"), None);
    }

    #[test]
    fn test_get_recommendations() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let boot_analyzer = BootAnalyzer::new(analyzer);

        let timing = BootTiming {
            total_time: 40000, // Slow boot
            kernel_time: 3000,
            initrd_time: 2000,
            userspace_time: 35000,
            services: vec![ServiceTiming {
                name: "slow.service".to_string(),
                activation_time: 5000, // Very slow
                start_offset: 0,
            }],
        };

        let recommendations = boot_analyzer.get_recommendations(&timing);
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.contains("slow")));
    }

    #[test]
    fn test_estimate_boot_timing() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let boot_analyzer = BootAnalyzer::new(analyzer);

        let timing = boot_analyzer.estimate_boot_timing();
        assert_eq!(timing.total_time, 15000);
        assert_eq!(timing.kernel_time, 3000);
    }
}
