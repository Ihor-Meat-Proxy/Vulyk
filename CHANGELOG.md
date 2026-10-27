# Changelog

All notable changes to Vulyk are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-10-27

Third release: two-way **Run in CI**, REST automation-repo indexing (no local
clone for GitHub/GitLab), security hardenings from the Oct 2026 audit follow-ups,
case-list performance, i18n/a11y polish, OpenAPI coverage, and upgrade `.env`
sync that tolerates a stopped API.

### Added

- **Run in CI** — Executions → **Run in CI** creates a Queued Vulyk run and
  dispatches GitHub `workflow_dispatch` with `vulyk_run_id` / `case_keys`;
  pipelines attach via `VULYK_RUN_ID`, `POST …/ci/runs/{id}/start`, and webhook
  `pipeline_url` sync; example workflow
  `.github/workflows/vulyk-dispatch-example.yml` (#93)
- **Automation repo Actions write token** — separate encrypted token + workflow
  file/ref for dispatch; Playwright reporter honors `VULYK_RUN_ID` (#93)
- **Self-hosted FOSS fonts** + per-user font picker (#83)
- **E2E ↔ Vulyk** — Playwright tests linked to cases with CI reporter (#88)
- Generic `.env` sync on upgrade (`scripts/env_settings.py`) with
  `@vulyk:sync` / `@vulyk:secret` / `@vulyk:required`; stopped-API preflight
  no longer aborts upgrades (#82)

### Changed

- **GitHub/GitLab automation indexing via REST** — tree/blobs over the provider
  API (incremental by blob SHA); no local `git` for those providers. Generic
  HTTPS remotes still shallow-clone; API image installs `git` (#92)
- Case list: denormalized `last_run_status`, EXISTS-based tag filter, gated
  leading-wildcard ILIKE (#84, #85, #86)
- Paginate run results/items and cap exports (#87)
- Runs list load-more beyond the soft page of 50; surface 500-case soft caps
  in CasePicker / Generate (#73, #74)

### Security

- Fail-closed auth under production markers (H11) (#67)
- API + nginx security headers (M4) (#75)
- CORS allowlist from `VULYK_CORS_ORIGINS` / base URL (M3) (#76)
- Double-submit CSRF for cookie-only mutations (M2) (#77)
- Secure session/CSRF cookies under production markers (M1) (#78)
- Harden Helm/Compose containers (M8) (#79)
- Bump `zip` crate 2 → 9 (M13) (#80)
- Bump Lighthouse to 13.5 and adapt PWA gates (M14) (#81)

### Fixed

- Optimistic concurrency on case update (`expected_version` → 409) (#91)
- Case version history always has a `case_versions` row per version (#98)
- Idempotent ingest replays: unique `(run_id, external_id, status)` (#90)
- Retry project key/prefix allocation on unique conflict (#89)
- Reject cross-project `case_id` / `suite_id` on run, result, case, and chat
  paths (#68, #69)
- Bulk tag affected count scoped to in-project cases (#70)
- MCP bulk result ingest is transactional (#71)
- Ignore stale CaseDetail/RunDetail loads; abort in-flight API fetches on
  navigation (#72, #97)
- Focus trap for RunDetailsDrawer and column settings (#95)
- i18n: aria-labels/placeholders, CaseDetail/Runs/Requirements, Admin panels
  and CI Runs chrome (#94, #96, #101)
- OpenAPI: SSO, webhooks, restore upload paths; campaign list limit/offset
  (#100, #103)
- Campaign suite lists echo clamped `Page.limit` (#99)
- Backup docs: restore API + system-admin RBAC alignment (#102)

### Performance

- Case list denormalization and query rewrites (#84–#86)
- Paginated run results/items (#87)

### Docs

- CI two-way trigger + `VULYK_RUN_ID` attach (`docs/guides/ci-integration.md`)
- Automation repo REST indexing (`docs/automation-repo.md`, upgrade notes)
- Backup restore API clarifications (`docs/guides/backup.md`)

### Release artifacts

- GHCR images: `vulyk-api` and `vulyk-web` tagged `0.3.0` and `latest`
- GitHub Release attaches Linux API/CLI binaries, frontend `dist` archive,
  Compose/install bundle, and SHA256 checksums

## [0.2.0] - 2026-10-27

Second release after the 0.1.0 open-source launch: Admin configuration depth,
content backups and in-app upgrades/rollback, case PDF export, security
hardenings from the October 2026 audit, and documentation aligned with `main`.

### Added

- **Option lists, custom fields, config export/import** — Admin Lists / Custom
  fields / Export·import; scoped field definitions; `vulyk-api config
  export|import` and `install.sh config-export|config-import` (#24)
- **Content backups** — Admin → Backups: ZIP archives to local / GitHub /
  GitLab / Nextcloud / Drive; schedules, restore modes; path confinement via
  `VULYK_CONTENT_BACKUP_DIR` (#30)
- **Case PDF export** — single and bulk (ZIP); Unicode/Cyrillic fonts; locale
  labels; bulk `case_ids` capped at 100 (#31, #51)
- **Cases list columns** — customizable columns, server-side sort, custom-field
  columns (#25)
- **Case editor UX** — Tab-add-step, scoped Ctrl/Cmd+S, sticky save bar (#29)
- **Reports drill-down** — deep-linkable run report with filters/grouping/export
  (#34)
- **Admin Updates** — check GitHub Releases, optional Compose updater sidecar,
  one-click upgrade (#37)
- **Post-upgrade rollback** — version-backup points, Admin Roll back,
  `install.sh rollback --confirm=ROLLBACK`, retention env vars (#39)
- Hardened upgrade backups: verified temp-DB restore, config/secrets bundle,
  attachments archive, auto-rollback to previous images, downgrade guards
- QA seed fragments + Playwright coverage for Updates/rollback, multi-repo CI
  ingest, password policy/lockout, TestId hardening (#44, #56, #62, #63)
- Docs index (`docs/README.md`) and Oct 2026 security / quality / review audits
  linked from README / AGENTS

### Security

- Reject unbound (null-user) headless API tokens (#45)
- CI webhooks **fail closed** without a verifying secret (#48)
- Enforce `ViewProject` on results and jobs GETs (#50)
- Redact CI config tokens and webhook secrets in API responses (#54)
- Harden SSO redirect, session revoke on password change, JWT secret
  fail-closed when auth required (#57)
- Block git SSRF; redact credential leaks; shared egress allowlist;
  `VULYK_EGRESS_ALLOW_PRIVATE` for lab only (#59)
- Narrow CI token permissions; block `create_project` via CI tokens (#61)
- Shared password policy (default 16+, floor 12), forced change, login lockout
  (#32)

### Fixed

- Preserve run-item `executed_at` when status is omitted (#42)
- Case-key ingest match is case-insensitive (#49)
- Avoid panics in report headers and PDF tables (#47)
- Stable React keys for case steps and chat drafts (#53)
- Per-requirement coverage from executed links (#60)
- Reclaim stale `running` jobs after worker crash (#46)
- OpenAPI / version alignment with crate semver (#58)
- Collision-proof QA seed fragments and agent check scripts (#36)

### Performance

- Batch ingest case lookup and last_automation updates (#52)
- Run Argon2 hash/verify via `spawn_blocking` (#55)

### Release artifacts

- GHCR images: `vulyk-api` and `vulyk-web` tagged `0.2.0` and `latest`
- GitHub Release attaches Linux API/CLI binaries, frontend `dist` archive,
  Compose/install bundle, and SHA256 checksums

## [0.1.0] - 2026-10-26

Initial open-source release of Vulyk (*вулик* — Ukrainian for *beehive*).

### Added

- **API** — Rust (Axum + SQLx + OpenAPI/Scalar): auth/RBAC, personal API tokens,
  projects, nested folders, auto case keys (`PREFIX-N`), plans, runs, step results,
  requirements links, reports, CI ingest (JUnit / Playwright / pytest / Vulyk JSON),
  automation executions (campaigns, iterations, pipeline upload), multi-repo CI
  dispatch/webhooks, LLM generate + chat propose/create, importers scaffold
- **UI** — React + TypeScript + Vite PWA: cases/plans/runs, folder tree, i18n
  (en/uk/fr/es/de), theming, mobile bottom nav, offline queue
- **MCP** — `vulyk-mcp` stdio/HTTP server for AI assistants
- **CLI** — `vulyk` reporter for CI result upload
- **Deploy** — Docker Compose (dev + prod), Helm chart, `./scripts/install.sh`
  (`compose` / `prod` / `dev` / `setup` / `upgrade` / `backup` / `restore`)
- **Upgrades** — `vulyk-api migrate status|run`, advisory locks, downgrade
  protection, `VULYK_AUTO_MIGRATE`, Helm pre-upgrade migrate Job, PVC keep policy
- **Versioning** — `/health` and `/ready` expose `version`; UI sidebar shows API
  semver; GitHub Releases + GHCR images via `.github/workflows/release.yml`
- **Docs** — architecture, admin/user guides, upgrade and releasing guides

### Release artifacts

- GHCR images: `vulyk-api` and `vulyk-web` tagged `0.1.0` and `latest`
- GitHub Release attaches Linux API binary, frontend `dist` archive, Compose/install
  bundle, and SHA256 checksums
