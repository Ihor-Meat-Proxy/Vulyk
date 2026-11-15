#!/usr/bin/env bash
# E2E / sample-seed hygiene for Playwright <-> Vulyk case keys.
#
# Fails when:
#   1. backend/seed/vulyk-qa.d/playwright-e2e.json VQA keys have numeric gaps
#   2. Two seed files (base or fragments) claim the same non-empty external_key
#   3. Spec @VQA-N / @VUE-N tags (or vulyk annotations) reference missing keys
#
# Usage: ./scripts/check-e2e-case-keys.sh
# Also run via ./scripts/check.sh (--quick includes this).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

python3 - <<'PY'
from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(".")
SEED_DIR = ROOT / "backend" / "seed"
FRAG_DIR = SEED_DIR / "vulyk-qa.d"
PE_PATH = FRAG_DIR / "playwright-e2e.json"
BASE_PATH = SEED_DIR / "vulyk-qa.json"
TESTS_DIR = ROOT / "e2e" / "tests"
VUE_CASES_DIR = ROOT / "e2e" / "cases"

KEY_RE = re.compile(r"^([A-Z][A-Z0-9]{1,7})-([0-9]+)$")
TAG_RE = re.compile(r"@((?:VQA|VUE)-(\d+))\b")
ANN_RE = re.compile(
    r"description\s*:\s*['\"]((?:VQA|VUE)-(\d+))['\"]"
)


def walk_cases(suite: dict, out: list[dict]) -> None:
    for case in suite.get("cases") or []:
        out.append(case)
    for child in suite.get("children") or []:
        walk_cases(child, out)


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        print(f"error: failed to parse {path}: {e}", file=sys.stderr)
        sys.exit(1)


def seed_files() -> list[Path]:
    files = [BASE_PATH]
    if FRAG_DIR.is_dir():
        files.extend(sorted(p for p in FRAG_DIR.glob("*.json") if p.is_file()))
    return [p for p in files if p.is_file()]


def all_seed_cases() -> list[tuple[str, dict]]:
    rows: list[tuple[str, dict]] = []
    for path in seed_files():
        data = load_json(path)
        cases: list[dict] = []
        for suite in data.get("suites") or []:
            walk_cases(suite, cases)
        for case in cases:
            rows.append((str(path), case))
    return rows


def vqa_keys_from_seed() -> dict[int, str]:
    found: dict[int, str] = {}
    for source, case in all_seed_cases():
        raw = (case.get("key") or "").strip()
        if not raw:
            continue
        m = KEY_RE.match(raw.upper())
        if not m or m.group(1) != "VQA":
            continue
        n = int(m.group(2))
        found.setdefault(n, source)
    return found


def vue_keys_from_cases() -> dict[int, str]:
    found: dict[int, str] = {}
    if not VUE_CASES_DIR.is_dir():
        return found
    for path in sorted(VUE_CASES_DIR.glob("*.json")):
        if path.name == "manifest.json":
            continue
        data = load_json(path)
        for case in data.get("cases") or []:
            raw = (case.get("key") or "").strip()
            m = KEY_RE.match(raw.upper())
            if not m or m.group(1) != "VUE":
                continue
            n = int(m.group(2))
            found.setdefault(n, str(path))
    return found


def check_playwright_e2e_gaps() -> list[str]:
    errors: list[str] = []
    if not PE_PATH.is_file():
        errors.append(f"missing {PE_PATH}")
        return errors
    data = load_json(PE_PATH)
    nums: list[int] = []
    cases: list[dict] = []
    for suite in data.get("suites") or []:
        walk_cases(suite, cases)
    for case in cases:
        raw = (case.get("key") or "").strip()
        if not raw:
            errors.append(
                f"{PE_PATH}: case without key "
                f"(title={(case.get('title') or {}).get('en')!r}). "
                "playwright-e2e keys must be explicit and stable."
            )
            continue
        m = KEY_RE.match(raw.upper())
        if not m or m.group(1) != "VQA":
            errors.append(f"{PE_PATH}: invalid key {raw!r} (expected VQA-N)")
            continue
        nums.append(int(m.group(2)))
    if not nums:
        errors.append(f"{PE_PATH}: no VQA keys found")
        return errors
    nums_sorted = sorted(set(nums))
    if len(nums_sorted) != len(nums):
        seen: dict[int, int] = defaultdict(int)
        for n in nums:
            seen[n] += 1
        dups = sorted(n for n, c in seen.items() if c > 1)
        errors.append(
            f"{PE_PATH}: duplicate VQA numbers inside fragment: "
            + ", ".join(f"VQA-{n}" for n in dups)
        )
    gaps: list[str] = []
    for a, b in zip(nums_sorted, nums_sorted[1:]):
        if b > a + 1:
            if b == a + 2:
                gaps.append(f"VQA-{a + 1}")
            else:
                gaps.append(f"VQA-{a + 1}..VQA-{b - 1}")
    if gaps:
        errors.append(
            f"{PE_PATH}: VQA key gaps ({len(gaps)} range(s)): {', '.join(gaps)}. "
            "Do not skip numbers when appending; claim the next free VQA after "
            "max(playwright-e2e) (or fill an existing hole). "
            "NEVER run sync-vulyk-case-links.mjs on a feature branch - it can "
            "renumber. Append keys manually; prefer feature fragments with keys "
            "omitted for non-e2e sample cases."
        )
    else:
        print(
            f"OK: {PE_PATH.name} - {len(nums_sorted)} contiguous VQA keys "
            f"(VQA-{nums_sorted[0]:03d}..VQA-{nums_sorted[-1]:03d})"
        )
    return errors


def check_external_key_collisions() -> list[str]:
    """Fail when the same external_key appears in two different seed files.

    Multiple cases inside one file may share a short `e2e:spec.ts` link; that is
    allowed. Cross-fragment (or base vs fragment) claims of the same key are not.
    """
    errors: list[str] = []
    by_ext: dict[str, list[str]] = defaultdict(list)
    for source, case in all_seed_cases():
        ek = (case.get("external_key") or "").strip()
        if not ek:
            continue
        title = (case.get("title") or {}).get("en") or (case.get("key") or "?")
        by_ext[ek].append(f"{source} ({title})")
    collisions = {}
    for ek, locs in by_ext.items():
        files = {loc.split(" (", 1)[0] for loc in locs}
        if len(files) > 1:
            collisions[ek] = locs
    if collisions:
        errors.append(
            f"duplicate external_key across seed files ({len(collisions)}):"
        )
        for ek in sorted(collisions)[:40]:
            locs = collisions[ek]
            errors.append(f"  {ek!r}")
            for loc in locs:
                errors.append(f"    - {loc}")
        if len(collisions) > 40:
            errors.append(f"  ... and {len(collisions) - 40} more")
    else:
        frag_n = len(list(FRAG_DIR.glob("*.json"))) if FRAG_DIR.is_dir() else 0
        print(f"OK: external_key unique across base + {frag_n} fragment(s)")
    return errors


def check_spec_tags() -> list[str]:
    errors: list[str] = []
    vqa = vqa_keys_from_seed()
    vue = vue_keys_from_cases()
    if not TESTS_DIR.is_dir():
        errors.append(f"missing {TESTS_DIR}")
        return errors

    missing: list[str] = []
    checked = 0
    for path in sorted(TESTS_DIR.glob("*.spec.ts")):
        text = path.read_text(encoding="utf-8")
        refs: set[str] = set()
        for m in TAG_RE.finditer(text):
            refs.add(m.group(1))
        for m in ANN_RE.finditer(text):
            refs.add(m.group(1))
        for ref in sorted(refs):
            checked += 1
            kind, _, num_s = ref.partition("-")
            n = int(num_s)
            if kind == "VQA" and n not in vqa:
                missing.append(f"{path.name}: @{ref} (not in sample seed)")
            elif kind == "VUE" and n not in vue:
                missing.append(f"{path.name}: @{ref} (not in e2e/cases)")
    if missing:
        errors.append(
            f"e2e tags/annotations reference missing keys ({len(missing)}):"
        )
        for row in missing[:60]:
            errors.append(f"  - {row}")
        if len(missing) > 60:
            errors.append(f"  ... and {len(missing) - 60} more")
    else:
        print(
            f"OK: {checked} VQA/VUE tag/annotation refs resolve "
            f"({len(vqa)} VQA seed keys, {len(vue)} VUE case keys)"
        )
    return errors


def main() -> None:
    errors: list[str] = []
    errors.extend(check_playwright_e2e_gaps())
    errors.extend(check_external_key_collisions())
    errors.extend(check_spec_tags())
    if errors:
        print("error: e2e case-key hygiene failed:", file=sys.stderr)
        for e in errors:
            print(f"  {e}", file=sys.stderr)
        print(
            "\nSee AGENTS.md (Sample seed keys / Recurring CI pitfalls) and "
            "e2e/README.md. Prefer manual append + feature fragments with keys "
            "omitted; never run sync-vulyk-case-links.mjs on feature branches "
            "without --force-renumber ownership.",
            file=sys.stderr,
        )
        sys.exit(1)
    print("OK: e2e case-key hygiene")


if __name__ == "__main__":
    main()
PY
