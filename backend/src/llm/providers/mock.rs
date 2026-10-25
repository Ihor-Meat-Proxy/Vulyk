use async_trait::async_trait;

use crate::error::AppResult;
use crate::llm::{LlmProvider, LlmRequest, LlmResponse};

/// Deterministic provider for unit/integration tests — never calls the network.
#[derive(Debug, Default, Clone)]
pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn complete(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let text = if req.user.contains("FRAMEWORK:") || req.system.contains("automation") {
            mock_automation(&req.user)
        } else if req.system.contains("Q&A") || req.user.to_lowercase().starts_with("question:") {
            mock_qa(&req.user)
        } else {
            mock_cases(&req.user)
        };
        Ok(LlmResponse {
            text,
            provider: "mock".into(),
            model: "mock-1".into(),
        })
    }
}

fn mock_cases(user: &str) -> String {
    let title_hint = user.lines().next().unwrap_or("Generated requirement");
    serde_json::json!([
        {
            "key_suffix": "GEN-1",
            "title": format!("Verify: {title_hint}"),
            "description": "Mock-generated case from requirements.",
            "precondition": "System is available.",
            "priority": "medium",
            "steps": [
                {
                    "action": "Open the feature under test",
                    "expected_result": "UI loads without error"
                },
                {
                    "action": "Perform the primary action from the requirement",
                    "expected_result": "Outcome matches the requirement"
                }
            ]
        }
    ])
    .to_string()
}

fn mock_automation(user: &str) -> String {
    if user.to_lowercase().contains("pytest") {
        r#"import pytest

def test_generated_case(page_or_client):
    """Mock pytest generated from Vulyk case."""
    assert True
"#
        .into()
    } else {
        r#"import { test, expect } from '@playwright/test';

test('generated case', async ({ page }) => {
  // Mock Playwright generated from Vulyk case
  await page.goto('/');
  await expect(page).toHaveTitle(/.+/);
});
"#
        .into()
    }
}

fn mock_qa(user: &str) -> String {
    format!(
        "Based on the project tests on file, here is a mock answer to: {}",
        user.chars().take(200).collect::<String>()
    )
}
