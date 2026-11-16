# Runs and executions

Vulyk separates **manual/planned runs** from the **Executions** workspace used for CI campaigns. Both store case results and feed Reports.

## Runs page

Open **Runs** (`/runs`). You need **manage plans** to create plans; **execute runs** / **upload results** to execute or ingest.

### Plans

1. Create a **plan** with a name (optional organisation of case sets).
2. Plans appear in the create-run form so you can start a run from a plan.

### Create a run

1. Enter a run name.
2. Optionally select a **plan** and/or **suite/folder**.
3. If neither plan nor suite is selected, the run can include **all cases**.
4. Create the run and open it at `/runs/{id}`.

### Execute manually

On the run detail page:

1. Select a case in the queue.
2. Record step results and an overall result status.
3. Add comments/notes as needed.
4. Attach files or (on mobile) a camera photo to a result when attachments are allowed.

**Result statuses:** Untested / Ready, Running, Passed, Failed, Blocked, Skipped, Untestable, Retest.

Every change becomes an immutable **result history** row (who, when, comment, elapsed, case version, attachments/defects). Status chips always show **icon + translated label** (colour-blind-safe palette).

On phones, use the bottom nav and large Passed / Failed / Blocked / Skipped controls.

### Upload JUnit into a run

On the Runs page you can **upload JUnit results** into a selected run. Mapping rules match CI ingest (keys in titles/properties). JUnit `skipped` → Skipped; JUnit `error` → Failed. Prefer project CI tokens and the open → upload → close flow for pipelines — see [CI/CD](cicd.md).

### Offline (PWA)

Recently viewed cases/runs can stay readable offline; step results may queue locally and sync when you reconnect, depending on your instance’s PWA support.

## Case-centric history

On a case:

- **History / Results** — all results across runs with filters and pass-rate summary
- **Executions** — automation history across iterations with a sparkline

The run execution panel also shows a **Result history** timeline for the selected case.

## Executions workspace

Open **Executions** (`/executions`).

1. Select an **iteration** in the left tree (Campaigns → Iterations).
2. **Dashboard** — status donut, pass-rate trend, flaky/slowest tests, editable schedule and cadence.
3. **Automated suites** — filterable table (created on/by, duration, pass/fail counts), Details drawer, report/artifact links, delete/rerun by RBAC.
4. Suite Details — per-case results with environment, expandable errors, link to case.
5. Deep links: `/executions/iterations/{id}`, `/executions/runs/{runId}`, `/executions/runs/{runId}/cases/{caseId}`.

### Permissions (Executions)

| Action | Permission |
|--------|------------|
| View tree / dashboard / suites | `view_project` |
| Create/edit campaigns & iterations | `manage_plans` |
| Delete automated suite | `execute_runs` |
| Rerun suite / Run in CI | `trigger_ci` |
| Upload CI results | `upload_results` |

### Run in CI

When a GitHub automation repo has an Actions write token and workflow configured, open a suite and click **Run in CI**. Vulyk creates a **Queued** run, dispatches `workflow_dispatch` with `vulyk_run_id` and `case_keys`, and shows the pipeline link when the workflow starts. Set `VULYK_RUN_ID` in the job so reporters **attach** to that run instead of opening a new one. Details: [CI/CD](cicd.md#run-in-ci-from-vulyk).

### CI suite statuses

| Status | Meaning |
|--------|---------|
| Queued | Waiting for workflow start (Run in CI) |
| Open | Pipeline registered; no results yet |
| In progress | Workflow started / results streaming |
| Closed | Job finished (pass or fail) |
| Cancelled | Interrupted / cancelled close |

Suites show GitHub/GitLab source badges and pipeline links when metadata was provided.

## Reports and dashboards

Open **Reports** (`/reports`):

- Project dashboard KPIs (drill into cases, runs, or coverage)
- Pass/fail trends
- Requirement coverage (`/reports/coverage`)
- Flaky detection
- Deep-linkable run report `/reports/runs/{id}` with filters, grouping, expandable steps, filtered CSV/PDF export

Export requires **export reports**. Backspace returns to Reports when you are not typing in a field.
