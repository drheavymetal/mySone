//! LLM providers for the queue-building chat feature.
//!
//! Two pluggable backends:
//! - DeepSeek: cheap remote API (`https://api.deepseek.com/v1/chat/completions`),
//!   structured JSON output via `response_format = json_object`.
//! - Ollama: local self-hosted models, JSON output via `format = "json"`.
//!
//! Both implement the same `LLMProvider` trait. Frontend gets back a single
//! response string; structured parsing (e.g. extracting `{items: [...]}`)
//! happens client-side so we keep this module narrow.

use crate::SoneError;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LLMProviderKind {
    #[default]
    Off,
    Deepseek,
    Ollama,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LLMSettings {
    #[serde(default)]
    pub provider: LLMProviderKind,
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default = "default_deepseek_model")]
    pub deepseek_model: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_base_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
}

fn default_deepseek_model() -> String {
    "deepseek-chat".into()
}
fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}
fn default_ollama_model() -> String {
    "llama3.1:8b".into()
}

impl Default for LLMSettings {
    fn default() -> Self {
        Self {
            provider: LLMProviderKind::Off,
            deepseek_api_key: String::new(),
            deepseek_model: default_deepseek_model(),
            ollama_base_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat_json(
        &self,
        system: &str,
        messages: &[ChatMessage],
    ) -> Result<String, SoneError>;
    async fn ping(&self) -> Result<(), SoneError>;
}

pub fn build_provider(s: &LLMSettings) -> Result<Box<dyn LLMProvider>, SoneError> {
    match s.provider {
        LLMProviderKind::Off => Err(SoneError::Audio(
            "AI backend disabled — choose a provider in Settings".into(),
        )),
        LLMProviderKind::Deepseek => {
            if s.deepseek_api_key.trim().is_empty() {
                return Err(SoneError::Audio("DeepSeek API key not set".into()));
            }
            Ok(Box::new(DeepseekProvider {
                api_key: s.deepseek_api_key.clone(),
                model: s.deepseek_model.clone(),
                http: build_http(),
            }))
        }
        LLMProviderKind::Ollama => Ok(Box::new(OllamaProvider {
            base_url: s.ollama_base_url.trim_end_matches('/').to_string(),
            model: s.ollama_model.clone(),
            http: build_http(),
        })),
    }
}

fn build_http() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("reqwest client")
}

// ── DeepSeek ───────────────────────────────────────────────────────────

struct DeepseekProvider {
    api_key: String,
    model: String,
    http: Client,
}

#[derive(Serialize)]
struct DeepseekChatBody<'a> {
    model: &'a str,
    messages: Vec<DeepseekMessage<'a>>,
    response_format: DeepseekFormat,
    temperature: f32,
}

#[derive(Serialize)]
struct DeepseekMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct DeepseekFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct DeepseekChatResponse {
    choices: Vec<DeepseekChoice>,
}
#[derive(Deserialize)]
struct DeepseekChoice {
    message: DeepseekRespMessage,
}
#[derive(Deserialize)]
struct DeepseekRespMessage {
    content: String,
}

#[async_trait]
impl LLMProvider for DeepseekProvider {
    async fn chat_json(
        &self,
        system: &str,
        messages: &[ChatMessage],
    ) -> Result<String, SoneError> {
        let mut payload = vec![DeepseekMessage {
            role: "system",
            content: system,
        }];
        for m in messages {
            payload.push(DeepseekMessage {
                role: m.role.as_str(),
                content: m.content.as_str(),
            });
        }
        let body = DeepseekChatBody {
            model: &self.model,
            messages: payload,
            response_format: DeepseekFormat { kind: "json_object" },
            temperature: 0.7,
        };
        let resp = self
            .http
            .post("https://api.deepseek.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| SoneError::Audio(format!("DeepSeek request: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SoneError::Audio(format!("DeepSeek {status}: {text}")));
        }
        let parsed: DeepseekChatResponse = resp
            .json()
            .await
            .map_err(|e| SoneError::Audio(format!("DeepSeek decode: {e}")))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| SoneError::Audio("DeepSeek empty choices".into()))?;
        Ok(content)
    }

    async fn ping(&self) -> Result<(), SoneError> {
        // Cheapest reachability test: 1-token completion.
        self.chat_json(
            "Reply with exactly: ok",
            &[ChatMessage {
                role: "user".into(),
                content: "ping".into(),
            }],
        )
        .await?;
        Ok(())
    }
}

// ── Ollama ─────────────────────────────────────────────────────────────

struct OllamaProvider {
    base_url: String,
    model: String,
    http: Client,
}

#[derive(Serialize)]
struct OllamaChatBody<'a> {
    model: &'a str,
    messages: Vec<DeepseekMessage<'a>>,
    format: &'static str,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaRespMessage,
}
#[derive(Deserialize)]
struct OllamaRespMessage {
    content: String,
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat_json(
        &self,
        system: &str,
        messages: &[ChatMessage],
    ) -> Result<String, SoneError> {
        let mut payload = vec![DeepseekMessage {
            role: "system",
            content: system,
        }];
        for m in messages {
            payload.push(DeepseekMessage {
                role: m.role.as_str(),
                content: m.content.as_str(),
            });
        }
        let body = OllamaChatBody {
            model: &self.model,
            messages: payload,
            format: "json",
            stream: false,
            options: OllamaOptions { temperature: 0.7 },
        };
        let url = format!("{}/api/chat", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SoneError::Audio(format!("Ollama request: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SoneError::Audio(format!("Ollama {status}: {text}")));
        }
        let parsed: OllamaChatResponse = resp
            .json()
            .await
            .map_err(|e| SoneError::Audio(format!("Ollama decode: {e}")))?;
        Ok(parsed.message.content)
    }

    async fn ping(&self) -> Result<(), SoneError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SoneError::Audio(format!("Ollama unreachable: {e}")))?;
        if !resp.status().is_success() {
            return Err(SoneError::Audio(format!(
                "Ollama returned {}",
                resp.status()
            )));
        }
        Ok(())
    }
}
