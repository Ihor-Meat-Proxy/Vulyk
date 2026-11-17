#!/usr/bin/env bash
# Verify GHCR images for a published Vulyk release (keyless cosign + optional
# GitHub attestations). Used by install.sh upgrade --verify-images and docs.
# Part of #294.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
log() { printf '==> %s\n' "$*" >&2; }

VERSION="${1:-}"
[[ -n "${VERSION}" ]] || die "usage: $0 <semver>   (e.g. 0.6.0)"
VERSION="${VERSION#v}"
[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]] \
  || die "invalid version '${VERSION}'"

OWNER_LC="$(
  if [[ -n "${VULYK_GHCR_OWNER:-}" ]]; then
    printf '%s' "${VULYK_GHCR_OWNER}" | tr '[:upper:]' '[:lower:]'
  else
    # Prefer git remote owner when available.
    remote="$(git -C "${ROOT}" remote get-url origin 2>/dev/null || true)"
    if [[ "${remote}" =~ github.com[:/]+([^/]+)/ ]]; then
      printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]'
    else
      printf 'ihor-meat-proxy'
    fi
  fi
)"

REPO="${VULYK_GITHUB_REPOSITORY:-${OWNER_LC}/vulyk}"
# Identity is the Release workflow on this repository (keyless Fulcio).
IDENTITY_REGEXP="${VULYK_COSIGN_IDENTITY_REGEXP:-^https://github.com/${REPO}/\\.github/workflows/release\\.yml@refs/tags/v}"
OIDC_ISSUER="${VULYK_COSIGN_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"

API_IMAGE="${VULYK_GHCR_API_IMAGE:-ghcr.io/${OWNER_LC}/vulyk-api:${VERSION}}"
WEB_IMAGE="${VULYK_GHCR_WEB_IMAGE:-ghcr.io/${OWNER_LC}/vulyk-web:${VERSION}}"
# Strip tag for attest subject helpers if callers pass full refs with digests later.
API_REF="${API_IMAGE}"
WEB_REF="${WEB_IMAGE}"

need_cosign() {
  command -v cosign >/dev/null 2>&1 || die "cosign not found (install https://docs.sigstore.dev/cosign/installation/)"
}

verify_one() {
  local image="$1"
  log "cosign verify ${image}"
  if [[ "${VULYK_COSIGN_DRY_RUN:-0}" == "1" ]]; then
    printf 'DRY_RUN cosign verify --certificate-identity-regexp=%s --certificate-oidc-issuer=%s %s\n' \
      "${IDENTITY_REGEXP}" "${OIDC_ISSUER}" "${image}"
    return 0
  fi
  cosign verify \
    --certificate-identity-regexp="${IDENTITY_REGEXP}" \
    --certificate-oidc-issuer="${OIDC_ISSUER}" \
    "${image}"
}

if [[ "${VULYK_COSIGN_DRY_RUN:-0}" != "1" ]]; then
  need_cosign
fi
verify_one "${API_REF}"
verify_one "${WEB_REF}"
log "OK: signatures verified for ${VERSION}"
