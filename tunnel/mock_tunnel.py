"""``mock-tunnel`` — HookBox local tunnel reference CLI (LOCKED §8, AC-40/41/S27).

Reverse-tunnels public traffic hitting ``<slug>.<MOCK_DOMAIN>`` down to a local
dev server, over a **single multiplexed WebSocket control channel** to the HookBox
server (``/ws/tunnel/<slug>``). This is the spec's "blueprint + simple CLI"; a Go
binary would be the production-grade choice (``_decisions.md`` §8) — this Python
reference satisfies the blueprint and the frozen §5.12 protocol exactly.

Usage::

    mock-tunnel --port 3000 --endpoint <slug> \\
        --server wss://hookbox.example.com --secret <owner_secret>

    # or, run as a module from the repo root:
    python -m tunnel --port 3000 --endpoint <slug> --server ws://localhost:8000 \\
        --secret <owner_secret>

Behavior (frozen §5.12 / arch §4.6):
  * **Bind auth (AC-S27):** presents ``Authorization: Bearer <owner_secret>`` on
    the WS handshake. A ``4401`` close (unauthenticated / wrong owner / slug not
    owned) is **fatal** — the CLI prints "auth failed / not your endpoint" and
    exits 2 (it does NOT blindly retry an auth failure).
  * **Forwarding (AC-40):** each ``{t:"req",...}`` frame is replayed to
    ``http://localhost:<port>`` and the local response framed back as
    ``{t:"res",...}``; a local connection error → ``{t:"err",...}`` so the public
    caller gets a deterministic 504.
  * **Takeover (§5.12):** an ``{t:"err", message:"rebound elsewhere"}`` (a second
    owner CLI bound the same slug) is fatal here — this CLI exits cleanly.
  * **Reconnect (AC-41):** any other drop triggers exponential-backoff reconnect
    (250ms→8s + jitter) and forwarding resumes; an in-flight public request during
    the gap is failed server-side (504) rather than waiting forever.

Dependencies: ``websockets`` (control channel; pinned in ``requirements.txt``) +
the Python stdlib (``argparse``, ``http.client``, ``asyncio``) — no extra deps for
the local replay.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import http.client
import json
import random
import signal
import sys
from typing import Dict, Optional, Tuple
from urllib.parse import urlencode

try:
    import websockets
    import websockets.exceptions as _ws_exc

    WebSocketException = _ws_exc.WebSocketException
    ConnectionClosed = _ws_exc.ConnectionClosed
    # ``InvalidStatus`` (websockets >=13) / ``InvalidStatusCode`` (<=12) carry the
    # rejected-handshake HTTP status. Resolve whichever this version ships and
    # alias the missing one to a never-raised placeholder so ``except (...)`` is safe.
    class _NeverRaised(Exception):  # placeholder for an absent exception class
        ...

    InvalidStatus = getattr(_ws_exc, "InvalidStatus", _NeverRaised)
    InvalidStatusCode = getattr(_ws_exc, "InvalidStatusCode", _NeverRaised)
except ImportError:  # pragma: no cover - dependency missing
    websockets = None  # type: ignore

    class WebSocketException(Exception):  # type: ignore
        ...

    class ConnectionClosed(WebSocketException):  # type: ignore
        code = None
        reason = ""

    class InvalidStatus(WebSocketException):  # type: ignore
        ...

    class InvalidStatusCode(WebSocketException):  # type: ignore
        status_code = None


# WS close codes the server uses (mirror app/routes/tunnel.py).
WS_CLOSE_UNAUTHORIZED = 4401
WS_CLOSE_REBOUND = 4409

# Reconnect backoff schedule (ms), mirroring static/js/request-stream.js (AC-41).
_BACKOFF_MS = [250, 500, 1000, 2000, 4000, 8000]
_LOCAL_REQUEST_TIMEOUT_S = 30


def _log(msg: str) -> None:
    """Single point for CLI status output (AC-46 CLI states)."""
    print(f"[mock-tunnel] {msg}", flush=True)


def _backoff_delay_s(attempt: int) -> float:
    base_ms = _BACKOFF_MS[min(attempt, len(_BACKOFF_MS) - 1)]
    jitter_ms = random.uniform(0, base_ms * 0.25)
    return (base_ms + jitter_ms) / 1000.0


class FatalTunnelError(Exception):
    """A non-retryable condition (auth failure / takeover) — exit, do not reconnect."""

    def __init__(self, message: str, exit_code: int = 2) -> None:
        super().__init__(message)
        self.exit_code = exit_code


def _build_ws_url(server: str, slug: str) -> str:
    """Compose ``<server>/ws/tunnel/<slug>`` from ``--server`` (ws[s]://host[:port])."""
    server = server.rstrip("/")
    if server.startswith(("http://", "https://")):
        # Tolerate an http(s):// server arg by mapping to ws(s)://.
        server = "ws" + server[4:]
    if not server.startswith(("ws://", "wss://")):
        server = "ws://" + server
    return f"{server}/ws/tunnel/{slug}"


def _replay_to_local(
    host: str,
    port: int,
    method: str,
    path: str,
    query: Dict[str, str],
    headers: Dict[str, str],
    body: bytes,
) -> Tuple[int, Dict[str, str], bytes]:
    """Blocking HTTP replay to the local dev server. Returns (status, headers, body).

    Runs in a thread via ``asyncio.to_thread`` so it never blocks the control-channel
    event loop. Raises on connection failure (caller frames it as an ``err``).
    """
    # Re-attach the query string to the path.
    if query:
        qs = urlencode(query, doseq=True)
        full_path = f"{path}?{qs}" if "?" not in path else f"{path}&{qs}"
    else:
        full_path = path

    conn = http.client.HTTPConnection(host, port, timeout=_LOCAL_REQUEST_TIMEOUT_S)
    try:
        # Drop hop-by-hop / host headers the server framed; set Host to the local
        # target so the dev server sees a coherent request.
        out_headers = {
            k: v for k, v in headers.items()
            if k.lower() not in {
                "connection", "keep-alive", "transfer-encoding", "upgrade",
                "host", "content-length",
            }
        }
        out_headers["Host"] = f"{host}:{port}"
        conn.request(method, full_path, body=body or None, headers=out_headers)
        resp = conn.getresponse()
        resp_body = resp.read()
        resp_headers = {k: v for k, v in resp.getheaders()}
        return resp.status, resp_headers, resp_body
    finally:
        conn.close()


async def _handle_req_frame(ws, frame: dict, local_host: str, local_port: int) -> None:
    """Replay one ``{t:"req",...}`` frame to localhost and send back the result."""
    req_id = frame.get("id")
    method = frame.get("method", "GET")
    path = frame.get("path", "/")
    query = frame.get("query") or {}
    headers = frame.get("headers") or {}
    body_b64 = frame.get("body_b64") or ""
    try:
        body = base64.b64decode(body_b64) if body_b64 else b""
    except (ValueError, TypeError):
        body = b""

    try:
        status, resp_headers, resp_body = await asyncio.to_thread(
            _replay_to_local, local_host, local_port, method, path, query, headers, body
        )
        _log(f"{method} {path} -> {status} ({len(resp_body)}B)")
        await ws.send(json.dumps({
            "t": "res",
            "id": req_id,
            "status": status,
            "headers": resp_headers,
            "body_b64": base64.b64encode(resp_body).decode("ascii"),
        }))
    except (ConnectionRefusedError, OSError) as exc:
        _log(f"{method} {path} -> LOCAL ERROR: {exc} (is localhost:{local_port} up?)")
        await ws.send(json.dumps({
            "t": "err",
            "id": req_id,
            "message": f"local replay failed: {exc}",
        }))


def _ws_connect(ws_url: str, headers: dict):
    """Open a websockets client, choosing the header kwarg this version supports.

    ``additional_headers`` (websockets >=13) vs ``extra_headers`` (<=12). We inspect
    the ``connect`` signature so we never rely on a lazily-raised ``TypeError``.
    """
    import inspect

    connect_kwargs = {"ping_interval": 20, "ping_timeout": 20, "max_size": None}
    try:
        params = inspect.signature(websockets.connect).parameters
    except (ValueError, TypeError):
        params = {}
    if "additional_headers" in params:
        return websockets.connect(ws_url, additional_headers=headers, **connect_kwargs)
    if "extra_headers" in params:
        return websockets.connect(ws_url, extra_headers=headers, **connect_kwargs)
    # Unknown signature: best effort with the modern kwarg.
    return websockets.connect(ws_url, additional_headers=headers, **connect_kwargs)


async def _serve_one_connection(
    ws_url: str, secret: str, local_host: str, local_port: int
) -> bool:
    """Open one control channel and pump frames until it closes.

    Returns ``True`` once the channel was successfully established (so the caller
    resets its reconnect backoff). Raises :class:`FatalTunnelError` on a
    non-retryable close (4401 auth / 4409 rebound); returns on a retryable drop so
    the outer loop reconnects.
    """
    headers = {"Authorization": f"Bearer {secret}"}
    connected = False
    async with _ws_connect(ws_url, headers) as ws:
        connected = True
        _log(f"Tunnel live: {ws_url} -> http://{local_host}:{local_port}")
        async for raw in ws:
            if isinstance(raw, (bytes, bytearray)):
                raw = bytes(raw).decode("utf-8", errors="replace")
            try:
                frame = json.loads(raw)
            except (ValueError, TypeError):
                continue
            if not isinstance(frame, dict):
                continue
            ftype = frame.get("t")
            if ftype == "req":
                # Fire each replay concurrently so a slow local request doesn't
                # block the control channel (multiplexed by id, §5.12).
                asyncio.create_task(_handle_req_frame(ws, frame, local_host, local_port))
            elif ftype == "bound":
                _log(f"Bound to endpoint '{frame.get('slug')}'.")
            elif ftype == "err":
                # A non-request-scoped err is a control signal (e.g. takeover).
                msg = str(frame.get("message", ""))
                if frame.get("id") is None:
                    if "rebound" in msg.lower():
                        raise FatalTunnelError(
                            "This endpoint was rebound by another tunnel "
                            "(last bind wins). Exiting.", exit_code=0,
                        )
                    _log(f"server error: {msg}")
            elif ftype == "ping":
                await ws.send(json.dumps({"t": "pong"}))
            elif ftype == "pong":
                pass
    return connected


def _classify_close(exc: Exception) -> Optional[FatalTunnelError]:
    """Map a connection-close exception to a fatal CLI error, or None to retry."""
    code = getattr(exc, "code", None)
    if code is None:
        # InvalidStatus(Code) carries the HTTP status of a rejected handshake.
        status = getattr(exc, "status_code", None)
        resp = getattr(exc, "response", None)
        if resp is not None:
            status = getattr(resp, "status_code", status)
        if status in (401, 403):
            return FatalTunnelError(
                "Authentication failed (HTTP %s): the --secret is not valid for "
                "this endpoint, or you do not own '<slug>'." % status, exit_code=2,
            )
        return None
    if code == WS_CLOSE_UNAUTHORIZED:
        return FatalTunnelError(
            "Auth failed / not your endpoint (WS 4401). Check --secret and "
            "--endpoint: the capability must own the slug.", exit_code=2,
        )
    if code == WS_CLOSE_REBOUND:
        return FatalTunnelError(
            "This endpoint was rebound by another tunnel (last bind wins). Exiting.",
            exit_code=0,
        )
    return None  # any other close → retryable


async def run_tunnel(
    server: str, slug: str, secret: str, local_host: str, local_port: int
) -> int:
    """Main reconnect loop. Returns a process exit code."""
    if websockets is None:
        _log("ERROR: the 'websockets' package is required. Install: pip install websockets")
        return 3

    ws_url = _build_ws_url(server, slug)
    _log(f"Connecting to {ws_url} (endpoint '{slug}') …")
    attempt = 0
    while True:
        connected = False
        try:
            connected = await _serve_one_connection(ws_url, secret, local_host, local_port)
            # Clean return = server closed normally; treat as retryable.
            _log("Control channel closed; reconnecting …")
        except FatalTunnelError as exc:
            _log(str(exc))
            return exc.exit_code
        except (ConnectionClosed, InvalidStatus, InvalidStatusCode) as exc:
            connected = getattr(exc, "code", None) is not None  # closed after a live handshake
            fatal = _classify_close(exc)
            if fatal is not None:
                _log(str(fatal))
                return fatal.exit_code
            _log(f"Disconnected ({exc!r}); reconnecting …")
        except (OSError, WebSocketException) as exc:
            _log(f"Connection error ({exc!r}); reconnecting …")
        except asyncio.CancelledError:
            raise
        except Exception as exc:  # noqa: BLE001
            _log(f"Unexpected error ({exc!r}); reconnecting …")

        # Reset backoff after a live connection epoch so a long-lived tunnel that
        # occasionally blips doesn't keep escalating toward the 8s cap (AC-41).
        if connected:
            attempt = 0

        delay = _backoff_delay_s(attempt)
        attempt += 1
        _log(f"Reconnecting in {delay:.1f}s (attempt {attempt}) …")
        try:
            await asyncio.sleep(delay)
        except asyncio.CancelledError:
            raise


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="mock-tunnel",
        description="Reverse-tunnel public HookBox traffic for <slug> to a local port.",
    )
    parser.add_argument(
        "--port", "-p", type=int, required=True,
        help="Local port to forward public requests to (e.g. 3000).",
    )
    parser.add_argument(
        "--endpoint", "-e", required=True,
        help="The HookBox endpoint slug/token to bind (must be owned by --secret).",
    )
    parser.add_argument(
        "--server", "-s", default="ws://localhost:8000",
        help="HookBox server WS URL (ws://host[:port] or wss://host). Default: ws://localhost:8000",
    )
    parser.add_argument(
        "--secret", required=True,
        help="Owner capability (owner_secret) — authenticates the bind (AC-S27).",
    )
    parser.add_argument(
        "--host", default="127.0.0.1",
        help="Local host to forward to. Default: 127.0.0.1",
    )
    return parser


def main(argv: Optional[list] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.port < 1 or args.port > 65535:
        _log("ERROR: --port must be 1..65535")
        return 2

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)

    # Graceful Ctrl-C.
    stop = loop.create_future()

    def _request_stop(*_a):
        if not stop.done():
            stop.set_result(None)

    try:
        loop.add_signal_handler(signal.SIGINT, _request_stop)
        loop.add_signal_handler(signal.SIGTERM, _request_stop)
    except (NotImplementedError, RuntimeError):
        # Signal handlers aren't available on all platforms (e.g. Windows).
        pass

    runner = loop.create_task(
        run_tunnel(args.server, args.endpoint, args.secret, args.host, args.port)
    )

    async def _main() -> int:
        done, _pending = await asyncio.wait(
            {runner, stop}, return_when=asyncio.FIRST_COMPLETED
        )
        if runner in done:
            return runner.result()
        # Stop requested → cancel the runner and shut down.
        _log("Shutting down (signal received) …")
        runner.cancel()
        try:
            await runner
        except asyncio.CancelledError:
            pass
        return 0

    try:
        return loop.run_until_complete(_main())
    finally:
        loop.close()


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
