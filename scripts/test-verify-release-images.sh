#!/usr/bin/env bash
# Smoke test for verify-release-images.sh (no network / no cosign binary required).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

bash -n scripts/verify-release-images.sh
chmod +x scripts/verify-release-images.sh

out="$(VULYK_COSIGN_DRY_RUN=1 ./scripts/verify-release-images.sh 0.6.0)"
printf '%s\n' "${out}" | grep -F 'cosign verify' >/dev/null || die "expected dry-run cosign verify lines"
printf '%s\n' "${out}" | grep -F 'workflows/release' >/dev/null || die "expected release.yml identity regexp"
printf '%s\n' "${out}" | grep -F 'vulyk-api:0.6.0' >/dev/null || die "expected api image ref"
printf '%s\n' "${out}" | grep -F 'vulyk-web:0.6.0' >/dev/null || die "expected web image ref"
printf '%s\n' "${out}" | grep -F 'token.actions.githubusercontent.com' >/dev/null || die "expected OIDC issuer"

# install.sh wires --verify-images / VULYK_VERIFY_IMAGES without requiring cosign when off
bash -n scripts/install.sh
grep -q -- '--verify-images' scripts/install.sh || die "install.sh missing --verify-images"
grep -q 'VULYK_VERIFY_IMAGES' scripts/install.sh || die "install.sh missing VULYK_VERIFY_IMAGES"
grep -q 'verify-release-images.sh' scripts/install.sh || die "install.sh must call verify-release-images.sh"
grep -q 'VULYK_VERIFY_IMAGES=0' .env.example || die ".env.example missing VULYK_VERIFY_IMAGES sync default"

echo "OK: verify-release-images dry-run"
