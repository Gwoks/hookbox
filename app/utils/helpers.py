"""Utility functions for HookBox.

Keeps the verified prior-art helpers ``generate_endpoint_id`` (ambiguity-stripped
alphabet, AC-6a) and ``format_headers`` (hop-by-hop strip, reused by the MITM
forward, AC-S9). Adds: a safe-key validator (AC-10a / SEC-AC-24), a jsonpath-lite
getter (templating + body matching, §5.7), and a deterministic RNG seam for chaos
tests (arch §9 R6).
"""

from __future__ import annotations

import json
import re
import secrets
import string
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

from config import ENDPOINT_ID_LENGTH

# Default expiry retained for backwards-compatible callers; not central anymore.
DEFAULT_EXPIRY_HOURS = 24

# Sensitive inbound headers that must never be forwarded upstream (AC-S9) — on top
# of the hop-by-hop set. The owner capability is carried on ``Authorization`` to
# the management plane only; it is dropped here so it never reaches an MITM target.
_SENSITIVE_FORWARD_HEADERS = {
    "authorization", "cookie", "x-owner-id", "x-user-id", "x-hookbox-cap",
}

_HOP_BY_HOP = {
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailers", "transfer-encoding", "upgrade", "host",
}

# Safe charset for user-supplied state keys, CRUD collection names, and :ids
# (AC-10a, AC-12a, SEC-AC-24) — blocks Redis separators so a crafted key cannot
# escape its endpoint namespace.
SAFE_KEY_RE = re.compile(r"^[A-Za-z0-9_-]{1,64}$")


def generate_endpoint_id(length: int = ENDPOINT_ID_LENGTH) -> str:
    """Ambiguity-stripped endpoint token (kept from prior art, AC-6a)."""
    alphabet = string.ascii_lowercase + string.ascii_uppercase + string.digits
    alphabet = alphabet.replace("0", "").replace("O", "")
    alphabet = alphabet.replace("1", "").replace("l", "").replace("I", "")
    return "".join(secrets.choice(alphabet) for _ in range(length))


def calculate_expiry(expires_in_hours: int = DEFAULT_EXPIRY_HOURS) -> datetime:
    return datetime.utcnow() + timedelta(hours=expires_in_hours)


def format_headers(headers) -> Dict[str, str]:
    """Strip hop-by-hop headers (kept from prior art; reused by MITM, AC-S9)."""
    return {
        (k.decode() if isinstance(k, bytes) else k):
        (v.decode() if isinstance(v, bytes) else v)
        for k, v in headers.items()
        if (k.decode() if isinstance(k, bytes) else k).lower() not in _HOP_BY_HOP
    }


def strip_forward_headers(headers: Dict[str, str]) -> Dict[str, str]:
    """Headers safe to forward to an MITM upstream: hop-by-hop + sensitive removed
    (AC-S9). The owner capability is never forwarded."""
    out: Dict[str, str] = {}
    for k, v in headers.items():
        kl = k.lower()
        if kl in _HOP_BY_HOP or kl in _SENSITIVE_FORWARD_HEADERS:
            continue
        out[k] = v
    return out


def is_safe_key(key: str) -> bool:
    """True iff ``key`` matches the safe charset (AC-10a / SEC-AC-24)."""
    return bool(isinstance(key, str) and SAFE_KEY_RE.match(key))


def jsonpath_lite(body: Any, path: str) -> Optional[str]:
    """Dot-path getter into a JSON body (``a.b.0.c``), §5.7 / AC-21.

    ``body`` may be a raw JSON string or an already-parsed object. Returns the
    value as a string, or ``None`` on any miss / non-JSON / out-of-range index.
    Never raises.
    """
    if body is None or path == "":
        return None
    obj: Any
    if isinstance(body, (dict, list)):
        obj = body
    elif isinstance(body, (str, bytes)):
        try:
            obj = json.loads(body)
        except (ValueError, TypeError):
            return None
    else:
        return None

    cur = obj
    for seg in path.split("."):
        if isinstance(cur, dict):
            if seg not in cur:
                return None
            cur = cur[seg]
        elif isinstance(cur, list):
            if not seg.lstrip("-").isdigit():
                return None
            idx = int(seg)
            if idx < 0 or idx >= len(cur):
                return None
            cur = cur[idx]
        else:
            return None

    if cur is None:
        return ""
    if isinstance(cur, bool):
        return "true" if cur else "false"
    if isinstance(cur, (dict, list)):
        return json.dumps(cur)
    return str(cur)
