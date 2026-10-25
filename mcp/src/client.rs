//! Thin HTTP client for the Vulyk API. MCP tools never talk to Postgres directly.

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct VulykClient {
    http: Client,
    base: String,
    token: String,
}

impl VulykClient {
    pub fn from_env() -> Result<Self> {
        let base =
            std::env::var("VULYK_API_URL").unwrap_or_else(|_| "http://localhost:8080".into());
        let token = std::env::var("VULYK_API_TOKEN").map_err(|_| {
            anyhow!("VULYK_API_TOKEN is required — create one via POST /api/v1/tokens")
        })?;
        if token.trim().is_empty() {
            return Err(anyhow!("VULYK_API_TOKEN is empty"));
        }
        Ok(Self {
            http: Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
        })
    }

    pub fn from_parts(base: String, token: String) -> Self {
        Self {
            http: Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let mut builder = self
            .http
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json");
        if let Some(b) = body {
            builder = builder.json(&b);
        }
        let resp = builder
            .send()
            .await
            .with_context(|| format!("HTTP {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("Vulyk API {status}: {text}"));
        }
        if text.is_empty() || status.as_u16() == 204 {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).with_context(|| format!("parse response from {url}"))
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.request(reqwest::Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }

    pub async fn put(&self, path: &str, body: Value) -> Result<Value> {
        self.request(reqwest::Method::PUT, path, Some(body)).await
    }
}
