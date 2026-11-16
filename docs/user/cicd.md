# CI/CD integration

Publish automated results into Vulyk and optionally start pipelines from **Executions → Run in CI**. Preferred lifecycle: **open run → upload results → close run** (including on failure).

## Prerequisites

1. A Vulyk project with case keys (for example `KOC-12` or `VQA-42`).
2. Tests that embed the key in the **title**, **annotation/tag**, or JUnit **property**.
3. A **project CI token** (`vulyk_ci_…`) from **Settings → Project CI tokens** (Admin or Test Automation Engineer). Copy the secret once; it is stored hashed.
4. Optional: link **Test automation repositories** under Settings so Generate can index the same remotes your CI builds, and so **Run in CI** can dispatch workflows.

Store the token as `VULYK_CI_TOKEN` or `VULYK_API_TOKEN`, plus `VULYK_API_URL` and `VULYK_PROJECT_KEY` / `VULYK_PROJECT_ID`, in your CI secrets.

CI tokens are narrowly scoped to reporting (open / upload / close) for that project. Prefer them over personal `vulyk_at_…` tokens in pipelines.

## Mapping case keys

### Playwright (TypeScript)

Preferred: annotation (also accepted as tag `@VQA-123` / `@KEY`):

```ts
import { test, expect } from '@playwright/test';

test('Login with valid password', {
  annotation: { type: 'vulyk', description: 'KOC-12' },
}, async ({ page }) => {
  // …
});

// Or put the key in the title:
test('KOC-12 Login with valid password', async ({ page }) => {
  // …
});

// Tag form understood by the Vulyk Playwright reporter:
test('Folder tree expands', {
  tag: '@VQA-42',
  annotation: { type: 'vulyk', description: 'VQA-42' },
}, async ({ page }) => {
  // …
});
```

The built-in reporter (`e2e/reporters/vulyk-reporter.ts`) reads `{ type: 'vulyk', description: 'KEY' }` (or `@KEY` tags), opens or attaches a run on `onBegin`, streams results on `onTestEnd`, and closes on `onEnd`.

Environment variables:

| Variable | Purpose |
|----------|---------|
| `VULYK_API_URL` | API base URL |
| `VULYK_CI_TOKEN` / `VULYK_API_TOKEN` | Bearer token |
| `VULYK_PROJECT_KEY` / `VULYK_PROJECT_ID` | Target project |
| `VULYK_RUN_ID` | Attach to an existing run (Run in CI) instead of opening a new one |
| `VULYK_REPORTER_DRY_RUN=1` | Log open/start/results/close without POSTing |

### Selenide / JUnit 5

Emit Surefire/Gradle JUnit XML and set a property recognized by Vulyk (`test_key`, `case_key`, or `vulyk_key`). Keys in the test name / `@DisplayName` also work; `@vulyk:KOC-12` in the name is recognized.

```java
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.extension.ExtensionContext;
import org.junit.jupiter.api.extension.TestWatcher;
import org.opentest4j.TestAbortedException;

// Example: put the Vulyk key in the display name and/or a custom reporter
// that writes <property name="test_key" value="KOC-12"/>.
@DisplayName("KOC-12 Login with valid password")
class LoginTest {
  @Test
  void validPassword() {
    // Selenide steps…
  }
}
```

JUnit XML shape Vulyk maps:

```xml
<testcase classname="tests.Login" name="valid_password" time="1.2">
  <properties>
    <property name="test_key" value="KOC-12"/>
  </properties>
</testcase>
```

Compatible with Xray-style `test_key` / TestRail-style `test_id` property names. Unknown keys appear in `unmatched_case_keys` in the API response — fix the test or create the case, then re-run.

### pytest

Put the key in the node id / test name, or set metadata for pytest-json-report:

```python
import pytest

@pytest.mark.vulyk("KOC-12")
def test_login_valid_password():
    ...

# pytest-json-report: metadata.case_key is supported when using format=pytest_json
def test_health(record_property):
    record_property("case_key", "KOC-12")
    assert True
```

Upload either JUnit XML (`--junitxml=report.xml`) or pytest JSON with `--format pytest_json`.

### Generic JUnit XML via curl / shell

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_ci_…
export VULYK_PROJECT_KEY=KOC

# Open → upload → close (scripts/vulyk-upload.sh)
./scripts/vulyk-upload.sh --open-run \
  --suite-name "Nightly #${BUILD_NUMBER:-local}" \
  --provider other \
  --branch "${GIT_BRANCH:-main}" \
  --commit "${GIT_COMMIT:-}"

./scripts/vulyk-upload.sh --file results.xml --format junit --run-id "$VULYK_RUN_ID"
./scripts/vulyk-upload.sh --close-run --run-id "$VULYK_RUN_ID"
```

One-shot pipeline ingest (auto campaign / iteration / suite):

```bash
./scripts/vulyk-upload.sh \
  --api-url "$VULYK_API_URL" \
  --token "$VULYK_API_TOKEN" \
  --project-key KOC \
  --pipeline \
  --campaign Integration_Automation \
  --iteration "Daily - main" \
  --created-by "$USER" \
  --file results.xml \
  --format junit
```

Supported `--format` values: `auto`, `junit`, `vulyk_json`, `playwright_json`, `pytest_json`.

## Start a run from the pipeline vs report into an existing run

| Mode | How | When to use |
|------|-----|-------------|
| Pipeline opens the run | Omit `VULYK_RUN_ID`; use `--open-run` / action `open-run: true` | Standard CI job that owns the suite lifecycle |
| Attach to existing run | Set `VULYK_RUN_ID` (or action `run-id`) from **Run in CI** | Vulyk dispatched the workflow; keep one run id end-to-end |
| Legacy create | `--create-run` / CLI `--create-run` | Older one-shot uploads into a plain run |
| Pipeline ingest | `--pipeline` + campaign/iteration | Auto-create Executions tree nodes |

Always **close** the run (`--close-run` or action `close: true`, with `if: always()` / `when: always`) so failed jobs still finalize status. Use `--cancelled` when the job was interrupted.

Use a stable `--external-ref` (the GitHub Action sets `github:repo:run_id:attempt` by default) to avoid duplicate opens.

## GitHub Actions

### Composite action (recommended)

```yaml
# .github/workflows/e2e.yml
jobs:
  playwright:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci && npx playwright test
        id: tests
        continue-on-error: true

      - name: Report to Vulyk
        if: always()
        uses: ./.github/actions/vulyk-report
        # or: uses: Ihor-Meat-Proxy/Vulyk/.github/actions/vulyk-report@main
        with:
          api-url: ${{ vars.VULYK_API_URL }}
          api-token: ${{ secrets.VULYK_CI_TOKEN }}
          project-key: KOC
          file: playwright-report/results.xml
          format: junit
          suite-name: Playwright ${{ github.workflow }} #${{ github.run_number }}
          close: 'true'
```

Inputs include optional `run-id` (attach), `campaign` / `iteration`, and `cancelled`.

### Shell-only in Actions

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_ci_…
export VULYK_PROJECT_KEY=KOC

./scripts/vulyk-upload.sh --open-run \
  --suite-name "Playwright #$GITHUB_RUN_NUMBER" \
  --provider github --repo "$GITHUB_REPOSITORY" \
  --branch "${GITHUB_REF_NAME}" --commit "$GITHUB_SHA" \
  --pipeline-url "${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"

./scripts/vulyk-upload.sh --file results.xml --format junit --run-id "$VULYK_RUN_ID"
./scripts/vulyk-upload.sh --close-run --run-id "$VULYK_RUN_ID"
```

A thinner one-shot Action also exists at `action/action.yml` for simple uploads.

## GitLab CI

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/Ihor-Meat-Proxy/Vulyk/main/ci/gitlab-ci-vulyk.yml'
  # or vendor: local: 'ci/gitlab-ci-vulyk.yml'

variables:
  VULYK_API_URL: "https://vulyk.example.com"
  VULYK_PROJECT_KEY: "KOC"
  VULYK_RESULTS_FILE: "report.xml"
  # Set VULYK_API_TOKEN in CI/CD variables (masked)

stages: [test, report]

playwright:
  stage: test
  script:
    - npm ci && npx playwright test
  artifacts:
    when: always
    paths: [report.xml]
  allow_failure: true

report-vulyk:
  stage: report
  extends: .vulyk_report
  needs: [playwright]
  when: always
```

The `.vulyk_report` job opens a run with GitLab pipeline/job URLs, uploads `$VULYK_RESULTS_FILE`, and closes the run. An alternate template is `ci/gitlab-upload.yml` (`.vulyk_upload`).

## Jenkins

There is no Jenkins-specific plugin required. Call the same shell uploader from a pipeline stage (ensure `curl`/bash and the results file are available):

```groovy
pipeline {
  agent any
  environment {
    VULYK_API_URL   = 'https://vulyk.example.com'
    VULYK_API_TOKEN = credentials('vulyk-ci-token')
    VULYK_PROJECT_KEY = 'KOC'
  }
  stages {
    stage('Test') {
      steps {
        sh 'mvn -B test || true'  // keep XML even on failures
      }
    }
    stage('Report to Vulyk') {
      steps {
        sh '''
          curl -fsSL -o vulyk-upload.sh \
            https://raw.githubusercontent.com/Ihor-Meat-Proxy/Vulyk/main/scripts/vulyk-upload.sh
          chmod +x vulyk-upload.sh
          ./vulyk-upload.sh --open-run \
            --suite-name "Jenkins #${BUILD_NUMBER}" \
            --provider other --branch "${GIT_BRANCH}" --commit "${GIT_COMMIT}" \
            --pipeline-url "${BUILD_URL}"
          ./vulyk-upload.sh --file target/surefire-reports/junitreports/TEST-*.xml \
            --format junit --run-id "$VULYK_RUN_ID" || \
          ./vulyk-upload.sh --file target/surefire-reports/*.xml \
            --format junit --run-id "$VULYK_RUN_ID"
          ./vulyk-upload.sh --close-run --run-id "$VULYK_RUN_ID"
        '''
      }
    }
  }
}
```

Prefer vendoring `scripts/vulyk-upload.sh` in your automation repo instead of downloading each build.

## CLI reporter

```bash
cargo run -p vulyk-cli -- report \
  --file report.xml --format junit \
  --create-run --project-key DEMO \
  --api-url https://vulyk.example.com
```

Environment: `VULYK_API_URL`, `VULYK_API_TOKEN`, `VULYK_RUN_ID`, `VULYK_PROJECT_KEY`, `VULYK_PROJECT_ID`, `VULYK_RUN_NAME`.

## Vulyk JSON format

```json
{
  "format": "vulyk/results@v1",
  "run": { "name": "optional run name hint" },
  "results": [
    {
      "external_id": "tests.test_api::test_health",
      "case_key": "DEMO-1",
      "title": "Health endpoint responds",
      "status": "passed",
      "message": "",
      "duration_ms": 12,
      "executed_at": "2026-10-25T12:00:00Z"
    }
  ]
}
```

Statuses: `passed`, `failed`, `skipped`, `error`, `blocked` (aliases like `pass` / `fail` are normalized).

## Run in CI from Vulyk

1. Link a **GitHub** automation repo under **Settings → Test automation repositories**.
2. Set **Actions write token** (`actions:write`) — separate from the read/index token.
3. Set **CI workflow file** (for example `.github/workflows/vulyk-dispatch-example.yml`) and **ref**.
4. Copy the example workflow that declares `workflow_dispatch` inputs `vulyk_run_id` and `case_keys`.
5. From **Executions**, click **Run in CI**. Set `VULYK_RUN_ID` in the job from the input so results attach to the queued run.

Localhost Vulyk is not reachable from GitHub-hosted runners — use a public tunnel or a self-hosted runner and set `VULYK_API_URL` accordingly.

Stale **Queued** runs fail after `VULYK_CI_QUEUED_TIMEOUT_SECS` (default 7200). Hard-killed reporters leaving `in_progress` are auto-cancelled after `VULYK_STALE_RUN_TTL_HOURS` (default 24).

### Webhooks

GitHub/GitLab **webhook secrets are required** (fail-closed) on CI configs. Prefer config-scoped URLs. API responses expose `has_webhook_secret` / `has_token` only — never the raw secret.

## Automation repositories

Under **Settings → Test automation repositories**:

| Field | Notes |
|-------|--------|
| Provider | `github`, `gitlab`, or `generic` |
| Remote URL | Public HTTPS (SSRF-guarded) |
| Branch | Dropdown lists remote branches; **Refresh branches**; newly seen names marked |
| Token | Encrypted; raises GitHub rate limits vs anonymous |
| Actions write token / workflow | For Run in CI |
| Poll interval | Default 15 minutes; `0` disables |
| Webhook URL / secret | Push-driven re-index after save |
| Projects | Multi-select; one repo can attach to many projects |

**Test connection**, **Re-index**, **Rotate secret**. GitHub/GitLab index via archive download (no local clone); generic HTTPS uses a shallow clone. Generate uses the index for BM25 context when producing automation code.

## Troubleshooting CI

| Symptom | Check |
|---------|--------|
| `401` / `403` | Token revoked? Wrong project? Missing `upload_results`? |
| Empty suite | Results path; `format`; job `if: always()` / `when: always` |
| `unmatched_case_keys` | Typo; case not in project; use `test_key` or Playwright annotation / `@KEY` |
| Duplicate runs | Stable `--external-ref` |
| Posts after close | Open a new run; closed runs reject new results |
| Git host rate limit | Add a read token on the automation repo; wait until reset time shown in Settings |

Interactive API docs: [/api/docs](/api/docs).
