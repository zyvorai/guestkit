// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! LLM providers — OpenAI, xAI, Anthropic, Ollama (local-ai).

use anyhow::{anyhow, Context, Result};
use rig::client::completion::CompletionClient;
use rig::completion::CompletionModel;
use rig::providers::openai;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    XAi,
    Anthropic,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: Provider::OpenAi,
            model: "gpt-4o".into(),
            api_key: None,
            base_url: None,
        }
    }
}

impl ProviderConfig {
    pub fn from_env() -> Result<Self> {
        if std::env::var("OLLAMA_HOST").is_ok()
            || std::env::var("GUESTKIT_AI_PROVIDER")
                .map(|v| v.eq_ignore_ascii_case("ollama"))
                .unwrap_or(false)
        {
            return Ok(Self {
                provider: Provider::Ollama,
                model: std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".into()),
                api_key: None,
                base_url: Some(
                    std::env::var("OLLAMA_HOST")
                        .unwrap_or_else(|_| "http://127.0.0.1:11434".into()),
                ),
            });
        }
        if std::env::var("XAI_API_KEY").is_ok() {
            return Ok(Self {
                provider: Provider::XAi,
                model: std::env::var("GUESTKIT_AI_MODEL")
                    .unwrap_or_else(|_| "grok-2-latest".into()),
                api_key: std::env::var("XAI_API_KEY").ok(),
                base_url: Some("https://api.x.ai/v1".into()),
            });
        }
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            return Ok(Self {
                provider: Provider::Anthropic,
                model: std::env::var("GUESTKIT_AI_MODEL")
                    .unwrap_or_else(|_| "claude-3-5-sonnet-latest".into()),
                api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
                base_url: None,
            });
        }
        Ok(Self {
            provider: Provider::OpenAi,
            model: std::env::var("GUESTKIT_AI_MODEL").unwrap_or_else(|_| "gpt-4o".into()),
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            base_url: std::env::var("OPENAI_BASE_URL").ok(),
        })
    }
}

/// Send a completion request to the configured provider.
pub async fn completion(config: &ProviderConfig, system: &str, user: &str) -> Result<String> {
    match config.provider {
        Provider::OpenAi => openai_completion(config, system, user).await,
        Provider::XAi => openai_compatible_http(config, system, user).await,
        Provider::Anthropic => anthropic_completion(config, system, user).await,
        Provider::Ollama => ollama_completion(config, system, user).await,
    }
}

async fn openai_completion(config: &ProviderConfig, system: &str, user: &str) -> Result<String> {
    let api_key = config
        .api_key
        .clone()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| anyhow!("OPENAI_API_KEY not set"))?;

    let full_prompt = format!("{system}\n\n{user}");
    let client = openai::Client::<reqwest::Client>::new(&api_key)
        .context("Failed to create OpenAI client")?;
    let model_name = if config.model.is_empty() {
        openai::GPT_4O
    } else {
        config.model.as_str()
    };
    let response = client
        .completions_api()
        .completion_model(model_name)
        .completion_request(&full_prompt)
        .send()
        .await
        .context("OpenAI completion failed")?;

    extract_rig_text(response)
}

async fn openai_compatible_http(
    config: &ProviderConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    let api_key = config
        .api_key
        .clone()
        .ok_or_else(|| anyhow!("API key not set"))?;
    let base = config
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".into());
    let body = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let resp = reqwest::Client::new()
        .post(format!("{base}/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    resp["choices"][0]["message"]["content"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("unexpected chat completion response"))
}

async fn anthropic_completion(config: &ProviderConfig, system: &str, user: &str) -> Result<String> {
    let api_key = config
        .api_key
        .clone()
        .ok_or_else(|| anyhow!("ANTHROPIC_API_KEY not set"))?;
    let body = serde_json::json!({
        "model": config.model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{"role": "user", "content": user}]
    });
    let resp = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    resp["content"][0]["text"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("unexpected Anthropic response"))
}

async fn ollama_completion(config: &ProviderConfig, system: &str, user: &str) -> Result<String> {
    let base = config
        .base_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:11434".into());
    let body = serde_json::json!({
        "model": config.model,
        "stream": false,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/chat"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    resp["message"]["content"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("unexpected Ollama response"))
}

fn extract_rig_text(
    response: rig::completion::CompletionResponse<rig::providers::openai::CompletionResponse>,
) -> Result<String> {
    use rig::completion::AssistantContent;
    match response.choice.first() {
        AssistantContent::Text(text) => Ok(text.text.clone()),
        _ => Err(anyhow!("unexpected response type from LLM")),
    }
}
