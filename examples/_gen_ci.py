#!/usr/bin/env python3
"""Generate examples/*/ci provider configs. Run from repo root: python3 examples/_gen_ci.py"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent

UPLOADER_BLOCK = """\
            set -eu
            if [ -f ./scripts/vulyk-upload.sh ]; then
              UPLOADER=./scripts/vulyk-upload.sh
            elif [ -f ../../scripts/vulyk-upload.sh ]; then
              UPLOADER=../../scripts/vulyk-upload.sh
            else
              curl -fsSL -o vulyk-upload.sh \\
                https://raw.githubusercontent.com/Ihor-Meat-Proxy/Vulyk/main/scripts/vulyk-upload.sh
              chmod +x vulyk-upload.sh
              UPLOADER=./vulyk-upload.sh
            fi
            export VULYK_API_URL VULYK_API_TOKEN
            export VULYK_PROJECT_KEY="${VULYK_PROJECT_KEY:-KOC}"
"""


def report_sh(
    suite: str,
    results: str,
    provider: str,
    branch_expr: str,
    commit_expr: str,
    pipeline_expr: str,
    run_id_expr: str,
    suite_num: str,
) -> str:
    suite_name = f"{suite} #{suite_num}"
    return f"""{UPLOADER_BLOCK}
            if [ -n "{run_id_expr}" ]; then
              "$UPLOADER" --start-run \\
                --branch "{branch_expr}" --commit "{commit_expr}" \\
                --pipeline-url "{pipeline_expr}" || true
            else
              VULYK_RUN_ID="$("$UPLOADER" --open-run --print-run-id \\
                --suite-name "{suite_name}" \\
                --provider {provider} \\
                --branch "{branch_expr}" --commit "{commit_expr}" \\
                --pipeline-url "{pipeline_expr}")"
              export VULYK_RUN_ID
            fi
            if [ -e "{results}" ]; then
              "$UPLOADER" --file "{results}" --format junit
            fi
            "$UPLOADER" --close-run
"""


FRAMEWORKS = {
    "selenide-maven": {
        "results": "target/surefire-reports",
        "suite": "selenide",
        "github_setup": """\
      - uses: actions/setup-java@cf277c60eb25467037889841efdb72551f06f6c3 # v4
        with:
          distribution: temurin
          java-version: '17'
          cache: maven
      - name: Install Chrome
        run: |
          sudo apt-get update -qq
          sudo apt-get install -y -qq chromium-browser || sudo apt-get install -y -qq chromium || true
      - name: Test
        run: mvn -B test || true
""",
        "gitlab_job": """\
  image: maven:3.9-eclipse-temurin-17
  script:
    - apt-get update -qq && apt-get install -y -qq chromium || true
    - mvn -B test || true
""",
        "generic_test": "mvn -B test || true",
        "image": "maven:3.9-eclipse-temurin-17",
        "jenkins_test": "mvn -B test || true",
    },
    "pytest": {
        "results": "report.xml",
        "suite": "pytest",
        "github_setup": """\
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # v5
        with:
          python-version: '3.12'
          cache: pip
      - name: Install
        run: pip install -e .
      - name: Test
        run: pytest || true
""",
        "gitlab_job": """\
  image: python:3.12
  script:
    - pip install -e .
    - pytest || true
""",
        "generic_test": "pip install -e . && pytest || true",
        "image": "python:3.12",
        "jenkins_test": "pip install -e . && pytest || true",
    },
    "playwright": {
        "results": "results.xml",
        "suite": "playwright",
        "github_setup": """\
      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4
        with:
          node-version: '20'
          cache: npm
      - name: Install
        run: |
          npm ci || npm install
          npx playwright install --with-deps chromium
      - name: Test
        run: npx playwright test || true
""",
        "gitlab_job": """\
  image: mcr.microsoft.com/playwright:v1.49.0-jammy
  script:
    - npm ci || npm install
    - npx playwright test || true
""",
        "generic_test": (
            "npm ci || npm install; "
            "npx playwright install --with-deps chromium; "
            "npx playwright test || true"
        ),
        "image": "mcr.microsoft.com/playwright:v1.49.0-jammy",
        "jenkins_test": (
            "npm ci || npm install && "
            "npx playwright install --with-deps chromium && "
            "npx playwright test || true"
        ),
    },
}


def write_github(ci: Path, suite: str, results: str, setup: str, provider: str) -> None:
    name = "github-actions.yml" if provider == "github" else "forgejo-actions.yml"
    provider_env = (
        ""
        if provider == "github"
        else "\n          VULYK_CI_PROVIDER: forgejo"
    )
    header = (
        "# Copy to .github/workflows/vulyk.yml in the automation repo.\n"
        if provider == "github"
        else (
            "# Copy to .forgejo/workflows/vulyk.yml (Forgejo) or "
            ".gitea/workflows/vulyk.yml (Gitea).\n"
            "# Set VULYK_CI_PROVIDER to forgejo or gitea to match the trigger.\n"
        )
    )
    body = report_sh(
        suite,
        results,
        provider,
        "${GITHUB_REF_NAME}",
        "${GITHUB_SHA}",
        "${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}",
        "${VULYK_RUN_ID:-}",
        "${GITHUB_RUN_NUMBER}",
    )
    text = f"""{header}# Secrets: VULYK_API_URL, VULYK_API_TOKEN, VULYK_PROJECT_KEY (optional).

name: Vulyk {suite}

on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      vulyk_run_id:
        description: Existing Vulyk run id (from Run in CI)
        required: false
        type: string
        default: ''

jobs:
  test-and-report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4
{setup.rstrip()}
      - name: Report to Vulyk
        if: always()
        env:
          VULYK_API_URL: ${{{{ secrets.VULYK_API_URL }}}}
          VULYK_API_TOKEN: ${{{{ secrets.VULYK_API_TOKEN }}}}
          VULYK_PROJECT_KEY: ${{{{ secrets.VULYK_PROJECT_KEY }}}}
          VULYK_RUN_ID: ${{{{ inputs.vulyk_run_id }}}}{provider_env}
        run: |
{body}
"""
    (ci / name).write_text(text)


def write_gitlab(ci: Path, suite: str, results: str, gitlab_job: str) -> None:
    body = report_sh(
        suite,
        results,
        "gitlab",
        "${CI_COMMIT_REF_NAME}",
        "${CI_COMMIT_SHA}",
        "${CI_PIPELINE_URL}",
        "${VULYK_RUN_ID:-}",
        "${CI_PIPELINE_ID}",
    )
    text = f"""# Copy into .gitlab-ci.yml (or include remotely). Masked vars:
# VULYK_API_URL, VULYK_API_TOKEN, VULYK_PROJECT_KEY

stages: [test, report]

test:
  stage: test
{gitlab_job.rstrip()}
  artifacts:
    when: always
    paths:
      - {results}
    expire_in: 1 day

report-vulyk:
  stage: report
  image: python:3.12-alpine
  needs: [test]
  when: always
  before_script:
    - apk add --no-cache curl bash
  script:
    - |
{body}
"""
    (ci / "gitlab-ci.yml").write_text(text)


def write_jenkins(ci: Path, suite: str, results: str, jenkins_test: str) -> None:
    body = report_sh(
        suite,
        results,
        "jenkins",
        "${GIT_BRANCH#origin/}",
        "${GIT_COMMIT}",
        "${BUILD_URL}",
        "${VULYK_RUN_ID:-}",
        "${BUILD_NUMBER}",
    )
    # Use .format with doubled braces for Groovy/Jenkins interpolations we want literal.
    text = """\
// Jenkins Declarative Pipeline for the {suite} example.
// Credentials id: vulyk-ci-token -> secret text for VULYK_API_TOKEN.

pipeline {{
  agent any
  parameters {{
    string(name: 'VULYK_RUN_ID', defaultValue: '', description: 'Existing Vulyk run id from Dispatch')
  }}
  environment {{
    VULYK_API_URL     = "${{env.VULYK_API_URL}}"
    VULYK_API_TOKEN   = credentials('vulyk-ci-token')
    VULYK_PROJECT_KEY = "${{env.VULYK_PROJECT_KEY ?: 'KOC'}}"
  }}
  stages {{
    stage('Checkout') {{ steps {{ checkout scm }} }}
    stage('Test') {{
      steps {{
        sh '''
          set -eu
          {jenkins_test}
        '''
      }}
    }}
    stage('Report to Vulyk') {{
      steps {{
        sh '''
{body}
        '''
      }}
    }}
  }}
}}
""".format(suite=suite, jenkins_test=jenkins_test, body=body)
    (ci / "Jenkinsfile").write_text(text)


def write_woodpecker(ci: Path, suite: str, results: str, image: str, generic_test: str) -> None:
    body = report_sh(
        suite,
        results,
        "woodpecker",
        "$${CI_COMMIT_BRANCH:-$${CI_COMMIT_SOURCE_BRANCH:-main}}",
        "$${CI_COMMIT_SHA}",
        "$${CI_PIPELINE_URL}",
        "$${VULYK_RUN_ID:-}",
        "$${CI_PIPELINE_NUMBER}",
    )
    text = f"""# Save as .woodpecker.yml. Secrets: VULYK_API_URL, VULYK_API_TOKEN, VULYK_PROJECT_KEY.
# Woodpecker interpolates ${{VAR}} — use $$ so the shell sees ${{VAR}}.

when:
  - event: [push, manual]
    branch: [main]

steps:
  - name: test
    image: {image}
    commands:
      - |
        set -eu
        {generic_test}
  - name: report-vulyk
    image: alpine:3.20
    depends_on: [test]
    when:
      - status: [success, failure]
    commands:
      - |
        set -eu
        apk add --no-cache curl bash >/dev/null
{body}
"""
    (ci / "woodpecker.yml").write_text(text)


def write_bitbucket(ci: Path, suite: str, results: str, image: str, generic_test: str) -> None:
    body = report_sh(
        suite,
        results,
        "bitbucket",
        "${BITBUCKET_BRANCH}",
        "${BITBUCKET_COMMIT}",
        "https://bitbucket.org/${BITBUCKET_REPO_FULL_NAME}/pipelines/results/${BITBUCKET_BUILD_NUMBER}",
        "${VULYK_RUN_ID:-}",
        "${BITBUCKET_BUILD_NUMBER}",
    )
    text = f"""# Bitbucket Pipelines (ingest only — no Vulyk → Bitbucket dispatch).
# Repository variables: VULYK_API_URL, VULYK_API_TOKEN, VULYK_PROJECT_KEY.

image: {image}

pipelines:
  default:
    - step:
        name: test-and-report
        script:
          - |
            set -eu
            {generic_test}
          - |
{body}
"""
    (ci / "bitbucket-pipelines.yml").write_text(text)


def main() -> None:
    for fw, meta in FRAMEWORKS.items():
        ci = ROOT / fw / "ci"
        ci.mkdir(parents=True, exist_ok=True)
        suite = meta["suite"]
        results = meta["results"]
        write_github(ci, suite, results, meta["github_setup"], "github")
        write_github(ci, suite, results, meta["github_setup"], "forgejo")
        write_gitlab(ci, suite, results, meta["gitlab_job"])
        write_jenkins(ci, suite, results, meta["jenkins_test"])
        write_woodpecker(ci, suite, results, meta["image"], meta["generic_test"])
        write_bitbucket(ci, suite, results, meta["image"], meta["generic_test"])
        print("wrote", fw, "->", ci)


if __name__ == "__main__":
    main()
