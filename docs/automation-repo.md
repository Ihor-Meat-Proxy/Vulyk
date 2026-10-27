# Automation repository

Named Git connections used when generating automation code so the model can reuse existing patterns. Repos are **app-level** and attach to **many projects** (many-to-many). Operator guide: [guides/ci-integration.md](./guides/ci-integration.md#automation-repositories-generate). Design archive: [automation-integration-design.md](./automation-integration-design.md).

## Connect

**Settings → Test automation repositories** (requires `manage_automation_repo` — Admin or Test Automation Engineer):

| Field | Notes |
|-------|--------|
| Name | Display label |
| Provider | `github`, `gitlab`, or `generic` |
| Remote URL | **Public HTTPS** clone URL (SSRF guard). Private/link-local/`file://` remotes are rejected unless `VULYK_EGRESS_ALLOW_PRIVATE=1` (lab/tests only) |
| Branch | Default `main` |
| Subpath | Optional subdirectory to index |
| Token | Encrypted at rest (`VULYK_SECRETS_KEY`); omit on update to keep existing; shown masked. Used as a Bearer token for GitHub/GitLab REST; optional for public repos |
| Actions write token | Separate encrypted GitHub token with `actions:write` for **Run in CI** (`workflow_dispatch`) |
| CI workflow file / ref | Workflow path or id and git ref for dispatch (GitHub only) |
| Projects | Multi-select; a repo may belong to several projects |

Buttons:

- **Save** — persist the connection
- **Test connection** — works on **unsaved form values** via `POST /api/v1/automation-repos/test` (never a raw 404). Saved repos without a new token use the stored encrypted token
- **Re-index** — requires a saved repo; refreshes the file/chunk index

Legacy **Admin → Integrations → Automation repo** (singular per project) remains as a shim during migration. Re-index stays disabled until the connection is saved; Test connection uses form values or the preview endpoint.

Status line shows `index_status`, file/chunk counts, **last indexed** time, short commit SHA, and whether a token is set.

## Indexing

### GitHub / GitLab (no local `git`)

- Tree and file contents are fetched through the provider **REST API** (GitHub `/git/trees` + blobs, GitLab `/repository/tree` + blobs)
- Respects **branch** and optional **subpath**
- **Incremental**: only blobs whose SHA differs from the stored `content_hash` are re-fetched; removed paths are dropped
- Rate limits: short backoff retries, then a clear error including remaining quota / reset
- Public remotes work without a token; private remotes need a read token

### Generic HTTPS Git

- Shallow clone/fetch into `VULYK_REPO_CACHE` (default `/tmp/vulyk-repos/{repo_id}`)
- The API image includes the **`git`** binary for this path. If `git` is missing, Test connection / Re-index return an explicit message (use `github`/`gitlab` instead)
- Respects `.gitignore` and skips `node_modules` / build dirs

### Shared limits

- Language filter (TS/JS/Python/…), max file size 200 KiB, capped file count
- Chunks ~40 lines stored in `automation_repo_chunks`

## Generation context

`POST .../llm/generate-code` loads BM25-ranked chunks (boosting `playwright.config`, `pytest.ini`, `package.json`, fixtures/helpers/pages) under a token budget, appends them to the prompt, and returns `context_files` for the UI. When several repos are linked to the project, pass `repo_id` (Generate page repo picker).

## API

- `GET/POST /api/v1/automation-repos`, `GET/PATCH/DELETE /api/v1/automation-repos/{id}`
- `PUT /api/v1/automation-repos/{id}/projects`, `GET /api/v1/projects/{id}/automation-repos`
- `POST /api/v1/automation-repos/test` — preview Test connection from form body (`provider`, `remote_url`, `branch`, optional `token` / `project_ids`)
- `POST .../automation-repos/{id}/test`, `POST .../reindex`
- Legacy: `GET/PUT/DELETE /api/v1/projects/{id}/automation-repo` (+ `/test`, `/reindex`)

## Upgrade notes

- **No database migration** for REST indexing (reuses `content_hash` for blob SHAs)
- **API image** installs `git` so the generic provider works in containers. GitHub/GitLab indexing does not require it. Rebuild/redeploy the API image on upgrade (`install.sh upgrade` / Helm image bump) — see [guides/upgrade.md](./guides/upgrade.md)
