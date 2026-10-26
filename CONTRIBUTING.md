# Contributing to Vulyk

Thanks for helping build the beehive.

## Ground rules

1. Read [docs/architecture.md](docs/architecture.md) before large changes. Pick a workstream id (W1–W11) and keep module boundaries.
2. Do **not** open PRs or commits against the OpenProject project itself. Document Vulyk↔OpenProject integration only in this repo.
3. Prefer small, reviewable PRs. Match existing style (`cargo fmt`, frontend oxlint/tsc).
4. License is AGPL-3.0 (existing `LICENSE`). Do not re-license without maintainer agreement.

## Dev setup

```bash
./scripts/install.sh check
./scripts/install.sh dev
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api
cd frontend && npm install && npm run dev
```

## Checks (same as CI)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd frontend && npm ci && npm run lint && npm run typecheck && npm run test && npm run check:test-ids && npm run build
docker build -f backend/Dockerfile -t vulyk-api:local .
docker build -f frontend/Dockerfile -t vulyk-web:local ./frontend
./scripts/check-migrations.sh
```

## Database migrations

Schema changes ship as SQLx files under `backend/migrations/`. CI enforces integrity
(`scripts/check-migrations.sh`): no edits to files that already exist on `main`, and
no colliding version prefixes.

### Rules

1. **Forward-only.** Do not add reversible `.down.sql` workflows for production. Fixes are new migrations.
2. **Never edit an applied migration.** Once a file has landed on `main` / a release, treat it as immutable. Ship a follow-up migration instead.
3. **Unique timestamped versions.** Prefer `YYYYMMDDHHMMSS_description.sql` (UTC). The numeric prefix before the first `_` is the SQLx version and must be unique.
4. **Expand / contract for breaking changes.** Add new columns/tables first (expand), deploy dual-write/read code, then remove old shapes in a later migration (contract). Avoid one-shot renames that break rolling deploys.
5. **Data migrations must be idempotent.** Use `WHERE` guards, `ON CONFLICT`, or `IF NOT EXISTS` so re-runs (or repair) are safe.
6. **Keep migrations small and reviewable.** Large backfills may use batched SQL; document runtime expectations in the PR.
7. **Never destroy operator data on upgrade.** Migrations must be additive/backward-compatible. Operator upgrades take a verified backup first and auto-roll back on migrate/health failure — see [docs/guides/upgrade.md](docs/guides/upgrade.md#data-safety-guarantees). The Upgrade CI job must keep passing (sample data + encrypted secrets survive).

Local helpers:

```bash
cargo run -p vulyk-api -- migrate status
cargo run -p vulyk-api -- migrate run
```

Upgrades for operators: [docs/guides/upgrade.md](docs/guides/upgrade.md).

## Playwright test ids

Interactive UI must expose stable `data-testid` attributes. Convention, list/row attrs, strip flag (`VITE_STRIP_TEST_IDS`), security (no PII/secrets in `data-*`), and enforcement are documented in [docs/testing/test-ids.md](docs/testing/test-ids.md). Use typed constants from `frontend/src/testing/testIds.ts` in components and e2e — do not invent ad-hoc selectors. Never put emails, names, passwords, or tokens into `data-testid` / `data-entity-*`.

## API / schema changes

- Add SQLx migrations under `backend/migrations/` (follow the rules above).
- Annotate routes with `utoipa` so `/api/docs` stays accurate.
- Update architecture notes if you change plugin contracts.
- When bumping the release version, update the root `VERSION` file,
  `[workspace.package].version` in `Cargo.toml`, `frontend/package.json`, Helm
  `appVersion`, and `CHANGELOG.md`, then run `./scripts/check-version.sh`. Tag
  `vX.Y.Z` on `main` to publish (see [docs/guides/releasing.md](docs/guides/releasing.md)).

## Parallel workstreams

Coordinate via the table in `docs/architecture.md`. Shared contracts: OpenAPI + this doc. Suggested merge order when conflicting: **W1 → W2/W3 → W4 → others**.
