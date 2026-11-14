# CI checks

Index of automated gates beyond the narrative in [ci-failures.md](ci-failures.md).
Local mirrors live in `./scripts/check.sh` and `./scripts/check-security.sh`.

## Workflows

| Workflow | File | When | Role |
| --- | --- | --- | --- |
| **CI** | `.github/workflows/ci.yml` | PR + push `main` | Required product gates (fmt, clippy, tests, migrations, upgrade, frontend, Docker, Playwright, …) |
| **Security** | `.github/workflows/security.yml` | PR + push `main` + **weekly** Monday | CVE/deps, containers, SAST, secrets |
| **Release** | `.github/workflows/release.yml` | tags / release | Publish images |

Lighthouse remains **advisory** (`continue-on-error`) inside `ci.yml` — see [ci-failures.md](ci-failures.md).

## Security workflow (detail)

| Job | Tool | Notes |
| --- | --- | --- |
| `rust-audit` | `cargo audit`, `cargo deny check` | Advisories + AGPL-compatible licenses + bans + sources. Config: `deny.toml`, `.cargo/audit.toml` |
| `npm-audit` | `npm audit --audit-level=high` | `frontend/` and `e2e/`. Low/moderate without fixes (e.g. `deepmerge` via `vite-plugin-pwa`) do not fail |
| `osv` | OSV-Scanner | Lockfiles → SARIF (Code Scanning). Soft on upload if GHAS unavailable |
| `trivy-fs` | Trivy filesystem | CRITICAL/HIGH, unfixed ignored; SARIF upload |
| `trivy-images` | Trivy on built API/web/updater images | Same severity policy; SARIF per image |
| `codeql` | GitHub CodeQL | `javascript-typescript` + `rust`, `security-extended` |
| `semgrep` | Semgrep community packs + `.semgrep.yml` | CI / Rust / JS / TS / OWASP |
| `gitleaks` | gitleaks-action | Config: `.gitleaks.toml` |

**Dependabot:** `.github/dependabot.yml` — weekly `cargo`, `npm` (frontend + e2e), `docker` (API/web/updater), `github-actions`.

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

## Code quality & API coverage

Additional gates (coverage thresholds, knip, API route coverage) are documented as they land in follow-up PRs. Existing quality bar today:

```bash
./scripts/check.sh        # fmt, clippy, seeds, migrations, lint, tsc, test-ids
./scripts/check.sh --full # + cargo test + vitest
```

## Upgrade path

Security/quality workflows must **not** change operator `.env`, Compose layout, or migrations. If a follow-up does, update `.env.example`, `install.sh`, and `./scripts/ci-env-upgrade-test.sh` in the **same** PR ([upgrade-path rule](../.cursor/rules/upgrade-path.mdc)).
