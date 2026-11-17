# CI checks

Index of automated gates beyond the narrative in [ci-failures.md](ci-failures.md).
Local mirrors live in `./scripts/check.sh`, `./scripts/check-security.sh`,
`./scripts/check-quality.sh`, `./scripts/check-shell-docker.sh`,
`./scripts/check-pr-hygiene.sh`, `./scripts/check-openapi-drift.sh`,
`./scripts/check-bundle-size.sh`, `./scripts/check-api-coverage.sh`, and
`./scripts/check-doc-links.sh`. Frontend: `npm run check:i18n` /
`npm run check:test-ids`.

## Workflows

| Workflow | File | When | Role |
| --- | --- | --- | --- |
| **CI** | `.github/workflows/ci.yml` | PR + push `main` | Required product gates (fmt, clippy, tests, API coverage, **Migration integrity** verify/lint/upgrade-from-release, upgrade base→PR, frontend, Docker, Playwright, doc links, …) |
| **Fast CI gates** | `.github/workflows/ci-fast-gates.yml` | PR + push `main` | Required **fast** checks (≤~2 min each, parallel + cached): PR hygiene, shellcheck/hadolint, machete, knip, cargo deny, npm audit, gitleaks |
| **Security** | `.github/workflows/security.yml` | PR + push `main` + **weekly** Monday | Deeper CVE/deps, containers, SAST, secrets (complements fast gates) |
| **Quality** | `.github/workflows/quality.yml` | PR + push `main` + weekly | Coverage floors, ESLint/Prettier, pedantic clippy; machete/knip also mirrored as fast required jobs |
| **Dependabot auto-merge** | `.github/workflows/dependabot-auto-merge.yml` | `pull_request_target` (Dependabot only) | Enables squash auto-merge for **patch** + **≥1.x minor** only; **0.x minor treated as breaking**; majors stay manual |
| **Release** | `.github/workflows/release.yml` | tags / release | Publish images |

Lighthouse remains **advisory** (`continue-on-error`) inside `ci.yml` — see [ci-failures.md](ci-failures.md).

## Fast CI gates (required)

Stable job names for branch protection (workflow **Fast CI gates**):

| Job name | Tool | Local |
| --- | --- | --- |
| `PR hygiene` | Conventional Commits title + `Closes #N` / `Part of #N` in body; **skips** `dependabot[bot]` / `renovate[bot]` | `PR_TITLE=… PR_BODY=… PR_AUTHOR=… ./scripts/check-pr-hygiene.sh` |
| `Shellcheck + hadolint` | shellcheck on `scripts/*.sh` + extracted Jenkins/GitLab snippets; hadolint on Dockerfiles (baseline: `.hadolint.yaml`) | `./scripts/check-shell-docker.sh` |
| `cargo machete` | Unused Rust crate deps | `cargo machete` / `./scripts/check-quality.sh --quick` |
| `knip` | Unused frontend files/deps/duplicates | `(cd frontend && npm run knip)` |
| `cargo deny` | Advisories + licenses + bans (`deny.toml`) | `cargo deny check` / `./scripts/check-security.sh --quick` |
| `npm audit` | `npm audit --omit=dev --audit-level=high` for `frontend/` + `e2e/` | same commands locally |
| `gitleaks` | Secrets scan (`.gitleaks.toml`) | `gitleaks detect --source . --config .gitleaks.toml` |
| `i18n completeness` | Locale key parity (en/uk/fr/es/de), placeholder parity, uk Latin allowlist, JSX `t()` literals | `(cd frontend && npm run check:i18n)` |
| `Test ID guard` | Duplicate `data-testid` + every `TestIds` value in `docs/dev/testing/test-ids.md` catalog | `(cd frontend && npm run check:test-ids)`; refresh catalog with `-- --write-catalog` |
| `OpenAPI drift` | Committed `docs/api/openapi.json` must match `ApiDoc`; `oasdiff breaking` vs base unless `VERSION` bumped | `./scripts/generate-openapi.sh` then `./scripts/check-openapi-drift.sh` |
| `OpenAPI Spectral` | Stoplight Spectral lint (`.spectral.yaml`): every operation has a summary; `ProblemDetails` + shared problem response components | `./scripts/check-openapi-spectral.sh` |
| `Bundle size budget` | Main `frontend/dist/assets/index-*.js` must not grow **>10%** vs base ref | `./scripts/check-bundle-size.sh` |
| `Accessibility smoke` | Playwright project `a11y-smoke`: axe on login/cases/runs/docs/settings; **serious/critical** fail (`color-contrast` disabled like other a11y specs) | `(cd e2e && npx playwright test --project=a11y-smoke)` with API + preview running |

Already required in **CI** (keep these names stable):

| Job name | Gate |
| --- | --- |
| `Backend (fmt, clippy, test)` | `cargo clippy --workspace --all-targets -- -D warnings` (+ fmt/tests) |
| `Frontend (lint, typecheck, test, build)` | `tsc --noEmit` via `npm run typecheck` (+ oxlint, vitest, test-ids, build) |

### Baselining / exceptions

| Gate | How to baseline |
| --- | --- |
| Hadolint | Add rule id + rationale to `.hadolint.yaml` `ignored` |
| i18n identical-to-en | Add key to `frontend/src/i18n/identical-allowlist.ts` |
| i18n uk Latin | Add token to `frontend/src/i18n/latin-allowlist.ts` |
| i18n JSX literals | Add regex to `frontend/scripts/i18n-literal-allowlist.txt` |
| Test ID catalog | `(cd frontend && npm run check:test-ids -- --write-catalog)` |
| OpenAPI drift | `./scripts/generate-openapi.sh` (commit `docs/api/openapi.json`) |
| OpenAPI breaking | Bump `VERSION` + workspace `version` in the same PR |
| Bundle size | Shrink the entry chunk, or intentionally exceed only with review (no silent raise of `BUNDLE_GROWTH_MAX` in CI) |
| cargo deny / gitleaks | Prefer fixes; justified ignores in `deny.toml` / `.gitleaks.toml` with review-by dates |

**Dependabot:** `.github/workflows/dependabot-auto-merge.yml` is intentionally unchanged. PR hygiene **skips** `dependabot[bot]` / `renovate[bot]`, so patch/minor auto-merge keeps working once the other required checks pass.

**Residual risk:** `Accessibility smoke` cold runs may exceed ~2 min (release API compile + Playwright browser install). Warm `rust-cache` / npm cache keeps it closer to the budget. Full Playwright in `ci.yml` remains the broad e2e gate.

### Doc links (offline)

The **Doc links (offline)** job runs [lychee](https://github.com/lycheeverse/lychee)
with `--offline` against root markdown (`README.md`, `AGENTS.md`,
`CONTRIBUTING.md`, `SECURITY.md`), `docs/**/*.md`, and `e2e/README.md`. It fails
on broken **relative** links only; external `http(s)` URLs are not fetched.
Config: `lychee.toml`. Local mirror:

```bash
cargo install lychee --locked   # once
./scripts/check-doc-links.sh
```

## Security workflow (detail)

| Job | Tool | Notes |
| --- | --- | --- |
| `rust-audit` | `cargo audit`, `cargo deny check` | Advisories + AGPL-compatible licenses + bans + sources. Config: `deny.toml`, `.cargo/audit.toml` |
| `npm-audit` | `npm audit --audit-level=high` | `frontend/` and `e2e/`. Low/moderate without fixes (e.g. `deepmerge` via `vite-plugin-pwa`) do not fail |
| `osv` | OSV-Scanner | Lockfiles → SARIF (Code Scanning). Soft on upload if GHAS unavailable |
| `trivy-fs` | Trivy filesystem | CRITICAL/HIGH vuln scanner (secrets via gitleaks); `.trivyignore`; SARIF |
| `trivy-images` | Trivy on built API/web/updater images | SARIF reports HIGH+CRITICAL; **gate fails on CRITICAL** (day-one ratchet) |
| `codeql` | GitHub CodeQL | `javascript-typescript` + `rust`, `security-extended` |
| `semgrep` | Semgrep community packs + `.semgrep.yml` | CI / Rust / JS / TS / OWASP |
| `gitleaks` | gitleaks-action | Config: `.gitleaks.toml` |

**Dependabot:** `.github/dependabot.yml` — weekly `cargo`, `npm` (frontend + e2e), `docker` (API/web/updater), `github-actions`.

**Dependabot auto-merge:** `.github/workflows/dependabot-auto-merge.yml` runs on `pull_request_target` for PRs authored by `dependabot[bot]`. It uses `dependabot/fetch-metadata` (SHA-pinned) and `gh pr merge --auto --squash` for safe bumps only: all `semver-patch`, plus `semver-minor` when the previous major is **≥ 1**. **0.x “minor” bumps are treated as breaking** (Dependabot still labels them `semver-minor`) and are never auto-merged; neither are semver-majors. Grouped updates use fetch-metadata’s highest `update-type` / version pair. The job does not check out PR code and only needs `contents: write` + `pull-requests: write`.

### Justified advisory ignores

Documented in `deny.toml` / `.cargo/audit.toml` with **review-by dates**:

| ID | Crate | Why ignored | Review by |
| --- | --- | --- | --- |
| RUSTSEC-2026-0387 | crypto-bigint (via jsonwebtoken) | No API-compatible jwt bump yet | 2027-05-01 |
| RUSTSEC-2023-0071 | rsa (via jsonwebtoken) | Marvin; jwt 11 still on rsa 0.9 | 2027-05-01 |
| RUSTSEC-2026-0187 | lopdf (via genpdf) | PDF **export** only; genpdf unmaintained | 2027-02-01 |
| RUSTSEC-2026-0459 | libsqlite3-sys | Unused (Postgres-only); `cargo audit` lockfile noise | 2027-02-01 |

**Fixed in the security PR wave:** `rmcp` ≥3.10 (RUSTSEC-2026-0462), `quick-xml` ≥0.41 (RUSTSEC-2026-0194/0195), npm `glob`/`minimatch` overrides under `vite-plugin-pwa`.

### Local security

```bash
cargo install cargo-audit cargo-deny --locked
./scripts/check-security.sh          # audit + deny + npm audit
./scripts/check-security.sh --quick  # Rust only
```

## Quality workflow

| Job | Tool | Notes |
| --- | --- | --- |
| `rust-quality` | `cargo machete`, `cargo llvm-cov`, clippy pedantic (advisory) | Lib coverage floors in `coverage-thresholds.toml` (lines≥12, functions≥14). Pedantic is `continue-on-error` with documented `-A` allows |
| `frontend-quality` | oxlint + ESLint + Prettier + knip + Vitest coverage | ESLint: typescript-eslint, react-hooks, jsx-a11y. Coverage floors in `frontend/vite.config.ts` (lines≥35, statements≥34, branches≥37, functions≥24) |

```bash
./scripts/check.sh            # fmt, clippy, seeds, migrations, lint, tsc, test-ids
./scripts/check.sh --full     # + cargo test + vitest
./scripts/check-quality.sh    # machete, eslint, prettier, knip, coverage
./scripts/check-quality.sh --quick
```

## API coverage

`API coverage (OpenAPI ↔ Axum)` is a required, per-operation gate:

1. `ApiDoc::openapi()` is enumerated as the contract inventory.
2. A source inventory of every `Router::route(...)` declaration detects routes
   that exist only in Axum. OpenAPI-only operations are detected in the other
   direction.
3. The assembled router receives every operation with `auth.required=true`.
   Protected operations must return `401`; public exceptions must be listed
   with a justification in `scripts/api-coverage-manifest.json`.
4. JSON request bodies receive malformed payload probes (`400`, `415`, or
   `422`). With `DATABASE_URL`, random identifiers also exercise the main
   not-found/forbidden path.
5. Every operation must declare a successful `2xx` response (or the `3xx`
   redirect that is the successful browser contract for SSO). Reports are uploaded
   as `api-coverage-report.json` and `api-coverage-summary.md`.

Run the same gate locally:

```bash
./scripts/check-api-coverage.sh
```

The DB-free inventory, drift, authentication, and public validation checks
always run. Authenticated validation, focused SSO alias coverage, and the
random-ID database sweep print explicit skips when `DATABASE_URL` is unset.

When adding or changing an endpoint, update the Axum registration and
`ApiDoc`/`#[utoipa::path]` together. Add a focused success/error integration
test for its business behavior. Only add a manifest exception when an error
class is genuinely inapplicable; include a concrete justification.

## Migrations

Required job **Migration integrity** always runs (no path filter). It hard-fails on
version collisions, edits to migrations that exist on `main`/the release tag,
failed fresh migrate verify, and a Squawk proof fixture. Squawk findings on real
migration diffs are **warnings** (annotations + `continue-on-error`). Details and
local commands: [migrations.md](migrations.md).

```bash
./scripts/check-migrations.sh
./scripts/lint-migrations-squawk.sh
./scripts/test-migration-lint.sh
DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk ./scripts/ci-migrations-verify.sh
```

## Upgrade path

Security/quality workflows must **not** change operator `.env`, Compose layout, or migrations. If a follow-up does, update `.env.example`, `install.sh`, and `./scripts/ci-env-upgrade-test.sh` in the **same** PR ([upgrade-path rule](../../.cursor/rules/upgrade-path.mdc)).
The Migration integrity job also runs `VULYK_UPGRADE_BASE_REF=v0.5.1` through
`scripts/ci-upgrade-test.sh` (in addition to the base→PR **Upgrade** job).
