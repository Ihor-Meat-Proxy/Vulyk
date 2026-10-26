# Vulyk API

Interactive OpenAPI docs are served by the API process:

| Environment | URL |
|-------------|-----|
| Local / Compose | http://localhost:8080/api/docs |
| Behind the UI proxy | http://localhost:5173/api/docs (dev) or same-origin `/api/docs` |

The machine-readable OpenAPI document is embedded via **utoipa** (`ApiDoc` in `backend/src/openapi.rs`).

## Conventions

- Base path: `/api/v1`
- Auth: session cookie or `Authorization: Bearer <jwt|api_token>`
- Errors: `{ "error": { "code": "...", "message": "..." } }` (English messages; UI shows them in any locale without crashing — seed content may be localized via bilingual fields)
- Locales: instance `default_locale` and `PATCH /api/v1/auth/me` `locale` accept `en` / `uk` / `fr` / `es` / `de`
- Health: `GET /health`, `GET /ready`

## Major surfaces

| Area | Paths |
|------|-------|
| Auth | `/api/v1/auth/*` |
| Projects / folders (suites) / cases | `/api/v1/projects…` — project `key` is optional on create (derived from name like `case_key_prefix`); projects expose `case_key_prefix` + `next_case_number`; case create omits `key` (auto `PREFIX-N`); suites list returns recursive `case_count` + `direct_case_count`; cases list accepts `suite_id`, `direct_only`, `unfiled_only` and sorts by numeric key; suite delete accepts `on_nonempty` |
| Automation repo | `/api/v1/projects/{id}/automation-repo` (+ `/test`, `/reindex`) — encrypted Git token, shallow index for generate-code context |
| LLM generate | `config_id` optional; generate-code returns `context_files` from the automation repo index |
| Plans / runs / results | `/api/v1/projects/{id}/plans`, `/runs` |
| Executions (campaigns / iterations / suites) | `/api/v1/projects/{id}/executions/tree`, `/campaigns`, `/iterations`, `/suites-runs` — see [guides/executions.md](guides/executions.md) |
| Ingest | `POST /api/v1/projects/{id}/ingest` (pipeline), `POST /api/v1/runs/{id}/ingest` (`junit` / `vulyk_json` / `playwright_json` / `pytest_json`), `…/results:bulk` |
| Reports (W8) | `/api/v1/projects/{id}/reports/*` |
| Requirements / ALM | `/requirements`, `/integrations`, `/defects` |
| Importers | `/api/v1/importers`, `/imports` |
| LLM | `/api/v1/projects/{id}/llm/*` (incl. `chat`, `chat/confirm`) |
| Chat tools | `GET /api/v1/llm/chat-tools` |
| Requirement folders | `/api/v1/projects/{id}/requirement-folders` |
| Tokens | `/api/v1/tokens` |

## Reports endpoints

- `GET …/reports/dashboard` — project dashboard
- `GET …/reports/runs/{run_id}/summary` — run summary
- `GET …/reports/trends` — pass/fail trends (`granularity=run|day`)
- `GET …/reports/coverage` — requirement coverage
- `GET …/reports/flaky` — flaky case detection
- `GET …/reports/export?format=csv|pdf&report=dashboard|coverage|trends|flaky|run` — file export

Further reading: [architecture.md](architecture.md), [auth-and-rbac.md](auth-and-rbac.md), [guides/ingest-and-ci.md](guides/ingest-and-ci.md).
