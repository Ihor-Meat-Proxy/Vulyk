use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::llm::{LlmProvider, LlmRequest, LlmResponse};

pub struct GeminiProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl GeminiProvider {
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
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<Content>,
}

#[derive(Debug, Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
struct Part {
    #[serde(default)]
    text: String,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn id(&self) -> &'static str {
        "gemini"
    }

    async fn complete(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, self.model, self.api_key
        );
        let resp = self
            .client
            .post(&url)
            .json(&json!({
                "system_instruction": { "parts": [{ "text": req.system }] },
                "contents": [{
                    "role": "user",
                    "parts": [{ "text": req.user }]
                }],
                "generationConfig": { "maxOutputTokens": req.max_tokens }
            }))
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("gemini HTTP: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| AppError::BadRequest(format!("gemini body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "gemini failed ({status}): {body}"
            )));
        }
        let parsed: GeminiResponse = serde_json::from_str(&body)
            .map_err(|e| AppError::BadRequest(format!("gemini parse: {e}")))?;
        let text = parsed
            .candidates
            .first()
            .and_then(|c| c.content.as_ref())
            .map(|c| {
                c.parts
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        Ok(LlmResponse {
            text,
            provider: "gemini".into(),
            model: self.model.clone(),
        })
    }
}
