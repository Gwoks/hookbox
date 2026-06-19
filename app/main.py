"""HookBox — Beeceptor-class API mocking & interception platform.

App factory + lifespan + plane dispatch + the P1 mock catch-all (mounted LAST).

Three hard-isolated planes (arch §3): P1 wildcard mock surface, P2 management API
(``/api/*``), P3 dashboard UI + static + WS/SSE feed. ``PlaneDispatchMiddleware``
tags each request; explicit routers are matched before the catch-all; the catch-all
re-checks ``request.state.plane`` so it can never shadow ``/api`` or the UI (AC-6).
"""

from __future__ import annotations

import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates

import config
from app.database import get_db, init_db, trace_writer
from app.middleware import PlaneDispatchMiddleware
from app.routes.api import router as api_router
from app.routes.ui import router as ui_router

logger = logging.getLogger("hookbox")
logging.basicConfig(level=logging.INFO)

BASE_DIR = Path(__file__).parent.parent


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup/shutdown (arch §2): init DB, open Redis pool, start the pub/sub
    relay + retention sweep, open the long-lived trace-writer connection, warm
    the rule cache. Each step degrades gracefully (Redis may be down)."""
    await init_db()
    await trace_writer.connect()

    # Redis pool (state/CRUD/rate-limit/pubsub). Never crash if Redis is down.
    try:
        from app.redis_state import redis_state
        await redis_state.connect()
    except Exception:  # noqa: BLE001
        logger.warning("Redis pool not available at startup; degrading per §5.11")

    # Pub/sub relay (trace:* fan-out + cfg:* cache invalidation), if present.
    relay_task = None
    try:
        from app.pubsub import start_relay
        relay_task = await start_relay()
    except Exception:  # noqa: BLE001
        logger.info("pub/sub relay not started (module pending or Redis down)")

    # Retention sweep (both caps, every RETENTION_SWEEP_SECONDS).
    sweep_task = None
    try:
        from app.utils.cleanup import start_retention_task
        sweep_task = start_retention_task()
    except Exception:  # noqa: BLE001
        logger.info("retention sweep not started (module pending)")

    if config.PATH_FALLBACK_ONLY:
        logger.warning(
            "MOCK_DOMAIN unset/misconfigured (%r) -> path-fallback-only mode; "
            "mock surface reachable only at /e/<token>/...", config.MOCK_DOMAIN,
        )

    try:
        yield
    finally:
        for task in (relay_task, sweep_task):
            if task is not None:
                task.cancel()
        await trace_writer.close()
        try:
            from app.redis_state import redis_state
            await redis_state.close()
        except Exception:  # noqa: BLE001
            pass
        # Close the shared MITM httpx client (issue .16).
        try:
            from app.interceptor.proxy import aclose as proxy_aclose
            await proxy_aclose()
        except Exception:  # noqa: BLE001
            pass


app = FastAPI(
    title="HookBox",
    description="Self-hosted API mocking & HTTP interception platform",
    version="2.0.0",
    lifespan=lifespan,
)

# Plane dispatch runs before routing (arch §3.1).
app.add_middleware(PlaneDispatchMiddleware)

# Static assets (P3). Frontend lane owns the files; we just mount the dir.
_static_dir = BASE_DIR / "static"
_static_dir.mkdir(parents=True, exist_ok=True)
app.mount("/static", StaticFiles(directory=str(_static_dir)), name="static")

templates = Jinja2Templates(directory=str(BASE_DIR / "templates"))

# P2 management API + P3 UI (explicit routers, matched before the catch-all).
app.include_router(api_router)
app.include_router(ui_router)


# --- Health (P3, public) ------------------------------------------------------
@app.get("/healthz")
async def healthz():
    """Liveness/readiness probe (§5.2 #19). Reports redis + db reachability."""
    redis_ok = False
    try:
        from app.redis_state import redis_state
        redis_ok = await redis_state.healthy()
    except Exception:  # noqa: BLE001
        redis_ok = False

    db_ok = False
    try:
        from app.database import DATABASE_PATH
        import aiosqlite
        async with aiosqlite.connect(DATABASE_PATH) as db:
            await db.execute("SELECT 1")
        db_ok = True
    except Exception:  # noqa: BLE001
        db_ok = False

    return {"status": "ok", "redis": redis_ok, "db": db_ok}


# --- WebSocket / SSE live feed (P3) ------------------------------------------
# The owner-gated WS/SSE feed (§5.4, OQ-4) is registered by app/websocket.py when
# that module is wired (issue hookbox-wrd.20). We attach it here if available so
# main.py stays the single registration point.
try:
    from app.websocket import register_feed_routes
    register_feed_routes(app)
except Exception:  # noqa: BLE001
    logger.info("live-feed WS/SSE routes not registered yet (pending wave)")

# Tunnel WS control channel (§5.12), if present (issue hookbox-wrd.21).
try:
    from app.routes.tunnel import register_tunnel_routes
    register_tunnel_routes(app)
except Exception:  # noqa: BLE001
    logger.info("tunnel WS routes not registered yet (pending wave)")


# --- P1 mock catch-all (registered LAST so explicit routes win) ---------------
_MOCK_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"]


@app.api_route("/{full_path:path}", methods=_MOCK_METHODS)
async def mock_catch_all(full_path: str, request: Request):
    """The behavioral P1 mock surface (§5.5).

    Guard 1 (plane): only ``request.state.plane == "mock"`` reaches the engine;
    anything tagged ``api``/``ui`` that fell through to here returns the UI 404 —
    the mock catch-all never serves API/UI content (AC-6).
    """
    plane = getattr(request.state, "plane", "ui")
    if plane != "mock":
        # An /api or UI path that wasn't matched by an explicit route → UI 404.
        return JSONResponse(
            status_code=404,
            content={"error": "not_found", "detail": "Resource not found."},
        )

    token = getattr(request.state, "token", None)
    mock_path = getattr(request.state, "mock_path", None) or "/" + full_path

    if not token:
        return JSONResponse(
            status_code=404,
            content={"error": "unknown_endpoint", "detail": "No endpoint token."},
        )

    # Delegate to the interceptor engine if it is wired (issue hookbox-wrd.14).
    # The engine owns the frozen resolution order + 404/410 token disposition.
    try:
        from app.interceptor.engine import handle_mock
    except Exception:  # noqa: BLE001 — engine pending in a later wave
        # Until the engine lands, still honor the unknown/gone token contract so
        # the dispatch + isolation behavior (this issue) is independently correct.
        return await _fallback_token_disposition(token, mock_path, request)

    return await handle_mock(request, token=token, mock_path=mock_path)


async def _fallback_token_disposition(token: str, mock_path: str, request: Request) -> Response:
    """Minimal unknown(404)/gone(410) disposition used until the engine is wired.

    Reads the durable ``endpoints`` table directly (cold path, pre-engine) and a
    Redis ``gone:<token>`` tombstone set on owner-delete (§5.5, OQ-1):
      * token present in ``endpoints``           -> engine pending → 501 placeholder
      * token absent but tombstoned (was valid)  -> 410 endpoint_gone (not logged)
      * token absent and never seen              -> 404 unknown_endpoint (not logged)
    """
    from app.database import DATABASE_PATH
    import aiosqlite

    exists = False
    try:
        async with aiosqlite.connect(DATABASE_PATH) as db:
            cur = await db.execute("SELECT 1 FROM endpoints WHERE token = ?", (token,))
            exists = await cur.fetchone() is not None
    except Exception:  # noqa: BLE001
        exists = False

    base_headers = {"X-HookBox-Endpoint": token}

    if exists:
        return JSONResponse(
            status_code=501,
            content={"error": "engine_pending", "detail": "Interceptor engine not yet wired."},
            headers=base_headers,
        )

    # Distinguish gone vs unknown via the tombstone (best-effort; Redis may be down).
    gone = False
    try:
        from app.redis_state import redis_state
        gone = await redis_state.is_gone(token)
    except Exception:  # noqa: BLE001
        gone = False

    if gone:
        return JSONResponse(
            status_code=410,
            content={"error": "endpoint_gone", "detail": "This endpoint was deleted or expired."},
            headers=base_headers,
        )
    return JSONResponse(
        status_code=404,
        content={"error": "unknown_endpoint", "detail": "No such endpoint."},
        headers=base_headers,
    )


if __name__ == "__main__":  # pragma: no cover
    import uvicorn
    uvicorn.run(app, host=config.HOST, port=config.PORT)
