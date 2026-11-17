#!/usr/bin/env bash
# Validate .devcontainer config (JSON + required files). Part of #294.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ -f .devcontainer/devcontainer.json ]] || die "missing .devcontainer/devcontainer.json"
[[ -f .devcontainer/post-create.sh ]] || die "missing .devcontainer/post-create.sh"

python3 - <<'PY'
import json
from pathlib import Path
p = Path(".devcontainer/devcontainer.json")
# Strip // comments if any (VS Code allows, we keep pure JSON).
text = p.read_text(encoding="utf-8")
data = json.loads(text)
assert data.get("name") == "Vulyk", data.get("name")
feats = data.get("features") or {}
need = (
    "ghcr.io/devcontainers/features/rust:1",
    "ghcr.io/devcontainers/features/node:1",
    "ghcr.io/devcontainers/features/docker-outside-of-docker:1",
)
for k in need:
    assert k in feats, f"missing feature {k}"
node = feats["ghcr.io/devcontainers/features/node:1"]
assert str(node.get("version")) == "22", node
assert data.get("postCreateCommand"), "postCreateCommand required"
assert "5173" in [str(x) for x in (data.get("forwardPorts") or [])]
print("OK: .devcontainer/devcontainer.json")
PY

bash -n .devcontainer/post-create.sh
grep -q 'npm-ci-secure' .devcontainer/post-create.sh \
  || die "post-create should mention npm-ci-secure / secure installs"
grep -q 'core.hooksPath' .devcontainer/post-create.sh \
  || die "post-create should set githooks"

echo "OK: Dev Container config"
