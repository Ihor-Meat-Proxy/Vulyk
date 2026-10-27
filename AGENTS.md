# Agent guide (Vulyk)

Cursor agents and humans: read this before opening or updating a PR. Do **not** touch any OpenProject repository — Vulyk↔OpenProject integration docs live only in this repo.

## Pre-merge checklist

1. **Rebase onto latest `main` right before you expect to merge** (not only at PR open). Parallel agents land shared-file changes constantly.
2. Run **`./scripts/check.sh`** (or `--full` before push). It mirrors the required CI gates.
3. Confirm **migrations**, **sample seeds**, **i18n**, and **e2e locators** follow the rules below.
4. Prefer small PRs. Match style: `cargo fmt`, clippy `-D warnings`, frontend oxlint/tsc, Playwright `data-testid`.

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

## Quick commands

```bash
./scripts/check.sh          # default local gate
./scripts/check.sh --full   # closer to CI
./scripts/check-seed-keys.sh
./scripts/check-migrations.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cd frontend && npm run lint && npm run typecheck && npm run check:test-ids
cd e2e && npm run typecheck
```

## More docs

- Doc index: [`docs/README.md`](docs/README.md)
- CI failure catalog + pipeline diagram: [`docs/ci-failures.md`](docs/ci-failures.md)
- Architecture: [`docs/architecture.md`](docs/architecture.md)
- Playwright test ids: [`docs/testing/test-ids.md`](docs/testing/test-ids.md)
- Audits (Oct 2026): [`docs/security/audit-2026-10.md`](docs/security/audit-2026-10.md), [`docs/quality/code-quality-2026-10.md`](docs/quality/code-quality-2026-10.md), [`docs/review/code-review-2026-10.md`](docs/review/code-review-2026-10.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
