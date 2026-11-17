#!/usr/bin/env bash
# Prove .npmrc min-release-age=7 does not block the security fast-path override.
# Part of #294 - run from repo root (also wired into ./scripts/check.sh --quick).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing $1"; }

need_cmd npm
need_cmd python3
need_cmd grep

for rc in frontend/.npmrc e2e/.npmrc; do
  grep -Eq '^[[:space:]]*min-release-age[[:space:]]*=[[:space:]]*7[[:space:]]*$' "${rc}" \
    || die "${rc} must set min-release-age=7"
  grep -Eq '^[[:space:]]*ignore-scripts[[:space:]]*=[[:space:]]*true[[:space:]]*$' "${rc}" \
    || die "${rc} must set ignore-scripts=true"
done

# Project .npmrc is active for package-local npm config.
age="$(cd frontend && npm config get min-release-age)"
[[ "${age}" == "7" ]] || die "expected frontend min-release-age=7 from .npmrc, got '${age}'"

# CLI overrides project config (security fast path).
overridden="$(cd frontend && npm config get min-release-age --min-release-age=0)"
[[ "${overridden}" == "0" ]] || die "expected --min-release-age=0 to override .npmrc, got '${overridden}'"

# Helper: default off; explicit on; Dependabot security event on.
[[ "$(VULYK_NPM_SECURITY_FAST_PATH=0 ./scripts/npm-security-fast-path.sh)" == "0" ]] \
  || die "explicit 0 should disable fast path"
[[ "$(VULYK_NPM_SECURITY_FAST_PATH=1 ./scripts/npm-security-fast-path.sh)" == "1" ]] \
  || die "explicit 1 should enable fast path"

TMP="$(mktemp)"
cat >"${TMP}" <<'JSON'
{
  "pull_request": {
    "user": { "login": "dependabot[bot]" },
    "title": "Bump leftpad from 1.0.0 to 1.0.1",
    "body": "Bumps leftpad to address a security vulnerability.\n\nGHSA-xxxx-xxxx-xxxx",
    "labels": [{ "name": "dependencies" }, { "name": "security" }]
  }
}
JSON
got="$(
  unset VULYK_NPM_SECURITY_FAST_PATH
  GITHUB_ACTOR='dependabot[bot]' GITHUB_EVENT_PATH="${TMP}" \
    ./scripts/npm-security-fast-path.sh
)"
rm -f "${TMP}"
[[ "${got}" == "1" ]] || die "Dependabot security event should enable fast path, got '${got}'"

# npm-ci-secure.sh must pass --min-release-age=0 when fast path is on.
# Use a dry wrapper: parse the script's planned argv via a fake npm on PATH.
FAKE_BIN="$(mktemp -d)"
cleanup() { rm -rf "${FAKE_BIN}"; }
trap cleanup EXIT
cat >"${FAKE_BIN}/npm" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"${NPM_ARGV_LOG}"
# Satisfy "npm ci" without network when called for real args.
exit 0
EOF
chmod +x "${FAKE_BIN}/npm"
NPM_ARGV_LOG="$(mktemp)"
export NPM_ARGV_LOG
# Empty allowlist rebuild skipped; package dirs exist.
PATH="${FAKE_BIN}:${PATH}" VULYK_NPM_SECURITY_FAST_PATH=1 \
  ./scripts/npm-ci-secure.sh frontend >/dev/null
grep -q -- '--min-release-age=0' "${NPM_ARGV_LOG}" \
  || die "npm-ci-secure.sh did not pass --min-release-age=0 under security fast path (argv: $(cat "${NPM_ARGV_LOG}"))"
grep -q -- 'ci' "${NPM_ARGV_LOG}" \
  || die "npm-ci-secure.sh did not invoke npm ci"

PATH="${FAKE_BIN}:${PATH}" VULYK_NPM_SECURITY_FAST_PATH=0 \
  ./scripts/npm-ci-secure.sh frontend >/dev/null
if grep -q -- '--min-release-age=0' "${NPM_ARGV_LOG}"; then
  die "npm-ci-secure.sh must NOT pass --min-release-age=0 when fast path is off"
fi

echo "OK: npm security fast path overrides min-release-age=7"
