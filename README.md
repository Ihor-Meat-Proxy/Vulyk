# Vulyk

**Self-hosted test management that stays under your control.**

*Vulyk* (вулик) is Ukrainian for *beehive*.

Vulyk is an open-source test management system for QA and engineering teams who want cases, plans, runs, CI results, and requirement coverage without shipping that data to a vendor SaaS. You run it on your own infrastructure—Docker Compose or Helm, Postgres, your network—under the AGPL. No seat meter, no proprietary export trap, no “your tests live in our cloud” compromise.

## Why Vulyk

- **Yours to host, yours to keep.** Compose and Helm deploy paths, Postgres as the system of record, verified backups and rollback-safe upgrades—operators stay in control of data and downtime.
- **Truly open source.** Licensed under **AGPL-3.0**. Read the code, audit the security model, fork if you need to. No closed core for the features you actually use.
- **Full TMS, not a thin tracker.** Nested folders, auto-allocated `PREFIX-N` case keys, steps, versioning, plans and runs with step results, requirements with coverage, option lists and custom fields, case PDF export.
- **CI that reports in—and runs out.** Ingest JUnit, Playwright, pytest, or Vulyk JSON from GitHub Actions and GitLab CI; multi-repo tokens and webhooks; **Run in CI** dispatches workflows and attaches results back to the same run.
- **Leave the vendor behind.** Import from TestRail, Xray, Zephyr, Squash TM, TestLink, qTest, Qase, Kiwi TCMS, or CSV—preview mappings before you commit.
- **Built for humans and agents.** React UI (PWA with offline result queue), OpenAPI at `/api/docs`, CLI reporter, and an **MCP** server so Cursor, Claude, and similar tools work against the same HTTP API. Optional multi-provider LLM for generate and chat.
- **Auth that fits real orgs.** Email/password or SSO (Google, Microsoft Entra, GitHub, GitLab, Nextcloud, generic OIDC), RBAC, personal API tokens, fail-closed production defaults.
- **Reports you can drill into.** Dashboards, pass/fail trends, requirement coverage, flaky detection, CSV/PDF export—deep-linkable run reports, not a dead summary page.
- **Ready for global teams.** UI locales: English, Українська, Français, Español, Deutsch. Appearance themes including high contrast.

## Get started

```bash
./scripts/install.sh compose --yes
```

Then open the UI at http://localhost:5173. Full install options (prod Compose, Helm, local Rust/Node), upgrades, and docs are below.

---

## Quick start

### Docker Compose (local / demos)

```bash
./scripts/install.sh compose --yes
# or: docker compose up --build
```

`./scripts/install.sh compose --yes` builds, starts, and runs first-time setup automatically on a fresh database. `./scripts/install.sh setup --yes` runs only the setup step (admin password, app language, timezone) on an already running install — e.g. after plain `docker compose up --build`. `--yes` means non-interactive (flags/env vars like `VULYK_ADMIN_PASSWORD`, or defaults).

| Service | URL |
|---------|-----|
| UI | http://localhost:5173 |
| API | http://localhost:8080 |
| OpenAPI (Scalar) | http://localhost:8080/api/docs |
| Health | http://localhost:8080/health |

**Seeded local credentials** (when `VULYK_SEED_DEMO=1`, the Compose default):

| Account | Email | Password |
|---------|-------|----------|
| Admin | `admin@vulyk.local` | Random ≥16 chars in `.env` as `VULYK_ADMIN_PASSWORD` (forced change on first login when generated) |
| Demo | `user@vulyk.local` | `VULYK_DEMO_PASSWORD` in `.env` (default `vulyk-demo-user-1`) |

`./scripts/install.sh` generates a ~24-character admin password when none is provided (never a short `admin` default) and stores it in `.env` without printing the value. Password policy: `VULYK_PASSWORD_MIN_LENGTH` (default **16**, floor **12**), enforced server-side; the UI loads the same policy from `/api/v1/auth/status`.

Env vars: see [`.env.example`](.env.example) and [docs/guides/first-run-setup.md](docs/guides/first-run-setup.md). UI languages: **en**, **uk**, **fr**, **es**, **de** (`--locale` / `VULYK_DEFAULT_LOCALE`; switch anytime via the header globe menu).

### Production Compose (auth required)

```bash
./scripts/install.sh prod --yes \
  --admin-email=admin@company.example \
  --admin-password='Str0ngPass'
```

Generates `.env.prod` with random secrets, enables healthchecks/volumes, sets `VULYK_AUTH_REQUIRED=true` and `VULYK_REGISTRATION_MODE=disabled`. See [docs/admin-guide.md](docs/admin-guide.md).

### Local development

```bash
./scripts/install.sh check   # toolchain (Linux / macOS / WSL2)
./scripts/install.sh dev --yes   # Postgres + .env first-run settings
set -a && source .env && set +a
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api
# or: cargo run -p vulyk-api -- setup --yes
cd frontend && npm install && npm run dev
```

### Helm

```bash
./scripts/install.sh helm-template
helm upgrade --install vulyk deploy/helm/vulyk --namespace vulyk --create-namespace \
  --set secrets.create=true \
  --set setup.adminPassword=change-me-admin12
```

Post-install Job runs `vulyk-api setup --yes`. Interactive: `kubectl exec -it deploy/…-api -- vulyk-api setup`.

## Releases

Install a published version from GHCR (no local Rust/Node build):

```bash
./scripts/install.sh compose --yes --version 0.3.0
./scripts/install.sh prod --yes --version 0.3.0
```

Maintainers: bump `VERSION`, tag `vX.Y.Z`, and the Release workflow publishes GHCR
images + GitHub Release artifacts. Guide: [docs/guides/releasing.md](docs/guides/releasing.md).

## Upgrading

Keep the Postgres volume; never run `docker compose down -v` (the `-v` flag deletes data).

```bash
./scripts/install.sh upgrade --yes                    # verified backup → pull/rebuild → migrate → health
./scripts/install.sh upgrade --yes --version 0.3.0    # published GHCR images
./scripts/install.sh upgrade --yes --to=v0.3.0        # git tag + source rebuild
./scripts/install.sh backup                           # verified bundle (DB + env/secrets)
./scripts/install.sh restore ./backups/vulyk-…/ --yes
```

Upgrades **never** regenerate `.env` / secrets / admin password; on failure they
auto-roll back to previous images + DB dump. Guarantees:
[docs/guides/upgrade.md](docs/guides/upgrade.md#data-safety-guarantees).

Helm runs `vulyk-api migrate run` as a pre-upgrade Job.

## Documentation

Full index: **[docs/README.md](docs/README.md)**.

| Doc | Description |
|-----|-------------|
| [docs/architecture.md](docs/architecture.md) | Modules, ERD, security summary |
| [docs/user-guide.md](docs/user-guide.md) | End-user workflows (cases, PDF, custom fields, Updates) |
| [docs/admin-guide.md](docs/admin-guide.md) | Deploy, env/security, lists/fields, backups, Helm |
| [docs/guides/releasing.md](docs/guides/releasing.md) | Version bump, tags, GHCR, install `--version` |
| [docs/guides/upgrade.md](docs/guides/upgrade.md) | Safe upgrades, verified backups, rollback |
| [docs/guides/in-app-updates.md](docs/guides/in-app-updates.md) | Admin Updates tab + updater sidecar |
| [docs/guides/backup.md](docs/guides/backup.md) | Content backup / restore (ZIP destinations) |
| [docs/api.md](docs/api.md) | API overview + OpenAPI link |
| [docs/auth-and-rbac.md](docs/auth-and-rbac.md) | Auth, JWT fail-closed, roles, tokens |
| [docs/guides/ci-integration.md](docs/guides/ci-integration.md) | Preferred CI reporting (multi-repo) |
| [docs/guides/ingest-and-ci.md](docs/guides/ingest-and-ci.md) | Formats, dispatch, webhooks |
| [docs/automation-repo.md](docs/automation-repo.md) | Automation Git repos for Generate |
| [docs/mcp.md](docs/mcp.md) | MCP server (Cursor / Claude) |
| [docs/llm.md](docs/llm.md) | LLM providers, generate, chat |
| [AGENTS.md](AGENTS.md) / [CONTRIBUTING.md](CONTRIBUTING.md) | PR guardrails, seeds, test ids |
| [docs/ci-failures.md](docs/ci-failures.md) | CI failure catalog |
| [docs/testing/test-ids.md](docs/testing/test-ids.md) | Playwright `data-testid` conventions |
| [docs/security/audit-2026-10.md](docs/security/audit-2026-10.md) | Security audit (Oct 2026) + fix PRs |
| [docs/quality/code-quality-2026-10.md](docs/quality/code-quality-2026-10.md) | Code quality audit (Oct 2026) |
| [docs/review/code-review-2026-10.md](docs/review/code-review-2026-10.md) | Senior code review (Oct 2026) |

## Repository layout

```
backend/    Rust API (Axum, Tokio, SQLx, utoipa)
frontend/   React + Vite + TypeScript UI
mcp/        MCP server (stdio + HTTP)
cli/        CLI / reporter
e2e/        Playwright end-to-end tests
deploy/     Kubernetes manifests + Helm chart
docs/       Architecture, guides, integrations
scripts/    install.sh, perf-smoke.sh, helpers
```

## Stack & capabilities

Rust API (Axum + SQLx + OpenAPI), React UI, MCP server, CLI reporter, Docker/Helm deploy.

- Auth / RBAC, projects (optional key), nested folders, auto `PREFIX-N` case keys
- Option lists, custom fields, config export/import; case PDF export
- Plans, runs, step results; automation executions (campaigns / iterations / pipeline ingest)
- Ingest: JUnit / Playwright / pytest / Vulyk JSON; multi-repo CI tokens + webhooks (secrets required)
- Requirements / ALM; importers; reports (dashboard, trends, coverage, flaky, CSV/PDF)
- Content backups; Admin Updates / rollback; LLM generate/chat; MCP; personal API tokens
- Locales: `en` / `uk` / `fr` / `es` / `de` — Interactive docs: `/api/docs`

## Testing

```bash
# Backend (needs DATABASE_URL)
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo test --workspace

# Frontend unit
cd frontend && npm ci && npm test && npm run build

# Playwright e2e (API + UI already running, or use CI job)
cd e2e && npm ci && npx playwright install chromium && npm test

# Perf smoke (~100k cases)
./scripts/install.sh perf-smoke
```

## License

**GNU Affero General Public License v3.0** — see [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
