# AI assist

Vulyk can call local or cloud LLMs to draft test cases, generate automation stubs, and chat over your project suite. Features appear on **Generate** (`/generate`) and **Chat** (`/chat`) when your role includes `llm_assist` and a provider is available.

## Configure a provider (admin)

System admins (and roles with `manage_llm_config`) open **Admin → LLM providers** (`/admin`, LLM tab).

Supported provider ids:

| Id | Notes |
|----|--------|
| `ollama` | Local Ollama OpenAI-compatible endpoint |
| `openai_compatible` | Unsloth / vLLM / any `/v1/chat/completions` server |
| `openrouter` | OpenRouter |
| `anthropic` | Anthropic Claude Messages API |
| `gemini` | Google Gemini |
| `openai` | OpenAI Chat Completions |
| `xai` | xAI Grok (OpenAI-compatible) |
| `mock` | Deterministic offline provider for demos/tests |

You can mark a config **Global (all projects)** or attach it to one project. API keys are encrypted at rest (`VULYK_SECRETS_KEY`). The API never returns plaintext keys (`has_api_key` only).

### Resolution order

When the UI does not pin a specific `config_id`:

1. Project-specific default, then other project configs
2. Global default, then other global configs
3. **Mock** only when no configs exist

Configured providers are never silently replaced by mock. Provider failures (for example rate limits) surface as clear errors in the UI.

Optional env: `VULYK_LLM_MOCK=1` keeps Compose/CI compatibility but does **not** override a saved provider config.

## Generate page

Open **Generate** (`/generate`).

### Cases from requirements

1. Pick the **model** (default preselected).
2. Provide requirements text and/or linked requirements context.
3. Review drafts (title, description, preconditions, priority, tags, steps).
4. Accept/create according to your role permissions.

### Automation code from cases

1. Pick the **model**.
2. Select cases with the **case picker** (search by key/title, folder tree, multi-select, folder select, chips). From the Cases list, bulk **Generate automation** opens Generate with the selection.
3. Optionally choose an **automation Git repository** for context (when the project has one or more repos under Settings → Test automation repositories).
4. Generate Playwright or pytest stubs. The model retrieves BM25-ranked context (framework configs, page objects, helpers, similar tests) and returns `context_files` for preview.
5. **Download** generated code from the UI — Vulyk does not auto-commit to Git.

### Automation repo context

Admins / Test Automation Engineers register repos under **Settings → Test automation repositories** (name, GitHub/GitLab/generic HTTPS URL, branch dropdown, optional subpath, encrypted token, optional Actions write token for Run in CI). Index via **Re-index**, webhook, or poll. See [CI/CD](cicd.md#automation-repositories).

## Chat

Open **Chat** (`/chat`).

1. Ask questions about the project’s cases (retrieval-backed Q&A).
2. The model may **propose** new cases or requirements. The UI shows a **preview card** (project + folder editable, drafts editable inline).
3. Confirm placement (“Is this the right project and folder?”), then click **Create**. Nothing is written until then.
4. Ambiguous placement returns a clarifying question instead of guessing.

Created items receive normal auto keys and are recorded with the user and model name.

### Who can create from Chat

| Capability | Admin | Test Architect | Test Automation Engineer | Developer | Viewer |
|------------|:-----:|:--------------:|:------------------------:|:---------:|:------:|
| Generate / chat ask | ✓ | ✓ | ✓ | ✓ | |
| Confirm create cases | ✓ | ✓ | ✓ | | |
| Confirm create requirements | ✓ | ✓ | | | |

Developers can ask/generate but cannot confirm creates. Viewers do not have `llm_assist`.

## Review and accept

Always treat AI output as a draft:

1. Check folder placement, priority, and steps against real product behaviour.
2. Edit fields inline on the preview before Create, or edit the case after creation.
3. Link requirements and tags as needed.
4. For automation stubs, align naming with your case keys (`KEY` in title, Playwright `{ type: 'vulyk', description: 'KEY' }`, or JUnit `test_key`) so CI ingest maps correctly.
