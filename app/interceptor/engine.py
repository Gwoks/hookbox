"""Core interceptor engine — the <10ms fast path (arch §4.1, AC-9/13/33b/38/49).

``handle_mock`` owns the **frozen resolution order** (§5.5):

    OPTIONS preflight → matching rule → Auto-CRUD → tunnel → MITM → default

and the **conditions order** applied around the served body (§5.5):

    rate-limit (429) → chaos (5xx / dropout) → latency (sleep)

Performance contract (AC-38): the endpoint config + compiled rules come from the
in-process ``rule_cache`` (no per-request DB read); per-endpoint **state is read
from Redis only when some rule gates on state** (lazy); the trace SQLite write and
the Redis publish run in a **fire-and-forget** ``asyncio.create_task`` that is
**never awaited** on the response path (AC-39). ``overhead_ms`` = total wall-clock
minus the applied (simulated) latency, recorded on the trace so <10ms is directly
observable.

Redis-down behavior (AC-49 / §5.11): matching survives (cache, not Redis);
state-gated rules **fail closed**; Auto-CRUD → 503; the rate limiter fails open
(bounded by the in-process body cap); trace publish failures are swallowed.

Sibling modules (``conditions``/``crud``/``proxy``/``cors``/tunnel) are imported
**defensively** so this engine is correct on its own (rule + default branches) and
those features slot in as their issues land.
"""

from __future__ import annotations

import asyncio
import json
import logging
import time
from typing import Dict, List, Optional, Tuple

from fastapi import Request
from fastapi.responses import JSONResponse, Response

import config
from app.interceptor import matcher
from app.interceptor.templating import TemplateContext, render_safe
from app.rule_cache import CompiledEndpoint, rule_cache
from app.redis_state import RedisUnavailable, redis_state

logger = logging.getLogger("hookbox.engine")

_CORS_PREFLIGHT_METHODS = "GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD"


# --- defensive optional-module loaders (each feature ships in its own issue) ---
def _opt(modpath: str, attr: str):
    try:
        mod = __import__(modpath, fromlist=[attr])
        return getattr(mod, attr)
    except Exception:  # noqa: BLE001
        return None


# --- CORS helpers (self-contained §5.6 until app/interceptor/cors.py lands) ----
def _cors_headers(request: Request, cors_enabled: bool) -> Dict[str, str]:
    """The §5.6 per-response CORS set (P1 only). Uses app/interceptor/cors.py if
    present (issue .18); otherwise applies the frozen header set inline so AC-19
    holds independently."""
    cors_mod_fn = _opt("app.interceptor.cors", "response_headers")
    if cors_mod_fn is not None:
        try:
            return cors_mod_fn(request, cors_enabled)
        except Exception:  # noqa: BLE001
            pass
    if not cors_enabled:
        return {}
    origin = request.headers.get("origin", "*")
    return {
        "Access-Control-Allow-Origin": origin,
        "Access-Control-Expose-Headers": "*",
        "Vary": "Origin",
    }


def _cors_preflight(request: Request, cors_enabled: bool) -> Response:
    cors_mod_fn = _opt("app.interceptor.cors", "preflight_response")
    if cors_mod_fn is not None:
        try:
            return cors_mod_fn(request, cors_enabled)
        except Exception:  # noqa: BLE001
            pass
    headers = {}
    if cors_enabled:
        origin = request.headers.get("origin", "*")
        req_headers = request.headers.get("access-control-request-headers", "*")
        headers = {
            "Access-Control-Allow-Origin": origin,
            "Access-Control-Allow-Methods": _CORS_PREFLIGHT_METHODS,
            "Access-Control-Allow-Headers": req_headers,
            "Access-Control-Max-Age": "600",
            "Vary": "Origin",
        }
    return Response(status_code=204, headers=headers)


# --- the pipeline -------------------------------------------------------------
async def handle_mock(request: Request, token: str, mock_path: str) -> Response:
    """Resolve one P1 mock request for ``token`` at ``mock_path`` (§5.5)."""
    t0 = time.monotonic()
    method = request.method.upper()

    # 0. Endpoint config + compiled rules from the in-process cache (no DB here).
    ep = await rule_cache.get(token)
    if ep is None:
        return await _unknown_or_gone(token)

    # 0a. Ingest body cap (AC-S18): reject oversize before buffering the whole body.
    body_bytes, too_big = await _read_body_capped(request)
    if too_big:
        return _identified(
            JSONResponse(status_code=413, content={"error": "payload_too_large",
                         "detail": "Request body exceeds the configured limit."}),
            token, "default", None, ep, request,
        )
    body_text = body_bytes.decode("utf-8", errors="replace") if body_bytes else ""

    headers_lower = {k.lower(): v for k, v in request.headers.items()}
    query = dict(request.query_params)
    trace: List[dict] = []
    applied_latency_ms = 0

    # 1. OPTIONS preflight short-circuit (still traced async).
    if method == "OPTIONS":
        resp = _cors_preflight(request, ep.cors_enabled)
        resp = _identified(resp, token, "cors", None, ep, request)
        _spawn_trace(token, method, mock_path, resp.status_code, "cors", None,
                     t0, applied_latency_ms, headers_lower, query, body_text,
                     resp, trace, {})
        return resp

    # 2. Lazily read state ONLY if some enabled rule gates on it (perf + fail-closed).
    state: Dict[str, str] = {}
    if ep.any_rule_gates_on_state:
        try:
            state = await redis_state.get_state(token)
            trace.append({"step": "state_read", "detail": f"{len(state)} key(s)"})
        except RedisUnavailable:
            # Fail CLOSED: leave state empty so gated rules are skipped (AC-9/49).
            state = {}
            trace.append({"step": "state_read", "detail": "redis down -> state empty (fail-closed)"})

    # 3. Match a rule (first enabled by priority,id — §5.3 / AC-33b).
    match = matcher.select(ep.rules, method, mock_path, headers_lower, query, body_text, state)

    served_by = "default"
    matched_rule_id: Optional[int] = None
    rule_latency_override: Optional[int] = None
    rule_rate_override: Optional[int] = None

    if match is not None:
        rule = match.rule
        matched_rule_id = rule.id
        served_by = "rule"
        rule_latency_override = rule.latency_ms
        rule_rate_override = rule.rate_limit_per_min
        trace.append({"step": "match", "detail": f"rule {rule.id} (priority {rule.priority})"})

        ctx = TemplateContext(
            method=method, path=mock_path, query=query, headers=headers_lower,
            path_params=match.path_params, body=body_text, state=state,
        )
        # 3a. State writes (rendered with the same ctx). Best-effort; never block.
        await _apply_state_writes(token, rule, ctx, trace)

        body_out = render_safe(rule.response.get("body_template", ""), ctx)
        status_code = int(rule.response.get("status_code", 200))
        content_type = rule.response.get("content_type", "application/json")
        rule_headers = dict(rule.response.get("headers") or {})
        rule_headers.setdefault("Content-Type", content_type)
        resp: Response = Response(content=body_out, status_code=status_code, headers=rule_headers)
        trace.append({"step": "template", "detail": f"rendered {len(body_out)} bytes"})
    else:
        # 4. No rule → Auto-CRUD → tunnel → MITM → default (in frozen order).
        resp, served_by = await _resolve_unmatched(
            request, ep, token, method, mock_path, query, headers_lower, body_text, body_bytes, trace
        )

    # 5. Conditions: rate-limit → chaos → latency (§5.5).
    eff_rate = rule_rate_override if rule_rate_override is not None else ep.rate_limit_per_min
    eff_latency = rule_latency_override if rule_latency_override is not None else ep.latency_ms

    rl_resp = await _rate_limit(token, matched_rule_id, eff_rate, request, ep, trace)
    if rl_resp is not None:
        rl_resp = _identified(rl_resp, token, "ratelimit", matched_rule_id, ep, request)
        _spawn_trace(token, method, mock_path, rl_resp.status_code, "ratelimit",
                     matched_rule_id, t0, 0, headers_lower, query, body_text, rl_resp, trace, state)
        return rl_resp

    chaos_resp = await _chaos(ep, request, trace)
    if chaos_resp is not None:
        if chaos_resp == "DROP":
            # Opt-in dropout: close the connection without a response (bounded).
            _spawn_trace(token, method, mock_path, 0, "chaos", matched_rule_id,
                         t0, 0, headers_lower, query, body_text, None, trace, state)
            return await _drop_connection()
        chaos_resp = _identified(chaos_resp, token, "chaos", matched_rule_id, ep, request)
        _spawn_trace(token, method, mock_path, chaos_resp.status_code, "chaos",
                     matched_rule_id, t0, 0, headers_lower, query, body_text, chaos_resp, trace, state)
        return chaos_resp

    if eff_latency and eff_latency > 0:
        applied_latency_ms = await _apply_latency(eff_latency, trace)

    # 6. Identifying + CORS headers on the final response.
    resp = _identified(resp, token, served_by, matched_rule_id, ep, request)

    # 7. Fire-and-forget trace + publish (never awaited — AC-39).
    _spawn_trace(token, method, mock_path, resp.status_code, served_by,
                 matched_rule_id, t0, applied_latency_ms, headers_lower, query,
                 body_text, resp, trace, state)
    return resp


# --- unmatched resolution: CRUD → tunnel → MITM → default ---------------------
async def _resolve_unmatched(
    request, ep: CompiledEndpoint, token: str, method: str, mock_path: str,
    query: Dict[str, str], headers_lower: Dict[str, str], body_text: str,
    body_bytes: bytes, trace: List[dict],
) -> Tuple[Response, str]:
    # 4a. Auto-CRUD.
    if ep.auto_crud:
        crud_handle = _opt("app.interceptor.crud", "handle")
        crud_matches = _opt("app.interceptor.crud", "matches")
        if crud_handle is not None and (crud_matches is None or crud_matches(mock_path)):
            try:
                resp = await crud_handle(token, method, mock_path, body_text)
                trace.append({"step": "crud", "detail": f"{method} {mock_path}"})
                return resp, "crud"
            except RedisUnavailable:
                trace.append({"step": "crud", "detail": "redis down -> 503"})
                return JSONResponse(status_code=503, content={"error": "store_unavailable",
                                    "detail": "Auto-CRUD store unavailable."}), "crud"
            except Exception:  # noqa: BLE001
                logger.exception("crud handler error")
                return JSONResponse(status_code=400, content={"error": "bad_request",
                                    "detail": "Invalid CRUD request."}), "crud"

    # 4b. Tunnel (if a CLI is bound for this slug).
    tunnel_active = _opt("app.routes.tunnel", "tunnel_registry")
    if tunnel_active is not None:
        try:
            if tunnel_active.is_active(token):
                forward = _opt("app.routes.tunnel", "forward_to_tunnel")
                if forward is not None:
                    resp = await forward(token, method, mock_path, query, headers_lower, body_bytes)
                    trace.append({"step": "tunnel", "detail": "forwarded to bound CLI"})
                    return resp, "tunnel"
        except Exception:  # noqa: BLE001
            logger.exception("tunnel forward error")
            return JSONResponse(status_code=504, content={"error": "no_tunnel",
                                "detail": "Tunnel error."}), "tunnel"

    # 4c. MITM forward.
    if ep.target_url:
        forward = _opt("app.interceptor.proxy", "forward")
        if forward is not None:
            try:
                resp = await forward(ep, method, mock_path, query, headers_lower, body_bytes)
                trace.append({"step": "forward", "detail": f"MITM -> {ep.target_url}"})
                return resp, "mitm"
            except Exception:  # noqa: BLE001
                logger.exception("MITM proxy error")
                return JSONResponse(status_code=502, content={"error": "upstream_unreachable",
                                    "detail": "Upstream forward failed."}), "mitm"

    # 4d. Default mode.
    if ep.default_mode == "echo":
        trace.append({"step": "default", "detail": "echo"})
        return _echo_response(request, method, mock_path, query, headers_lower, body_text), "default"
    trace.append({"step": "default", "detail": "mock_404"})
    return JSONResponse(status_code=404, content={"error": "no_match",
                        "detail": "No rule matched this request."}), "default"


def _echo_response(request, method, mock_path, query, headers_lower, body_text) -> Response:
    payload = {
        "method": method, "path": mock_path, "query": query,
        "headers": headers_lower, "body": body_text,
    }
    return JSONResponse(status_code=200, content=payload)


# --- conditions ---------------------------------------------------------------
async def _rate_limit(token, rule_id, limit, request, ep, trace) -> Optional[Response]:
    """Rate-limit check via the optional conditions module or directly via the
    Redis token bucket. Fails OPEN on Redis loss (AC-S20). Returns a 429 Response
    when over the limit, else None."""
    if not limit or limit <= 0:
        return None
    cond_fn = _opt("app.interceptor.conditions", "check_rate_limit")
    try:
        if cond_fn is not None:
            result = await cond_fn(token, limit, rule_id)
        else:
            result = await redis_state.rate_limit_check(token, limit, 60, rule_id)
    except Exception:  # noqa: BLE001
        return None  # fail open
    if result is None or result.allowed:
        return None
    trace.append({"step": "rate_limit", "detail": f"429 (limit {limit}/min)"})
    return JSONResponse(
        status_code=429,
        content={"error": "rate_limited", "detail": "Rate limit exceeded."},
        headers={
            "Retry-After": str(result.retry_after),
            "X-RateLimit-Limit": str(result.limit),
            "X-RateLimit-Remaining": str(result.remaining),
        },
    )


async def _chaos(ep, request, trace):
    """Chaos injection via the optional conditions module. Returns a 5xx Response,
    the sentinel ``"DROP"`` for opt-in connection-drop, or None."""
    if not ep.chaos_pct or ep.chaos_pct <= 0:
        return None
    cond_fn = _opt("app.interceptor.conditions", "roll_chaos")
    if cond_fn is None:
        return None
    try:
        outcome = cond_fn(ep)
    except Exception:  # noqa: BLE001
        return None
    if outcome is None:
        return None
    if outcome == "DROP":
        trace.append({"step": "chaos", "detail": "dropout (connection closed)"})
        return "DROP"
    # outcome is an int status code.
    trace.append({"step": "chaos", "detail": f"{outcome}"})
    return JSONResponse(status_code=int(outcome), content={"error": "chaos",
                        "detail": "Injected chaos failure."})


async def _apply_latency(ms: int, trace: List[dict]) -> int:
    """Apply simulated latency via the conditions module (clamped 0–LATENCY_MAX_MS,
    AC-24/AC-27c) and return the milliseconds actually applied. Falls back to an
    inline clamped sleep if the conditions module is unavailable."""
    apply_fn = _opt("app.interceptor.conditions", "apply_latency")
    if apply_fn is not None:
        applied = await apply_fn(ms)
    else:
        applied = max(0, min(int(ms), config.LATENCY_MAX_MS))
        if applied > 0:
            await asyncio.sleep(applied / 1000.0)
    trace.append({"step": "latency", "detail": f"{applied}ms"})
    return applied


async def _drop_connection() -> Response:
    """Opt-in dropout: signal a connection drop. Starlette will close the response;
    we raise to abort the cycle without a normal body."""
    # Returning an empty 444-ish response closest to "no response"; the ASGI
    # server closes the connection. We use status 0 -> Starlette rejects, so use
    # a bare Response that the server flushes with no content.
    return Response(status_code=499, content=b"", headers={"Connection": "close"})


# --- state writes -------------------------------------------------------------
async def _apply_state_writes(token, rule, ctx: TemplateContext, trace: List[dict]) -> None:
    if not rule.state_writes:
        return
    for w in rule.state_writes:
        key = w.get("key", "")
        val_tmpl = w.get("value", "")
        value = render_safe(val_tmpl, ctx)
        try:
            await redis_state.set_state(token, key, value)
            # Reflect immediately so {{state.k}} in the same response sees it.
            ctx.state[key] = value
            trace.append({"step": "state_write", "detail": f"{key}={value}"})
            await redis_state.publish(f"trace:{token}",
                                      {"type": "state_changed", "data": {"token": token, "key": key, "value": value}})
        except RedisUnavailable:
            trace.append({"step": "state_write", "detail": f"{key}: redis down (skipped)"})
        except Exception:  # noqa: BLE001
            trace.append({"step": "state_write", "detail": f"{key}: invalid key (skipped)"})


# --- helpers ------------------------------------------------------------------
async def _read_body_capped(request: Request) -> Tuple[bytes, bool]:
    """Read the request body, enforcing MAX_INGEST_BODY_BYTES (AC-S18).

    Starlette buffers the body; we check Content-Length first for an early reject,
    then guard the read result length as a backstop.
    """
    cap = config.MAX_INGEST_BODY_BYTES
    cl = request.headers.get("content-length")
    if cl is not None:
        try:
            if int(cl) > cap:
                return b"", True
        except ValueError:
            pass
    body = await request.body()
    if len(body) > cap:
        return b"", True
    return body, False


def _identified(resp: Response, token: str, served_by: str, rule_id: Optional[int],
                ep: CompiledEndpoint, request: Request) -> Response:
    """Attach X-HookBox-* identifying headers (§5.5) + auto-CORS (§5.6, P1)."""
    resp.headers["X-HookBox-Endpoint"] = token
    resp.headers["X-HookBox-Served-By"] = served_by
    if rule_id is not None:
        resp.headers["X-HookBox-Rule-Id"] = str(rule_id)
    for k, v in _cors_headers(request, ep.cors_enabled).items():
        resp.headers[k] = v
    return resp


async def _unknown_or_gone(token: str) -> Response:
    """404 unknown_endpoint vs 410 endpoint_gone (§5.5, OQ-1). Not logged."""
    gone = False
    try:
        gone = await redis_state.is_gone(token)
    except Exception:  # noqa: BLE001
        gone = False
    headers = {"X-HookBox-Endpoint": token}
    if gone:
        return JSONResponse(status_code=410, headers=headers,
                            content={"error": "endpoint_gone",
                                     "detail": "This endpoint was deleted or expired."})
    return JSONResponse(status_code=404, headers=headers,
                        content={"error": "unknown_endpoint", "detail": "No such endpoint."})


def _spawn_trace(token, method, path, status_code, served_by, matched_rule_id,
                 t0, applied_latency_ms, req_headers, query, req_body,
                 resp: Optional[Response], trace: List[dict], state: Dict[str, str]) -> None:
    """Fire-and-forget the trace write + pub/sub publish (AC-39). Never awaited."""
    duration_ms = int((time.monotonic() - t0) * 1000)
    overhead_ms = max(0, duration_ms - int(applied_latency_ms))

    resp_headers: Dict[str, str] = {}
    resp_body: Optional[str] = None
    if resp is not None:
        resp_headers = {k: v for k, v in resp.headers.items()}
        resp_body = _extract_body(resp)

    record = {
        "token": token, "method": method, "path": path, "status_code": status_code,
        "served_by": served_by, "matched_rule_id": matched_rule_id,
        "duration_ms": duration_ms, "overhead_ms": overhead_ms,
        "request_headers": _redact(req_headers), "query_params": query,
        "request_body": req_body[: config.MAX_BODY_BYTES] if req_body else None,
        "response_headers": resp_headers,
        "response_body": resp_body[: config.MAX_BODY_BYTES] if resp_body else None,
        "trace": trace, "state_snapshot": state,
    }
    try:
        asyncio.create_task(_persist_and_publish(record))
    except RuntimeError:
        # No running loop (e.g. unit test without a loop) — best effort, skip.
        logger.debug("no event loop for trace task; skipped")


def _extract_body(resp: Response) -> Optional[str]:
    body = getattr(resp, "body", None)
    if body is None:
        return None
    if isinstance(body, (bytes, bytearray)):
        return bytes(body).decode("utf-8", errors="replace")
    return str(body)


_REDACT_HEADERS = {"authorization", "cookie", "x-owner-id"}


def _redact(headers: Dict[str, str]) -> Dict[str, str]:
    """Never persist the owner capability / cookies into a trace (AC-S25)."""
    return {k: ("<redacted>" if k.lower() in _REDACT_HEADERS else v) for k, v in headers.items()}


async def _persist_and_publish(record: dict) -> None:
    """Background: write the trace (with write-time prune) and publish the summary
    to ``trace:<token>``. Swallows all errors (R4 / AC-49)."""
    from app.database import trace_writer
    try:
        row_id = await trace_writer.insert_trace(record)
    except Exception:  # noqa: BLE001
        logger.exception("trace persist failed (swallowed)")
        row_id = None

    summary = {
        "id": row_id or 0, "token": record["token"], "method": record["method"],
        "path": record["path"], "status_code": record["status_code"],
        "served_by": record["served_by"], "matched_rule_id": record["matched_rule_id"],
        "duration_ms": record["duration_ms"], "overhead_ms": record["overhead_ms"],
        "timestamp": _now_iso(),
    }
    try:
        await redis_state.publish_trace(record["token"], {"type": "new_request", "data": summary})
    except Exception:  # noqa: BLE001
        logger.debug("trace publish failed (swallowed)")


def _now_iso() -> str:
    from datetime import datetime, timezone
    return datetime.now(timezone.utc).isoformat()
