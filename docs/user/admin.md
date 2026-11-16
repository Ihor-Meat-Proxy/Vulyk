# Admin

System administrators and project admins use **Admin** (`/admin`) and related settings pages to manage people, instance defaults, appearance defaults, integrations, and upgrades.

## Admin tabs

Visibility depends on role. System admins see the full set; project admins see a subset (for example projects, integrations, roles, LLM, tokens).

| Tab | Typical audience | Purpose |
|-----|------------------|---------|
| Users | System admin | Create/disable users, reset passwords |
| Settings | System admin | Instance name, locale, registration mode, sample data |
| Updates | System admin | Check GitHub Releases, apply/rollback (Compose sidecar) |
| SSO | System admin | OAuth2/OIDC providers |
| Backups | System admin | Content backup destinations, policies, run/restore |
| Lists | System admin | Option lists for custom fields |
| Custom fields | System admin | Field definitions and scopes |
| Export / import | System admin | Config JSON/YAML bundles |
| Integrations | Admin+ | ALM / CI-related settings; links to CI tokens |
| Roles | Project admin+ | Memberships / role assignment for projects |
| Projects | Project admin+ | Project list and membership helpers |
| LLM | `manage_llm_config` | LLM provider configs |
| Tokens | `manage_api_tokens` | Personal API tokens (also available under My settings) |

## Users and roles

### System admin

`is_system_admin` grants all project permissions plus `manage_users`, `manage_system_settings`, `manage_sso`, and `manage_global_integrations`.

### Per-project roles

| Role | Highlights |
|------|------------|
| Admin | Full project control including memberships and CI config |
| Test Architect | Cases, requirements, plans, import, LLM config |
| Test Automation Engineer | Cases, execute/upload, trigger CI, automation repos, tokens |
| Developer | View, execute/upload, trigger CI, LLM assist (no case create) |
| Viewer | View project and reports/export only |

Registration modes: `open`, `invite`, or `disabled` (production default). SSO providers (Google, Microsoft Entra, GitHub, GitLab, Nextcloud, generic OIDC) are configured under **SSO**; callback URL is `{VULYK_BASE_URL}/api/v1/auth/sso/callback`.

## Instance settings and sample data

Under **Admin → Settings**:

- Instance display name, default locale (`en` / `uk` / `fr` / `es` / `de`), timezone, registration mode
- Default theme for new users
- **Sample data** — list installed sample projects, **Remove sample data** (confirm; only `is_sample` rows), **Re-seed sample data**

## Appearance, fonts, and themes

Open **Appearance** from the gear menu (`/appearance`) — not in the left nav.

- **Theme:** System (follows `prefers-color-scheme` / high contrast), Light, Dark, High contrast, or Sepia / night warm
- **Font (UI):** Inter (default), IBM Plex Sans, Noto Sans, or Source Sans 3 — **self-hosted** in the app bundle (no third-party font CDN)
- **Font size:** 87%–138%
- **Density:** Comfortable or Compact
- **Reduced motion:** Follow system, always reduce, or allow motion

Preferences save to the account when signed in and mirror in the browser so the theme applies before paint. Admins set the **instance default theme** under Admin settings.

## Languages

Users switch language with the header **globe** (English, Українська, Français, Español, Deutsch). Instance default locale is set at first-run or under Admin settings (`VULYK_DEFAULT_LOCALE`).

## Lists and custom fields

See [Concepts](concepts.md#custom-fields-and-option-lists). Only system admins manage definitions; project editors set values on cases/requirements.

## Content backups

**Admin → Backups** schedules or runs **content** backups (ZIP of projects/cases/fields) to local volume, GitHub, GitLab, Nextcloud, or Drive. This is separate from Postgres dumps used by `install.sh backup` / upgrades.

## Config export / import

**Admin → Export / import** exports a versioned JSON/YAML bundle (projects settings, custom fields, option lists, optional memberships by email, registration/SSO without secrets, LLM without keys, integrations without tokens, appearance defaults, automation repos without tokens). Optionally include case/requirement/run data. Import supports dry-run preview, merge or replace.

CLI wrappers: `./scripts/install.sh config-export` / `config-import`, or `vulyk-api config export|import`.

## Updates, backup, and rollback

### Admin → Updates

1. See **current** API version (`/health`) and **latest** GitHub Release.
2. Click **Check for updates** (or use a periodic interval).
3. Read release notes when a newer version exists.
4. On Docker Compose with the **updater sidecar** (`VULYK_UPDATER_TOKEN` + updates profile), click **Update now**. Same verified-backup → migrate → health path as `./scripts/install.sh upgrade --version`, with automatic rollback on failure.
5. After success, **Roll back to vX.Y.Z** may appear for `VULYK_ROLLBACK_RETENTION_DAYS` (default 7). Type `ROLLBACK` to confirm — post-upgrade data is lost. Version backups (last N installed versions) can be downloaded.
6. On Kubernetes or installs without the sidecar, one-click Update/rollback are hidden; the UI shows **manual** `install.sh` / Helm commands instead.

Update check errors (GitHub rate limit, network, HTTP) surface as translated messages with **Retry**. See [Troubleshooting](troubleshooting.md#update-check-errors).

### Upgrade via install.sh

```bash
# Verified backup → pull/rebuild → migrate → up → health
./scripts/install.sh upgrade --yes
./scripts/install.sh upgrade --yes --version 0.1.0

# Rollback (typed confirmation)
./scripts/install.sh rollback --confirm=ROLLBACK

# Ad-hoc DB/config backup & restore
./scripts/install.sh backup
./scripts/install.sh restore ./backups/vulyk-VERSION-TIMESTAMP/ --yes
```

Guarantees:

- Verified backup first (integrity-checked dump; never overwrite existing `.env` secrets/admin passwords)
- Missing env keys only are synced from `.env.example`
- Auto-rollback on failed migrate/health
- **Never** run `docker compose down -v` on an instance whose data you care about

## My settings

Gear → **My settings** (`/settings`): display name, language, timezone, notifications, password change, personal API tokens, **Project CI tokens**, and **Test automation repositories**.
