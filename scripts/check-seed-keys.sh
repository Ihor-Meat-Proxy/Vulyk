#!/usr/bin/env bash
# Fail with a clear error if sample seed case keys collide, or if fragment
# registration is inconsistent. Mirrors the allocation rules in sample_seed.rs.
# Usage: ./scripts/check-seed-keys.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

python3 - <<'PY'
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(".")
SEED_DIR = ROOT / "backend" / "seed"
SAMPLE_RS = ROOT / "backend" / "src" / "sample_seed.rs"
FRAG_DIR = SEED_DIR / "vulyk-qa.d"
KEY_RE = re.compile(r"^([A-Z][A-Z0-9]{1,7})-([0-9]+)$")


def walk_cases(suite: dict, path: str, out: list[tuple[str, dict, str]]) -> None:
    name = (suite.get("name") or {}).get("en") or "?"
    here = f"{path}/{name}"
    for i, case in enumerate(suite.get("cases") or []):
        out.append((here, case, f"cases[{i}]"))
    for i, child in enumerate(suite.get("children") or []):
        walk_cases(child, here, out)


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001 - surface parse errors clearly
        print(f"error: failed to parse {path}: {e}", file=sys.stderr)
        sys.exit(1)


def registered_fragments() -> list[str]:
    text = SAMPLE_RS.read_text(encoding="utf-8")
    # Drop line comments so doc examples are ignored.
    stripped = "\n".join(
        line.split("//", 1)[0] for line in text.splitlines()
    )
    # Entries look like: ("pdf-export", include_str!("../seed/vulyk-qa.d/pdf-export.json")),
    return re.findall(
        r'include_str!\(\s*"\.\./seed/vulyk-qa\.d/([^"]+)"\s*\)',
        stripped,
    )


def allocate(prefix: str, items: list[tuple[str, dict, str, str]]) -> None:
    """Assign missing keys in-place; items are (source, suite_path, case, loc)."""
    max_n = 0
    seen: dict[str, str] = {}
    empties: list[tuple[str, dict, str]] = []

    for source, suite_path, case, loc in items:
        raw = (case.get("key") or "").strip()
        where = f"{source}:{suite_path}/{loc}"
        if not raw:
            empties.append((where, case, source))
            continue
        m = KEY_RE.match(raw.upper())
        if not m:
            print(
                f"error: invalid case key {raw!r} at {where}\n"
                f"  expected PREFIX-N with PREFIX matching ^[A-Z][A-Z0-9]{{1,7}}$ "
                f"(e.g. VQA-171). Prefer omitting key so allocation assigns the next free number.",
                file=sys.stderr,
            )
            sys.exit(1)
        key = f"{m.group(1)}-{int(m.group(2))}"
        if m.group(1) != prefix:
            print(
                f"error: case key {raw!r} at {where} uses prefix {m.group(1)}, "
                f"expected project prefix {prefix}",
                file=sys.stderr,
            )
            sys.exit(1)
        max_n = max(max_n, int(m.group(2)))
        if key in seen:
            print(
                f"error: duplicate sample seed case key {key}\n"
                f"  first:  {seen[key]}\n"
                f"  second: {where}\n"
                f"Fix: omit keys in feature fragments under backend/seed/vulyk-qa.d/ "
                f"(see README there), or renumber after rebasing onto latest main. "
                f"Do not append the same VQA-NNN range from parallel PRs.",
                file=sys.stderr,
            )
            sys.exit(1)
        seen[key] = where
        case["key"] = key

    next_n = max_n
    for where, case, _source in empties:
        next_n += 1
        key = f"{prefix}-{next_n:03d}"
        if key in seen:
            print(
                f"error: allocated key {key} collided at {where} (already {seen[key]})",
                file=sys.stderr,
            )
            sys.exit(1)
        case["key"] = key
        seen[key] = where


def main() -> None:
    errors = 0

    # -- Kyiv concert (standalone) --
    kyiv_path = SEED_DIR / "kyiv-concert-2026.json"
    kyiv = load_json(kyiv_path)
    kyiv_items: list[tuple[str, dict, str, str]] = []
    for suite in kyiv.get("suites") or []:
        cases: list[tuple[str, dict, str]] = []
        walk_cases(suite, "suites", cases)
        for suite_path, case, loc in cases:
            kyiv_items.append((str(kyiv_path), suite_path, case, loc))
    allocate("KC", [(s, sp, c, loc) for s, sp, c, loc in kyiv_items])
    print(f"OK: {kyiv_path} - {len(kyiv_items)} keys unique (prefix KC)")

    # -- Vulyk QA base + fragments --
    base_path = SEED_DIR / "vulyk-qa.json"
    base = load_json(base_path)
    items: list[tuple[str, dict, str, str]] = []
    for suite in base.get("suites") or []:
        cases = []
        walk_cases(suite, "suites", cases)
        for suite_path, case, loc in cases:
            items.append((str(base_path), suite_path, case, loc))

    on_disk = sorted(
        p.name
        for p in FRAG_DIR.glob("*.json")
        if p.is_file()
    ) if FRAG_DIR.is_dir() else []
    registered = registered_fragments()

    missing_reg = [f for f in on_disk if f not in registered]
    missing_files = [f for f in registered if f not in on_disk]
    if missing_reg:
        print(
            "error: fragment JSON present but not registered in "
            f"backend/src/sample_seed.rs VULYK_QA_FRAGMENTS:\n  "
            + "\n  ".join(missing_reg),
            file=sys.stderr,
        )
        errors = 1
    if missing_files:
        print(
            "error: VULYK_QA_FRAGMENTS references missing files under "
            f"backend/seed/vulyk-qa.d/:\n  " + "\n  ".join(missing_files),
            file=sys.stderr,
        )
        errors = 1
    if errors:
        sys.exit(1)

    for fname in registered:
        path = FRAG_DIR / fname
        if not path.is_file():
            continue
        frag = load_json(path)
        feature = (frag.get("feature") or path.stem).strip()
        for suite in frag.get("suites") or []:
            cases = []
            walk_cases(suite, f"fragment:{feature}", cases)
            for suite_path, case, loc in cases:
                items.append((str(path), suite_path, case, loc))

    allocate("VQA", items)
    if errors:
        sys.exit(1)

    highest = max(int(KEY_RE.match(c["key"]).group(2)) for _s, _sp, c, _l in items)
    print(
        f"OK: Vulyk QA - {len(items)} keys unique after allocation "
        f"(base {base_path.name} + {len(registered)} fragment(s); "
        f"highest VQA-{highest:03d})"
    )


if __name__ == "__main__":
    main()
PY
