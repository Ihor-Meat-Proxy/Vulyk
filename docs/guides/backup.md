# Content backups

Vulyk supports two complementary backup layers:

| | **Database backup** | **Content backup** (this guide) |
|--|---------------------|----------------------------------|
| Tool | `./scripts/install.sh backup` / `pg_dump` | Admin → **Backups** (API job) |
| Scope | Entire Postgres (users, sessions, jobs, …) | Portable project archives (cases, requirements, fields, …) |
| Format | SQL dump / custom `pg_dump` | Versioned ZIP with JSON + checksums (`content-1.0`) |
| Destinations | Local `./backups/` (install script) | Local volume, GitHub, GitLab, Nextcloud, Google Drive |
| Use when | Disaster recovery, upgrades | Cross-instance copy, Git history, selective restore |

Design details and Mermaid flows: [docs/backup-design.md](../backup-design.md).  
DB upgrade path: [guides/upgrade.md](upgrade.md).

## Quick start

1. Sign in as a **system administrator**.
2. Open **Admin → Backups**.
3. **Add destination** — start with `local` and path `./data/content-backups` (or a mounted volume). Click **Test connection**.
4. Select the destination (and optionally a project) → **Back up now**.
5. Confirm a **success** row in History; use **Download** or **Restore** (dry-run preview first).

## Schedules & change detection

Create a **policy** with schedule `hourly` or `daily`, optional project scope, debounce (default 30 minutes), and retention (`retain_count` / `retain_days`).

The API scheduler (advisory-locked across replicas) only uploads when cases or requirements changed since the last successful backup. Unchanged windows are recorded as `skipped`.

## Restore modes

| Mode | Behavior |
|------|----------|
| `new_project` | Create a project from the archive (renames key on clash) |
| `merge` | Match cases/requirements by key; `skip` or `overwrite` conflicts |
| `replace` | Clear project content (keep memberships), then import |

Custom fields map by key/name; missing definitions and option lists are created. Case keys and **tombstones** are preserved — retired numbers are never reused.

## Destinations

Credentials are encrypted with `VULYK_SECRETS_KEY` and **masked** in the UI. Supported kinds: `local`, `github`, `gitlab`, `nextcloud`, `gdrive`. Pack mode `unpacked` writes a JSON tree (better Git diffs); `zip` stores a single archive.

**Local root:** set `VULYK_CONTENT_BACKUP_DIR` (default `./data/content-backups`). Local destination paths must stay under that directory (SSRF / path-escape guard). Outbound GitHub/GitLab/Nextcloud/Drive URLs use the shared egress allowlist; set `VULYK_EGRESS_ALLOW_PRIVATE=1` only for lab/mock servers.

## API

See OpenAPI tag **backups** (`/api/docs`) — destinations, policies, run, history, download, restore, and restore upload. All routes require a **system administrator**. Preview a restore with `dry_run: true` on `POST /api/v1/admin/backups/restore` (or `?dry_run=true` on `/restore/upload`); there is no separate `/restore/preview` endpoint.
