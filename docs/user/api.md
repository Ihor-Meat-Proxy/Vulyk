# API tokens and OpenAPI

Use the HTTP API for CI reporters, CLI, MCP, and custom integrations. Interactive docs are served by the API itself (self-hosted Scalar — no CDN; works offline / air-gapped).

## OpenAPI UI

Open the in-app docs at **`/api/docs`** (same origin as your Vulyk UI when proxied). Machine-readable schema: `/api/openapi.json`.

### Using Scalar

- **Search** — sidebar search, or Ctrl/⌘+K
- **Test Request** — runs against the same-origin server (no Scalar proxy)
- **Auth**
  1. Open **Auth** → `bearer_auth` → paste a JWT from login or a personal API token. Scalar sends `Authorization: Bearer …`
  2. If you are already signed in on this origin, the HttpOnly `vulyk_session` cookie is sent automatically with Test Request

## Conventions

- Base path: `/api/v1`
- Auth: session cookie (`vulyk_session`) or `Authorization: Bearer <jwt|api_token|ci_token>`
- Errors: [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) `application/problem+json` (see below)
- Health: `GET /health`, `GET /ready`

## Errors (RFC 9457 Problem Details)

Failed requests return `Content-Type: application/problem+json` with:

| Field | Meaning |
|-------|---------|
| `type` | URI identifying the problem type (`urn:vulyk:api:problem:{code}`) |
| `title` | Short summary for the status class (e.g. `Not Found`) |
| `status` | HTTP status code |
| `detail` | Human-readable explanation for this occurrence |
| `code` | Stable machine-readable code (`not_found`, `unauthorized`, …) |
| `reset_at` | Optional UTC timestamp (git host rate limits) |
| `error` | **Transitional** nested `{ code, message }` for older clients |

Example (`404`):

```json
{
  "type": "urn:vulyk:api:problem:not_found",
  "title": "Not Found",
  "status": 404,
  "detail": "not found: project",
  "code": "not_found",
  "error": { "code": "not_found", "message": "not found: project" }
}
```

Prefer `detail` / `code` in new clients. The nested `error` object remains for one
upgrade cycle so existing parsers keep working. Shared OpenAPI response components
(`ProblemBadRequest`, `ProblemUnauthorized`, `ProblemForbidden`, `ProblemNotFound`)
document the schema with examples in `/api/docs`.

## Token types

| Prefix | Kind | Where to create | Scope |
|--------|------|-----------------|-------|
| `vulyk_at_…` | Personal API token | Gear → My settings → Tokens, or Admin → Tokens | One project + allowlisted permissions, intersected with the owner’s RBAC. Secret shown once |
| `vulyk_ci_…` | Project CI token | Settings → Project CI tokens | Open / upload / close reporting for one project only |
| JWT | Login access token | `POST /api/v1/auth/login` | Short-lived (`VULYK_JWT_TTL_HOURS`, default 12) |

Tokens with no bound user, no project, or empty permissions are rejected (`401`). Prefer CI tokens in pipelines; prefer personal tokens for CLI/MCP as yourself. After upgrading to scoped tokens, recreate `VULYK_API_TOKEN` (legacy unscoped tokens are revoked).

### Create a personal token

1. Sign in.
2. Open gear → **My settings** (`/settings`) → Tokens (or **Admin → Tokens**).
3. Choose a **project** and the **permissions** the token may use.
4. Create a token, copy `vulyk_at_…` once, store it as `VULYK_API_TOKEN`.

### Create a project CI token

1. Sign in as Admin or Test Automation Engineer.
2. Open **Settings → Project CI tokens** (`/settings#ci-tokens`).
3. Choose the project → **Create token** → name it → copy `vulyk_ci_…`.

## Example: authenticated request

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_at_…   # or vulyk_ci_… for CI routes

curl -sS -H "Authorization: Bearer $VULYK_API_TOKEN" \
  "$VULYK_API_URL/api/v1/projects"
```

CI open/upload/close helpers: `scripts/vulyk-upload.sh` and [.github/actions/vulyk-report](cicd.md#github-actions). See also [CI/CD](cicd.md).

## Major surfaces

| Area | Paths |
|------|-------|
| Auth | `/api/v1/auth/*` |
| Projects / folders / cases | `/api/v1/projects…`, `/api/v1/cases/by-key/{key}` |
| Runs / ingest | `/api/v1/projects/{id}/runs`, `/ingest`, `/api/v1/runs/{id}/ingest` |
| CI reporting | `/api/v1/projects/{id}/ci/runs`, project CI tokens |
| Outbound webhooks | `/api/v1/projects/{id}/outbound-webhooks` (tag `outbound-webhooks`) — see [outbound webhooks](../guides/outbound-webhooks.md) |
| Executions | `/api/v1/projects/{id}/executions/…` |
| Reports | `/api/v1/projects/{id}/reports/*` |
| Requirements | `/requirements` |
| Importers | `/api/v1/importers`, `/imports` |
| LLM | `/api/v1/projects/{id}/llm/*` |
| Tokens | `/api/v1/tokens` |
| Admin updates / backups | `/api/v1/admin/updates*`, `/api/v1/admin/backups/*` |

For Try-it examples and full schemas, use `/api/docs`.
