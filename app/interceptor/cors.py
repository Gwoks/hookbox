"""Auto-CORS engine — the frozen §5.6 header set (AC-18/19, AC-S16/S17).

Every **intercepted (P1 mock-surface) request** gets wide-open, dynamic CORS with
**zero** user configuration:

  * **Preflight** — an ``OPTIONS`` to any mock path (when ``cors_enabled``) returns
    ``204`` with the frozen header set (reflected ``Origin`` / reflected
    ``Access-Control-Request-Headers``, ``Allow-Methods``,
    ``Max-Age: 600``, ``Vary: Origin``) **without** any user-defined rule (AC-18).
  * **Every non-preflight response** (mock / CRUD / MITM / tunnel / default / chaos /
    ratelimit) carries ``Access-Control-Allow-Origin`` (reflected or ``*``),
    ``Access-Control-Expose-Headers: *`` and ``Vary: Origin`` (AC-19).

Two security invariants are baked in (§5.6, RESOLVED OQ-12):

  * **No credentialed wildcard (AC-S17):** ``Access-Control-Allow-Credentials`` is
    **never** emitted. Reflecting the request ``Origin`` (rather than literal ``*``)
    is what makes "wide-open" usable from browsers *without* claiming credential
    support — and per the Fetch spec ``Allow-Origin: *`` + ``Allow-Credentials:
    true`` is invalid, so we never pair them.
  * **P1-only (AC-S16):** these headers are produced **only** here, on the mock
    interception plane (the engine calls this module); the management API (P2,
    ``/api/*``) emits **no** wildcard CORS because nothing on that plane calls this.

When ``cors_enabled`` is False the per-response set is empty and the preflight is a
bare ``204`` (the engine still answers ``OPTIONS`` so a preflight never falls
through to a rule / 404).
"""

from __future__ import annotations

from typing import Dict

from fastapi import Request
from fastapi.responses import Response

# The methods we always advertise as allowed on the mock surface (§5.6, frozen).
ALLOW_METHODS = "GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD"
# Preflight cache lifetime in seconds (§5.6).
MAX_AGE = "600"


def _reflected_origin(request: Request) -> str:
    """Reflect the request ``Origin`` so the response is usable by a credential-less
    browser fetch; fall back to ``*`` when no Origin header is present (AC-S17 — we
    reflect rather than send ``*`` + credentials)."""
    return request.headers.get("origin") or "*"


def response_headers(request: Request, cors_enabled: bool) -> Dict[str, str]:
    """The §5.6 per-response CORS header set for a non-preflight P1 response (AC-19).

    Returns an empty dict when CORS is disabled for the endpoint. Never includes
    ``Access-Control-Allow-Credentials`` (AC-S17).
    """
    if not cors_enabled:
        return {}
    return {
        "Access-Control-Allow-Origin": _reflected_origin(request),
        "Access-Control-Expose-Headers": "*",
        "Vary": "Origin",
    }


def preflight_headers(request: Request, cors_enabled: bool) -> Dict[str, str]:
    """The §5.6 preflight (``OPTIONS``) header set (AC-18).

    Reflects the requested ``Access-Control-Request-Headers`` (or ``*``) so any
    custom header is permitted. Empty when CORS is disabled.
    """
    if not cors_enabled:
        return {}
    req_headers = request.headers.get("access-control-request-headers") or "*"
    return {
        "Access-Control-Allow-Origin": _reflected_origin(request),
        "Access-Control-Allow-Methods": ALLOW_METHODS,
        "Access-Control-Allow-Headers": req_headers,
        "Access-Control-Max-Age": MAX_AGE,
        "Vary": "Origin",
    }


def preflight_response(request: Request, cors_enabled: bool) -> Response:
    """Build the ``204`` preflight response for an ``OPTIONS`` mock request (AC-18).

    Always a ``204`` (even when CORS is disabled, so an ``OPTIONS`` is answered
    deterministically rather than falling through to a rule/404); the
    ``X-HookBox-*`` identifying headers are added by the engine.
    """
    return Response(status_code=204, headers=preflight_headers(request, cors_enabled))
