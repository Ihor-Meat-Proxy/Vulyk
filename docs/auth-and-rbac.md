# Authentication & RBAC

Vulyk supports local users, session cookies, short-lived JWTs, personal API tokens (CI/MCP), and OAuth2/OIDC SSO. Access is gated by **system admin** flags and five **per-project roles**.

Bearer auth accepts **JWT**, **session secrets** (`vulyk_s_…`), or **API tokens** (`vulyk_at_…` and legacy `vulk_…`).

## Auth modes

| Mode | Env | Behavior |
|------|-----|----------|
| Open (default local/dev) | `VULYK_AUTH_REQUIRED` / `VULYK_REQUIRE_AUTH` unset or false | Anonymous CRUD for projects/suites/cases still works. Admin/user routes require auth. |
| Enforced | `VULYK_AUTH_REQUIRED=true` or `VULYK_REQUIRE_AUTH=1` | Project APIs require Bearer JWT, session cookie, or API token. Project lists are filtered by membership (system admins see all). |

## Credentials & first-run

1. **Env seed** (preferred for Compose/Helm): set `VULYK_ADMIN_EMAIL` + `VULYK_ADMIN_PASSWORD` — on API boot, creates the system admin and marks setup complete. See [guides/first-run-setup.md](guides/first-run-setup.md).
2. **CLI**: `vulyk-api setup` (interactive or `--yes` with flags).
3. **Web wizard**: when setup is incomplete, use the one-time setup token printed in API logs (`vulyk_setup_…`).
4. **Login**: `POST /api/v1/auth/login` → JWT + session cookie.
5. **Register**: subject to [registration modes](#registration-modes).
6. **Personal API tokens**: `POST /api/v1/tokens` → secret shown once. Use as `VULYK_API_TOKEN` for MCP/CLI.
7. **Logout**: `POST /api/v1/auth/logout`.
8. **SSO**: see [guides/sso.md](guides/sso.md).

Password hashes use **Argon2id**. Session and API token secrets are stored as **SHA-256** hashes.

### Password policy

Single shared server policy (`VULYK_PASSWORD_MIN_LENGTH`, default **16**, floor **12**; letter + digit required) used by setup CLI, install.sh-provided passwords, bootstrap/register, user create/admin reset, change-password, and SSO local-password fallback. Clear `400` validation errors on reject. The same policy is published on `GET /api/v1/auth/status` as `password_policy` so the UI never drifts.

**Login** only verifies the Argon2 hash (any existing length). After a successful login, if the plaintext no longer meets policy, the API sets `must_change_password=true`. Until the password is changed, **all other API routes are blocked server-side** (403 `password_change_required`); only status/me/change-password/logout remain allowed. Login attempts are rate-limited with exponential backoff.

### Force password change

Seeded admins with `VULYK_ADMIN_MUST_CHANGE=1` (set by `install.sh` when it generates a password), or any password below policy, get `must_change_password=true`. Existing short/legacy passwords can still sign in, then must change to a policy-compliant password.

### Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `VULYK_AUTH_REQUIRED` / `VULYK_REQUIRE_AUTH` | `false` | Enforce auth on project APIs |
| `VULYK_JWT_SECRET` | insecure dev default only when auth is **off** | HMAC secret for JWTs. **Fail-closed:** when auth is required, unset/empty/`vulyk-dev-jwt-secret-change-me` panics at boot — set a strong secret in prod |
| `VULYK_JWT_TTL_HOURS` | `12` | Access token lifetime |
| `VULYK_SESSION_TTL_HOURS` | `168` | Session cookie lifetime |
| `VULYK_COOKIE_SECURE` | auto | When `1`/`true`, session cookie sets `Secure`. When unset, Secure follows HTTPS/`VULYK_BASE_URL` |
| `VULYK_ADMIN_EMAIL` | `admin@vulyk.local` | Seeded / setup admin email |
| `VULYK_ADMIN_PASSWORD` | (unset) | When set, seeds admin and completes setup |
| `VULYK_ADMIN_DISPLAY_NAME` | `Administrator` | Admin display name |
| `VULYK_ADMIN_MUST_CHANGE` | `false` | Force password change after seed (install.sh sets for generated passwords) |
| `VULYK_PASSWORD_MIN_LENGTH` | `16` | Shared min length (floor 12) |
| `VULYK_SEED_DEMO` | `0` | Seed demo user `user@vulyk.local` |
| `VULYK_DEMO_PASSWORD` | `vulyk-demo-user-1` | Demo user password (must meet policy) |
| `VULYK_INSTANCE_NAME` | `Vulyk` | Instance display name |
| `VULYK_DEFAULT_LOCALE` | `en` | Instance default locale (`en` / `uk` / `fr` / `es` / `de`); users override via globe menu (`users.locale` + `localStorage`) |
| `VULYK_DEFAULT_TIMEZONE` | `UTC` | Default timezone |
| `VULYK_REGISTRATION_MODE` | `disabled` | `open` \| `invite` \| `disabled` |
| `VULYK_BASE_URL` | (empty) | Public API/UI base URL (SSO callbacks) |
| `VULYK_SETUP_TOKEN` | (unset) | Optional one-time token for headless setup |

## Registration modes

| Mode | Behavior |
|------|----------|
| `open` | Anyone can `POST /api/v1/auth/register` (optional email-domain allowlist). |
| `invite` | Registration requires a valid invite token; admins create invites. |
| `disabled` | Self-registration off. New users via admin create, SSO JIT (if allowed), or invites only as configured. |

Production Compose/Helm default to **`disabled`**. Local Compose defaults to **`open`**.

## Roles

### System admin

`users.is_system_admin = true`. Grants all project permissions plus system-only capabilities:

| Permission | Capability |
|------------|------------|
| `manage_users` | Create/disable users, reset passwords |
| `manage_system_settings` | Instance name, locale, registration mode, … |
| `manage_sso` | Configure OAuth/OIDC providers |
| `manage_global_integrations` | Global ALM / CI / integration settings |

System admins bypass project membership checks.

### Per-project roles (five)

Creating a project while authenticated grants the creator **`admin`** membership.

Legacy API aliases: `project_lead` → `test_architect`, `tester` → `test_automation_engineer`.

### Permission matrix

| Permission | Admin | Test Architect | Test Automation Engineer | Developer | Viewer | System admin |
|------------|:-----:|:--------------:|:------------------------:|:---------:|:------:|:------------:|
| `view_project` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `update_project` | ✓ | ✓ | | | | ✓ |
| `delete_project` | ✓ | | | | | ✓ |
| `manage_memberships` | ✓ | | | | | ✓ |
| `manage_suites` | ✓ | ✓ | ✓ | | | ✓ |
| `create_edit_cases` | ✓ | ✓ | ✓ | | | ✓ |
| `delete_cases` | ✓ | ✓ | | | | ✓ |
| `manage_requirements` | ✓ | ✓ | | | | ✓ |
| `manage_plans` | ✓ | ✓ | | | | ✓ |
| `import_cases` | ✓ | ✓ | | | | ✓ |
| `view_reports` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `export_reports` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `execute_runs` | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `upload_results` | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `manage_attachments` | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `comment` | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `trigger_ci` | ✓ | | ✓ | ✓ | | ✓ |
| `manage_ci_config` | ✓ | | ✓ | | | ✓ |
| `llm_assist` | ✓ | ✓ | ✓ | ✓ | | ✓ |
| `manage_llm_config` | ✓ | ✓ | | | | ✓ |
| `manage_automation_repo` | ✓ | ✓ | ✓ | | | ✓ |
| `manage_api_tokens` | ✓ | | ✓ | | | ✓ |
| `manage_users` | | | | | | ✓ |
| `manage_system_settings` | | | | | | ✓ |
| `manage_sso` | | | | | | ✓ |
| `manage_global_integrations` | | | | | | ✓ |

Role keys: `admin`, `test_architect`, `test_automation_engineer`, `developer`, `viewer`.

## Tokens & CI

- **Personal API tokens** (`vulyk_at_…`): bound to a user. Tokens with `user_id IS NULL` are **rejected** (401) — they are never treated as system admin.
- **Project CI tokens** (`vulyk_ci_…`): scoped to CI reporting (open/upload/close). They do **not** grant broad project admin (e.g. cannot `create_project`). See [guides/ci-integration.md](guides/ci-integration.md).
- **Webhook secrets**: required for GitHub/GitLab webhook authorization (fail-closed). Responses expose `has_webhook_secret` only.

## Admin UI

Admin panel: **Users**, **Roles**, **Projects**, **Integrations**, **SSO**, **LLM providers**, **API tokens**, **Lists**, **Custom fields**, **Export / import**, **Backups**, **Updates**, **Instance settings** / **Sample data**.

## Migrations

- `backend/migrations/20261025143000_auth_rbac.sql` — sessions, memberships, user admin columns, API token ownership
- `backend/migrations/20261025150000_roles_setup_sso.sql` — five-role matrix, setup tokens, registration/invites, SSO providers

Security audit (Oct 2026) + fix PRs: [security/audit-2026-10.md](security/audit-2026-10.md).
