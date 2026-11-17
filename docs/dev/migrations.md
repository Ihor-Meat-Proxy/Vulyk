# Database migrations

Schema changes ship as SQLx files under [`backend/migrations/`](../../backend/migrations/).
Production is **forward-only**. CI verifies fresh applies, upgrade-from-release,
optional down-migration reversibility, and Squawk safety lint.

## Rules

1. **Forward-only in production.** Do not ship `.down.sql` for production fixes —
   add a new migration instead. Optional sibling `*.down.sql` files are allowed for
   tests / future tooling; when present, CI runs up → down → up.
2. **Never edit an applied or released migration.** Files on `main` and on the
   latest release tag (prefer `v0.5.1`) are immutable. CI **hard-fails** on
   modify/delete. Ship a follow-up timestamped migration.
3. **Unique version prefixes.** Prefer `YYYYMMDDHHMMSS_description.sql` (UTC).
   The numeric prefix before the first `_` is the SQLx version and must be unique.
4. **Expand / contract** for breaking shape changes (add → dual-write → remove).
5. **Idempotent data migrations** (`WHERE` guards, `ON CONFLICT`, `IF NOT EXISTS`).
6. **Never destroy operator data on upgrade.** See [guides/upgrade.md](../guides/upgrade.md).
7. **Same-PR upgrade path** for env/compose/schema/install changes — see
   [AGENTS.md](../../AGENTS.md#upgrade-path-same-pr).

## Local commands

```bash
# Uniqueness + immutability vs origin/main and release tag
./scripts/check-migrations.sh
./scripts/check-migrations.sh origin/main

# Squawk risky-DDL lint (downloads binary if needed)
./scripts/lint-migrations-squawk.sh
./scripts/lint-migrations-squawk.sh origin/main

# Proof that Squawk flags deliberately bad SQL
./scripts/test-migration-lint.sh

# Fresh empty DB: migrate + status + schema smoke (+ sqlx prepare --check when macros exist)
DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk ./scripts/ci-migrations-verify.sh

# Optional downs: up -> down -> up
DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk ./scripts/ci-migrations-reversibility.sh

# Upgrade smoke from a release tag (same script as base-SHA upgrade CI)
VULYK_UPGRADE_BASE_REF=v0.5.1 DATABASE_URL=... ./scripts/ci-upgrade-test.sh

cargo run -p vulyk-api -- migrate status
cargo run -p vulyk-api -- migrate run
```

`./scripts/check.sh` always runs uniqueness/immutability and, when `squawk` is
available, the Squawk lint (soft locally).

## What CI runs

Job **Migration integrity** (always on PR + `main`, no path filter):

| Step | Severity | Script |
| --- | --- | --- |
| Unique versions + immutable vs PR base/`main` | **Hard fail** | `scripts/check-migrations.sh` |
| Immutable vs release tag (`v0.5.1` or latest `v*`) | **Hard fail** | same |
| Squawk on new/changed migration SQL | **Warning** (annotations, `continue-on-error`) | `scripts/lint-migrations-squawk.sh` |
| Squawk proof fixture (`DROP COLUMN` / `NOT NULL` / locking index) | **Hard fail** if not flagged | `scripts/test-migration-lint.sh` |
| Fresh migrate + status + schema smoke | **Hard fail** | `scripts/ci-migrations-verify.sh` |
| Reversibility when `*.down.sql` exist | **Hard fail** / skip if none | `scripts/ci-migrations-reversibility.sh` |
| Upgrade from `v0.5.1` seed → HEAD + API smoke | **Hard fail** / skip+warn if tag missing | `VULYK_UPGRADE_BASE_REF=v0.5.1` + `scripts/ci-upgrade-test.sh` |

The separate **Upgrade (base → PR)** job (`scripts/ci-upgrade-test.sh` with the
PR base SHA) remains unchanged and is also required.

### sqlx offline / prepare --check

The API mostly uses runtime `sqlx::query` (no committed `backend/.sqlx`).
`ci-migrations-verify.sh` therefore:

1. Migrates an empty database and asserts `migrate status` has `pending: 0`.
2. Smokes core tables via `psql`.
3. Runs `cargo sqlx prepare --check` **only if** `backend/.sqlx` exists or
   `query!` macros appear; otherwise runs `cargo test -p vulyk-api --test migrate_cli`.

When compile-time macros land, commit offline data and keep prepare --check green.

### Squawk warning vs hard-fail

- **Soft:** Squawk findings on real migration diffs (risky but sometimes intentional
  expand/contract steps). CI posts `::warning` annotations and continues.
- **Hard:** Editing a migration that already exists on a **release tag** or on the
  PR base / `main`. Also hard: uniqueness collisions, failed fresh migrate,
  failed upgrade-from-release, and the proof fixture not being flagged.

## See also

- [CONTRIBUTING.md](../../CONTRIBUTING.md) — contributor migration section
- [ci-checks.md](ci-checks.md) — CI index
- [ci-failures.md](ci-failures.md) — failure catalog
- [guides/upgrade.md](../guides/upgrade.md) — operator upgrades
