# vulyk-mcp

Production MCP server for Vulyk (workstream **W10**). Tools call `/api/v1` with `VULYK_API_TOKEN`.

```bash
export VULYK_API_URL=http://localhost:8080
export VULYK_API_TOKEN=vulyk_at_…
cargo run -p vulyk-mcp          # stdio
cargo run -p vulyk-mcp -- http  # http://127.0.0.1:8090/mcp
```

See **[docs/mcp.md](../docs/mcp.md)** for Claude Desktop, Cursor, and tool reference.
