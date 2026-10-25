mod anthropic;
mod gemini;
mod mock;
mod openai_compatible;

use super::{LlmProvider, ProviderConfig, ProviderKind};
use crate::error::AppResult;

pub use mock::MockProvider;

pub fn build(cfg: &ProviderConfig) -> AppResult<Box<dyn LlmProvider>> {
    match cfg.kind {
        ProviderKind::Mock => Ok(Box::new(MockProvider)),
        ProviderKind::Ollama => Ok(Box::new(openai_compatible::OpenAiCompatible::new(
            "ollama",
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434/v1".into()),
            cfg.model.clone(),
            cfg.api_key.clone(),
        ))),
        ProviderKind::OpenaiCompatible => Ok(Box::new(openai_compatible::OpenAiCompatible::new(
            "openai_compatible",
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:8000/v1".into()),
            cfg.model.clone(),
            cfg.api_key.clone(),
        ))),
        ProviderKind::Openrouter => Ok(Box::new(openai_compatible::OpenAiCompatible::new(
            "openrouter",
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
            cfg.model.clone(),
            Some(require_key(cfg, "openrouter")?),
        ))),
        ProviderKind::Openai => Ok(Box::new(openai_compatible::OpenAiCompatible::new(
            "openai",
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into()),
            cfg.model.clone(),
            Some(require_key(cfg, "openai")?),
        ))),
        ProviderKind::Xai => Ok(Box::new(openai_compatible::OpenAiCompatible::new(
            "xai",
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://api.x.ai/v1".into()),
            cfg.model.clone(),
            Some(require_key(cfg, "xai")?),
        ))),
        ProviderKind::Anthropic => Ok(Box::new(anthropic::AnthropicProvider::new(
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            cfg.model.clone(),
            require_key(cfg, "anthropic")?,
        ))),
        ProviderKind::Gemini => Ok(Box::new(gemini::GeminiProvider::new(
            cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into()),
            cfg.model.clone(),
            require_key(cfg, "gemini")?,
        ))),
    }
}

fn require_key(cfg: &ProviderConfig, name: &str) -> AppResult<String> {
    cfg.api_key
        .clone()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| crate::error::AppError::BadRequest(format!("{name} requires an API key")))
}
