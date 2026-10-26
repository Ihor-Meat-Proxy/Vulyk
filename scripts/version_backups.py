#!/usr/bin/env python3
"""Version-backup registry for Vulyk upgrades (separate from ./backups and content backups).

Layout under VERSION_BACKUP_DIR (default: <root>/version-backups):
  vb-<version>-<utc>-<id8>/
    META.json
    db.sql.gz
    config/
    attachments.tar.gz (optional)
    SHA256SUMS

META.json roles:
  - version: retained as one of the last N installed-version snapshots
  - rollback: post-upgrade rollback point with expires_at

Never delete while META.in_progress is true.
Never prune the current running version's version-role backup.
Never prune an unexpired rollback-role backup.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import time
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any


def _utc_now() -> datetime:
    return datetime.now(timezone.utc)


def _parse_iso(s: str | None) -> datetime | None:
    if not s:
        return None
    try:
        return datetime.fromisoformat(s.replace("Z", "+00:00"))
    except ValueError:
        return None


def root_dir() -> Path:
    return Path(os.environ.get("VULYK_ROOT", Path(__file__).resolve().parents[1]))


def backup_dir() -> Path:
    override = os.environ.get("VULYK_VERSION_BACKUP_DIR", "").strip()
    if override:
        return Path(override)
    return root_dir() / "version-backups"


def rollback_days() -> int:
    try:
        return max(1, int(os.environ.get("VULYK_ROLLBACK_RETENTION_DAYS", "7")))
    except ValueError:
        return 7


def version_keep() -> int:
    try:
        return max(1, int(os.environ.get("VULYK_VERSION_BACKUP_COUNT", "3")))
    except ValueError:
        return 3


def sanitize_version(v: str) -> str:
    return "".join(c if c.isalnum() or c in "._-" else "_" for c in v)[:64] or "unknown"


def load_meta(path: Path) -> dict[str, Any] | None:
    meta_path = path / "META.json"
    if not meta_path.is_file():
        return None
    try:
        data = json.loads(meta_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    data["_path"] = str(path)
    data["_id"] = path.name
    return data


def write_meta(path: Path, meta: dict[str, Any]) -> None:
    clean = {k: v for k, v in meta.items() if not k.startswith("_")}
    (path / "META.json").write_text(
        json.dumps(clean, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def dir_size(path: Path) -> int:
    total = 0
    for p in path.rglob("*"):
        if p.is_file():
            try:
                total += p.stat().st_size
            except OSError:
                pass
    return total


def list_bundles() -> list[dict[str, Any]]:
    base = backup_dir()
    if not base.is_dir():
        return []
    items: list[dict[str, Any]] = []
    for child in sorted(base.iterdir()):
        if not child.is_dir() or not child.name.startswith("vb-"):
            continue
        meta = load_meta(child)
        if not meta:
            continue
        meta["size_bytes"] = dir_size(child)
        items.append(meta)
    items.sort(key=lambda m: m.get("created_at") or "", reverse=True)
    return items


def find_bundle(bundle_id: str) -> dict[str, Any] | None:
    base = backup_dir()
    path = base / bundle_id
    if path.is_dir():
        return load_meta(path)
    for m in list_bundles():
        if m.get("id") == bundle_id or m.get("_id") == bundle_id:
            return m
    return None


def active_rollback(current_version: str | None = None) -> dict[str, Any] | None:
    now = _utc_now()
    candidates = []
    for m in list_bundles():
        roles = m.get("roles") or []
        if "rollback" not in roles:
            continue
        if m.get("in_progress"):
            continue
        exp = _parse_iso(m.get("expires_at"))
        if exp and exp <= now:
            continue
        # Prefer rollback targeting current install (from_version is pre-upgrade)
        candidates.append(m)
    if not candidates:
        return None
    if current_version:
        # Prefer the rollback whose to_version matches current (we upgraded TO current)
        matched = [
            m
            for m in candidates
            if (m.get("to_version") or "").lstrip("v") == current_version.lstrip("v")
        ]
        if matched:
            candidates = matched
    candidates.sort(key=lambda m: m.get("created_at") or "", reverse=True)
    return candidates[0]


def register_from_bundle(
    source: Path,
    *,
    version: str,
    from_version: str,
    to_version: str,
    api_image: str,
    web_image: str,
    as_rollback: bool,
    as_version: bool,
) -> dict[str, Any]:
    """Copy an existing verified backup bundle into version-backups/ with META."""
    source = source.resolve()
    if not source.is_dir() or not (source / "db.sql.gz").is_file():
        raise SystemExit(f"source bundle missing db.sql.gz: {source}")
    base = backup_dir()
    base.mkdir(parents=True, exist_ok=True)
    bid = uuid.uuid4().hex[:8]
    ts = _utc_now().strftime("%Y%m%dT%H%M%SZ")
    name = f"vb-{sanitize_version(version)}-{ts}-{bid}"
    dest = base / name
    if dest.exists():
        raise SystemExit(f"refusing overwrite: {dest}")
    shutil.copytree(source, dest)
    roles: list[str] = []
    if as_version:
        roles.append("version")
    if as_rollback:
        roles.append("rollback")
    now = _utc_now()
    meta: dict[str, Any] = {
        "id": str(uuid.uuid4()),
        "roles": roles,
        "version": version.lstrip("v"),
        "from_version": from_version.lstrip("v"),
        "to_version": to_version.lstrip("v"),
        "created_at": now.isoformat().replace("+00:00", "Z"),
        "expires_at": (now + timedelta(days=rollback_days())).isoformat().replace("+00:00", "Z")
        if as_rollback
        else None,
        "api_image": api_image,
        "web_image": web_image,
        "in_progress": False,
        "source_bundle": str(source),
    }
    write_meta(dest, meta)
    meta["_path"] = str(dest)
    meta["_id"] = dest.name
    meta["size_bytes"] = dir_size(dest)
    return meta


def set_in_progress(bundle_id: str, value: bool) -> dict[str, Any]:
    meta = find_bundle(bundle_id)
    if not meta:
        raise SystemExit(f"bundle not found: {bundle_id}")
    path = Path(meta["_path"])
    meta["in_progress"] = bool(value)
    write_meta(path, meta)
    return load_meta(path) or meta


def prune(*, current_version: str = "") -> dict[str, Any]:
    """Expire rollback roles and prune excess version snapshots."""
    now = _utc_now()
    cur = current_version.lstrip("v")
    removed: list[str] = []
    kept: list[str] = []
    demoted: list[str] = []

    bundles = list_bundles()
    for m in bundles:
        path = Path(m["_path"])
        if m.get("in_progress"):
            kept.append(path.name)
            continue
        roles = list(m.get("roles") or [])
        changed = False
        if "rollback" in roles:
            exp = _parse_iso(m.get("expires_at"))
            if exp and exp <= now:
                roles.remove("rollback")
                m["roles"] = roles
                m["expires_at"] = None
                changed = True
                demoted.append(path.name)
        if changed:
            write_meta(path, m)

    # Refresh after demotions
    bundles = list_bundles()
    # Group version-role bundles by version (keep newest per version)
    by_version: dict[str, list[dict[str, Any]]] = {}
    for m in bundles:
        roles = m.get("roles") or []
        if "version" not in roles:
            continue
        ver = (m.get("version") or "").lstrip("v")
        by_version.setdefault(ver, []).append(m)
    for ver, group in by_version.items():
        group.sort(key=lambda x: x.get("created_at") or "", reverse=True)
        # Keep only newest per version for counting
        by_version[ver] = group

    # Order versions by newest backup created_at
    version_order = sorted(
        by_version.keys(),
        key=lambda v: by_version[v][0].get("created_at") or "",
        reverse=True,
    )
    keep_n = version_keep()
    retain_versions = set(version_order[:keep_n])
    if cur:
        retain_versions.add(cur)

    for ver, group in by_version.items():
        for idx, m in enumerate(group):
            path = Path(m["_path"])
            roles = list(m.get("roles") or [])
            # Always keep newest for retained versions; drop extras of same version
            protect = (
                m.get("in_progress")
                or "rollback" in roles  # unexpired rollback still listed
                or (ver in retain_versions and idx == 0)
            )
            if protect:
                kept.append(path.name)
                continue
            # Remove version role; delete dir if no roles left
            if "version" in roles:
                roles.remove("version")
            if not roles:
                shutil.rmtree(path, ignore_errors=True)
                removed.append(path.name)
            else:
                m["roles"] = roles
                write_meta(path, m)
                demoted.append(path.name)
                kept.append(path.name)

    # Delete orphan dirs with no roles
    for m in list_bundles():
        path = Path(m["_path"])
        roles = m.get("roles") or []
        if not roles and not m.get("in_progress"):
            shutil.rmtree(path, ignore_errors=True)
            removed.append(path.name)

    return {
        "removed": sorted(set(removed)),
        "demoted": sorted(set(demoted)),
        "kept": sorted(set(kept)),
        "rollback_days": rollback_days(),
        "version_keep": keep_n,
        "current_version": cur,
    }


def public_list(current_version: str = "") -> dict[str, Any]:
    items = []
    for m in list_bundles():
        items.append(
            {
                "id": m.get("_id") or m.get("id"),
                "meta_id": m.get("id"),
                "version": m.get("version"),
                "from_version": m.get("from_version"),
                "to_version": m.get("to_version"),
                "roles": m.get("roles") or [],
                "created_at": m.get("created_at"),
                "expires_at": m.get("expires_at"),
                "size_bytes": m.get("size_bytes") or dir_size(Path(m["_path"])),
                "api_image": m.get("api_image"),
                "web_image": m.get("web_image"),
                "in_progress": bool(m.get("in_progress")),
                "path": m.get("_path"),
            }
        )
    rb = active_rollback(current_version)
    return {
        "items": items,
        "rollback": None
        if not rb
        else {
            "id": rb.get("_id") or rb.get("id"),
            "version": rb.get("version") or rb.get("from_version"),
            "from_version": rb.get("from_version"),
            "to_version": rb.get("to_version"),
            "expires_at": rb.get("expires_at"),
            "created_at": rb.get("created_at"),
            "size_bytes": rb.get("size_bytes") or dir_size(Path(rb["_path"])),
            "path": rb.get("_path"),
        },
        "retention": {
            "rollback_days": rollback_days(),
            "version_count": version_keep(),
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Vulyk version-backup manager")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_list = sub.add_parser("list", help="List version backups as JSON")
    p_list.add_argument("--current-version", default="")

    p_reg = sub.add_parser("register", help="Register a verified bundle")
    p_reg.add_argument("--source", required=True)
    p_reg.add_argument("--version", required=True, help="Version of data in the bundle")
    p_reg.add_argument("--from-version", default="")
    p_reg.add_argument("--to-version", default="")
    p_reg.add_argument("--api-image", default="")
    p_reg.add_argument("--web-image", default="")
    p_reg.add_argument("--rollback", action="store_true")
    p_reg.add_argument("--no-version-role", action="store_true")

    p_prune = sub.add_parser("prune", help="Expire rollbacks and prune old versions")
    p_prune.add_argument("--current-version", default="")

    p_rb = sub.add_parser("active-rollback", help="Print active rollback point JSON")
    p_rb.add_argument("--current-version", default="")

    p_lock = sub.add_parser("set-in-progress")
    p_lock.add_argument("--id", required=True)
    p_lock.add_argument("--value", choices=["true", "false"], required=True)

    p_find = sub.add_parser("find")
    p_find.add_argument("--id", required=True)

    args = parser.parse_args(argv)

    if args.cmd == "list":
        print(json.dumps(public_list(args.current_version), indent=2))
    elif args.cmd == "register":
        meta = register_from_bundle(
            Path(args.source),
            version=args.version,
            from_version=args.from_version or args.version,
            to_version=args.to_version or "",
            api_image=args.api_image,
            web_image=args.web_image,
            as_rollback=args.rollback,
            as_version=not args.no_version_role,
        )
        print(json.dumps(meta, indent=2))
    elif args.cmd == "prune":
        print(json.dumps(prune(current_version=args.current_version), indent=2))
    elif args.cmd == "active-rollback":
        rb = active_rollback(args.current_version)
        print(json.dumps(rb, indent=2))
    elif args.cmd == "set-in-progress":
        print(
            json.dumps(
                set_in_progress(args.id, args.value == "true"),
                indent=2,
            )
        )
    elif args.cmd == "find":
        print(json.dumps(find_bundle(args.id), indent=2))
    return 0


if __name__ == "__main__":
    # Avoid unused import warning for time in some linters
    _ = time
    raise SystemExit(main())
