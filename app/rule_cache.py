"""In-process rule cache (arch §4.1, AC-34, AC-38, AC-49).

A ``dict[token] -> CompiledEndpoint`` that powers the <10ms fast path: the
interceptor reads the endpoint config + the **pre-sorted, pre-compiled** rule list
from memory — it opens **no per-request DB connection** on the matched path.

  * ``get(token)``        — return the cached :class:`CompiledEndpoint`, loading
                            from SQLite on a cold miss (the only DB read, amortized).
  * ``invalidate(token)`` — drop the cache entry; the next ``get`` reloads. Driven
                            by the ``cfg:<token>`` pub/sub signal on any management
                            write (``app/pubsub.py``) and called directly in-process
                            for the single-instance case (AC-34).

Missing tokens are cached negatively for a short TTL so a flood of unknown-token
hits cannot hammer SQLite, while a freshly-created endpoint still appears promptly
(the negative entry is invalidated by ``cfg`` on create, and expires anyway).

Redis is **not** on the match read path (AC-49): a Redis outage never affects
matching — only state-gated rules degrade (fail-closed, evaluated in the matcher).
"""

from __future__ import annotations

import json
import logging
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import aiosqlite

from config import DATABASE_PATH
from app.interceptor.matcher import CompiledRule, compile_rule

logger = logging.getLogger("hookbox.rule_cache")

# How long a negative (token-not-found) entry stays cached.
_NEG_TTL_SECONDS = 5.0


@dataclass
class CompiledEndpoint:
    """The hot, in-memory view of an endpoint: config + compiled, ordered rules."""

    token: str
    owner_id: str
    auto_crud: bool
    target_url: Optional[str]
    default_mode: str
    latency_ms: int
    rate_limit_per_min: int
    chaos_pct: int
    cors_enabled: bool
    rules: List[CompiledRule] = field(default_factory=list)

    @property
    def any_rule_gates_on_state(self) -> bool:
        return any(r.enabled and r.gates_on_state for r in self.rules)


class RuleCache:
    def __init__(self, db_path=DATABASE_PATH) -> None:
        self._db_path = db_path
        self._cache: Dict[str, CompiledEndpoint] = {}
        self._negative: Dict[str, float] = {}  # token -> expiry monotonic ts

    def invalidate(self, token: str) -> None:
        """Drop any cached (positive or negative) entry for ``token`` (AC-34)."""
        self._cache.pop(token, None)
        self._negative.pop(token, None)

    def invalidate_all(self) -> None:
        self._cache.clear()
        self._negative.clear()

    def peek(self, token: str) -> Optional[CompiledEndpoint]:
        """Non-loading lookup (returns the cached entry or None)."""
        return self._cache.get(token)

    async def get(self, token: str) -> Optional[CompiledEndpoint]:
        """Return the compiled endpoint for ``token`` (cold-loads from SQLite).

        Returns ``None`` if the token does not exist (cached negatively for a few
        seconds). Never raises on a normal miss.
        """
        hit = self._cache.get(token)
        if hit is not None:
            return hit

        neg_exp = self._negative.get(token)
        if neg_exp is not None:
            if neg_exp > time.monotonic():
                return None
            self._negative.pop(token, None)

        loaded = await self._load(token)
        if loaded is None:
            self._negative[token] = time.monotonic() + _NEG_TTL_SECONDS
            return None
        self._cache[token] = loaded
        return loaded

    async def _load(self, token: str) -> Optional[CompiledEndpoint]:
        try:
            async with aiosqlite.connect(self._db_path) as db:
                db.row_factory = aiosqlite.Row
                cur = await db.execute(
                    "SELECT * FROM endpoints WHERE token = ?", (token,)
                )
                ep = await cur.fetchone()
                if ep is None:
                    return None
                cur = await db.execute(
                    "SELECT * FROM mock_rules WHERE token = ? ORDER BY priority, id",
                    (token,),
                )
                rule_rows = await cur.fetchall()
        except Exception:  # noqa: BLE001 - a DB hiccup must not crash the fast path
            logger.exception("rule_cache load failed for %s", token)
            return None

        rules: List[CompiledRule] = []
        for r in rule_rows:
            try:
                rules.append(
                    compile_rule(
                        {
                            "id": r["id"],
                            "priority": r["priority"],
                            "enabled": bool(r["enabled"]),
                            "match": json.loads(r["match_json"] or "{}"),
                            "response": json.loads(r["response_json"] or "{}"),
                            "state_writes": json.loads(r["state_writes_json"] or "[]"),
                            "latency_ms": r["latency_ms"],
                            "rate_limit_per_min": r["rate_limit_per_min"],
                            "webhook_action": json.loads(r["webhook_json"]) if r["webhook_json"] else None,
                        }
                    )
                )
            except Exception:  # noqa: BLE001 - skip a single malformed rule, keep the rest
                logger.exception("skipping malformed rule id=%s on %s", r["id"], token)

        return CompiledEndpoint(
            token=ep["token"],
            owner_id=ep["owner_id"],
            auto_crud=bool(ep["auto_crud"]),
            target_url=ep["target_url"],
            default_mode=ep["default_mode"],
            latency_ms=ep["latency_ms"],
            rate_limit_per_min=ep["rate_limit_per_min"],
            chaos_pct=ep["chaos_pct"],
            cors_enabled=bool(ep["cors_enabled"]),
            rules=rules,
        )


# Module-level singleton (referenced by app/routes/api.py invalidation + pubsub).
rule_cache = RuleCache()
