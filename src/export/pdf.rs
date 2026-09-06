// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! PDF report generation with professional layout

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// PDF export options
#[derive(Debug, Clone)]
pub struct PdfExportOptions {
    /// Include page numbers
    pub include_page_numbers: bool,

    /// Include table of contents
    pub include_toc: bool,

    /// Use color (vs black and white)
    pub use_color: bool,

    /// Paper size (A4, Letter, etc.)
    pub paper_size: PaperSize,

    /// Font size for body text
    pub font_size: f32,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            include_page_numbers: true,
            include_toc: true,
            use_color: true,
            paper_size: PaperSize::A4,
            font_size: 12.0,
        }
    }
}

/// Paper size options
#[derive(Debug, Clone, Copy)]
pub enum PaperSize {
    A4,
    Letter,
    Legal,
}

impl PaperSize {
    fn to_mm(self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
        }
    }
}

/// Inspection data for PDF export
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

/// Filesystem information
#[derive(Debug, Clone)]
pub struct FilesystemInfo {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    pub size: u64,
    pub used: u64,
    pub available: u64,
}

/// Package information
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
}

/// User information
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub username: String,
    pub uid: String,
    pub home: String,
    pub shell: String,
}

/// Network interface information
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: String,
    pub ip_addresses: Vec<String>,
    pub state: String,
}

/// PDF report exporter
pub struct PdfExporter {
    options: PdfExportOptions,
}

impl PdfExporter {
    /// Create a new PDF exporter with default options
    pub fn new(options: PdfExportOptions) -> Self {
        Self { options }
    }

    /// Generate PDF report
    pub fn generate<P: AsRef<Path>>(
        &self,
        output_path: P,
        data: &InspectionData,
    ) -> std::io::Result<()> {
        let (width_mm, height_mm) = self.options.paper_size.to_mm();

        let mut doc = PdfDocument::new(&format!("VM Inspection Report - {}", data.hostname));

        let mut ops: Vec<Op> = vec![
            Op::StartTextSection,
            Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    icc_profile: None,
                }),
            },
        ];

        // Add title
        self.push_text(
            &mut ops,
            "VM Inspection Report",
            self.options.font_size + 8.0,
            20.0,
            height_mm - 30.0,
            true,
        );

        let subtitle = format!(
            "{} - {}",
            data.hostname,
            chrono::Local::now().format("%Y-%m-%d")
        );
        self.push_text(
            &mut ops,
            &subtitle,
            self.options.font_size,
            20.0,
            height_mm - 40.0,
            false,
        );

        // Current Y position for content
        let mut y_pos = height_mm - 60.0;

        // System Information Section
        y_pos = self.add_section_header(&mut ops, "System Information", y_pos);
        y_pos = self.add_text_line(&mut ops, &format!("OS Type: {}", data.os_type), y_pos);
        y_pos = self.add_text_line(
            &mut ops,
            &format!("Distribution: {}", data.distribution),
            y_pos,
        );
        y_pos = self.add_text_line(&mut ops, &format!("Version: {}", data.version), y_pos);
        y_pos = self.add_text_line(
            &mut ops,
            &format!("Architecture: {}", data.architecture),
            y_pos,
        );
        y_pos = self.add_text_line(&mut ops, &format!("Product: {}", data.product_name), y_pos);
        y_pos = self.add_text_line(
            &mut ops,
            &format!("Package Format: {}", data.package_format),
            y_pos,
        );
        y_pos = self.add_text_line(
            &mut ops,
            &format!("Package Manager: {}", data.package_manager),
            y_pos,
        );

        if let Some(kernel) = &data.kernel_version {
            y_pos = self.add_text_line(&mut ops, &format!("Kernel: {}", kernel), y_pos);
        }

        if let Some(mem) = data.total_memory {
            y_pos = self.add_text_line(
                &mut ops,
                &format!("Memory: {} GB", mem / 1024 / 1024 / 1024),
                y_pos,
            );
        }

        if let Some(vcpus) = data.vcpus {
            y_pos = self.add_text_line(&mut ops, &format!("vCPUs: {}", vcpus), y_pos);
        }

        y_pos -= 10.0;

        // Filesystems Section
        if !data.filesystems.is_empty() {
            y_pos = self.add_section_header(&mut ops, "Filesystems", y_pos);

            for fs in data.filesystems.iter().take(10) {
                let fs_text = format!(
                    "{} -> {} ({}) - {:.1} GB / {:.1} GB",
                    fs.device,
                    fs.mountpoint,
                    fs.fstype,
                    fs.used as f64 / 1024.0 / 1024.0 / 1024.0,
                    fs.size as f64 / 1024.0 / 1024.0 / 1024.0,
                );
                y_pos = self.add_text_line(&mut ops, &fs_text, y_pos);

                // Check if we need a new page
                if y_pos < 30.0 {
                    break;
                }
            }

            y_pos -= 10.0;
        }

        // Packages Section (show count)
        if !data.packages.is_empty() {
            y_pos = self.add_section_header(&mut ops, "Installed Packages", y_pos);
            y_pos = self.add_text_line(
                &mut ops,
                &format!("Total packages: {}", data.packages.len()),
                y_pos,
            );

            // Show first 20 packages
            let packages_to_show = data.packages.iter().take(20);
            for pkg in packages_to_show {
                let pkg_text = format!("{} - {} ({})", pkg.name, pkg.version, pkg.arch);
                y_pos = self.add_text_line(&mut ops, &pkg_text, y_pos);

                if y_pos < 30.0 {
                    break;
                }
            }

            if data.packages.len() > 20 {
                y_pos = self.add_text_line(
                    &mut ops,
                    &format!("... and {} more packages", data.packages.len() - 20),
                    y_pos,
                );
            }

            y_pos -= 10.0;
        }

        // Users Section
        if !data.users.is_empty() {
            y_pos = self.add_section_header(&mut ops, "User Accounts", y_pos);

            for user in data.users.iter().take(15) {
                let user_text = format!(
                    "{} (UID: {}) - {} [{}]",
                    user.username, user.uid, user.home, user.shell,
                );
                y_pos = self.add_text_line(&mut ops, &user_text, y_pos);

                if y_pos < 30.0 {
                    break;
                }
            }

            y_pos -= 10.0;
        }

        // Network Interfaces Section
        if !data.interfaces.is_empty() {
            y_pos = self.add_section_header(&mut ops, "Network Interfaces", y_pos);

            for iface in &data.interfaces {
                let iface_text = format!(
                    "{} - {} [{}] - {}",
                    iface.name,
                    iface.mac_address,
                    iface.ip_addresses.join(", "),
                    iface.state,
                );
                y_pos = self.add_text_line(&mut ops, &iface_text, y_pos);

                if y_pos < 30.0 {
                    break;
                }
            }
        }

        // Add footer with page number if enabled
        if self.options.include_page_numbers {
            self.push_text(
                &mut ops,
                "Page 1",
                self.options.font_size - 2.0,
                width_mm / 2.0 - 10.0,
                15.0,
                false,
            );
        }

        ops.push(Op::EndTextSection);

        let page = PdfPage::new(Mm(width_mm), Mm(height_mm), ops);
        doc.with_pages(vec![page]);

        let mut warnings = Vec::new();
        doc.save_writer(
            &mut BufWriter::new(File::create(output_path)?),
            &PdfSaveOptions::default(),
            &mut warnings,
        );

        Ok(())
    }

    /// Push a `SetFont` + `SetTextCursor` + `ShowText` op sequence for one line of text,
    /// positioned absolutely (mirrors the old `PdfLayerReference::use_text` API).
    fn push_text(
        &self,
        ops: &mut Vec<Op>,
        text: &str,
        size: f32,
        x_mm: f32,
        y_mm: f32,
        bold: bool,
    ) {
        ops.push(Op::SetFont {
            font: PdfFontHandle::Builtin(if bold {
                BuiltinFont::HelveticaBold
            } else {
                BuiltinFont::Helvetica
            }),
            size: Pt(size),
        });
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(x_mm), Mm(y_mm)),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        });
    }

    /// Add a section header
    fn add_section_header(&self, ops: &mut Vec<Op>, text: &str, y_pos: f32) -> f32 {
        self.push_text(ops, text, self.options.font_size + 4.0, 20.0, y_pos, true);
        y_pos - 10.0
    }

    /// Add a text line
    fn add_text_line(&self, ops: &mut Vec<Op>, text: &str, y_pos: f32) -> f32 {
        self.push_text(ops, text, self.options.font_size, 25.0, y_pos, false);
        y_pos - 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_export_options_default() {
        let options = PdfExportOptions::default();
        assert!(options.include_page_numbers);
        assert!(options.include_toc);
        assert!(options.use_color);
        assert_eq!(options.font_size, 12.0);
    }

    #[test]
    fn test_paper_size_a4() {
        let size = PaperSize::A4.to_mm();
        assert_eq!(size, (210.0, 297.0));
    }

    #[test]
    fn test_paper_size_letter() {
        let size = PaperSize::Letter.to_mm();
        assert_eq!(size, (215.9, 279.4));
    }

    #[test]
    fn test_pdf_exporter_creation() {
        let exporter = PdfExporter::new(PdfExportOptions::default());
        assert_eq!(exporter.options.font_size, 12.0);
    }

    #[test]
    fn test_inspection_data_creation() {
        let data = InspectionData {
            hostname: "test-vm".to_string(),
            os_type: "linux".to_string(),
            distribution: "ubuntu".to_string(),
            version: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            product_name: "Ubuntu".to_string(),
            package_format: "deb".to_string(),
            package_manager: "apt".to_string(),
            kernel_version: Some("5.15.0".to_string()),
            total_memory: Some(8589934592),
            vcpus: Some(4),
            filesystems: vec![],
            packages: vec![],
            users: vec![],
            interfaces: vec![],
        };

        assert_eq!(data.hostname, "test-vm");
        assert_eq!(data.os_type, "linux");
    }
}

#[cfg(test)]
mod sanity_pdf_output {
    use super::*;

    #[test]
    fn generate_writes_valid_nonempty_pdf() {
        let dir = std::env::temp_dir();
        let out_path = dir.join("guestkit_pdf_sanity_check.pdf");

        let data = InspectionData {
            hostname: "sanity-vm".to_string(),
            os_type: "linux".to_string(),
            distribution: "ubuntu".to_string(),
            version: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            product_name: "Ubuntu".to_string(),
            package_format: "deb".to_string(),
            package_manager: "apt".to_string(),
            kernel_version: Some("5.15.0-generic".to_string()),
            total_memory: Some(8589934592),
            vcpus: Some(4),
            filesystems: vec![FilesystemInfo {
                device: "/dev/sda1".to_string(),
                mountpoint: "/".to_string(),
                fstype: "ext4".to_string(),
                size: 107374182400,
                used: 53687091200,
                available: 53687091200,
            }],
            packages: vec![PackageInfo {
                name: "openssh-server".to_string(),
                version: "1:8.9p1".to_string(),
                arch: "amd64".to_string(),
            }],
            users: vec![UserInfo {
                username: "ubuntu".to_string(),
                uid: "1000".to_string(),
                home: "/home/ubuntu".to_string(),
                shell: "/bin/bash".to_string(),
            }],
            interfaces: vec![NetworkInterface {
                name: "eth0".to_string(),
                mac_address: "52:54:00:12:34:56".to_string(),
                ip_addresses: vec!["10.0.0.5".to_string()],
                state: "up".to_string(),
            }],
        };

        let exporter = PdfExporter::new(PdfExportOptions::default());
        exporter
            .generate(&out_path, &data)
            .expect("PDF generation should succeed");

        let bytes = std::fs::read(&out_path).expect("PDF file should exist");
        assert!(
            bytes.len() > 500,
            "PDF output suspiciously small: {} bytes",
            bytes.len()
        );
        assert!(
            bytes.starts_with(b"%PDF-"),
            "output does not start with a PDF header"
        );
        assert!(
            bytes.windows(5).any(|w| w == b"%%EOF"),
            "output missing PDF trailer (%%EOF)"
        );

        std::fs::remove_file(&out_path).ok();
    }
}
