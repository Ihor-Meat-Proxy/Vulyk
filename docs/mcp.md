# MCP server (W10)

`vulyk-mcp` is a thin [Model Context Protocol](https://modelcontextprotocol.io/) server over the Vulyk HTTP API. It does **not** open a second database path.

## Auth

1. Create a token: `POST /api/v1/tokens` `{ "name": "cursor" }` (UI: **Admin → API tokens**).  
2. Export:

```bash
export VULYK_API_URL=http://localhost:8080
export VULYK_API_TOKEN=vulk_…   # shown once at creation
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
| `search_cases` | List/search cases (`q`, optional `suite_id`) |
| `get_case` | Case + steps |
| `create_case` / `update_case` | Mutate cases |
| `list_runs` / `create_run` / `update_run` | Runs |
| `trigger_run` | Mark run `running` |
| `list_results` / `ingest_results` | Results |
| `ask_tests` | LLM Q&A over project tests |

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
        "VULYK_API_TOKEN": "vulk_…"
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
        "VULYK_API_TOKEN": "vulk_…"
      }
    }
  }
}
```

### HTTP clients

Connect to `http://127.0.0.1:8090/mcp` after `vulyk-mcp http`. Use the same API token only for the Vulyk backend — the MCP HTTP transport itself is intended for trusted local use; put it behind your usual network controls in production.
