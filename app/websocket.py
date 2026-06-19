"""Real-time live-feed transport — owner-gated WS + SSE (§5.4, OQ-4, AC-27/29/30).

This module REPLACES the old in-process WebSocket manager. It is now **fed by the
Redis pub/sub relay** (``app/pubsub.py``): a served mock request publishes a
``new_request`` event to ``trace:<token>``; the relay calls
:meth:`ConnectionManager.broadcast` which fans the event out to every locally
connected WS **and** SSE client for that token. Mock serving + SQLite logging are
never on this path (R4/§5.11) — a dead Redis degrades only the live feed.

Owner gate (OQ-4 / AC-S12, the security-mandated change folded into §5.4):
  subscribing to a feed REQUIRES the owner capability, presented as the
  ``?cap=<owner_secret>`` query parameter and verified **server-side before**
  ``accept()`` (WS) / before the first ``data:`` frame (SSE). The server resolves
  the token, hashes the cap, and confirms the resolved owner owns the token;
  otherwise it refuses **before any frame** — WS close ``4401`` / SSE ``401``. An
  anonymous, wrong-cap, or cross-owner subscribe receives **zero** events.

Channel isolation (AC-32 / AC-S13): the registry is keyed by token, so a client
authed for ``tokenA`` only ever receives ``tokenA`` events.

Connection cap (AC-S22): concurrent feed connections per endpoint are bounded by
``WS_MAX_CONN_PER_ENDPOINT``; excess are refused (WS close ``1013`` / SSE ``503``)
so fan-out memory can't grow unbounded.

Backpressure (R8): :meth:`broadcast` sends per-client with a short timeout
(``WS_SEND_TIMEOUT_S``) and drops dead/slow clients rather than stalling the relay.
"""

from __future__ import annotations

import asyncio
import json
import logging
from datetime import datetime, timezone
from typing import Dict, Set

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse, StreamingResponse

import config
from app.auth import verify_cap_owns_token

logger = logging.getLogger("hookbox.ws")

# WS close codes (§5.4).
WS_CLOSE_UNAUTHORIZED = 4401   # missing/wrong/cross-owner cap (OQ-4)
WS_CLOSE_TRY_LATER = 1013      # connection cap reached (AC-S22)

# SSE heartbeat interval (keeps proxies from idling the stream out; the client
# also pings — AC-30d). A comment line ``: ping`` is ignored by EventSource.
_SSE_HEARTBEAT_S = 25.0


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


class ConnectionManager:
    """Local registry of feed subscribers (WS sockets + SSE queues) per token.

    ``broadcast(token, payload)`` is the single fan-out entry point used by the
    pub/sub relay; it must match the callable the relay resolves
    (``async (token, payload) -> None``).
    """

    def __init__(self) -> None:
        self._ws: Dict[str, Set[WebSocket]] = {}
        self._sse: Dict[str, Set[asyncio.Queue]] = {}

    # --- counts / caps --------------------------------------------------------
    def count(self, token: str) -> int:
        return len(self._ws.get(token, ())) + len(self._sse.get(token, ()))

    def at_capacity(self, token: str) -> bool:
        return self.count(token) >= config.WS_MAX_CONN_PER_ENDPOINT

    # --- WS registry ----------------------------------------------------------
    def add_ws(self, token: str, ws: WebSocket) -> None:
        self._ws.setdefault(token, set()).add(ws)

    def remove_ws(self, token: str, ws: WebSocket) -> None:
        conns = self._ws.get(token)
        if conns is not None:
            conns.discard(ws)
            if not conns:
                self._ws.pop(token, None)

    # --- SSE registry ---------------------------------------------------------
    def add_sse(self, token: str, queue: asyncio.Queue) -> None:
        self._sse.setdefault(token, set()).add(queue)

    def remove_sse(self, token: str, queue: asyncio.Queue) -> None:
        queues = self._sse.get(token)
        if queues is not None:
            queues.discard(queue)
            if not queues:
                self._sse.pop(token, None)

    # --- fan-out (called by the pub/sub relay) --------------------------------
    async def broadcast(self, token: str, payload: dict) -> None:
        """Fan ``payload`` out to all WS + SSE clients subscribed to ``token``.

        Per-client send is guarded by ``WS_SEND_TIMEOUT_S``; a dead/slow client is
        dropped (R8). Never raises into the relay.
        """
        # WS clients — sent CONCURRENTLY (each with its own timeout) so a slow socket
        # can't serialize the fan-out. The relay loop awaits this inline for every
        # token, so sequential sends would let one endpoint's slow clients (up to the
        # per-endpoint cap × WS_SEND_TIMEOUT_S) stall the global feed; with gather the
        # broadcast is bounded by a single timeout regardless of client count.
        ws_clients = list(self._ws.get(token, ()))
        if ws_clients:
            async def _send(ws: WebSocket):
                try:
                    await asyncio.wait_for(ws.send_json(payload), timeout=config.WS_SEND_TIMEOUT_S)
                    return None
                except Exception:  # noqa: BLE001 - drop dead/slow socket
                    return ws
            for ws in await asyncio.gather(*[_send(c) for c in ws_clients]):
                if ws is not None:
                    self.remove_ws(token, ws)

        # SSE clients (non-blocking put; if a client's buffer is full, drop it).
        dead_q = []
        for q in list(self._sse.get(token, ())):
            try:
                q.put_nowait(payload)
            except asyncio.QueueFull:
                dead_q.append(q)
            except Exception:  # noqa: BLE001
                dead_q.append(q)
        for q in dead_q:
            self.remove_sse(token, q)

    # Back-compat alias (some call sites historically used this name).
    async def broadcast_to_endpoint(self, token: str, payload: dict) -> None:
        await self.broadcast(token, payload)


manager = ConnectionManager()


# --- route registration -------------------------------------------------------
def register_feed_routes(app: FastAPI) -> None:
    """Register the owner-gated WS (`/ws/{token}`) + SSE (`/sse/{token}`) feed.

    Called once from ``app/main.py`` so main stays the single registration point.
    """

    @app.websocket("/ws/{token}")
    async def feed_ws(websocket: WebSocket, token: str):
        # OQ-4 gate: verify the capability and refuse with close code 4401 BEFORE
        # any data frame on failure (AC-S12 / AC-29 / AC-30).
        #
        # We MUST accept() before close() to deliver the application close code.
        # Per the ASGI/Starlette state machine, a ``websocket.close`` sent while
        # still CONNECTING is translated by the server into an HTTP 403 handshake
        # rejection, which discards the close code — the browser then sees onclose
        # code 1006 (abnormal), never 4401, so request-stream.js misses its 4401
        # branch and hammers the gate with backoff reconnects (the anti-behavior
        # AC-30 forbids). Accepting first, then closing, sends a real close frame
        # carrying code 4401. This still satisfies §5.4 "refuse before any frame"
        # because we send ZERO data frames (no hello, no new_request) before the
        # close — only the close itself. Security outcome (refused, zero events)
        # is unchanged.
        cap = websocket.query_params.get("cap")
        if not await verify_cap_owns_token(token, cap):
            await websocket.accept()
            await websocket.close(code=WS_CLOSE_UNAUTHORIZED)
            return

        # Connection cap (AC-S22): refuse excess. Same accept()-then-close() so the
        # client receives the 1013 (try-later) close code on the wire.
        if manager.at_capacity(token):
            await websocket.accept()
            await websocket.close(code=WS_CLOSE_TRY_LATER)
            return

        await websocket.accept()
        manager.add_ws(token, websocket)
        try:
            # hello on connect (lets the client sync + confirm the channel, §5.4).
            await websocket.send_json({
                "type": "hello",
                "data": {"token": token, "server_time": _now_iso()},
            })
            # Server→client only; we read inbound text purely to detect close and
            # to answer client keepalive pings (§5.4 c→s "ping").
            while True:
                msg = await websocket.receive_text()
                if msg == "ping":
                    # Starlette auto-answers protocol-level pings; for the app-level
                    # "ping" text we reply with a pong frame via send.
                    try:
                        await websocket.send_text("pong")
                    except Exception:  # noqa: BLE001
                        break
        except WebSocketDisconnect:
            pass
        except Exception:  # noqa: BLE001
            logger.debug("ws feed error for %s (closing)", token)
        finally:
            manager.remove_ws(token, websocket)

    @app.get("/sse/{token}")
    async def feed_sse(token: str, cap: str | None = None):
        # OQ-4 gate: verify the capability BEFORE the first data: frame.
        if not await verify_cap_owns_token(token, cap):
            return JSONResponse(
                status_code=401,
                content={"error": "unauthorized", "detail": "Valid owner capability required."},
            )
        if manager.at_capacity(token):
            return JSONResponse(
                status_code=503,
                content={"error": "too_many_connections", "detail": "Feed connection cap reached."},
            )

        queue: asyncio.Queue = asyncio.Queue(maxsize=1000)
        manager.add_sse(token, queue)

        async def event_stream():
            try:
                # hello first (mirrors the WS handshake, §5.4).
                yield _sse_format("hello", {"token": token, "server_time": _now_iso()})
                while True:
                    try:
                        payload = await asyncio.wait_for(queue.get(), timeout=_SSE_HEARTBEAT_S)
                    except asyncio.TimeoutError:
                        # Heartbeat comment (ignored by EventSource) to keep alive.
                        yield ": ping\n\n"
                        continue
                    event_type = payload.get("type", "message") if isinstance(payload, dict) else "message"
                    data = payload.get("data", payload) if isinstance(payload, dict) else payload
                    yield _sse_format(event_type, data)
            except asyncio.CancelledError:
                raise
            finally:
                manager.remove_sse(token, queue)

        return StreamingResponse(
            event_stream(),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "X-Accel-Buffering": "no",  # disable proxy buffering
                "Connection": "keep-alive",
            },
        )


def _sse_format(event_type: str, data) -> str:
    """One SSE frame: ``event:`` line + ``data:`` line(s). Payload is JSON."""
    body = json.dumps(data, default=str)
    return f"event: {event_type}\ndata: {body}\n\n"
