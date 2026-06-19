"""Data-retention sweep (§5.8, AC-35/36/37) — enforces BOTH caps on an interval.

Two caps, both configurable and documented (LOCKED §6):
  * **24-hour TTL** — delete ``request_logs`` rows older than ``TRACE_TTL_HOURS``.
  * **100-trace per-endpoint cap** — keep only the newest ``TRACE_CAP`` traces per
    token (the write-time prune in ``TraceWriter.insert_trace`` holds the cap
    *between* sweeps; this is the periodic backstop).

The sweep runs every ``RETENTION_SWEEP_SECONDS`` (default 300) in a background
task started from the app lifespan. It uses bound parameters everywhere a value is
involved (AC-S23); the only interpolated tokens are integers we own from config
(``TRACE_TTL_HOURS``/``TRACE_CAP``), never request-derived input.

The sweep opens its own short-lived connection per pass (it is infrequent, off the
fast path) and never crashes the app on a DB hiccup (logged-and-continue).
"""

from __future__ import annotations

import asyncio
import logging

import aiosqlite

from config import DATABASE_PATH, RETENTION_SWEEP_SECONDS, TRACE_CAP, TRACE_TTL_HOURS

logger = logging.getLogger("hookbox.cleanup")


# 24h TTL sweep (arch §5.8). The interval is an int from config (validated), bound
# into the modifier string — no request-derived value is interpolated (AC-S23).
def _ttl_sql(ttl_hours: int) -> str:
    hours = max(0, int(ttl_hours))
    return f"DELETE FROM request_logs WHERE created_at < datetime('now', '-{hours} hours')"


# 100-cap per endpoint: delete rows that are NOT among the newest TRACE_CAP by id
# for their own token. ``?`` binds the cap.
_CAP_SQL = """
DELETE FROM request_logs
WHERE id IN (
    SELECT rl.id FROM request_logs rl
    WHERE rl.id NOT IN (
        SELECT r2.id FROM request_logs r2
        WHERE r2.token = rl.token
        ORDER BY r2.id DESC
        LIMIT ?
    )
)
"""


async def sweep_once() -> tuple[int, int]:
    """Run one retention pass enforcing both caps. Returns ``(ttl_deleted,
    cap_deleted)``. Never raises (logs + returns zeros on error)."""
    try:
        async with aiosqlite.connect(DATABASE_PATH) as db:
            # 24h TTL (AC-36).
            cur = await db.execute(_ttl_sql(TRACE_TTL_HOURS))
            ttl_deleted = cur.rowcount or 0
            # 100-cap per endpoint (AC-35).
            cur = await db.execute(_CAP_SQL, (TRACE_CAP,))
            cap_deleted = cur.rowcount or 0
            await db.commit()
        if ttl_deleted or cap_deleted:
            logger.info(
                "retention sweep: removed %d expired + %d over-cap traces",
                ttl_deleted, cap_deleted,
            )
        return ttl_deleted, cap_deleted
    except Exception:  # noqa: BLE001 - a sweep failure must never crash the app
        logger.exception("retention sweep failed (will retry next interval)")
        return 0, 0


async def _retention_loop() -> None:
    """Run the sweep forever on ``RETENTION_SWEEP_SECONDS`` (AC-37)."""
    interval = max(1, int(RETENTION_SWEEP_SECONDS))
    logger.info(
        "retention sweep started: every %ds (TTL=%dh, cap=%d)",
        interval, TRACE_TTL_HOURS, TRACE_CAP,
    )
    while True:
        try:
            await sweep_once()
        except asyncio.CancelledError:
            raise
        except Exception:  # noqa: BLE001
            logger.exception("retention loop error (continuing)")
        await asyncio.sleep(interval)


def start_retention_task() -> asyncio.Task:
    """Start the retention sweep task (called from the app lifespan). Returns the
    task so it can be cancelled on shutdown."""
    return asyncio.create_task(_retention_loop())
