# Vulyk framework examples

Minimal, runnable sample projects that map case keys and report results into
Vulyk. Use them as a template for your automation repo.

| Sample | Stack | Results format | Guide |
|--------|-------|----------------|-------|
| [`selenide-maven/`](selenide-maven/) | Java 17 · Maven · Selenide · JUnit 5 | Surefire JUnit XML | [Framework examples](../docs/guides/framework-examples.md#selenide--junit-5-maven) |
| [`pytest/`](pytest/) | Python 3.11+ · pytest | JUnit XML (`junit_family=xunit1`) | [Framework examples](../docs/guides/framework-examples.md#pytest) |
| [`playwright/`](playwright/) | Node 20+ · Playwright Test | JUnit XML reporter | [Framework examples](../docs/guides/framework-examples.md#playwright) |

Each sample includes **CI configs for every supported provider** under `ci/`:

| File | Provider |
|------|----------|
| `ci/github-actions.yml` | GitHub Actions |
| `ci/forgejo-actions.yml` | Forgejo / Gitea Actions |
| `ci/gitlab-ci.yml` | GitLab CI |
| `ci/Jenkinsfile` | Jenkins |
| `ci/woodpecker.yml` | Woodpecker |
| `ci/bitbucket-pipelines.yml` | Bitbucket Pipelines |

Copy the matching file into your automation repo (rename to the provider’s
expected path), set `VULYK_API_URL` / `VULYK_API_TOKEN` / `VULYK_PROJECT_KEY`,
and point Vulyk **CI triggers** at the workflow when you want **Run in CI**.

Shared upload helpers and provider templates also live in the repo root
[`ci/`](../ci/) and [`scripts/vulyk-upload.sh`](../scripts/vulyk-upload.sh).
Operator overview: [`docs/guides/ci-integration.md`](../docs/guides/ci-integration.md).
In-app key mapping notes: [`docs/user/cicd.md`](../docs/user/cicd.md).
