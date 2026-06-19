"""PlaneDispatchMiddleware (arch §3.1, AC-4/5/6/6a).

Runs **before** routing. It inspects ``Host`` + ``path`` via the pure
``app.planes.resolve_plane`` and tags the request:

  * ``request.state.plane``      -> "mock" | "api" | "ui"
  * ``request.state.token``      -> endpoint token (mock plane only, else None)
  * ``request.state.mock_path``  -> the path handed to the interceptor (mock only)

Cheap: no DB, no Redis. Two independent guards keep the mock catch-all from ever
shadowing ``/api`` or the UI (arch §3.1):

1. **Host guard (here):** on the **app host** a request only becomes ``mock`` if it
   carries the ``/e/<token>/`` prefix; plain ``/api/...`` / ``/d/...`` resolve to
   ``api`` / ``ui``. If a request that resolved to ``api``/``ui`` somehow reaches
   the catch-all, the catch-all checks ``request.state.plane`` and returns the UI
   404 instead of mock content.
2. **Route-order guard (main.py):** the catch-all route is registered **last**, so
   Starlette matches the explicit ``/api``, ``/``, ``/d``, ``/static``, ``/ws`` and
   ``/sse`` routes first.

Conversely, on a **mock host** (``<token>.<MOCK_DOMAIN>``) *every* path — including
``/api`` and ``/static`` — is ``mock`` for that endpoint, so the management API is
unreachable there by construction (AC-6).

**Implementation: pure ASGI middleware (not ``BaseHTTPMiddleware``).** Starlette's
``BaseHTTPMiddleware`` re-wraps the downstream response and streams its body through
an anyio memory stream; when the client disconnects mid-stream (or a recycled
keep-alive connection is interrupted) that wrapper can desync the buffered body
against ``Content-Length`` and raise ``RuntimeError: Response content longer than
Content-Length``. This middleware never buffers or re-emits the body: it only sets
plane state on ``scope["state"]`` (which backs ``request.state``) and injects the
``X-HookBox-Plane`` observability header by mutating the ``http.response.start``
message headers in a thin ``send`` wrapper. The response body bytes pass through
untouched, so the ``Content-Length`` desync class is structurally impossible here.
"""

from __future__ import annotations

import logging

from starlette.datastructures import Headers, MutableHeaders
from starlette.types import ASGIApp, Message, Receive, Scope, Send

from app.planes import resolve_plane

logger = logging.getLogger("hookbox.middleware")


class PlaneDispatchMiddleware:
    """Pure ASGI 3 middleware. ``add_middleware(PlaneDispatchMiddleware)`` calls
    ``PlaneDispatchMiddleware(app)``; Starlette then invokes ``__call__``."""

    def __init__(self, app: ASGIApp) -> None:
        self.app = app

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        # Only HTTP carries the plane header + Content-Length; pass websocket and
        # lifespan straight through untouched. (WS plane resolution is done in the
        # WS route handlers themselves from the path/host.)
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        host = Headers(scope=scope).get("host", "")
        path = scope.get("path", "") or "/"
        result = resolve_plane(host, path)

        # Back ``request.state.plane/token/mock_path`` via scope["state"] — Starlette
        # lazily does ``scope.setdefault("state", {})`` and reads from it, so
        # downstream handlers see these without a Request wrapper here.
        state = scope.setdefault("state", {})
        state["plane"] = result.plane
        state["token"] = result.token
        state["mock_path"] = result.mock_path

        plane_value = result.plane

        async def send_with_plane_header(message: Message) -> None:
            # Inject the observability header on the response start ONLY; never
            # touch the body messages (no buffering, no Content-Length rewrite).
            if message["type"] == "http.response.start":
                headers = MutableHeaders(scope=message)
                headers["X-HookBox-Plane"] = plane_value
            await send(message)

        await self.app(scope, receive, send_with_plane_header)
