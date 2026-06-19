"""Shared pytest fixtures.

Integration tests run against a REAL uvicorn subprocess (so WebSocket + true
concurrency work) backed by a real Redis + a throwaway SQLite file. If Redis is
unreachable the integration tests skip (unit tests still run).
"""
from __future__ import annotations

import contextlib
import os
import socket
import subprocess
import sys
import time

import httpx
import pytest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if REPO not in sys.path:  # so `import app` / `import config` resolve under any pytest import mode
    sys.path.insert(0, REPO)
REDIS_URL = os.environ.get("REDIS_URL", "redis://localhost:6379/0")


def _redis_ok() -> bool:
    try:
        import redis  # noqa: PLC0415

        redis.Redis.from_url(REDIS_URL, socket_connect_timeout=2).ping()
        return True
    except Exception:
        return False


def _free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


@pytest.fixture(scope="session")
def live_server():
    """Boot the app on a free port; yield its base URL; tear down."""
    if not _redis_ok():
        pytest.skip("Redis not available (set REDIS_URL or start redis-server)")
    port = _free_port()
    db_path = f"/tmp/hookbox_test_{port}.db"
    env = {
        **os.environ,
        "DATABASE_PATH": db_path,
        "MOCK_DOMAIN": "mock.local",
        "APP_HOST": "localhost",
        "REDIS_URL": REDIS_URL,
    }
    proc = subprocess.Popen(
        [sys.executable, "-m", "uvicorn", "app.main:app",
         "--host", "127.0.0.1", "--port", str(port), "--log-level", "warning"],
        cwd=REPO, env=env,
    )
    base = f"http://127.0.0.1:{port}"
    deadline = time.time() + 30
    try:
        with httpx.Client() as c:
            while time.time() < deadline:
                if proc.poll() is not None:
                    pytest.fail("server process exited during startup")
                try:
                    if c.get(base + "/healthz", timeout=2).status_code == 200:
                        break
                except Exception:
                    time.sleep(0.3)
            else:
                proc.terminate()
                pytest.fail("server did not become healthy in 30s")
        yield base
    finally:
        proc.terminate()
        with contextlib.suppress(Exception):
            proc.wait(timeout=5)
        if proc.poll() is None:
            proc.kill()
        for p in (db_path, db_path + "-wal", db_path + "-shm"):
            with contextlib.suppress(Exception):
                os.remove(p)


@pytest.fixture
def client(live_server):
    with httpx.Client(base_url=live_server, timeout=10) as c:
        yield c


def auth(secret: str) -> dict:
    return {"Authorization": f"Bearer {secret}"}


@pytest.fixture
def owner(client, request):
    """(owner_secret, primary_token) for a UNIQUE-per-test email — the per-call
    secret rotation must not invalidate another test's capability."""
    safe = "".join(ch if ch.isalnum() else "-" for ch in request.node.name.lower())
    s = client.post("/api/session", json={"email": f"{safe}@example.com"}).json()
    token = (s.get("primary") or {}).get("token") or (s.get("endpoints") or [{}])[0].get("token")
    return s["owner_secret"], token
