# Automation & CI reporting — comparison with SquashTM, TestRail, and Xray

This note summarizes what Vulyk borrows from established test-management products when designing **multi-repo automation links** and the **CI reporting API**, and why. It is a design rationale, not a feature-parity checklist.

Related: [automation-integration-design.md](./automation-integration-design.md), [guides/ci-integration.md](./guides/ci-integration.md).

## Sources (official docs)

| Product | Topics we studied | Links |
|---------|-------------------|-------|
| **SquashTM / AUTOM / DEVOPS** | Orchestrator + Result Publisher; CI import of automated suites into iterations; automated test reference mapping; partial import responses | [CI/CD integration](https://tm-en.doc.squashtest.com/v16/howto/ci-cd/index.html), [Import results from pipelines](https://tm-en.doc.squashtest.com/latest/user-guide/manage-automated-tests/devops/importResultsFromPipeline.html), [Squash DEVOPS overview](https://autom-devops-en.doc.squashtest.com/2022-09/devops/devops.html), [Pilot from Squash TM](https://autom-devops-en.doc.squashtest.com/2024-07/autom/pilotFromSquash.html) |
| **TestRail** | `add_run` / `add_results_for_cases` / `close_run`; **trcli** JUnit upload; `automation_id`; JUnit `test_id` property | [Runs API](https://support.testrail.com/hc/en-us/articles/7077874763156-Runs), [Importing results](https://support.testrail.com/hc/en-us/articles/15758390538260-Importing-test-results), [trcli commands](https://support.testrail.com/hc/en-us/articles/12908518569364-CLI-commands-reference), [Code-first / automation_id](https://support.testrail.com/hc/en-us/articles/12609674354068-Code-first-workflow), [JUnit mapping](https://support.testrail.com/hc/en-us/articles/12989737200276-JUnit-to-TestRail-mapping) |
| **Xray** (Cloud / Server) | REST import of JUnit / Xray JSON; Test Execution entity; `test_key` / `test_id` in JUnit properties; GitHub/GitLab CI recipes | [Import execution results (Cloud REST v2)](https://docs.getxray.app/space/XRAYCLOUD/44577176/Import%20Execution%20Results%20-%20REST%20v2), [JUnit XML reports](https://getxraydocs.atlassian.net/wiki/spaces/XRAYCLOUD/pages/44564745/Taking+advantage+of+JUnit+XML+reports), [Xray JSON format](https://getxraydocs.atlassian.net/wiki/spaces/XRAYCLOUD/pages/44565311), [Server REST import](https://getxraydocs.atlassian.net/wiki/spaces/XRAY/pages/301667997) |

## What we borrow

### 1. Explicit run lifecycle (TestRail)

TestRail’s clear **create → add results → close** flow (`add_run`, `add_results_for_cases`, `close_run`) maps cleanly to CI jobs that open a run at pipeline start, stream results, and close on success *or* failure.

**Vulyk:** CI runs use statuses `open` → `in_progress` → `closed`, with dedicated endpoints plus auto-transitions (first result → `in_progress`; finish/close → `closed`). We keep friendlier aliases (`running` ↔ `in_progress`, `completed` ↔ `closed`) so existing UI and ingest callers keep working.

### 2. Case-key / automation-id mapping from JUnit (TestRail + Xray)

Both products avoid fragile classname-only matching when teams already maintain cases in the TM tool:

- TestRail: JUnit `<property name="test_id" value="C123"/>` (or comma-separated IDs); code-first `automation_id` = `classname.name`.
- Xray: `<property name="test_key" value="CALC-2"/>` or `test_id`; otherwise Generic Test Definition from `classname` + `name`.

**Vulyk:** Prefer an explicit Vulyk case key (`KOC-12`, `DEMO-3`) from:

1. JUnit properties: `test_key`, `test_id`, `case_key`, `vulyk_key`, `vulyk:key`
2. Title / name annotations: `@vulyk:KOC-12`, `@key=KOC-12`, `[KOC-12]`
3. Free-form key pattern in the title
4. Optional `external_key` / automation id on the case (TestRail-style fallback)

We do **not** auto-create cases on unknown keys by default (friendlier than TestRail code-first duplication; closer to SquashTM’s “must match a reference” model). Unknown keys are **reported**, never silently dropped.

### 3. Partial success & unknown references (SquashTM)

SquashTM’s `POST /import/results/{iteration_id}` returns **207** with per-test errors when a reference is missing, instead of aborting the whole suite.

**Vulyk:** Batch and JUnit ingest always return `unmatched_case_keys` (and per-item errors where useful). Matched results are persisted; unknown keys are listed so CI logs stay actionable.

### 4. Automated suite under an iteration / campaign (SquashTM)

SquashTM creates an **automated suite** on an iteration when CI publishes results. Vulyk already models Campaign → Iteration → Automated suite (`test_runs`). The CI reporting API continues to attach suites to iterations (auto-create optional) and surfaces pipeline metadata on the suite.

### 5. Test Execution as a first-class CI artifact (Xray)

Xray’s Test Execution issue holds import metadata (revision, environments, plan link) and is the UI unit for a CI upload.

**Vulyk:** A CI `test_run` stores provider, repo, branch, commit, pipeline/job URL, suite name, and environment in structured fields / `meta`, with a **source badge** and pipeline link in Executions — without depending on Jira issues.

### 6. Project-scoped credentials for CI (all three, simplified)

SquashTM uses Bearer tokens against the results API; TestRail uses user/API key; Xray uses client credentials or personal tokens scoped to Jira.

**Vulyk:** **Project-scoped CI API tokens** (create/revoke in Settings, SHA-256 hashed at rest, shown once). Narrower than personal tokens for MCP/admin: a pipeline token only uploads results for its project. Admins and **Test Automation Engineers** can manage them (`manage_api_tokens`).

### 7. Ready-made CI adapters (TestRail trcli + Xray Actions)

**Vulyk:** Composite GitHub Action (`.github/actions/vulyk-report` + existing `action/`), GitLab CI template, and `scripts/vulyk-upload.sh` that open → upload → close (including `if: always()` / `when: always` on failure). Playwright examples put case keys in titles or annotations.

### 8. Multiple Git repositories per project (Squash AUTOM practice)

Squash AUTOM notes that an iteration may contain automated tests from **multiple Git repositories**. Vulyk promotes automation repos from a single per-project row to **app-level repositories** with a many-to-many link to projects (several repos per project; one repo shared across projects). Generate and other consumers show a **repo picker** when more than one is linked.

## What we deliberately simplify (friendlier UX)

| Area | Industry default | Vulyk choice |
|------|------------------|--------------|
| Orchestrator | Squash Orchestrator / agents | Optional later; CI pushes results (like Squash pipeline import & Xray import) |
| Auto-create cases | TestRail trcli code-first | Off by default; unknown keys reported |
| Jira issue types | Xray Test / Test Execution | Native runs & cases; no Jira dependency |
| Close semantics | TestRail close is irreversible archive | Soft close: no further writes; reopen only via explicit admin/API if needed |
| Mapping IDs | `C123` / Jira keys | Stable project keys `PREFIX-N` already used in Vulyk |
| Token UX | Long-lived user passwords / shared keys | Project CI tokens + masked display; personal tokens remain for MCP |

## Mapping cheat sheet

| Concern | SquashTM | TestRail | Xray | Vulyk |
|---------|----------|----------|------|-------|
| Open run | Suite created on import | `add_run` | Test Execution create/import | `POST .../ci/runs` → `open` |
| Post results | Import JSON by `reference` | `add_results_for_cases` | JUnit / Xray JSON import | `POST .../results` or `/ingest` |
| Close | Suite status calculated | `close_run` | Finish dates on execution | `POST .../close` (+ auto) |
| JUnit key | Automated test reference (parser) | `test_id` / `automation_id` | `test_key` / `test_id` | `test_key` / title / `@vulyk:` |
| Unknown key | 207 + error list | Create or skip (mode) | Error if key forced | Report in `unmatched_case_keys` |
| CI helpers | Orchestrator / Jenkins / scripts | **trcli** | Official Actions / templates | Action + GitLab template + `vulyk-upload.sh` |
