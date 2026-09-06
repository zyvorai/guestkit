// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Report export functionality

pub mod html;
pub mod markdown;
pub mod pdf;

use crate::cli::formatters::InspectionReport;
use anyhow::Result;
use std::path::Path;

/// Export format for reports
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Html,
    Markdown,
    Pdf,
}

impl ExportFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "html" => Ok(ExportFormat::Html),
            "md" | "markdown" => Ok(ExportFormat::Markdown),
            "pdf" => Ok(ExportFormat::Pdf),
            _ => Err(anyhow::anyhow!("Unknown export format: {}", s)),
        }
    }

    #[allow(dead_code)]
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Html => "html",
            ExportFormat::Markdown => "md",
            ExportFormat::Pdf => "pdf",
        }
    }
}

/// Export an inspection report to a file
pub fn export_report(
    report: &InspectionReport,
    format: ExportFormat,
    output_path: &Path,
) -> Result<()> {
    match format {
        ExportFormat::Html => {
            let content = html::generate_html_report(report)?;
            std::fs::write(output_path, content)?;
        }
        ExportFormat::Markdown => {
            let content = markdown::generate_markdown_report(report)?;
            std::fs::write(output_path, content)?;
        }
        ExportFormat::Pdf => {
            pdf::generate_pdf_to_file(report, output_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_format_from_str_html() {
        let format = ExportFormat::from_str("html").unwrap();
        assert_eq!(format.extension(), "html");
    }

    #[test]
    fn test_export_format_from_str_markdown() {
        assert!(ExportFormat::from_str("markdown").is_ok());
        assert!(ExportFormat::from_str("md").is_ok());

        let format = ExportFormat::from_str("md").unwrap();
        assert_eq!(format.extension(), "md");
    }

    #[test]
    fn test_export_format_from_str_pdf() {
        let format = ExportFormat::from_str("pdf").unwrap();
        assert_eq!(format.extension(), "pdf");
    }

    #[test]
    fn test_export_format_from_str_case_insensitive() {
        assert!(ExportFormat::from_str("HTML").is_ok());
        assert!(ExportFormat::from_str("Html").is_ok());
        assert!(ExportFormat::from_str("MARKDOWN").is_ok());
        assert!(ExportFormat::from_str("Pdf").is_ok());
    }

    #[test]
    fn test_export_format_from_str_invalid() {
        assert!(ExportFormat::from_str("json").is_err());
        assert!(ExportFormat::from_str("xml").is_err());
        assert!(ExportFormat::from_str("txt").is_err());
        assert!(ExportFormat::from_str("").is_err());
    }

    #[test]
    fn test_export_format_extension_html() {
        let format = ExportFormat::Html;
        assert_eq!(format.extension(), "html");
    }

    #[test]
    fn test_export_format_extension_markdown() {
        let format = ExportFormat::Markdown;
        assert_eq!(format.extension(), "md");
    }

    #[test]
    fn test_export_format_extension_pdf() {
        let format = ExportFormat::Pdf;
        assert_eq!(format.extension(), "pdf");
    }

    #[test]
    fn test_markdown_aliases() {
        let md_format = ExportFormat::from_str("md").unwrap();
        let markdown_format = ExportFormat::from_str("markdown").unwrap();

        assert_eq!(md_format.extension(), markdown_format.extension());
    }
}
