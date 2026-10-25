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
cd frontend && npm ci && npm run lint && npm run typecheck && npm run test && npm run build
docker build -f backend/Dockerfile -t vulyk-api:local .
docker build -f frontend/Dockerfile -t vulyk-web:local ./frontend
```

## API / schema changes

- Add SQLx migrations under `backend/migrations/` (never edit applied migrations in place).
- Annotate routes with `utoipa` so `/api/docs` stays accurate.
- Update architecture notes if you change plugin contracts.

## Parallel workstreams

Coordinate via the table in `docs/architecture.md`. Shared contracts: OpenAPI + this doc. Suggested merge order when conflicting: **W1 → W2/W3 → W4 → others**.
