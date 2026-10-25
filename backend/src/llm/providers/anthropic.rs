use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::llm::{LlmProvider, LlmRequest, LlmResponse};

pub struct AnthropicProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(base_url: String, model: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<Block>,
}

#[derive(Debug, Deserialize)]
struct Block {
    #[serde(default)]
    text: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn complete(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.model,
                "max_tokens": req.max_tokens,
                "system": req.system,
                "messages": [
                    { "role": "user", "content": req.user }
                ]
            }))
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("anthropic HTTP: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| AppError::BadRequest(format!("anthropic body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "anthropic failed ({status}): {body}"
            )));
        }
        let parsed: AnthropicResponse = serde_json::from_str(&body)
            .map_err(|e| AppError::BadRequest(format!("anthropic parse: {e}")))?;
        let text = parsed
            .content
            .into_iter()
            .map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");
        Ok(LlmResponse {
            text,
            provider: "anthropic".into(),
            model: self.model.clone(),
        })
    }
}
