#!/usr/bin/env bash
# Locked, script-safe npm install for frontend/ or e2e/.
# Uses package-lock.json (fails if out of sync) and ignores lifecycle scripts
# unless a package is listed in scripts/npm-install-scripts-allowlist.txt.
#
# Security fast path (#294): project .npmrc sets min-release-age=7. Dependabot
# security updates (or VULYK_NPM_SECURITY_FAST_PATH=1) pass --min-release-age=0
# so a patch published <7 days ago still installs in CI/Docker. See
# docs/dev/supply-chain.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOWLIST="${ROOT}/scripts/npm-install-scripts-allowlist.txt"
FAST_PATH_HELPER="${ROOT}/scripts/npm-security-fast-path.sh"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

DIR="${1:-}"
[[ -n "${DIR}" ]] || die "usage: $0 <frontend|e2e|path-to-package-dir> [--security-fast-path] [npm-ci-extra-args...]"
shift || true

EXTRA=()
FORCE_FAST=0
while (($#)); do
  case "$1" in
    --security-fast-path)
      FORCE_FAST=1
      shift
      ;;
    *)
      EXTRA+=("$1")
      shift
      ;;
  esac
done

if [[ "${DIR}" == "frontend" || "${DIR}" == "e2e" ]]; then
  PKG_DIR="${ROOT}/${DIR}"
else
  PKG_DIR="$(cd "${DIR}" && pwd)"
fi

[[ -f "${PKG_DIR}/package-lock.json" ]] || die "missing package-lock.json in ${PKG_DIR}"
[[ -f "${PKG_DIR}/package.json" ]] || die "missing package.json in ${PKG_DIR}"

cd "${PKG_DIR}"

NPM_ARGS=(ci --ignore-scripts)

if [[ "${FORCE_FAST}" -eq 1 ]]; then
  export VULYK_NPM_SECURITY_FAST_PATH=1
fi
FAST="$("${FAST_PATH_HELPER}")"
if [[ "${FAST}" == "1" ]]; then
  # CLI overrides project .npmrc min-release-age (docs.npmjs.com config precedence).
  NPM_ARGS+=(--min-release-age=0)
  printf '==> npm security fast path: --min-release-age=0 (bypassing .npmrc age gate)\n' >&2
fi

NPM_ARGS+=("${EXTRA[@]+"${EXTRA[@]}"}")

# Prefer local .npmrc ignore-scripts; also pass the flag for callers without .npmrc.
npm "${NPM_ARGS[@]}"

if [[ -f "${ALLOWLIST}" ]]; then
  mapfile -t pkgs < <(grep -vE '^\s*(#|$)' "${ALLOWLIST}" || true)
  if ((${#pkgs[@]} > 0)); then
    printf '==> npm rebuild (allowlisted): %s\n' "${pkgs[*]}"
    npm rebuild "${pkgs[@]}"
  fi
fi
