"""Async Redis facade for HookBox (§5.8 keyspace, §5.11 degradation).

A single shared connection pool backs:
  * per-endpoint state KV  ``state:<token>``           (hash, AC-8/9/22)
  * Auto-CRUD collections  ``crud:<token>:<collection>`` (list, §4.3)
  * rate-limit token bucket ``rl:<token>`` / ``rl:<token>:<rule_id>`` (Lua, AC-25)
  * pub/sub fan-out          ``trace:<token>`` / ``cfg:<token>``

Every key is namespaced by the endpoint token and all user-supplied key parts are
validated against the safe charset (AC-10a / SEC-AC-24) before they reach Redis,
so a crafted key can never escape its endpoint namespace.

Degradation contract (§5.11) is implemented at the call sites that use this
facade; this module exposes a clean ``RedisUnavailable`` signal and a
``healthy()`` probe. The rate limiter here returns an explicit *fail-open*
sentinel so the interceptor can allow-but-bound on Redis loss (AC-S20).
"""

from __future__ import annotations

import asyncio
import json
import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

import redis.asyncio as aioredis
from redis.exceptions import RedisError, WatchError

from config import (
    REDIS_URL,
    STATE_TTL_SECONDS,
    CRUD_TTL_SECONDS,
    CRUD_MAX_ITEMS,
)
from app.utils.helpers import is_safe_key

logger = logging.getLogger("hookbox.redis")


class RedisUnavailable(Exception):
    """Raised when a Redis operation cannot complete (drives §5.11 degradation)."""


class InvalidKey(ValueError):
    """A user-supplied key part failed the safe-charset check (SEC-AC-24)."""


# Atomic sliding-window-ish token bucket (AC-25). Returns {allowed, remaining,
# retry_after}. We model a fixed window of ``window`` seconds carrying ``limit``
# tokens; the first request in a window sets the key with TTL = window.
_TOKEN_BUCKET_LUA = """
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
if limit <= 0 then
  return {1, -1, 0}
end
local current = tonumber(redis.call('GET', key) or '0')
if current >= limit then
  local ttl = redis.call('TTL', key)
  if ttl < 0 then ttl = window end
  return {0, 0, ttl}
end
current = redis.call('INCR', key)
if current == 1 then
  redis.call('EXPIRE', key, window)
end
local ttl = redis.call('TTL', key)
if ttl < 0 then ttl = window end
return {1, limit - current, ttl}
"""


@dataclass
class RateLimitResult:
    allowed: bool
    limit: int
    remaining: int
    retry_after: int
    degraded: bool = False  # True when Redis was down and we failed open (AC-S20)


class RedisState:
    def __init__(self, url: str = REDIS_URL) -> None:
        self._url = url
        self._redis: Optional[aioredis.Redis] = None
        self._bucket_sha: Optional[str] = None

    # --- lifecycle ------------------------------------------------------------
    async def connect(self) -> None:
        if self._redis is not None:
            return
        self._redis = aioredis.from_url(
            self._url,
            encoding="utf-8",
            decode_responses=True,
            socket_connect_timeout=2,
            socket_timeout=2,
            health_check_interval=30,
        )
        # Best-effort: pre-load the Lua script. If Redis is down at startup we
        # simply load it lazily on first use.
        try:
            self._bucket_sha = await self._redis.script_load(_TOKEN_BUCKET_LUA)
        except RedisError:
            logger.warning("Redis not reachable at startup; will retry lazily")

    async def close(self) -> None:
        if self._redis is not None:
            try:
                await self._redis.aclose()
            finally:
                self._redis = None

    @property
    def client(self) -> aioredis.Redis:
        if self._redis is None:
            raise RedisUnavailable("redis pool not initialized")
        return self._redis

    async def healthy(self) -> bool:
        if self._redis is None:
            return False
        try:
            return bool(await self._redis.ping())
        except RedisError:
            return False

    def raw(self) -> Optional[aioredis.Redis]:
        """Expose the raw client for the pub/sub relay (it owns its own pubsub)."""
        return self._redis

    def pubsub_connection(self) -> aioredis.Redis:
        """Build a **dedicated** client for the pub/sub relay's long-lived
        subscription.

        Must NOT carry ``socket_timeout``: an idle ``pubsub().listen()`` blocks on
        a socket read between messages, and the shared fast-path client's
        ``socket_timeout=2`` (kept tight to protect the <10ms budget on state /
        rate-limit ops) would raise ``TimeoutError`` every 2s on an idle feed —
        killing the relay and dropping ``new_request`` events (defect: live feed
        dead). The relay's connection therefore sets ``socket_timeout=None`` so an
        idle subscription simply waits. ``socket_connect_timeout`` is still bounded
        so a dead Redis fails fast at connect (the relay then backs off and
        reconnects, R4 / §5.11).
        """
        return aioredis.from_url(
            self._url,
            encoding="utf-8",
            decode_responses=True,
            socket_connect_timeout=2,
            socket_timeout=None,
            health_check_interval=30,
        )

    # --- key builders (namespaced + validated) --------------------------------
    @staticmethod
    def _state_key(token: str) -> str:
        return f"state:{token}"

    @staticmethod
    def _crud_key(token: str, collection: str) -> str:
        if not is_safe_key(collection):
            raise InvalidKey(collection)
        return f"crud:{token}:{collection}"

    @staticmethod
    def _rl_key(token: str, rule_id: Optional[int] = None) -> str:
        return f"rl:{token}:{rule_id}" if rule_id is not None else f"rl:{token}"

    # --- state KV (AC-8/9/10/10a) ---------------------------------------------
    async def get_state(self, token: str) -> Dict[str, str]:
        """Read the full per-endpoint state hash. Fail-CLOSED: on Redis loss the
        caller treats state as empty (§5.11) rather than matching gated rules."""
        try:
            data = await self.client.hgetall(self._state_key(token))
            return data or {}
        except (RedisError, RedisUnavailable):
            raise RedisUnavailable("state read failed")

    async def set_state(self, token: str, key: str, value: str) -> None:
        if not is_safe_key(key):
            raise InvalidKey(key)
        try:
            k = self._state_key(token)
            await self.client.hset(k, key, value)
            await self.client.expire(k, STATE_TTL_SECONDS)
        except (RedisError, RedisUnavailable):
            raise RedisUnavailable("state write failed")

    async def clear_state(self, token: str) -> None:
        try:
            await self.client.delete(self._state_key(token))
        except (RedisError, RedisUnavailable):
            raise RedisUnavailable("state clear failed")

    # --- Auto-CRUD list ops (§4.3) — caller maps RedisUnavailable -> 503 -------
    async def crud_list(self, token: str, collection: str) -> List[dict]:
        try:
            raw = await self.client.lrange(self._crud_key(token, collection), 0, -1)
            return [json.loads(x) for x in raw]
        except (RedisError, RedisUnavailable):
            raise RedisUnavailable("crud list failed")

    async def crud_append(self, token: str, collection: str, item: dict) -> None:
        try:
            k = self._crud_key(token, collection)
            if await self.client.llen(k) >= CRUD_MAX_ITEMS:
                raise ValueError("collection_full")
            await self.client.rpush(k, json.dumps(item))
            await self.client.expire(k, CRUD_TTL_SECONDS)
        except (RedisError,) as exc:
            raise RedisUnavailable("crud append failed") from exc

    async def crud_replace_all(self, token: str, collection: str, items: List[dict]) -> None:
        """Rewrite the whole list atomically (used by update/delete)."""
        try:
            k = self._crud_key(token, collection)
            async with self.client.pipeline(transaction=True) as pipe:
                pipe.delete(k)
                if items:
                    pipe.rpush(k, *[json.dumps(i) for i in items])
                    pipe.expire(k, CRUD_TTL_SECONDS)
                await pipe.execute()
        except (RedisError,) as exc:
            raise RedisUnavailable("crud replace failed") from exc

    async def crud_cas(self, token: str, collection: str, mutate, retries: int = 20):
        """Atomic read-modify-write of a CRUD collection via a WATCH/MULTI optimistic
        transaction — closes the lost-update window of ``crud_list()`` + a later
        ``crud_replace_all()`` under concurrent writers, and is correct across workers
        / replicas (hookbox-65m).

        ``mutate(items) -> (result, new_items)``: ``items`` is the decoded list. Return
        ``new_items=None`` to make NO change (e.g. item not found) and just return
        ``result``; otherwise the list is atomically replaced with ``new_items``. The
        read→mutate→write is retried on a concurrent modification (``WatchError``).
        Caller maps :class:`RedisUnavailable` -> 503 (§5.11).
        """
        k = self._crud_key(token, collection)
        try:
            for _ in range(retries):
                async with self.client.pipeline() as pipe:
                    try:
                        await pipe.watch(k)
                        raw = await pipe.lrange(k, 0, -1)        # immediate (watch mode)
                        items = [json.loads(x) for x in raw]
                        result, new_items = mutate(items)
                        if new_items is None:
                            await pipe.unwatch()
                            return result
                        pipe.multi()
                        pipe.delete(k)
                        if new_items:
                            pipe.rpush(k, *[json.dumps(i) for i in new_items])
                            pipe.expire(k, CRUD_TTL_SECONDS)
                        await pipe.execute()
                        return result
                    except WatchError:
                        await asyncio.sleep(0)  # yield so a pending EXEC lands, then retry
                        continue
            raise RedisUnavailable("crud cas: contention retry limit exceeded")
        except (RedisError,) as exc:
            raise RedisUnavailable("crud cas failed") from exc

    async def crud_clear(self, token: str, collection: str) -> None:
        try:
            await self.client.delete(self._crud_key(token, collection))
        except (RedisError,) as exc:
            raise RedisUnavailable("crud clear failed") from exc

    # --- rate-limit token bucket (AC-25, AC-S19, AC-S20) ----------------------
    async def rate_limit_check(
        self, token: str, limit: int, window: int = 60, rule_id: Optional[int] = None
    ) -> RateLimitResult:
        """Atomic token-bucket check. ``limit<=0`` => unlimited.

        Fails OPEN on Redis loss (AC-S20): returns ``allowed=True, degraded=True``
        — the in-process body/size caps still bound the request elsewhere.
        """
        if limit <= 0:
            return RateLimitResult(True, 0, -1, 0)
        key = self._rl_key(token, rule_id)
        try:
            if self._bucket_sha is None:
                self._bucket_sha = await self.client.script_load(_TOKEN_BUCKET_LUA)
            try:
                res = await self.client.evalsha(
                    self._bucket_sha, 1, key, str(limit), str(window)
                )
            except RedisError:
                # Script may have been flushed; reload once.
                self._bucket_sha = await self.client.script_load(_TOKEN_BUCKET_LUA)
                res = await self.client.evalsha(
                    self._bucket_sha, 1, key, str(limit), str(window)
                )
            allowed, remaining, retry_after = int(res[0]), int(res[1]), int(res[2])
            return RateLimitResult(
                allowed=bool(allowed),
                limit=limit,
                remaining=max(remaining, 0),
                retry_after=max(retry_after, 0),
            )
        except (RedisError, RedisUnavailable):
            logger.warning("rate limiter Redis loss -> failing OPEN (bounded)")
            return RateLimitResult(True, limit, limit, 0, degraded=True)

    # --- pub/sub (§5.4 fan-out, §4.5) -----------------------------------------
    async def publish(self, channel: str, payload: Any) -> None:
        """Publish a JSON payload. Swallowed-and-logged (R4): a dead Redis degrades
        the live feed, never the mock path."""
        try:
            data = payload if isinstance(payload, str) else json.dumps(payload, default=str)
            await self.client.publish(channel, data)
        except (RedisError, RedisUnavailable):
            logger.debug("publish to %s failed (swallowed)", channel)

    async def publish_trace(self, token: str, event: dict) -> None:
        await self.publish(f"trace:{token}", event)

    async def publish_cfg_invalidation(self, token: str) -> None:
        await self.publish(f"cfg:{token}", {"token": token})

    # --- endpoint tombstone (AC-7a / OQ-1: 410 vs 404) ------------------------
    # A best-effort marker that an endpoint token *did exist* but was deleted or
    # pruned, so the mock surface can return 410 endpoint_gone (rather than the
    # 404 unknown_endpoint of a never-existed token). Best-effort: if Redis is
    # down we fall back to 404, which is the safe default.
    @staticmethod
    def _gone_key(token: str) -> str:
        return f"gone:{token}"

    async def mark_gone(self, token: str, ttl_seconds: int = 7 * 24 * 3600) -> None:
        try:
            k = self._gone_key(token)
            await self.client.set(k, "1", ex=ttl_seconds)
        except (RedisError, RedisUnavailable):
            logger.debug("mark_gone(%s) failed (swallowed)", token)

    async def is_gone(self, token: str) -> bool:
        try:
            return bool(await self.client.exists(self._gone_key(token)))
        except (RedisError, RedisUnavailable):
            return False


# Module-level singleton, initialized in the app lifespan.
redis_state = RedisState()
