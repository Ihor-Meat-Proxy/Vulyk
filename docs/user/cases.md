# Cases

Work with test cases from the **Cases** page (home route `/`) and case detail (`/cases/{KEY}`).

## Choose a project and folder

1. Select a project in the top-bar switcher.
2. Use the left **folder tree** to browse nested folders. On narrow viewports open the **Folders** drawer.
3. Click a folder to filter the list (includes subfolders by default). Enable **Direct only** to show only cases in that folder.
4. Expanding a folder lazy-loads its cases in pages of 100 — use **Load more** when needed.

## Create a case

1. Click **Create case**.
2. Enter a **title** (required). Optionally set description, preconditions, priority, status, tags, steps, and custom fields.
3. Save. Vulyk allocates the next key `PREFIX-N` for the project. Admins may edit the project prefix only before any cases exist.

Deleted keys are never reused.

## Steps and expected results

Each step has an **Action** and an **Expected** result. Steps are ordered and appear in PDF exports and run execution.

### Keyboard shortcuts (edit mode only)

Available only while the case is in **Edit** mode:

| Shortcut | Action |
|----------|--------|
| **Tab** in Expected on the **last** step (when Action or Expected has content) | Add a new empty step and focus its Action |
| **Ctrl+Enter** (Windows/Linux) or **⌘Enter** (macOS) in a step field | Add the next step and focus Action |
| **Ctrl+S** / **⌘S** | Save the case (suppresses the browser Save Page dialog) |

**Backspace**-to-list is disabled while editing and never fires inside inputs or dialogs. A sticky Save/Cancel bar shows unsaved changes; navigating away with dirty fields warns you.

## Priority, status, and tags

- Set **priority** and **status** on create or edit (and via bulk actions on the list when permitted).
- Add **tags** as free-form labels; show them with the Tags column.
- Custom fields configured by an admin appear on the form and optionally as columns/filters.

## Case detail tabs

Open a case from the list or folder tree (links use the key URL):

- **Details** — fields, steps, linked requirements, edit/version history snapshots
- **History / Results** — all results across runs (filter by run, status, date, user) with latest status and pass-rate summary
- **Executions** — cross-iteration automation history with a status sparkline

Breadcrumbs show **Project > Folder > … > KEY title**. Press **Backspace** (when focus is not in an input/dialog) to return to the Cases list filtered to the case’s folder.

## Columns, sort, and filters

### Columns

Use **Columns** to show/hide and reorder fields (drag headers or keyboard ←/→ in the menu). **Title** cannot be hidden. Extra columns include Automated, Last run status, Updated, Folder, Tags, and project custom fields marked for list display.

Layout is saved per user per project (with a browser fallback). **Reset to default** restores Key / Title / Priority / Status.

### Sort

Default order is numeric key suffix (`PREFIX-2` before `PREFIX-10`). Click a column header for **server-side sort** (works with pagination).

### Search (FTS) and filters

The search box uses PostgreSQL **full-text search** over the case search vector, with prefix matching. Queries of length **3 or more** also allow contains-style matching. Shorter queries rely on FTS and prefix matching only.

Combine search with folder selection, status/priority filters, and custom-field filters that an admin enabled for filtering.

## Organise folders

- Create, rename, delete, and drag folders (or use **Move to** on mobile).
- Deleting a non-empty folder asks whether to **move cases to the parent** or **delete them** (deleted cases retire their keys).

## Import and export

- **Import** (`/import`) brings cases from supported importers (and CSV). Imported keys come from the project counter; original IDs are stored as `external_key`. See [Import](import.md).
- **Export as PDF** on case detail (and per-row / bulk on the Cases list) downloads a server-generated PDF: header metadata, steps table, linked requirements, and the latest mixed manual + automated results (default 5). Multiple selected cases download as a ZIP of PDFs (**at most 100** cases per bulk request). Labels follow your UI language. Requires the **export reports** permission (available to all project roles that can view reports).

## Versioning

Saving edits can create version snapshots listed under **Edit history** on the Details tab (`v1`, `v2`, … with optional change summary and timestamp). Run result history can show which **case version** was executed. This is edit/snapshot history — not a separate branching VCS.

## Bulk actions

On the Cases list, select multiple cases (when your role allows) to bulk-set status, edit custom fields, export PDFs, or open **Generate automation** with the selection prefilled on the Generate page.
