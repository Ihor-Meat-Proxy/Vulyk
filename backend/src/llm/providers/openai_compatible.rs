use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::llm::{LlmProvider, LlmRequest, LlmResponse};

/// Shared client for Ollama (OpenAI mode), Unsloth/local servers, OpenRouter, OpenAI, xAI.
pub struct OpenAiCompatible {
    id: &'static str,
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatible {
    pub fn new(id: &'static str, base_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            id,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatible {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn complete(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = self.client.post(&url).json(&json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": req.user },
            ]
        }));
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let resp = builder.send().await.map_err(|e| {
            AppError::BadRequest(format!(
                "LLM call failed via {} / {} (network): {e}",
                self.id, self.model
            ))
        })?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| {
            AppError::BadRequest(format!(
                "LLM call failed via {} / {} (read body): {e}",
                self.id, self.model
            ))
        })?;
        if !status.is_success() {
            let rate_hint = if status.as_u16() == 429 {
                " Rate limited — wait and retry, or switch off free-tier models."
            } else {
                ""
            };
            // Surface the real provider failure; never silently fall back to mock.
            return Err(AppError::BadRequest(format!(
                "LLM call failed via {} / {} (HTTP {status}): {body}{rate_hint}",
                self.id, self.model
            )));
        }
        let parsed: ChatCompletion = serde_json::from_str(&body).map_err(|e| {
            AppError::BadRequest(format!(
                "invalid {} / {} response: {e}; body={body}",
                self.id, self.model
            ))
        })?;
        let text = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(LlmResponse {
            text,
            provider: self.id.into(),
            model: self.model.clone(),
        })
    }
}
