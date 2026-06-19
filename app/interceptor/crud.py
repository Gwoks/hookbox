"""Auto-CRUD engine over Redis-backed JSON arrays (arch §4.3, §5.5; AC-11/12/12a/12b).

When ``auto_crud`` is enabled and **no rule matches**, the interceptor turns the
endpoint into a zero-config REST DB backend over a Redis list per collection
(``crud:<token>:<collection>``). The frozen REST lifecycle (§5.5 / AC-12):

    POST   /<collection>        -> 201  create (server-generated uuid4 ``id``)
    GET    /<collection>        -> 200  list (the whole array)
    GET    /<collection>/<id>   -> 200 one | 404
    PUT    /<collection>/<id>   -> 200 replace (keeps ``id``) | 404
    PATCH  /<collection>/<id>   -> 200 shallow-merge | 404
    DELETE /<collection>/<id>   -> 204 | 404
    (HEAD mirrors GET; OPTIONS never reaches here — handled by the CORS preflight.)

Validation & bounds (AC-12a / AC-12b / SEC-AC-21/24):
  * ``<collection>`` and ``<id>`` must match ``^[A-Za-z0-9_-]{1,64}$`` (the same
    safe charset as state keys) so a crafted segment cannot inject a Redis
    separator and escape the endpoint namespace; a bad segment is treated as a
    non-CRUD path (``matches`` returns False → falls through to MITM/default).
  * A write body must be a JSON **object**; invalid / empty / non-object / oversize
    (> ``CRUD_MAX_ITEM_BYTES``) bodies are rejected with **400** (never 500).
  * The collection is bounded by ``CRUD_MAX_ITEMS`` (enforced in the Redis facade)
    and per-item bytes by ``CRUD_MAX_ITEM_BYTES``; exceeding either is **400**.

Redis-down (§5.11 / AC-49): every store op raises :class:`RedisUnavailable`, which
the engine maps to **503** (we never fabricate or silently lose data). The id is a
``uuid4`` so concurrent ``POST`` s never collide (AC-12a).

The engine resolution order keeps **rules > CRUD** (a rule matching ``/<collection>``
wins and this module is never consulted for that request), and CRUD is rate-limited
on writes by the engine's ``eff_rate`` token bucket (AC-S19).
"""

from __future__ import annotations

import json
import logging
import re
import uuid
from typing import List, Optional, Tuple

from fastapi.responses import JSONResponse, Response

import config
from app.redis_state import RedisUnavailable, redis_state
from app.utils.helpers import is_safe_key

logger = logging.getLogger("hookbox.crud")

# A CRUD path is ``/<collection>`` or ``/<collection>/<id>``. Anything deeper (or
# with empty/oversize/unsafe segments) is NOT CRUD and falls through to the next
# resolution stage. We validate the charset of each segment with ``is_safe_key``.
_WRITE_METHODS = {"POST", "PUT", "PATCH", "DELETE"}

# crud_cas result sentinels (distinguish not-found / too-large from a returned item).
_NOT_FOUND = object()
_TOO_LARGE = object()
_OK = object()


def _parse_path(mock_path: str) -> Optional[Tuple[str, Optional[str]]]:
    """Parse ``/<collection>[/<id>]`` → ``(collection, id|None)``.

    Returns ``None`` when the path is not a valid 1- or 2-segment CRUD path whose
    segments pass the safe charset (so the caller treats it as a non-CRUD path).
    """
    if not mock_path or not mock_path.startswith("/"):
        return None
    # Drop a single trailing slash so ``/books/`` == ``/books``.
    trimmed = mock_path[:-1] if mock_path.endswith("/") and len(mock_path) > 1 else mock_path
    segs = [s for s in trimmed.split("/") if s != ""]
    if len(segs) == 1:
        coll = segs[0]
        if not is_safe_key(coll):
            return None
        return coll, None
    if len(segs) == 2:
        coll, ident = segs
        if not is_safe_key(coll) or not is_safe_key(ident):
            return None
        return coll, ident
    return None


def matches(mock_path: str) -> bool:
    """True iff ``mock_path`` looks like a CRUD collection/item path (AC-12).

    Used by the engine to decide whether to consult Auto-CRUD before MITM/default.
    A 3+ segment path or an unsafe segment is **not** CRUD (returns False).
    """
    return _parse_path(mock_path) is not None


def _bad_request(detail: str) -> JSONResponse:
    return JSONResponse(status_code=400, content={"error": "bad_request", "detail": detail})


def _not_found() -> JSONResponse:
    return JSONResponse(status_code=404, content={"error": "not_found",
                        "detail": "No such item."})


def _parse_object_body(body_text: str) -> dict:
    """Parse a write body into a JSON object or raise ``ValueError`` (→ 400).

    Enforces the per-item byte cap (AC-12b) before parsing.
    """
    if body_text is None or body_text.strip() == "":
        raise ValueError("empty body")
    if len(body_text.encode("utf-8")) > config.CRUD_MAX_ITEM_BYTES:
        raise ValueError("item too large")
    try:
        obj = json.loads(body_text)
    except (ValueError, TypeError) as exc:
        raise ValueError("body is not valid JSON") from exc
    if not isinstance(obj, dict):
        raise ValueError("body must be a JSON object")
    return obj


async def handle(token: str, method: str, mock_path: str, body_text: str) -> Response:
    """Execute one Auto-CRUD operation (§5.5 / AC-12).

    Raises :class:`RedisUnavailable` on Redis loss so the engine returns 503
    (AC-49). All bad-input paths return a 400/404 JSONResponse (never 500).
    """
    parsed = _parse_path(mock_path)
    if parsed is None:
        # Defensive: the engine only calls us when ``matches`` is True, but never
        # 500 if it changes — treat as no-match → 404.
        return _not_found()
    collection, ident = parsed
    method = method.upper()

    # --- collection-level (no id) --------------------------------------------
    if ident is None:
        if method in ("GET", "HEAD"):
            items = await redis_state.crud_list(token, collection)
            return JSONResponse(status_code=200, content=items)
        if method == "POST":
            try:
                obj = _parse_object_body(body_text)
            except ValueError as exc:
                return _bad_request(str(exc))
            # Server-assigned id wins (collision-safe uuid4, AC-12a). A client id
            # is ignored/overwritten so two POSTs never collide.
            obj["id"] = uuid.uuid4().hex
            try:
                await redis_state.crud_append(token, collection, obj)
            except RedisUnavailable:
                raise
            except ValueError:
                # CRUD_MAX_ITEMS reached (collection_full) — bounded, not 500.
                return _bad_request("collection is full")
            return JSONResponse(status_code=201, content=obj)
        # PUT/PATCH/DELETE on a collection (no id) is not a CRUD item op.
        return _bad_request("an item id is required for this method")

    # --- item-level (collection + id) ----------------------------------------
    # GET/HEAD are read-only; the mutating verbs use an ATOMIC read-modify-write
    # (redis_state.crud_cas, WATCH/MULTI + retry) so two concurrent writers to the
    # same collection can't lose an update (hookbox-65m). Input is parsed/validated
    # (→ 400) BEFORE the transaction; not-found / too-large come back via sentinels.
    if method in ("GET", "HEAD"):
        items = await redis_state.crud_list(token, collection)
        idx = _find_index(items, ident)
        if idx is None:
            return _not_found()
        return JSONResponse(status_code=200, content=items[idx])

    if method == "PUT":
        try:
            obj = _parse_object_body(body_text)
        except ValueError as exc:
            return _bad_request(str(exc))
        obj["id"] = ident  # id is immutable on replace

        def _put(items):
            i = _find_index(items, ident)
            if i is None:
                return _NOT_FOUND, None
            items[i] = dict(obj)
            return items[i], items

        result = await redis_state.crud_cas(token, collection, _put)
        if result is _NOT_FOUND:
            return _not_found()
        return JSONResponse(status_code=200, content=result)

    if method == "PATCH":
        try:
            patch = _parse_object_body(body_text)
        except ValueError as exc:
            return _bad_request(str(exc))

        def _patch(items):
            i = _find_index(items, ident)
            if i is None:
                return _NOT_FOUND, None
            merged = dict(items[i])
            merged.update(patch)
            merged["id"] = ident  # id is immutable on merge
            if len(json.dumps(merged).encode("utf-8")) > config.CRUD_MAX_ITEM_BYTES:
                return _TOO_LARGE, None
            items[i] = merged
            return merged, items

        result = await redis_state.crud_cas(token, collection, _patch)
        if result is _NOT_FOUND:
            return _not_found()
        if result is _TOO_LARGE:
            return _bad_request("item too large")
        return JSONResponse(status_code=200, content=result)

    if method == "DELETE":
        def _delete(items):
            i = _find_index(items, ident)
            if i is None:
                return _NOT_FOUND, None
            del items[i]
            return _OK, items

        result = await redis_state.crud_cas(token, collection, _delete)
        if result is _NOT_FOUND:
            return _not_found()
        return Response(status_code=204)

    return _bad_request("unsupported method for an item")


def _find_index(items: List[dict], ident: str) -> Optional[int]:
    """Return the list index whose element ``id`` == ``ident``, else None."""
    for i, it in enumerate(items):
        if isinstance(it, dict) and str(it.get("id")) == ident:
            return i
    return None
