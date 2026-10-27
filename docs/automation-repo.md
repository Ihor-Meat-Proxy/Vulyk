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
| Token | Encrypted at rest (`VULYK_SECRETS_KEY`); omit on update to keep existing; shown masked |
| Projects | Multi-select; a repo may belong to several projects |

Buttons: **Save**, **Test connection** (`git ls-remote`), **Re-index** (shallow clone/fetch + index).

Legacy **Admin → Integrations → Automation repo** (singular per project) remains as a shim during migration.

## Indexing

- Shallow clone/fetch into `VULYK_REPO_CACHE` (default `/tmp/vulyk-repos/{repo_id}`)
- Respects `.gitignore` and skips `node_modules` / build dirs
- Language filter (TS/JS/Python/…), max file size 200 KiB, capped file count
- Chunks ~40 lines stored in `automation_repo_chunks`

## Generation context

`POST .../llm/generate-code` loads BM25-ranked chunks (boosting `playwright.config`, `pytest.ini`, `package.json`, fixtures/helpers/pages) under a token budget, appends them to the prompt, and returns `context_files` for the UI. When several repos are linked to the project, pass `repo_id` (Generate page repo picker).

API:

- `GET/POST /api/v1/automation-repos`, `GET/PATCH/DELETE /api/v1/automation-repos/{id}`
- `PUT /api/v1/automation-repos/{id}/projects`, `GET /api/v1/projects/{id}/automation-repos`
- `POST .../test`, `POST .../reindex`
- Legacy: `GET/PUT/DELETE /api/v1/projects/{id}/automation-repo`
