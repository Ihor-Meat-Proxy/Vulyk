# Vulyk

Open-source **test management** system. *Vulyk* (вулик) is Ukrainian for *beehive*.

Rust API (Axum + SQLx + OpenAPI), React UI, MCP server, CLI reporter, Docker/Helm deploy, and CI.

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
| Admin | `admin@vulyk.local` | `admin` (must change on first login) |
| Demo | `user@vulyk.local` | `user` |

Env vars: see [`.env.example`](.env.example) and [docs/guides/first-run-setup.md](docs/guides/first-run-setup.md).

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
  --set setup.adminPassword=change-me-admin1
```

Post-install Job runs `vulyk-api setup --yes`. Interactive: `kubectl exec -it deploy/…-api -- vulyk-api setup`.

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/architecture.md](docs/architecture.md) | Product plan, ERD, workstreams W0–W11 |
| [docs/user-guide.md](docs/user-guide.md) | End-user workflows |
| [docs/admin-guide.md](docs/admin-guide.md) | Deploy, backups, perf smoke, Helm |
| [docs/api.md](docs/api.md) | API overview + OpenAPI link |
| [docs/auth-and-rbac.md](docs/auth-and-rbac.md) | Auth, roles, registration, seeding |
| [docs/guides/first-run-setup.md](docs/guides/first-run-setup.md) | Compose / Helm / CLI / wizard |
| [docs/guides/sso.md](docs/guides/sso.md) | Google, Entra, GitHub, GitLab, Nextcloud |
| [docs/integrations/](docs/integrations/) | OpenProject, Jira, importers |
| [docs/mcp.md](docs/mcp.md) | MCP server (Cursor / Claude) |
| [docs/llm.md](docs/llm.md) | LLM providers |
| [docs/guides/ingest-and-ci.md](docs/guides/ingest-and-ci.md) | JUnit ingest & CI triggers |
| [docs/guides/pwa-and-mobile.md](docs/guides/pwa-and-mobile.md) | PWA install, offline queue, mobile UX |

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

## API highlights

- Auth / RBAC, projects, suites, cases (FTS, tags, custom fields, versions)
- Plans, runs, step results, attachments
- Ingest: JUnit + Vulyk JSON, bulk results, job worker
- CI: GitHub/GitLab dispatch + webhooks
- Requirements / ALM (OpenProject, Jira Cloud/DC)
- Importers: TestLink, TestRail, Xray, Zephyr, Squash, qTest, Qase, Kiwi, CSV
- **Reports (W8):** dashboard, trends, requirement coverage, flaky detection, CSV/PDF export
- LLM generate/chat; MCP tools; personal API tokens
- Interactive docs: `/api/docs`

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
