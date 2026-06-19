"""Rule matcher (arch §3.3/§4.1, AC-9, AC-33b).

Selects the first enabled rule that matches a request, ordered by ``priority``
(lower first) then ``id`` — deterministic (§5.3 ordering). Matching covers:

  * **method**     — ``ANY`` or an exact (case-insensitive) HTTP verb.
  * **path**       — exact, ``:param`` segments (captured for templating), and a
                     trailing ``/*`` wildcard. Compiled once to a regex.
  * **headers**    — required header name→value (case-insensitive name).
  * **query**      — required query key→value.
  * **body**       — jsonpath-lite ``eq/neq/contains/exists`` conditions.
  * **state**      — ``state_requirements`` (``eq/neq/exists/absent``) — gates the
                     rule on per-endpoint Redis state. **Fail-CLOSED** (AC-9/AC-49):
                     if state is unavailable (Redis down) a state-gated rule is
                     **skipped**, never silently matched.

Path captures (``:name``) are returned so the templating engine can resolve
``{{request.path.<name>}}`` (§5.7).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Pattern, Tuple

from app.models import MatchCriteria
from app.utils.helpers import jsonpath_lite


def compile_path(path: str) -> Pattern[str]:
    """Compile a match path into an anchored regex.

    ``/users/:id`` -> capture group ``id``; trailing ``/*`` -> match any suffix;
    everything else is matched literally. The match path is normalized to start
    with ``/``.
    """
    if not path:
        path = "/*"
    if not path.startswith("/"):
        path = "/" + path

    # Pure catch-all: match any path. (Handle before the trailing-/* strip below.)
    if path in ("/*", "*", "/**"):
        return re.compile(r"^/.*$")

    wildcard = False
    if path.endswith("/*"):
        wildcard = True
        path = path[:-2]  # strip the trailing "/*"

    segments = [s for s in path.split("/") if s != ""]
    parts: List[str] = []
    for seg in segments:
        if seg.startswith(":") and len(seg) > 1:
            name = seg[1:]
            # A capture is a single non-slash segment.
            parts.append(rf"/(?P<{re.escape(name)}>[^/]+)")
        else:
            parts.append("/" + re.escape(seg))

    body = "".join(parts) if parts else "/"
    if wildcard:
        # Prefix followed by anything (a further "/..." suffix) or nothing.
        pattern = rf"^{body}(?:/.*)?$" if body != "/" else r"^/.*$"
    else:
        pattern = rf"^{body}/?$" if body != "/" else r"^/$"
    return re.compile(pattern)


@dataclass
class CompiledRule:
    """A rule compiled for the fast path (precompiled path regex + parsed criteria)."""

    id: int
    priority: int
    enabled: bool
    method: str                       # upper-cased; "ANY" matches all
    path_pattern: Pattern[str]
    raw_path: str
    headers: Dict[str, str]           # lower-cased names
    query: Dict[str, str]
    body_conditions: List[dict]       # [{path, op, value}]
    state_requirements: List[dict]    # [{key, op, value}]
    response: dict                    # ResponseSpec dict
    state_writes: List[dict]          # [{key, value}]
    latency_ms: Optional[int]
    rate_limit_per_min: Optional[int]
    webhook_action: Optional[dict]

    @property
    def gates_on_state(self) -> bool:
        return bool(self.state_requirements)


def compile_rule(rule: dict) -> CompiledRule:
    """Build a :class:`CompiledRule` from a DB-shaped rule dict (already JSON-parsed
    ``match``/``response``/``state_writes``)."""
    match = rule.get("match") or {}
    if isinstance(match, MatchCriteria):
        match = match.model_dump()
    headers = {k.lower(): v for k, v in (match.get("headers") or {}).items()}
    return CompiledRule(
        id=int(rule["id"]),
        priority=int(rule.get("priority", 100)),
        enabled=bool(rule.get("enabled", True)),
        method=str(match.get("method", "ANY")).upper(),
        path_pattern=compile_path(match.get("path", "/*")),
        raw_path=match.get("path", "/*"),
        headers=headers,
        query=dict(match.get("query") or {}),
        body_conditions=list(match.get("body_conditions") or []),
        state_requirements=list(match.get("state_requirements") or []),
        response=dict(rule.get("response") or {}),
        state_writes=list(rule.get("state_writes") or []),
        latency_ms=rule.get("latency_ms"),
        rate_limit_per_min=rule.get("rate_limit_per_min"),
        webhook_action=rule.get("webhook_action"),
    )


# --- individual predicate helpers ---------------------------------------------
def _method_ok(rule: CompiledRule, method: str) -> bool:
    return rule.method == "ANY" or rule.method == method.upper()


def _path_ok(rule: CompiledRule, path: str) -> Optional[Dict[str, str]]:
    m = rule.path_pattern.match(path)
    if m is None:
        return None
    return m.groupdict()


def _headers_ok(rule: CompiledRule, headers: Dict[str, str]) -> bool:
    for name, want in rule.headers.items():
        if headers.get(name.lower()) != want:
            return False
    return True


def _query_ok(rule: CompiledRule, query: Dict[str, str]) -> bool:
    for key, want in rule.query.items():
        if query.get(key) != want:
            return False
    return True


def _body_ok(rule: CompiledRule, body: str) -> bool:
    for cond in rule.body_conditions:
        path = cond.get("path", "")
        op = cond.get("op", "eq")
        want = cond.get("value")
        got = jsonpath_lite(body, path)
        if op == "exists":
            if got is None:
                return False
        elif op == "eq":
            if got != (want or ""):
                return False
        elif op == "neq":
            if got == (want or ""):
                return False
        elif op == "contains":
            if got is None or (want or "") not in got:
                return False
        else:
            return False
    return True


def _state_ok(rule: CompiledRule, state: Dict[str, str]) -> bool:
    """Evaluate state requirements against the (possibly empty) state snapshot.

    When ``state`` is empty because Redis was unavailable, an ``eq``/``contains``
    requirement simply does not hold → the rule is skipped (fail-CLOSED, AC-9/49).
    """
    for req in rule.state_requirements:
        key = req.get("key", "")
        op = req.get("op", "eq")
        want = req.get("value")
        present = key in state
        got = state.get(key)
        if op == "exists":
            if not present:
                return False
        elif op == "absent":
            if present:
                return False
        elif op == "eq":
            if not present or got != (want or ""):
                return False
        elif op == "neq":
            # neq holds if absent or differs.
            if present and got == (want or ""):
                return False
        else:
            return False
    return True


@dataclass
class MatchResult:
    rule: CompiledRule
    path_params: Dict[str, str] = field(default_factory=dict)


def select(
    rules: List[CompiledRule],
    method: str,
    path: str,
    headers: Dict[str, str],
    query: Dict[str, str],
    body: str,
    state: Dict[str, str],
) -> Optional[MatchResult]:
    """Return the first enabled, fully-matching rule (rules are pre-sorted by
    ``priority`` then ``id``), with its captured path params. ``None`` if none match.

    ``state`` is the (lazily-read) per-endpoint snapshot; pass ``{}`` when no rule
    gates on state or when Redis is down (state-gated rules then fail closed).
    """
    for rule in rules:
        if not rule.enabled:
            continue
        if not _method_ok(rule, method):
            continue
        params = _path_ok(rule, path)
        if params is None:
            continue
        if not _headers_ok(rule, headers):
            continue
        if not _query_ok(rule, query):
            continue
        if not _body_ok(rule, body):
            continue
        if rule.gates_on_state and not _state_ok(rule, state):
            continue
        return MatchResult(rule=rule, path_params=params)
    return None
