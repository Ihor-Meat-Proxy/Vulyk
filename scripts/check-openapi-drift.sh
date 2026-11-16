#!/usr/bin/env bash
# Fail if docs/api/openapi.json drifts from ApiDoc, or if oasdiff reports
# breaking changes without a VERSION / workspace version bump vs base.
#
# Env:
#   OPENAPI_BASE_REF   git ref for baseline spec (default: origin/main)
#   SKIP_OASDIFF=1     drift-only (skip breaking-change gate)
#   OASDIFF_BIN        path to oasdiff binary (optional)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

COMMITTED="docs/api/openapi.json"
BASE_REF="${OPENAPI_BASE_REF:-origin/main}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

GENERATED="${TMP_DIR}/openapi.generated.json"
BASELINE="${TMP_DIR}/openapi.base.json"

if [[ ! -f "${COMMITTED}" ]]; then
  echo "error: missing committed OpenAPI at ${COMMITTED}" >&2
  echo "  run: ./scripts/generate-openapi.sh" >&2
  exit 1
fi

echo "==> regenerate OpenAPI from ApiDoc"
cargo run -p vulyk-api --example dump_openapi --quiet -- --out "${GENERATED}"

echo "==> diff against committed ${COMMITTED}"
if ! diff -u "${COMMITTED}" "${GENERATED}"; then
  echo "error: OpenAPI drift detected. Regenerate with:" >&2
  echo "  ./scripts/generate-openapi.sh" >&2
  exit 1
fi
echo "OK: committed OpenAPI matches ApiDoc"

if [[ "${SKIP_OASDIFF:-}" == "1" ]]; then
  echo "OK: oasdiff skipped (SKIP_OASDIFF=1)"
  exit 0
fi

need_oasdiff() {
  if [[ -n "${OASDIFF_BIN:-}" && -x "${OASDIFF_BIN}" ]]; then
    return 0
  fi
  if command -v oasdiff >/dev/null 2>&1; then
    OASDIFF_BIN="$(command -v oasdiff)"
    return 0
  fi
  echo "error: oasdiff not found (install or set OASDIFF_BIN)" >&2
  exit 1
}

# Resolve baseline: prefer committed file at BASE_REF; else skip if missing on base.
if git cat-file -e "${BASE_REF}:${COMMITTED}" 2>/dev/null; then
  git show "${BASE_REF}:${COMMITTED}" >"${BASELINE}"
else
  echo "OK: no baseline OpenAPI at ${BASE_REF}:${COMMITTED} (first commit); skip oasdiff"
  exit 0
fi

current_version="$(tr -d '[:space:]' <VERSION)"
base_version="$(git show "${BASE_REF}:VERSION" 2>/dev/null | tr -d '[:space:]' || true)"
version_bumped=0
if [[ -n "${base_version}" && "${current_version}" != "${base_version}" ]]; then
  version_bumped=1
fi

need_oasdiff
echo "==> oasdiff breaking (${BASE_REF} -> HEAD)"
set +e
BREAKING_OUT="$("${OASDIFF_BIN}" breaking "${BASELINE}" "${GENERATED}" 2>&1)"
BREAKING_EC=$?
set -e
if [[ "${BREAKING_EC}" -eq 0 ]]; then
  echo "OK: no breaking OpenAPI changes"
  exit 0
fi

echo "${BREAKING_OUT}"
if [[ "${version_bumped}" -eq 1 ]]; then
  echo "OK: breaking OpenAPI changes allowed (VERSION ${base_version} -> ${current_version})"
  exit 0
fi

echo "error: breaking OpenAPI changes require a VERSION bump (still ${current_version}; base ${base_version:-unknown})" >&2
echo "  bump VERSION / workspace.package.version in the same PR, or avoid the break." >&2
exit 1
