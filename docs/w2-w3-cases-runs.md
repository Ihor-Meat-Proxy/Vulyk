# W2 + W3: Case repository depth & test execution

This note documents the Case / Plan / Run surface delivered after the Phase 1 foundation.

## W2 — Case repository depth

| Capability | API / UI |
|------------|----------|
| Steps editor | Case detail edit mode; `PUT .../cases/{id}` with `steps[]` |
| Tags | `CRUD /projects/{id}/tags`; case create/update accepts `tag_ids` / `tag_names` |
| Custom fields | `CRUD /projects/{id}/custom-fields`; `POST .../cases/{id}/custom-fields` |
| Version history | Snapshots on create/update; `GET .../cases/{id}/versions` (+ `/{version}`) |
| FTS search | `GET .../cases?q=` and `GET .../cases/search?q=` (tsvector + `pg_trgm` / ILIKE) |
| Pagination | `?limit=&offset=` → `{ items, total, limit, offset }` (default 50, max 500) |
| Bulk ops | `POST .../cases/bulk` — `delete`, `set_status`, `set_priority`, `move_suite`, `add_tags`, `remove_tags`, `set_tags` |
| Virtualised list | Frontend `VirtualList` + incremental “Load more” pages |

Migrations: `003_20261025140000_case_repository_depth.sql`.

## W3 — Plans, runs, results

| Capability | API / UI |
|------------|----------|
| Plans | `CRUD /projects/{id}/plans` with optional `case_ids` |
| Create run | `POST .../runs` from `plan_id`, `suite_ids`, `case_ids`, `case_statuses`, `tag_names` |
| Run progress | Counts by status + percent on run detail |
| Execute step-by-step | `GET .../runs/{id}/items/{item_id}`; UI statuses + per-step status |
| Results (manual execute) | `POST .../runs/{id}/results/execute` with optional step statuses; syncs `run_items` |
| Results (bulk ingest / MCP) | `POST .../runs/{id}/results` with `{ "results": [...] }` |
| Trigger | `POST .../runs/{id}/trigger` sets status `running` |
| Assignees / comments | On `run_items` and results |
| Attachments | `POST .../attachments` (base64, 5 MiB) for `case` / `result` / `run` |

Migrations: `004_20261025140100_plans_runs_results.sql`.

## Status vocabulary

Run items / manual results: `untested`, `passed`, `failed`, `blocked`, `skipped`, `retest` (results also allow `error` for ingest).

## Out of scope (other workstreams)

Auth/RBAC (W1), JUnit ingest & CLI reporter (W4), CI triggers (W5), ALM/importers (W6/W7), LLM/MCP (W9/W10).
