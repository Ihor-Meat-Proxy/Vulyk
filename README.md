# Vulyk

Open-source **test management** system. *Vulyk* (вулик) is Ukrainian for *beehive*.

Phase 1 foundation plus LLM providers (W9) and a production MCP server (W10): Rust API (Axum + SQLx + OpenAPI), React UI, MCP/CLI, Docker Compose, and CI.

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
mcp/        MCP server (stdio + HTTP) — see docs/mcp.md
cli/        CLI / reporter skeleton (W4)
deploy/     Kubernetes manifests + Helm chart skeleton
docs/       Architecture, LLM, MCP setup
scripts/    install.sh
```

## Architecture

See **[docs/architecture.md](docs/architecture.md)** for the full product plan, ERD, plugin interfaces (importers, ALM, LLM, CI), MCP design, and parallel workstreams **W0–W11**.

- LLM providers & chat: **[docs/llm.md](docs/llm.md)**
- MCP setup (Cursor / Claude Desktop): **[docs/mcp.md](docs/mcp.md)**

## API (Phase 1 + W9/W10)

- `GET /health`, `GET /ready`
- `CRUD /api/v1/projects`
- `CRUD /api/v1/projects/{id}/suites`
- `CRUD /api/v1/projects/{id}/cases` (with steps; optional `?suite_id=` / `?q=` search)
- `CRUD /api/v1/projects/{id}/runs` (+ trigger, results ingest)
- Requirements / ALM (W6): `/requirements`, case links, `/integrations`, `/defects`
- Importers (W7): `GET /api/v1/importers`, `POST .../imports:preview`, `POST .../imports`
- `CRUD /api/v1/projects/{id}/llm/configs` + generate-cases / generate-code / ask
- `POST/GET/DELETE /api/v1/tokens` (MCP API tokens)
- Interactive docs: `/api/docs`
- Integration notes: [docs/integrations/](docs/integrations/)

## License

This project is licensed under the **GNU Affero General Public License v3.0** (see [LICENSE](LICENSE)).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
