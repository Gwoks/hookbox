"""Unit tests for the rule matcher (§3.3/§4.1, AC-9/33b) — incl. ReDoS safety."""
from app.interceptor.matcher import compile_path, compile_rule, select


def test_exact_path():
    p = compile_path("/hello")
    assert p.match("/hello") is not None
    assert p.match("/hello/x") is None


def test_param_capture():
    p = compile_path("/users/:id")
    m = p.match("/users/42")
    assert m and m.groupdict() == {"id": "42"}
    assert p.match("/users/42/posts") is None


def test_trailing_wildcard():
    p = compile_path("/api/*")
    assert p.match("/api/anything/deep") is not None
    assert p.match("/other") is None


def test_user_regex_metachars_are_escaped_no_redos():
    # A user path full of regex metacharacters must match LITERALLY (not as a
    # regex), so a crafted pattern cannot cause catastrophic backtracking.
    p = compile_path("/a.(b)+[c]")
    assert p.match("/a.(b)+[c]") is not None
    assert p.match("/aXbXc") is None        # '.' did not act as a wildcard


def _rule(rid, prio, **match):
    return compile_rule({"id": rid, "priority": prio, "enabled": True,
                         "match": match, "response": {}})


def test_select_precedence_priority_then_id():
    lo = _rule(2, 1, path="/x")
    hi = _rule(1, 9, path="/x")
    sel = select([lo, hi], "GET", "/x", {}, {}, "", {})
    assert sel and sel.rule.id == 2     # lower priority number wins


def test_disabled_rule_skipped():
    r = compile_rule({"id": 1, "priority": 1, "enabled": False,
                      "match": {"path": "/x"}, "response": {}})
    assert select([r], "GET", "/x", {}, {}, "", {}) is None


def test_state_gated_fails_closed_when_state_absent():
    r = _rule(1, 1, path="/s", state_requirements=[{"key": "auth", "op": "eq", "value": "true"}])
    assert select([r], "GET", "/s", {}, {}, "", {}) is None              # no state -> no match
    assert select([r], "GET", "/s", {}, {}, "", {"auth": "true"}) is not None
