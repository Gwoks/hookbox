"""SQLite data layer for HookBox (aiosqlite, WAL).

Owns the §5.8 durable schema (``owners``, ``endpoints``, ``mock_rules``,
``request_logs``), connection management, and the fire-and-forget trace writer.

Two access patterns (arch §7):
  * Management routes use the short-lived per-request ``get_db`` dependency.
  * The interceptor fast path opens **no** per-request DB connection — it reads
    from the in-process rule cache and writes traces through a single long-lived
    background connection owned by :class:`TraceWriter` (AC-39).

All SQL uses bound parameters only — no request-derived value is ever
interpolated via f-string / % / .format (AC-S23).
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import secrets
import string
from typing import Any, Optional

import aiosqlite

from config import DATABASE_PATH, OWNER_SECRET_BYTES, ENDPOINT_ID_LENGTH, TRACE_CAP

logger = logging.getLogger("hookbox.db")

DATABASE_PATH.parent.mkdir(parents=True, exist_ok=True)


# --- Schema (§5.8) ------------------------------------------------------------

_DDL = """
CREATE TABLE IF NOT EXISTS owners (
    owner_id    TEXT PRIMARY KEY,
    email       TEXT UNIQUE NOT NULL,
    secret_hash TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE TABLE IF NOT EXISTS endpoints (
    token              TEXT PRIMARY KEY,
    owner_id           TEXT NOT NULL,
    name               TEXT,
    auto_crud          INTEGER NOT NULL DEFAULT 0,
    target_url         TEXT,
    default_mode       TEXT NOT NULL DEFAULT 'mock_404',
    latency_ms         INTEGER NOT NULL DEFAULT 0,
    rate_limit_per_min INTEGER NOT NULL DEFAULT 0,
    chaos_pct          INTEGER NOT NULL DEFAULT 0,
    cors_enabled       INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    last_hit           TEXT,
    request_count      INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (owner_id) REFERENCES owners(owner_id)
);
CREATE INDEX IF NOT EXISTS idx_endpoints_owner ON endpoints(owner_id);

CREATE TABLE IF NOT EXISTS mock_rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL,
    name        TEXT,
    priority    INTEGER NOT NULL DEFAULT 100,
    enabled     INTEGER NOT NULL DEFAULT 1,
    match_json  TEXT NOT NULL DEFAULT '{}',
    response_json TEXT NOT NULL DEFAULT '{}',
    state_writes_json TEXT NOT NULL DEFAULT '[]',
    latency_ms  INTEGER,
    rate_limit_per_min INTEGER,
    webhook_json TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_rules_token ON mock_rules(token, priority, id);

CREATE TABLE IF NOT EXISTS request_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    token           TEXT NOT NULL,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    served_by       TEXT NOT NULL,
    matched_rule_id INTEGER,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    overhead_ms     INTEGER NOT NULL DEFAULT 0,
    request_headers TEXT,
    query_params    TEXT,
    request_body    TEXT,
    response_headers TEXT,
    response_body   TEXT,
    trace_json      TEXT,
    state_snapshot  TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_logs_token_id ON request_logs(token, id DESC);
CREATE INDEX IF NOT EXISTS idx_logs_created ON request_logs(created_at);
"""


async def _apply_pragmas(db: aiosqlite.Connection) -> None:
    await db.execute("PRAGMA journal_mode=WAL;")
    await db.execute("PRAGMA synchronous=NORMAL;")
    await db.execute("PRAGMA foreign_keys=ON;")


async def init_db() -> None:
    """Create the durable schema with WAL + foreign keys enabled."""
    async with aiosqlite.connect(DATABASE_PATH) as db:
        await _apply_pragmas(db)
        await db.executescript(_DDL)
        await db.commit()
    logger.info("HookBox database initialized at %s", DATABASE_PATH)


async def get_db():
    """Per-request short-lived connection dependency for management routes."""
    db = await aiosqlite.connect(DATABASE_PATH)
    db.row_factory = aiosqlite.Row
    await db.execute("PRAGMA foreign_keys=ON;")
    try:
        yield db
    finally:
        await db.close()


# --- Identity / token helpers -------------------------------------------------

def hash_email(email: str) -> str:
    """Non-secret owner id derived from email (sha256 → 16 hex). [reuses prior art]"""
    return hashlib.sha256(email.lower().strip().encode()).hexdigest()[:16]


def gen_owner_secret() -> str:
    """CSPRNG bearer capability (256-bit). Returned once on /api/session, stored hashed."""
    return secrets.token_urlsafe(OWNER_SECRET_BYTES)


def hash_secret(secret: str) -> str:
    """sha256 of the owner_secret for at-rest storage / lookup (§5.1)."""
    return hashlib.sha256(secret.encode()).hexdigest()


_TOKEN_ALPHABET = (
    (string.ascii_lowercase + string.ascii_uppercase + string.digits)
    .replace("0", "").replace("O", "")
    .replace("1", "").replace("l", "").replace("I", "")
)


def gen_token(length: int = ENDPOINT_ID_LENGTH) -> str:
    """Ambiguity-stripped endpoint token (AC-6a). Independent of owner id (AC-S5)."""
    return "".join(secrets.choice(_TOKEN_ALPHABET) for _ in range(length))


# Backwards-friendly alias used by some helpers.
def create_user_token() -> str:  # pragma: no cover - legacy name
    return gen_owner_secret()


# --- Fire-and-forget trace writer (AC-39, AC-35, AC-S23) ----------------------

_INSERT_TRACE_SQL = """
INSERT INTO request_logs
    (token, method, path, status_code, served_by, matched_rule_id,
     duration_ms, overhead_ms, request_headers, query_params, request_body,
     response_headers, response_body, trace_json, state_snapshot)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"""

# Write-time prune: delete this token's rows beyond the newest TRACE_CAP by id.
_PRUNE_SQL = """
DELETE FROM request_logs
WHERE token = ?
  AND id NOT IN (
      SELECT id FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ?
  )
"""


class TraceWriter:
    """Serialized writes over a single long-lived connection.

    The interceptor fast path never opens its own DB connection; instead it hands
    a fully-built trace dict to :meth:`insert_trace`, which is awaited only inside
    a fire-and-forget ``asyncio.create_task`` (never on the response path).
    """

    def __init__(self, db_path=DATABASE_PATH, trace_cap: int = TRACE_CAP) -> None:
        self._db_path = db_path
        self._trace_cap = trace_cap
        self._conn: Optional[aiosqlite.Connection] = None
        self._lock = asyncio.Lock()

    async def connect(self) -> None:
        if self._conn is not None:
            return
        self._conn = await aiosqlite.connect(self._db_path)
        await _apply_pragmas(self._conn)
        await self._conn.commit()

    async def close(self) -> None:
        if self._conn is not None:
            try:
                await self._conn.close()
            finally:
                self._conn = None

    @staticmethod
    def _dump(value: Any) -> Optional[str]:
        if value is None:
            return None
        if isinstance(value, str):
            return value
        return json.dumps(value, default=str)

    async def insert_trace(self, trace: dict) -> Optional[int]:
        """Insert one trace row and prune beyond the cap. Returns the new row id.

        Swallows-and-logs all errors (R4): a trace write must never crash the
        background task that wraps it.
        """
        if self._conn is None:
            await self.connect()
        params = (
            trace.get("token"),
            trace.get("method"),
            trace.get("path"),
            int(trace.get("status_code", 0)),
            trace.get("served_by"),
            trace.get("matched_rule_id"),
            int(trace.get("duration_ms", 0)),
            int(trace.get("overhead_ms", 0)),
            self._dump(trace.get("request_headers")),
            self._dump(trace.get("query_params")),
            trace.get("request_body"),
            self._dump(trace.get("response_headers")),
            trace.get("response_body"),
            self._dump(trace.get("trace")),
            self._dump(trace.get("state_snapshot")),
        )
        try:
            async with self._lock:
                cur = await self._conn.execute(_INSERT_TRACE_SQL, params)
                row_id = cur.lastrowid
                await cur.close()
                # Write-time prune to newest TRACE_CAP for this token (AC-35).
                await self._conn.execute(
                    _PRUNE_SQL,
                    (trace.get("token"), trace.get("token"), self._trace_cap),
                )
                await self._conn.commit()
            return row_id
        except Exception:  # noqa: BLE001 - telemetry write must never raise upward
            logger.exception("trace write failed (swallowed)")
            return None


# Module-level singleton, initialized in the app lifespan.
trace_writer = TraceWriter()
