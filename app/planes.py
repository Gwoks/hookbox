"""Pure 3-plane resolution logic (arch §3.1/§3.2, AC-4/5/6/6a).

Decides which of the three hard-isolated request planes a request belongs to,
purely from ``Host`` + ``path`` — **no DB, no ASGI** — so it is trivially
unit-testable. ``PlaneDispatchMiddleware`` (``app/middleware.py``) applies it.

Planes:
  * ``mock``  (P1) — the public wildcard mock surface ``<token>.<MOCK_DOMAIN>/<path>``
                     and the localhost path-fallback ``/e/<token>/<path>``. On a
                     mock host **everything** (incl. ``/api`` and ``/static``) is the
                     mock's own path — management is unreachable there by construction.
  * ``api``   (P2) — the management REST API ``/api/*`` (app host only).
  * ``ui``    (P3) — the dashboard UI + static + WS feed ``/``, ``/d/*``,
                     ``/static/*``, ``/ws/*``, ``/sse/*``, ``/healthz`` (app host).

The bare apex (``https://<MOCK_DOMAIN>/`` with no subdomain), ``localhost``,
``127.0.0.1``, ``[::1]`` and ``<APP_HOST>`` all resolve to the UI plane (AC-6a).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional, Tuple

import config

# Path prefixes that, on the **app host**, belong to the management/UI planes and
# must never be served by the mock catch-all (AC-6).
_API_PREFIX = "/api"
_UI_EXACT = {"/", "/healthz", "/favicon.ico", "/robots.txt"}
_UI_PREFIXES = ("/static/", "/static", "/d/", "/d", "/ws/", "/ws", "/sse/", "/sse")


@dataclass(frozen=True)
class PlaneResult:
    plane: str                      # "mock" | "api" | "ui"
    token: Optional[str] = None     # endpoint token when plane == "mock"
    mock_path: Optional[str] = None  # the mock path handed to the engine (P1)


def _strip_port(host: str) -> str:
    """Drop a trailing ``:port`` (and brackets for bare IPv6 literals are kept)."""
    if not host:
        return ""
    host = host.strip()
    # IPv6 literal in brackets, optionally with a port: [::1]:8000
    if host.startswith("["):
        end = host.find("]")
        if end != -1:
            return host[: end + 1].lower()
        return host.lower()
    # host:port -> host (only split the LAST colon to be safe for bare names)
    if ":" in host:
        # A bare IPv6 without brackets would contain multiple colons; such a value
        # is not a valid Host header for our purposes, so only strip a single
        # trailing :digits port.
        head, _, tail = host.rpartition(":")
        if tail.isdigit():
            return head.lower()
    return host.lower()


def _strip_port_preserve_case(host: str) -> str:
    """Like ``_strip_port`` but does NOT lower-case — used to recover the
    case-sensitive endpoint token from a subdomain label (tokens use a mixed-case
    alphabet; DNS treats the label case-insensitively but our token does not, so
    we must preserve the label's original case)."""
    if not host:
        return ""
    host = host.strip()
    if host.startswith("["):
        end = host.find("]")
        return host[: end + 1] if end != -1 else host
    if ":" in host:
        head, _, tail = host.rpartition(":")
        if tail.isdigit():
            return head
    return host


def subdomain_of(host: str, mock_domain: str) -> Optional[str]:
    """Return the case-preserved ``<token>`` iff ``host == <token>.<mock_domain>``
    (single label), matching the domain suffix **case-insensitively**.

    Returns ``None`` for the bare apex, the app host, or any non-matching host.
    A multi-label subdomain (``a.b.<domain>``) is **not** a valid single token
    and returns ``None`` (it falls through to the UI plane).

    The token label's **case is preserved** because endpoint tokens use a
    mixed-case alphabet (``gen_token``); lower-casing it would break subdomain
    addressing (AC-4) vs the case-sensitive path-fallback (AC-5).
    """
    if not host or not mock_domain or "." not in mock_domain:
        return None
    raw = _strip_port_preserve_case(host)
    md = mock_domain.lower().strip()
    suffix = "." + md
    if not raw.lower().endswith(suffix):
        return None
    label = raw[: len(raw) - len(suffix)]
    if not label or "." in label:
        return None  # apex (empty) or multi-label -> not a mock token
    return label


def _path_fallback_token(path: str) -> Tuple[Optional[str], Optional[str]]:
    """Parse ``/e/<token>/<rest>`` → ``(token, mock_path="/<rest>")`` (arch §3.2).

    ``/e/<token>`` (no trailing path) → ``(token, "/")``. ``/e`` alone → no token.
    """
    # path is the URL path, already starting with "/".
    parts = path.split("/", 3)  # ['', 'e', '<token>', '<rest...>']
    if len(parts) < 3 or parts[1] != "e":
        return None, None
    token = parts[2]
    if not token:
        return None, None
    rest = parts[3] if len(parts) > 3 else ""
    mock_path = "/" + rest
    return token, mock_path


def resolve_plane(host: str, path: str) -> PlaneResult:
    """Resolve the plane for ``(host, path)`` (arch §3.1).

    Order matters: a wildcard mock host wins first (everything on it is mock),
    then the localhost path-fallback, then ``/api`` (management), then UI.
    """
    # P1a — wildcard mock host: EVERYTHING here is the mock's own path.
    sub = subdomain_of(host, config.MOCK_DOMAIN)
    if sub is not None and sub not in config.APP_HOSTS:
        return PlaneResult(plane="mock", token=sub, mock_path=path or "/")

    # P1b — localhost path-fallback /e/<token>/<rest> (any host).
    if path == "/e" or path == "/e/" or path.startswith("/e/"):
        token, mock_path = _path_fallback_token(path)
        if token is not None:
            return PlaneResult(plane="mock", token=token, mock_path=mock_path)
        # Malformed /e with no token -> UI 404 (not a mock).
        return PlaneResult(plane="ui")

    # From here we are on an app host (or the bare apex / unknown host → UI).
    # P2 — management API.
    if path == _API_PREFIX or path.startswith(_API_PREFIX + "/"):
        return PlaneResult(plane="api")

    # P3 — UI / static / ws / sse / health.
    if path in _UI_EXACT:
        return PlaneResult(plane="ui")
    for pref in _UI_PREFIXES:
        if path == pref or path.startswith(pref + "/") or path.startswith(pref):
            return PlaneResult(plane="ui")

    # Default app-host fallthrough → UI (the UI router returns 404 inside).
    return PlaneResult(plane="ui")
