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
| Run `./scripts/check-seed-keys.sh` after every rebase | Assume uniqueness because your branch was green yesterday |

See `backend/seed/vulyk-qa.d/README.md` and `docs/ci-failures.md`.

## Playwright / `data-testid`

- Prefer **`page.getByTestId(TestIds.…)`** (or `getByTestId('…')`) over ambiguous text / CSS.
- Source of truth: `frontend/src/testing/testIds.ts` + `docs/testing/test-ids.md`.
- Add a constant, spread `testIdProps` / `listItemProps` on the control, update page objects.
- Run `npm run check:test-ids` (frontend) and keep `e2e/tests/test-id-audit.spec.ts` green.
- Avoid `getByText` for controls that exist more than once (toasts, “Save”, folder labels) — that causes strict-mode failures when sibling PRs duplicate UI.

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
Details: [`docs/ci-checks.md`](docs/ci-checks.md).

## Quick commands

```bash
./scripts/check.sh           # default local gate
./scripts/check.sh --full    # closer to CI
./scripts/check-security.sh  # CVE / license / npm audit
./scripts/check-quality.sh   # machete, eslint, prettier, knip, coverage
./scripts/check-seed-keys.sh
./scripts/check-migrations.sh
./scripts/check-api-coverage.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cd frontend && npm run lint && npm run lint:eslint && npm run format:check && npm run typecheck && npm run check:test-ids
cd e2e && npm run typecheck
```

## More docs

- Doc index: [`docs/README.md`](docs/README.md)
- CI checks index (security + quality): [`docs/ci-checks.md`](docs/ci-checks.md)
- CI failure catalog + pipeline diagram: [`docs/ci-failures.md`](docs/ci-failures.md)
- Architecture: [`docs/architecture.md`](docs/architecture.md)
- Playwright test ids: [`docs/testing/test-ids.md`](docs/testing/test-ids.md)
- Audits (Oct 2026): [`docs/security/audit-2026-10.md`](docs/security/audit-2026-10.md), [`docs/quality/code-quality-2026-10.md`](docs/quality/code-quality-2026-10.md), [`docs/review/code-review-2026-10.md`](docs/review/code-review-2026-10.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
