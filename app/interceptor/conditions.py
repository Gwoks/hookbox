"""Simulated network conditions — latency · rate-limit · chaos (§5.5, AC-24/25/26/27c).

The interceptor applies these *around* the served body in the **frozen order**
(§5.5): **rate-limit → chaos → latency**. This module owns the three knobs; the
engine calls into it (``check_rate_limit`` / ``roll_chaos``) and reuses
``clamp_*``/``apply_latency`` so behavior is centralized and testable.

  * **Latency (AC-24):** ``apply_latency(ms)`` sleeps ``asyncio.sleep(ms/1000)`` —
    non-blocking, so a slow endpoint never stalls other endpoints' event loop. The
    value is clamped to ``[0, LATENCY_MAX_MS]`` (AC-27c).

  * **Rate limit (AC-25/AC-S19):** ``check_rate_limit`` delegates to the Redis Lua
    token bucket (``redis_state.rate_limit_check``) keyed per endpoint, or per rule
    when a ``rule_id`` override is in play; ``limit<=0`` means unlimited. Over-limit
    is surfaced by the engine as ``429`` + ``Retry-After`` + ``X-RateLimit-*``. The
    bucket is consulted on the engine's *effective* rate for **every** served path
    incl. MITM forwards and Auto-CRUD writes (AC-S19), and **fails OPEN** (bounded by
    the in-process body cap) on Redis loss (AC-S20 / §5.11).

  * **Chaos (AC-26, RESOLVED OQ-2):** ``roll_chaos(ep)`` rolls the injectable RNG
    against ``chaos_pct`` (0–100, clamped). On a hit the **default** outcome is a
    random failure from ``{502, 503, 504}`` (the engine returns JSON
    ``{error:"chaos"}``). A **connection-drop** (``dropout``) outcome is **opt-in**
    (an endpoint/rule carrying ``chaos_mode == "dropout"``) — ``roll_chaos`` then
    returns the sentinel ``"DROP"`` and the engine closes the connection, bounded by
    ``CHAOS_DROP_TIMEOUT_S`` so it can never hang a worker. ``chaos_pct=0`` never
    fires; ``chaos_pct=100`` always fires. Because ``chaos_mode`` is **not** part of
    the frozen §5.3 schema, it is read defensively via ``getattr(ep, "chaos_mode",
    "error")`` — the default is the contract-mandated random-5xx behavior and the
    frozen contract is untouched.

**Determinism (arch §9 R6):** the module RNG is a ``random.Random`` seam that tests
inject via :func:`set_rng` (e.g. ``set_rng(random.Random(0))``) so ``chaos_pct=100``
→ always fail and ``0`` → never, deterministically.

All knobs are also clamped server-side at the API boundary (``EndpointConfigPatch``
in ``app/models.py``); the clamps here are a defense-in-depth backstop so an
out-of-range value that somehow reaches the cache is never applied raw (AC-27c).
"""

from __future__ import annotations

import asyncio
import logging
import random
from typing import Optional, Union

import config
from app.redis_state import RateLimitResult, redis_state

logger = logging.getLogger("hookbox.conditions")

# Chaos failure statuses (§5.5 / OQ-2 default = random 5xx).
CHAOS_STATUSES = (502, 503, 504)
# Sentinel the engine recognizes for the opt-in connection-drop variant.
DROP = "DROP"

# Injectable RNG seam (arch §9 R6). Module-default uses the system CSPRNG-seeded
# Mersenne Twister; tests swap in a seeded ``random.Random`` for determinism.
_rng: random.Random = random.Random()


def set_rng(rng: random.Random) -> None:
    """Inject a deterministic RNG (tests). ``set_rng(random.Random(seed))``."""
    global _rng
    _rng = rng


def reset_rng() -> None:
    """Restore the default (non-deterministic) RNG."""
    global _rng
    _rng = random.Random()


# --- clamps (AC-27c — defense-in-depth backstop over the pydantic clamps) ------
def clamp_latency(ms: Optional[int]) -> int:
    """Clamp latency to ``[0, LATENCY_MAX_MS]``. ``None`` → 0."""
    if ms is None:
        return 0
    try:
        ms = int(ms)
    except (TypeError, ValueError):
        return 0
    return max(0, min(ms, config.LATENCY_MAX_MS))


def clamp_rate(limit: Optional[int]) -> int:
    """Clamp a rate limit to ``[0, RATE_LIMIT_MAX_PER_MIN]``. ``None``/<=0 → 0
    (unlimited)."""
    if limit is None:
        return 0
    try:
        limit = int(limit)
    except (TypeError, ValueError):
        return 0
    if limit <= 0:
        return 0
    return min(limit, config.RATE_LIMIT_MAX_PER_MIN)


def clamp_chaos(pct: Optional[int]) -> int:
    """Clamp a chaos percentage to ``[0, 100]``. ``None`` → 0."""
    if pct is None:
        return 0
    try:
        pct = int(pct)
    except (TypeError, ValueError):
        return 0
    return max(0, min(pct, config.CHAOS_MAX_PCT))


# --- latency (AC-24) ----------------------------------------------------------
async def apply_latency(ms: Optional[int]) -> int:
    """Sleep for the (clamped) latency and return the milliseconds actually applied.

    Uses ``asyncio.sleep`` so the wait is cooperative and never blocks other
    endpoints' requests on the event loop (AC-24). Returns the applied ms so the
    caller can record ``overhead_ms = duration - applied_latency``.
    """
    applied = clamp_latency(ms)
    if applied > 0:
        await asyncio.sleep(applied / 1000.0)
    return applied


# --- rate limit (AC-25 / AC-S19 / AC-S20) -------------------------------------
async def check_rate_limit(
    token: str, limit: int, rule_id: Optional[int] = None, window: int = 60
) -> RateLimitResult:
    """Token-bucket check for ``token`` (or ``token:rule_id`` when a rule overrides).

    ``limit<=0`` → unlimited (``allowed=True``). Fails **OPEN** (``allowed=True,
    degraded=True``) on Redis loss (AC-S20); the in-process ingest body cap still
    bounds the request elsewhere, so it is never unbounded. The engine maps a
    not-allowed result to ``429`` + ``Retry-After`` + ``X-RateLimit-Limit/Remaining``.
    """
    eff = clamp_rate(limit)
    if eff <= 0:
        return RateLimitResult(allowed=True, limit=0, remaining=-1, retry_after=0)
    try:
        return await redis_state.rate_limit_check(token, eff, window, rule_id)
    except Exception:  # noqa: BLE001 - never let the limiter crash the fast path
        logger.warning("rate-limit check error -> failing OPEN (bounded)")
        return RateLimitResult(allowed=True, limit=eff, remaining=eff, retry_after=0, degraded=True)


# --- chaos (AC-26 / OQ-2) -----------------------------------------------------
def _chaos_pct_of(ep) -> int:
    return clamp_chaos(getattr(ep, "chaos_pct", 0))


def _is_dropout_mode(ep) -> bool:
    """Whether the opt-in connection-drop variant is enabled for this endpoint/rule.

    Read defensively: ``chaos_mode`` is **not** a frozen §5.3 field, so its absence
    means the contract-default error-mode (random 5xx). Only an explicit
    ``chaos_mode == "dropout"`` opts into the connection-drop variant (OQ-2).
    """
    mode = getattr(ep, "chaos_mode", "error")
    return isinstance(mode, str) and mode.lower() == "dropout"


def roll_chaos(ep) -> Optional[Union[int, str]]:
    """Roll chaos for ``ep``. Returns:

      * an ``int`` status from ``{502,503,504}`` on a hit in **error** mode (default),
      * the sentinel ``"DROP"`` on a hit in opt-in **dropout** mode,
      * ``None`` when chaos does not fire.

    Deterministic under an injected seeded RNG (arch §9 R6): ``chaos_pct=100`` always
    fires, ``0`` never. **Bounded:** the engine subjects the chaos response to the
    same rate/size caps as every other P1 path, and the ``DROP`` close is bounded by
    ``CHAOS_DROP_TIMEOUT_S`` (AC-26 / OQ-2 — no unbounded abuse vector).
    """
    pct = _chaos_pct_of(ep)
    if pct <= 0:
        return None
    # randint(1,100) <= pct gives an exact integer-percent hit rate; pct=100 always.
    if _rng.randint(1, 100) > pct:
        return None
    if _is_dropout_mode(ep):
        return DROP
    return _rng.choice(CHAOS_STATUSES)
