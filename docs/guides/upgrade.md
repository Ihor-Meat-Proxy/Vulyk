# Upgrading Vulyk

Safe upgrades keep the **Postgres named volume / PVC**. Schema changes are
forward-only SQLx migrations. Prefer the install script or Helm migrate Job over
hand-editing the database.

## Critical warning

**Never run `docker compose down -v` (or `docker compose down --volumes`) on an
instance whose data you care about.** The `-v` flag deletes named volumes,
including Postgres data. Use `docker compose down` without `-v`, or stop
individual services (`docker compose stop api web`).

## Compose (local / production)

### Recommended: `./scripts/install.sh upgrade`

```bash
# Backup → git pull (or --to tag) → rebuild → migrate run → up → health check
./scripts/install.sh upgrade --yes

# Pin a release tag or branch
./scripts/install.sh upgrade --yes --to=v0.1.0

# Skip backup only when you already have an off-box dump
./scripts/install.sh upgrade --yes --no-backup
```

What it does:

1. **Preflight** — Docker, free disk, current version (`/health` + git)
2. **Migration status** — `vulyk-api migrate status` (lists pending)
3. **Backup** — `pg_dump` → `./backups/vulyk-<version>-<timestamp>.sql.gz`
   (retention via `VULYK_BACKUP_RETENTION`, default 10)
4. **Checkout** — optional `--to <tag|branch>`, otherwise `git pull --ff-only`
5. **Rebuild** images; **stop API/web only** (DB volume untouched)
6. **One-shot migrate** — `vulyk-api migrate run` **before** starting the new API
7. **Start** stack and **health-check** `/health` (includes `version`)

On failure it **stops**, keeps the volume, and prints a restore command.

### Manual backup / restore

```bash
./scripts/install.sh backup
./scripts/install.sh restore ./backups/vulyk-0.1.0-20260101T120000Z.sql.gz --yes
```

### Environment

| Variable | Compose dev | Compose prod | Meaning |
|----------|-------------|--------------|---------|
| `VULYK_AUTO_MIGRATE` | `true` (default) | `false` | Apply migrations on API startup |
| `VULYK_BACKUP_RETENTION` | `10` | `10` | Gzipped dumps to keep under `./backups/` |

Prod Compose expects migrations from `install.sh upgrade` (or
`docker compose run --rm api vulyk-api migrate run`) before the API serves
traffic. If auto-migrate is off and migrations are pending, the API **refuses
to start**.

### CLI reference

```bash
vulyk-api version              # semver from Cargo.toml
vulyk-api migrate status       # applied + pending (exit 1 if pending, 2 if schema newer)
vulyk-api migrate run          # apply pending (Postgres advisory lock)
```

Downgrade protection: if the database has migrations the binary does not know,
the API and `migrate run` refuse to proceed. Restore a matching backup or deploy
a newer binary.

## Production checklist

1. Schedule / take a backup (`install.sh backup` or `pg_dump -Fc` off-box).
2. Read the release notes / changelog for the target tag.
3. Run `./scripts/install.sh upgrade --yes --to=vX.Y.Z`.
4. Confirm `/health` returns `"status":"ok"` and the expected `"version"`.
5. Spot-check sample projects / critical cases.
6. Keep the backup until the next successful upgrade.

Also back up attachment volumes and secrets (`VULYK_JWT_SECRET`,
`VULYK_SECRETS_KEY`) — see [admin-guide.md](../admin-guide.md).

## Helm / Kubernetes

Chart path: [`deploy/helm/vulyk`](../../deploy/helm/vulyk).

### Migrate Job (pre-upgrade hook)

On `helm upgrade`, a Job runs `vulyk-api migrate run` **before** API pods roll
(`helm.sh/hook: pre-upgrade`). Fresh installs migrate via an API **initContainer**
(same command; advisory-locked so replicas do not race).

API pods set `VULYK_AUTO_MIGRATE=false` (see `values.yaml` → `migrate.autoMigrate`).

### Backup before upgrade

```bash
kubectl exec -n vulyk deploy/vulyk-postgresql -- \
  pg_dump -U vulyk -d vulyk -Fc > "vulyk-$(date -u +%Y%m%dT%H%M%SZ).dump"
```

Prefer VolumeSnapshots for large PVCs in addition to logical dumps.

### PVC retention

`postgresql.persistence.keepOnUninstall: true` (default) annotates the PVC with
`helm.sh/resource-policy: keep` so `helm uninstall` does **not** delete Postgres
data. Set to `false` only for disposable demos.

### Example upgrade

```bash
# 1) Backup (above)
# 2) Upgrade chart / images
helm upgrade vulyk deploy/helm/vulyk --namespace vulyk \
  --set image.api=ghcr.io/example/vulyk-api:0.1.0 \
  --set image.web=ghcr.io/example/vulyk-web:0.1.0

# 3) Watch migrate hook + rollout
kubectl get jobs -n vulyk
kubectl rollout status deploy/vulyk-vulyk-api -n vulyk
curl -fsS https://vulyk.example/health
```

Setup Job is **post-install only** (not re-run on every upgrade).

## Versioning

- Semver lives in `backend/Cargo.toml`, `cli/Cargo.toml`, `mcp/Cargo.toml`, and
  `frontend/package.json` (keep them aligned).
- Tag releases as `vMAJOR.MINOR.PATCH` and publish a GitHub Release with changelog
  (see `CHANGELOG.md` and `.github/workflows/release.yml`).
- `/health` and `/ready` include `"version"`. The UI sidebar footer shows the same
  value from `/health`.

## Related

- [Migration authoring rules](../../CONTRIBUTING.md#database-migrations)
- [Admin guide — backups](../admin-guide.md#backups)
- [First-run setup](first-run-setup.md)
