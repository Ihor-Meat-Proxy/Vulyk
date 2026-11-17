# MCP server (W10)

`vulyk-mcp` is a thin [Model Context Protocol](https://modelcontextprotocol.io/) server over the Vulyk HTTP API. It does **not** open a second database path.

## Auth

1. Create a token: `POST /api/v1/tokens` `{ "name": "cursor" }` (UI: **Admin → API tokens**).  
2. Export:

```bash
export VULYK_API_URL=http://localhost:8080
export VULYK_API_TOKEN=vulyk_at_…   # shown once at creation (legacy vulk_… still accepted)
```

The MCP process sends `Authorization: Bearer …` on every call. Invalid tokens are rejected by the API.

## Transports

```bash
# stdio (default) — Claude Desktop, Cursor, most local clients
cargo run -p vulyk-mcp
# or: cargo run -p vulyk-mcp -- stdio

# Streamable HTTP
cargo run -p vulyk-mcp -- http
# listens on VULYK_MCP_BIND (default 127.0.0.1:8090), path /mcp
```

## Tools

| Tool | Purpose |
|------|---------|
| `list_projects` / `get_project` | Discover projects (no out-of-band UUID) |
| `search_cases` | List/search cases (`q`, suite, tag, status, priority, sort, pagination) |
| `get_case` / `create_case` / `update_case` | Case CRUD |
| `list_suites` | Folders with case counts |
| `list_runs` / `get_run` / `create_run` / `update_run` | Runs (`get_run` accepts UUID, slug, or display ID) |
| `trigger_run` | Mark run **status** `running` only — does **not** start CI |
| `list_ci_configs` / `dispatch_ci` / `get_dispatch` | Start Jenkins/Forgejo/Gitea/Woodpecker/GitHub/GitLab pipelines; `external_url` is the build link |
| `generate_cases` / `generate_code` / `ask_tests` / `chat` / `chat_confirm` | LLM assist |
| `list_results` / `ingest_results` / `list_case_results` | Results |
| `list_repos` / `get_repo` / `create_repo` | Automation/source repositories (multi-repo) |
| `list_campaigns` / `get_campaign` / `create_campaign` | Campaigns (`PREFIX-C-n`, never reused) |
| `list_iterations` / `get_iteration` / `create_iteration` | Iterations (`PREFIX-IT-n`, never reused) |
| `list_requirements` / `get_requirement` / `create_requirement` / `link_requirement` | Requirements |
| `report_dashboard` / `report_run_summary` / `report_run_detail` / `report_trends` / `report_coverage` / `report_flaky` | Reports |

Admin, backups, and webhooks stay off the default tool surface.

## OpenAPI coverage contract

Every operation in [`docs/api/openapi.json`](api/openapi.json) must either:

1. Be mapped to an MCP tool in [`mcp/openapi-coverage.json`](../mcp/openapi-coverage.json), or  
2. Appear in that file’s `excluded_operations` with a justification.

`excluded_tags` always includes `admin`, `backups`, and `webhooks`. CI / `./scripts/check.sh` runs `cargo test -p vulyk-mcp --test openapi_coverage` so new API routes fail the gate until they get a tool or an explicit exclusion.

## Client setup

### Cursor

Add to MCP settings (`.cursor/mcp.json` or Cursor Settings → MCP):

```json
{
  "mcpServers": {
    "vulyk": {
      "command": "cargo",
      "args": ["run", "-q", "-p", "vulyk-mcp"],
      "cwd": "/absolute/path/to/Vulyk",
      "env": {
        "VULYK_API_URL": "http://localhost:8080",
        "VULYK_API_TOKEN": "vulyk_at_…"
      }
    }
  }
}
```

Or point `command` at a built binary: `target/release/vulyk-mcp`.

### Claude Desktop

Edit `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "vulyk": {
      "command": "/absolute/path/to/vulyk-mcp",
      "env": {
        "VULYK_API_URL": "http://localhost:8080",
        "VULYK_API_TOKEN": "vulyk_at_…"
      }
    }
  }
}
```

### HTTP clients

Connect to `http://127.0.0.1:8090/mcp` after `vulyk-mcp http`. Use the same API token only for the Vulyk backend — the MCP HTTP transport itself is intended for trusted local use; put it behind your usual network controls in production.
