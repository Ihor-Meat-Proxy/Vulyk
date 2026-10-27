# Automation integration — vendor comparison (archive)

> **Historical notes** on conventions borrowed from SquashTM, TestRail, and Xray
> (case-key properties, campaign/iteration trees, CI upload patterns).

**Operators:** follow [guides/ci-integration.md](guides/ci-integration.md) and
[guides/executions.md](guides/executions.md). Design archive:
[automation-integration-design.md](automation-integration-design.md).

### Conventions Vulyk adopted

| Source | Idea in Vulyk |
|--------|----------------|
| Xray / TestRail | Case keys in titles, annotations, or JUnit `test_key` / `case_key` properties |
| SquashTM | Campaign → Iteration → Automated suite tree on **Executions** |
| Common CI practice | Open run → upload results → close (always), project-scoped CI tokens (`vulyk_ci_…`) |

Mapping details and Action/GitLab templates: [guides/ci-integration.md](guides/ci-integration.md#mapping-case-keys).
