# Framework examples (Selenide, pytest, Playwright)

End-to-end sample projects under [`examples/`](../../examples/) show how to map
case keys and report results into Vulyk on **every supported CI**:

| Provider | Dispatch from Vulyk | Example CI file (per sample) |
|----------|---------------------|------------------------------|
| GitHub Actions | yes (`workflow_dispatch`) | `ci/github-actions.yml` → `.github/workflows/vulyk.yml` |
| Forgejo / Gitea Actions | yes | `ci/forgejo-actions.yml` |
| GitLab CI | yes | `ci/gitlab-ci.yml` → `.gitlab-ci.yml` |
| Jenkins | yes | `ci/Jenkinsfile` |
| Woodpecker | yes | `ci/woodpecker.yml` → `.woodpecker.yml` |
| Bitbucket Pipelines | **ingest only** | `ci/bitbucket-pipelines.yml` |
| Generic / other | ingest only | use `vulyk-upload.sh --provider other` |

Operator overview (tokens, open → upload → close): [ci-integration.md](./ci-integration.md).
In-app key mapping: [../user/cicd.md](../user/cicd.md).

## Shared prerequisites

1. A Vulyk project with case keys that match the samples (`KOC-12`, `KOC-13`, or
   rename the keys in the tests to your project).
2. A **project CI token** (Settings → Project CI tokens).
3. Secrets / CI variables: `VULYK_API_URL`, `VULYK_API_TOKEN`, optional
   `VULYK_PROJECT_KEY` (defaults to `KOC` in the samples).
4. Copy one sample folder into an automation repo (or point CI at
   `examples/<name>` in this monorepo). Each sample’s `ci/` configs look for
   `./scripts/vulyk-upload.sh`, then `../../scripts/vulyk-upload.sh`, then curl
   the script from `main`.

## Selenide / JUnit 5 (Maven)

Path: [`examples/selenide-maven/`](../../examples/selenide-maven/).

| Step | Command / note |
|------|----------------|
| Run locally | `mvn -B test` (Chrome/Chromium, headless) |
| Results | `target/surefire-reports/TEST-*.xml` |
| Key mapping | `@DisplayName("KOC-12 …")` and `@VulykCaseKey("KOC-12")` → Surefire properties |

```bash
cd examples/selenide-maven
mvn -B test
export VULYK_API_URL=… VULYK_API_TOKEN=… VULYK_PROJECT_KEY=KOC
VULYK_RUN_ID="$(../../scripts/vulyk-upload.sh --open-run --print-run-id \
  --suite-name selenide-local --provider other)"
export VULYK_RUN_ID
../../scripts/vulyk-upload.sh --file target/surefire-reports --format junit
../../scripts/vulyk-upload.sh --close-run
```

Jenkins-oriented variant of the same pattern also lives at
[`ci/Jenkinsfile.selenide`](../../ci/Jenkinsfile.selenide).

## pytest

Path: [`examples/pytest/`](../../examples/pytest/).

| Step | Command / note |
|------|----------------|
| Run locally | `pip install -e . && pytest` |
| Results | `report.xml` (`junit_family=xunit1`) |
| Key mapping | `@pytest.mark.vulyk("KOC-12")` + [`pytest_vulyk.py`](../../examples/pytest/pytest_vulyk.py) for marker skips |

```bash
cd examples/pytest
python3 -m venv .venv && source .venv/bin/activate
pip install -e .
pytest
# then open → upload report.xml → close (same as Selenide snippet)
```

Repo-root plugin (identical behaviour): [`ci/pytest_vulyk.py`](../../ci/pytest_vulyk.py).

## Playwright

Path: [`examples/playwright/`](../../examples/playwright/).

| Step | Command / note |
|------|----------------|
| Run locally | `npm install && npx playwright install chromium && npm test` |
| Results | `results.xml` (Playwright JUnit reporter) |
| Key mapping | `tag: '@KOC-12'` + `annotation: { type: 'vulyk', description: 'KOC-12' }` |

For streaming results with the dogfood reporter (open/start/results/close inside
the test run), see [../dev/testing/playwright-vulyk.md](../dev/testing/playwright-vulyk.md)
and `e2e/reporters/vulyk-reporter.ts`.

## Wire a sample to each CI

1. Copy `examples/<framework>/ci/<provider-file>` to the path your CI expects
   (table above).
2. Add the three Vulyk secrets / variables.
3. Push or run a manual pipeline once; confirm **Executions** shows the suite,
   provider badge, and matched case keys.
4. Optional **Run in CI**: create a CI trigger in Vulyk Settings pointing at the
   workflow / job (not available for Bitbucket or `other`).

Regenerate all sample CI files after editing the shared template logic:

```bash
python3 examples/_gen_ci.py
```

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `unmatched_case_keys` in the upload response | Align test keys with cases in the project, or create the missing cases |
| Empty results upload | Ensure the test step wrote XML even on failure (`\|\| true` / `when: always`) |
| Wrong provider badge | Pass `--provider …` or rely on auto-detect; clear conflicting host CI env vars when simulating |
| Localhost API unreachable from SaaS CI | Use a tunnel or a self-hosted runner — see [ci-integration.md](./ci-integration.md#private--http-ci-urls-j10) |
