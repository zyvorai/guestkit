// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Template system for customizable report generation

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Template format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateFormat {
    Html,
    Markdown,
    Text,
}

/// Template verbosity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateLevel {
    Minimal,
    Standard,
    Detailed,
}

/// Escape HTML special characters to prevent template injection
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Template engine for report generation
pub struct TemplateEngine {
    templates: HashMap<String, String>,
}

impl TemplateEngine {
    /// Create a new template engine with built-in templates
    pub fn new() -> Self {
        let mut engine = Self {
            templates: HashMap::new(),
        };

        // Load built-in templates
        engine.load_builtin_templates();
        engine
    }

    /// Load built-in templates
    fn load_builtin_templates(&mut self) {
        // HTML templates
        self.templates.insert(
            "html_minimal".to_string(),
            include_str!("../../templates/html_minimal.tpl").to_string(),
        );
        self.templates.insert(
            "html_standard".to_string(),
            include_str!("../../templates/html_standard.tpl").to_string(),
        );
        self.templates.insert(
            "html_detailed".to_string(),
            include_str!("../../templates/html_detailed.tpl").to_string(),
        );

        // Markdown templates
        self.templates.insert(
            "markdown_minimal".to_string(),
            include_str!("../../templates/markdown_minimal.tpl").to_string(),
        );
        self.templates.insert(
            "markdown_standard".to_string(),
            include_str!("../../templates/markdown_standard.tpl").to_string(),
        );
        self.templates.insert(
            "markdown_detailed".to_string(),
            include_str!("../../templates/markdown_detailed.tpl").to_string(),
        );

        // Text templates
        self.templates.insert(
            "text_minimal".to_string(),
            include_str!("../../templates/text_minimal.tpl").to_string(),
        );
        self.templates.insert(
            "text_standard".to_string(),
            include_str!("../../templates/text_standard.tpl").to_string(),
        );
    }

    /// Load a custom template from file
    pub fn load_template<P: AsRef<Path>>(&mut self, name: &str, path: P) -> Result<()> {
        let content = fs::read_to_string(path.as_ref()).with_context(|| {
            format!("Failed to read template file: {}", path.as_ref().display())
        })?;

        // Validate template
        self.validate_template(&content)?;

        self.templates.insert(name.to_string(), content);
        Ok(())
    }

    /// Load a template from string
    pub fn load_template_string(&mut self, name: &str, content: &str) -> Result<()> {
        // Validate template
        self.validate_template(content)?;

        self.templates.insert(name.to_string(), content.to_string());
        Ok(())
    }

    /// Get a template by name
    pub fn get_template(&self, name: &str) -> Option<&str> {
        self.templates.get(name).map(|s| s.as_str())
    }

    /// Get template name for format and level
    pub fn get_template_name(format: TemplateFormat, level: TemplateLevel) -> String {
        let format_str = match format {
            TemplateFormat::Html => "html",
            TemplateFormat::Markdown => "markdown",
            TemplateFormat::Text => "text",
        };

        let level_str = match level {
            TemplateLevel::Minimal => "minimal",
            TemplateLevel::Standard => "standard",
            TemplateLevel::Detailed => "detailed",
        };

        format!("{}_{}", format_str, level_str)
    }

    /// Render a template with variables
    pub fn render(
        &self,
        template_name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        let template = self
            .get_template(template_name)
            .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

        let mut result = template.to_string();

        // Determine if this is an HTML template so we can escape values.
        // Check both the template name and the template content for HTML indicators.
        let template_lower = template.to_lowercase();
        let is_html = template_name.starts_with("html")
            || template_lower.contains("<html")
            || template_lower.contains("<body")
            || template_lower.contains("<div")
            || template_lower.contains("<!doctype");

        // Replace all variables in the format {{variable_name}}
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            let safe_value = if is_html {
                html_escape(value)
            } else {
                value.clone()
            };
            result = result.replace(&placeholder, &safe_value);
        }

        // Check for unresolved variables — treat as an error so callers
        // don't silently produce output with literal {{variable}} strings
        if result.contains("{{") && result.contains("}}") {
            let unresolved = self.find_unresolved_variables(&result);
            if !unresolved.is_empty() {
                anyhow::bail!(
                    "Template '{}' has unresolved variables: {}",
                    template_name,
                    unresolved.join(", ")
                );
            }
        }

        Ok(result)
    }

    /// Validate template syntax
    fn validate_template(&self, content: &str) -> Result<()> {
        let mut open_count = 0;
        let mut close_count = 0;
        let mut chars = content.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '{' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '{' {
                        chars.next(); // consume second '{'
                        open_count += 1;
                    }
                }
            } else if ch == '}' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '}' {
                        chars.next(); // consume second '}'
                        close_count += 1;
                    }
                }
            }
        }

        if open_count != close_count {
            return Err(anyhow::anyhow!(
                "Template syntax error: mismatched braces ({{ {} }}, }} {} }})",
                open_count,
                close_count
            ));
        }

        Ok(())
    }

    /// Find unresolved variables in rendered template
    fn find_unresolved_variables(&self, rendered: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut chars = rendered.chars().peekable();
        let mut current_var = String::new();
        let mut in_variable = false;

        while let Some(ch) = chars.next() {
            if ch == '{' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '{' {
                        chars.next(); // consume second '{'
                        in_variable = true;
                        current_var.clear();
                        continue;
                    }
                }
            } else if ch == '}' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '}' {
                        chars.next(); // consume second '}'
                        if in_variable {
                            variables.push(current_var.trim().to_string());
                            in_variable = false;
                        }
                        continue;
                    }
                }
            }

            if in_variable {
                current_var.push(ch);
            }
        }

        variables
    }

    /// List all available templates
    pub fn list_templates(&self) -> Vec<String> {
        let mut names: Vec<String> = self.templates.keys().cloned().collect();
        names.sort();
        names
    }

    /// Check if a template exists
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create variable map from inspection data
pub fn create_variable_map(
    hostname: &str,
    os_type: &str,
    distribution: &str,
    version: &str,
    architecture: &str,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    vars.insert("hostname".to_string(), hostname.to_string());
    vars.insert("os_type".to_string(), os_type.to_string());
    vars.insert("distribution".to_string(), distribution.to_string());
    vars.insert("version".to_string(), version.to_string());
    vars.insert("architecture".to_string(), architecture.to_string());
    vars.insert(
        "timestamp".to_string(),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );
    vars.insert("tool_name".to_string(), "GuestKit".to_string());
    vars.insert(
        "tool_version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_engine_creation() {
        let engine = TemplateEngine::new();
        assert!(!engine.templates.is_empty());
        assert!(engine.has_template("html_minimal"));
        assert!(engine.has_template("markdown_standard"));
    }

    #[test]
    fn test_template_name_generation() {
        let name = TemplateEngine::get_template_name(TemplateFormat::Html, TemplateLevel::Minimal);
        assert_eq!(name, "html_minimal");

        let name =
            TemplateEngine::get_template_name(TemplateFormat::Markdown, TemplateLevel::Detailed);
        assert_eq!(name, "markdown_detailed");
    }

    #[test]
    fn test_render_template() {
        let mut vars = HashMap::new();
        vars.insert("hostname".to_string(), "test-vm".to_string());
        vars.insert("os_type".to_string(), "linux".to_string());

        // Create a simple test template
        let mut test_engine = TemplateEngine::new();
        test_engine
            .load_template_string("test", "Host: {{hostname}}, OS: {{os_type}}")
            .unwrap();

        let result = test_engine.render("test", &vars).unwrap();
        assert_eq!(result, "Host: test-vm, OS: linux");
    }

    #[test]
    fn test_validate_template_valid() {
        let engine = TemplateEngine::new();
        let valid_template = "Hello {{name}}, your OS is {{os_type}}";
        assert!(engine.validate_template(valid_template).is_ok());
    }

    #[test]
    fn test_validate_template_invalid() {
        let engine = TemplateEngine::new();
        let invalid_template = "Hello {{name}, missing closing brace";
        assert!(engine.validate_template(invalid_template).is_err());
    }

    #[test]
    fn test_find_unresolved_variables() {
        let engine = TemplateEngine::new();
        let rendered = "Hello {{name}}, OS: linux, Version: {{version}}";
        let unresolved = engine.find_unresolved_variables(rendered);
        assert_eq!(unresolved.len(), 2);
        assert!(unresolved.contains(&"name".to_string()));
        assert!(unresolved.contains(&"version".to_string()));
    }

    #[test]
    fn test_list_templates() {
        let engine = TemplateEngine::new();
        let templates = engine.list_templates();
        assert!(!templates.is_empty());
        assert!(templates.contains(&"html_minimal".to_string()));
        assert!(templates.contains(&"markdown_standard".to_string()));
    }

    #[test]
    fn test_create_variable_map() {
        let vars = create_variable_map("test-vm", "linux", "ubuntu", "22.04", "x86_64");

        assert_eq!(vars.get("hostname").unwrap(), "test-vm");
        assert_eq!(vars.get("os_type").unwrap(), "linux");
        assert_eq!(vars.get("distribution").unwrap(), "ubuntu");
        assert_eq!(vars.get("version").unwrap(), "22.04");
        assert_eq!(vars.get("architecture").unwrap(), "x86_64");
        assert!(vars.contains_key("timestamp"));
        assert_eq!(vars.get("tool_name").unwrap(), "GuestKit");
    }

    #[test]
    fn test_load_custom_template() {
        let mut engine = TemplateEngine::new();
        let custom = "Custom template for {{hostname}}";

        engine.load_template_string("custom", custom).unwrap();

        assert!(engine.has_template("custom"));
        assert_eq!(engine.get_template("custom").unwrap(), custom);
    }
}
