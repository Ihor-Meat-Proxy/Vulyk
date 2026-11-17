#!/usr/bin/env bash
# Decide whether this install should bypass project .npmrc min-release-age
# (security fast path for Dependabot advisory updates — Part of #294).
#
# Prints "1" or "0" to stdout.
# Override: VULYK_NPM_SECURITY_FAST_PATH=1|0|true|false
# Auto: Dependabot PR with security label / advisory body markers.
set -euo pipefail

explicit="${VULYK_NPM_SECURITY_FAST_PATH:-}"
case "${explicit,,}" in
  1|true|yes|on) echo 1; exit 0 ;;
  0|false|no|off) echo 0; exit 0 ;;
esac

actor="${GITHUB_ACTOR:-}"
if [[ "${actor}" != "dependabot[bot]" ]]; then
  echo 0
  exit 0
fi

# Labels from the Actions event payload when present.
if [[ -n "${GITHUB_EVENT_PATH:-}" && -f "${GITHUB_EVENT_PATH}" ]]; then
  if python3 - "${GITHUB_EVENT_PATH}" <<'PY'
import json, re, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    ev = json.load(f)
pr = ev.get("pull_request") or {}
labels = {str(x.get("name", "")).lower() for x in (pr.get("labels") or [])}
if "security" in labels:
    raise SystemExit(0)
blob = f"{pr.get('title') or ''}\n{pr.get('body') or ''}".lower()
if re.search(
    r"security (vulnerabilit|advisory|update)|ghsa-[0-9a-z]{4}-|cve-[0-9]{4}-",
    blob,
):
    raise SystemExit(0)
raise SystemExit(1)
PY
  then
    echo 1
    exit 0
  fi
fi

echo 0
