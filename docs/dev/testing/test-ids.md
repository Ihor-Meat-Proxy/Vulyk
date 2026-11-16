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

- `npm run check:test-ids` (frontend) — static scan for duplicate string literals (per-file and cross-file for non-list ids), interactive JSX missing `data-testid` / `testIdProps`, suspicious literals in test attrs, and **catalog sync** with this doc (every `TestIds` value must appear below). Refresh with `npm run check:test-ids -- --write-catalog`.
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

<!-- BEGIN:test-ids-catalog -->

## Catalog (auto-generated)

Generated from `frontend/src/testing/testIds.ts`. Do not edit by hand —
run `npm run check:test-ids -- --write-catalog` (from `frontend/`).

| Test id | TestIds key |
| --- | --- |
| `admin-auth-off-banner` | `adminAuthOffBanner` |
| `admin-automation-branch-input` | `adminAutomationBranchInput` |
| `admin-automation-provider-select` | `adminAutomationProviderSelect` |
| `admin-automation-reindex-button` | `adminAutomationReindexButton` |
| `admin-automation-save-button` | `adminAutomationSaveButton` |
| `admin-automation-subpath-input` | `adminAutomationSubpathInput` |
| `admin-automation-test-button` | `adminAutomationTestButton` |
| `admin-automation-token-input` | `adminAutomationTokenInput` |
| `admin-automation-url-input` | `adminAutomationUrlInput` |
| `admin-backups-dest-base-url` | `adminBackupsDestBaseUrl` |
| `admin-backups-dest-branch` | `adminBackupsDestBranch` |
| `admin-backups-dest-delete` | `adminBackupsDestDelete` |
| `admin-backups-dest-folder-id` | `adminBackupsDestFolderId` |
| `admin-backups-dest-form` | `adminBackupsDestForm` |
| `admin-backups-dest-kind` | `adminBackupsDestKind` |
| `admin-backups-dest-name` | `adminBackupsDestName` |
| `admin-backups-dest-owner` | `adminBackupsDestOwner` |
| `admin-backups-dest-path` | `adminBackupsDestPath` |
| `admin-backups-dest-remote-path` | `adminBackupsDestRemotePath` |
| `admin-backups-dest-repo` | `adminBackupsDestRepo` |
| `admin-backups-dest-row` | `adminBackupsDestRow` |
| `admin-backups-dest-save` | `adminBackupsDestSave` |
| `admin-backups-dest-secret` | `adminBackupsDestSecret` |
| `admin-backups-dest-test` | `adminBackupsDestTest` |
| `admin-backups-dest-username` | `adminBackupsDestUsername` |
| `admin-backups-destinations` | `adminBackupsDestinations` |
| `admin-backups-download` | `adminBackupsDownload` |
| `admin-backups-history` | `adminBackupsHistory` |
| `admin-backups-manual` | `adminBackupsManual` |
| `admin-backups-pack-mode` | `adminBackupsPackMode` |
| `admin-backups-panel` | `adminBackupsPanel` |
| `admin-backups-policies` | `adminBackupsPolicies` |
| `admin-backups-policy-dest` | `adminBackupsPolicyDest` |
| `admin-backups-policy-project` | `adminBackupsPolicyProject` |
| `admin-backups-policy-retain` | `adminBackupsPolicyRetain` |
| `admin-backups-policy-save` | `adminBackupsPolicySave` |
| `admin-backups-policy-schedule` | `adminBackupsPolicySchedule` |
| `admin-backups-restore` | `adminBackupsRestore` |
| `admin-backups-restore-counts` | `adminBackupsRestoreCounts` |
| `admin-backups-restore-preview` | `adminBackupsRestorePreview` |
| `admin-backups-run-dest` | `adminBackupsRunDest` |
| `admin-backups-run-now` | `adminBackupsRunNow` |
| `admin-backups-run-project` | `adminBackupsRunProject` |
| `admin-backups-run-row` | `adminBackupsRunRow` |
| `admin-backups-run-status` | `adminBackupsRunStatus` |
| `admin-backups-test-msg` | `adminBackupsTestMsg` |
| `admin-config-export-button` | `adminConfigExportButton` |
| `admin-config-format-select` | `adminConfigFormatSelect` |
| `admin-config-import-button` | `adminConfigImportButton` |
| `admin-config-import-choose-button` | `adminConfigImportChooseButton` |
| `admin-config-import-drop-zone` | `adminConfigImportDropZone` |
| `admin-config-import-input` | `adminConfigImportInput` |
| `admin-config-include-data-checkbox` | `adminConfigIncludeDataCheckbox` |
| `admin-config-include-folders-checkbox` | `adminConfigIncludeFoldersCheckbox` |
| `admin-config-include-memberships-checkbox` | `adminConfigIncludeMembershipsCheckbox` |
| `admin-config-preview-button` | `adminConfigPreviewButton` |
| `admin-config-project-checkbox` | `adminConfigProjectCheckbox` |
| `admin-config-scope-select` | `adminConfigScopeSelect` |
| `admin-config-strategy-select` | `adminConfigStrategySelect` |
| `admin-custom-field-row` | `adminCustomFieldRow` |
| `admin-custom-fields-active-checkbox` | `adminCustomFieldsActiveCheckbox` |
| `admin-custom-fields-add-button` | `adminCustomFieldsAddButton` |
| `admin-custom-fields-cancel-button` | `adminCustomFieldsCancelButton` |
| `admin-custom-fields-default-value-input` | `adminCustomFieldsDefaultValueInput` |
| `admin-custom-fields-delete-button` | `adminCustomFieldsDeleteButton` |
| `admin-custom-fields-edit-button` | `adminCustomFieldsEditButton` |
| `admin-custom-fields-entity-select` | `adminCustomFieldsEntitySelect` |
| `admin-custom-fields-help-text-input` | `adminCustomFieldsHelpTextInput` |
| `admin-custom-fields-key-input` | `adminCustomFieldsKeyInput` |
| `admin-custom-fields-name-input` | `adminCustomFieldsNameInput` |
| `admin-custom-fields-option-list-select` | `adminCustomFieldsOptionListSelect` |
| `admin-custom-fields-project-checkbox` | `adminCustomFieldsProjectCheckbox` |
| `admin-custom-fields-required-checkbox` | `adminCustomFieldsRequiredCheckbox` |
| `admin-custom-fields-scope-select` | `adminCustomFieldsScopeSelect` |
| `admin-custom-fields-show-in-filters-checkbox` | `adminCustomFieldsShowInFiltersCheckbox` |
| `admin-custom-fields-show-in-list-checkbox` | `adminCustomFieldsShowInListCheckbox` |
| `admin-custom-fields-sort-order-input` | `adminCustomFieldsSortOrderInput` |
| `admin-custom-fields-type-select` | `adminCustomFieldsTypeSelect` |
| `admin-error` | `adminError` |
| `admin-integration-create-button` | `adminIntegrationCreateButton` |
| `admin-integration-kind-select` | `adminIntegrationKindSelect` |
| `admin-integration-list` | `adminIntegrationList` |
| `admin-integration-name-input` | `adminIntegrationNameInput` |
| `admin-integration-row` | `adminIntegrationRow` |
| `admin-integrations-ci-tokens-link` | `adminIntegrationsCiTokensLink` |
| `admin-list-item-row` | `adminListsItemRow` |
| `admin-list-row` | `adminListsRow` |
| `admin-lists-add-button` | `adminListsAddButton` |
| `admin-lists-description-input` | `adminListsDescriptionInput` |
| `admin-lists-item-add-button` | `adminListsItemAddButton` |
| `admin-lists-item-delete-button` | `adminListsItemDeleteButton` |
| `admin-lists-item-deprecate-button` | `adminListsItemDeprecateButton` |
| `admin-lists-item-label-input` | `adminListsItemLabelInput` |
| `admin-lists-item-move-down-button` | `adminListsItemMoveDownButton` |
| `admin-lists-item-move-up-button` | `adminListsItemMoveUpButton` |
| `admin-lists-key-input` | `adminListsKeyInput` |
| `admin-lists-name-input` | `adminListsNameInput` |
| `admin-lists-open-button` | `adminListsOpenButton` |
| `admin-lists-rename-button` | `adminListsRenameButton` |
| `admin-lists-rename-input` | `adminListsRenameInput` |
| `admin-llm-api-key-input` | `adminLlmApiKeyInput` |
| `admin-llm-base-url-input` | `adminLlmBaseUrlInput` |
| `admin-llm-default-checkbox` | `adminLlmDefaultCheckbox` |
| `admin-llm-delete-button` | `adminLlmDeleteButton` |
| `admin-llm-global-checkbox` | `adminLlmGlobalCheckbox` |
| `admin-llm-list` | `adminLlmList` |
| `admin-llm-model-input` | `adminLlmModelInput` |
| `admin-llm-name-input` | `adminLlmNameInput` |
| `admin-llm-provider-select` | `adminLlmProviderSelect` |
| `admin-llm-row` | `adminLlmRow` |
| `admin-llm-save-button` | `adminLlmSaveButton` |
| `admin-loading` | `adminLoading` |
| `admin-membership-row` | `adminMembershipRow` |
| `admin-page` | `adminPage` |
| `admin-project-create-button` | `adminProjectCreateButton` |
| `admin-project-description-input` | `adminProjectDescriptionInput` |
| `admin-project-key-input` | `adminProjectKeyInput` |
| `admin-project-list` | `adminProjectList` |
| `admin-project-name-input` | `adminProjectNameInput` |
| `admin-project-row` | `adminProjectRow` |
| `admin-role-save-button` | `adminRoleSaveButton` |
| `admin-role-select` | `adminRoleSelect` |
| `admin-role-user-select` | `adminRoleUserSelect` |
| `admin-rollback-dialog-input` | `adminRollbackDialogInput` |
| `admin-settings-base-url-input` | `adminSettingsBaseUrlInput` |
| `admin-settings-ci-allow-private-checkbox` | `adminSettingsCiAllowPrivateCheckbox` |
| `admin-settings-ci-allowlist-input` | `adminSettingsCiAllowlistInput` |
| `admin-settings-ci-private-warning` | `adminSettingsCiPrivateWarning` |
| `admin-settings-default-role-select` | `adminSettingsDefaultRoleSelect` |
| `admin-settings-instance-name-input` | `adminSettingsInstanceNameInput` |
| `admin-settings-locale-select` | `adminSettingsLocaleSelect` |
| `admin-settings-registration-select` | `adminSettingsRegistrationSelect` |
| `admin-settings-save-button` | `adminSettingsSaveButton` |
| `admin-settings-theme-select` | `adminSettingsThemeSelect` |
| `admin-settings-timezone-input` | `adminSettingsTimezoneInput` |
| `admin-sso-list` | `adminSsoList` |
| `admin-sso-row` | `adminSsoRow` |
| `admin-sso-toggle-button` | `adminSsoToggleButton` |
| `admin-tab-backups` | `adminTabBackups` |
| `admin-tab-config` | `adminTabConfig` |
| `admin-tab-custom-fields` | `adminTabCustomFields` |
| `admin-tab-integrations` | `adminTabIntegrations` |
| `admin-tab-lists` | `adminTabLists` |
| `admin-tab-llm` | `adminTabLlm` |
| `admin-tab-projects` | `adminTabProjects` |
| `admin-tab-roles` | `adminTabRoles` |
| `admin-tab-settings` | `adminTabSettings` |
| `admin-tab-sso` | `adminTabSso` |
| `admin-tab-tokens` | `adminTabTokens` |
| `admin-tab-updates` | `adminTabUpdates` |
| `admin-tab-users` | `adminTabUsers` |
| `admin-token-create-button` | `adminTokenCreateButton` |
| `admin-token-list` | `adminTokenList` |
| `admin-token-name-input` | `adminTokenNameInput` |
| `admin-token-revoke-button` | `adminTokenRevokeButton` |
| `admin-token-row` | `adminTokenRow` |
| `admin-token-secret` | `adminTokenSecret` |
| `admin-updates-apply-button` | `adminUpdatesApplyButton` |
| `admin-updates-backup-download` | `adminUpdatesBackupDownload` |
| `admin-updates-backup-list` | `adminUpdatesBackupList` |
| `admin-updates-backup-row` | `adminUpdatesBackupRow` |
| `admin-updates-check-button` | `adminUpdatesRetryButton` |
| `admin-updates-check-error` | `adminUpdatesCheckError` |
| `admin-updates-current-version` | `adminUpdatesCurrentVersion` |
| `admin-updates-interval-input` | `adminUpdatesIntervalInput` |
| `admin-updates-interval-save-button` | `adminUpdatesIntervalSaveButton` |
| `admin-updates-job-list` | `adminUpdatesJobList` |
| `admin-updates-job-log` | `adminUpdatesJobLog` |
| `admin-updates-job-row` | `adminUpdatesJobRow` |
| `admin-updates-job-status` | `adminUpdatesJobStatus` |
| `admin-updates-latest-version` | `adminUpdatesLatestVersion` |
| `admin-updates-manual-command` | `adminUpdatesManualCommand` |
| `admin-updates-notes` | `adminUpdatesNotes` |
| `admin-updates-panel` | `adminUpdatesPanel` |
| `admin-updates-progress` | `adminUpdatesProgress` |
| `admin-updates-release-link` | `adminUpdatesReleaseLink` |
| `admin-updates-rollback-button` | `adminUpdatesRollbackButton` |
| `admin-updates-rollback-command` | `adminUpdatesRollbackCommand` |
| `admin-updates-rollback-expiry` | `adminUpdatesRollbackExpiry` |
| `admin-updates-status` | `adminUpdatesStatus` |
| `admin-user-create-button` | `adminUserCreateButton` |
| `admin-user-display-name-input` | `adminUserDisplayNameInput` |
| `admin-user-email-input` | `adminUserEmailInput` |
| `admin-user-list` | `adminUserList` |
| `admin-user-password-input` | `adminUserPasswordInput` |
| `admin-user-row` | `adminUserRow` |
| `admin-user-system-admin-checkbox` | `adminUserSystemAdminCheckbox` |
| `api-docs-menu-link` | `headerGearDocs` |
| `app-shell` | `appShell` |
| `app-toast` | `appToast` |
| `app-toasts` | `appToasts` |
| `app-version` | `appVersion` |
| `appearance-density-select` | `appearanceDensitySelect` |
| `appearance-font-family-item` | `appearanceFontFamilyItem` |
| `appearance-font-family-select` | `appearanceFontFamilySelect` |
| `appearance-font-size-item` | `appearanceFontSizeItem` |
| `appearance-font-size-select` | `appearanceFontSizeSelect` |
| `appearance-page` | `appearancePage` |
| `appearance-reduced-motion-checkbox` | `appearanceReducedMotionCheckbox` |
| `appearance-save-button` | `appearanceSaveButton` |
| `appearance-theme-select` | `appearanceThemeSelect` |
| `automated-suite-delete-button` | `automatedSuiteDeleteButton` |
| `automated-suite-details-button` | `automatedSuiteDetailsButton` |
| `automated-suite-name-link` | `automatedSuiteNameLink` |
| `automated-suite-report-button` | `automatedSuiteReportButton` |
| `automated-suite-rerun-button` | `automatedSuiteRerunButton` |
| `automated-suite-row` | `automatedSuiteRow` |
| `automated-suite-run-in-ci-button` | `automatedSuiteRunInCiButton` |
| `automated-suites-apply-button` | `automatedSuitesApplyButton` |
| `automated-suites-created-by-input` | `automatedSuitesCreatedByInput` |
| `automated-suites-empty` | `automatedSuitesEmpty` |
| `automated-suites-error` | `automatedSuitesError` |
| `automated-suites-loading` | `automatedSuitesLoading` |
| `automated-suites-next-button` | `automatedSuitesNextButton` |
| `automated-suites-prev-button` | `automatedSuitesPrevButton` |
| `automated-suites-search-input` | `automatedSuitesSearchInput` |
| `automated-suites-sort-select` | `automatedSuitesSortSelect` |
| `automated-suites-status-filter` | `automatedSuitesStatusFilter` |
| `automated-suites-table` | `automatedSuitesTable` |
| `bottom-nav` | `bottomNav` |
| `bottom-nav-cases` | `bottomNavCases` |
| `bottom-nav-executions` | `bottomNavExecutions` |
| `bottom-nav-reports` | `bottomNavReports` |
| `bottom-nav-runs` | `bottomNavRuns` |
| `brand-logo` | `brandLogo` |
| `brand-logo-link-sidebar` | `brandLogoLinkSidebar` |
| `brand-logo-link-topbar` | `brandLogoLinkTopbar` |
| `campaign-dialog-cancel` | `campaignDialogCancel` |
| `campaign-dialog-description-input` | `campaignDialogDescriptionInput` |
| `campaign-dialog-error` | `campaignDialogError` |
| `campaign-dialog-name-input` | `campaignDialogNameInput` |
| `campaign-dialog-panel` | `campaignDialogPanel` |
| `campaign-dialog-submit` | `campaignDialogSubmit` |
| `case-detail-actions` | `caseDetailActions` |
| `case-detail-add-step-button` | `caseDetailAddStepButton` |
| `case-detail-automated` | `caseDetailAutomated` |
| `case-detail-back` | `caseDetailBackLink` |
| `case-detail-breadcrumb-current` | `caseDetailBreadcrumbCurrent` |
| `case-detail-breadcrumb-folder` | `caseDetailBreadcrumbFolder` |
| `case-detail-breadcrumb-project` | `caseDetailBreadcrumbProject` |
| `case-detail-breadcrumbs` | `caseDetailBreadcrumbs` |
| `case-detail-cancel-button` | `caseDetailCancelButton` |
| `case-detail-copy-link` | `caseDetailCopyLink` |
| `case-detail-custom-field-input` | `caseDetailCustomFieldInput` |
| `case-detail-custom-fields` | `caseDetailCustomFields` |
| `case-detail-delete-button` | `caseDetailDeleteButton` |
| `case-detail-deleted` | `caseDetailDeleted` |
| `case-detail-deleted-back` | `caseDetailDeletedBack` |
| `case-detail-description-input` | `caseDetailDescriptionInput` |
| `case-detail-edit-button` | `caseDetailEditButton` |
| `case-detail-editing` | `caseDetailEditing` |
| `case-detail-error` | `caseDetailError` |
| `case-detail-execution-link` | `caseDetailExecutionLink` |
| `case-detail-executions-next-button` | `caseDetailExecutionsNextButton` |
| `case-detail-executions-pager` | `caseDetailExecutionsPager` |
| `case-detail-executions-pass-rate` | `caseDetailExecutionsPassRate` |
| `case-detail-executions-prev-button` | `caseDetailExecutionsPrevButton` |
| `case-detail-export-pdf` | `caseDetailExportPdf` |
| `case-detail-folder-select` | `caseDetailFolderSelect` |
| `case-detail-history` | `caseDetailHistory` |
| `case-detail-key-badge` | `caseDetailKeyBadge` |
| `case-detail-link-requirement-button` | `caseDetailLinkRequirementButton` |
| `case-detail-link-requirement-select` | `caseDetailLinkRequirementSelect` |
| `case-detail-loading` | `caseDetailLoading` |
| `case-detail-not-found` | `caseDetailNotFound` |
| `case-detail-page` | `caseDetailPage` |
| `case-detail-panel` | `caseDetailPanel` |
| `case-detail-precondition-input` | `caseDetailPreconditionInput` |
| `case-detail-priority-select` | `caseDetailPrioritySelect` |
| `case-detail-results-apply-button` | `caseDetailResultsApplyButton` |
| `case-detail-results-next-button` | `caseDetailResultsNextButton` |
| `case-detail-results-prev-button` | `caseDetailResultsPrevButton` |
| `case-detail-results-run-filter` | `caseDetailResultsRunFilter` |
| `case-detail-results-status-filter` | `caseDetailResultsStatusFilter` |
| `case-detail-save-button` | `caseDetailSaveButton` |
| `case-detail-status-select` | `caseDetailStatusSelect` |
| `case-detail-step` | `caseDetailStep` |
| `case-detail-step-action` | `caseDetailStepAction` |
| `case-detail-step-data` | `caseDetailStepData` |
| `case-detail-step-expected` | `caseDetailStepExpected` |
| `case-detail-step-hint` | `caseDetailStepHint` |
| `case-detail-step-remove` | `caseDetailStepRemove` |
| `case-detail-steps` | `caseDetailSteps` |
| `case-detail-sticky-bar` | `caseDetailStickyBar` |
| `case-detail-sticky-unsaved` | `caseDetailStickyUnsaved` |
| `case-detail-tab-details` | `caseDetailTabDetails` |
| `case-detail-tab-executions` | `caseDetailTabExecutions` |
| `case-detail-tab-history` | `caseDetailTabHistory` |
| `case-detail-tags-input` | `caseDetailTagsInput` |
| `case-detail-title` | `caseDetailTitle` |
| `case-detail-title-input` | `caseDetailTitleInput` |
| `case-detail-title-text` | `caseDetailTitleText` |
| `case-list-automated-filter` | `caseListAutomatedFilter` |
| `case-list-bulk-bar` | `caseListBulkBar` |
| `case-list-bulk-custom-field-apply-button` | `caseListBulkCustomFieldApplyButton` |
| `case-list-bulk-custom-field-select` | `caseListBulkCustomFieldSelect` |
| `case-list-bulk-custom-field-value` | `caseListBulkCustomFieldValue` |
| `case-list-clear-selection-button` | `caseListClearSelectionButton` |
| `case-list-col-automated` | `caseListColAutomated` |
| `case-list-col-created-at` | `caseListColCreatedAt` |
| `case-list-col-custom-field` | `caseListColCustomField` |
| `case-list-col-folder` | `caseListColFolder` |
| `case-list-col-key` | `caseListColKey` |
| `case-list-col-last-run-status` | `caseListColLastRunStatus` |
| `case-list-col-priority` | `caseListColPriority` |
| `case-list-col-sort` | `caseListColSort` |
| `case-list-col-status` | `caseListColStatus` |
| `case-list-col-tags` | `caseListColTags` |
| `case-list-col-title` | `caseListColTitle` |
| `case-list-col-updated-at` | `caseListColUpdatedAt` |
| `case-list-column-item` | `caseListColumnItem` |
| `case-list-column-item-checkbox` | `caseListColumnItemCheckbox` |
| `case-list-column-item-move-left` | `caseListColumnItemMoveLeft` |
| `case-list-column-item-move-right` | `caseListColumnItemMoveRight` |
| `case-list-columns-button` | `caseListColumnsButton` |
| `case-list-columns-menu` | `caseListColumnsMenu` |
| `case-list-columns-reset-button` | `caseListColumnsResetButton` |
| `case-list-count` | `caseListCount` |
| `case-list-custom-field-filter` | `caseListCustomFieldFilter` |
| `case-list-deprecate-button` | `caseListDeprecateButton` |
| `case-list-direct-only-checkbox` | `caseListDirectOnlyCheckbox` |
| `case-list-empty` | `caseListEmpty` |
| `case-list-error` | `caseListError` |
| `case-list-export-pdf` | `caseListExportPdf` |
| `case-list-folders-backdrop` | `caseListFoldersBackdrop` |
| `case-list-folders-drawer` | `caseListFoldersDrawer` |
| `case-list-folders-drawer-close` | `caseListFoldersDrawerClose` |
| `case-list-folders-toggle` | `caseListFoldersToggle` |
| `case-list-generate-button` | `caseListGenerateButton` |
| `case-list-head` | `caseListHead` |
| `case-list-load-more-button` | `caseListLoadMoreButton` |
| `case-list-loading` | `caseListLoading` |
| `case-list-mark-ready-button` | `caseListMarkReadyButton` |
| `case-list-new-case-button` | `caseListNewCaseButton` |
| `case-list-new-title-input` | `caseListNewTitleInput` |
| `case-list-panel` | `caseListPanel` |
| `case-list-search-input` | `caseListSearchInput` |
| `case-list-status-filter` | `caseListStatusFilter` |
| `case-list-table` | `caseListTable` |
| `case-list-tag-input` | `caseListTagInput` |
| `case-list-virtual-scroll` | `caseListVirtualScroll` |
| `case-picker` | `casePicker` |
| `case-picker-all-folders` | `casePickerAllFolders` |
| `case-picker-chip` | `casePickerChip` |
| `case-picker-chips` | `casePickerChips` |
| `case-picker-empty` | `casePickerEmpty` |
| `case-picker-folder` | `casePickerFolder` |
| `case-picker-folders` | `casePickerFolders` |
| `case-picker-item` | `casePickerItem` |
| `case-picker-list` | `casePickerList` |
| `case-picker-search-input` | `casePickerSearchInput` |
| `case-picker-truncated-notice` | `casePickerTruncatedNotice` |
| `case-row` | `caseRow` |
| `case-row-automated` | `caseRowAutomated` |
| `case-row-checkbox` | `caseRowCheckbox` |
| `case-row-created-at` | `caseRowCreatedAt` |
| `case-row-custom-field` | `caseRowCustomField` |
| `case-row-export-pdf` | `caseRowExportPdf` |
| `case-row-folder` | `caseRowFolder` |
| `case-row-key` | `caseRowKey` |
| `case-row-last-run-status` | `caseRowLastRunStatus` |
| `case-row-priority` | `caseRowPriority` |
| `case-row-status` | `caseRowStatus` |
| `case-row-tags` | `caseRowTags` |
| `case-row-title` | `caseRowTitle` |
| `case-row-updated-at` | `caseRowUpdatedAt` |
| `change-password-confirm-input` | `changePasswordConfirmInput` |
| `change-password-current-input` | `changePasswordCurrentInput` |
| `change-password-disabled-hint` | `changePasswordDisabledHint` |
| `change-password-error` | `changePasswordError` |
| `change-password-new-input` | `changePasswordNewInput` |
| `change-password-panel` | `changePasswordPanel` |
| `change-password-policy-note` | `changePasswordPolicyNote` |
| `change-password-shell` | `changePasswordShell` |
| `change-password-sign-out-button` | `changePasswordSignOutButton` |
| `change-password-submit-button` | `changePasswordSubmitButton` |
| `change-password-subtitle` | `changePasswordSubtitle` |
| `chat-apply-button` | `chatApplyButton` |
| `chat-dismiss-button` | `chatDismissButton` |
| `chat-draft-case` | `chatDraftCase` |
| `chat-draft-requirement` | `chatDraftRequirement` |
| `chat-empty` | `chatEmpty` |
| `chat-error` | `chatError` |
| `chat-folder-select` | `chatFolderSelect` |
| `chat-input` | `chatInput` |
| `chat-loading` | `chatLoading` |
| `chat-message` | `chatMessage` |
| `chat-messages` | `chatMessages` |
| `chat-model-select` | `chatModelSelect` |
| `chat-new-folder-input` | `chatNewFolderInput` |
| `chat-page` | `chatPage` |
| `chat-project-select` | `chatProjectSelect` |
| `chat-proposal` | `chatProposal` |
| `chat-propose-button` | `chatProposeButton` |
| `chat-revise-button` | `chatReviseButton` |
| `chat-send-button` | `chatSendButton` |
| `confirm-dialog-cancel` | `confirmDialogCancel` |
| `confirm-dialog-confirm` | `confirmDialogConfirm` |
| `dialog-backdrop` | `dialogBackdrop` |
| `dialog-overlay` | `dialogOverlay` |
| `dialog-panel` | `dialogPanel` |
| `docs-article` | `docsArticle` |
| `docs-content-link` | `docsContentLink` |
| `docs-guide-link` | `docsGuideLink` |
| `docs-menu-link` | `docsMenuLink` |
| `docs-page` | `docsPage` |
| `docs-search` | `docsSearch` |
| `docs-sidebar` | `docsSidebar` |
| `docs-toc` | `docsToc` |
| `docs-toc-heading` | `docsTocHeading` |
| `docs-toc-page-` | `docsTocPagePrefix` |
| `execution-assignee-input` | `executionAssigneeInput` |
| `execution-attach-drop-zone` | `executionAttachDropZone` |
| `execution-attach-note-button` | `executionAttachNoteButton` |
| `execution-back-link` | `executionBackLink` |
| `execution-back-to-queue-button` | `executionBackToQueueButton` |
| `execution-block-button` | `executionBlockButton` |
| `execution-camera-button` | `executionCameraButton` |
| `execution-camera-input` | `executionCameraInput` |
| `execution-case-title-link` | `executionCaseTitleLink` |
| `execution-comment-input` | `executionCommentInput` |
| `execution-fail-button` | `executionFailButton` |
| `execution-live-badge` | `executionLiveBadge` |
| `execution-next-button` | `executionNextButton` |
| `execution-node` | `executionNode` |
| `execution-page` | `executionPage` |
| `execution-pass-button` | `executionPassButton` |
| `execution-prev-button` | `executionPrevButton` |
| `execution-result-open-link` | `executionResultOpenLink` |
| `execution-skip-button` | `executionSkipButton` |
| `execution-step-status-select` | `executionStepStatusSelect` |
| `execution-suite-link` | `executionSuiteLink` |
| `executions-empty` | `executionsEmpty` |
| `executions-error` | `executionsError` |
| `executions-loading` | `executionsLoading` |
| `executions-new-campaign-button` | `executionsNewCampaignButton` |
| `executions-page` | `executionsPage` |
| `executions-pipeline-link` | `executionsPipelineLink` |
| `executions-tree` | `executionsTree` |
| `folder-all-cases` | `folderAllCases` |
| `folder-case-item` | `folderCaseItem` |
| `folder-create-dialog-input` | `folderCreateDialogInput` |
| `folder-delete-dialog-cancel` | `folderDeleteDialogCancel` |
| `folder-delete-dialog-confirm` | `folderDeleteDialogConfirm` |
| `folder-delete-dialog-delete-cases-option` | `folderDeleteDialogDeleteCasesOption` |
| `folder-delete-dialog-delete-label` | `folderDeleteDialogDeleteLabel` |
| `folder-delete-dialog-move-label` | `folderDeleteDialogMoveLabel` |
| `folder-delete-dialog-move-option` | `folderDeleteDialogMoveOption` |
| `folder-delete-dialog-options` | `folderDeleteDialogOptions` |
| `folder-delete-dialog-panel` | `folderDeleteDialogPanel` |
| `folder-node` | `folderNode` |
| `folder-node-add-child` | `folderNodeAddChild` |
| `folder-node-button` | `folderNodeButton` |
| `folder-node-count` | `folderNodeCount` |
| `folder-node-delete` | `folderNodeDelete` |
| `folder-node-load-more` | `folderNodeLoadMore` |
| `folder-node-name` | `folderNodeName` |
| `folder-node-rename` | `folderNodeRename` |
| `folder-node-twistie` | `folderNodeTwistie` |
| `folder-rename-dialog-input` | `folderRenameDialogInput` |
| `folder-tree` | `folderTree` |
| `folder-tree-empty` | `folderTreeEmpty` |
| `folder-tree-loading` | `folderTreeLoading` |
| `folder-tree-new-button` | `folderTreeNewButton` |
| `folder-unfiled` | `folderUnfiled` |
| `folder-unfiled-button` | `folderUnfiledButton` |
| `folder-unfiled-twistie` | `folderUnfiledTwistie` |
| `generate-case-picker` | `generateCasePicker` |
| `generate-code-model-select` | `generateCodeModelSelect` |
| `generate-code-submit-button` | `generateCodeSubmitButton` |
| `generate-copy-button` | `generateCopyButton` |
| `generate-error` | `generateError` |
| `generate-framework-select` | `generateFrameworkSelect` |
| `generate-model-select` | `generateModelSelect` |
| `generate-output` | `generateOutput` |
| `generate-page` | `generatePage` |
| `generate-repo-select` | `generateRepoSelect` |
| `generate-requirements-input` | `generateRequirementsInput` |
| `generate-submit-button` | `generateSubmitButton` |
| `header-brand-logo` | `headerBrandLogo` |
| `header-gear-admin` | `headerGearAdmin` |
| `header-gear-appearance` | `headerGearAppearance` |
| `header-gear-auth-mode` | `headerGearAuthMode` |
| `header-gear-menu` | `headerGearMenu` |
| `header-gear-menu-panel` | `headerGearMenuPanel` |
| `header-gear-settings` | `headerGearSettings` |
| `header-gear-sign-in` | `headerGearSignIn` |
| `header-gear-sign-out` | `headerGearSignOut` |
| `header-language-item` | `headerLanguageItem` |
| `header-language-menu` | `headerLanguageMenu` |
| `header-language-menu-panel` | `headerLanguageMenuPanel` |
| `header-menu-toggle` | `headerMenuToggle` |
| `header-project-select` | `headerProjectSelect` |
| `import-error` | `importError` |
| `import-file-input` | `importFileInput` |
| `import-format-select` | `importFormatSelect` |
| `import-importer-select` | `importImporterSelect` |
| `import-mapping-input` | `importMappingInput` |
| `import-page` | `importPage` |
| `import-preview-button` | `importPreviewButton` |
| `import-submit-button` | `importSubmitButton` |
| `import-success` | `importSuccess` |
| `iteration-cadence-input` | `iterationCadenceInput` |
| `iteration-dashboard` | `iterationDashboard` |
| `iteration-dialog-cadence-input` | `iterationDialogCadenceInput` |
| `iteration-dialog-cancel` | `iterationDialogCancel` |
| `iteration-dialog-description-input` | `iterationDialogDescriptionInput` |
| `iteration-dialog-error` | `iterationDialogError` |
| `iteration-dialog-mode-select` | `iterationDialogModeSelect` |
| `iteration-dialog-name-input` | `iterationDialogNameInput` |
| `iteration-dialog-panel` | `iterationDialogPanel` |
| `iteration-dialog-submit` | `iterationDialogSubmit` |
| `iteration-display-id` | `iterationDisplayId` |
| `iteration-flaky-empty` | `iterationFlakyEmpty` |
| `iteration-flaky-item` | `iterationFlakyItem` |
| `iteration-save-schedule-button` | `iterationSaveScheduleButton` |
| `iteration-scheduled-end-input` | `iterationScheduledEndInput` |
| `iteration-scheduled-start-input` | `iterationScheduledStartInput` |
| `iteration-slow-empty` | `iterationSlowEmpty` |
| `iteration-slow-item` | `iterationSlowItem` |
| `login-brand-tag` | `loginBrandTag` |
| `login-display-name-input` | `loginDisplayNameInput` |
| `login-email-input` | `loginEmailInput` |
| `login-error` | `loginError` |
| `login-panel` | `loginPanel` |
| `login-password-input` | `loginPasswordInput` |
| `login-register-toggle` | `loginRegisterToggle` |
| `login-session-expired-notice` | `loginSessionExpiredNotice` |
| `login-sso-button` | `loginSsoButton` |
| `login-submit-button` | `loginSubmitButton` |
| `nav-drawer` | `navDrawer` |
| `nav-drawer-backdrop` | `navDrawerBackdrop` |
| `nav-drawer-close` | `navDrawerClose` |
| `nav-link-cases` | `navLinkCases` |
| `nav-link-chat` | `navLinkChat` |
| `nav-link-ci` | `navLinkCi` |
| `nav-link-executions` | `navLinkExecutions` |
| `nav-link-generate` | `navLinkGenerate` |
| `nav-link-import` | `navLinkImport` |
| `nav-link-reports` | `navLinkReports` |
| `nav-link-requirements` | `navLinkRequirements` |
| `nav-link-runs` | `navLinkRuns` |
| `not-found-back-link` | `notFoundBackLink` |
| `not-found-page` | `notFoundPage` |
| `not-found-title` | `notFoundTitle` |
| `password-requirements` | `passwordRequirements` |
| `primary-nav` | `primaryNav` |
| `prompt-dialog-cancel` | `promptDialogCancel` |
| `prompt-dialog-confirm` | `promptDialogConfirm` |
| `prompt-dialog-error` | `promptDialogError` |
| `prompt-dialog-input` | `promptDialogInput` |
| `pwa-dismiss-button` | `pwaDismissButton` |
| `pwa-install-button` | `pwaInstallButton` |
| `pwa-install-dismiss-button` | `pwaInstallDismissButton` |
| `pwa-sync-now-button` | `pwaSyncNowButton` |
| `pwa-toast-install` | `pwaToastInstall` |
| `pwa-toast-offline` | `pwaToastOffline` |
| `pwa-toast-offline-ready` | `pwaToastOfflineReady` |
| `pwa-toast-pending` | `pwaToastPending` |
| `pwa-toast-update` | `pwaToastUpdate` |
| `pwa-toasts` | `pwaToasts` |
| `pwa-update-button` | `pwaUpdateButton` |
| `reports-coverage-back` | `reportsCoverageBack` |
| `reports-coverage-breadcrumb-reports` | `reportsCoverageBreadcrumbReports` |
| `reports-coverage-breadcrumbs` | `reportsCoverageBreadcrumbs` |
| `reports-coverage-covered` | `reportsCoverageCovered` |
| `reports-coverage-drilldown` | `reportsCoverageDrilldown` |
| `reports-coverage-page` | `reportsCoveragePage` |
| `reports-coverage-preview` | `reportsCoveragePreview` |
| `reports-coverage-refresh` | `reportsCoverageRefresh` |
| `reports-coverage-req` | `reportsCoverageReq` |
| `reports-coverage-title` | `reportsCoverageTitle` |
| `reports-coverage-uncovered` | `reportsCoverageUncovered` |
| `reports-coverage-uncovered-empty` | `reportsCoverageUncoveredEmpty` |
| `reports-empty` | `reportsEmpty` |
| `reports-error` | `reportsError` |
| `reports-export-button` | `reportsExportButton` |
| `reports-export-coverage-button` | `reportsExportCoverageButton` |
| `reports-export-flaky-button` | `reportsExportFlakyButton` |
| `reports-export-pdf-button` | `reportsExportPdfButton` |
| `reports-export-run-button` | `reportsExportRunButton` |
| `reports-export-trends-button` | `reportsExportTrendsButton` |
| `reports-flaky` | `reportsFlaky` |
| `reports-flaky-item` | `reportsFlakyItem` |
| `reports-kpi-cases` | `reportsKpiCases` |
| `reports-kpi-coverage` | `reportsKpiCoverage` |
| `reports-kpi-requirements` | `reportsKpiRequirements` |
| `reports-kpi-runs` | `reportsKpiRuns` |
| `reports-kpis` | `reportsKpis` |
| `reports-loading` | `reportsLoading` |
| `reports-other-legend` | `reportsOtherLegend` |
| `reports-page` | `reportsPage` |
| `reports-refresh-button` | `reportsRefreshButton` |
| `reports-run-abandon-button` | `reportsRunAbandonButton` |
| `reports-run-back` | `reportsRunBack` |
| `reports-run-branch` | `reportsRunBranch` |
| `reports-run-breadcrumb-reports` | `reportsRunBreadcrumbReports` |
| `reports-run-breadcrumbs` | `reportsRunBreadcrumbs` |
| `reports-run-commit` | `reportsRunCommit` |
| `reports-run-comparison` | `reportsRunComparison` |
| `reports-run-duration` | `reportsRunDuration` |
| `reports-run-environment` | `reportsRunEnvironment` |
| `reports-run-error` | `reportsRunError` |
| `reports-run-export-csv` | `reportsRunExportCsv` |
| `reports-run-export-pdf` | `reportsRunExportPdf` |
| `reports-run-filter-assignee` | `reportsRunFilterAssignee` |
| `reports-run-filter-folder` | `reportsRunFilterFolder` |
| `reports-run-filter-priority` | `reportsRunFilterPriority` |
| `reports-run-filter-search` | `reportsRunFilterSearch` |
| `reports-run-filter-status` | `reportsRunFilterStatus` |
| `reports-run-filters` | `reportsRunFilters` |
| `reports-run-group-folder` | `reportsRunGroupFolder` |
| `reports-run-group-none` | `reportsRunGroupNone` |
| `reports-run-group-row` | `reportsRunGroupRow` |
| `reports-run-group-status` | `reportsRunGroupStatus` |
| `reports-run-groups` | `reportsRunGroups` |
| `reports-run-header` | `reportsRunHeader` |
| `reports-run-kpi-pass-rate` | `reportsRunKpiPassRate` |
| `reports-run-kpi-skipped` | `reportsRunKpiSkipped` |
| `reports-run-kpis` | `reportsRunKpis` |
| `reports-run-page` | `reportsRunPage` |
| `reports-run-page-next` | `reportsRunPageNext` |
| `reports-run-page-prev` | `reportsRunPagePrev` |
| `reports-run-pagination` | `reportsRunPagination` |
| `reports-run-pass-rate-hint` | `reportsRunPassRateHint` |
| `reports-run-pipeline` | `reportsRunPipeline` |
| `reports-run-ran-by` | `reportsRunRanBy` |
| `reports-run-refresh` | `reportsRunRefresh` |
| `reports-run-results` | `reportsRunResults` |
| `reports-run-results-table` | `reportsRunResultsTable` |
| `reports-run-row` | `reportsRunRow` |
| `reports-run-row-detail` | `reportsRunRowDetail` |
| `reports-run-row-expand` | `reportsRunRowExpand` |
| `reports-run-row-key` | `reportsRunRowKey` |
| `reports-run-sort` | `reportsRunSort` |
| `reports-run-source` | `reportsRunSource` |
| `reports-run-summary-item` | `reportsRunSummaryItem` |
| `reports-run-title` | `reportsRunTitle` |
| `reports-summaries` | `reportsSummaries` |
| `reports-summary-seg-failed` | `reportsSummarySegFailed` |
| `reports-summary-seg-other` | `reportsSummarySegOther` |
| `reports-summary-seg-passed` | `reportsSummarySegPassed` |
| `reports-title` | `reportsTitle` |
| `reports-trend-item` | `reportsTrendItem` |
| `reports-trend-seg-failed` | `reportsTrendSegFailed` |
| `reports-trend-seg-other` | `reportsTrendSegOther` |
| `reports-trend-seg-passed` | `reportsTrendSegPassed` |
| `reports-trends` | `reportsTrends` |
| `requirement-row` | `requirementRow` |
| `requirement-row-key` | `requirementRowKey` |
| `requirement-row-title` | `requirementRowTitle` |
| `requirements-add-button` | `requirementsAddButton` |
| `requirements-custom-fields-save-button` | `requirementsCustomFieldsSaveButton` |
| `requirements-empty` | `requirementsEmpty` |
| `requirements-error` | `requirementsError` |
| `requirements-list` | `requirementsList` |
| `requirements-loading` | `requirementsLoading` |
| `requirements-page` | `requirementsPage` |
| `requirements-title-input` | `requirementsTitleInput` |
| `run-detail-abandon-button` | `runDetailAbandonButton` |
| `run-detail-add-cases-button` | `runDetailAddCasesButton` |
| `run-detail-back-link` | `runDetailBackLink` |
| `run-detail-case-row` | `runDetailCaseRow` |
| `run-detail-cases` | `runDetailCases` |
| `run-detail-complete-button` | `runDetailCompleteButton` |
| `run-detail-error` | `runDetailError` |
| `run-detail-live-badge` | `runDetailLiveBadge` |
| `run-detail-panel` | `runDetailPanel` |
| `run-detail-progress` | `runDetailProgress` |
| `run-detail-start-button` | `runDetailStartButton` |
| `run-details-drawer` | `runDetailsDrawer` |
| `run-details-drawer-backdrop` | `runDetailsDrawerBackdrop` |
| `run-details-drawer-close` | `runDetailsDrawerClose` |
| `run-details-drawer-link` | `runDetailsDrawerLink` |
| `run-details-empty` | `runDetailsEmpty` |
| `run-details-error` | `runDetailsError` |
| `run-details-loading` | `runDetailsLoading` |
| `run-details-next-button` | `runDetailsNextButton` |
| `run-details-prev-button` | `runDetailsPrevButton` |
| `run-details-result-row` | `runDetailsResultRow` |
| `run-details-result-toggle` | `runDetailsResultToggle` |
| `run-display-id` | `runDisplayId` |
| `run-row` | `runRow` |
| `run-row-abandon` | `runRowAbandonButton` |
| `run-row-display-id` | `runRowDisplayId` |
| `run-row-link` | `runRowLink` |
| `run-row-name` | `runRowName` |
| `run-row-status` | `runRowStatus` |
| `runs-ci-base-url-input` | `runsCiBaseUrlInput` |
| `runs-ci-dispatch-button` | `runsCiDispatchButton` |
| `runs-ci-external-link` | `runsCiExternalLink` |
| `runs-ci-job-path-input` | `runsCiJobPathInput` |
| `runs-ci-json-input` | `runsCiJsonInput` |
| `runs-ci-name-input` | `runsCiNameInput` |
| `runs-ci-provider-select` | `runsCiProviderSelect` |
| `runs-ci-save-button` | `runsCiSaveButton` |
| `runs-ci-test-connection-button` | `runsCiTestConnectionButton` |
| `runs-ci-test-connection-result` | `runsCiTestConnectionResult` |
| `runs-ci-token-input` | `runsCiTokenInput` |
| `runs-ci-tokens-link` | `runsCiTokensLink` |
| `runs-ci-use-crumb-checkbox` | `runsCiUseCrumbCheckbox` |
| `runs-ci-username-input` | `runsCiUsernameInput` |
| `runs-create-button` | `runsCreateButton` |
| `runs-create-name-input` | `runsCreateNameInput` |
| `runs-empty` | `runsEmpty` |
| `runs-error` | `runsError` |
| `runs-list` | `runsList` |
| `runs-live-badge` | `runsLiveBadge` |
| `runs-load-more-button` | `runsLoadMoreButton` |
| `runs-loading` | `runsLoading` |
| `runs-page` | `runsPage` |
| `runs-plan-add-button` | `runsPlanAddButton` |
| `runs-plan-name-input` | `runsPlanNameInput` |
| `runs-plan-select` | `runsPlanSelect` |
| `runs-suite-select` | `runsSuiteSelect` |
| `runs-upload-choose-button` | `runsUploadChooseButton` |
| `runs-upload-disabled-hint` | `runsUploadDisabledHint` |
| `runs-upload-drop-zone` | `runsUploadDropZone` |
| `runs-upload-file-input` | `runsUploadFileInput` |
| `runs-upload-run-select` | `runsUploadRunSelect` |
| `settings-automation-actions-token-input` | `settingsAutomationActionsTokenInput` |
| `settings-automation-branch-input` | `settingsAutomationBranchInput` |
| `settings-automation-branch-select` | `settingsAutomationBranchSelect` |
| `settings-automation-cancel-button` | `settingsAutomationCancelButton` |
| `settings-automation-guide-link` | `settingsAutomationGuideLink` |
| `settings-automation-name-input` | `settingsAutomationNameInput` |
| `settings-automation-panel` | `settingsAutomationPanel` |
| `settings-automation-poll-interval-input` | `settingsAutomationPollIntervalInput` |
| `settings-automation-project-checkbox` | `settingsAutomationProjectCheckbox` |
| `settings-automation-provider-select` | `settingsAutomationProviderSelect` |
| `settings-automation-refresh-branches-button` | `settingsAutomationRefreshBranchesButton` |
| `settings-automation-reindex-button` | `settingsAutomationReindexButton` |
| `settings-automation-repo-row` | `settingsAutomationRepoRow` |
| `settings-automation-rotate-webhook-button` | `settingsAutomationRotateWebhookButton` |
| `settings-automation-save-button` | `settingsAutomationSaveButton` |
| `settings-automation-status` | `settingsAutomationStatus` |
| `settings-automation-subpath-input` | `settingsAutomationSubpathInput` |
| `settings-automation-test-button` | `settingsAutomationTestButton` |
| `settings-automation-token-input` | `settingsAutomationTokenInput` |
| `settings-automation-url-input` | `settingsAutomationUrlInput` |
| `settings-automation-webhook-secret` | `settingsAutomationWebhookSecret` |
| `settings-automation-webhook-url` | `settingsAutomationWebhookUrl` |
| `settings-automation-workflow-file-input` | `settingsAutomationWorkflowFileInput` |
| `settings-automation-workflow-ref-input` | `settingsAutomationWorkflowRefInput` |
| `settings-change-password-button` | `settingsChangePasswordButton` |
| `settings-ci-guide-link` | `settingsCiGuideLink` |
| `settings-ci-panel` | `settingsCiPanel` |
| `settings-ci-project-select` | `settingsCiProjectSelect` |
| `settings-ci-token-copy-button` | `settingsCiTokenCopyButton` |
| `settings-ci-token-copy-env-button` | `settingsCiTokenCopyEnvButton` |
| `settings-ci-token-copy-notice` | `settingsCiTokenCopyNotice` |
| `settings-ci-token-create-button` | `settingsCiTokenCreateButton` |
| `settings-ci-token-created` | `settingsCiTokenCreated` |
| `settings-ci-token-env-block` | `settingsCiTokenEnvBlock` |
| `settings-ci-token-last-used` | `settingsCiTokenLastUsed` |
| `settings-ci-token-name-input` | `settingsCiTokenNameInput` |
| `settings-ci-token-row` | `settingsCiTokenRow` |
| `settings-ci-token-secret` | `settingsCiTokenSecret` |
| `settings-ci-tokens-nav-link` | `settingsCiTokensNavLink` |
| `settings-confirm-password-input` | `settingsConfirmPasswordInput` |
| `settings-create-token-button` | `settingsCreateTokenButton` |
| `settings-current-password-input` | `settingsCurrentPasswordInput` |
| `settings-display-name-input` | `settingsDisplayNameInput` |
| `settings-email-input` | `settingsEmailInput` |
| `settings-error` | `settingsError` |
| `settings-locale-select` | `settingsLocaleSelect` |
| `settings-message` | `settingsMessage` |
| `settings-new-password-input` | `settingsNewPasswordInput` |
| `settings-notify-email-checkbox` | `settingsNotifyEmailCheckbox` |
| `settings-notify-in-app-checkbox` | `settingsNotifyInAppCheckbox` |
| `settings-notify-run-updates-checkbox` | `settingsNotifyRunUpdatesCheckbox` |
| `settings-page` | `settingsPage` |
| `settings-save-profile-button` | `settingsSaveProfileButton` |
| `settings-sign-in-link` | `settingsSignInLink` |
| `settings-timezone-input` | `settingsTimezoneInput` |
| `settings-token-list` | `settingsTokenList` |
| `settings-token-name-input` | `settingsTokenNameInput` |
| `settings-token-row` | `settingsTokenRow` |
| `settings-token-secret` | `settingsTokenSecret` |
| `setup-base-url-input` | `setupBaseUrlInput` |
| `setup-display-name-input` | `setupDisplayNameInput` |
| `setup-email-input` | `setupEmailInput` |
| `setup-error` | `setupError` |
| `setup-instance-name-input` | `setupInstanceNameInput` |
| `setup-locale-select` | `setupLocaleSelect` |
| `setup-panel` | `setupPanel` |
| `setup-password-input` | `setupPasswordInput` |
| `setup-registration-select` | `setupRegistrationSelect` |
| `setup-seed-sample-checkbox` | `setupSeedSampleCheckbox` |
| `setup-submit-button` | `setupSubmitButton` |
| `setup-success` | `setupSuccess` |
| `setup-timezone-input` | `setupTimezoneInput` |
| `setup-token-input` | `setupTokenInput` |
| `status-badge` | `statusBadge` |

<!-- END:test-ids-catalog -->
