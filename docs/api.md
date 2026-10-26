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
| Projects / folders (suites) / cases | `/api/v1/projects…` — project `key` is optional on create (derived from name like `case_key_prefix`); projects expose `case_key_prefix` + `next_case_number`; case create omits `key` (auto `PREFIX-N`); suites list returns recursive `case_count` + `direct_case_count`; cases list accepts `suite_id`, `direct_only`, `unfiled_only`, server-side `sort`/`order`/`cf_ids`, and returns enriched list rows; suite delete accepts `on_nonempty`; `GET /api/v1/cases/by-key/{key}` resolves a case by human-readable key (case-insensitive; `410 case_deleted` when the key is retired) |
| Case list column prefs | `GET/PUT/DELETE /api/v1/projects/{id}/preferences/case-list-columns` — per-user layout (order/visibility + optional sort); Title cannot be hidden |
| Automation repos | `/api/v1/automation-repos` (+ project links); legacy `/api/v1/projects/{id}/automation-repo` — see [automation-integration-design.md](automation-integration-design.md) |
| CI reporting | `/api/v1/projects/{id}/ci/runs` (open / results / ingest / close), project CI tokens — [guides/ci-integration.md](guides/ci-integration.md) |
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
| Admin sample data | `GET/DELETE /api/v1/admin/sample-data`, `POST /api/v1/admin/sample-data/reseed` (system admin) — list / remove / re-seed `is_sample` projects |

## Reports endpoints

Permission: `view_reports` for reads; `export_reports` for export.

- `GET …/reports/dashboard` — project dashboard
- `GET …/reports/runs/{run_id}/summary` — run summary counts
- `GET …/reports/runs/{run_id}` — run report header (source kind, env, branch/commit, pipeline, KPIs)
- `GET …/reports/runs/{run_id}/results` — paginated case results (`status`, `folder`, `suite_id`, `priority`, `assignee`, `q`, `sort`, `limit`, `offset`); includes previous status + `newly_failing` / `fixed` / `flaky` comparison
- `GET …/reports/runs/{run_id}/cases/{case_id}` — expandable steps + comment history for one case in the run
- `GET …/reports/runs/{run_id}/groups` — group by `folder` or `status` with counts and pass %
- `GET …/reports/trends` — pass/fail trends (`granularity=run|day`)
- `GET …/reports/coverage` — requirement coverage summary
- `GET …/reports/coverage/detail` — covered / uncovered requirements with linked cases and latest status
- `GET …/reports/flaky` — flaky case detection
- `GET …/reports/export?format=csv|pdf&report=dashboard|coverage|trends|flaky|run|run_results` — file export (`run_results` respects the same filters as `/results`)

UI routes: `/reports`, `/reports/runs/{runId}` (deep-linkable; query `status` pre-filters), `/reports/coverage`.

## Case PDF export

Requires `export_reports`. Labels follow `locale` query (or `Accept-Language`): `en`, `uk`, `fr`, `es`, `de`. PDFs embed DejaVu fonts (Unicode / Cyrillic). `results` defaults to **5** (max 50) and mixes manual + automated `test_results` newest first.

- `GET /api/v1/projects/{project_id}/cases/{id_or_key}/export.pdf?results=5&locale=uk` — single case by UUID or key
- `GET /api/v1/cases/{id_or_key}/export.pdf?results=5` — same, resolves project from the case
- `POST /api/v1/projects/{project_id}/cases/export` — body `{ "case_ids": ["…"], "results": 5, "locale": "en" }`; one id → PDF, multiple → ZIP of PDFs

Further reading: [architecture.md](architecture.md), [auth-and-rbac.md](auth-and-rbac.md), [guides/ci-integration.md](guides/ci-integration.md), [guides/ingest-and-ci.md](guides/ingest-and-ci.md), [user-guide.md](user-guide.md).
