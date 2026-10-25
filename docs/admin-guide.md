# Vulyk admin guide

## Production deploy

### Docker Compose (recommended starting point)

```bash
./scripts/install.sh prod --yes \
  --admin-email=admin@company.example \
  --admin-password='Str0ngPass' \
  --registration-mode=disabled
# generates .env.prod with random secrets + first-run settings; auth required
```

Stack file: [`docker-compose.prod.yml`](../docker-compose.prod.yml)

- Named volumes for Postgres data, backup mount, and attachments
- Healthchecks on `db`, `api`, and `web`
- `VULYK_AUTH_REQUIRED=true`, `VULYK_REGISTRATION_MODE=disabled`, `VULYK_SEED_DEMO=0` by default
- Optional env seed via `VULYK_ADMIN_EMAIL` / `VULYK_ADMIN_PASSWORD`, or:

```bash
docker compose -f docker-compose.prod.yml --env-file .env.prod run --rm api \
  vulyk-api setup --yes
```

Dev/local stack: `./scripts/install.sh compose` (auth optional, demo seed on, registration open).

Full first-run paths: [guides/first-run-setup.md](guides/first-run-setup.md).

### Helm

```bash
# Validate render
./scripts/install.sh helm-template

# Install (create demo secrets — replace for real prod)
helm upgrade --install vulyk deploy/helm/vulyk \
  --namespace vulyk --create-namespace \
  --set secrets.create=true \
  --set auth.required=true \
  --set setup.adminPassword=change-me-admin1 \
  --set image.api=ghcr.io/example/vulyk-api:0.2.0 \
  --set image.web=ghcr.io/example/vulyk-web:0.2.0
```

Chart path: [`deploy/helm/vulyk`](../deploy/helm/vulyk). Auth is **required by default**. A setup Job runs `vulyk-api setup --yes`. Provide Secret keys:

| Key | Purpose |
|-----|---------|
| `DATABASE_URL` | Postgres URL |
| `VULYK_JWT_SECRET` | JWT / session signing |
| `VULYK_SECRETS_KEY` | Envelope encryption for LLM/integration/SSO secrets |
| `POSTGRES_PASSWORD` | Required when chart-managed Postgres is enabled |
| `VULYK_ADMIN_PASSWORD` | First-run admin (or use `setup.adminPasswordSecretRef`) |

Interactive fallback:

```bash
kubectl exec -it deploy/vulyk-vulyk-api -- vulyk-api setup
```

Raw manifests under [`deploy/kubernetes`](../deploy/kubernetes) are a minimal alternative.

### Bootstrap / default credentials

When seeded for local Compose (`VULYK_SEED_DEMO=1`):

| Account | Email | Password |
|---------|-------|----------|
| System admin | `admin@vulyk.local` | `admin` (must change) |
| Demo user | `user@vulyk.local` | `user` |

1. Complete first-run (env seed, setup Job/CLI, or web wizard + setup token).
2. Change the admin password if the default was used.
3. Explore the seeded sample project **Kyiv Open-Air Concert 2026** (`KYIV_CONCERT_2026`) — nested suites, ~35 cases, requirements, and two example runs (dress rehearsal + ticketing smoke). Opt out with `VULYK_SEED_SAMPLE=0` or uncheck the wizard box.
4. Create additional projects and memberships under **Admin**.
5. Mint API tokens for CI/MCP.
6. Configure LLM providers, ALM integrations, and [SSO](guides/sso.md) as needed.

## Backups

### Compose / VM Postgres

```bash
# Logical dump
docker compose -f docker-compose.prod.yml exec -T db \
  pg_dump -U vulyk -d vulyk -Fc > "vulyk-$(date +%F).dump"

# Restore into an empty database
docker compose -f docker-compose.prod.yml exec -T db \
  pg_restore -U vulyk -d vulyk --clean --if-exists < vulyk-YYYY-MM-DD.dump
```

Schedule dumps (cron/systemd timer) and copy artifacts off-box. The prod Compose file mounts `vulyk_pg_backup` at `/backup` inside the DB container for optional on-volume copies:

```bash
docker compose -f docker-compose.prod.yml exec db \
  pg_dump -U vulyk -d vulyk -Fc -f /backup/vulyk.dump
```

### Kubernetes

```bash
kubectl exec -n vulyk deploy/vulyk-postgresql -- \
  pg_dump -U vulyk -d vulyk -Fc > vulyk.dump
```

Prefer VolumeSnapshots for the PVC in addition to logical dumps for large datasets.

### What to backup

- PostgreSQL (required)
- Attachment volume/PVC if you store uploads outside the DB
- Secrets (`VULYK_JWT_SECRET`, `VULYK_SECRETS_KEY`) — without them encrypted integration/LLM/SSO keys cannot be decrypted

## Performance smoke

After the API is up:

```bash
export DATABASE_URL=postgres://vulyk:…@localhost:5432/vulyk
export VULYK_API_URL=http://127.0.0.1:8080
./scripts/install.sh perf-smoke
# or: ./scripts/perf-smoke.sh
```

Seeds ~100k cases (SQL when `DATABASE_URL`+`psql` available) and asserts list/search latency budgets.

## Users, RBAC & SSO

- [auth-and-rbac.md](auth-and-rbac.md) — roles, permission matrix, registration modes, seeding
- [guides/sso.md](guides/sso.md) — Google, Entra, GitHub, GitLab, Nextcloud
- [guides/first-run-setup.md](guides/first-run-setup.md) — Compose / Helm / CLI / wizard

## Install script platforms

`scripts/install.sh` supports **Linux**, **macOS**, and **WSL2**. Native Windows is not supported — use WSL2.

Modes: `compose`, `prod`, `dev`, `setup`, `check`, `helm-template`, `perf-smoke`.

First-run flags: `--admin-email=`, `--admin-password=`, `--locale=`, `--timezone=`, `--registration-mode=`, `--seed-demo=`, `--yes`.
