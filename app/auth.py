"""Owner-capability auth (§5.1) — replaces the insecure ``X-User-Id`` header-trust.

A non-secret ``owner_id`` (= ``hash_email``) plus a **secret** bearer
``owner_secret`` (256-bit CSPRNG, stored sha256-hashed). Every ``/api/*`` route
except ``POST /api/session`` requires ``Authorization: Bearer <owner_secret>``:
  * missing / malformed / unknown secret           -> 401 (AC-S1, AC-S4)
  * valid secret but does not own ``{token}``       -> 404 (AC-S2, AC-S3 — never
    confirm token existence to a non-owner)
"""

from __future__ import annotations

import aiosqlite
from fastapi import Depends, Header, HTTPException, status

from app.database import DATABASE_PATH, get_db, hash_secret


def _unauthorized() -> HTTPException:
    return HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail={"error": "unauthorized", "detail": "Valid owner capability required."},
        headers={"WWW-Authenticate": "Bearer"},
    )


def _parse_bearer(authorization: str | None) -> str:
    if not authorization:
        raise _unauthorized()
    parts = authorization.split(" ", 1)
    if len(parts) != 2 or parts[0].lower() != "bearer" or not parts[1].strip():
        raise _unauthorized()
    return parts[1].strip()


async def require_owner(
    authorization: str | None = Header(default=None),
    db: aiosqlite.Connection = Depends(get_db),
) -> str:
    """Resolve and return the authenticated ``owner_id`` from the bearer secret.

    The public ``owner_id`` is **never** accepted as a credential — only the
    hashed secret is looked up. Returns the owner_id string.
    """
    secret = _parse_bearer(authorization)
    secret_hash = hash_secret(secret)
    cur = await db.execute(
        "SELECT owner_id FROM owners WHERE secret_hash = ?", (secret_hash,)
    )
    row = await cur.fetchone()
    if row is None:
        raise _unauthorized()
    return row[0]


async def verify_cap_owns_token(token: str, secret: str | None) -> bool:
    """Return True iff ``secret`` is a valid owner capability that owns ``token``.

    Used by the **WebSocket / SSE feed gate** (§5.4, OQ-4) and the **tunnel bind
    handshake** (§5.12, OQ-5), which run *before* FastAPI dependency injection and
    therefore cannot use :func:`require_owner`. Opens its own short-lived
    connection. Constant-ish: a missing/blank secret returns False without a DB
    hit; otherwise one indexed lookup by ``secret_hash`` then an owner-match on the
    endpoint row. Never raises (a DB hiccup → False, i.e. deny).
    """
    if not secret or not token:
        return False
    try:
        secret_hash = hash_secret(secret)
        async with aiosqlite.connect(DATABASE_PATH) as db:
            db.row_factory = aiosqlite.Row
            cur = await db.execute(
                "SELECT owner_id FROM owners WHERE secret_hash = ?", (secret_hash,)
            )
            owner = await cur.fetchone()
            if owner is None:
                return False
            cur = await db.execute(
                "SELECT owner_id FROM endpoints WHERE token = ?", (token,)
            )
            ep = await cur.fetchone()
            if ep is None:
                return False
            return ep["owner_id"] == owner["owner_id"]
    except Exception:  # noqa: BLE001 - deny on any error
        return False


async def assert_owns_endpoint(
    token: str, owner_id: str, db: aiosqlite.Connection
) -> aiosqlite.Row:
    """Return the endpoint row iff ``owner_id`` owns ``token``; else 404.

    404 (not 403) so a non-owner cannot distinguish "exists but not mine" from
    "does not exist" (AC-S2/S3).
    """
    cur = await db.execute("SELECT * FROM endpoints WHERE token = ?", (token,))
    row = await cur.fetchone()
    if row is None or row["owner_id"] != owner_id:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"error": "not_found", "detail": "Endpoint not found."},
        )
    return row
