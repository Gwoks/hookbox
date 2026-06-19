"""Sandboxed response-templating engine (§5.7, AC-20..23, AC-S10/S11).

A **hand-written single-pass scanner** over the exact §5.7 allow-list. There is
**no** ``eval``/``exec``, **no** Jinja ``render_template_string``/``Template().render``,
and **no** ``str.format`` over attacker-controlled text. The scanner finds each
``{{ ... }}`` span, parses the inner expression against a fixed grammar, and
substitutes a *string* result. Anything not on the allow-list is **left literal**
(the raw ``{{...}}`` stays) — it never 500s and never leaks server internals.

SSTI is structurally impossible here: the inner expression is tokenized and matched
against a closed set of tag handlers. ``{{ 7*7 }}``, ``{{ config }}``,
``{{ ''.__class__.__mro__ }}``, ``{{ self }}`` are all *unknown tags* → returned
verbatim, executing zero Python (AC-S10).

DoS bounds (AC-S11): templates longer than ``TEMPLATE_MAX_SIZE`` are returned
unrendered; at most ``TEMPLATE_MAX_TAGS`` substitutions are performed (further tags
are left literal). Failures are swallowed → the offending tag is left literal.
"""

from __future__ import annotations

import logging
import secrets
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Dict, List, Optional

from config import TEMPLATE_MAX_SIZE, TEMPLATE_MAX_TAGS
from app.utils.helpers import jsonpath_lite

logger = logging.getLogger("hookbox.templating")

_OPEN = "{{"
_CLOSE = "}}"


@dataclass
class TemplateContext:
    """Everything a template tag may read. All values are already-extracted
    plain Python — the engine performs **no** attribute access on user objects.
    """

    method: str = ""
    path: str = ""                                  # mock path, e.g. /users/5
    query: Dict[str, str] = field(default_factory=dict)
    headers: Dict[str, str] = field(default_factory=dict)   # lower-cased names
    path_params: Dict[str, str] = field(default_factory=dict)  # from :name segments
    body: str = ""                                  # raw request body string
    state: Dict[str, str] = field(default_factory=dict)
    rng: Any = secrets                              # injectable RNG seam (tests)

    def header(self, name: str) -> str:
        return self.headers.get(name.lower(), "")


# --- argument tokenizer (single quotes + bare numeric tokens) -----------------
def _tokenize_args(s: str) -> Optional[List[str]]:
    """Split the part after the verb into args. Single-quoted literals and bare
    (non-space) tokens are supported. Returns None on a malformed quote so the
    caller leaves the whole tag literal.
    """
    args: List[str] = []
    i, n = 0, len(s)
    while i < n:
        c = s[i]
        if c.isspace():
            i += 1
            continue
        if c == "'":
            j = s.find("'", i + 1)
            if j == -1:
                return None  # unterminated quote -> malformed
            args.append(s[i + 1:j])
            i = j + 1
        else:
            j = i
            while j < n and not s[j].isspace():
                j += 1
            args.append(s[i:j])
            i = j
    return args


# --- individual tag handlers --------------------------------------------------
def _t_now(ctx: TemplateContext, args: List[str]) -> Optional[str]:
    fmt = args[0] if args else "iso"
    now = datetime.now(timezone.utc)
    if fmt == "iso":
        return now.isoformat()
    if fmt == "unix":
        return str(int(now.timestamp()))
    if fmt == "epoch_ms":
        return str(int(now.timestamp() * 1000))
    return None  # unknown format arg -> literal


def _t_random(ctx: TemplateContext, args: List[str]) -> Optional[str]:
    if not args:
        return None
    kind = args[0]
    if kind == "uuid":
        return str(uuid.uuid4())
    if kind == "int":
        if len(args) != 3:
            return None
        try:
            lo, hi = int(args[1]), int(args[2])
        except (TypeError, ValueError):
            return None
        if lo > hi:
            lo, hi = hi, lo
        span = hi - lo + 1
        if span <= 0:
            return None
        # Use secrets-backed selection by default; ctx.rng may inject a seeded RNG.
        if hasattr(ctx.rng, "randint"):
            return str(ctx.rng.randint(lo, hi))
        return str(lo + secrets.randbelow(span))
    if kind == "hex":
        if len(args) != 2:
            return None
        try:
            length = int(args[1])
        except (TypeError, ValueError):
            return None
        if length <= 0 or length > 4096:
            return None
        nbytes = (length + 1) // 2
        return secrets.token_hex(nbytes)[:length]
    return None


def _t_request(ctx: TemplateContext, expr: str) -> Optional[str]:
    """Handle the ``request.*`` family. ``expr`` is the full inner expression,
    e.g. ``request.query.id`` or ``request.body.user.name``."""
    rest = expr[len("request."):]
    if rest == "method":
        return ctx.method
    if rest == "path":
        return ctx.path
    if rest == "body":
        return ctx.body
    if rest.startswith("query."):
        return ctx.query.get(rest[len("query."):], "")
    if rest.startswith("path."):
        return ctx.path_params.get(rest[len("path."):], "")
    if rest.startswith("header."):
        return ctx.header(rest[len("header."):])
    if rest.startswith("body."):
        val = jsonpath_lite(ctx.body, rest[len("body."):])
        return val if val is not None else ""
    return None


def _t_state(ctx: TemplateContext, expr: str) -> Optional[str]:
    key = expr[len("state."):]
    if not key:
        return None
    return ctx.state.get(key, "")


def _resolve_tag(inner: str, ctx: TemplateContext) -> Optional[str]:
    """Resolve one tag's inner text (already stripped of ``{{``/``}}``).

    Returns the substituted string, or ``None`` if the tag is unknown/malformed
    (the caller then leaves the raw ``{{...}}`` literal). Never raises.
    """
    expr = inner.strip()
    if not expr:
        return None

    # request.* / state.* are dotted families (no whitespace verb form).
    if expr == "request.method" or expr.startswith("request."):
        return _t_request(ctx, expr.split()[0]) if " " not in expr else None
    if expr.startswith("state."):
        return _t_state(ctx, expr) if " " not in expr else None

    # verb-style tags: "now", "now 'iso'", "random 'uuid'", "random 'int' 1 9"
    parts = expr.split(None, 1)
    verb = parts[0]
    arg_str = parts[1] if len(parts) > 1 else ""

    if verb == "now":
        args = _tokenize_args(arg_str)
        return _t_now(ctx, args) if args is not None else None
    if verb == "random":
        args = _tokenize_args(arg_str)
        return _t_random(ctx, args) if args is not None else None

    # Bare {{request}} / {{state}} (no member) are not valid tags -> literal.
    return None


def render(template: str, ctx: TemplateContext) -> str:
    """Render ``template`` against ``ctx`` with the §5.7 sandboxed grammar.

    Single pass, left-to-right. Unknown/malformed tags are left literal. Bounded
    by ``TEMPLATE_MAX_SIZE`` (over-size → returned unrendered) and
    ``TEMPLATE_MAX_TAGS`` (further tags left literal). Never raises.
    """
    if not template:
        return template or ""
    if len(template) > TEMPLATE_MAX_SIZE:
        logger.warning("template exceeds TEMPLATE_MAX_SIZE (%d) -> unrendered", TEMPLATE_MAX_SIZE)
        return template

    out: List[str] = []
    i = 0
    n = len(template)
    tags_rendered = 0

    while i < n:
        start = template.find(_OPEN, i)
        if start == -1:
            out.append(template[i:])
            break
        # Emit everything up to the opening braces.
        out.append(template[i:start])
        end = template.find(_CLOSE, start + len(_OPEN))
        if end == -1:
            # No closing braces — the rest is literal.
            out.append(template[start:])
            break

        inner = template[start + len(_OPEN):end]
        raw = template[start:end + len(_CLOSE)]

        if tags_rendered >= TEMPLATE_MAX_TAGS:
            # DoS bound hit: leave this (and following) tags literal.
            out.append(raw)
            i = end + len(_CLOSE)
            continue

        # A nested "{{" inside the span means this isn't a clean tag; emit the
        # opening braces literally and resume scanning just after them so we don't
        # accidentally swallow a following valid tag.
        nested = inner.find(_OPEN)
        if nested != -1:
            out.append(_OPEN)
            i = start + len(_OPEN)
            continue

        try:
            resolved = _resolve_tag(inner, ctx)
        except Exception:  # noqa: BLE001 - fail safe & quiet (AC-S11)
            logger.debug("template tag raised; leaving literal: %r", inner)
            resolved = None

        if resolved is None:
            out.append(raw)            # unknown/malformed -> literal (AC-23)
        else:
            out.append(resolved)
            tags_rendered += 1
        i = end + len(_CLOSE)

    return "".join(out)


def render_safe(template: str, ctx: TemplateContext) -> str:
    """Belt-and-suspenders wrapper: never raises, never leaks a stack trace into
    a response body (AC-S11/AC-S25). On any unexpected failure returns the raw
    template unchanged."""
    try:
        return render(template, ctx)
    except Exception:  # noqa: BLE001
        logger.exception("templating failed (swallowed); returning raw template")
        return template
