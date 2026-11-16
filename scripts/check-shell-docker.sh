#!/usr/bin/env bash
# Shellcheck (scripts + extracted CI/Jenkins snippets) + hadolint (Dockerfiles).
# Local mirror of the Fast CI gate "Shellcheck + hadolint".
# Usage: ./scripts/check-shell-docker.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing '$1' (install: $2)" >&2
    exit 1
  fi
}

need_cmd shellcheck "https://github.com/koalaman/shellcheck#installing"
need_cmd hadolint "https://github.com/hadolint/hadolint#install"
need_cmd python3 "python3"

SCRIPTS=(
  scripts/install.sh
  scripts/perf-smoke.sh
  scripts/vulyk-upload.sh
  scripts/test-vulyk-upload.sh
  scripts/check-migrations.sh
  scripts/lint-migrations-squawk.sh
  scripts/test-migration-lint.sh
  scripts/ci-migrations-verify.sh
  scripts/ci-migrations-reversibility.sh
  scripts/check-seed-keys.sh
  scripts/check-e2e-case-keys.sh
  scripts/check.sh
  scripts/ci-upgrade-test.sh
  scripts/ci-env-upgrade-test.sh
  scripts/test-install-backup-capture.sh
  scripts/check-version.sh
  scripts/check-security.sh
  scripts/check-quality.sh
  scripts/check-api-coverage.sh
  scripts/check-doc-links.sh
  scripts/e2e-local.sh
  scripts/check-shell-docker.sh
  scripts/check-pr-hygiene.sh
)

# Newly added fast-gate helpers (optional until PR2/PR3 land).
for optional in \
  scripts/check-openapi-drift.sh \
  scripts/generate-openapi.sh \
  scripts/check-bundle-size.sh
do
  if [[ -f "${optional}" ]]; then
    SCRIPTS+=("${optional}")
  fi
done

echo "==> bash -n"
for s in "${SCRIPTS[@]}"; do
  bash -n "${s}"
done
bash -n scripts/githooks/pre-commit
bash -n scripts/githooks/pre-push

echo "==> shellcheck (bash scripts)"
shellcheck -x "${SCRIPTS[@]}"

SNIPPET_DIR="$(mktemp -d)"
trap 'rm -rf "${SNIPPET_DIR}"' EXIT

echo "==> extract + shellcheck CI / Jenkins snippets (bash, severity>=warning)"
python3 - "${SNIPPET_DIR}" <<'PY'
import re, sys
from pathlib import Path

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
files = []

# Jenkins Groovy: sh '''...''' / sh """..."""
for jf in sorted(Path("ci").glob("Jenkinsfile*")):
    text = jf.read_text(encoding="utf-8")
    blocks = re.findall(r"sh\s+'''(.*?)'''", text, re.S)
    blocks += re.findall(r'sh\s+"""(.*?)"""', text, re.S)
    for i, body in enumerate(blocks):
        p = out / f"{jf.name}-{i}.sh"
        p.write_text("#!/usr/bin/env bash\n" + body.strip() + "\n", encoding="utf-8")
        files.append(p)

# GitLab CI YAML multiline script blocks under "script:" (best-effort).
for yml in sorted(Path("ci").glob("gitlab*.yml")):
    text = yml.read_text(encoding="utf-8")
    # Capture indented blocks after "script:" that look like shell (set -e / curl / bash).
    for i, m in enumerate(re.finditer(r"(?m)^(\s*)script:\s*\n((?:\1  .*\n)+)", text)):
        block = m.group(2)
        lines = []
        for line in block.splitlines():
            stripped = re.sub(r"^\s*-\s*\|?\s?", "", line)
            # Drop pure YAML list markers that are single commands already stripped
            if stripped.startswith("- "):
                stripped = stripped[2:]
            lines.append(stripped)
        body = "\n".join(lines).strip()
        if not body or "set -" not in body and "curl" not in body and "bash" not in body:
            # Prefer blocks that look like real shell programs
            if "vulyk-upload" not in body and "VULYK_" not in body:
                continue
        p = out / f"{yml.name}-{i}.sh"
        p.write_text("#!/usr/bin/env bash\n" + body + "\n", encoding="utf-8")
        files.append(p)

print(f"extracted {len(files)} snippet(s)")
for p in files:
    print(p)
PY

mapfile -t SNIPPETS < <(find "${SNIPPET_DIR}" -name '*.sh' -type f | sort)
if [[ "${#SNIPPETS[@]}" -gt 0 ]]; then
  # -S warning: ignore SC2012 info noise in example upload snippets.
  shellcheck -s bash -S warning "${SNIPPETS[@]}"
else
  echo "warning: no CI/Jenkins shell snippets extracted" >&2
fi

echo "==> hadolint"
hadolint \
  --config .hadolint.yaml \
  backend/Dockerfile \
  frontend/Dockerfile \
  deploy/updater/Dockerfile

echo "OK: ./scripts/check-shell-docker.sh passed"
