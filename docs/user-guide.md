# Vulyk user guide

Vulyk is a test management system: organize cases into suites, plan and execute runs, ingest CI results, link requirements, and review dashboards.

## Sign in

1. Open the UI (default `http://localhost:5173`).
2. On a fresh install, **Create the first admin** (bootstrap).
3. Later visits use **Sign in** with email/password.
4. Personal API tokens for CLI/MCP live under **Admin → Tokens**.

When auth is disabled (`VULYK_AUTH_REQUIRED=false`, local compose default), the UI runs in open mode without a login wall unless bootstrap is still available.

## Projects & cases

- Use the project switcher in the top bar.
- **Cases** lists cases with FTS search, tag/status filters, and bulk status updates.
- **Create case** from the toolbar (key + title). Open a case for steps, tags, custom fields, and version history.
- **Import** brings cases from TestLink, TestRail, Xray, Zephyr, Squash, qTest, Qase, Kiwi, or CSV.

## Plans & runs

- **Runs** creates an execution queue from a plan, suite, or all cases.
- Open a run to execute items, record step results, and attach notes.
- On phones, use the bottom nav and large Pass / Fail / Blocked controls; you can attach a camera photo to a result.
- Offline: recently viewed cases/runs stay readable; step results queue locally and sync when you are back online. See [PWA and mobile](guides/pwa-and-mobile.md).
- **Upload JUnit results** on the Runs page to ingest CI XML into a selected run.
- **CI** configures GitHub Actions / GitLab triggers and dispatch history.

## Requirements & coverage

- **Requirements** tracks manual or ALM-synced requirements and links them to cases.
- **Reports** shows run summaries, pass/fail trends, requirement coverage, flaky detection, and CSV/PDF export.

## LLM assists

- **Generate** drafts cases or automation code (provider configured by an admin).
- **Chat** asks questions over the project’s cases.

## CLI reporter

```bash
cargo run -p vulyk-cli -- report \
  --file report.xml --format junit \
  --create-run --project-key DEMO
```

See [guides/ingest-and-ci.md](guides/ingest-and-ci.md).
