//! LLM provider abstraction (workstream W9).
//!
//! Adapters call remote APIs; generated content is returned to callers for
//! review — this module does not persist cases or code on its own.

pub mod providers;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Ollama,
    OpenaiCompatible,
    Openrouter,
    Anthropic,
    Gemini,
    Openai,
    Xai,
    Mock,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai_compatible",
            Self::Openrouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Openai => "openai",
            Self::Xai => "xai",
            Self::Mock => "mock",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "ollama" => Ok(Self::Ollama),
            "openai_compatible" | "unsloth" => Ok(Self::OpenaiCompatible),
            "openrouter" => Ok(Self::Openrouter),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" | "google" => Ok(Self::Gemini),
            "openai" => Ok(Self::Openai),
            "xai" | "grok" => Ok(Self::Xai),
            "mock" => Ok(Self::Mock),
            other => Err(AppError::BadRequest(format!(
                "unknown LLM provider: {other}"
            ))),
        }
    }

    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Self::Ollama => Some("http://127.0.0.1:11434"),
            Self::OpenaiCompatible => Some("http://127.0.0.1:8000/v1"),
            Self::Openrouter => Some("https://openrouter.ai/api/v1"),
            Self::Anthropic => Some("https://api.anthropic.com"),
            Self::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
            Self::Openai => Some("https://api.openai.com/v1"),
            Self::Xai => Some("https://api.x.ai/v1"),
            Self::Mock => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key: Option<String>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn complete(&self, req: LlmRequest) -> AppResult<LlmResponse>;
}

pub fn build_provider(cfg: &ProviderConfig) -> AppResult<Box<dyn LlmProvider>> {
    providers::build(cfg)
}
