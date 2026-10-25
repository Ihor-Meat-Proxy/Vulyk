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
- Errors: `{ "error": { "code": "...", "message": "..." } }`
- Health: `GET /health`, `GET /ready`

## Major surfaces

| Area | Paths |
|------|-------|
| Auth | `/api/v1/auth/*` |
| Projects / suites / cases | `/api/v1/projects…` |
| Plans / runs / results | `/api/v1/projects/{id}/plans`, `/runs` |
| Ingest | `POST /api/v1/runs/{id}/ingest`, `…/results:bulk` |
| Reports (W8) | `/api/v1/projects/{id}/reports/*` |
| Requirements / ALM | `/requirements`, `/integrations`, `/defects` |
| Importers | `/api/v1/importers`, `/imports` |
| LLM | `/api/v1/projects/{id}/llm/*` |
| Tokens | `/api/v1/tokens` |

## Reports endpoints

- `GET …/reports/dashboard` — project dashboard
- `GET …/reports/runs/{run_id}/summary` — run summary
- `GET …/reports/trends` — pass/fail trends (`granularity=run|day`)
- `GET …/reports/coverage` — requirement coverage
- `GET …/reports/flaky` — flaky case detection
- `GET …/reports/export?format=csv|pdf&report=dashboard|coverage|trends|flaky|run` — file export

Further reading: [architecture.md](architecture.md), [auth-and-rbac.md](auth-and-rbac.md), [guides/ingest-and-ci.md](guides/ingest-and-ci.md).
