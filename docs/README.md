# Vulyk documentation

Index of guides and references. Prefer these over outdated workstream notes.

## Start here

| Doc | Audience |
|-----|----------|
| [../README.md](../README.md) | Install, quick start, repo layout |
| [user-guide.md](user-guide.md) | Testers and day-to-day workflows |
| [admin-guide.md](admin-guide.md) | Operators: deploy, backups, Admin UI |
| [architecture.md](architecture.md) | System shape, modules, data model |
| [api.md](api.md) | HTTP API overview (OpenAPI at `/api/docs`) |
| [auth-and-rbac.md](auth-and-rbac.md) | Auth modes, JWT/sessions/tokens, RBAC |

## Deploy & operations

| Doc | Topic |
|-----|-------|
| [guides/first-run-setup.md](guides/first-run-setup.md) | Compose / Helm / CLI / wizard |
| [guides/upgrade.md](guides/upgrade.md) | Safe upgrades, verified backups, migrate CLI |
| [guides/in-app-updates.md](guides/in-app-updates.md) | Admin Updates tab + updater sidecar |
| [guides/releasing.md](guides/releasing.md) | Version bump, tags, GHCR, install `--version` |
| [guides/backup.md](guides/backup.md) | Content backup / restore (ZIP destinations) |
| [backup-design.md](backup-design.md) | Content backup architecture (Mermaid) |
| [guides/sso.md](guides/sso.md) | Google, Entra, GitHub, GitLab, Nextcloud |

## CI, automation & integrations

| Doc | Topic |
|-----|-------|
| [guides/ci-integration.md](guides/ci-integration.md) | Preferred CI reporting (open → upload → close) |
| [guides/ingest-and-ci.md](guides/ingest-and-ci.md) | Formats, dispatch, webhooks |
| [guides/executions.md](guides/executions.md) | Campaigns, iterations, pipeline upload |
| [automation-repo.md](automation-repo.md) | Multi-repo Git index for Generate |
| [integrations/](integrations/) | OpenProject, Jira, importers |
| [mcp.md](mcp.md) | MCP server for AI assistants |
| [llm.md](llm.md) | LLM providers, generate, chat |

## Product & UX

| Doc | Topic |
|-----|-------|
| [guides/pwa-and-mobile.md](guides/pwa-and-mobile.md) | PWA, offline queue, mobile UX |
| [design-tokens.md](design-tokens.md) | Themes, appearance, locale prefs |

## Contributors

| Doc | Topic |
|-----|-------|
| [../AGENTS.md](../AGENTS.md) | Agent / PR guardrails |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Hooks, migrations, i18n, test ids |
| [ci-failures.md](ci-failures.md) | CI failure catalog |
| [testing/test-ids.md](testing/test-ids.md) | Playwright `data-testid` conventions |
| [testing/playwright-vulyk.md](testing/playwright-vulyk.md) | Map e2e tests to VQA cases + optional CI reporter |
| [../backend/seed/vulyk-qa.d/README.md](../backend/seed/vulyk-qa.d/README.md) | QA seed fragments |

## Dated audits (October 2026)

Keep these as historical records. Finding bodies describe pre-fix code; status tables and fix-PR lists record remediation.

| Audit | Path |
|-------|------|
| Security | [security/audit-2026-10.md](security/audit-2026-10.md) |
| Code quality | [quality/code-quality-2026-10.md](quality/code-quality-2026-10.md) |
| Senior code review | [review/code-review-2026-10.md](review/code-review-2026-10.md) |

## Historical / superseded notes

| Doc | Status |
|-----|--------|
| [automation-integration-design.md](automation-integration-design.md) | Design archive → use CI + automation-repo guides |
| [automation-integration-comparison.md](automation-integration-comparison.md) | Vendor comparison archive |
| [w2-w3-cases-runs.md](w2-w3-cases-runs.md) | Workstream delivery note → user-guide / api |
