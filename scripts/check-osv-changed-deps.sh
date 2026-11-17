#!/usr/bin/env bash
# Run OSV on lockfiles and fail only when a package that CHANGED in this PR
# (vs BASE_REF) has a known vulnerability. Baseline vulns stay tracked by the
# Security workflow + deny/audit ignores (Part of #294).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

BASE_REF="${BASE_REF:-${GITHUB_BASE_REF:-}}"
HEAD_REF="${HEAD_REF:-HEAD}"
[[ -n "${BASE_REF}" ]] || die "set BASE_REF"

if [[ "${BASE_REF}" != */* && "${BASE_REF}" != origin/* && "${BASE_REF}" != refs/* ]]; then
  if git rev-parse -q --verify "origin/${BASE_REF}" >/dev/null; then
    BASE_REF="origin/${BASE_REF}"
  fi
fi

need_osv() {
  if command -v osv-scanner >/dev/null 2>&1; then
    return 0
  fi
  die "osv-scanner not on PATH (CI installs via google/osv-scanner-action)"
}

OUT="$(mktemp)"
trap 'rm -f "${OUT}"' EXIT

# Prefer the action-installed binary; locally allow docker fallback for docs.
if command -v osv-scanner >/dev/null 2>&1; then
  set +e
  osv-scanner scan source \
    -L Cargo.lock \
    -L frontend/package-lock.json \
    -L e2e/package-lock.json \
    --format=json \
    --output-file="${OUT}"
  osv_rc=$?
  set -e
else
  die "osv-scanner not on PATH"
fi

# osv-scanner exits non-zero when vulns exist; we still parse JSON.
python3 - "${OUT}" "${BASE_REF}" "${HEAD_REF}" "${osv_rc}" <<'PY'
import json, subprocess, sys
from pathlib import Path

out, base, head, osv_rc = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])

def git_show(ref: str, path: str):
    p = subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        capture_output=True,
        text=True,
    )
    return p.stdout if p.returncode == 0 else None

def cargo_versions(text: str) -> dict[str, set[str]]:
    vers: dict[str, set[str]] = {}
    name = None
    for line in text.splitlines():
        if line.startswith("name = "):
            name = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("version = ") and name:
            ver = line.split("=", 1)[1].strip().strip('"')
            vers.setdefault(name, set()).add(ver)
            name = None
    return vers

def npm_versions(text: str) -> dict[str, set[str]]:
    data = json.loads(text)
    vers: dict[str, set[str]] = {}
    for key, meta in (data.get("packages") or {}).items():
        if key == "" or not isinstance(meta, dict):
            continue
        name = meta.get("name")
        if not name:
            name = key[len("node_modules/") :] if key.startswith("node_modules/") else key
        ver = meta.get("version")
        if ver:
            vers.setdefault(name, set()).add(ver)
    return vers

changed: set[tuple[str, str]] = set()  # (ecosystem, name)
locks = [
    ("Cargo.lock", "Rust", cargo_versions),
    ("frontend/package-lock.json", "npm", npm_versions),
    ("e2e/package-lock.json", "npm", npm_versions),
]
for path, eco, parser in locks:
    bt, ht = git_show(base, path), git_show(head, path)
    if ht is None:
        continue
    head_v = parser(ht)
    base_v = parser(bt) if bt else {}
    for name, versions in head_v.items():
        if name not in base_v or versions != base_v.get(name):
            changed.add((eco.lower(), name))

raw = Path(out).read_text(encoding="utf-8") if Path(out).stat().st_size else "{}"
try:
    data = json.loads(raw) if raw.strip() else {}
except json.JSONDecodeError:
    print("warning: could not parse OSV JSON; osv exit", osv_rc, file=sys.stderr)
    raise SystemExit(0 if osv_rc == 0 else 1)

# osv-scanner JSON: results[].packages[].package.{name,ecosystem} + vulnerabilities
hits = []
for result in data.get("results") or []:
    for pkg in result.get("packages") or []:
        package = pkg.get("package") or {}
        name = package.get("name") or ""
        eco = (package.get("ecosystem") or "").lower()
        # Normalize ecosystems: crates.io / npm
        if eco in {"crates.io", "rust"}:
            key = ("rust", name)
        elif eco in {"npm", "npmjs"}:
            key = ("npm", name)
        else:
            key = (eco, name)
        vulns = pkg.get("vulnerabilities") or []
        if not vulns:
            continue
        # Match changed set loosely on name for rust/npm
        name_changed = any(n == name for _, n in changed) or key in changed
        if not name_changed:
            # also match unscoped
            short = name.split("/")[-1]
            name_changed = any(n == name or n.endswith("/" + short) or n == short for _, n in changed)
        if name_changed:
            ids = [v.get("id") or v.get("aliases", [""])[0] for v in vulns]
            hits.append((name, eco, ids))

if not hits:
    print("OK: no OSV findings on packages changed in this PR")
    raise SystemExit(0)

print("OSV findings on changed packages:")
for name, eco, ids in hits:
    print(f"  - {eco} {name}: {', '.join(ids)}")
raise SystemExit(1)
PY
