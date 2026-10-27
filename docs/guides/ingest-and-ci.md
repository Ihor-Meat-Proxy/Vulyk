# Ingest & CI integrations (W4 / W5)

Upload automated results into Vulyk and trigger GitHub Actions / GitLab pipelines from the UI.

> **Preferred CI flow** (open → upload → close, project CI tokens, multi-repo settings): see **[ci-integration.md](./ci-integration.md)**. This page covers formats, one-shot ingest, dispatch, and webhooks.

## Vulyk JSON format (`vulyk/results@v1`)

```json
{
  "format": "vulyk/results@v1",
  "run": { "name": "optional run name hint" },
  "results": [
    {
      "external_id": "tests.test_api::test_health",
      "case_key": "DEMO-1",
      "title": "Health endpoint responds",
      "status": "passed",
      "message": "",
      "duration_ms": 12,
      "executed_at": "2026-10-25T12:00:00Z"
    }
  ]
}
```

Statuses: `passed`, `failed`, `skipped`, `error`, `blocked` (aliases like `pass` / `fail` are normalized).

`case_key` optionally links a result to an existing case in the run’s project. Unmatched keys are returned in the ingest response.

## JUnit XML

pytest, Playwright, Surefire, and similar reporters are supported. Each `<testcase>` becomes a result with `external_id` = `classname::name` (or `name` alone). Keys like `KC-12` in the title are mapped automatically; unmatched keys are returned in the response.

## Playwright / pytest JSON

| Format | Flag | Notes |
|--------|------|-------|
| Playwright JSON reporter | `playwright_json` | Nested `suites` / `specs` / `tests` |
| pytest-json-report | `pytest_json` | `tests[].nodeid` + `outcome`; `metadata.case_key` supported |

## API

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/v1/projects/{id}/runs` | Create a run |
| `POST` | `/api/v1/projects/{id}/ingest` | **Pipeline ingest**: auto campaign/iteration/suite + parse results |
| `POST` | `/api/v1/runs/{id}/ingest` | Parse JUnit / Vulyk / Playwright / pytest JSON into an existing run |
| `POST` | `/api/v1/runs/{id}/results:bulk` | Bulk insert already-normalized results |
| `GET` | `/api/v1/jobs/{id}` | Poll background ingest jobs |
| `POST` | `/api/v1/ci-configs/{id}/dispatch` | Trigger GitHub `workflow_dispatch` or GitLab pipeline |
| `POST` | `/api/v1/webhooks/github[/{config_id}]` | Sync GitHub Actions status |
| `POST` | `/api/v1/webhooks/gitlab[/{config_id}]` | Sync GitLab pipeline status |

Campaign / iteration / suite dashboard APIs: see [executions.md](executions.md).

Payloads larger than ~200 results (or `async_mode: true`) are processed by the Postgres job worker (`SKIP LOCKED`).

## CLI

```bash
cargo build -p vulyk-cli --release
./target/release/vulyk report \
  --file report.xml \
  --create-run \
  --project-key DEMO \
  --api-url http://127.0.0.1:8080 \
  --run-name "local pytest"
```

Environment: `VULYK_API_URL`, `VULYK_API_TOKEN`, `VULYK_RUN_ID`, `VULYK_PROJECT_KEY`, `VULYK_PROJECT_ID`, `VULYK_RUN_NAME`.

## Shell uploader (Action / GitLab)

```bash
./scripts/vulyk-upload.sh \
  --api-url http://127.0.0.1:8080 \
  --file report.xml \
  --create-run \
  --project-key DEMO

# Preferred for CI: auto-create campaign + iteration + suite
./scripts/vulyk-upload.sh \
  --api-url http://127.0.0.1:8080 \
  --file playwright.json \
  --format playwright_json \
  --pipeline \
  --project-key DEMO \
  --campaign Integration_Automation \
  --iteration "Daily - main" \
  --created-by "$GITHUB_ACTOR"
```

## GitHub Action

```yaml
- uses: Ihor-Meat-Proxy/Vulyk/action@main
  with:
    api-url: ${{ vars.VULYK_API_URL }}
    api-token: ${{ secrets.VULYK_API_TOKEN }}
    project-key: DEMO
    file: junit.xml
    format: junit
```

## GitLab CI template

See [`ci/gitlab-upload.yml`](../../ci/gitlab-upload.yml). Set `VULYK_API_URL`, `VULYK_PROJECT_KEY`, and `VULYK_RESULTS_FILE`, then `extends: .vulyk_upload`.

## CI config JSON

**GitHub**

```json
{
  "token": "ghp_…",
  "owner": "org",
  "repo": "repo",
  "workflow_id": "ci.yml",
  "ref": "main"
}
```

**GitLab**

```json
{
  "base_url": "https://gitlab.com",
  "project_id": "group/project",
  "trigger_token": "…",
  "ref": "main"
}
```

**Required `webhook_secret`** on the CI config (fail-closed): configs without a
verifying secret do **not** authorize webhook traffic. GitHub uses
`X-Hub-Signature-256`, GitLab uses `X-Gitlab-Token`. Prefer config-scoped URLs:
`/api/v1/webhooks/github/{config_id}` or `/api/v1/webhooks/gitlab/{config_id}`.
API responses expose `has_webhook_secret` / `has_token` only — raw secrets are
never returned after create/update.

## UI

**Runs** lists project runs and ingested results, and lets you save CI configs + dispatch pipelines. Status updates arrive via webhooks and are mirrored onto the linked Vulyk run.
