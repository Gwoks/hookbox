"""Redis pub/sub relay (arch §4.5, AC-34 cfg-invalidation half).

One background task per process (started at lifespan) subscribes to two channel
patterns and relays them locally:

  * ``cfg:<token>``   → invalidate the in-process ``rule_cache`` so a management
                        write is picked up on the next mock request (AC-34). This
                        is the half owned by the engine issue.
  * ``trace:<token>`` → fan out the event to locally-connected WS/SSE feed clients
                        (owner-gated feed, §5.4). The broadcast hook is provided by
                        ``app/websocket.py`` (issue hookbox-wrd.20); until it is
                        wired this relay simply ignores ``trace:*`` (the trace is
                        still persisted to SQLite — the live feed just isn't fanned).

The relay tolerates a Redis outage: it logs, backs off, and reconnects, never
crashing the app (R4 / §5.11). It does not touch the mock fast path.
"""

from __future__ import annotations

import asyncio
import json
import logging
from typing import Optional

from app.rule_cache import rule_cache
from app.redis_state import redis_state

logger = logging.getLogger("hookbox.pubsub")

_RECONNECT_BACKOFF_S = 2.0


def _token_from_channel(channel: str, prefix: str) -> Optional[str]:
    if channel.startswith(prefix):
        return channel[len(prefix):]
    return None


def _broadcast_hook():
    """Resolve the WS/SSE broadcast function lazily (issue .20). Returns a callable
    ``async (token, payload) -> None`` or None if the feed isn't wired yet."""
    try:
        from app.websocket import manager
        return getattr(manager, "broadcast", None) or getattr(manager, "broadcast_to_endpoint", None)
    except Exception:  # noqa: BLE001
        return None


async def _relay_loop() -> None:
    """Subscribe to ``cfg:*`` + ``trace:*`` and relay forever (with reconnect)."""
    while True:
        # Use a DEDICATED connection without ``socket_timeout`` — the shared
        # fast-path client's 2s read timeout would otherwise raise on an idle
        # subscription and kill the relay (dropping live-feed events). See
        # ``RedisState.pubsub_connection``.
        if redis_state.raw() is None:
            await asyncio.sleep(_RECONNECT_BACKOFF_S)
            continue
        conn = None
        psub = None
        try:
            conn = redis_state.pubsub_connection()
            psub = conn.pubsub()
            await psub.psubscribe("cfg:*", "trace:*")
            logger.info("pub/sub relay subscribed to cfg:* and trace:*")
            async for msg in psub.listen():
                if msg is None or msg.get("type") not in ("pmessage", "message"):
                    continue
                channel = msg.get("channel", "")
                if isinstance(channel, bytes):
                    channel = channel.decode("utf-8", "replace")
                data = msg.get("data")
                if isinstance(data, bytes):
                    data = data.decode("utf-8", "replace")

                # cfg:<token> → invalidate the rule cache (AC-34).
                token = _token_from_channel(channel, "cfg:")
                if token is not None:
                    rule_cache.invalidate(token)
                    logger.debug("rule_cache invalidated for %s (cfg)", token)
                    continue

                # trace:<token> → fan out to local feed clients (issue .20).
                token = _token_from_channel(channel, "trace:")
                if token is not None:
                    hook = _broadcast_hook()
                    if hook is not None:
                        try:
                            payload = json.loads(data) if isinstance(data, str) else data
                        except (ValueError, TypeError):
                            payload = data
                        try:
                            await hook(token, payload)
                        except Exception:  # noqa: BLE001
                            logger.debug("feed broadcast failed (swallowed)")
                    continue
        except asyncio.CancelledError:
            raise
        except Exception:  # noqa: BLE001
            logger.warning("pub/sub relay error; reconnecting in %ss", _RECONNECT_BACKOFF_S)
            await asyncio.sleep(_RECONNECT_BACKOFF_S)
        finally:
            if psub is not None:
                try:
                    await psub.aclose()
                except Exception:  # noqa: BLE001
                    pass
            if conn is not None:
                try:
                    await conn.aclose()
                except Exception:  # noqa: BLE001
                    pass


async def start_relay() -> asyncio.Task:
    """Start the relay task (called from the app lifespan). Returns the task so it
    can be cancelled on shutdown."""
    return asyncio.create_task(_relay_loop())
