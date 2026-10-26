# Automation executions

SquashTM-inspired **Campaigns → Iterations → Automated suites** workspace with a TestRail/Xray-like UI.

## Concepts

| Concept | Meaning |
|---------|---------|
| **Campaign** | Nested folder for organizing iterations (e.g. `Integration_Automation`) |
| **Iteration** | Test cycle with assigned cases, schedule, cadence, and live status |
| **Automated suite** | One CI pipeline upload (a `test_run` linked to an iteration) |
| **Execution** | A single case result inside a suite |

Dashboard statuses (colour-blind-safe icon + label): **Ready**, **Running**, **Passed**, **Failed**, **Blocked**, **Skipped**, **Untestable**. Legacy **Untested** / **Retest** map into **Ready** on the donut.

## UI

1. Open **Executions** in the sidebar (or bottom nav on mobile).
2. Select an iteration in the left tree.
3. **Dashboard** — status donut, pass-rate trend, flaky / slowest tests, editable schedule & cadence.
4. **Automated suites** — filterable table with Details drawer, report/artifact links, delete / rerun (RBAC).
5. Suite **Details** — per-case results with environment, expandable errors, link to case.
6. Case page **Executions** tab — full history across iterations with sparkline and pass-rate.

## Permissions

| Action | Permission |
|--------|------------|
| View tree / dashboard / suites | `view_project` |
| Create/edit campaigns & iterations | `manage_plans` |
| Delete automated suite | `execute_runs` |
| Rerun suite shell | `trigger_ci` |
| Upload CI results | `upload_results` |

## CI reporting (recommended)

For open → upload → close with project CI tokens, GitHub Action, and GitLab template, see **[ci-integration.md](./ci-integration.md)**.

## Pipeline ingest API

Auto-create campaign + iteration + suite run and ingest results:

```http
POST /api/v1/projects/{project_id}/ingest
```

```json
{
  "format": "junit",
  "content": "<?xml …>",
  "campaign": "Integration_Automation",
  "iteration": "Daily - dev (17.8) + NC 34",
  "cadence": "daily",
  "run_name": "CI #42",
  "created_by_name": "WALL-E-rror",
  "environment": { "runner": "Playwright", "browser": "chromium" },
  "artifacts": [
    { "kind": "report", "label": "HTML", "url": "https://ci.example/report/" },
    { "kind": "trace", "url": "https://ci.example/trace.zip" }
  ]
}
```

Supported formats: `junit`, `vulyk_json`, `playwright_json`, `pytest_json`.

Cases are matched by **Vulyk key** in the title / annotation (e.g. `KC-12`, `@vulyk:KC-12`) or by `external_key`.

## Shell uploader

```bash
./scripts/vulyk-upload.sh \
  --api-url http://127.0.0.1:8080 \
  --token "$VULYK_API_TOKEN" \
  --project-key KYIV_CONCERT_2026 \
  --pipeline \
  --campaign Integration_Automation \
  --iteration "Daily - dev" \
  --created-by "$GITHUB_ACTOR" \
  --environment '{"runner":"Playwright"}' \
  --artifacts '[{"kind":"report","url":"https://…"}]' \
  --file playwright-report.json \
  --format playwright_json
```

Legacy `--create-run` / `--run-id` still targets `POST /api/v1/runs/{id}/ingest`.

## Sample data

The Kyiv Open-Air Concert sample project seeds `Sample Integration_Automation` (plus release campaigns), daily/manual iterations, and ~15 **Sample nightly #N** automated suites (creator **Sample CI**, artifact URLs on `example.invalid`) with varied outcomes — mostly passed, a few failed, one in progress, one cancelled — while keeping a flaky case for flaky-test detection. All sample entities carry `is_sample` and can be removed/re-seeded from Admin → Settings → Sample data.
