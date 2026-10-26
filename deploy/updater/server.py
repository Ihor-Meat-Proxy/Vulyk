#!/usr/bin/env python3
"""Vulyk updater sidecar — authenticated internal HTTP wrapper around install.sh upgrade.

Does NOT expose Docker to the API container. Speaks only to callers that present
VULYK_UPDATER_TOKEN. Runs the same flow as:
  ./scripts/install.sh upgrade --yes --version X.Y.Z
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

ROOT = Path(os.environ.get("VULYK_ROOT", "/vulyk"))
TOKEN = os.environ.get("VULYK_UPDATER_TOKEN", "")
BIND = os.environ.get("VULYK_UPDATER_BIND", "0.0.0.0:9091")
INSTALL = ROOT / "scripts" / "install.sh"

_lock = threading.Lock()
_jobs: dict[str, dict[str, Any]] = {}
_current: str | None = None

SEMVER = re.compile(r"^v?\d+\.\d+\.\d+([.-].*)?$")


def _now() -> float:
    return time.time()


def _auth_ok(handler: BaseHTTPRequestHandler) -> bool:
    if not TOKEN:
        return False
    got = handler.headers.get("X-Vulyk-Updater-Token") or handler.headers.get(
        "Authorization", ""
    ).removeprefix("Bearer ").strip()
    return got == TOKEN


def _json(handler: BaseHTTPRequestHandler, code: int, body: dict[str, Any]) -> None:
    raw = json.dumps(body).encode("utf-8")
    handler.send_response(code)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(raw)))
    handler.end_headers()
    handler.wfile.write(raw)


def _read_json(handler: BaseHTTPRequestHandler) -> dict[str, Any]:
    length = int(handler.headers.get("Content-Length") or "0")
    if length <= 0:
        return {}
    data = handler.rfile.read(length)
    return json.loads(data.decode("utf-8") or "{}")


def _run_upgrade(job_id: str, version: str) -> None:
    global _current
    job = _jobs[job_id]
    log_path = Path(f"/tmp/vulyk-upgrade-{job_id}.log")
    cmd = ["bash", str(INSTALL), "upgrade", "--yes", "--version", version]
    job["status"] = "running"
    job["message"] = f"Starting upgrade to {version}"
    job["progress"] = 5
    job["updated_at"] = _now()
    try:
        with log_path.open("w", encoding="utf-8") as logf:
            logf.write(f"+ {' '.join(cmd)}\n")
            logf.flush()
            proc = subprocess.Popen(
                cmd,
                cwd=str(ROOT),
                stdout=logf,
                stderr=subprocess.STDOUT,
                env={**os.environ},
            )
            # Poll progress while running
            while proc.poll() is None:
                job["log"] = log_path.read_text(encoding="utf-8", errors="replace")[-20000:]
                job["progress"] = min(90, int(job.get("progress", 5)) + 2)
                job["updated_at"] = _now()
                time.sleep(2)
            rc = proc.returncode
            job["log"] = log_path.read_text(encoding="utf-8", errors="replace")[-50000:]
            if rc == 0:
                job["status"] = "succeeded"
                job["progress"] = 100
                job["message"] = f"Upgraded to {version}"
            else:
                # install.sh auto-rolls back; surface as failed/rolled_back
                rolled = "AUTO-ROLLBACK" in job["log"] or "automatic rollback" in job["log"]
                job["status"] = "rolled_back" if rolled else "failed"
                job["progress"] = 100
                job["message"] = f"Upgrade failed (exit {rc})"
    except Exception as exc:  # noqa: BLE001
        job["status"] = "failed"
        job["message"] = f"updater error: {exc}"
        job["progress"] = 100
    finally:
        job["finished_at"] = _now()
        job["updated_at"] = _now()
        with _lock:
            if _current == job_id:
                _current = None


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt: str, *args: Any) -> None:  # noqa: A003
        print(f"updater: {self.address_string()} - {fmt % args}", flush=True)

    def do_GET(self) -> None:  # noqa: N802
        path = urlparse(self.path).path
        if path == "/health":
            _json(self, 200, {"status": "ok", "service": "vulyk-updater"})
            return
        if not _auth_ok(self):
            _json(self, 401, {"error": "unauthorized"})
            return
        if path == "/v1/capabilities":
            _json(
                self,
                200,
                {
                    "upgrade": True,
                    "install_script": INSTALL.is_file(),
                    "root": str(ROOT),
                },
            )
            return
        if path.startswith("/v1/jobs/"):
            job_id = path.rsplit("/", 1)[-1]
            job = _jobs.get(job_id)
            if not job:
                _json(self, 404, {"error": "job not found"})
                return
            _json(self, 200, job)
            return
        if path == "/v1/jobs":
            items = sorted(_jobs.values(), key=lambda j: j.get("created_at", 0), reverse=True)
            _json(self, 200, {"items": items[:50]})
            return
        _json(self, 404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        path = urlparse(self.path).path
        if not _auth_ok(self):
            _json(self, 401, {"error": "unauthorized"})
            return
        if path != "/v1/upgrade":
            _json(self, 404, {"error": "not found"})
            return
        body = _read_json(self)
        version = str(body.get("version") or "").strip()
        if not SEMVER.match(version):
            _json(self, 400, {"error": "invalid version"})
            return
        version = version[1:] if version.startswith("v") else version
        if not INSTALL.is_file():
            _json(self, 500, {"error": "install.sh missing in VULYK_ROOT"})
            return
        global _current
        with _lock:
            if _current is not None:
                _json(self, 409, {"error": "upgrade already running", "job_id": _current})
                return
            job_id = str(uuid.uuid4())
            _current = job_id
            _jobs[job_id] = {
                "id": job_id,
                "target_version": version,
                "status": "pending",
                "progress": 0,
                "message": "queued",
                "log": "",
                "created_at": _now(),
                "updated_at": _now(),
                "finished_at": None,
            }
        threading.Thread(target=_run_upgrade, args=(job_id, version), daemon=True).start()
        _json(self, 202, {"job_id": job_id, "target_version": version})


def main() -> None:
    if not TOKEN:
        raise SystemExit("VULYK_UPDATER_TOKEN is required")
    host, _, port_s = BIND.partition(":")
    port = int(port_s or "9091")
    server = ThreadingHTTPServer((host or "0.0.0.0", port), Handler)
    print(f"vulyk-updater listening on {host}:{port} root={ROOT}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
