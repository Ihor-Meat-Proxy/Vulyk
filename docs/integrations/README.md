# Integrations

Vulyk integrates with ALM tools, CI systems, and case importers without forking those products.

| Integration | Doc |
|-------------|-----|
| OpenProject (requirements / defects) | [openproject.md](openproject.md) |
| Jira Cloud / Server-DC | [jira.md](jira.md) |
| Case importers (TestLink, TestRail, Xray, …) | [importers.md](importers.md) |
| CI result reporting (GitHub / GitLab) | [../guides/ci-integration.md](../guides/ci-integration.md) |
| CI ingest & triggers (legacy one-shot) | [../guides/ingest-and-ci.md](../guides/ingest-and-ci.md) |
| Design: multi-repo + CI API | [../automation-integration-design.md](../automation-integration-design.md) |
| Comparison: SquashTM / TestRail / Xray | [../automation-integration-comparison.md](../automation-integration-comparison.md) |

Configure project integrations in the UI (**Admin** / project settings) or via `/api/v1/projects/{id}/integrations`.
