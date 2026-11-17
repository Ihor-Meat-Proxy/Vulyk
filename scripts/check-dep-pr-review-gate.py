#!/usr/bin/env python3
"""Detect Dependabot PR changes that need owner review before auto-merge.

Flags (exit 2):
  - Newly added packages (Cargo.lock / package-lock.json)
  - Newly introduced npm install scripts (hasInstallScript)
  - Maintainer-change signal (DEPENDABOT_MAINTAINER_CHANGES=true)

Exit 0 when only version bumps of existing packages.
Exit 1 on usage / IO errors.

Usage:
  BASE_REF=origin/main HEAD_REF=HEAD ./scripts/check-dep-pr-review-gate.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK_PATHS = (
    "Cargo.lock",
    "frontend/package-lock.json",
    "e2e/package-lock.json",
)


def die(msg: str, code: int = 1) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(code)


def git_show(ref: str, path: str) -> str | None:
    proc = subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return None
    return proc.stdout


def cargo_packages(lock_text: str) -> set[str]:
    names: set[str] = set()
    current: str | None = None
    for line in lock_text.splitlines():
        if line.startswith("name = "):
            current = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("version = ") and current:
            names.add(current)
            current = None
    return names


def npm_packages(lock_text: str) -> tuple[set[str], set[str]]:
    """Return (all package ids, ids with hasInstallScript)."""
    data = json.loads(lock_text)
    packages = data.get("packages") or {}
    names: set[str] = set()
    with_scripts: set[str] = set()
    for key, meta in packages.items():
        if key == "" or not isinstance(meta, dict):
            continue
        # lockfile v3 keys like "node_modules/foo" or "node_modules/@scope/pkg"
        name = meta.get("name")
        if not name:
            if key.startswith("node_modules/"):
                name = key[len("node_modules/") :]
            else:
                name = key
        names.add(name)
        if meta.get("hasInstallScript") is True:
            with_scripts.add(name)
    return names, with_scripts


def analyze(base: str, head: str) -> list[str]:
    reasons: list[str] = []
    for path in LOCK_PATHS:
        base_text = git_show(base, path)
        head_text = git_show(head, path)
        if head_text is None:
            continue
        if base_text is None:
            reasons.append(f"new lockfile introduced: {path}")
            continue
        if path.endswith("Cargo.lock"):
            added = sorted(cargo_packages(head_text) - cargo_packages(base_text))
            if added:
                preview = ", ".join(added[:12])
                more = "" if len(added) <= 12 else f" (+{len(added) - 12} more)"
                reasons.append(f"new Cargo packages ({path}): {preview}{more}")
        else:
            base_pkgs, base_scripts = npm_packages(base_text)
            head_pkgs, head_scripts = npm_packages(head_text)
            added = sorted(head_pkgs - base_pkgs)
            if added:
                preview = ", ".join(added[:12])
                more = "" if len(added) <= 12 else f" (+{len(added) - 12} more)"
                reasons.append(f"new npm packages ({path}): {preview}{more}")
            new_scripts = sorted(head_scripts - base_scripts)
            if new_scripts:
                preview = ", ".join(new_scripts[:12])
                more = "" if len(new_scripts) <= 12 else f" (+{len(new_scripts) - 12} more)"
                reasons.append(
                    f"new npm install scripts ({path}): {preview}{more}"
                )
    return reasons


def main() -> None:
    base = os.environ.get("BASE_REF") or os.environ.get("GITHUB_BASE_REF")
    head = os.environ.get("HEAD_REF") or "HEAD"
    if not base:
        die("set BASE_REF (or GITHUB_BASE_REF) to the merge-base / base branch tip")

    # Resolve remote base when given a bare branch name.
    if "/" not in base and not base.startswith("origin/"):
        candidate = f"origin/{base}"
        probe = subprocess.run(
            ["git", "rev-parse", "--verify", candidate],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if probe.returncode == 0:
            base = candidate

    reasons = analyze(base, head)
    if os.environ.get("DEPENDABOT_MAINTAINER_CHANGES", "").lower() in {
        "1",
        "true",
        "yes",
    }:
        reasons.append("Dependabot reported maintainer changes")

    reviewed = os.environ.get("SUPPLY_CHAIN_REVIEWED", "").lower() in {
        "1",
        "true",
        "yes",
    }

    if not reasons:
        print("OK: no new packages, install scripts, or maintainer changes")
        return

    print("Supply-chain review required before auto-merge:")
    for r in reasons:
        print(f"  - {r}")
    if reviewed:
        print(
            "OK: supply-chain-reviewed label/override present; allowing merge gate"
        )
        return
    print(
        "Add label 'supply-chain-reviewed' after owner review, or leave for manual merge.",
        file=sys.stderr,
    )
    raise SystemExit(2)


if __name__ == "__main__":
    main()
