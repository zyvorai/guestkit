// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 2 — multi-step agent loop over evidence snapshot tools.

use crate::ai::prompts::{self, tool_loop_instructions};
use crate::ai::providers::{self, Provider, ProviderConfig};
use crate::ai::rig_tools::SnapshotContext;
use crate::ai::tools::SnapshotTools;
use crate::assurance::boot_target_from_str;
use crate::boot::{analyze_bootability, BootabilityReport};
use crate::evidence::snapshot::EvidenceSnapshot;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_steps: usize,
    pub boot_target: String,
    pub provider: ProviderConfig,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 5,
            boot_target: "generic".into(),
            provider: ProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub answer: String,
    pub steps: usize,
    pub tool_calls: Vec<ToolCallRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args: Value,
    pub result_preview: String,
}

/// Run the Guest Intelligence Agent on a pre-collected evidence snapshot.
pub async fn run_agent_on_evidence(
    evidence: &EvidenceSnapshot,
    query: &str,
    config: &AgentConfig,
) -> Result<AgentResult> {
    let boot_target = boot_target_from_str(&config.boot_target);
    let boot = analyze_bootability(evidence, boot_target);
    run_agent_on_evidence_with_boot(evidence, Some(&boot), query, config).await
}

pub async fn run_agent_on_evidence_with_boot(
    evidence: &EvidenceSnapshot,
    boot: Option<&BootabilityReport>,
    query: &str,
    config: &AgentConfig,
) -> Result<AgentResult> {
    let provider = if config.provider.api_key.is_some()
        || config.provider.provider == providers::Provider::Ollama
    {
        config.provider.clone()
    } else {
        ProviderConfig::from_env()?
    };

    // Cross-run memory: fold a short summary of prior findings on this same
    // VM (see ai/memory.rs) into the query both tool-calling paths receive,
    // so a re-run after a fix doesn't start from zero context.
    let image_path = std::path::Path::new(&evidence.image_path);
    let prior_memory = crate::ai::memory::load(image_path);
    let augmented_query = match &prior_memory {
        Some(mem) => {
            let summary = crate::ai::memory::context_summary(mem, 5);
            if summary.is_empty() {
                query.to_string()
            } else {
                format!("{summary}\nCurrent query: {query}")
            }
        }
        None => query.to_string(),
    };

    // Native, schema-validated tool-calling (rig::agent::AgentBuilder +
    // multi_turn) — only for providers rig has a real completion-model
    // client for. Anthropic/xAI have rig clients too but this codebase's
    // providers.rs doesn't build them yet (see run_text_tool_loop's
    // openai_compatible_http/anthropic_completion raw-HTTP paths); Ollama
    // has no native rig client at all. Both fall back to the original
    // text-instructed parse_tool_call loop below, unchanged.
    let result = if provider.provider == Provider::OpenAi {
        match run_native_openai(
            evidence,
            boot,
            &augmented_query,
            &provider,
            config.max_steps,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                log::warn!("native OpenAI tool-calling failed, falling back to text loop: {e}");
                run_text_tool_loop(evidence, boot, &augmented_query, config, &provider).await
            }
        }
    } else {
        run_text_tool_loop(evidence, boot, &augmented_query, config, &provider).await
    };

    if let Ok(ref r) = result {
        let entry = crate::ai::memory::MemoryEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            query: query.to_string(),
            answer: r.answer.clone(),
            boot_score: boot.map(|b| b.score),
            tool_call_count: r.tool_calls.len(),
        };
        if let Err(e) = crate::ai::memory::record(image_path, entry) {
            log::warn!("failed to record AI agent memory: {e}");
        }
    }

    result
}

/// Native tool-calling path for OpenAI via rig's `AgentBuilder` +
/// `PromptRequest::multi_turn` — tool calls are parsed and schema-validated
/// by the provider SDK instead of scraped out of raw completion text.
async fn run_native_openai(
    evidence: &EvidenceSnapshot,
    boot: Option<&BootabilityReport>,
    query: &str,
    provider: &ProviderConfig,
    max_steps: usize,
) -> Result<AgentResult> {
    use rig::agent::AgentBuilder;
    use rig::client::completion::CompletionClient;
    use rig::completion::Prompt;
    use rig::providers::openai;

    let api_key = provider
        .api_key
        .clone()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| anyhow!("OPENAI_API_KEY not set"))?;
    let client = openai::Client::<reqwest::Client>::new(&api_key)
        .context("Failed to create OpenAI client")?;
    let model_name = if provider.model.is_empty() {
        openai::GPT_4O
    } else {
        provider.model.as_str()
    };
    let model = client.completions_api().completion_model(model_name);

    let ctx = SnapshotContext::new(Arc::new(evidence.clone()), Arc::new(boot.cloned()));
    let system = format!(
        "{}\n\nYou have tools available — call them instead of guessing at evidence.",
        prompts::system_prompt()
    );
    let agent = crate::ai::rig_tools::register_all(AgentBuilder::new(model), &ctx)
        .preamble(&system)
        .build();

    let hook = ToolCallCollector::default();
    let query = format!(
        "Evidence OS: {} {}\nQuery: {query}",
        evidence.os.distribution, evidence.os.version
    );
    let answer = agent
        .prompt(query.as_str())
        .with_hook(hook.clone())
        .multi_turn(max_steps)
        .await
        .context("OpenAI agent prompt failed")?;

    let tool_calls = hook.take();
    let steps = tool_calls.len() + 1;
    Ok(AgentResult {
        answer,
        steps,
        tool_calls,
    })
}

#[derive(Clone, Default)]
struct ToolCallCollector(Arc<Mutex<Vec<ToolCallRecord>>>);

impl ToolCallCollector {
    fn take(&self) -> Vec<ToolCallRecord> {
        std::mem::take(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl<M: rig::completion::CompletionModel> rig::agent::PromptHook<M> for ToolCallCollector {
    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        args: &str,
        result: &str,
        _cancel_sig: rig::agent::CancelSignal,
    ) {
        let parsed_args = serde_json::from_str(args).unwrap_or(Value::Null);
        let preview = result.chars().take(2000).collect::<String>();
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(ToolCallRecord {
                tool: tool_name.to_string(),
                args: parsed_args,
                result_preview: preview,
            });
    }
}

/// Legacy text-instructed tool loop: the model is told to reply with a raw
/// `{"tool": "...", "args": {...}}` line, which we regex/JSON-scrape out of
/// the completion text. Used for every provider without a native rig
/// tool-calling path (see run_agent_on_evidence_with_boot).
async fn run_text_tool_loop(
    evidence: &EvidenceSnapshot,
    boot: Option<&BootabilityReport>,
    query: &str,
    config: &AgentConfig,
    provider: &ProviderConfig,
) -> Result<AgentResult> {
    let tools = SnapshotTools::new(evidence, boot);

    let mut transcript = format!(
        "Evidence OS: {} {}\nQuery: {}\n\nInitial context:\n{}\n",
        evidence.os.distribution,
        evidence.os.version,
        query,
        serde_json::to_string_pretty(&tools.call("get_semantic_summary", &Value::Null)?)?
    );

    let system = format!(
        "{}\n\n{}",
        prompts::system_prompt(),
        tool_loop_instructions()
    );

    let mut tool_calls = Vec::new();
    let mut answer = String::new();

    for step in 0..config.max_steps {
        let user = if step == 0 {
            format!("{transcript}\nAnswer the query or request a tool.")
        } else {
            format!("{transcript}\nContinue — answer or call another tool.")
        };

        let response = providers::completion(provider, &system, &user).await?;
        if let Some(call) = parse_tool_call(&response) {
            let result = tools.call(&call.tool, &call.args)?;
            let preview = serde_json::to_string(&result)?
                .chars()
                .take(2000)
                .collect::<String>();
            tool_calls.push(ToolCallRecord {
                tool: call.tool.clone(),
                args: call.args.clone(),
                result_preview: preview.clone(),
            });
            transcript.push_str(&format!("\nTool `{}` returned:\n{preview}\n", call.tool));
            continue;
        }
        answer = response;
        return Ok(AgentResult {
            answer,
            steps: step + 1,
            tool_calls,
        });
    }

    if answer.is_empty() {
        answer = providers::completion(
            provider,
            &system,
            &format!("{transcript}\nProvide final answer now without tools."),
        )
        .await?;
    }

    Ok(AgentResult {
        answer,
        steps: config.max_steps,
        tool_calls,
    })
}

#[derive(Debug)]
struct ParsedToolCall {
    tool: String,
    args: Value,
}

fn parse_tool_call(response: &str) -> Option<ParsedToolCall> {
    for line in response.lines() {
        let line = line.trim();
        let Some(json_start) = line.find('{') else {
            continue;
        };
        let json_part = &line[json_start..];
        if let Ok(v) = serde_json::from_str::<Value>(json_part) {
            if let (Some(tool), args) = (v.get("tool").and_then(|t| t.as_str()), v.get("args")) {
                return Some(ParsedToolCall {
                    tool: tool.to_string(),
                    args: args.cloned().unwrap_or(Value::Object(Default::default())),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_json_line() {
        let call =
            parse_tool_call(r#"Checking… {"tool":"list_systemd_units","args":{"type":"timer"}}"#);
        assert!(call.is_some());
        assert_eq!(call.unwrap().tool, "list_systemd_units");
    }
}
