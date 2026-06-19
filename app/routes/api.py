"""Management API (P2) — the 19 §5.2 endpoints with real owner-capability auth.

Replaces the prior ``X-User-Id`` header-trust. All responses are JSON; error
bodies are uniformly ``{"error": "<code>", "detail": "<human>"}``. Endpoint-scoped
routes return 404 (not 403) for a valid-but-non-owner capability (AC-S2/S3).
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from typing import Optional

import aiosqlite
from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from fastapi.responses import JSONResponse

import config
from app.auth import assert_owns_endpoint, require_owner
from app.database import gen_owner_secret, gen_token, get_db, hash_email, hash_secret
from app.models import (
    EndpointConfigPatch,
    EndpointCreate,
    EndpointDetail,
    EndpointSummary,
    Message,
    MockRule,
    MockRuleCreate,
    MockRulePatch,
    RequestDetail,
    RequestSummary,
    SessionCreate,
    SessionResponse,
)
from app.redis_state import RedisUnavailable, redis_state

router = APIRouter(prefix="/api", tags=["API"])


# --- serialization helpers ----------------------------------------------------
def _mock_url(token: str) -> str:
    if config.PATH_FALLBACK_ONLY:
        # No usable wildcard domain — surface the path-fallback URL here too.
        return f"/e/{token}"
    return f"https://{token}.{config.MOCK_DOMAIN}"


def _path_url(token: str) -> str:
    return f"/e/{token}"


def _to_dt(value) -> Optional[datetime]:
    if value is None:
        return None
    if isinstance(value, datetime):
        return value
    try:
        return datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except (ValueError, TypeError):
        return None


def _endpoint_summary(row: aiosqlite.Row) -> EndpointSummary:
    return EndpointSummary(
        token=row["token"],
        name=row["name"],
        mock_url=_mock_url(row["token"]),
        path_url=_path_url(row["token"]),
        created_at=_to_dt(row["created_at"]) or datetime.now(timezone.utc),
        last_hit=_to_dt(row["last_hit"]),
        request_count=row["request_count"],
    )


def _endpoint_detail(row: aiosqlite.Row, tunnel_active: bool = False) -> EndpointDetail:
    return EndpointDetail(
        token=row["token"],
        name=row["name"],
        mock_url=_mock_url(row["token"]),
        path_url=_path_url(row["token"]),
        auto_crud=bool(row["auto_crud"]),
        target_url=row["target_url"],
        default_mode=row["default_mode"],
        latency_ms=row["latency_ms"],
        rate_limit_per_min=row["rate_limit_per_min"],
        chaos_pct=row["chaos_pct"],
        cors_enabled=bool(row["cors_enabled"]),
        tunnel_active=tunnel_active,
        created_at=_to_dt(row["created_at"]) or datetime.now(timezone.utc),
        last_hit=_to_dt(row["last_hit"]),
        request_count=row["request_count"],
    )


def _tunnel_active(token: str) -> bool:
    """Lazily consult the tunnel registry if it exists (avoids hard import cycle)."""
    try:
        from app.routes.tunnel import tunnel_registry  # local import
        return tunnel_registry.is_active(token)
    except Exception:
        return False


def _rule_from_row(row: aiosqlite.Row) -> MockRule:
    return MockRule(
        id=row["id"],
        token=row["token"],
        name=row["name"],
        priority=row["priority"],
        enabled=bool(row["enabled"]),
        match=json.loads(row["match_json"] or "{}"),
        response=json.loads(row["response_json"] or "{}"),
        state_writes=json.loads(row["state_writes_json"] or "[]"),
        latency_ms=row["latency_ms"],
        rate_limit_per_min=row["rate_limit_per_min"],
        webhook_action=json.loads(row["webhook_json"]) if row["webhook_json"] else None,
        created_at=_to_dt(row["created_at"]) or datetime.now(timezone.utc),
    )


async def _new_endpoint(db: aiosqlite.Connection, owner_id: str, name: Optional[str]) -> aiosqlite.Row:
    token = gen_token()
    # Extremely unlikely collision; retry a couple of times.
    for _ in range(5):
        cur = await db.execute("SELECT 1 FROM endpoints WHERE token = ?", (token,))
        if await cur.fetchone() is None:
            break
        token = gen_token()
    await db.execute(
        "INSERT INTO endpoints (token, owner_id, name) VALUES (?, ?, ?)",
        (token, owner_id, name),
    )
    await db.commit()
    cur = await db.execute("SELECT * FROM endpoints WHERE token = ?", (token,))
    return await cur.fetchone()


async def _invalidate(token: str) -> None:
    """Publish cfg invalidation so the in-process rule cache reloads (AC-34).

    Best-effort: also invalidate the local cache directly (single-instance)."""
    try:
        from app.rule_cache import rule_cache
        rule_cache.invalidate(token)
    except Exception:
        pass
    await redis_state.publish_cfg_invalidation(token)


# --- 1. session (no auth) -----------------------------------------------------
@router.post("/session", response_model=SessionResponse)
async def create_session(data: SessionCreate, request: Request,
                         db: aiosqlite.Connection = Depends(get_db)):
    """Email → instant session. Upserts the owner, **rotates** the secret on every
    call (§5.9), auto-provisions a first endpoint if none, returns the primary.

    Constant in shape/status for new vs existing emails (AC-S5)."""
    # Per-source anti-enumeration rate limit (AC-S5); fails open if Redis is down.
    client_ip = request.client.host if request.client else "unknown"
    try:
        rl = await redis_state.rate_limit_check(
            token=f"session:{client_ip}", limit=config.SESSION_RATE_LIMIT_PER_MIN, window=60
        )
        if not rl.allowed:
            return JSONResponse(
                status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                content={"error": "rate_limited", "detail": "Too many session requests."},
                headers={"Retry-After": str(rl.retry_after)},
            )
    except Exception:
        pass

    owner_id = hash_email(data.email)
    new_secret = gen_owner_secret()
    secret_hash = hash_secret(new_secret)
    email = data.email.lower().strip()

    # Upsert owner, rotating the secret (works for both new and existing — same shape).
    await db.execute(
        """
        INSERT INTO owners (owner_id, email, secret_hash, last_seen)
        VALUES (?, ?, ?, datetime('now'))
        ON CONFLICT(owner_id) DO UPDATE SET
            secret_hash = excluded.secret_hash,
            last_seen = datetime('now')
        """,
        (owner_id, email, secret_hash),
    )
    await db.commit()

    cur = await db.execute(
        "SELECT * FROM endpoints WHERE owner_id = ? ORDER BY created_at, token", (owner_id,)
    )
    rows = await cur.fetchall()
    if not rows:
        rows = [await _new_endpoint(db, owner_id, None)]

    summaries = [_endpoint_summary(r) for r in rows]
    return SessionResponse(
        owner_id=owner_id,
        owner_secret=new_secret,
        endpoints=summaries,
        primary=summaries[0],
    )


# --- 2. list endpoints --------------------------------------------------------
@router.get("/endpoints", response_model=list[EndpointSummary])
async def list_endpoints(owner_id: str = Depends(require_owner),
                         db: aiosqlite.Connection = Depends(get_db)):
    cur = await db.execute(
        "SELECT * FROM endpoints WHERE owner_id = ? ORDER BY created_at, token", (owner_id,)
    )
    rows = await cur.fetchall()
    return [_endpoint_summary(r) for r in rows]


# --- 3. create endpoint -------------------------------------------------------
@router.post("/endpoints", response_model=EndpointDetail, status_code=status.HTTP_201_CREATED)
async def create_endpoint(data: EndpointCreate, owner_id: str = Depends(require_owner),
                          db: aiosqlite.Connection = Depends(get_db)):
    row = await _new_endpoint(db, owner_id, data.name)
    return _endpoint_detail(row, _tunnel_active(row["token"]))


# --- 4. get endpoint ----------------------------------------------------------
@router.get("/endpoints/{token}", response_model=EndpointDetail)
async def get_endpoint(token: str, owner_id: str = Depends(require_owner),
                       db: aiosqlite.Connection = Depends(get_db)):
    row = await assert_owns_endpoint(token, owner_id, db)
    return _endpoint_detail(row, _tunnel_active(token))


# --- 5. patch endpoint config -------------------------------------------------
_PATCH_COLUMNS = {
    "name", "auto_crud", "target_url", "default_mode",
    "latency_ms", "rate_limit_per_min", "chaos_pct", "cors_enabled",
}


@router.patch("/endpoints/{token}", response_model=EndpointDetail)
async def patch_endpoint(token: str, data: EndpointConfigPatch,
                         owner_id: str = Depends(require_owner),
                         db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    fields = data.model_dump(exclude_unset=True)
    if fields:
        # Build a parameterized SET from a fixed column allow-list (AC-S23).
        sets = []
        values = []
        for col in _PATCH_COLUMNS:
            if col in fields:
                val = fields[col]
                if isinstance(val, bool):
                    val = int(val)
                sets.append(f"{col} = ?")
                values.append(val)
        if sets:
            values.append(token)
            await db.execute(
                "UPDATE endpoints SET " + ", ".join(sets) + " WHERE token = ?", values
            )
            await db.commit()
            await _invalidate(token)
    cur = await db.execute("SELECT * FROM endpoints WHERE token = ?", (token,))
    return _endpoint_detail(await cur.fetchone(), _tunnel_active(token))


# --- 6. delete endpoint -------------------------------------------------------
@router.delete("/endpoints/{token}", response_model=Message)
async def delete_endpoint(token: str, owner_id: str = Depends(require_owner),
                          db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    await db.execute("DELETE FROM endpoints WHERE token = ?", (token,))
    await db.commit()
    await _invalidate(token)
    try:
        await redis_state.clear_state(token)
    except RedisUnavailable:
        pass
    # Tombstone so the mock surface returns 410 endpoint_gone (AC-7a / OQ-1).
    await redis_state.mark_gone(token)
    return Message(message="Endpoint deleted.")


# --- 7. list rules ------------------------------------------------------------
@router.get("/endpoints/{token}/rules", response_model=list[MockRule])
async def list_rules(token: str, owner_id: str = Depends(require_owner),
                     db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        "SELECT * FROM mock_rules WHERE token = ? ORDER BY priority, id", (token,)
    )
    return [_rule_from_row(r) for r in await cur.fetchall()]


# --- 8. create rule -----------------------------------------------------------
@router.post("/endpoints/{token}/rules", response_model=MockRule, status_code=status.HTTP_201_CREATED)
async def create_rule(token: str, data: MockRuleCreate,
                      owner_id: str = Depends(require_owner),
                      db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        """
        INSERT INTO mock_rules
            (token, name, priority, enabled, match_json, response_json,
             state_writes_json, latency_ms, rate_limit_per_min, webhook_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            token, data.name, data.priority, int(data.enabled),
            data.match.model_dump_json(), data.response.model_dump_json(),
            json.dumps([w.model_dump() for w in data.state_writes]),
            data.latency_ms, data.rate_limit_per_min,
            data.webhook_action.model_dump_json() if data.webhook_action else None,
        ),
    )
    await db.commit()
    rule_id = cur.lastrowid
    await _invalidate(token)
    cur = await db.execute("SELECT * FROM mock_rules WHERE id = ?", (rule_id,))
    return _rule_from_row(await cur.fetchone())


# --- 9. get rule --------------------------------------------------------------
@router.get("/endpoints/{token}/rules/{rule_id}", response_model=MockRule)
async def get_rule(token: str, rule_id: int, owner_id: str = Depends(require_owner),
                   db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        "SELECT * FROM mock_rules WHERE id = ? AND token = ?", (rule_id, token)
    )
    row = await cur.fetchone()
    if row is None:
        raise HTTPException(status_code=404, detail={"error": "not_found", "detail": "Rule not found."})
    return _rule_from_row(row)


# --- 10. patch rule -----------------------------------------------------------
@router.patch("/endpoints/{token}/rules/{rule_id}", response_model=MockRule)
async def patch_rule(token: str, rule_id: int, data: MockRulePatch,
                     owner_id: str = Depends(require_owner),
                     db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        "SELECT * FROM mock_rules WHERE id = ? AND token = ?", (rule_id, token)
    )
    row = await cur.fetchone()
    if row is None:
        raise HTTPException(status_code=404, detail={"error": "not_found", "detail": "Rule not found."})

    fields = data.model_dump(exclude_unset=True)
    sets, values = [], []
    if "name" in fields:
        sets.append("name = ?"); values.append(fields["name"])
    if "priority" in fields:
        sets.append("priority = ?"); values.append(fields["priority"])
    if "enabled" in fields:
        sets.append("enabled = ?"); values.append(int(fields["enabled"]))
    if "match" in fields and data.match is not None:
        sets.append("match_json = ?"); values.append(data.match.model_dump_json())
    if "response" in fields and data.response is not None:
        sets.append("response_json = ?"); values.append(data.response.model_dump_json())
    if "state_writes" in fields and data.state_writes is not None:
        sets.append("state_writes_json = ?")
        values.append(json.dumps([w.model_dump() for w in data.state_writes]))
    if "latency_ms" in fields:
        sets.append("latency_ms = ?"); values.append(fields["latency_ms"])
    if "rate_limit_per_min" in fields:
        sets.append("rate_limit_per_min = ?"); values.append(fields["rate_limit_per_min"])
    if "webhook_action" in fields:
        sets.append("webhook_json = ?")
        values.append(data.webhook_action.model_dump_json() if data.webhook_action else None)

    if sets:
        values.extend([rule_id, token])
        await db.execute(
            "UPDATE mock_rules SET " + ", ".join(sets) + " WHERE id = ? AND token = ?",
            values,
        )
        await db.commit()
        await _invalidate(token)
    cur = await db.execute("SELECT * FROM mock_rules WHERE id = ?", (rule_id,))
    return _rule_from_row(await cur.fetchone())


# --- 11. delete rule ----------------------------------------------------------
@router.delete("/endpoints/{token}/rules/{rule_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_rule(token: str, rule_id: int, owner_id: str = Depends(require_owner),
                      db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        "SELECT 1 FROM mock_rules WHERE id = ? AND token = ?", (rule_id, token)
    )
    if await cur.fetchone() is None:
        raise HTTPException(status_code=404, detail={"error": "not_found", "detail": "Rule not found."})
    await db.execute("DELETE FROM mock_rules WHERE id = ? AND token = ?", (rule_id, token))
    await db.commit()
    await _invalidate(token)
    return JSONResponse(status_code=status.HTTP_204_NO_CONTENT, content=None)


# --- 12. list requests --------------------------------------------------------
@router.get("/endpoints/{token}/requests", response_model=list[RequestSummary])
async def list_requests(token: str, owner_id: str = Depends(require_owner),
                        limit: int = Query(50, ge=1, le=200), offset: int = Query(0, ge=0),
                        db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    cur = await db.execute(
        "SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?",
        (token, limit, offset),
    )
    rows = await cur.fetchall()
    return [_request_summary(r) for r in rows]


def _request_summary(row: aiosqlite.Row) -> RequestSummary:
    return RequestSummary(
        id=row["id"], token=row["token"], method=row["method"], path=row["path"],
        status_code=row["status_code"], served_by=row["served_by"],
        matched_rule_id=row["matched_rule_id"], duration_ms=row["duration_ms"],
        overhead_ms=row["overhead_ms"],
        timestamp=_to_dt(row["created_at"]) or datetime.now(timezone.utc),
    )


# --- 13. get request detail ---------------------------------------------------
@router.get("/requests/{request_id}", response_model=RequestDetail)
async def get_request(request_id: int, owner_id: str = Depends(require_owner),
                      db: aiosqlite.Connection = Depends(get_db)):
    cur = await db.execute("SELECT * FROM request_logs WHERE id = ?", (request_id,))
    row = await cur.fetchone()
    if row is None:
        raise HTTPException(status_code=404, detail={"error": "not_found", "detail": "Request not found."})
    # Ownership via the trace's endpoint (404 for non-owner — AC-S2).
    await assert_owns_endpoint(row["token"], owner_id, db)
    return RequestDetail(
        id=row["id"], token=row["token"], method=row["method"], path=row["path"],
        status_code=row["status_code"], served_by=row["served_by"],
        matched_rule_id=row["matched_rule_id"], duration_ms=row["duration_ms"],
        overhead_ms=row["overhead_ms"],
        timestamp=_to_dt(row["created_at"]) or datetime.now(timezone.utc),
        request_headers=json.loads(row["request_headers"] or "{}"),
        query_params=json.loads(row["query_params"] or "{}"),
        request_body=row["request_body"],
        response_headers=json.loads(row["response_headers"] or "{}"),
        response_body=row["response_body"],
        trace=json.loads(row["trace_json"] or "[]"),
        state_snapshot=json.loads(row["state_snapshot"] or "{}"),
    )


# --- 14. clear request history ------------------------------------------------
@router.delete("/endpoints/{token}/requests", response_model=Message)
async def clear_requests(token: str, owner_id: str = Depends(require_owner),
                         db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    await db.execute("DELETE FROM request_logs WHERE token = ?", (token,))
    await db.commit()
    return Message(message="Trace history cleared.")


# --- 15. get state ------------------------------------------------------------
@router.get("/endpoints/{token}/state")
async def get_state(token: str, owner_id: str = Depends(require_owner),
                    db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    try:
        state = await redis_state.get_state(token)
    except RedisUnavailable:
        return JSONResponse(status_code=503, content={"error": "state_unavailable", "detail": "State store unavailable."})
    return {"state": state}


# --- 16. clear state ----------------------------------------------------------
@router.delete("/endpoints/{token}/state", response_model=Message)
async def clear_state(token: str, owner_id: str = Depends(require_owner),
                      db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    try:
        await redis_state.clear_state(token)
    except RedisUnavailable:
        return JSONResponse(status_code=503, content={"error": "state_unavailable", "detail": "State store unavailable."})
    return Message(message="State cleared.")


# --- 17. peek collection ------------------------------------------------------
@router.get("/endpoints/{token}/collections/{name}")
async def peek_collection(token: str, name: str, owner_id: str = Depends(require_owner),
                          db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    from app.utils.helpers import is_safe_key
    if not is_safe_key(name):
        raise HTTPException(status_code=422, detail={"error": "invalid_collection", "detail": "Invalid collection name."})
    try:
        items = await redis_state.crud_list(token, name)
    except RedisUnavailable:
        return JSONResponse(status_code=503, content={"error": "store_unavailable", "detail": "Collection store unavailable."})
    return {"items": items}


# --- 18. clear collection -----------------------------------------------------
@router.delete("/endpoints/{token}/collections/{name}", response_model=Message)
async def clear_collection(token: str, name: str, owner_id: str = Depends(require_owner),
                           db: aiosqlite.Connection = Depends(get_db)):
    await assert_owns_endpoint(token, owner_id, db)
    from app.utils.helpers import is_safe_key
    if not is_safe_key(name):
        raise HTTPException(status_code=422, detail={"error": "invalid_collection", "detail": "Invalid collection name."})
    try:
        await redis_state.crud_clear(token, name)
    except RedisUnavailable:
        return JSONResponse(status_code=503, content={"error": "store_unavailable", "detail": "Collection store unavailable."})
    return Message(message="Collection cleared.")
