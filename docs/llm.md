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

## Secrets

API keys are encrypted at rest (AES-256-GCM). Set `VULYK_SECRETS_KEY` to a 32-byte value (base64 or hex) or a passphrase (SHA-256 derived). Plaintext keys are **never** returned by the API (`has_api_key` only).

For CI and local tests without credentials:

```bash
export VULYK_LLM_MOCK=1
```

When no config exists (or mock is forced), generation uses the mock provider — no network calls, no real keys.
