# SSO setup (OAuth2 / OIDC)

Vulyk supports Google, Microsoft Entra ID, GitHub, GitLab (cloud + self-hosted), Nextcloud, and generic OIDC. Providers are seeded disabled; enable them in **Admin → SSO** or via env on API boot.

Callback URL (all providers):

```text
{VULYK_BASE_URL}/api/v1/auth/sso/callback
```

Example local: `http://localhost:8080/api/v1/auth/sso/callback`  
Example prod: `https://vulyk.example.com/api/v1/auth/sso/callback`

Set `VULYK_BASE_URL` to the public origin users reach (no trailing slash). Authorize flow starts at:

```text
GET /api/v1/auth/sso/{provider_key}/start
```

## Env overrides

On boot, the API applies non-empty env vars into the matching `sso_providers` row:

| Variable | Purpose |
|----------|---------|
| `VULYK_SSO_<KEY>_CLIENT_ID` | OAuth client id |
| `VULYK_SSO_<KEY>_CLIENT_SECRET` | Client secret (encrypted at rest) |
| `VULYK_SSO_<KEY>_ENABLED` | `1` / `true` to enable |
| `VULYK_SSO_<KEY>_ISSUER` | OIDC issuer URL (discovery) |
| `VULYK_SSO_<KEY>_BASE_URL` | GitLab / Nextcloud instance base |
| `VULYK_SSO_<KEY>_TENANT` | Microsoft Entra tenant id / `common` / `organizations` |

`<KEY>` is one of: `GOOGLE`, `MICROSOFT`, `GITHUB`, `GITLAB`, `NEXTCLOUD`, `OIDC`.

JIT provisioning respects instance `registration_mode` and optional `allowed_email_domains` / group→role mappings on the provider.

---

## Google

1. [Google Cloud Console](https://console.cloud.google.com/) → APIs & Services → Credentials → Create OAuth client (Web).
2. Authorized redirect URI: `{VULYK_BASE_URL}/api/v1/auth/sso/callback`
3. Copy Client ID / Client secret into Admin → SSO → **Google**, or:

```bash
VULYK_SSO_GOOGLE_CLIENT_ID=….apps.googleusercontent.com
VULYK_SSO_GOOGLE_CLIENT_SECRET=…
VULYK_SSO_GOOGLE_ENABLED=1
# optional; default issuer:
# VULYK_SSO_GOOGLE_ISSUER=https://accounts.google.com
```

| Field | Value |
|-------|-------|
| Scopes (default) | `openid email profile` |
| Issuer | `https://accounts.google.com` |

---

## Microsoft Entra ID (Azure AD)

1. Entra admin center → App registrations → New registration.
2. Redirect URI (Web): `{VULYK_BASE_URL}/api/v1/auth/sso/callback`
3. Certificates & secrets → new client secret.
4. Set **tenant** to your directory tenant id (recommended) or `organizations` / `common`.

```bash
VULYK_SSO_MICROSOFT_CLIENT_ID=<application-(client)-id>
VULYK_SSO_MICROSOFT_CLIENT_SECRET=<secret-value>
VULYK_SSO_MICROSOFT_TENANT=<tenant-id>
VULYK_SSO_MICROSOFT_ENABLED=1
```

| Field | Value |
|-------|-------|
| Scopes (default) | `openid email profile offline_access` |
| Issuer | `https://login.microsoftonline.com/{tenant}/v2.0` |

Expose email claims as needed (Token configuration → optional claims: `email`).

---

## GitHub

1. GitHub → Settings → Developer settings → OAuth Apps → New.
2. Authorization callback URL: `{VULYK_BASE_URL}/api/v1/auth/sso/callback`
3. Create client secret.

```bash
VULYK_SSO_GITHUB_CLIENT_ID=…
VULYK_SSO_GITHUB_CLIENT_SECRET=…
VULYK_SSO_GITHUB_ENABLED=1
```

| Field | Value |
|-------|-------|
| Scopes (default) | `read:user user:email` |
| Auth / token | `https://github.com/login/oauth/authorize` · `…/access_token` |

GitHub is OAuth2 (not OIDC discovery). Email is resolved from the user API + `/user/emails` when needed.

---

## GitLab (gitlab.com or self-hosted)

1. GitLab → Preferences / Admin → Applications → New.
2. Redirect URI: `{VULYK_BASE_URL}/api/v1/auth/sso/callback`
3. Scopes: `openid`, `email`, `profile` (and `read_user` if required by your version).

**Cloud (gitlab.com):**

```bash
VULYK_SSO_GITLAB_CLIENT_ID=…
VULYK_SSO_GITLAB_CLIENT_SECRET=…
VULYK_SSO_GITLAB_ENABLED=1
# default base_url is https://gitlab.com
```

**Self-hosted:**

```bash
VULYK_SSO_GITLAB_CLIENT_ID=…
VULYK_SSO_GITLAB_CLIENT_SECRET=…
VULYK_SSO_GITLAB_BASE_URL=https://gitlab.example.com
# optional OIDC discovery:
# VULYK_SSO_GITLAB_ISSUER=https://gitlab.example.com
VULYK_SSO_GITLAB_ENABLED=1
```

| Field | Value |
|-------|-------|
| Scopes (default) | `openid email profile` |
| Endpoints (no issuer) | `{base}/oauth/authorize`, `{base}/oauth/token`, `{base}/oauth/userinfo` |

---

## Nextcloud

Requires the Nextcloud **OAuth2** app (and optionally OIDC).

1. Nextcloud → Settings → Security / OAuth2 → Add client.
2. Redirection URI: `{VULYK_BASE_URL}/api/v1/auth/sso/callback`
3. Set **base_url** to your Nextcloud origin (required).

```bash
VULYK_SSO_NEXTCLOUD_CLIENT_ID=…
VULYK_SSO_NEXTCLOUD_CLIENT_SECRET=…
VULYK_SSO_NEXTCLOUD_BASE_URL=https://cloud.example.com
VULYK_SSO_NEXTCLOUD_ENABLED=1
# optional:
# VULYK_SSO_NEXTCLOUD_ISSUER=https://cloud.example.com
```

| Field | Value |
|-------|-------|
| Scopes (default) | `openid email profile` |
| Authorize | `{base}/index.php/apps/oauth2/authorize` |
| Token | `{base}/index.php/apps/oauth2/api/v1/token` |
| Userinfo | `{base}/ocs/v2.php/cloud/user` (OCS) |

---

## Generic OIDC

Use provider key `oidc` for any standards-compliant IdP (Keycloak, Authentik, Okta, …).

```bash
VULYK_SSO_OIDC_CLIENT_ID=…
VULYK_SSO_OIDC_CLIENT_SECRET=…
VULYK_SSO_OIDC_ISSUER=https://idp.example.com/realms/vulyk
VULYK_SSO_OIDC_ENABLED=1
```

| Field | Value |
|-------|-------|
| Scopes (default) | `openid email profile` |
| Discovery | `{issuer}/.well-known/openid-configuration` |

---

## Admin UI & API

- List public providers: `GET /api/v1/auth/sso/providers`
- Admin CRUD: `/api/v1/admin/sso/providers` (system admin / `manage_sso`)
- Group → role mapping and email-domain allowlists are stored per provider (`group_role_mapping`, `allowed_email_domains`).

Related: [auth-and-rbac.md](../auth-and-rbac.md), [first-run-setup.md](first-run-setup.md).
