# Vulyk code review — 2026-10-26

> Historical snapshot. Doc index: [../README.md](../README.md). Companion audits:
> [../security/audit-2026-10.md](../security/audit-2026-10.md),
> [../quality/code-quality-2026-10.md](../quality/code-quality-2026-10.md).
> Several findings (jobs reclaim, PDF cap, React keys, etc.) landed in follow-up
> PRs after this review — prefer current code over unfixed wording below.

Senior review of the monorepo (`backend`, `frontend`, `e2e`, `mcp`, `cli`,
`docs`, migrations). Scope: correctness, edge cases, races, migration /
upgrade safety, OpenAPI vs handlers, error handling, large-project performance
(~20k cases), accessibility, i18n (EN/UK/FR/ES/DE), docs vs behaviour, and test
gaps. **Out of scope:** security deep-dive and pure refactors (separate agents).

Each finding is tagged **bug** / **risk** / **improvement**, with `file:line`.
Confirmed bugs fixed in follow-up PRs are marked **Fixed** with the PR link.

---

## Executive summary

The foundation is solid: forward-only SQLx migrations, `SKIP LOCKED` job claims,
transactional case-key allocation, paginated case list with virtualization,
locale parity tests, and broad OpenAPI coverage. The highest-impact issues found
are (1) run-item `executed_at` clobber (#42 merged), (2) jobs stuck `running`
after a crash, (3) case-sensitive ingest key match, (4) unbounded bulk PDF ZIP,
plus follow-ups: OpenAPI version drift, binary requirement coverage %,
cross-project ID acceptance, and frontend navigation/i18n gaps (see §Additional
findings). At 20k cases the list query’s per-row `last_run_status` subquery and
tag `LEFT JOIN` + `GROUP BY` are the main scale risks.

---

## P0 / P1 — confirmed bugs

### 1. **bug** — `PUT …/run_items/{id}` resets `executed_at` on every update

`backend/src/routes/runs.rs:612-631`

Updating only `assignee` / `comment` (no `status` in the body) still computes
`executed_at = NOW()` whenever the current status is not `Untested`. That
rewrites the original execution timestamp and skews history, progress timelines,
and “last executed” UX.

**Fix:** Preserve `existing.executed_at` when `body.status` is omitted; only
clear/set it when status is explicitly provided.

**Fixed (merged):** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/42

### 2. **bug** — Stale `running` jobs never reclaimed

`backend/src/services/jobs.rs:42-66`, `backend/src/jobs/mod.rs:24-70`

`claim_next` only selects `status = 'pending'`. If the API process dies after
claiming a job, the row stays `running` indefinitely. Large ingest payloads
queued above `ASYNC_THRESHOLD` (200) then never complete; operators see a
permanent “running” job with no recovery short of manual SQL.

**Fix:** Before claim (or inside the same statement), reset jobs with
`status = 'running' AND locked_at < NOW() - interval 'N minutes'` back to
`pending` (honouring `max_attempts`), and cover with a unit/integration test.

**Fixed:** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/46

### 3. **bug** — Ingest case-key lookup is case-sensitive

`backend/src/services/ingest.rs:176-183` (and callers at `:56-64`)

Parsers usually uppercase keys (`ingest/case_key.rs`), but `parse_vulyk_json`
passes `case_key` through unchanged, and `find_case_id` uses
`WHERE … AND key = $2` (exact). A payload with `"case_key": "demo-1"` against
stored `DEMO-1` lands in `unmatched_case_keys` and drops the result.

**Fix:** Normalize with `normalize_case_key` / `upper(key) = $2` before lookup
(same as `find_case_by_key`).

**Fixed (merged):** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/49

### 4. **bug** — Bulk PDF export has no `case_ids` cap

`backend/src/services/case_export.rs:1125-1167`,
`backend/src/routes/cases.rs:215-240`

`results` is clamped (max 50) but `case_ids` can be arbitrarily large. Each PDF
is built in memory and concatenated into a ZIP — easy to OOM or stall the API
on a 20k-case project “select all + export”.

**Fix:** Reject requests above a documented max (e.g. 100) with `400 bad_request`;
document in `docs/api.md`.

**Fixed (merged):** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/51

---

## P1 / P2 — races & correctness risks

### 5. **risk** — Prefix / project-key allocation TOCTOU

`backend/src/services/case_keys.rs:198-243`, `287-322`;
`backend/src/routes/projects.rs:99-129`

`unique_prefix` / `unique_project_key` check-then-insert outside a transaction.
Unique indexes prevent corruption, but concurrent creates can surface as opaque
`conflict` DB errors instead of a clean retry. Prefer insert + retry on unique
violation, or take an advisory lock around allocation.

### 6. **risk** — Ingest idempotency check is racy

`backend/src/services/ingest.rs:215-235`;
`backend/migrations/20251025140000_w4_jobs_and_ingest.sql:36-37`

Idempotent replay is `SELECT` then `INSERT` on `(run_id, external_id, status)`
with only a non-unique index on `(run_id, external_id)`. Concurrent workers can
insert duplicate history rows for the same external id. Consider a partial unique
index or `ON CONFLICT` if duplicate suppression is required.

### 7. **risk** — Case updates lack optimistic concurrency

`backend/src/routes/cases.rs:722-727`

`version` increments but updates do not require `expected_version` / `If-Match`.
Two editors can silently overwrite each other; version history still snapshots
pre-update state, so recovery is possible but UX is “last write wins”.

### 8. **risk** — `snapshot_case` swallows version conflicts

`backend/src/services/cases.rs:253-263`

`ON CONFLICT (case_id, version) DO NOTHING` can drop a snapshot under concurrent
updaters, leaving a version number without a corresponding row.

### 9. **risk** — `update_run_item` auto-completes run using only `Untested`

`backend/src/routes/runs.rs:635-650`

Completion checks `status = 'Untested'` only. Items left in `Ready` / `Running`
do not block “completed”, which may surprise Squash-style workflows.

### 10. **risk** — Folder delete `delete_cases` relies on FK cascade for child suites

`backend/src/routes/suites.rs:301-365`;
`backend/migrations/002_core_schema.sql:24`

Cases in the tree are deleted explicitly; child suites disappear via
`ON DELETE CASCADE` on `parent_id`. Behaviour is correct but undocumented and
easy to break if the FK ever changes to `SET NULL` / `RESTRICT`. Prefer an
explicit `DELETE FROM suites WHERE id = ANY(descendant_ids)`.

---

## Performance (20k cases)

### 11. **risk** — Case list correlated `last_run_status` subquery

`backend/src/routes/cases.rs:400-405`

For every candidate row (before `LIMIT`), Postgres evaluates
`SELECT … FROM test_results … ORDER BY executed_at DESC LIMIT 1`. Indexes on
`(case_id, executed_at DESC)` exist, but sorting/filtering 20k rows still hurts.
Prefer a lateral join only for the page, or a denormalized
`last_run_status` column (ingest already updates `last_automation_*`).

### 12. **risk** — Tag filter via `LEFT JOIN` + `GROUP BY`

`backend/src/routes/cases.rs:346-429`

Joining `case_tags` / `tags` multiplies rows; `COUNT(DISTINCT)` and `GROUP BY`
compensate but add cost. Use `EXISTS` / `IN (SELECT …)` for the tag predicate
when `tag` is set, and avoid the join when it is not.

### 13. **risk** — Title/key `ILIKE '%'||q||'%'` defeats index use

`backend/src/routes/cases.rs:353-357`

Alongside FTS, leading-wildcard `ILIKE` forces seq scans on large projects.
Keep FTS / `pg_trgm` as the primary path; gate or remove the `%q%` fallback for
short queries.

### 14. **risk** — Post-ingest per-case `UPDATE` loop

`backend/src/services/ingest.rs:82-97`

Matched cases are updated one-by-one after bulk insert. A 5k-result ingest
issues thousands of round-trips. Batch with `UPDATE … FROM UNNEST` or a temp table.

### 15. **risk** — Unbounded list endpoints

`backend/src/routes/runs.rs:690-696` (`list_results`),
`backend/src/routes/runs.rs:450-464` (`list_run_items` loads all items),
`backend/src/routes/reports.rs:469-499` (export with `limit: 10_000`)

Large runs/exports can return multi‑MB JSON. Cap or paginate consistently with
`PageQuery` (max 500).

### 16. **improvement** — Frontend case list already virtualizes + cancels

`frontend/src/pages/CaseList.tsx:352-382`, `:949`

Good: `cancelled` flag and `VirtualList`. Remaining gap: `api.ts` has no
`AbortController`, so in-flight fetches still hit the network after navigation
(`frontend/src/api.ts:17-54`).

---

## API contract / OpenAPI / errors

### 17. **risk** — OpenAPI omits webhook + some SSO / backup handlers

`backend/src/openapi.rs:73-301` vs `backend/src/routes/webhooks.rs`,
`auth.rs` (`sso_start` / `sso_callback`), `backups.rs::restore_upload`

Most handlers are registered in `ApiDoc`, but GitHub/GitLab webhooks and SSO
callback/start are annotated incompletely or missing from `paths(…)`. Generated
clients and `/api/docs` under-document production surfaces.

### 18. **improvement** — `AppError::Gone` always emits `case_deleted`

`backend/src/error.rs:47`

Only case-key retirement uses `Gone` today (`cases.rs:654`), so the code is
accurate for now, but the mapping is too specific if `Gone` is reused.

### 19. **improvement** — Campaign list query params not in OpenAPI params list

`backend/src/routes/campaigns.rs:565-571` (`list_suite_results`) declares no
`limit`/`offset` query params in utoipa despite accepting them.

### 20. **risk** — Auth gaps on some read endpoints (noted for security agent)

`backend/src/routes/cases.rs:1136-1177` (`list_case_results_*`),
`backend/src/routes/runs.rs:569-576` (`list_run_results_history_endpoint`)

No `require_permission` — intentional when `auth.required = false`, but with
auth enabled these still skip membership checks. Tracked for the security pass;
not fixed here.

---

## Migrations / upgrade / rollback

### 21. **risk** — Forward-only migrations; no SQL down scripts

`backend/migrations/*`, `docs/guides/upgrade.md`

Matches documented design (backup → migrate → health; rollback restores DB
dump). Operators cannot partially reverse a single migration without restore.
Ensure every new migration is expand/contract safe (additive columns, dual-write
windows) — current set largely is.

### 22. **improvement** — Job claim index only covers `pending`

`backend/migrations/20251025140000_w4_jobs_and_ingest.sql:23-24`

After adding stale reclaim, add a partial index on
`(locked_at) WHERE status = 'running'` so reclaim scans stay cheap.

### 23. **risk** — Destructive suite/case deletes are irreversible except backup

`suites.rs` `delete_cases`, bulk case delete + tombstones. Keys are never
reissued (good), but content recovery depends on content/version backups. Docs
are accurate; UI should keep confirming destructive actions (already partly
there).

---

## Accessibility & i18n

### 24. **risk** — Hardcoded English `aria-label`s / placeholders

Examples:

- `frontend/src/pages/CaseDetail.tsx:680,994,1062,1163,1188,1204,1226`
- `frontend/src/pages/RunDetail.tsx:271,278,304,392,400,508,522,549`
- `frontend/src/pages/Runs.tsx:166,287,296,315`
- `frontend/src/pages/RequirementsPage.tsx:69`
- `frontend/src/components/Layout.tsx:97`, `BottomNav.tsx:50`

Locale dictionaries are key-parity complete across EN/UK/FR/ES/DE
(`frontend/src/i18n/parity.test.ts`, 584 keys each), but screen-reader strings
bypass `t(…)`. Non-English users hear English chrome.

### 25. **improvement** — FTS config is English-only

`backend/src/routes/cases.rs:355`, `case_list.rs:79`

`plainto_tsquery('english', …)` under-serves Ukrainian (and other) title search.
Longer-term: `simple` config + `pg_trgm`, or per-locale configs.

### 26. **improvement** — PDF export locales are complete; UI chrome mostly wired

`backend/src/services/case_export.rs` embeds EN/UK/FR/ES/DE labels. Keep new UI
strings in all five locale files (AGENTS.md).

---

## Docs accuracy

### 27. **improvement** — API doc should mention bulk export max (after fix)

`docs/api.md` “Case PDF export” — add the `case_ids` maximum once capped.

### 28. **improvement** — Architecture ERD slightly behind schema

`docs/architecture.md` ERD omits campaigns/iterations, case key tombstones,
`user_project_preferences`, automation repos, backups — fine as Phase-1 plan,
but link to newer guides so readers do not treat it as current physical schema.

### 29. **improvement** — CI failure catalog is accurate and should stay updated

`docs/ci-failures.md` correctly documents seed-key collisions, Lighthouse
advisory, Playwright locator churn. Continue using fragment seeds + TestIds.

---

## Test coverage gaps

### 30. **risk** — No regression for run-item `executed_at` preservation — **addressed in #42**

Covered by `backend/tests/run_item_executed_at.rs`.

### 31. **risk** — No test for job reclaim after crash — **addressed in #46**

Covered by `backend/tests/job_reclaim.rs`.

### 32. **risk** — No ingest test for mixed-case `case_key` — **addressed in #49**

Covered by `backend/tests/ingest_case_key_casefold.rs`.

### 33. **improvement** — Limited e2e around bulk export / large lists

`e2e/tests/case-export-pdf.spec.ts` covers happy path; no stress for many ids
or 20k list scroll. Folder lazy load has coverage (`folder-tree-lazy.spec.ts`).

### 34. **improvement** — MCP / CLI surface thinner than API

`mcp/` and `cli/` smoke tests exist but do not mirror every new API
(campaigns, PDF export, column prefs). Acceptable if they call HTTP; watch drift.

---

## What looks healthy

- Case key allocation via `UPDATE … RETURNING` inside a transaction
  (`case_keys.rs:104-123`) — concurrency-safe.
- Locale key parity tests for all five languages.
- Case list column sort whitelist (`case_list.rs`) avoids SQL injection via
  sort field.
- Password policy shared across setup/seed/change (`auth/password.rs`); login
  accepts legacy and forces change.
- Upgrade path with verified backup + rollback points is documented and
  CI-tested (`upgrade-test` job).
- Playwright `TestIds` pattern + audit spec reduce locator flake.

---

## Fix plan (this review cycle)

| Priority | Finding | PR |
| --- | --- | --- |
| P0 | #1 run-item `executed_at` | [#42](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/42) **merged** |
| P0 | #2 stale job reclaim | [#46](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/46) |
| P1 | #3 ingest case_key normalize | [#49](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/49) **merged** |
| P1 | #4 bulk export cap | [#51](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/51) **merged** |
| P1 | #35 OpenAPI version pin | [#58](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/58) **merged** |
| P1 | #36 requirement coverage % | [#60](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/60) **merged** |
| Later | #11–#15 perf | separate scale PR (not in this pass) |
| Later | #24 / #44–#46 a11y i18n | separate UI PR |
| Later | #37–#40 project isolation | correctness follow-up (overlaps security) |
| Security agent | #20 auth on result history | fixed by [#50](https://github.com/Ihor-Meat-Proxy/Vulyk/pull/50) |

---

## Additional findings (deep-dive follow-up)

From parallel passes: [Backend correctness review](bc-3c5709c2-a5ba-5dcd-bb51-254d81c7f86c),
[API OpenAPI migrations review](bc-b5893db1-cbd4-5106-8751-4dca2dfbf693),
[Frontend i18n a11y review](bc-3f5b1306-07ec-5c58-bccd-6aebc3d973cf).

### 35. **bug** — OpenAPI `info.version` is `0.2.0` while product is `0.1.0`

`backend/src/openapi.rs:69` vs `VERSION` / `CARGO_PKG_VERSION`.

**Fixed (merged):** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/58

### 36. **bug** — Per-requirement `coverage_percent` is binary 0/100

`backend/src/services/reports.rs:259` ignored `cases_with_results`.

**Fixed (merged):** https://github.com/Ihor-Meat-Proxy/Vulyk/pull/60

### 37. **bug** — Cross-project `case_id` accepted into runs/results

`backend/src/routes/runs.rs` create-run / ingest; `backend/src/services/ingest.rs`
trusts client `case_id` without `project_id` filter. FK allows any case UUID.

### 38. **bug** — `suite_id` not validated against project

`backend/src/routes/cases.rs` create/update/bulk move; chat confirm. Cases can
hang under another project’s folder.

### 39. **bug** — Bulk tag ops report `affected = case_ids.len()` blindly

`backend/src/routes/cases.rs:912-922` — foreign/missing IDs still counted.

### 40. **bug** — MCP bulk ingest commits partial batches then 400s

`backend/src/routes/runs.rs:724-776` — per-row insert without a transaction.

### 41. **bug** — OpenAPI omits restore/upload + webhooks; SSO paths unregistered

`backups.rs` restore/upload; `webhooks.rs` (4); `auth.rs` `sso_start` /
`sso_callback` have annotations but are missing from `ApiDoc::paths`.

### 42. **bug** — Campaigns `Page.limit` echoes unclamped query value

`backend/src/routes/campaigns.rs:545-550` vs service clamp `1..=200`.

### 43. **bug** — Docs invent restore/preview + project-admin backup RBAC

`docs/backup-design.md` vs `RequireSystemAdmin` + `dry_run` on restore only.

### 44. **bug** — CaseDetail / RunDetail stale paint on fast navigation

`frontend/src/pages/CaseDetail.tsx:365-390`, `RunDetail.tsx:69-89` —
`cancelled` does not gate `setDetail` / related setters.

### 45. **bug** — CasePicker / Generate silently cap at 500 cases

`frontend/src/components/CasePicker.tsx:44-46`, `Generate.tsx:92`.

### 46. **bug** — Runs list hard-capped at 50 with no load-more

`frontend/src/pages/RunsPage.tsx:50` + `api.ts` default limit.

### 47. **risk** — Admin / Runs / Requirements / CaseDetail largely English-only

Locale **key** parity is green; many surfaces still use hardcoded English
(including `aria-label`s). Docs claiming full EN/UK/FR/ES/DE UI overstate
coverage.

### 48. **risk** — Dialogs without focus trap

`RunDetailsDrawer`, `CaseListColumnSettings` — `role="dialog"` without initial
focus / Tab trap / restore.

### 49. **improvement** — Sample VQA counts disagree across guides (~140 / ~160 / ~195 / actual ~200)

Align admin-guide / user-guide / first-run-setup to one figure.

---

## Review method

Static review of services, routes, migrations, OpenAPI registration, frontend
list/i18n/a11y paths, and docs. Parallel exploration via
[Backend correctness review](bc-3c5709c2-a5ba-5dcd-bb51-254d81c7f86c),
[API OpenAPI migrations review](bc-b5893db1-cbd4-5106-8751-4dca2dfbf693), and
[Frontend i18n a11y review](bc-3f5b1306-07ec-5c58-bccd-6aebc3d973cf).
No production load test at 20k rows was run in this environment (no local
Postgres); scale notes are from query shape + indexes.
