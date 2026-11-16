# Playwright `data-testid` convention

Vulyk ships stable, unique `data-testid` attributes on interactive UI so e2e tests never guess CSS or i18n-dependent selectors.

## Naming

kebab-case: `<page>-<component>-<element>`

Examples:

| Element | Test id |
| --- | --- |
| New case button | `case-list-new-case-button` |
| Login email field | `login-email-input` |
| Header gear menu trigger | `header-gear-menu` |
| Header gear auth mode badge | `header-gear-auth-mode` |
| Gear → Docs (`/docs`) | `docs-menu-link` |
| Gear → API docs (`/api/docs`) | `api-docs-menu-link` |
| In-app docs page / TOC / article | `docs-page` / `docs-toc` / `docs-article` |
| Detailed guide link (per page) | `docs-guide-link-<suffix>` |
| Sidebar brand home link | `brand-logo-link-sidebar` |
| Mobile topbar brand home link | `brand-logo-link-topbar` |
| Admin Updates status / check error | `admin-updates-status` / `admin-updates-check-error` |
| Admin custom fields add | `admin-custom-fields-add-button` |
| Admin open-mode (auth off) warning | `admin-auth-off-banner` |

Every **button**, **input**, **select**, **checkbox**, **link**, **tab**, **menu item**, **dialog**, **toast**, **table header / sort control**, and **empty / loading / error** state gets one id. Ids must be **unique per page** (same string may appear once per mounted page).

## Lists, tables, trees

- Container: e.g. `case-list-table`, `folder-tree`, `runs-list`
- Each row / item uses a stable base id (`case-row`, `folder-node`, `run-row`, `execution-node`, `requirement-row`, `admin-project-row`, `chat-message`, `case-list-column-item`) plus:
  - `data-entity-id` — UUID
  - `data-entity-key` — human key when applicable (e.g. `KOC-12`, `VQA-001`)
  - `data-index` — 0-based position in the current view
  - `data-depth` — tree depth (folders / execution tree)
- Child controls are scoped under the base: `case-row-title`, `case-row-status`, `folder-node-count`, etc.

Prefer locating a row by entity attributes, then scoping children:

```ts
page.getByTestId('case-row').filter({ has: page.locator('[data-entity-key="VQA-001"]') })
// or
page.locator('[data-testid="case-row"][data-entity-key="VQA-001"]')
  .getByTestId('case-row-title')
```

## Typed constants

Source of truth: [`frontend/src/testing/testIds.ts`](../../../frontend/src/testing/testIds.ts)

- Components import `TestIds` and spread helpers from `frontend/src/testing` (`testIdProps`, `listItemProps`).
- E2e imports the same module: `import { TestIds } from '../../frontend/src/testing/testIds'`.

## Stripping (optional)

Ids are present in **all** builds by default (cheap; recommended by Playwright). To omit them:

```bash
VITE_STRIP_TEST_IDS=true npm run build
```

When set, `testIdProps` / `listItemProps` return empty objects so `data-testid`, `data-entity-id`, `data-entity-key`, `data-index`, and `data-depth` are never written to the DOM.

CI and Playwright preview builds **must not** set this flag.

## Security

Shipping stable `data-testid` values in production is **safe** when they stay non-sensitive:

- Test ids are **static labels** (`login-email-input`, `case-row`) — they name the control, not the user. Playwright recommends leaving them in production builds.
- List attrs may repeat **entity UUIDs / keys already exposed** by the API and URLs (`data-entity-id`, `data-entity-key`) plus positional `data-index` / `data-depth`.
- They must **never** contain PII or secrets: no emails, display names, passwords, session/JWT/API token secrets, or raw token prefixes beyond what the product already shows as public metadata.

Helpers call `assertSafeTestAttrs` (see `frontend/src/testing/security.ts`) and throw if a value looks suspicious (`@`, JWT-shaped strings, high-entropy blobs, non-kebab test ids). `listItemProps` only emits `data-entity-key` when the value matches a short API/URL key pattern — freeform display names (e.g. run titles with spaces) are omitted so a bad call site cannot blank the page.

If an environment still prefers zero test markup, build with `VITE_STRIP_TEST_IDS=true` so the helpers emit nothing.

## Enforcement

- `npm run check:test-ids` (frontend) — static scan for duplicate string literals, interactive JSX missing `data-testid` / `testIdProps`, and suspicious literals in test attrs.
- `npm run test` — unit tests for `security.ts` (PII/secret patterns) and strip-flag behaviour.
- `e2e/tests/test-id-audit.spec.ts` — visits main pages; fails on duplicate ids, unlabeled interactive controls, or DOM `data-*` values that look like emails/secrets.

## Adding a control

1. Add a constant to `TestIds` (or reuse a list-item child pattern).
2. Spread `{...testIdProps(TestIds.…)}` (or `listItemProps`) on the element.
3. Use `page.getByTestId(TestIds.…)` in Playwright / page objects.

## UX polish (F13–F22)

Additional ids used by `e2e/tests/ux-polish-f13-f22.spec.ts`:

| Element | Test id |
| --- | --- |
| App 404 page | `not-found-page` / `not-found-title` / `not-found-back-link` |
| Case executions pass rate / pager | `case-detail-executions-pass-rate` / `case-detail-executions-pager` |
| Case not-found panel | `case-detail-not-found` |
| Forced password policy note / disabled hint | `change-password-policy-note` / `change-password-disabled-hint` |
| Runs upload disabled hint | `runs-upload-disabled-hint` |
| Runs upload drop zone / choose button | `runs-upload-drop-zone` / `runs-upload-choose-button` |
| Execution attach drop zone | `execution-attach-drop-zone` |
| Admin config import drop zone / choose | `admin-config-import-drop-zone` / `admin-config-import-choose-button` |
| Coverage uncovered empty | `reports-coverage-uncovered-empty` |
4. Reference the id in seeded Vulyk QA steps when the case is UI-driven.

## Recent additions (case list density)

- Case list virtual scrollport: `case-list-virtual-scroll` (sticky header lives inside)

## Recent additions (F8–F12)

- Case picker: `case-picker-chips`, `case-picker-folders`, `case-picker-list`
- CI tokens: `settings-ci-token-env-block`, `settings-ci-token-copy-button`,
  `settings-ci-token-copy-env-button`, `settings-ci-token-copy-notice`,
  `settings-ci-token-created`, `settings-ci-token-last-used`,
  `runs-ci-tokens-link`, `admin-integrations-ci-tokens-link`
- Run report: `reports-run-kpi-skipped`, `reports-run-kpi-pass-rate`,
  `reports-run-pass-rate-hint`, `reports-other-legend`

## Stale run auto-cancel / admin abandon

| Element | Test id |
| --- | --- |
| Run detail abandon button | `run-detail-abandon-button` |
| Runs list row abandon button | `run-row-abandon` (list child) |
| Run report abandon button | `reports-run-abandon-button` |

## Readable run / iteration display ids

| Element | Test id |
| --- | --- |
| Run detail display id chip | `run-display-id` |
| Runs list row display id | `run-row-display-id` |
| Iteration dashboard display id | `iteration-display-id` |

## A11y pass (imp #8)

- Case picker: labelled checkboxes/chips/folders (`aria-pressed`, `aria-live`)
- CI token secret/env/copy notice: `role` + `aria-live` / `aria-label`
- Admin updates check error: `role="alert"` + `aria-live="assertive"`
- Live/in-progress status badges: icon + label + `role="status"`

## In-app dialogs

| Element | Test id |
| --- | --- |
| Dialog overlay / backdrop / panel | `dialog-overlay` / `dialog-backdrop` / `dialog-panel` |
| Confirm confirm / cancel | `confirm-dialog-confirm` / `confirm-dialog-cancel` |
| Prompt input / confirm / cancel / error | `prompt-dialog-input` / `prompt-dialog-confirm` / `prompt-dialog-cancel` / `prompt-dialog-error` |
| Campaign create dialog | `campaign-dialog-panel`, `campaign-dialog-name-input`, `campaign-dialog-description-input`, `campaign-dialog-submit`, `campaign-dialog-cancel`, `campaign-dialog-error` |
| Iteration create dialog | `iteration-dialog-panel`, `iteration-dialog-name-input`, `iteration-dialog-description-input`, `iteration-dialog-cadence-input`, `iteration-dialog-mode-select`, `iteration-dialog-submit`, `iteration-dialog-cancel`, `iteration-dialog-error` |
| Folder create / rename inputs | `folder-create-dialog-input` / `folder-rename-dialog-input` |
| Folder delete dialog | `folder-delete-dialog-panel`, `folder-delete-dialog-options`, `folder-delete-dialog-move-option`, `folder-delete-dialog-move-label`, `folder-delete-dialog-delete-cases-option`, `folder-delete-dialog-delete-label`, `folder-delete-dialog-confirm`, `folder-delete-dialog-cancel` |
| Admin rollback typed confirm | `admin-rollback-dialog-input` |
