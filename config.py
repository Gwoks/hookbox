"""Configuration for HookBox — Beeceptor-class API mocking & interception platform.

All values are env-driven with safe defaults (PRD AC-44). The app must never crash
on a missing env var; an unset/misconfigured ``MOCK_DOMAIN`` degrades to
path-fallback-only mode (a warning is logged at startup) rather than failing.
"""

import os
from pathlib import Path

# --- Base ---------------------------------------------------------------------
BASE_DIR = Path(__file__).parent


def _get_bool(name: str, default: bool) -> bool:
    raw = os.getenv(name)
    if raw is None:
        return default
    return raw.strip().lower() in ("1", "true", "yes", "on")


def _get_int(name: str, default: int) -> int:
    raw = os.getenv(name)
    if raw is None or raw.strip() == "":
        return default
    try:
        return int(raw)
    except (TypeError, ValueError):
        return default


# --- Hosting / routing (LOCKED §2, arch §3) -----------------------------------
# The wildcard mock surface lives under ``*.<MOCK_DOMAIN>``. The documented default
# is ``mock.local``. If unset/blank the app serves path-fallback-only mode.
MOCK_DOMAIN = os.getenv("MOCK_DOMAIN", "mock.local").strip()

# The canonical app host that serves the UI + management API. Used to resolve the
# apex/UI plane and to never let the mock catch-all shadow the UI/API.
APP_HOST = os.getenv("APP_HOST", "localhost").strip()

HOST = os.getenv("APP_BIND_HOST", "0.0.0.0")
PORT = _get_int("APP_PORT", 8000)

# Host names that always resolve to the UI plane (never the interceptor).
APP_HOSTS = {h for h in {
    "localhost",
    "127.0.0.1",
    "[::1]",
    APP_HOST,
    MOCK_DOMAIN,  # the bare apex serves the UI, not the interceptor
} if h}

# --- Durable store (SQLite WAL, §5.8) -----------------------------------------
DATABASE_PATH = Path(os.getenv("DATABASE_PATH", str(BASE_DIR / "data" / "hookbox.db")))

# --- Ephemeral store / pub-sub (Redis) ----------------------------------------
REDIS_URL = os.getenv("REDIS_URL", "redis://localhost:6379/0")

# --- Token / secret entropy ---------------------------------------------------
# generate_endpoint_id() uses an ambiguity-stripped alphabet (AC-6a).
ENDPOINT_ID_LENGTH = _get_int("ENDPOINT_ID_LENGTH", 10)
OWNER_SECRET_BYTES = _get_int("OWNER_SECRET_BYTES", 32)  # secrets.token_urlsafe(32) -> 256-bit

# --- Data retention (LOCKED §6, §5.8, AC-35/36/37) ----------------------------
TRACE_CAP = _get_int("TRACE_CAP", 100)                      # hard per-endpoint trace cap
TRACE_TTL_HOURS = _get_int("TRACE_TTL_HOURS", 24)           # 24h TTL sweep
RETENTION_SWEEP_SECONDS = _get_int("RETENTION_SWEEP_SECONDS", 300)

# --- Redis ephemeral TTLs (§5.8) ----------------------------------------------
STATE_TTL_SECONDS = _get_int("STATE_TTL_SECONDS", 24 * 3600)   # state:<token>
CRUD_TTL_SECONDS = _get_int("CRUD_TTL_SECONDS", 24 * 3600)     # crud:<token>:<coll>

# --- Simulated network condition bounds (§5.3, AC-27c) ------------------------
LATENCY_MAX_MS = _get_int("LATENCY_MAX_MS", 10000)
RATE_LIMIT_MAX_PER_MIN = _get_int("RATE_LIMIT_MAX_PER_MIN", 100000)
CHAOS_MAX_PCT = 100
# Server-side bound so an opt-in chaos "dropout" close can never hang a worker.
CHAOS_DROP_TIMEOUT_S = _get_int("CHAOS_DROP_TIMEOUT_S", 30)

# --- Ingest / body caps (AC-S18, AC-S11, AC-12b) ------------------------------
# Max P1 request body we will buffer before rejecting with 413.
MAX_INGEST_BODY_BYTES = _get_int("MAX_INGEST_BODY_BYTES", 1_000_000)
# Max captured request/response body persisted to a trace (truncated beyond this).
MAX_BODY_BYTES = _get_int("MAX_BODY_BYTES", 256_000)
# Templating DoS bounds (AC-S11).
TEMPLATE_MAX_SIZE = _get_int("TEMPLATE_MAX_SIZE", 256_000)
TEMPLATE_MAX_TAGS = _get_int("TEMPLATE_MAX_TAGS", 500)
# Auto-CRUD bounds (AC-12b).
CRUD_MAX_ITEMS = _get_int("CRUD_MAX_ITEMS", 1000)
CRUD_MAX_ITEM_BYTES = _get_int("CRUD_MAX_ITEM_BYTES", 64_000)

# --- MITM / proxy policy (security §4.3, AC-S6..S9) ---------------------------
MITM_TIMEOUT_S = _get_int("MITM_TIMEOUT_S", 10)
MITM_MAX_BODY_BYTES = _get_int("MITM_MAX_BODY_BYTES", 5_000_000)
MITM_ALLOW_PRIVATE = _get_bool("MITM_ALLOW_PRIVATE", False)
MITM_FOLLOW_REDIRECTS = _get_bool("MITM_FOLLOW_REDIRECTS", False)
MITM_MAX_REDIRECTS = _get_int("MITM_MAX_REDIRECTS", 3)

# --- Real-time feed bounds (AC-S22) -------------------------------------------
WS_MAX_CONN_PER_ENDPOINT = _get_int("WS_MAX_CONN_PER_ENDPOINT", 50)
WS_SEND_TIMEOUT_S = float(os.getenv("WS_SEND_TIMEOUT_S", "5"))

# --- Tunnel -------------------------------------------------------------------
TUNNEL_REQUEST_TIMEOUT_S = _get_int("TUNNEL_REQUEST_TIMEOUT_S", 30)

# --- Session anti-enumeration rate limit (AC-S5) ------------------------------
SESSION_RATE_LIMIT_PER_MIN = _get_int("SESSION_RATE_LIMIT_PER_MIN", 30)

# --- Derived flag -------------------------------------------------------------
# When MOCK_DOMAIN is blank/misconfigured we serve path-fallback-only mode.
PATH_FALLBACK_ONLY = not bool(MOCK_DOMAIN) or "." not in MOCK_DOMAIN
