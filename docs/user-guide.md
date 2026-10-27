# Vulyk user guide

Vulyk is a test management system: organize cases into nested folders, plan and execute runs, ingest CI results, link requirements, and review dashboards.

## Sign in

1. Open the UI (default `http://localhost:5173`).
2. On a fresh install, **Create the first admin** (bootstrap).
3. Later visits use **Sign in** with email/password.
4. Personal API tokens for CLI/MCP live under gear → **My settings** (or **Admin settings → Tokens** for admins managing others).

When auth is disabled (`VULYK_AUTH_REQUIRED=false`, local compose default), the UI runs in open mode without a login wall unless bootstrap is still available. The gear menu shows an **Open mode** badge.

## Demo sample data

On demo/dev installs (`VULYK_SEED_SAMPLE=1`, wizard default on), setup seeds two projects idempotently (skipped when the project key already exists; not seeded in prod unless you enable the flag). Every seeded row is marked `is_sample` and shows a **Sample** badge in lists and detail headers.

| Project | Key | Prefix | Contents |
|---------|-----|--------|----------|
| Kyiv Open-Air Concert 2026 | `KYIV_CONCERT_2026` | `KC` | Concert-domain suites, ~35 cases, requirements, dress rehearsal + ticketing smoke runs, plus **Sample nightly #N** automated suites (varied pass/fail/in-progress/cancelled; creator **Sample CI**) |
| Vulyk QA | `VULYK_QA` | `VQA` | Nested folders by product area, ~220 cases (base + `backend/seed/vulyk-qa.d/` fragments) with Playwright-oriented steps (folder tree, case list columns, key URLs, i18n/locales, header menus, executions, status badges, PDF export, multi-repo CI, password policy, Admin Updates, lists/custom fields, …), requirements per area, smoke + regression runs |

**Admin → Settings → Sample data** lists installed sample projects, **Remove sample data** (confirm, danger) deletes only `is_sample` rows (idempotent; never user-created projects), and **Re-seed sample data** restores the demos. Case keys on remove go through the retired-key path before the sample project is deleted.

Seed credentials (when `VULYK_SEED_DEMO=1` / admin env seed): admin email `admin@vulyk.local` with password from `VULYK_ADMIN_PASSWORD` in `.env` (forced change when install-generated); demo `user@vulyk.local` / `VULYK_DEMO_PASSWORD` (default `vulyk-demo-user-1`, TAE on sample projects). Password policy defaults to 16+ characters (see `VULYK_PASSWORD_MIN_LENGTH`). Data files live under `backend/seed/` (`kyiv-concert-2026.json`, `vulyk-qa.json`, plus fragments in `vulyk-qa.d/`; regenerate the base with `python3 backend/seed/generate_vulyk_qa.py`).

## Projects & cases

- Use the project switcher in the top bar.
- **Cases** shows a SquashTM-style **folder tree** on the left of the page (the only folder UI — not duplicated in the app sidebar). Nested folders + cases with priority bar and status icon; **All cases** is the tree root. Folder badges show the **recursive** case total (tooltip includes the direct count). Expanding a folder **lazy-loads** its direct cases (100 at a time with Load more). Selecting a folder filters the list (including subfolders by default; use **Direct only** to exclude descendants). Default order is numeric key suffix (`PREFIX-2` before `PREFIX-10`); click a column header for **server-side sort** (works with pagination / large projects). Use **Columns** to show/hide and reorder fields (drag headers or keyboard ←/→ in the menu). Title cannot be hidden. Extra columns include Automated, Last run status, Updated, Folder, Tags, and project custom fields. Layout is saved per user per project (and in the browser as a fallback); **Reset to default** restores Key / Title / Priority / Status. On narrow viewports the tree opens as a **Folders** drawer.
- **Create case** needs only a title — keys are allocated automatically as `PREFIX-N` (prefix from project name initials, e.g. *Kyiv Open-Air Concert 2026* → `KOC`). Admins may edit the prefix before any cases exist. Deleted numbers are never reused.
- Case detail URLs use the key (`/cases/PLTK-2194`). List and folder-tree links open the key URL. Old `/cases/<uuid>` bookmarks still work and **replace**-redirect to the key URL. Opening a deleted/retired key shows a clear **Case deleted** page. Click the key badge to copy it (toast + keyboard Enter/Space); optional **Copy link** copies the absolute URL. **Backspace** (when focus is not in an input/dialog) returns to the Cases list filtered to the case’s folder; the back control shows the shortcut hint. Breadcrumbs above the back link are **Project > Folder > … > KEY title**.
- Open a case to view Details, History / Results, and Executions. **Edit** opens an inline editor with a sticky Save/Cancel bar, an unsaved-changes indicator, and a leave warning when navigating away with dirty fields.
- Create / rename / delete / drag folders (or **Move to** on mobile). Deleting a non-empty folder asks whether to move cases to the parent or delete them (deleted cases retire their keys).
- **Import** brings cases from TestLink, TestRail, Xray, Zephyr, Squash, qTest, Qase, Kiwi, or CSV; imported keys come from the project counter and original IDs are stored as `external_key`.
- **Export as PDF** on the case detail page (and per-row / bulk on the Cases list) downloads a server-generated PDF: header metadata, steps table, linked requirements, and the latest mixed manual + automated results (default 5, configurable). Multiple selected cases download as a ZIP of PDFs (**at most 100** cases per bulk request). Labels follow your UI language (EN/UK/FR/ES/DE); Ukrainian and other Unicode text are supported. Requires the **export reports** permission (all project roles).
- **Custom fields** (when an admin has defined them): checkbox, dropdown / multi-select (from option lists), text, long text, number, date, URL appear on create/edit/detail, as optional list columns/filters, and in bulk edit. Admins manage definitions under **Admin → Lists** and **Admin → Custom fields**; see [admin-guide.md](admin-guide.md#lists-custom-fields--config-export).

### Keyboard shortcuts (case editor)

Available only while a test case is in **Edit** mode (the Ctrl/Cmd+S listener is unregistered when you leave the editor so the browser’s Save Page works elsewhere). **Backspace**-to-list is disabled while editing and never fires inside inputs/dialogs:

| Shortcut | Action |
|----------|--------|
| **Tab** in the **Expected** field of the **last** step (when Action or Expected has content) | Add a new empty step and focus its Action field. Shift+Tab and Tab on non-last steps keep normal focus order; an empty last step does not spawn another. |
| **Ctrl+Enter** (Windows/Linux) or **⌘Enter** (macOS) in a step field | Add the next step and focus its Action. |
| **Ctrl+S** (Windows/Linux) or **⌘S** (macOS) | Save the case from anywhere in the editor (including textareas). Suppresses the browser save dialog. Ignored while a save is in flight, on key repeat, with Shift/Alt, or when another modal/dialog is open. Shows validation errors, a **No changes** toast when nothing changed, or a saved confirmation. |

The Save button tooltip and sticky bar show the platform-appropriate shortcut. A hint under the last step (“Press Tab or Ctrl/⌘+Enter…”) appears while that step’s fields are focused.

## Plans & runs

- **Runs** creates an execution queue from a plan, suite, or all cases.
- Open a run to execute items, record step results, and attach notes.
- Result statuses are **Untested** / **Ready** (initial), **Running**, **Passed**, **Failed**, **Blocked**, **Skipped**, **Untestable**, and **Retest**. Run/CI lifecycle statuses (**Completed**, **In progress**, **Cancelled**, **Open** / **Closed**, …) use the same shared status chip. Every chip is **icon + translated label** (never colour alone, never raw `snake_case` enums) with colour-blind-safe Okabe–Ito colours that meet WCAG AA in System / Light / Dark / High contrast. Every status change is stored as an immutable history row (who, when, comment, elapsed, case version, attachments/defects).
- The run execution panel shows a **Result history** timeline for the selected case. The case page **History / Results** tab lists all results across runs (filter by run, status, date, user) with latest status and pass-rate summary. Case **edit/version** history remains on the Details tab. The **Executions** tab shows cross-iteration history with a status sparkline.
- On phones, use the bottom nav and large **Passed / Failed / Blocked / Skipped** controls; you can attach a camera photo to a result.
- Offline: recently viewed cases/runs stay readable; step results queue locally and sync when you are back online. See [PWA and mobile](guides/pwa-and-mobile.md).
- **Upload JUnit results** on the Runs page to ingest CI XML into a selected run. JUnit `skipped` → Skipped; JUnit `error` → Failed.
- **CI** configures GitHub Actions / GitLab triggers and dispatch history.

## Automation executions

- **Executions** organises CI work as **Campaigns → Iterations → Automated suites** (SquashTM-style tree, TestRail/Xray-like UI).
- Iteration **Dashboard**: status donut, pass-rate trend, flaky/slowest tests, editable schedule and cadence.
- **Automated suites** table: created on/by, duration, pass/fail counts, Details drawer, report/artifact links, delete/rerun by RBAC.
- Pipeline upload can auto-create the campaign/iteration/run — see [guides/executions.md](guides/executions.md), [guides/ci-integration.md](guides/ci-integration.md), and [guides/ingest-and-ci.md](guides/ingest-and-ci.md).

## Requirements & coverage

- **Requirements** tracks manual or ALM-synced requirements and links them to cases.
- **Reports** shows run summaries, pass/fail trends, requirement coverage, flaky detection, and CSV/PDF export. Click a summary card, trend row, or bar segment to open a deep-linkable run report (`/reports/runs/{id}`) with filters, grouping, expandable steps, and filtered CSV/PDF export. KPI cards drill into the case list, runs list, or requirement coverage detail. Backspace returns to Reports when you are not typing in a field.

## Language & settings

- **Language:** click the globe icon in the header. The menu lists **English**, **Українська**, **Français**, **Español**, and **Deutsch** (checkmark on the current locale). Choice is saved on your account when signed in, and in `localStorage` (`vulyk.locale`) for anonymous / open-mode sessions. Dates and numbers use locale-aware formatting (`Intl`).
- **User menu:** the gear icon opens profile actions — **My settings** (`/settings`: name, language, timezone, notifications, password, personal API tokens), **Appearance**, **Admin settings** (admins only), Help/API docs, and **Log out** (or **Sign in** when anonymous). Auth mode (**Open mode** / **Auth required**) appears as a badge inside this menu, not as top-bar text.
- Instance default locale/theme are set under gear → **Admin settings** (or at first-run setup with `--locale`).

### Admin → Updates

System admins can open **Admin settings → Updates** to:

1. See the **current** API version (`/health`) and the **latest** GitHub Release for this repo.
2. Click **Check for updates** (or rely on an optional periodic interval).
3. Read release notes when a newer version exists.
4. On Docker Compose with the **updater sidecar** enabled (`VULYK_UPDATER_TOKEN` + Compose profile `updates`), click **Update now**. That runs the same verified-backup → migrate → health path as `./scripts/install.sh upgrade --version`, with automatic rollback on failure. Progress and logs appear on the page.
5. After a successful upgrade, a **Roll back to vX.Y.Z** button appears for `VULYK_ROLLBACK_RETENTION_DAYS` (default 7). Confirm by typing `ROLLBACK` — post-upgrade data is lost. The page also lists **version backups** (last N installed versions) with download.
6. On Kubernetes or installs without the sidecar, **Update now** / one-click rollback are hidden and **manual** `install.sh upgrade` / `rollback` commands are shown instead.

Details: [guides/in-app-updates.md](guides/in-app-updates.md) and [guides/upgrade.md](guides/upgrade.md#data-safety-guarantees).

### Admin → Backups / Export

System admins can also:

- **Admin → Backups** — schedule or run **content** backups (ZIP of projects/cases/fields) to local volume, GitHub, GitLab, Nextcloud, or Drive — [guides/backup.md](guides/backup.md). This is separate from DB dumps used by `install.sh backup` / upgrades.
- **Admin → Export / import** — versioned JSON/YAML config bundles (fields, lists, settings; secrets redacted) — [admin-guide.md](admin-guide.md#config-export--import-admin--export--import).

## Appearance

Open **Appearance** from the gear user menu (or go to `/appearance`). It is not in the left nav.

- **Theme:** System (follows `prefers-color-scheme`, and high contrast when `prefers-contrast: more`), Light, Dark, High contrast (WCAG AAA), or Sepia / night warm (low blue).
- **Font size:** step buttons from 87% to 138%.
- **Density:** Comfortable or Compact spacing.
- **Reduced motion:** Follow system, always reduce, or allow motion (also honours `prefers-reduced-motion`).

Preferences are saved to your account when signed in and mirrored in the browser so the chosen theme applies before the app paints (no flash of the wrong theme). An admin can set the **instance default theme** under gear → **Admin settings**.

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

See [guides/ci-integration.md](guides/ci-integration.md) and [guides/ingest-and-ci.md](guides/ingest-and-ci.md).
