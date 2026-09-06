// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTML export functionality with Chart.js visualizations
//!
//! This module generates interactive HTML reports with charts and graphs
//! for VM inspection results.

use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Escape HTML special characters to prevent XSS
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// HTML export options
#[derive(Debug, Clone)]
pub struct HtmlExportOptions {
    /// Include Chart.js visualizations
    pub include_charts: bool,
    /// Include CSS styling
    pub include_styles: bool,
    /// Use dark theme
    pub dark_theme: bool,
    /// Include table of contents
    pub include_toc: bool,
    /// Responsive design
    pub responsive: bool,
}

impl Default for HtmlExportOptions {
    fn default() -> Self {
        Self {
            include_charts: true,
            include_styles: true,
            dark_theme: false,
            include_toc: true,
            responsive: true,
        }
    }
}

/// HTML exporter for inspection results
pub struct HtmlExporter {
    options: HtmlExportOptions,
}

impl HtmlExporter {
    /// Create a new HTML exporter with default options
    pub fn new() -> Self {
        Self {
            options: HtmlExportOptions::default(),
        }
    }

    /// Create a new HTML exporter with custom options
    pub fn with_options(options: HtmlExportOptions) -> Self {
        Self { options }
    }

    /// Generate HTML report from inspection data
    pub fn generate<P: AsRef<Path>>(
        &self,
        output_path: P,
        data: &InspectionData,
    ) -> std::io::Result<()> {
        let html = self.build_html(data);
        let mut file = File::create(output_path)?;
        file.write_all(html.as_bytes())?;
        Ok(())
    }

    /// Build complete HTML document
    fn build_html(&self, data: &InspectionData) -> String {
        let mut html = String::new();

        // HTML header
        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html lang=\"en\">\n");
        html.push_str("<head>\n");
        html.push_str("    <meta charset=\"UTF-8\">\n");
        html.push_str(
            "    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        html.push_str(&format!(
            "    <title>VM Inspection Report - {}</title>\n",
            html_escape(&data.hostname)
        ));

        // Include styles
        if self.options.include_styles {
            html.push_str("    <style>\n");
            html.push_str(&self.get_css());
            html.push_str("    </style>\n");
        }

        // Include Chart.js
        if self.options.include_charts {
            html.push_str("    <script src=\"https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js\"></script>\n");
        }

        html.push_str("</head>\n");
        html.push_str("<body");
        if self.options.dark_theme {
            html.push_str(" class=\"dark-theme\"");
        }
        html.push_str(">\n");

        // Header
        html.push_str(&self.build_header(data));

        // Table of contents
        if self.options.include_toc {
            html.push_str(&self.build_toc());
        }

        // Main content
        html.push_str("    <div class=\"container\">\n");

        // Overview section
        html.push_str(&self.build_overview_section(data));

        // System section
        html.push_str(&self.build_system_section(data));

        // Filesystems section
        html.push_str(&self.build_filesystems_section(data));

        // Packages section
        html.push_str(&self.build_packages_section(data));

        // Users section
        html.push_str(&self.build_users_section(data));

        // Network section
        html.push_str(&self.build_network_section(data));

        html.push_str("    </div>\n");

        // Include chart scripts
        if self.options.include_charts {
            html.push_str(&self.build_chart_scripts(data));
        }

        // Footer
        html.push_str(&self.build_footer());

        html.push_str("</body>\n");
        html.push_str("</html>\n");

        html
    }

    /// Get CSS styles
    fn get_css(&self) -> String {
        let base_css = include_str!("../templates/report.css");

        if self.options.dark_theme {
            format!("{}\n{}", base_css, self.get_dark_theme_css())
        } else {
            base_css.to_string()
        }
    }

    /// Get dark theme CSS
    fn get_dark_theme_css(&self) -> String {
        r#"
        .dark-theme {
            background-color: #1a1a1a;
            color: #e0e0e0;
        }
        .dark-theme .card {
            background-color: #2d2d2d;
            border-color: #404040;
        }
        .dark-theme table {
            color: #e0e0e0;
        }
        .dark-theme table th {
            background-color: #404040;
        }
        .dark-theme table tr:hover {
            background-color: #353535;
        }
        "#
        .to_string()
    }

    /// Build header section
    fn build_header(&self, data: &InspectionData) -> String {
        format!(
            r#"    <header class="header">
        <h1>🖥️ VM Inspection Report</h1>
        <h2>{} ({})</h2>
        <p class="timestamp">Generated: {}</p>
    </header>
"#,
            html_escape(&data.hostname),
            html_escape(&data.os_type),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }

    /// Build table of contents
    fn build_toc(&self) -> String {
        r##"    <nav class="toc">
        <h3>Table of Contents</h3>
        <ul>
            <li><a href="#overview">Overview</a></li>
            <li><a href="#system">System Information</a></li>
            <li><a href="#filesystems">Filesystems</a></li>
            <li><a href="#packages">Installed Packages</a></li>
            <li><a href="#users">User Accounts</a></li>
            <li><a href="#network">Network Configuration</a></li>
        </ul>
    </nav>
"##
        .to_string()
    }

    /// Build overview section
    fn build_overview_section(&self, data: &InspectionData) -> String {
        let mut html = String::new();
        html.push_str(
            r#"        <section id="overview" class="section">
            <h2>📊 Overview</h2>
            <div class="stats-grid">
"#,
        );

        html.push_str(&format!(
            r#"                <div class="stat-card">
                    <h3>OS Type</h3>
                    <p class="stat-value">{}</p>
                </div>
                <div class="stat-card">
                    <h3>Filesystems</h3>
                    <p class="stat-value">{}</p>
                </div>
                <div class="stat-card">
                    <h3>Packages</h3>
                    <p class="stat-value">{}</p>
                </div>
                <div class="stat-card">
                    <h3>Users</h3>
                    <p class="stat-value">{}</p>
                </div>
"#,
            html_escape(&data.os_type),
            data.filesystems.len(),
            data.packages.len(),
            data.users.len()
        ));

        html.push_str("            </div>\n");

        if self.options.include_charts {
            html.push_str(
                r#"            <div class="chart-container">
                <canvas id="overviewChart"></canvas>
            </div>
"#,
            );
        }

        html.push_str("        </section>\n");
        html
    }

    /// Build system information section
    fn build_system_section(&self, data: &InspectionData) -> String {
        format!(
            r#"        <section id="system" class="section">
            <h2>💻 System Information</h2>
            <div class="info-table">
                <table>
                    <tr><th>Property</th><th>Value</th></tr>
                    <tr><td>Hostname</td><td>{}</td></tr>
                    <tr><td>OS Type</td><td>{}</td></tr>
                    <tr><td>Distribution</td><td>{}</td></tr>
                    <tr><td>Version</td><td>{}</td></tr>
                    <tr><td>Architecture</td><td>{}</td></tr>
                    <tr><td>Package Format</td><td>{}</td></tr>
                    <tr><td>Package Manager</td><td>{}</td></tr>
                </table>
            </div>
        </section>
"#,
            html_escape(&data.hostname),
            html_escape(&data.os_type),
            html_escape(&data.distribution),
            html_escape(&data.version),
            html_escape(&data.architecture),
            html_escape(&data.package_format),
            html_escape(&data.package_manager)
        )
    }

    /// Build filesystems section
    fn build_filesystems_section(&self, data: &InspectionData) -> String {
        let mut html = String::new();
        html.push_str(
            r#"        <section id="filesystems" class="section">
            <h2>💾 Filesystems</h2>
            <div class="info-table">
                <table>
                    <thead>
                        <tr>
                            <th>Device</th>
                            <th>Mountpoint</th>
                            <th>Type</th>
                            <th>Size</th>
                            <th>Used</th>
                            <th>Available</th>
                        </tr>
                    </thead>
                    <tbody>
"#,
        );

        for fs in &data.filesystems {
            html.push_str(&format!(
                r#"                        <tr>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                        </tr>
"#,
                html_escape(&fs.device),
                html_escape(&fs.mountpoint),
                html_escape(&fs.fstype),
                format_bytes(fs.size),
                format_bytes(fs.used),
                format_bytes(fs.available)
            ));
        }

        html.push_str(
            r#"                    </tbody>
                </table>
            </div>
"#,
        );

        if self.options.include_charts {
            html.push_str(
                r#"            <div class="chart-container">
                <canvas id="filesystemChart"></canvas>
            </div>
"#,
            );
        }

        html.push_str("        </section>\n");
        html
    }

    /// Build packages section
    fn build_packages_section(&self, data: &InspectionData) -> String {
        let mut html = String::new();
        html.push_str(&format!(
            r#"        <section id="packages" class="section">
            <h2>📦 Installed Packages</h2>
            <p>Total packages: {}</p>
            <div class="info-table">
                <table>
                    <thead>
                        <tr>
                            <th>Package</th>
                            <th>Version</th>
                            <th>Architecture</th>
                        </tr>
                    </thead>
                    <tbody>
"#,
            data.packages.len()
        ));

        // Show first 50 packages
        for pkg in data.packages.iter().take(50) {
            html.push_str(&format!(
                r#"                        <tr>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                        </tr>
"#,
                html_escape(&pkg.name),
                html_escape(&pkg.version),
                html_escape(&pkg.arch)
            ));
        }

        if data.packages.len() > 50 {
            html.push_str(&format!(
                r#"                        <tr>
                            <td colspan="3" class="text-center"><em>... and {} more packages</em></td>
                        </tr>
"#,
                data.packages.len() - 50
            ));
        }

        html.push_str(
            r#"                    </tbody>
                </table>
            </div>
        </section>
"#,
        );
        html
    }

    /// Build users section
    fn build_users_section(&self, data: &InspectionData) -> String {
        let mut html = String::new();
        html.push_str(
            r#"        <section id="users" class="section">
            <h2>👤 User Accounts</h2>
            <div class="info-table">
                <table>
                    <thead>
                        <tr>
                            <th>Username</th>
                            <th>UID</th>
                            <th>Home Directory</th>
                            <th>Shell</th>
                        </tr>
                    </thead>
                    <tbody>
"#,
        );

        for user in &data.users {
            html.push_str(&format!(
                r#"                        <tr>
                            <td>{}</td>
                            <td>{}</td>
                            <td><code>{}</code></td>
                            <td><code>{}</code></td>
                        </tr>
"#,
                html_escape(&user.username),
                html_escape(&user.uid),
                html_escape(&user.home),
                html_escape(&user.shell)
            ));
        }

        html.push_str(
            r#"                    </tbody>
                </table>
            </div>
        </section>
"#,
        );
        html
    }

    /// Build network section
    fn build_network_section(&self, data: &InspectionData) -> String {
        if data.interfaces.is_empty() {
            return r#"        <section id="network" class="section">
            <h2>🌐 Network Configuration</h2>
            <p><em>No network information available</em></p>
        </section>
"#
            .to_string();
        }

        let mut html = String::new();
        html.push_str(
            r#"        <section id="network" class="section">
            <h2>🌐 Network Configuration</h2>
            <div class="info-table">
                <table>
                    <thead>
                        <tr>
                            <th>Interface</th>
                            <th>IP Address</th>
                            <th>MAC Address</th>
                            <th>Status</th>
                        </tr>
                    </thead>
                    <tbody>
"#,
        );

        for iface in &data.interfaces {
            html.push_str(&format!(
                r#"                        <tr>
                            <td>{}</td>
                            <td><code>{}</code></td>
                            <td><code>{}</code></td>
                            <td><span class="status-badge">{}</span></td>
                        </tr>
"#,
                html_escape(&iface.name),
                html_escape(&iface.ip_addresses),
                html_escape(&iface.mac_address),
                html_escape(&iface.state)
            ));
        }

        html.push_str(
            r#"                    </tbody>
                </table>
            </div>
        </section>
"#,
        );
        html
    }

    /// Build Chart.js scripts
    fn build_chart_scripts(&self, data: &InspectionData) -> String {
        format!(
            r#"    <script>
        // Overview chart
        const overviewCtx = document.getElementById('overviewChart');
        if (overviewCtx) {{
            new Chart(overviewCtx, {{
                type: 'bar',
                data: {{
                    labels: ['Filesystems', 'Packages', 'Users', 'Network Interfaces'],
                    datasets: [{{
                        label: 'Count',
                        data: [{}, {}, {}, {}],
                        backgroundColor: [
                            'rgba(54, 162, 235, 0.5)',
                            'rgba(255, 206, 86, 0.5)',
                            'rgba(75, 192, 192, 0.5)',
                            'rgba(153, 102, 255, 0.5)'
                        ],
                        borderColor: [
                            'rgba(54, 162, 235, 1)',
                            'rgba(255, 206, 86, 1)',
                            'rgba(75, 192, 192, 1)',
                            'rgba(153, 102, 255, 1)'
                        ],
                        borderWidth: 1
                    }}]
                }},
                options: {{
                    responsive: true,
                    plugins: {{
                        title: {{
                            display: true,
                            text: 'System Components Overview'
                        }}
                    }},
                    scales: {{
                        y: {{
                            beginAtZero: true
                        }}
                    }}
                }}
            }});
        }}

        // Filesystem chart
        const fsCtx = document.getElementById('filesystemChart');
        if (fsCtx) {{
            new Chart(fsCtx, {{
                type: 'doughnut',
                data: {{
                    labels: {},
                    datasets: [{{
                        label: 'Filesystem Size',
                        data: {},
                        backgroundColor: [
                            'rgba(255, 99, 132, 0.5)',
                            'rgba(54, 162, 235, 0.5)',
                            'rgba(255, 206, 86, 0.5)',
                            'rgba(75, 192, 192, 0.5)',
                            'rgba(153, 102, 255, 0.5)'
                        ]
                    }}]
                }},
                options: {{
                    responsive: true,
                    plugins: {{
                        title: {{
                            display: true,
                            text: 'Filesystem Distribution'
                        }}
                    }}
                }}
            }});
        }}
    </script>
"#,
            data.filesystems.len(),
            data.packages.len(),
            data.users.len(),
            data.interfaces.len(),
            serde_json::to_string(
                &data
                    .filesystems
                    .iter()
                    .map(|fs| &fs.device)
                    .collect::<Vec<_>>()
            )
            .unwrap_or_else(|_| "[]".to_string()),
            serde_json::to_string(
                &data
                    .filesystems
                    .iter()
                    .map(|fs| fs.size)
                    .collect::<Vec<_>>()
            )
            .unwrap_or_else(|_| "[]".to_string())
        )
    }

    /// Build footer
    fn build_footer(&self) -> String {
        format!(
            r#"    <footer class="footer">
        <p>Generated by <strong>guestkit</strong> - VM Inspection Tool</p>
        <p>Report generated at {}</p>
    </footer>
"#,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }
}

impl Default for HtmlExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Inspection data structure for HTML export
#[derive(Debug, Clone)]
pub struct InspectionData {
    pub hostname: String,
    pub os_type: String,
    pub distribution: String,
    pub version: String,
    pub architecture: String,
    pub product_name: String,
    pub package_format: String,
    pub package_manager: String,
    pub kernel_version: Option<String>,
    pub total_memory: Option<u64>,
    pub vcpus: Option<u32>,
    pub filesystems: Vec<FilesystemInfo>,
    pub packages: Vec<PackageInfo>,
    pub users: Vec<UserInfo>,
    pub interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Clone)]
pub struct FilesystemInfo {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    pub size: i64,
    pub used: i64,
    pub available: i64,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub username: String,
    pub uid: String,
    pub home: String,
    pub shell: String,
}

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: String,
    pub ip_addresses: String,
    pub state: String,
}

/// Format bytes to human-readable string
fn format_bytes(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = if bytes < 0 { 0.0 } else { bytes as f64 };
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_exporter_creation() {
        let exporter = HtmlExporter::new();
        assert!(exporter.options.include_charts);
        assert!(exporter.options.include_styles);
    }

    #[test]
    fn test_html_export_options_default() {
        let options = HtmlExportOptions::default();
        assert!(options.include_charts);
        assert!(options.include_styles);
        assert!(!options.dark_theme);
        assert!(options.include_toc);
        assert!(options.responsive);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_inspection_data_creation() {
        let data = InspectionData {
            hostname: "test-vm".to_string(),
            os_type: "linux".to_string(),
            distribution: "ubuntu".to_string(),
            version: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            product_name: "Ubuntu 22.04".to_string(),
            package_format: "deb".to_string(),
            package_manager: "apt".to_string(),
            kernel_version: Some("5.15.0-generic".to_string()),
            total_memory: Some(8 * 1024 * 1024 * 1024),
            vcpus: Some(4),
            filesystems: vec![],
            packages: vec![],
            users: vec![],
            interfaces: vec![],
        };

        assert_eq!(data.hostname, "test-vm");
        assert_eq!(data.os_type, "linux");
        assert_eq!(data.version, "22.04");
    }
}
