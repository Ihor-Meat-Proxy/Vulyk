#!/usr/bin/env python3
"""Unit tests for version-backup prune / expiry (no Docker)."""

from __future__ import annotations

import json
import os
import shutil
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

# Import sibling module
import version_backups as vb


def _write_bundle(base: Path, name: str, meta: dict) -> Path:
    path = base / name
    path.mkdir(parents=True)
    (path / "db.sql.gz").write_bytes(b"\x1f\x8b" + b"\x00" * 20)
    (path / "config").mkdir()
    (path / "config" / ".env").write_text("VULYK_SECRETS_KEY=test\n", encoding="utf-8")
    vb.write_meta(path, meta)
    return path


class VersionBackupTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="vulyk-vb-"))
        os.environ["VULYK_VERSION_BACKUP_DIR"] = str(self.tmp)
        os.environ["VULYK_ROLLBACK_RETENTION_DAYS"] = "7"
        os.environ["VULYK_VERSION_BACKUP_COUNT"] = "3"

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_prune_expires_rollback_but_keeps_version_within_count(self) -> None:
        now = datetime.now(timezone.utc)
        expired = (now - timedelta(days=1)).isoformat().replace("+00:00", "Z")
        _write_bundle(
            self.tmp,
            "vb-0.1.0-old-aaaaaaaa",
            {
                "id": "a",
                "roles": ["rollback", "version"],
                "version": "0.1.0",
                "from_version": "0.1.0",
                "to_version": "0.2.0",
                "created_at": (now - timedelta(days=10)).isoformat().replace("+00:00", "Z"),
                "expires_at": expired,
                "in_progress": False,
                "api_image": "vulyk-api:0.1.0",
                "web_image": "vulyk-web:0.1.0",
            },
        )
        result = vb.prune(current_version="0.2.0")
        meta = vb.load_meta(self.tmp / "vb-0.1.0-old-aaaaaaaa")
        assert meta is not None
        self.assertNotIn("rollback", meta["roles"])
        self.assertIn("version", meta["roles"])
        self.assertIn("vb-0.1.0-old-aaaaaaaa", result["demoted"])

    def test_never_delete_in_progress_rollback(self) -> None:
        now = datetime.now(timezone.utc)
        expired = (now - timedelta(days=1)).isoformat().replace("+00:00", "Z")
        _write_bundle(
            self.tmp,
            "vb-0.1.0-busy-bbbbbbbb",
            {
                "id": "b",
                "roles": ["rollback"],
                "version": "0.1.0",
                "from_version": "0.1.0",
                "to_version": "0.2.0",
                "created_at": now.isoformat().replace("+00:00", "Z"),
                "expires_at": expired,
                "in_progress": True,
                "api_image": "",
                "web_image": "",
            },
        )
        vb.prune(current_version="0.2.0")
        self.assertTrue((self.tmp / "vb-0.1.0-busy-bbbbbbbb").is_dir())
        meta = vb.load_meta(self.tmp / "vb-0.1.0-busy-bbbbbbbb")
        assert meta is not None
        self.assertIn("rollback", meta["roles"])

    def test_keeps_last_n_versions_and_current(self) -> None:
        now = datetime.now(timezone.utc)
        for i, ver in enumerate(["0.1.0", "0.2.0", "0.3.0", "0.4.0"]):
            _write_bundle(
                self.tmp,
                f"vb-{ver}-t-cccccccc",
                {
                    "id": ver,
                    "roles": ["version"],
                    "version": ver,
                    "from_version": ver,
                    "to_version": "",
                    "created_at": (now - timedelta(days=4 - i))
                    .isoformat()
                    .replace("+00:00", "Z"),
                    "expires_at": None,
                    "in_progress": False,
                    "api_image": "",
                    "web_image": "",
                },
            )
        # keep 3; current 0.4.0 must stay; 0.1.0 is oldest and should go
        result = vb.prune(current_version="0.4.0")
        names = {p.name for p in self.tmp.iterdir() if p.is_dir()}
        self.assertNotIn("vb-0.1.0-t-cccccccc", names)
        self.assertIn("vb-0.2.0-t-cccccccc", names)
        self.assertIn("vb-0.3.0-t-cccccccc", names)
        self.assertIn("vb-0.4.0-t-cccccccc", names)
        self.assertIn("vb-0.1.0-t-cccccccc", result["removed"])

    def test_unexpired_rollback_survives_even_if_beyond_version_count(self) -> None:
        now = datetime.now(timezone.utc)
        future = (now + timedelta(days=3)).isoformat().replace("+00:00", "Z")
        _write_bundle(
            self.tmp,
            "vb-0.0.1-rb-dddddddd",
            {
                "id": "old",
                "roles": ["rollback", "version"],
                "version": "0.0.1",
                "from_version": "0.0.1",
                "to_version": "0.5.0",
                "created_at": (now - timedelta(days=30))
                .isoformat()
                .replace("+00:00", "Z"),
                "expires_at": future,
                "in_progress": False,
                "api_image": "",
                "web_image": "",
            },
        )
        for i, ver in enumerate(["0.2.0", "0.3.0", "0.4.0", "0.5.0"]):
            _write_bundle(
                self.tmp,
                f"vb-{ver}-n-eeeeeeee",
                {
                    "id": ver,
                    "roles": ["version"],
                    "version": ver,
                    "from_version": ver,
                    "to_version": "",
                    "created_at": (now - timedelta(days=3 - i))
                    .isoformat()
                    .replace("+00:00", "Z"),
                    "expires_at": None,
                    "in_progress": False,
                    "api_image": "",
                    "web_image": "",
                },
            )
        vb.prune(current_version="0.5.0")
        self.assertTrue((self.tmp / "vb-0.0.1-rb-dddddddd").is_dir())
        meta = vb.load_meta(self.tmp / "vb-0.0.1-rb-dddddddd")
        assert meta is not None
        self.assertIn("rollback", meta["roles"])

    def test_register_and_active_rollback(self) -> None:
        src = self.tmp / "src-bundle"
        src.mkdir()
        (src / "db.sql.gz").write_bytes(b"\x1f\x8b" + b"\x00" * 8)
        (src / "config").mkdir()
        meta = vb.register_from_bundle(
            src,
            version="0.1.0",
            from_version="0.1.0",
            to_version="0.2.0",
            api_image="ghcr.io/x/vulyk-api:0.1.0",
            web_image="ghcr.io/x/vulyk-web:0.1.0",
            as_rollback=True,
            as_version=True,
        )
        self.assertIn("rollback", meta["roles"])
        rb = vb.active_rollback("0.2.0")
        assert rb is not None
        self.assertEqual(rb["from_version"], "0.1.0")
        listing = vb.public_list("0.2.0")
        self.assertEqual(len(listing["items"]), 1)
        self.assertIsNotNone(listing["rollback"])


if __name__ == "__main__":
    unittest.main()
