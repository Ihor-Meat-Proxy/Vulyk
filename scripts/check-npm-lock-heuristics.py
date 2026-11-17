#!/usr/bin/env python3
"""Heuristic malware / supply-chain checks on npm lockfile diffs (Part of #294).

Compares BASE_REF..HEAD_REF package-lock.json files and fails when the diff
looks high-risk for auto-merge:

  - New packages that introduce install scripts
  - Newly enabled hasInstallScript on an existing package
  - Typosquat-ish names vs a small set of popular packages (edit distance 1-2
    on the last path segment, excluding exact matches and scoped org changes)

Routine version bumps of existing packages without new install scripts pass.

Usage:
  BASE_REF=origin/main HEAD_REF=HEAD ./scripts/check-npm-lock-heuristics.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCKS = ("frontend/package-lock.json", "e2e/package-lock.json")

# Popular packages we watch for near-miss names (unscoped last segment).
POPULAR = {
    "react",
    "react-dom",
    "lodash",
    "express",
    "axios",
    "webpack",
    "vite",
    "typescript",
    "eslint",
    "prettier",
    "jest",
    "mocha",
    "chalk",
    "debug",
    "uuid",
    "rimraf",
    "minimist",
    "request",
    "node-fetch",
    "cross-env",
}


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


def parse_lock(text: str) -> dict[str, dict]:
    data = json.loads(text)
    out: dict[str, dict] = {}
    for key, meta in (data.get("packages") or {}).items():
        if key == "" or not isinstance(meta, dict):
            continue
        name = meta.get("name")
        if not name:
            name = key[len("node_modules/") :] if key.startswith("node_modules/") else key
        out[name] = {
            "version": meta.get("version"),
            "hasInstallScript": bool(meta.get("hasInstallScript")),
            "resolved": meta.get("resolved"),
        }
    return out


def unscoped(name: str) -> str:
    if name.startswith("@"):
        parts = name.split("/", 1)
        return parts[1] if len(parts) == 2 else name
    return name.split("/")[-1]


def edit_distance(a: str, b: str) -> int:
    if a == b:
        return 0
    if abs(len(a) - len(b)) > 2:
        return 99
    # Classic DP for small strings
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            ins = cur[j - 1] + 1
            delete = prev[j] + 1
            sub = prev[j - 1] + (0 if ca == cb else 1)
            cur.append(min(ins, delete, sub))
        prev = cur
    return prev[-1]


def typosquat_hits(name: str) -> list[str]:
    target = unscoped(name).lower()
    hits: list[str] = []
    for pop in POPULAR:
        if target == pop:
            continue
        dist = edit_distance(target, pop)
        # Near-miss: 1 edit, or 2 edits on longer names.
        if dist == 1 or (dist == 2 and len(target) >= 5 and len(pop) >= 5):
            hits.append(pop)
    return hits


def analyze(base: str, head: str) -> list[str]:
    findings: list[str] = []
    for path in LOCKS:
        base_text = git_show(base, path)
        head_text = git_show(head, path)
        if head_text is None:
            continue
        if base_text is None:
            findings.append(f"new lockfile {path}")
            continue
        base_pkgs = parse_lock(base_text)
        head_pkgs = parse_lock(head_text)
        added = sorted(set(head_pkgs) - set(base_pkgs))
        for name in added:
            meta = head_pkgs[name]
            if meta["hasInstallScript"]:
                findings.append(
                    f"{path}: new package {name}@{meta.get('version')} has install scripts"
                )
            for pop in typosquat_hits(name):
                findings.append(
                    f"{path}: new package {name} is within edit distance of popular '{pop}'"
                )
        for name in sorted(set(head_pkgs) & set(base_pkgs)):
            before = base_pkgs[name]
            after = head_pkgs[name]
            if after["hasInstallScript"] and not before["hasInstallScript"]:
                findings.append(
                    f"{path}: {name} newly gained hasInstallScript at {after.get('version')}"
                )
            # resolved host flip away from registry.npmjs.org is suspicious
            br = before.get("resolved") or ""
            ar = after.get("resolved") or ""
            if br and ar and "registry.npmjs.org" in br and "registry.npmjs.org" not in ar:
                findings.append(
                    f"{path}: {name} resolved host changed away from registry.npmjs.org"
                )
    return findings


def main() -> None:
    base = os.environ.get("BASE_REF") or os.environ.get("GITHUB_BASE_REF")
    head = os.environ.get("HEAD_REF") or "HEAD"
    if not base:
        die("set BASE_REF (or GITHUB_BASE_REF)")

    if "/" not in base and not base.startswith("origin/"):
        candidate = f"origin/{base}"
        if (
            subprocess.run(
                ["git", "rev-parse", "--verify", candidate],
                cwd=ROOT,
                capture_output=True,
            ).returncode
            == 0
        ):
            base = candidate

    findings = analyze(base, head)
    reviewed = os.environ.get("SUPPLY_CHAIN_REVIEWED", "").lower() in {
        "1",
        "true",
        "yes",
    }
    if not findings:
        print("OK: npm lock heuristics clean")
        return
    print("npm lock heuristics findings:")
    for f in findings:
        print(f"  - {f}")
    if reviewed:
        print("OK: supply-chain-reviewed override present")
        return
    print(
        "Add label 'supply-chain-reviewed' after owner review, or leave for manual merge.",
        file=sys.stderr,
    )
    raise SystemExit(2)


if __name__ == "__main__":
    main()
