# LLM providers (W9)

Vulyk can call local and cloud LLMs to:

1. **Generate test cases** from requirements text, linked requirements, or a **PR/branch diff**  
2. **Generate automation** (Playwright or pytest) from existing cases  
3. **Chat with your tests** — project-scoped Q&A, plus propose/create cases and requirements after confirmation  

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

Configure per project under gear → **Admin settings** → **LLM providers**, or via:

- `GET /api/v1/llm/providers`
- `CRUD /api/v1/projects/{id}/llm/configs`
- `POST /api/v1/projects/{id}/llm/generate-cases`
- `POST /api/v1/projects/{id}/llm/diff-preview` — list changed files + token budget for a PR/branch
- `POST /api/v1/projects/{id}/llm/generate-cases-from-diff` — generate drafts from a selected diff
- `GET /api/v1/automation-repos/{id}/pulls` — open PRs/MRs (GitHub, GitLab, Forgejo/Gitea)
- `POST /api/v1/projects/{id}/llm/generate-code`
- `POST /api/v1/projects/{id}/llm/ask` — plain Q&A
- `POST /api/v1/projects/{id}/llm/chat` — Q&A **or** propose cases/requirements (JSON-schema / tool fallback)
- `POST /api/v1/projects/{id}/llm/chat/confirm` — persist an edited proposal (RBAC gated)
- `GET /api/v1/llm/chat-tools` — tool catalog (`list_folders`, `search_cases`, `propose_test_cases`, `propose_requirements`)

## Resolution order

For chat, case generation, and automation generation (when `config_id` is omitted):

1. **Project-specific** default, then any other project config  
2. **Global** default (`project_id` null), then any other global config  
3. **Mock** — only when no configs exist  

Configured providers are never silently replaced by mock. If a real provider call fails (for example OpenRouter HTTP 429 on a free model), the API returns a clear error that includes the provider and model; the UI surfaces that error instead of falling back to mock.

Mark a config **Global (all projects)** in Admin → LLM providers to share one default across projects.

## Generate page

- **Model** dropdown on “Cases from requirements”, “Cases from a PR or branch diff”, and “Automation code” (default preselected).
- **Diff generation**: pick a linked GitHub / GitLab / Forgejo / Gitea repo, an open PR/MR or `base…head` branch compare, preview changed files with an approximate token budget, include/exclude paths, then generate. Saved cases store provenance in `case_generation_sources` (repo, PR number/URL, base/head refs, commit SHA, included paths) and expose it on `TestCaseDetail.generation`.
- **Case picker** (search by key/title, folder tree, multi-select, folder select, chips). Cases page bulk action **Generate automation** opens Generate with the selection.
- **Automation repo** (Settings → Test automation repositories): connect GitHub / GitLab / Bitbucket / Forgejo / Gitea / generic HTTPS (token encrypted). Hosted providers re-index via REST archive download; generic remotes still shallow-clone. Generate-code retrieves BM25 context and returns `context_files` for preview. See [automation-repo.md](./automation-repo.md) and [Run in CI](./guides/ci-integration.md#two-way-trigger-run-in-ci).

## Chat propose / create

From a natural-language prompt the model can draft one or more **test cases** (title, description, preconditions, priority, tags, ordered steps) or **requirements** (title, description, acceptance criteria), optionally linking cases to requirements.

Placement uses retrieval over the suite / requirement-folder tree and similar items. The UI shows a **preview card** (project + folder editable, drafts editable inline, short reason) and asks *“Is this the right project and folder?”*. Nothing is written until **Create**. Ambiguous placement returns a clarifying question instead of guessing.

Models without native tool calling return a JSON envelope (`propose_cases` / `propose_requirements` / `clarify` / `answer`). Created items receive normal auto keys and are recorded in `chat_creation_events` with the user and model name.

### RBAC

| Capability | Admin | Test Architect | Test Automation Engineer | Developer | Viewer |
|------------|:-----:|:--------------:|:------------------------:|:---------:|:------:|
| Generate / chat ask (`llm_assist`) | ✓ | ✓ | ✓ | ✓ | |
| Confirm create cases | ✓ | ✓ | ✓ | | |
| Confirm create requirements | ✓ | ✓ | | | |
| Configure automation repo | ✓ | ✓ | ✓ | | |

## Secrets

API keys are encrypted at rest (AES-256-GCM). Set `VULYK_SECRETS_KEY` to a 32-byte value (base64 or hex) or a passphrase (SHA-256 derived). Plaintext keys are **never** returned by the API (`has_api_key` only).

For CI and local tests without credentials you can leave providers unconfigured — the mock adapter answers with `provider=mock` / `model=mock-1` and makes no network calls. `VULYK_LLM_MOCK=1` is retained for Compose/CI compatibility but does **not** override a saved provider config.
