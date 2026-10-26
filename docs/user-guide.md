# Vulyk user guide

Vulyk is a test management system: organize cases into nested folders, plan and execute runs, ingest CI results, link requirements, and review dashboards.

## Sign in

1. Open the UI (default `http://localhost:5173`).
2. On a fresh install, **Create the first admin** (bootstrap).
3. Later visits use **Sign in** with email/password.
4. Personal API tokens for CLI/MCP live under **Admin → Tokens**.

When auth is disabled (`VULYK_AUTH_REQUIRED=false`, local compose default), the UI runs in open mode without a login wall unless bootstrap is still available.

## Projects & cases

- Use the project switcher in the top bar.
- **Cases** shows a SquashTM-style **folder tree** on the left (nested folders + cases with priority bar and status icon) and the filtered list on the right. Selecting a folder filters the list (including subfolders).
- **Create case** needs only a title — keys are allocated automatically as `PREFIX-N` (prefix from project name initials, e.g. *Kyiv Open-Air Concert 2026* → `KOC`). Admins may edit the prefix before any cases exist. Deleted numbers are never reused.
- Create / rename / delete / drag folders (or **Move to** on mobile). Deleting a non-empty folder asks whether to move cases to the parent or delete them (deleted cases retire their keys).
- **Import** brings cases from TestLink, TestRail, Xray, Zephyr, Squash, qTest, Qase, Kiwi, or CSV; imported keys come from the project counter and original IDs are stored as `external_key`.

## Plans & runs

- **Runs** creates an execution queue from a plan, suite, or all cases.
- Open a run to execute items, record step results, and attach notes.
- Result statuses are **Untested** (initial), **Passed**, **Failed**, **Blocked**, **Skipped**, and **Retest**. Every status change is stored as an immutable history row (who, when, comment, elapsed, case version, attachments/defects).
- The run execution panel shows a **Result history** timeline for the selected case. The case page **History / Results** tab lists all results across runs (filter by run, status, date, user) with latest status and pass-rate summary. Case **edit/version** history remains on the Details tab.
- On phones, use the bottom nav and large **Passed / Failed / Blocked / Skipped** controls; you can attach a camera photo to a result.
- Offline: recently viewed cases/runs stay readable; step results queue locally and sync when you are back online. See [PWA and mobile](guides/pwa-and-mobile.md).
- **Upload JUnit results** on the Runs page to ingest CI XML into a selected run. JUnit `skipped` → Skipped; JUnit `error` → Failed.
- **CI** configures GitHub Actions / GitLab triggers and dispatch history.

## Requirements & coverage

- **Requirements** tracks manual or ALM-synced requirements and links them to cases.
- **Reports** shows run summaries, pass/fail trends, requirement coverage, flaky detection, and CSV/PDF export.

## Appearance

Open **Appearance** from the top bar (or go to `/appearance`).

- **Theme:** System (follows `prefers-color-scheme`, and high contrast when `prefers-contrast: more`), Light, Dark, High contrast (WCAG AAA), or Sepia / night warm (low blue).
- **Font size:** step buttons from 87% to 138%.
- **Density:** Comfortable or Compact spacing.
- **Reduced motion:** Follow system, always reduce, or allow motion (also honours `prefers-reduced-motion`).

Preferences are saved to your account when signed in and mirrored in the browser so the chosen theme applies before the app paints (no flash of the wrong theme). An admin can set the **instance default theme** under **Admin → Settings**.

Result statuses (**Passed**, **Failed**, **Blocked**, **Skipped**, **Retest**, **Untested**) use a colour-blind-safe Okabe–Ito palette with an **icon and label** everywhere (badges, run execution, charts). Charts also use hatch patterns plus a text legend.

## LLM assists

- **Generate** drafts cases or automation code (provider configured by an admin). Pick the model, cases (search/folder/multi-select), and optionally an automation Git repo for context.
- **Chat** asks questions over the project’s cases and can **propose** new cases or requirements. Review the preview card (project/folder + drafts), then click **Create**. Viewer and Developer can chat but cannot create.

## CLI reporter

```bash
cargo run -p vulyk-cli -- report \
  --file report.xml --format junit \
  --create-run --project-key DEMO
```

See [guides/ingest-and-ci.md](guides/ingest-and-ci.md).
