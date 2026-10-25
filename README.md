# Vulyk

Open-source **test management** system. *Vulyk* (вулик) is Ukrainian for *beehive*.

Phase 1 delivers the monorepo foundation: Rust API (Axum + SQLx + OpenAPI), React UI shell, MCP/CLI stubs, Docker Compose, and CI.

## Quick start

### Docker Compose (recommended)

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

### Local development

```bash
./scripts/install.sh dev   # starts Postgres only
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api
cd frontend && npm install && npm run dev
```

## Repository layout

```
backend/    Rust API (Axum, Tokio, SQLx, utoipa)
frontend/   React + Vite + TypeScript UI
mcp/        MCP server skeleton (tools in workstream W10)
cli/        CLI / reporter skeleton (W4)
deploy/     Kubernetes manifests + Helm chart skeleton
docs/       Architecture and design
scripts/    install.sh
```

## Architecture

See **[docs/architecture.md](docs/architecture.md)** for the full product plan, ERD, plugin interfaces (importers, ALM, LLM, CI), MCP design, and parallel workstreams **W0–W11**.

## API (Phase 1)

- `GET /health`, `GET /ready`
- `CRUD /api/v1/projects`
- `CRUD /api/v1/projects/{id}/suites`
- `CRUD /api/v1/projects/{id}/cases` (with steps; optional `?suite_id=` / `?q=` search)
- Interactive docs: `/api/docs`

## License

This project is licensed under the **GNU Affero General Public License v3.0** (see [LICENSE](LICENSE)).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
