# W2 + W3: Case repository depth & test execution

This note documents the Case / Plan / Run surface delivered after the Phase 1 foundation.

## W2 — Case repository depth

| Capability | API / UI |
|------------|----------|
| Automatic case keys | `POST .../cases` allocates `case_key_prefix-N` via atomic `UPDATE projects SET next_case_number = next_case_number + 1 RETURNING …`. Prefix derived from project name initials (stop words/years skipped); unique across projects; editable until first case. Deletes insert `case_key_tombstones` — numbers never reused. Imports store original id in `external_key`. UI routes are `/cases/{KEY}`; `GET /api/v1/cases/by-key/{key}` resolves globally (case-insensitive). Retired keys return `410` with `error.code=case_deleted`. |
| Nested folders | Suites are unlimited-depth folders. List returns recursive `case_count` + `direct_case_count` (single CTE aggregation). `GET .../cases?suite_id=` includes descendants; `direct_only=true` / `unfiled_only=true` for tree lazy load. Cases order by `sort_order` then numeric key suffix. `DELETE .../suites/{id}?on_nonempty=move_to_parent\|delete_cases`. UI: left tree expands per-folder (paginated 100), priority bar, colour-blind status icons, drag-and-drop / Move to. |
| Steps editor | Case detail edit mode; `PUT .../cases/{id}` with `steps[]` |
| Tags | `CRUD /projects/{id}/tags`; case create/update accepts `tag_ids` / `tag_names` |
| Custom fields | `CRUD /projects/{id}/custom-fields`; `POST .../cases/{id}/custom-fields` |
| Version history | Snapshots on create/update; `GET .../cases/{id}/versions` (+ `/{version}`) |
| FTS search | `GET .../cases?q=` and `GET .../cases/search?q=` (tsvector + `pg_trgm` / ILIKE) |
| Pagination | `?limit=&offset=` → `{ items, total, limit, offset }` (default 50, max 500) |
| Bulk ops | `POST .../cases/bulk` — `delete`, `set_status`, `set_priority`, `move_suite`, `add_tags`, `remove_tags`, `set_tags` |
| PDF export | `GET .../cases/{id\|key}/export.pdf?results=5&locale=…` (also `/api/v1/cases/{id\|key}/export.pdf`); bulk `POST .../cases/export` → PDF or ZIP. Server-side genpdf + embedded DejaVu (Unicode/Cyrillic). UI: case detail **Export as PDF**, list row PDF + bulk bar. Permission: `export_reports`. |
| Virtualised list | Frontend `VirtualList` + incremental “Load more” pages |
| Customizable columns | Cases table: drag headers or Columns menu (←/→) to reorder; hide/show (Title always on). Extra columns: Automated (`external_key`), Last run (`test_results`), Updated/Created, Folder, Tags, plus `cf:<uuid>` custom-field columns via the column registry. Server-side `?sort=&order=` (whitelist + `cf:<uuid>`) with indexes; list rows enrich `automated`, `last_run_status`, `folder_name`, `tag_names`, optional `cf_ids`. Prefs: `GET/PUT/DELETE .../preferences/case-list-columns` per user/project (localStorage fallback). Reset to default in UI. |

Migrations: `003_20261025140000_case_repository_depth.sql`, `20261026150000_case_keys_and_folders.sql`, `20261026230000_case_list_columns.sql`.

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

Canonical PascalCase (DB / API / UI): `Untested`, `Passed`, `Failed`, `Blocked`, `Skipped`, `Retest`.

Input is normalized case-insensitively (and JUnit `error` → `Failed`, `skipped` → `Skipped`). Result rows are **append-only** history; never updated in place.

History APIs:

- `GET /api/v1/cases/{id}/results` (+ project-scoped twin) — paginated, filter by run/status/user/date; includes summary (latest status, pass rate)
- `GET .../runs/{id}/items/{item_id}/results` — chronological timeline for a run item
- `GET .../runs/{id}/results/history` — run-level history (optional `case_id`)
- MCP: `list_case_results`

## Out of scope (other workstreams)

Auth/RBAC (W1), JUnit ingest & CLI reporter (W4), CI triggers (W5), ALM/importers (W6/W7), LLM/MCP (W9/W10).
