# Vulyk

Open-source **test management** system. *Vulyk* (вулик) is Ukrainian for *beehive*.

Rust API (Axum + SQLx + OpenAPI), React UI, MCP server, CLI reporter, Docker/Helm deploy, and CI.

## Quick start

### Docker Compose (local / demos)

```bash
./scripts/install.sh compose
# or: docker compose up --build
```

| Service | URL |
|---------|-----|
| UI | http://localhost:5173 |
| API | http://localhost:8080 |
| OpenAPI (Scalar) | http://localhost:8080/api/docs |
| Health | http://localhost:8080/health |

### Production Compose (auth required)

```bash
./scripts/install.sh prod
```

Generates `.env.prod` with random secrets, enables healthchecks/volumes, and sets `VULYK_AUTH_REQUIRED=true`. See [docs/admin-guide.md](docs/admin-guide.md).

### Local development

```bash
./scripts/install.sh check   # toolchain (Linux / macOS / WSL2)
./scripts/install.sh dev     # Postgres only
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api
cd frontend && npm install && npm run dev
```

### Helm

```bash
./scripts/install.sh helm-template
helm upgrade --install vulyk deploy/helm/vulyk --namespace vulyk --create-namespace \
  --set secrets.create=true
```

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/architecture.md](docs/architecture.md) | Product plan, ERD, workstreams W0–W11 |
| [docs/user-guide.md](docs/user-guide.md) | End-user workflows |
| [docs/admin-guide.md](docs/admin-guide.md) | Deploy, backups, perf smoke, Helm |
| [docs/api.md](docs/api.md) | API overview + OpenAPI link |
| [docs/auth-and-rbac.md](docs/auth-and-rbac.md) | Auth & roles |
| [docs/integrations/](docs/integrations/) | OpenProject, Jira, importers |
| [docs/mcp.md](docs/mcp.md) | MCP server (Cursor / Claude) |
| [docs/llm.md](docs/llm.md) | LLM providers |
| [docs/guides/ingest-and-ci.md](docs/guides/ingest-and-ci.md) | JUnit ingest & CI triggers |

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
