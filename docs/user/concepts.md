# Concepts

This page defines the building blocks you see across Cases, Requirements, Runs, Executions, and Reports.

## Projects

A **project** is the top-level workspace. Switch projects with the project switcher in the top bar. Each project has:

- A **key** (for example `VULYK_QA`) used in APIs and CI reporting
- A **case key prefix** (for example `VQA`) used to allocate human-readable case keys `PREFIX-N`
- Memberships with one of five roles: Admin, Test Architect, Test Automation Engineer, Developer, Viewer

Creating a project while signed in grants you project **Admin** membership. System admins see all projects.

## Folders (suites)

Cases live in a nested **folder tree** on the Cases page (SquashTM-style). The tree is the only folder UI — it is not duplicated in the app sidebar.

- **All cases** is the tree root.
- Folder badges show the **recursive** case total (tooltip includes the direct count).
- Expanding a folder **lazy-loads** its cases (100 at a time, with Load more).
- Selecting a folder filters the list (including subfolders by default; use **Direct only** to exclude descendants).
- Create, rename, delete, and drag folders (or **Move to** on mobile). Deleting a non-empty folder asks whether to move cases to the parent or delete them.

## Cases and keys

A **test case** has a title, optional description/preconditions, ordered steps (action + expected result), priority, status, tags, and optional custom fields.

- Creating a case needs only a **title**. Keys are allocated automatically as `PREFIX-N`.
- Deleted numbers are **never reused**. Opening a retired key shows a clear **Case deleted** page.
- Case detail URLs use the key (`/cases/PLTK-2194`). Old UUID bookmarks redirect to the key URL.
- Click the key badge to copy it; optional **Copy link** copies the absolute URL.

Each successful edit can produce a **version snapshot** (shown on the Details tab as edit/version history). Result history records the case version that was executed.

## Tags

Tags are free-form labels on cases. They appear as an optional list column and help you find related cases. Sample data may use tags such as `planned` for draft coverage gaps.

## Custom fields and option lists

System admins define **option lists** (Admin → Lists) and **custom fields** (Admin → Custom fields).

**Field types:** checkbox, dropdown / multi-select (from an option list), text, long text, number, date, URL.

**Entity:** fields can apply to test cases, requirements, or both.

**Scope:** all projects, or selected projects. Narrowing scope hides the field but keeps stored values.

Values appear on create/edit/detail, optional list columns and filters, bulk edit, importers (field mapping), and AI context when relevant. Project editors set values; only system admins manage definitions.

Built-in list columns also include **Automated** (driven by `external_key` or CI results), which is distinct from any custom “automation planned” checkbox an admin may define.

## Requirements

A **requirement** tracks something that should be covered by cases. You can create requirements manually, sync them from ALM integrations when configured, and **link** them to cases from the case detail page. Reports → coverage shows which requirements have linked cases and latest status.

## Runs

A **run** is an execution queue: a named set of cases (from a plan, a folder/suite, or all cases) that testers or automation will execute.

Manual result statuses include Untested / Ready, Running, Passed, Failed, Blocked, Skipped, Untestable, and Retest. Every status change is stored as an immutable history row (who, when, comment, elapsed, case version, attachments/defects).

CI lifecycle statuses include Queued, Open, In progress, Closed, and Cancelled.

## Executions

**Executions** organises automation in a Campaigns → Iterations → Automated suites tree:

| Concept | Meaning |
|---------|---------|
| Campaign | Nested folder for iterations |
| Iteration | Test cycle with schedule, cadence, and live status |
| Automated suite | One CI pipeline upload (a run linked to an iteration) |
| Execution | A single case result inside a suite |

Pipeline ingest can auto-create campaign, iteration, and suite when you report with campaign/iteration metadata.

## Reports

**Reports** aggregates run summaries, pass/fail trends, requirement coverage, and flaky detection, with CSV/PDF export. Deep-linkable run reports live at `/reports/runs/{id}`; coverage detail at `/reports/coverage`.
