#!/usr/bin/env bash
# Lint docs/api/openapi.json with Stoplight Spectral (.spectral.yaml).
# Used by CI (OpenAPI drift job) and ./scripts/check.sh (non-quick).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

SPEC="${OPENAPI_SPEC:-docs/api/openapi.json}"
RULES="${SPECTRAL_RULES:-.spectral.yaml}"

if [[ ! -f "${SPEC}" ]]; then
  echo "error: missing OpenAPI document at ${SPEC}" >&2
  exit 1
fi
if [[ ! -f "${RULES}" ]]; then
  echo "error: missing Spectral rules at ${RULES}" >&2
  exit 1
fi

SPECTRAL_BIN="${SPECTRAL_BIN:-}"
if [[ -z "${SPECTRAL_BIN}" ]]; then
  if command -v spectral >/dev/null 2>&1; then
    SPECTRAL_BIN="$(command -v spectral)"
  fi
fi

run_spectral() {
  local bin="$1"
  echo "==> spectral lint ${SPEC} (rules ${RULES})"
  "${bin}" lint "${SPEC}" --ruleset "${RULES}" --fail-severity error --verbose
}

if [[ -n "${SPECTRAL_BIN}" ]]; then
  run_spectral "${SPECTRAL_BIN}"
  echo "OK: Spectral lint passed"
  exit 0
fi

# Prefer a local npx install when spectral is not on PATH (CI / fresh clones).
if command -v npx >/dev/null 2>&1; then
  echo "==> spectral lint ${SPEC} via npx @stoplight/spectral-cli"
  npx --yes @stoplight/spectral-cli@6.15.0 lint "${SPEC}" \
    --ruleset "${RULES}" --fail-severity error --verbose
  echo "OK: Spectral lint passed"
  exit 0
fi

echo "error: spectral not found (install @stoplight/spectral-cli or set SPECTRAL_BIN)" >&2
exit 1
