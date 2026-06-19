"""Tunnel WS control-channel server (§5.12, OQ-5; arch §4.6, AC-40/40a/41/S27).

The ``mock-tunnel`` CLI opens **one** WebSocket to ``/ws/tunnel/{slug}`` and the
server multiplexes every public request hitting ``<slug>.<MOCK_DOMAIN>`` (the
**tunnel** branch of the §5.5 resolution order: ``rule > CRUD > tunnel > MITM >
default``) down that socket, replaying the CLI's response back to the public
caller.

**Bind auth (FROZEN — §5.12 / AC-S27).** The CLI presents the endpoint's owner
capability as ``Authorization: Bearer <owner_secret>`` on the WS handshake. The
server verifies the resolved owner **owns ``{slug}`` BEFORE registration /
``accept()`` (via :func:`app.auth.verify_cap_owns_token`); an unauthenticated or
wrong-owner bind is rejected with **WS close ``4401``** and never registered.
Because binding is capability-gated, a cross-owner hijack is impossible.

**Slug contention (FROZEN — last authenticated bind wins / takeover).** A second
*correctly-authenticated owner* CLI binding an already-bound ``{slug}`` **takes
over**: the new tunnel is registered and the prior connection is closed with a
clear ``{t:"err", message:"rebound elsewhere"}`` frame followed by a WS close.
Subsequent public traffic goes to the new tunnel.

**Framing (JSON text frames, multiplexed by request id — §5.12 / arch §4.6):**
  * ``→client {t:"req", id, method, path, query, headers, body_b64}``
  * ``←client {t:"res", id, status, headers, body_b64}``
  * ``↔      {t:"ping"} / {t:"pong"}``  (keepalive)
  * ``←client {t:"err", id, message}``

**No tunnel connected:** public callers get ``504 {error:"no_tunnel"}`` (not a
hang) — :func:`forward_to_tunnel` is only reached when :meth:`is_active` is true,
and any drop / per-request timeout (``TUNNEL_REQUEST_TIMEOUT_S``, AC-41) resolves
the pending future to a deterministic 504. Tunnelled traffic is still subject to
the same ingest/rate caps as every other P1 path (the engine applies those before
the forward — AC-S18/S19).
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
from typing import Dict, Optional, Tuple

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse, Response

import config
from app.auth import verify_cap_owns_token

logger = logging.getLogger("hookbox.tunnel")

# WS close codes (mirror §5.4 conventions).
WS_CLOSE_UNAUTHORIZED = 4401   # missing/wrong/cross-owner cap on bind (AC-S27)
WS_CLOSE_REBOUND = 4409        # prior connection displaced by a takeover (§5.12)


def _parse_bearer(authorization: Optional[str]) -> Optional[str]:
    """Extract ``<secret>`` from an ``Authorization: Bearer <secret>`` header.

    Returns ``None`` for a missing/garbage header so the caller refuses the bind.
    """
    if not authorization:
        return None
    parts = authorization.split(" ", 1)
    if len(parts) != 2 or parts[0].lower() != "bearer" or not parts[1].strip():
        return None
    return parts[1].strip()


class TunnelConnection:
    """One bound CLI control channel for a slug.

    Owns the WS socket, a monotonically-increasing request-id counter, and the map
    of in-flight request ids → ``asyncio.Future`` awaiting the CLI's ``res``/``err``
    frame. Thread-safety is provided by the single-threaded asyncio event loop;
    sends are serialized through ``_send_lock`` so concurrent public requests can't
    interleave partial frames.
    """

    def __init__(self, slug: str, websocket: WebSocket) -> None:
        self.slug = slug
        self.ws = websocket
        self._next_id = 0
        self._pending: Dict[int, "asyncio.Future[Tuple[int, dict, bytes]]"] = {}
        self._send_lock = asyncio.Lock()
        self.closed = False

    def _alloc_id(self) -> int:
        self._next_id += 1
        return self._next_id

    async def send_request(
        self,
        method: str,
        path: str,
        query: Dict[str, str],
        headers: Dict[str, str],
        body: bytes,
    ) -> Tuple[int, dict, bytes]:
        """Frame a public request to the CLI and await its response.

        Returns ``(status, headers, body_bytes)``. Raises :class:`TunnelError` on
        timeout (``TUNNEL_REQUEST_TIMEOUT_S``), a CLI ``err`` frame, or a dropped
        control channel so the engine surfaces a deterministic ``504`` (AC-40a/41).
        """
        if self.closed:
            raise TunnelError("tunnel closed")
        req_id = self._alloc_id()
        loop = asyncio.get_running_loop()
        fut: "asyncio.Future[Tuple[int, dict, bytes]]" = loop.create_future()
        self._pending[req_id] = fut
        frame = {
            "t": "req",
            "id": req_id,
            "method": method,
            "path": path,
            "query": query,
            "headers": headers,
            "body_b64": base64.b64encode(body or b"").decode("ascii"),
        }
        try:
            async with self._send_lock:
                await self.ws.send_text(json.dumps(frame))
        except Exception as exc:  # noqa: BLE001 - socket went away mid-send
            self._pending.pop(req_id, None)
            raise TunnelError("tunnel send failed") from exc

        try:
            return await asyncio.wait_for(fut, timeout=config.TUNNEL_REQUEST_TIMEOUT_S)
        except asyncio.TimeoutError as exc:
            self._pending.pop(req_id, None)
            raise TunnelError("tunnel request timed out") from exc
        finally:
            self._pending.pop(req_id, None)

    def resolve(self, req_id: int, status: int, headers: dict, body: bytes) -> None:
        """Complete the pending future for ``req_id`` with the CLI's response."""
        fut = self._pending.get(req_id)
        if fut is not None and not fut.done():
            fut.set_result((status, headers, body))

    def reject(self, req_id: int, message: str) -> None:
        """Fail the pending future for ``req_id`` (CLI ``err`` frame)."""
        fut = self._pending.get(req_id)
        if fut is not None and not fut.done():
            fut.set_exception(TunnelError(message))

    def fail_all(self, message: str) -> None:
        """Fail every in-flight request (control channel dropped — AC-41)."""
        for fut in list(self._pending.values()):
            if not fut.done():
                fut.set_exception(TunnelError(message))
        self._pending.clear()

    async def send_keepalive_pong(self) -> None:
        try:
            async with self._send_lock:
                await self.ws.send_text(json.dumps({"t": "pong"}))
        except Exception:  # noqa: BLE001
            pass

    async def notify_rebound(self) -> None:
        """Tell a displaced CLI it was rebound elsewhere, then close (§5.12)."""
        try:
            async with self._send_lock:
                await self.ws.send_text(json.dumps({"t": "err", "message": "rebound elsewhere"}))
        except Exception:  # noqa: BLE001
            pass
        try:
            await self.ws.close(code=WS_CLOSE_REBOUND, reason="rebound elsewhere")
        except Exception:  # noqa: BLE001
            pass


class TunnelError(Exception):
    """A tunnel forward could not complete (drop / timeout / CLI error)."""


class TunnelRegistry:
    """Process-local map of ``slug -> TunnelConnection`` (single-instance scope).

    The engine calls :meth:`is_active` (the §5.5 tunnel-branch gate) and
    :func:`forward_to_tunnel` reads the bound connection. :meth:`bind` implements
    the last-authenticated-bind-wins takeover (§5.12).
    """

    def __init__(self) -> None:
        self._tunnels: Dict[str, TunnelConnection] = {}

    def is_active(self, slug: str) -> bool:
        conn = self._tunnels.get(slug)
        return conn is not None and not conn.closed

    def get(self, slug: str) -> Optional[TunnelConnection]:
        conn = self._tunnels.get(slug)
        if conn is not None and conn.closed:
            return None
        return conn

    async def bind(self, slug: str, conn: TunnelConnection) -> Optional[TunnelConnection]:
        """Register ``conn`` for ``slug``; return the displaced prior connection (if
        any) so the caller can notify+close it (takeover — §5.12). Last bind wins."""
        prior = self._tunnels.get(slug)
        self._tunnels[slug] = conn
        if prior is not None and prior is not conn:
            prior.closed = True
            prior.fail_all("rebound elsewhere")
            return prior
        return None

    def unbind(self, slug: str, conn: TunnelConnection) -> bool:
        """Remove ``conn`` iff it is still the registered tunnel for ``slug``.

        Returns True if it was the active binding (so the caller flips
        ``tunnel_active`` / notifies the dashboard). A connection that was already
        displaced by a takeover is a no-op here (the new tunnel stays bound)."""
        current = self._tunnels.get(slug)
        if current is conn:
            self._tunnels.pop(slug, None)
            return True
        return False


# Process-local singleton resolved by the engine via ``_opt(...)`` (arch §4.1).
tunnel_registry = TunnelRegistry()


async def forward_to_tunnel(
    token: str,
    method: str,
    mock_path: str,
    query: Dict[str, str],
    headers: Dict[str, str],
    body: bytes,
) -> Response:
    """Engine entry point: replay a public request down the bound tunnel (§5.5).

    Returns the CLI's response as a Starlette ``Response``. On no/closed tunnel,
    timeout, or a CLI error, returns a deterministic ``504 {error:"no_tunnel"}``
    (AC-40a/41) rather than hanging.
    """
    conn = tunnel_registry.get(token)
    if conn is None:
        return JSONResponse(
            status_code=504,
            content={"error": "no_tunnel", "detail": "No tunnel is connected for this endpoint."},
        )
    try:
        status, resp_headers, resp_body = await conn.send_request(
            method, mock_path, query, headers, body
        )
    except TunnelError as exc:
        logger.info("tunnel forward failed for %s: %s", token, exc)
        return JSONResponse(
            status_code=504,
            content={"error": "no_tunnel", "detail": "Tunnel request did not complete."},
        )

    # Strip hop-by-hop headers the upstream localhost server may have set; the
    # engine re-applies identifying + CORS headers afterwards.
    safe_headers = {
        k: v for k, v in (resp_headers or {}).items()
        if k.lower() not in {
            "connection", "keep-alive", "transfer-encoding", "content-length",
            "content-encoding",
        }
    }
    return Response(content=resp_body, status_code=int(status), headers=safe_headers)


async def _publish_endpoint_updated(token: str) -> None:
    """Best-effort: tell the dashboard the endpoint's ``tunnel_active`` changed so
    it refreshes settings (§5.4 ``endpoint_updated``). Swallowed on Redis loss."""
    try:
        from app.redis_state import redis_state
        await redis_state.publish_trace(
            token, {"type": "endpoint_updated", "data": {"token": token, "fields": ["tunnel_active"]}}
        )
    except Exception:  # noqa: BLE001
        logger.debug("endpoint_updated publish failed for %s (swallowed)", token)


def register_tunnel_routes(app: FastAPI) -> None:
    """Register ``/ws/tunnel/{slug}``. Called once from ``app/main.py``."""

    @app.websocket("/ws/tunnel/{slug}")
    async def tunnel_ws(websocket: WebSocket, slug: str):
        # --- Bind auth BEFORE accept() (§5.12 / AC-S27) -----------------------
        # The capability arrives on the Authorization header of the WS handshake;
        # browsers can't set it but the CLI (and tests) can. We also accept a
        # ``?cap=`` query fallback parity with the feed gate, but the header is the
        # frozen transport.
        secret = _parse_bearer(websocket.headers.get("authorization"))
        if secret is None:
            secret = websocket.query_params.get("cap")
        if not await verify_cap_owns_token(slug, secret):
            # Refuse with close code 4401 (AC-S27 negative tests a/b). We accept()
            # BEFORE close() so the CLI receives the application close code 4401 on
            # the wire: a ``websocket.close`` sent while still CONNECTING is turned
            # into an HTTP 403 handshake rejection by the ASGI server, discarding
            # the code (the CLI's websockets client would raise InvalidStatus
            # 'HTTP 403' with rcvd_close=None instead of seeing 4401). No
            # registration occurs and zero data frames are sent before the close,
            # so the security outcome (rejected bind, A's tunnel untouched) holds.
            await websocket.accept()
            await websocket.close(code=WS_CLOSE_UNAUTHORIZED, reason="unauthorized")
            return

        await websocket.accept()
        conn = TunnelConnection(slug, websocket)

        # Last-authenticated-bind-wins takeover (§5.12): displace any prior tunnel.
        displaced = await tunnel_registry.bind(slug, conn)
        if displaced is not None:
            asyncio.create_task(displaced.notify_rebound())
        await _publish_endpoint_updated(slug)

        try:
            # Greet the CLI so it can confirm the bind succeeded.
            await websocket.send_text(json.dumps({"t": "bound", "slug": slug}))
            while True:
                raw = await websocket.receive_text()
                await _handle_cli_frame(conn, raw)
        except WebSocketDisconnect:
            pass
        except Exception:  # noqa: BLE001
            logger.debug("tunnel ws error for %s (closing)", slug)
        finally:
            conn.closed = True
            conn.fail_all("tunnel disconnected")
            was_active = tunnel_registry.unbind(slug, conn)
            if was_active:
                # Only announce inactivity if this connection was still the bound
                # one (a displaced connection's slot now belongs to the new tunnel).
                await _publish_endpoint_updated(slug)


async def _handle_cli_frame(conn: TunnelConnection, raw: str) -> None:
    """Parse and dispatch one inbound CLI frame. Never raises into the read loop."""
    try:
        frame = json.loads(raw)
    except (ValueError, TypeError):
        return
    if not isinstance(frame, dict):
        return
    ftype = frame.get("t")

    if ftype == "res":
        req_id = frame.get("id")
        if not isinstance(req_id, int):
            return
        status = int(frame.get("status", 200))
        headers = frame.get("headers") or {}
        if not isinstance(headers, dict):
            headers = {}
        body_b64 = frame.get("body_b64") or ""
        try:
            body = base64.b64decode(body_b64) if body_b64 else b""
        except (ValueError, TypeError):
            body = b""
        conn.resolve(req_id, status, headers, body)

    elif ftype == "err":
        req_id = frame.get("id")
        message = str(frame.get("message", "tunnel error"))
        if isinstance(req_id, int):
            conn.reject(req_id, message)

    elif ftype == "ping":
        await conn.send_keepalive_pong()

    elif ftype == "pong":
        # Client answering our (rare) ping; nothing to do.
        return
