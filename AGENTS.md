# Agent guide (Vulyk)

Cursor agents and humans: read this before opening or updating a PR. Do **not** touch any OpenProject repository — Vulyk↔OpenProject integration docs live only in this repo.

## Pre-merge checklist

1. **Rebase onto latest `main` right before you expect to merge** (not only at PR open). Parallel agents land shared-file changes constantly.
2. Run **`./scripts/check.sh`** (or `--full` before push). It mirrors the required CI gates.
3. Confirm **migrations**, **sample seeds**, **i18n**, and **e2e locators** follow the rules below.
4. Prefer small PRs. Match style: `cargo fmt`, clippy `-D warnings`, frontend oxlint/tsc, Playwright `data-testid`.
5. If the PR can affect **upgrading an existing install**, complete the [Upgrade path](#upgrade-path-same-pr) section in the **same PR**.
6. When changing an HTTP endpoint, keep its Axum method/path and
   `#[utoipa::path]` registration aligned, add focused success/error tests, and
   run `./scripts/check-api-coverage.sh`.

## Upgrade path (same PR)

Any change that could affect upgrading an existing install must update the
upgrade/migration path **in the same PR** (see `.cursor/rules/upgrade-path.mdc`
and [docs/guides/upgrade.md](docs/guides/upgrade.md)):

- New/renamed env or config keys → update **`.env.example`** with sync markers
  (`@vulyk:sync` / `@vulyk:secret` / `@vulyk:required`). `.env.example` is the
  source of truth; `scripts/env_settings.py` + `install.sh` **add missing keys
  only** (backup first; never overwrite existing values; generate secrets when
  marked).
- Compose services/volumes/image names → keep interpolation upgrade-safe (avoid
  bare `${VAR:?…}` unless the key is synced with a default or `@vulyk:secret`).
- DB schema/data → forward-only SQLx migration (never edit migrations on `main`).
- `./scripts/install.sh upgrade` preflight must tolerate a **stopped API**
  (fall back to git/compose version; do not abort on `curl` exit 7).
- Keep **`./scripts/ci-env-upgrade-test.sh`** green (previous release `.env` → HEAD).

Do **not** overwrite operator `.env`, secrets, or admin passwords on upgrade.

Install git hooks (once per clone):

```bash
git config core.hooksPath scripts/githooks
```

- `pre-commit` → `./scripts/check.sh --quick` (fmt, seed keys, migrations, lint, tsc, testids)
- `pre-push` → `./scripts/check.sh --full` (+ clippy, cargo test, vitest)

## Sample seed keys (collision-proof)

**Never** invent the next `VQA-NNN` in a parallel PR by reading an old `vulyk-qa.json` and appending. That is the #1 CI killer when several agents land at once.

| Do | Don't |
| --- | --- |
| Add a fragment under `backend/seed/vulyk-qa.d/<feature>.json` with **keys omitted**, register it in `VULYK_QA_FRAGMENTS` in `backend/src/sample_seed.rs` | Append hardcoded `VQA-16x` blocks to `vulyk-qa.json` / `generate_vulyk_qa.py` from multiple PRs |
| Leave `"key"` empty so seed time / `./scripts/check-seed-keys.sh` allocates the next free `VQA-N` | Copy keys from another open PR |
| Run `./scripts/check-seed-keys.sh` **and** `./scripts/check-e2e-case-keys.sh` after every rebase | Assume uniqueness because your branch was green yesterday |
| For Playwright e2e: **append** one case + next free `VQA-N` to `playwright-e2e.json` by hand | Run `sync-vulyk-case-links.mjs` / `generate-vue-cases.mjs` on a feature branch (renumbers / rebuilds inventories) |

See `backend/seed/vulyk-qa.d/README.md` and `docs/dev/ci-failures.md`.

## Playwright / `data-testid`

- Prefer **`page.getByTestId(TestIds.…)`** (or `getByTestId('…')`) over ambiguous text / CSS.
- Source of truth: `frontend/src/testing/testIds.ts` + `docs/dev/testing/test-ids.md`.
- Add a constant, spread `testIdProps` / `listItemProps` on the control, update page objects.
- Run `npm run check:test-ids` (frontend) and keep `e2e/tests/test-id-audit.spec.ts` green.
- Avoid `getByText` for controls that exist more than once (toasts, “Save”, folder labels) — that causes strict-mode failures when sibling PRs duplicate UI.

### UI feature PRs must extend e2e

When a PR adds or changes a **user-visible UI flow**:

1. Add or update a Playwright spec under `e2e/tests/` (page objects in `e2e/pages/`).
2. Tag the test with Vulyk case keys (`@VQA-N` for the sample seed **and** `@VUE-N` for the dedicated UI e2e project).
3. Refresh case sources carefully (see pitfall below):
   - **NEVER** run `node e2e/scripts/sync-vulyk-case-links.mjs` on a feature
     branch to “pick up” one new test — even a dry run historically rewrote
     `playwright-e2e.json` and renumbered unrelated `@VQA-*` tags. Default sync
     is now **additive** (preserves keys); full rebuild requires
     `--force-renumber` and must not land from parallel PRs.
   - Prefer **appending** one case to `backend/seed/vulyk-qa.d/playwright-e2e.json`
     with the next free `VQA-N`, and appending to `e2e/cases/<folder>.json` with
     the next free `VUE-N` (check `manifest.json` / highest key). Register a
     narrative fragment under `vulyk-qa.d/` with **keys omitted** when the sample
     project should show the case too.
   - `generate-vue-cases.mjs --annotate` still rebuilds / renumbers `VUE-*` —
     append manually instead on feature branches.
4. Prefer local Docker: `./scripts/e2e-local.sh` (see [docs/dev/e2e-local.md](docs/dev/e2e-local.md)). Do **not** add new required CI jobs for this suite without an explicit decision.

### Recurring CI pitfalls

- **NEVER run `sync-vulyk-case-links.mjs` on feature branches** to allocate one
  key. Prefer manual append of the new case + key. The script’s default mode is
  additive (keeps existing keys); `--force-renumber` rebuilds the inventory and
  can still churn tags — restore `playwright-e2e.json` from `main` if you ran it
  by mistake.
- **`generate-vue-cases.mjs` renumbers all `VUE-N` keys** in sorted file/title
  order. Adding one test mid-alphabet shifts later keys. Append the new `VUE-N`
  (next after the current max) to `e2e/cases/*.json` + `manifest.json` instead.
- **`./scripts/check-e2e-case-keys.sh`** (also in `./scripts/check.sh --quick`)
  fails on: gaps in `playwright-e2e.json` VQA numbers, duplicate `external_key`
  across seed files, or spec `@VQA`/`@VUE` tags that do not resolve.

## API route coverage

- `ApiDoc::openapi()` is the canonical operation inventory; every Axum
  method/path must appear there and every OpenAPI operation must dispatch.
- `backend/tests/api_coverage.rs` performs DB-free route/auth/validation sweeps
  and a DB-backed random-ID error sweep when `DATABASE_URL` is set.
- Public operations and inapplicable error classes require a justification in
  `scripts/api-coverage-manifest.json`; stale entries fail the gate.
- CI uploads JSON and Markdown reports from `target/api-coverage/`.
- Add endpoint-specific integration assertions for normal success and its main
  `400`/`403`/`404` behavior; the generic inventory sweep does not replace
  business-behavior tests.

## Migrations (parallel PRs)

- Forward-only SQLx files under `backend/migrations/`.
- **Unique** `YYYYMMDDHHMMSS_description.sql` timestamps (UTC). If two PRs pick the same second, rename yours after rebase.
- **Never edit** a migration that already exists on `main` — ship a follow-up.
- CI: `./scripts/check-migrations.sh`. Details in `CONTRIBUTING.md`.

## i18n (FR / ES / DE)

When you add user-visible UI strings:

1. Add keys to **all** locale files under `frontend/src/i18n/` (`en`, `uk`, `fr`, `es`, `de`) — do not leave FR/ES/DE missing.
2. Do not assert translated copy in Playwright; use test ids / roles.
3. Keep the globe language menu and gear user menu working (login shell included).

## Lighthouse

The **Lighthouse PWA/a11y (advisory)** job uses `browser-actions/setup-chrome` and **`continue-on-error: true`**. Chrome CDN flakes must not block merges. Still fix real score regressions before release. Do not reintroduce `@puppeteer/browsers install chrome@stable` as the only path.

## Shared files — rebase discipline

High-churn paths (rebase carefully, re-run checks):

- `backend/seed/vulyk-qa.json`, `backend/seed/generate_vulyk_qa.py`, `backend/src/sample_seed.rs`
- `backend/migrations/*`
- `frontend/src/testing/testIds.ts`, layout/header/case list components
- `e2e/pages/*`, `e2e/tests/*`
- `.github/workflows/ci.yml`

## Security scanning

PRs and `main` also run [`.github/workflows/security.yml`](.github/workflows/security.yml)
(weekly cron as well): `cargo audit` / `cargo deny`, `npm audit`, OSV, Trivy (FS +
images), CodeQL, Semgrep, gitleaks. Dependabot: [`.github/dependabot.yml`](.github/dependabot.yml).

```bash
./scripts/check-security.sh          # local mirror (audit + deny + npm audit)
./scripts/check-security.sh --quick  # Rust only
```

Justified RustSec ignores live in `deny.toml` and `.cargo/audit.toml` with review-by
dates — prefer upgrades; do not extend ignores without a reason + expiry.
Details: [`docs/dev/ci-checks.md`](docs/dev/ci-checks.md).

## Quick commands

```bash
./scripts/check.sh           # default local gate
./scripts/check.sh --full    # closer to CI
./scripts/check-security.sh  # CVE / license / npm audit
./scripts/check-quality.sh   # machete, eslint, prettier, knip, coverage
./scripts/check-seed-keys.sh
./scripts/check-e2e-case-keys.sh
./scripts/check-migrations.sh
./scripts/check-api-coverage.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cd frontend && npm run lint && npm run lint:eslint && npm run format:check && npm run typecheck && npm run check:test-ids
cd e2e && npm run typecheck
```

## More docs

- Doc index: [`docs/README.md`](docs/README.md)
- CI checks index (security + quality): [`docs/dev/ci-checks.md`](docs/dev/ci-checks.md)
- CI failure catalog + pipeline diagram: [`docs/dev/ci-failures.md`](docs/dev/ci-failures.md)
- Architecture: [`docs/dev/architecture.md`](docs/dev/architecture.md)
- Playwright test ids: [`docs/dev/testing/test-ids.md`](docs/dev/testing/test-ids.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
