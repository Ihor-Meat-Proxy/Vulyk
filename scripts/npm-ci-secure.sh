#!/usr/bin/env bash
# Locked, script-safe npm install for frontend/ or e2e/.
# Uses package-lock.json (fails if out of sync) and ignores lifecycle scripts
# unless a package is listed in scripts/npm-install-scripts-allowlist.txt.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOWLIST="${ROOT}/scripts/npm-install-scripts-allowlist.txt"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

DIR="${1:-}"
[[ -n "${DIR}" ]] || die "usage: $0 <frontend|e2e|path-to-package-dir> [npm-ci-extra-args...]"
shift || true

if [[ "${DIR}" == "frontend" || "${DIR}" == "e2e" ]]; then
  PKG_DIR="${ROOT}/${DIR}"
else
  PKG_DIR="$(cd "${DIR}" && pwd)"
fi

[[ -f "${PKG_DIR}/package-lock.json" ]] || die "missing package-lock.json in ${PKG_DIR}"
[[ -f "${PKG_DIR}/package.json" ]] || die "missing package.json in ${PKG_DIR}"

cd "${PKG_DIR}"

# Prefer local .npmrc ignore-scripts; also pass the flag for callers without .npmrc.
npm ci --ignore-scripts "$@"

if [[ -f "${ALLOWLIST}" ]]; then
  mapfile -t pkgs < <(grep -vE '^\s*(#|$)' "${ALLOWLIST}" || true)
  if ((${#pkgs[@]} > 0)); then
    printf '==> npm rebuild (allowlisted): %s\n' "${pkgs[*]}"
    npm rebuild "${pkgs[@]}"
  fi
fi
