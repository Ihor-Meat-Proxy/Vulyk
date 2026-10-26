# LLM providers (W9)

Vulyk can call local and cloud LLMs to:

1. **Generate test cases** from requirements text or linked requirements  
2. **Generate automation** (Playwright or pytest) from existing cases  
3. **Chat with your tests** — project-scoped Q&A over the case catalogue  

## Providers

| Id | Notes |
|----|--------|
| `ollama` | Local Ollama OpenAI-compatible endpoint |
| `openai_compatible` | Unsloth / vLLM / any `/v1/chat/completions` server |
| `openrouter` | OpenRouter |
| `anthropic` | Anthropic Claude Messages API |
| `gemini` | Google Gemini `generateContent` |
| `openai` | OpenAI Chat Completions |
| `xai` | xAI Grok (OpenAI-compatible) |
| `mock` | Deterministic offline provider for tests / demos |

Configure per project under **Admin → LLM providers**, or via:

- `GET /api/v1/llm/providers`
- `CRUD /api/v1/projects/{id}/llm/configs`
- `POST /api/v1/projects/{id}/llm/generate-cases`
- `POST /api/v1/projects/{id}/llm/generate-code`
- `POST /api/v1/projects/{id}/llm/ask`

## Resolution order

For chat, case generation, and automation generation (when `config_id` is omitted):

1. **Project-specific** default, then any other project config  
2. **Global** default (`project_id` null), then any other global config  
3. **Mock** — only when no configs exist  

Configured providers are never silently replaced by mock. If a real provider call fails (for example OpenRouter HTTP 429 on a free model), the API returns a clear error that includes the provider and model; the UI surfaces that error instead of falling back to mock.

Mark a config **Global (all projects)** in Admin → LLM providers to share one default across projects.

## Secrets

API keys are encrypted at rest (AES-256-GCM). Set `VULYK_SECRETS_KEY` to a 32-byte value (base64 or hex) or a passphrase (SHA-256 derived). Plaintext keys are **never** returned by the API (`has_api_key` only).

For CI and local tests without credentials you can leave providers unconfigured — the mock adapter answers with `provider=mock` / `model=mock-1` and makes no network calls. `VULYK_LLM_MOCK=1` is retained for Compose/CI compatibility but does **not** override a saved provider config.
