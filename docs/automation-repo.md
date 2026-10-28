# Automation repository

Named Git connections used when generating automation code so the model can reuse existing patterns. Repos are **app-level** and attach to **many projects** (many-to-many). Operator guide: [guides/ci-integration.md](./guides/ci-integration.md#automation-repositories-generate). Design archive: [automation-integration-design.md](./automation-integration-design.md).

## Connect

**Settings → Test automation repositories** (requires `manage_automation_repo` — Admin or Test Automation Engineer):

| Field | Notes |
|-------|--------|
| Name | Display label |
| Provider | `github`, `gitlab`, or `generic` |
| Remote URL | **Public HTTPS** clone URL (SSRF guard). Private/link-local/`file://` remotes are rejected unless `VULYK_EGRESS_ALLOW_PRIVATE=1` (lab/tests only) |
| Branch | Selected branch to index (dropdown lists remote branches for GitHub/GitLab; newly seen names are marked) |
| Subpath | Optional subdirectory to index |
| Token | Encrypted at rest (`VULYK_SECRETS_KEY`); omit on update to keep existing; shown masked. Used as a Bearer token for GitHub/GitLab REST; **recommended** for public repos too (raises rate limits above the unauthenticated 60 req/h) |
| Actions write token | Separate encrypted GitHub token with `actions:write` for **Run in CI** (`workflow_dispatch`) |
| CI workflow file / ref | Workflow path or id and git ref for dispatch (GitHub only) |
| Poll interval | Minutes between cheap branch-head checks (default **15**; **0** disables). Uses `ETag` / `If-None-Match` so unchanged tips do not burn rate limit |
| Webhook URL / secret | Per-repo generated secret + URL for push-driven re-index (shown after save) |
| Projects | Multi-select; a repo may belong to several projects |

Buttons:

- **Save** — persist the connection (generates webhook secret on create)
- **Test connection** — works on **unsaved form values** via `POST /api/v1/automation-repos/test` (never a raw 404). Saved repos without a new token use the stored encrypted token
- **Re-index** — requires a saved repo; refreshes the file/chunk index from a single archive download
- **Refresh branches** — lists remote branches for the dropdown
- **Rotate secret** — regenerates the webhook shared secret

Legacy **Admin → Integrations → Automation repo** (singular per project) remains as a shim during migration. Re-index stays disabled until the connection is saved; Test connection uses form values or the preview endpoint.

Status line shows `index_status`, file/chunk counts, **last indexed** time (local), short commit SHA, **trigger** (`manual` / `webhook` / `poll`), and whether a token is set.

## Indexing

### GitHub / GitLab (no local `git`)

Indexing downloads **one archive** for the branch tip and extracts only the configured subpath:

| Provider | Endpoint |
|----------|----------|
| GitHub | `GET /repos/{owner}/{repo}/zipball/{ref}` |
| GitLab | `GET /projects/{id}/repository/archive.zip?sha={ref}` |

- Works **without a token** on public remotes (still subject to host rate limits)
- Respects **branch** and optional **subpath**
- Path safety: archive entries with `..`, absolute paths, or unsafe components are skipped
- Size caps: max archive **50 MiB**, max file **200 KiB**, language filter + max file count
- **Incremental persist**: content SHA-256 compared to stored `content_hash`; unchanged paths keep chunks; removed paths are dropped
- Rate limits: short backoff retries, then a structured `git_host_rate_limit` error with **reset time**. The UI shows a translated message with reset in **local time** and a hint to add a read-only token

Branch tip / Test connection uses a single commits API call (also the basis for polling with ETag).

### Generic HTTPS Git

- Shallow clone/fetch into `VULYK_REPO_CACHE` (default `/tmp/vulyk-repos/{repo_id}`)
- The API image includes the **`git`** binary for this path. If `git` is missing, Test connection / Re-index return an explicit message (use `github`/`gitlab` instead)
- Respects `.gitignore` and skips `node_modules` / build dirs

### Shared limits

- Language filter (TS/JS/Python/…), max file size 200 KiB, capped file count
- Chunks ~40 lines stored in `automation_repo_chunks`

## Automatic re-index

### Webhook (preferred)

`POST /api/v1/webhooks/automation-repos/{repo_id}` (unauthenticated; secret-verified):

- **GitHub**: `X-Hub-Signature-256` HMAC of the body with the per-repo secret; handles `push` when `ref` matches the configured branch
- **GitLab**: `X-Gitlab-Token` must equal the per-repo secret; handles Push Hook for the configured branch

Skipped when an index is already `indexing`. Trigger recorded as `webhook`.

### Polling (fallback)

Background poller (tick every `VULYK_AUTOMATION_REPO_POLL_TICK_SECS`, default 60s) loads repos whose `poll_interval_minutes > 0` are due:

1. `GET` branch tip with `If-None-Match: <poll_etag>`
2. HTTP **304** → update `last_polled_at` only (does not consume GitHub rate limit when ETag matches)
3. New SHA different from `commit_sha` → re-index with trigger `poll`
4. Skip if status is already `indexing`

## Generation context

`POST .../llm/generate-code` loads BM25-ranked chunks (boosting `playwright.config`, `pytest.ini`, `package.json`, fixtures/helpers/pages) under a token budget, appends them to the prompt, and returns `context_files` for the UI. When several repos are linked to the project, pass `repo_id` (Generate page repo picker).

## API

- `GET/POST /api/v1/automation-repos`, `GET/PATCH/DELETE /api/v1/automation-repos/{id}`
- `PUT /api/v1/automation-repos/{id}/projects`, `GET /api/v1/projects/{id}/automation-repos`
- `POST /api/v1/automation-repos/test` — preview Test connection from form body
- `POST /api/v1/automation-repos/branches` — preview remote branches from form body
- `GET .../automation-repos/{id}/branches` — list branches (marks newly appeared names)
- `POST .../automation-repos/{id}/test`, `POST .../reindex`
- `POST /api/v1/webhooks/automation-repos/{id}` — push webhook
- Legacy: `GET/PUT/DELETE /api/v1/projects/{id}/automation-repo` (+ `/test`, `/reindex`)

## Upgrade notes

- Migration `20261028102013_automation_repo_archive_sync.sql` adds `webhook_secret` (backfilled), `poll_interval_minutes` (default **15**), `last_index_trigger`, `poll_etag`, `last_polled_at`, `seen_branches` — upgrade-safe defaults; existing `.env` unchanged
- Optional env: `VULYK_AUTOMATION_REPO_POLL_TICK_SECS` (default 60, min 15) — poller wake interval only; per-repo interval stays in the DB
- **API image** still installs `git` for the generic provider. GitHub/GitLab archive indexing does not require it. Rebuild/redeploy the API image on upgrade (`install.sh upgrade` / Helm image bump) — see [guides/upgrade.md](./guides/upgrade.md)
