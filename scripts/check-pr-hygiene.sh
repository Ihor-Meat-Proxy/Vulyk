#!/usr/bin/env bash
# PR hygiene: Conventional Commits title + Closes/Part of issue reference.
# Skips Dependabot / Renovate bot authors.
# Usage:
#   PR_TITLE="..." PR_BODY="..." PR_AUTHOR="..." ./scripts/check-pr-hygiene.sh
#   ./scripts/check-pr-hygiene.sh --title "..." --body-file path --author "..."
set -euo pipefail

TITLE="${PR_TITLE:-}"
BODY="${PR_BODY:-}"
AUTHOR="${PR_AUTHOR:-}"
BODY_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --title)
      TITLE="${2:-}"
      shift 2
      ;;
    --body)
      BODY="${2:-}"
      shift 2
      ;;
    --body-file)
      BODY_FILE="${2:-}"
      shift 2
      ;;
    --author)
      AUTHOR="${2:-}"
      shift 2
      ;;
    -h|--help)
      sed -n '2,8p' "$0"
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -n "${BODY_FILE}" ]]; then
  BODY="$(cat "${BODY_FILE}")"
fi

case "${AUTHOR}" in
  dependabot\[bot\]|dependabot|renovate\[bot\]|renovate)
    echo "OK: PR hygiene skipped for bot author (${AUTHOR})"
    exit 0
    ;;
esac

if [[ -z "${TITLE}" ]]; then
  echo "error: PR title is empty (set PR_TITLE or --title)" >&2
  exit 1
fi

# Conventional Commits: type(scope)!: subject
# https://www.conventionalcommits.org/
if ! printf '%s\n' "${TITLE}" | grep -Eq \
  '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9._/-]+\))?!?: .+'; then
  echo "error: PR title must use Conventional Commits" >&2
  echo "  got: ${TITLE}" >&2
  echo "  example: ci(fast-gates): add required shellcheck and knip jobs" >&2
  exit 1
fi

if ! printf '%s\n' "${BODY}" | grep -Eiq '(^|[[:space:]])(Closes|Part of|Fixes|Resolves) #[0-9]+'; then
  echo "error: PR body must mention an issue via 'Closes #N' or 'Part of #N'" >&2
  echo "  (also accepted: Fixes #N / Resolves #N)" >&2
  exit 1
fi

echo "OK: PR hygiene passed (title + issue reference)"
