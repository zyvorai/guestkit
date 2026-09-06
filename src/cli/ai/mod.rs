// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! AI-powered VM diagnostics and assistance
//!
//! This module is only available when the 'ai' feature is enabled.
//! Set OPENAI_API_KEY environment variable to use.

#[cfg(feature = "ai")]
pub mod assistant;
#[cfg(feature = "ai")]
pub mod tools;

#[cfg(feature = "ai")]
pub use assistant::run_ai_assistant;

#[cfg(not(feature = "ai"))]
pub fn run_ai_assistant(_image: &std::path::Path, _query: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "AI features not enabled. Rebuild with --features ai and set OPENAI_API_KEY environment variable."
    );
}
