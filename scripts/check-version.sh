#!/usr/bin/env bash
# Ensure VERSION matches Cargo workspace, frontend package.json, and Helm appVersion.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ -f VERSION ]] || die "VERSION file missing"
VERSION="$(tr -d '[:space:]' < VERSION)"
[[ -n "${VERSION}" ]] || die "VERSION is empty"
[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]] \
  || die "VERSION must look like semver (got: ${VERSION})"

# Root workspace.package.version is the Rust single source of truth.
ws_ver="$(awk '
  /^\[workspace\.package\]/ { in_pkg=1; next }
  /^\[/ { in_pkg=0 }
  in_pkg && /^version[[:space:]]*=/ {
    gsub(/"/, "", $3); print $3; exit
  }
' Cargo.toml)"
[[ "${ws_ver}" == "${VERSION}" ]] \
  || die "Cargo.toml [workspace.package] version=${ws_ver:-<missing>} (expected ${VERSION})"

for crate in backend/Cargo.toml cli/Cargo.toml mcp/Cargo.toml; do
  if ! grep -qE '^version\.workspace[[:space:]]*=[[:space:]]*true' "${crate}"; then
    die "${crate}: expected version.workspace = true"
  fi
done

pkg="$(python3 -c 'import json; print(json.load(open("frontend/package.json"))["version"])')"
[[ "${pkg}" == "${VERSION}" ]] \
  || die "frontend/package.json: version=${pkg} (expected ${VERSION})"

app_ver="$(awk '
  /^appVersion:/ {
    v=$2; gsub(/"/, "", v); print v; exit
  }
' deploy/helm/vulyk/Chart.yaml)"
[[ "${app_ver}" == "${VERSION}" ]] \
  || die "deploy/helm/vulyk/Chart.yaml appVersion=${app_ver:-<missing>} (expected ${VERSION})"

if ! grep -qE "^## \[${VERSION}\]" CHANGELOG.md; then
  die "CHANGELOG.md missing ## [${VERSION}] section"
fi

printf 'OK: version %s aligned (VERSION, Cargo workspace, frontend, Helm, CHANGELOG)\n' "${VERSION}"
