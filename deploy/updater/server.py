#!/usr/bin/env python3
"""Vulyk updater sidecar — authenticated wrapper around install.sh upgrade/rollback.

Does NOT expose Docker to the API container. Speaks only to callers that present
VULYK_UPDATER_TOKEN. Runs the same flows as:
  ./scripts/install.sh upgrade --yes --version X.Y.Z
  ./scripts/install.sh rollback --confirm=ROLLBACK [--version X.Y.Z]
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import tarfile
import tempfile
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse

ROOT = Path(os.environ.get("VULYK_ROOT", "/vulyk"))
TOKEN = os.environ.get("VULYK_UPDATER_TOKEN", "")
BIND = os.environ.get("VULYK_UPDATER_BIND", "0.0.0.0:9091")
INSTALL = ROOT / "scripts" / "install.sh"
VB_PY = ROOT / "scripts" / "version_backups.py"

_lock = threading.Lock()
_jobs: dict[str, dict[str, Any]] = {}
_current: str | None = None

SEMVER = re.compile(r"^v?\d+\.\d+\.\d+([.-].*)?$")
SAFE_ID = re.compile(r"^vb-[A-Za-z0-9._-]+$")


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


def _vb_env() -> dict[str, str]:
    env = {**os.environ, "VULYK_ROOT": str(ROOT)}
    return env


def _vb_cmd(*args: str) -> dict[str, Any]:
    if not VB_PY.is_file():
        raise FileNotFoundError("version_backups.py missing")
    proc = subprocess.run(
        ["python3", str(VB_PY), *args],
        cwd=str(ROOT),
        env=_vb_env(),
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "vb failed")
    text = proc.stdout.strip() or "null"
    return json.loads(text)


def _run_job(job_id: str, cmd: list[str], success_msg: str) -> None:
    global _current
    job = _jobs[job_id]
    log_path = Path(f"/tmp/vulyk-job-{job_id}.log")
    job["status"] = "running"
    job["message"] = " ".join(cmd)
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
                job["message"] = success_msg
            else:
                rolled = "AUTO-ROLLBACK" in job["log"] or "automatic rollback" in job["log"]
                job["status"] = "rolled_back" if rolled else "failed"
                job["progress"] = 100
                job["message"] = f"Command failed (exit {rc})"
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


def _start_job(kind: str, target: str, cmd: list[str], success_msg: str) -> dict[str, Any]:
    global _current
    with _lock:
        if _current is not None:
            raise RuntimeError(f"busy:{_current}")
        job_id = str(uuid.uuid4())
        _current = job_id
        _jobs[job_id] = {
            "id": job_id,
            "kind": kind,
            "target_version": target,
            "status": "pending",
            "progress": 0,
            "message": "queued",
            "log": "",
            "created_at": _now(),
            "updated_at": _now(),
            "finished_at": None,
        }
    threading.Thread(
        target=_run_job, args=(job_id, cmd, success_msg), daemon=True
    ).start()
    return {"job_id": job_id, "target_version": target, "kind": kind}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt: str, *args: Any) -> None:  # noqa: A003
        print(f"updater: {self.address_string()} - {fmt % args}", flush=True)

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        path = parsed.path
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
                    "rollback": True,
                    "version_backups": VB_PY.is_file(),
                    "install_script": INSTALL.is_file(),
                    "root": str(ROOT),
                },
            )
            return
        if path == "/v1/version-backups":
            qs = parse_qs(parsed.query)
            current = (qs.get("current_version") or [""])[0]
            try:
                _json(self, 200, _vb_cmd("list", "--current-version", current))
            except Exception as exc:  # noqa: BLE001
                _json(self, 500, {"error": str(exc)})
            return
        if path.startswith("/v1/version-backups/") and path.endswith("/download"):
            bundle_id = path[len("/v1/version-backups/") : -len("/download")]
            if not SAFE_ID.match(bundle_id):
                _json(self, 400, {"error": "invalid bundle id"})
                return
            try:
                meta = _vb_cmd("find", "--id", bundle_id)
            except Exception as exc:  # noqa: BLE001
                _json(self, 404, {"error": str(exc)})
                return
            if not meta:
                _json(self, 404, {"error": "not found"})
                return
            src = Path(meta["_path"])
            if not src.is_dir():
                _json(self, 404, {"error": "bundle path missing"})
                return
            with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
                tmp_path = Path(tmp.name)
            try:
                with tarfile.open(tmp_path, "w:gz") as tar:
                    tar.add(src, arcname=src.name)
                data = tmp_path.read_bytes()
            finally:
                tmp_path.unlink(missing_ok=True)
            self.send_response(200)
            self.send_header("Content-Type", "application/gzip")
            self.send_header(
                "Content-Disposition",
                f'attachment; filename="{bundle_id}.tar.gz"',
            )
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
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
        if path == "/v1/version-backups/prune":
            body = _read_json(self)
            current = str(body.get("current_version") or "")
            try:
                _json(self, 200, _vb_cmd("prune", "--current-version", current))
            except Exception as exc:  # noqa: BLE001
                _json(self, 500, {"error": str(exc)})
            return
        if path == "/v1/upgrade":
            body = _read_json(self)
            version = str(body.get("version") or "").strip()
            if not SEMVER.match(version):
                _json(self, 400, {"error": "invalid version"})
                return
            version = version[1:] if version.startswith("v") else version
            if not INSTALL.is_file():
                _json(self, 500, {"error": "install.sh missing in VULYK_ROOT"})
                return
            try:
                started = _start_job(
                    "upgrade",
                    version,
                    ["bash", str(INSTALL), "upgrade", "--yes", "--version", version],
                    f"Upgraded to {version}",
                )
            except RuntimeError as exc:
                msg = str(exc)
                if msg.startswith("busy:"):
                    _json(
                        self,
                        409,
                        {"error": "upgrade already running", "job_id": msg.split(":", 1)[1]},
                    )
                    return
                _json(self, 500, {"error": msg})
                return
            _json(self, 202, started)
            return
        if path == "/v1/rollback":
            body = _read_json(self)
            if str(body.get("confirm") or "") != "ROLLBACK":
                _json(self, 400, {"error": "confirm must be ROLLBACK"})
                return
            version = str(body.get("version") or "").strip()
            if version:
                if not SEMVER.match(version):
                    _json(self, 400, {"error": "invalid version"})
                    return
                version = version[1:] if version.startswith("v") else version
            if not INSTALL.is_file():
                _json(self, 500, {"error": "install.sh missing in VULYK_ROOT"})
                return
            cmd = [
                "bash",
                str(INSTALL),
                "rollback",
                "--confirm=ROLLBACK",
            ]
            if version:
                cmd.extend(["--version", version])
            try:
                started = _start_job(
                    "rollback",
                    version or "active",
                    cmd,
                    f"Rolled back to {version or 'active rollback point'}",
                )
            except RuntimeError as exc:
                msg = str(exc)
                if msg.startswith("busy:"):
                    _json(
                        self,
                        409,
                        {"error": "job already running", "job_id": msg.split(":", 1)[1]},
                    )
                    return
                _json(self, 500, {"error": msg})
                return
            _json(self, 202, started)
            return
        _json(self, 404, {"error": "not found"})


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
