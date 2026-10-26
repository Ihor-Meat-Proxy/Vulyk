# Automation repository

Per-project Git connection used when generating automation code so the model can reuse existing patterns.

## Connect

**Admin → Integrations → Automation repo** (requires `manage_automation_repo`):

| Field | Notes |
|-------|--------|
| Provider | `github`, `gitlab`, or `generic` |
| Remote URL | HTTPS clone URL |
| Branch | Default `main` |
| Subpath | Optional subdirectory to index |
| Token | Encrypted at rest (`VULYK_SECRETS_KEY`); omit on update to keep existing |

Buttons: **Save connection**, **Test connection** (`git ls-remote`), **Re-index** (shallow clone/fetch + index).

## Indexing

- Shallow clone/fetch into `VULYK_REPO_CACHE` (default `/tmp/vulyk-repos/{project_id}`)
- Respects `.gitignore` and skips `node_modules` / build dirs
- Language filter (TS/JS/Python/…), max file size 200 KiB, capped file count
- Chunks ~40 lines stored in `automation_repo_chunks`

## Generation context

`POST .../llm/generate-code` loads BM25-ranked chunks (boosting `playwright.config`, `pytest.ini`, `package.json`, fixtures/helpers/pages) under a token budget, appends them to the prompt, and returns `context_files` for the UI.

API:

- `GET/PUT/DELETE /api/v1/projects/{id}/automation-repo`
- `POST .../automation-repo/test`
- `POST .../automation-repo/reindex`
