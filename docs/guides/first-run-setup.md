# First-run setup

Complete Vulyk setup by creating the first system admin, instance name, locale/timezone, and registration mode. Prefer **env seeding** for Compose/Helm automation; use the **CLI**, **install.sh**, or **web wizard** when you need an interactive path.

## What gets configured

| Setting | Env / flag | Notes |
|---------|------------|--------|
| Admin email | `VULYK_ADMIN_EMAIL` / `--admin-email` | Default local: `admin@vulyk.local` |
| Admin password | `VULYK_ADMIN_PASSWORD` / `--admin-password` | Env seed accepts short `admin` (forces change). CLI requires ≥8 chars + letter + digit |
| Locale | `VULYK_DEFAULT_LOCALE` / `--locale` | `en` or `uk` |
| Timezone | `VULYK_DEFAULT_TIMEZONE` / `--timezone` | e.g. `UTC`, `Europe/Kyiv` |
| Registration | `VULYK_REGISTRATION_MODE` / `--registration-mode` | `open` \| `invite` \| `disabled` |
| Instance name | `VULYK_INSTANCE_NAME` / `--instance-name` | |
| Demo users | `VULYK_SEED_DEMO` | `1` seeds `user@vulyk.local` / `user` |
| Sample projects | `VULYK_SEED_SAMPLE` / `--seed-sample` | Default `1` — seeds **Kyiv Open-Air Concert 2026** (`KC-*`, ~35 cases) and **Vulyk QA** (`VQA-*`, ~130+ product self-tests). Set `0` to skip. Wizard checkbox defaults on. Not seeded in prod unless enabled. |
| Base URL | `VULYK_BASE_URL` / `--base-url` | Required for SSO callbacks |
| Setup token | `VULYK_SETUP_TOKEN` / `--setup-token` | One-time; see below |

After success, `setup_completed=true` and outstanding setup tokens are consumed.

---

## Docker Compose (local)

```bash
./scripts/install.sh compose
# interactive prompts, or:
./scripts/install.sh compose --yes \
  --admin-email=admin@vulyk.local \
  --admin-password=admin \
  --locale=en --timezone=UTC \
  --registration-mode=open
```

Writes `.env` and starts the stack. Compose defaults:

- `VULYK_ADMIN_EMAIL=admin@vulyk.local`
- `VULYK_ADMIN_PASSWORD=admin` → `must_change_password` on first login
- `VULYK_SEED_DEMO=1`
- `VULYK_REGISTRATION_MODE=open`

**Explicit setup pass** (if admin was not seeded):

```bash
./scripts/install.sh setup --yes
# or:
docker compose run --rm api vulyk-api setup --yes \
  --admin-email=you@example.com \
  --admin-password='Str0ngPass' \
  --locale=en --timezone=UTC \
  --registration-mode=open
```

Without Docker: `cargo run -p vulyk-api -- setup --yes …` (install.sh falls back automatically).

---

## Production Compose

```bash
./scripts/install.sh prod --yes \
  --admin-email=admin@company.example \
  --admin-password='Str0ngPass' \
  --registration-mode=disabled \
  --seed-demo=0
```

Writes `.env.prod` (random JWT/secrets + your first-run values). Prod defaults: `VULYK_REGISTRATION_MODE=disabled`, `VULYK_SEED_DEMO=0`.

```bash
docker compose -f docker-compose.prod.yml --env-file .env.prod run --rm api \
  vulyk-api setup --yes
```

---

## Bare metal / `dev` mode

```bash
./scripts/install.sh dev --yes   # Postgres + .env
set -a && source .env && set +a
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
cargo run -p vulyk-api           # env seed on boot
# or:
cargo run -p vulyk-api -- setup --yes
./scripts/install.sh setup --yes
```

---

## Helm

`deploy/helm/vulyk` values:

```yaml
setup:
  adminEmail: admin@vulyk.local
  adminPasswordSecretRef: # {name, key} or leave empty and use chart secret
  locale: en
  timezone: UTC
  registrationMode: disabled
  seedDemo: false
```

A post-install **Job** runs `vulyk-api setup --yes …`, reading the password from the secret.

```bash
helm upgrade --install vulyk deploy/helm/vulyk \
  --namespace vulyk --create-namespace \
  --set secrets.create=true \
  --set setup.adminPassword=change-me-admin1
```

Interactive fallback:

```bash
kubectl exec -it deploy/vulyk-vulyk-api -- vulyk-api setup
```

(Exact deployment name: `{{ release }}-vulyk-api` — see Helm NOTES.)

---

## CLI `vulyk-api setup`

```bash
vulyk-api setup                          # interactive
vulyk-api setup --yes \
  --admin-email=admin@example.com \
  --admin-password='Str0ngPass' \
  --locale=en \
  --timezone=UTC \
  --registration-mode=disabled \
  --instance-name=Vulyk \
  --base-url=https://vulyk.example.com
```

Flags also read from the env vars listed above. Fails with conflict if setup already completed.

---

## Web wizard + setup token

If no `VULYK_ADMIN_PASSWORD` is set on first boot, the API prints a one-time token:

```text
Vulyk first-run setup token (one-time, protect this value):
  vulyk_setup_<hex>
```

Open the UI; the wizard asks for the token (or pass `VULYK_SETUP_TOKEN` / `--setup-token` to the CLI). Token hashes are stored in `setup_tokens` and consumed when setup finishes.

---

## Default credentials (seeded local)

| User | Email | Password | Notes |
|------|-------|----------|--------|
| System admin | `admin@vulyk.local` | `admin` | Must change password |
| Demo user | `user@vulyk.local` | `user` | When `VULYK_SEED_DEMO=1` |

Never use these defaults in production.

See also: [auth-and-rbac.md](../auth-and-rbac.md), [admin-guide.md](../admin-guide.md), [sso.md](sso.md).
