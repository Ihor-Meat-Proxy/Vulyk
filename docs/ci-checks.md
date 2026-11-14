# CI checks

Index of automated gates beyond the narrative in [ci-failures.md](ci-failures.md).
Local mirrors live in `./scripts/check.sh`, `./scripts/check-security.sh`, and
`./scripts/check-api-coverage.sh`.

## Workflows

| Workflow | File | When | Role |
| --- | --- | --- | --- |
| **CI** | `.github/workflows/ci.yml` | PR + push `main` | Required product gates (fmt, clippy, tests, API coverage, migrations, upgrade, frontend, Docker, Playwright, …) |
| **Security** | `.github/workflows/security.yml` | PR + push `main` + **weekly** Monday | CVE/deps, containers, SAST, secrets |
| **Dependabot auto-merge** | `.github/workflows/dependabot-auto-merge.yml` | `pull_request_target` (Dependabot only) | Enables squash auto-merge for **patch/minor** (incl. grouped Actions updates); majors stay manual |
| **Release** | `.github/workflows/release.yml` | tags / release | Publish images |

Lighthouse remains **advisory** (`continue-on-error`) inside `ci.yml` — see [ci-failures.md](ci-failures.md).

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

**Dependabot auto-merge:** `.github/workflows/dependabot-auto-merge.yml` runs on `pull_request_target` for PRs authored by `dependabot[bot]`. It uses `dependabot/fetch-metadata` (SHA-pinned) and `gh pr merge --auto --squash` when `update-type` is `semver-patch` or `semver-minor` (grouped github-actions updates included when the highest bump is still patch/minor). **Semver-major bumps are never auto-merged.** The job does not check out PR code and only needs `contents: write` + `pull-requests: write`.

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

## Upgrade path

Security/quality workflows must **not** change operator `.env`, Compose layout, or migrations. If a follow-up does, update `.env.example`, `install.sh`, and `./scripts/ci-env-upgrade-test.sh` in the **same** PR ([upgrade-path rule](../.cursor/rules/upgrade-path.mdc)).
