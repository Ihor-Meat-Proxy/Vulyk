# Contributing to Vulyk

Thanks for helping build the beehive.

## Ground rules

1. Read [docs/README.md](docs/README.md) and [docs/dev/architecture.md](docs/dev/architecture.md) before large changes. Keep module boundaries (`routes` / `services` / …).
2. Do **not** open PRs or commits against the OpenProject project itself. Document Vulyk↔OpenProject integration only in this repo.
3. Prefer small, reviewable PRs. Match existing style (`cargo fmt`, frontend oxlint/tsc).
4. License is AGPL-3.0 (existing `LICENSE`). Do not re-license without maintainer agreement.

## Dev setup

```bash
./scripts/install.sh check
./scripts/install.sh dev
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api
./scripts/npm-ci-secure.sh frontend && (cd frontend && npm run dev)
```

## Checks (same as CI)

Prefer the single entrypoint (also used by git hooks):

```bash
./scripts/check.sh          # fmt, seed keys, migrations, lint, tsc, testids, e2e tsc
./scripts/check.sh --full   # + clippy, cargo test, vitest
git config core.hooksPath scripts/githooks   # once per clone
```

Manual equivalents:

```bash
./scripts/check-seed-keys.sh
./scripts/check-migrations.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/npm-ci-secure.sh frontend && (cd frontend && npm run lint && npm run typecheck && npm run check:test-ids && npm run test && npm run build)
docker build -f backend/Dockerfile -t vulyk-api:local .
docker build -f frontend/Dockerfile -t vulyk-web:local .
```

Agents: see root [AGENTS.md](AGENTS.md), [docs/dev/ci-failures.md](docs/dev/ci-failures.md), and [SECURITY.md](SECURITY.md).

## Database migrations

Schema changes ship as SQLx files under `backend/migrations/`. Full rules, local
commands, Squawk vs hard-fail behavior, and what CI runs are documented in
**[docs/dev/migrations.md](docs/dev/migrations.md)**.

Summary:

1. **Forward-only** in production (no `.down.sql` for fixes). Optional downs are
   exercised by CI when present.
2. **Never edit** a migration on `main` or a **release tag** (`v0.6.0` / latest
   `v*`) — CI hard-fails. Add a new timestamped file instead.
3. **Unique** `YYYYMMDDHHMMSS_description.sql` version prefixes.
4. **Expand / contract** for breaking shape changes; keep data migrations idempotent.
5. **Same-PR upgrade path** for env/compose/schema/install changes — see
   [AGENTS.md](AGENTS.md#upgrade-path-same-pr).

```bash
./scripts/check-migrations.sh
./scripts/lint-migrations-squawk.sh          # risky DDL warnings
cargo run -p vulyk-api -- migrate status
cargo run -p vulyk-api -- migrate run
```

Upgrades for operators: [docs/guides/upgrade.md](docs/guides/upgrade.md).

## Playwright test ids

Interactive UI must expose stable `data-testid` attributes. Convention, list/row attrs, strip flag (`VITE_STRIP_TEST_IDS`), security (no PII/secrets in `data-*`), and enforcement are documented in [docs/dev/testing/test-ids.md](docs/dev/testing/test-ids.md). Use typed constants from `frontend/src/testing/testIds.ts` in components and e2e — do not invent ad-hoc selectors. Never put emails, names, passwords, or tokens into `data-testid` / `data-entity-*`.

## API / schema changes

- Add SQLx migrations under `backend/migrations/` (follow the rules above).
- Annotate routes with `utoipa` so `/api/docs` stays accurate.
- Update architecture notes if you change plugin contracts.
- When bumping the release version, update the root `VERSION` file,
  `[workspace.package].version` in `Cargo.toml`, `frontend/package.json`, Helm
  `appVersion`, and `CHANGELOG.md`, then run `./scripts/check-version.sh`. Tag
  `vX.Y.Z` on `main` to publish (see [docs/dev/releasing.md](docs/dev/releasing.md)).

## Parallel workstreams

Shared contracts: OpenAPI + this doc + [docs/dev/architecture.md](docs/dev/architecture.md). Prefer small PRs; rebase onto latest `main` before merge.

**Rebase onto latest `main` immediately before merge.** For sample cases, prefer
`backend/seed/vulyk-qa.d/` fragments with omitted keys over appending `VQA-NNN` to
the shared seed (see `AGENTS.md`). For UI e2e, use `data-testid` / `TestIds` only.

## Lighthouse

The CI job **Lighthouse PWA/a11y (advisory)** is `continue-on-error` and installs
Chrome via `browser-actions/setup-chrome`. Do not block merges on Chrome CDN flakes;
still fix real PWA/a11y score regressions before release.
