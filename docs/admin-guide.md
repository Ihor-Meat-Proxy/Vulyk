# Vulyk admin guide

## Production deploy

### Docker Compose (recommended starting point)

```bash
./scripts/install.sh prod
# generates .env.prod with random secrets; auth required by default
```

Stack file: [`docker-compose.prod.yml`](../docker-compose.prod.yml)

- Named volumes for Postgres data, backup mount, and attachments
- Healthchecks on `db`, `api`, and `web`
- `VULYK_AUTH_REQUIRED=true` by default

Dev/local stack remains `./scripts/install.sh compose` (auth optional).

### Helm

```bash
# Validate render
./scripts/install.sh helm-template

# Install (create demo secrets — replace for real prod)
helm upgrade --install vulyk deploy/helm/vulyk \
  --namespace vulyk --create-namespace \
  --set secrets.create=true \
  --set auth.required=true \
  --set image.api=ghcr.io/example/vulyk-api:0.2.0 \
  --set image.web=ghcr.io/example/vulyk-web:0.2.0
```

Chart path: [`deploy/helm/vulyk`](../deploy/helm/vulyk). Auth is **required by default**. Provide Secret keys:

| Key | Purpose |
|-----|---------|
| `DATABASE_URL` | Postgres URL |
| `VULYK_JWT_SECRET` | JWT / session signing |
| `VULYK_SECRETS_KEY` | Envelope encryption for LLM/integration secrets |
| `POSTGRES_PASSWORD` | Required when chart-managed Postgres is enabled |

Raw manifests under [`deploy/kubernetes`](../deploy/kubernetes) are a minimal alternative.

### Bootstrap

1. Open the UI and create the first system admin (bootstrap).
2. Create projects and memberships under **Admin**.
3. Mint API tokens for CI/MCP.
4. Configure LLM providers and ALM integrations as needed.

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
- Secrets (`VULYK_JWT_SECRET`, `VULYK_SECRETS_KEY`) — without them encrypted integration/LLM keys cannot be decrypted

## Performance smoke

After the API is up:

```bash
export DATABASE_URL=postgres://vulyk:…@localhost:5432/vulyk
export VULYK_API_URL=http://127.0.0.1:8080
./scripts/install.sh perf-smoke
# or: ./scripts/perf-smoke.sh
```

Seeds ~100k cases (SQL when `DATABASE_URL`+`psql` available) and asserts list/search latency budgets.

## Users & RBAC

See [auth-and-rbac.md](auth-and-rbac.md). System admins manage users; project roles gate membership-aware routes.

## Install script platforms

`scripts/install.sh` supports **Linux**, **macOS**, and **WSL2**. Native Windows is not supported — use WSL2. Modes: `compose`, `prod`, `dev`, `check`, `helm-template`, `perf-smoke`.
