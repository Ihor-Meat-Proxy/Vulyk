# Content backups

Vulyk supports two complementary backup layers:

| | **Database backup** | **Content backup** (this guide) |
|--|---------------------|----------------------------------|
| Tool | `./scripts/install.sh backup` / `pg_dump` | Admin → **Backups** (API job) |
| Scope | Entire Postgres (users, sessions, jobs, …) | Portable project archives (cases, requirements, fields, …) |
| Format | SQL dump / custom `pg_dump` | Versioned ZIP with JSON + checksums (`content-1.0`) |
| Destinations | Local `./backups/` (install script) | Local volume, GitHub, GitLab, Nextcloud, Google Drive |
| Use when | Disaster recovery, upgrades | Cross-instance copy, Git history, selective restore |

DB upgrade path: [upgrade.md](upgrade.md). Admin overview: [admin-guide.md](../admin-guide.md).

## Quick start

1. Sign in as a **system administrator**.
2. Open **Admin → Backups**.
3. **Add destination** — start with `local` and path `./data/content-backups` (or a mounted volume). Click **Test connection**.
4. Select the destination (and optionally a project) → **Back up now**.
5. Confirm a **success** row in History; use **Download** or **Restore** (dry-run preview first).

## What a content archive contains

Archives declare `schema_version` (`content-1.0`). Typical layout:

```text
ContentArchive
├── manifest.json          # ids, scope, checksums, created_at, producer
├── config.json            # ConfigBundle-compatible subset (lists, fields, project meta, folders)
├── projects/<key>/
│   ├── project.json
│   ├── folders.json
│   ├── tags.json
│   ├── cases.json
│   ├── requirements.json
│   ├── tombstones.json    # retired case numbers (never reuse)
│   └── attachments/       # optional binaries
└── checksums.sha256
```

Importers migrate known older minors forward; unknown future major versions are rejected.

## Schedules & change detection

Create a **policy** with schedule `hourly` or `daily`, optional project scope, debounce (default 30 minutes), and retention (`retain_count` / `retain_days`).

The API scheduler takes a Postgres advisory lock across replicas and only uploads when cases or requirements changed since the last successful backup (watermark + debounce). Unchanged windows are recorded as `skipped`. Manual **Back up now** bypasses change detection. Job workers claim with `FOR UPDATE SKIP LOCKED`.

Pack modes: `zip` (single archive) or `unpacked` (JSON tree — better Git diffs).

## Restore modes

| Mode | Behavior |
|------|----------|
| `new_project` | Create a project from the archive (renames key on clash) |
| `merge` | Match cases/requirements by key; `skip` or `overwrite` conflicts |
| `replace` | Clear project content (keep memberships), then import |

Custom fields map by key/name; missing definitions and option lists are created. Case keys and **tombstones** are preserved — retired numbers are never reused. Always preview with `dry_run` first.

## Destinations

Credentials are encrypted with `VULYK_SECRETS_KEY` and **masked** in the UI. Supported kinds: `local`, `github`, `gitlab`, `nextcloud`, `gdrive`.

**Local root:** set `VULYK_CONTENT_BACKUP_DIR` (default `./data/content-backups`). Local destination paths must stay under that directory (path-escape guard). Outbound GitHub/GitLab/Nextcloud/Drive URLs use the shared egress allowlist; set `VULYK_EGRESS_ALLOW_PRIVATE=1` only for lab/mock servers.

Do not embed live integration/LLM API keys in content archives (config export passphrase path remains separate). Treat destination repos as sensitive when attachments are included.

## API

See OpenAPI tag **backups** (`/api/docs`) — destinations, policies, run, history, download, restore, and restore upload. All routes require a **system administrator**. Preview a restore with `dry_run: true` on `POST /api/v1/admin/backups/restore` (or `?dry_run=true` on `/restore/upload`); there is no separate `/restore/preview` endpoint.
