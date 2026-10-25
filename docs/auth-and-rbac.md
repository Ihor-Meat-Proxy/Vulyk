# Authentication & RBAC (W1)

Vulyk supports local users, session cookies, short-lived JWTs, and personal API tokens for CI/MCP. Per-project roles control access; system admins manage users and global settings.

W9/W10 already introduced headless `api_tokens` for MCP. W1 extends those tokens with optional `user_id` / scopes / expiry and adds user sessions + JWT login. Bearer auth accepts **JWT**, **session secrets** (`vulyk_s_…`), or **API tokens** (`vulyk_at_…` and legacy `vulk_…`).

## Auth modes

| Mode | Env | Behavior |
|------|-----|----------|
| Open (default local/dev) | `VULYK_AUTH_REQUIRED` / `VULYK_REQUIRE_AUTH` unset or false | Anonymous CRUD for projects/suites/cases still works. Admin/user routes require auth. |
| Enforced | `VULYK_AUTH_REQUIRED=true` or `VULYK_REQUIRE_AUTH=1` | Project APIs require Bearer JWT, session cookie, or API token. Project lists are filtered by membership (system admins see all). |

## Credentials

1. **Bootstrap** (empty `users` table only): `POST /api/v1/auth/bootstrap` → first system admin + JWT + `Set-Cookie: vulyk_session=…`.
2. **Login**: `POST /api/v1/auth/login` → JWT + session cookie.
3. **Personal API tokens**: `POST /api/v1/tokens` → secret shown once (`secret` field). Use as `VULYK_API_TOKEN` for MCP/CLI.
4. **Logout**: `POST /api/v1/auth/logout`.

Password hashes use **Argon2id**. Session and API token secrets are stored as **SHA-256** hashes (`crypto::hash_token`).

### Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `VULYK_AUTH_REQUIRED` / `VULYK_REQUIRE_AUTH` | `false` | Enforce auth on project APIs |
| `VULYK_JWT_SECRET` | insecure dev default | HMAC secret for JWTs |
| `VULYK_JWT_TTL_HOURS` | `12` | Access token lifetime |
| `VULYK_SESSION_TTL_HOURS` | `168` | Session cookie lifetime |

## Roles

### System admin

`users.is_system_admin = true`. Manages users, integration placeholders, and bypasses project membership checks.

### Per-project roles (`memberships.role`)

| Role | Capabilities (W1) |
|------|-------------------|
| `viewer` | Read project + memberships |
| `tester` | Viewer + (future) execute runs / write results |
| `project_lead` | Tester + update project, manage memberships |
| `admin` | Project lead + delete project |

Creating a project while authenticated grants the creator `admin` membership.

## Admin UI

Admin panel tabs: **Users**, **Roles**, **Projects**, **Integrations** (placeholders for W6), **LLM providers** (W9 configs), **API tokens** (CI/MCP).

## Migration

`backend/migrations/20261025143000_auth_rbac.sql` — sessions, memberships, integration_settings, user admin columns, and alters `api_tokens` from W9/W10.
