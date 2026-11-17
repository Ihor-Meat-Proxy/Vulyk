# Vulyk

<p align="left">
  <img src="docs/assets/vulyk-logo.svg" alt="Vulyk" height="48" />
</p>

**Self-hosted test management that stays under your control.**

*Vulyk* (вулик) is Ukrainian for *beehive*. It is an open-source TMS for QA and
engineering teams who want cases, plans, runs, CI results, and requirement
coverage without shipping that data to a vendor SaaS. You run it on your own
infrastructure — Docker Compose or Helm, Postgres, your network — under the
**AGPL-3.0**.

## Features

- Nested folders, auto-allocated `PREFIX-N` case keys, steps, versioning, plans/runs
- Requirements & coverage; option lists & custom fields; case PDF export
- CI ingest (JUnit, Playwright, pytest, Vulyk JSON) and **Run in CI** dispatch
- Importers from TestRail, Xray, Zephyr, Squash TM, TestLink, qTest, Qase, Kiwi, CSV
- Auth: email/password or SSO (OIDC providers), RBAC, personal & project CI tokens
- React PWA UI (offline result queue), OpenAPI, CLI reporter, MCP server, optional LLM
- Locales: English, Українська, Français, Español, Deutsch

## Quick start

```bash
./scripts/install.sh compose --yes
```

| Service | URL |
|---------|-----|
| UI | http://localhost:5173 |
| API | http://localhost:8080 |
| OpenAPI (Scalar) | http://localhost:8080/api/docs |
| Health | http://localhost:8080/health |

`compose --yes` builds, starts, and runs first-time setup on a fresh database.
Seeded local credentials (when `VULYK_SEED_DEMO=1`, the Compose default):

| Account | Email | Password |
|---------|-------|----------|
| Admin | `admin@vulyk.local` | `VULYK_ADMIN_PASSWORD` in `.env` (generated; change on first login) |
| Demo | `user@vulyk.local` | `VULYK_DEMO_PASSWORD` (default `vulyk-demo-user-1`) |

Env reference: [`.env.example`](.env.example). First-run details:
[docs/guides/first-run-setup.md](docs/guides/first-run-setup.md).

**Production Compose** (auth required):

```bash
./scripts/install.sh prod --yes \
  --admin-email=admin@company.example \
  --admin-password='Str0ngPass'
```

**Local development:** `./scripts/install.sh check` then `./scripts/install.sh dev --yes`,
run `cargo run -p vulyk-api` and `./scripts/npm-ci-secure.sh frontend && (cd frontend && npm run dev)`.

**Published images** (no local Rust/Node build):

```bash
./scripts/install.sh compose --yes --version 0.4.0
```

Helm: `./scripts/install.sh helm-template` then
`helm upgrade --install vulyk deploy/helm/vulyk …` — see
[docs/admin-guide.md](docs/admin-guide.md).

## Upgrade / rollback

Keep the Postgres volume; never run `docker compose down -v`.

```bash
./scripts/install.sh upgrade --yes                    # verified backup → pull → migrate → health
./scripts/install.sh upgrade --yes --version 0.4.0    # published GHCR images
./scripts/install.sh backup                           # verified DB + env/secrets bundle
./scripts/install.sh restore ./backups/vulyk-…/ --yes
./scripts/install.sh rollback --confirm=ROLLBACK      # after a failed upgrade path
```

Upgrades never regenerate `.env` / secrets / admin password; on failure they
auto-roll back. Full guarantees:
[docs/guides/upgrade.md](docs/guides/upgrade.md#data-safety-guarantees).

## Documentation

| | |
|--|--|
| **In-app help** | `/docs` in the UI — sources under [docs/user/](docs/user/) |
| **API docs** | `/api/docs` (OpenAPI) — overview [docs/api.md](docs/api.md) |
| **Operators** | [docs/admin-guide.md](docs/admin-guide.md), [docs/guides/](docs/guides/) |
| **Developers / agents** | [docs/dev/](docs/dev/), [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md) |
| **Security** | [SECURITY.md](SECURITY.md) |
| **Full index** | [docs/README.md](docs/README.md) |

## CI integrations

Publish results from **GitHub Actions**, **Forgejo / Gitea**, **GitLab CI**,
**Jenkins**, **Woodpecker**, **Bitbucket Pipelines**, or any pipeline that can
POST **JUnit** / Playwright / pytest / Vulyk JSON:

- Operator & token flow: [docs/guides/ci-integration.md](docs/guides/ci-integration.md)
- Framework samples (Selenide / pytest / Playwright on every CI): [examples/](examples/) · [docs/guides/framework-examples.md](docs/guides/framework-examples.md)
- Formats, dispatch, webhooks: [docs/guides/ingest-and-ci.md](docs/guides/ingest-and-ci.md)
- In-app CI/CD help: [docs/user/cicd.md](docs/user/cicd.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md). Local gates:
`./scripts/check.sh` (hooks: `git config core.hooksPath scripts/githooks`).
Doc link check: `./scripts/check-doc-links.sh`. Developer docs live under
[docs/dev/](docs/dev/).

## License

**GNU Affero General Public License v3.0** — see [LICENSE](LICENSE).
