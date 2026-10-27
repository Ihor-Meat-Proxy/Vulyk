#!/usr/bin/env python3
"""Parse .env.example annotations and sync operator .env files.

Source of truth: `.env.example` (see docs/guides/upgrade.md).

Markers (same line as KEY=..., or the immediately preceding comment line):

  @vulyk:sync              include this key in install/upgrade sync even if commented
  @vulyk:secret            when adding a missing key, generate a random secret if default empty
  @vulyk:required          preflight treats missing/empty as a config gap

Every *uncommented* `KEY=value` line is synced by default (unless `@vulyk:nosync`).
Commented keys are synced only when marked `@vulyk:sync`.

Never overwrites existing keys. Subcommands: list | sync | gaps | validate.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_EXAMPLE = ROOT / ".env.example"

ASSIGN_RE = re.compile(
    r"^(?P<commented>#\s*)?(?P<key>[A-Za-z_][A-Za-z0-9_]*)=(?P<value>.*)$"
)
MARKER_RE = re.compile(r"@vulyk:(?P<flag>sync|secret|required|nosync)\b")


@dataclass
class EnvSetting:
    key: str
    default: str
    secret: bool
    required: bool
    sync: bool
    commented_in_example: bool


def _strip_inline_comment(value: str) -> str:
    """Strip unquoted trailing # comment from an env value."""
    out: list[str] = []
    in_single = False
    in_double = False
    i = 0
    while i < len(value):
        ch = value[i]
        if ch == "'" and not in_double:
            in_single = not in_single
            out.append(ch)
        elif ch == '"' and not in_single:
            in_double = not in_double
            out.append(ch)
        elif ch == "#" and not in_single and not in_double:
            break
        else:
            out.append(ch)
        i += 1
    return "".join(out).rstrip()


def parse_env_example(path: Path) -> list[EnvSetting]:
    lines = path.read_text(encoding="utf-8").splitlines()
    pending_flags: set[str] = set()
    by_key: dict[str, EnvSetting] = {}

    for raw in lines:
        stripped = raw.strip()
        if not stripped:
            pending_flags = set()
            continue

        flags_on_line = {m.group("flag") for m in MARKER_RE.finditer(stripped)}
        m = ASSIGN_RE.match(stripped)
        if m is None:
            # Marker-only comment accumulates for the next KEY= line.
            if stripped.startswith("#") and flags_on_line:
                pending_flags |= flags_on_line
            else:
                pending_flags = set()
            continue

        key = m.group("key")
        commented = bool(m.group("commented"))
        raw_value = m.group("value")
        flags = set(pending_flags) | flags_on_line
        pending_flags = set()
        value = _strip_inline_comment(raw_value).strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
            value = value[1:-1]

        sync = (not commented and "nosync" not in flags) or ("sync" in flags)
        if "nosync" in flags:
            sync = False

        by_key[key] = EnvSetting(
            key=key,
            default=value,
            secret="secret" in flags,
            required="required" in flags,
            sync=sync,
            commented_in_example=commented,
        )

    return list(by_key.values())


def read_env_keys(path: Path) -> dict[str, str]:
    if not path.is_file():
        return {}
    out: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if "=" not in stripped:
            continue
        key, _, value = stripped.partition("=")
        key = key.strip()
        if key:
            out[key] = value
    return out


def random_secret() -> str:
    return secrets.token_hex(32)


def sync_env(
    env_path: Path,
    example_path: Path,
    *,
    dry_run: bool = False,
) -> list[dict[str, str]]:
    settings = [s for s in parse_env_example(example_path) if s.sync]
    existing = read_env_keys(env_path)
    added: list[dict[str, str]] = []

    lines_to_append: list[str] = []
    for s in settings:
        if s.key in existing:
            continue
        if s.secret and not s.default:
            value = random_secret()
            how = "generated-secret"
        else:
            value = s.default
            how = "default"
        added.append({"key": s.key, "how": how})
        lines_to_append.append(f"{s.key}={value}")

    if dry_run or not lines_to_append:
        return added

    env_path.parent.mkdir(parents=True, exist_ok=True)
    with env_path.open("a", encoding="utf-8") as fh:
        if env_path.stat().st_size > 0:
            fh.write("\n")
        fh.write("# --- added by install.sh / env_settings.py (upgrade-safe sync) ---\n")
        for line in lines_to_append:
            fh.write(line + "\n")
    try:
        os.chmod(env_path, 0o600)
    except OSError:
        pass
    return added


def config_gaps(env_path: Path, example_path: Path) -> list[str]:
    """Return human-readable gaps for @vulyk:required keys + critical secrets."""
    settings = {s.key: s for s in parse_env_example(example_path)}
    existing = read_env_keys(env_path)
    gaps: list[str] = []

    for s in settings.values():
        if not s.required:
            continue
        val = existing.get(s.key)
        if val is None:
            gaps.append(f"{s.key} (missing)")
        elif not str(val).strip():
            gaps.append(f"{s.key} (empty)")

    for critical in ("VULYK_JWT_SECRET", "VULYK_SECRETS_KEY"):
        val = existing.get(critical)
        if val is None:
            gaps.append(f"{critical} (missing)")
        elif not str(val).strip():
            gaps.append(f"{critical} (empty)")

    # De-dupe while preserving order
    seen: set[str] = set()
    out: list[str] = []
    for g in gaps:
        if g not in seen:
            seen.add(g)
            out.append(g)
    return out


def compose_required_vars(compose_paths: list[Path]) -> set[str]:
    """Vars referenced as ${VAR:?...} (unset/empty fails compose parse)."""
    found: set[str] = set()
    pat = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\?:")
    for path in compose_paths:
        if not path.is_file():
            continue
        found.update(pat.findall(path.read_text(encoding="utf-8")))
    return found


def validate_example(example_path: Path, compose_paths: list[Path]) -> list[str]:
    """Fail when a new required/compose-critical key cannot be defaulted safely."""
    settings = {s.key: s for s in parse_env_example(example_path)}
    errors: list[str] = []
    for key, s in settings.items():
        if not s.sync:
            continue
        # Required keys must be secret-generated or have a non-empty default.
        if s.required and not s.default and not s.secret:
            errors.append(
                f"{key}: @vulyk:required synced key has empty default and is not "
                f"@vulyk:secret"
            )
    for key in sorted(compose_required_vars(compose_paths)):
        s = settings.get(key)
        if s is None or not s.sync:
            errors.append(
                f"{key}: compose uses ${{{key}:?...}} but .env.example does not "
                f"@vulyk:sync this key with a default or @vulyk:secret"
            )
        elif not s.default and not s.secret:
            errors.append(
                f"{key}: compose requires ${{{key}:?...}} but sync default is empty "
                f"without @vulyk:secret"
            )
    return errors


def _add_example_arg(p: argparse.ArgumentParser) -> None:
    p.add_argument(
        "--example",
        type=Path,
        default=DEFAULT_EXAMPLE,
        help="path to .env.example",
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_list = sub.add_parser("list", help="print sync settings as JSON")
    _add_example_arg(p_list)
    p_list.add_argument("--sync-only", action="store_true")

    p_sync = sub.add_parser("sync", help="append missing keys to an env file")
    _add_example_arg(p_sync)
    p_sync.add_argument("env_file", type=Path)
    p_sync.add_argument("--dry-run", action="store_true")

    p_gaps = sub.add_parser("gaps", help="report required/critical config gaps")
    _add_example_arg(p_gaps)
    p_gaps.add_argument("env_file", type=Path)

    p_val = sub.add_parser(
        "validate",
        help="fail if synced keys lack defaults/secrets or compose ?: keys are unsynced",
    )
    _add_example_arg(p_val)
    p_val.add_argument(
        "--compose",
        type=Path,
        nargs="*",
        default=[ROOT / "docker-compose.yml", ROOT / "docker-compose.prod.yml"],
    )

    args = ap.parse_args()

    if args.cmd == "list":
        settings = parse_env_example(args.example)
        if args.sync_only:
            settings = [s for s in settings if s.sync]
        json.dump([asdict(s) for s in settings], sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0

    if args.cmd == "sync":
        added = sync_env(args.env_file, args.example, dry_run=args.dry_run)
        json.dump({"added": added}, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0

    if args.cmd == "gaps":
        gaps = config_gaps(args.env_file, args.example)
        json.dump({"gaps": gaps}, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 1 if gaps else 0

    if args.cmd == "validate":
        errors = validate_example(args.example, list(args.compose))
        json.dump({"errors": errors}, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 1 if errors else 0

    return 2


if __name__ == "__main__":
    sys.exit(main())
