# Vulyk documentation

<p align="left">
  <img src="assets/vulyk-mark.svg" alt="Vulyk" height="40" />
</p>

## User / in-app docs

Help rendered in the product at `/docs`:

| Doc | Topic |
|-----|-------|
| [user/](user/) | In-app Markdown (overview, cases, runs, CI/CD, admin, …) |
| [user-guide.md](user-guide.md) | Short pointer + day-to-day notes (prefer `user/` for product help) |

## Operators

| Doc | Topic |
|-----|-------|
| [admin-guide.md](admin-guide.md) | Deploy, env/security, Admin UI, Helm |
| [guides/first-run-setup.md](guides/first-run-setup.md) | Compose / Helm / CLI / wizard |
| [guides/upgrade.md](guides/upgrade.md) | Safe upgrades, verified backups, rollback |
| [guides/backup.md](guides/backup.md) | Content backup / restore (ZIP destinations) |
| [guides/in-app-updates.md](guides/in-app-updates.md) | Admin Updates tab + updater sidecar |
| [guides/sso.md](guides/sso.md) | Google, Entra, GitHub, GitLab, Nextcloud |
| [guides/pwa-and-mobile.md](guides/pwa-and-mobile.md) | PWA, offline queue, mobile UX |
| [../SECURITY.md](../SECURITY.md) | Vulnerability reporting & security posture |

## Developers / agents

| Doc | Topic |
|-----|-------|
| [dev/](dev/) | Contributor & agent documentation |
| [dev/architecture.md](dev/architecture.md) | Modules, ERD, security summary |
| [dev/ci-checks.md](dev/ci-checks.md) | CI / security / quality check index |
| [dev/supply-chain.md](dev/supply-chain.md) | Dependabot cooldown, locked installs, npm security fast path |
| [dev/ci-failures.md](dev/ci-failures.md) | CI failure catalog |
| [dev/migrations.md](dev/migrations.md) | SQLx migrations, Squawk, verify / upgrade CI |
| [dev/testing/test-ids.md](dev/testing/test-ids.md) | Playwright `data-testid` conventions |
| [dev/testing/playwright-vulyk.md](dev/testing/playwright-vulyk.md) | Map e2e tests to VQA cases + reporter |
| [dev/e2e-local.md](dev/e2e-local.md) | Local Docker Playwright suite |
| [dev/releasing.md](dev/releasing.md) | Version bump, tags, GHCR |
| [dev/design-tokens.md](dev/design-tokens.md) | Themes, appearance, locale prefs |
| [../AGENTS.md](../AGENTS.md) | Agent / PR guardrails |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Hooks, migrations, i18n, test ids |
| [../backend/seed/vulyk-qa.d/README.md](../backend/seed/vulyk-qa.d/README.md) | QA seed fragments |

## API / integrations

| Doc | Topic |
|-----|-------|
| [api.md](api.md) | HTTP API overview (OpenAPI at `/api/docs`) |
| [auth-and-rbac.md](auth-and-rbac.md) | Auth modes, JWT/sessions/tokens, RBAC |
| [guides/ci-integration.md](guides/ci-integration.md) | CI reporting + **Run in CI** |
| [guides/framework-examples.md](guides/framework-examples.md) | Selenide / pytest / Playwright samples on every CI |
| [../examples/](../examples/) | Runnable sample projects + per-provider CI configs |
| [guides/outbound-webhooks.md](guides/outbound-webhooks.md) | Signed outbound webhooks (case/run/result events) |
| [guides/ingest-and-ci.md](guides/ingest-and-ci.md) | Formats, dispatch, webhooks |
| [guides/executions.md](guides/executions.md) | Campaigns, iterations, pipeline upload |
| [automation-repo.md](automation-repo.md) | Automation Git repos for Generate |
| [integrations/](integrations/) | OpenProject, Jira, importers |
| [mcp.md](mcp.md) | MCP server for AI assistants |
| [llm.md](llm.md) | LLM providers, generate, chat |

## Start here

| Doc | Audience |
|-----|----------|
| [../README.md](../README.md) | Install, quick start, upgrade |
| [user/](user/) | Product help (in-app) |
| [admin-guide.md](admin-guide.md) | Operators |
| [dev/](dev/) | Contributors and agents |
