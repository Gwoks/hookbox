//! Rule matcher — PORT of `app/interceptor/matcher.py` (AC-11, AC-13, §5.3/§5.5).
//!
//! Selects the first **enabled** rule that fully matches a request, with rules
//! pre-sorted by `priority` (lower first) then `id` — deterministic ordering.
//! Matching covers method / path (exact · `:param` · trailing `/*`) / required
//! headers (case-insensitive name) / required query / `body_conditions`
//! (jsonpath-lite `eq|neq|contains|exists`) / `state_requirements`
//! (`eq|neq|exists|absent`).
//!
//! Path patterns are compiled into a segment matcher (no regex dependency) so a
//! crafted pattern cannot cause regex blowup. `:name` segments are captured for
//! `{{request.path.<name>}}` templating (§5.7).
//!
//! State-gated rules **fail closed** (AC-11/AC-57): when `state` is empty because
//! the store was unreadable, an `eq`/`contains`/`exists` requirement does not
//! hold → the rule is skipped, never silently matched.

use std::collections::BTreeMap;

use crate::helpers::jsonpath_lite;
use crate::models::{
    BodyCondition, MatchCriteria, ResponseSpec, StateRequirement, StateWrite, WebhookAction,
};

/// A compiled path: a sequence of segment matchers plus a trailing-wildcard flag.
#[derive(Debug, Clone)]
pub struct CompiledPath {
    segments: Vec<Segment>,
    /// Trailing `/*` — match this prefix followed by anything (or nothing).
    wildcard: bool,
    /// Pure catch-all (`/*`, `*`, `/**`) — matches any path.
    catch_all: bool,
}

#[derive(Debug, Clone)]
enum Segment {
    Literal(String),
    /// `:name` — captures one non-empty, non-slash segment under `name`.
    Param(String),
}

/// Compile a match path (`/users/:id`, `/v1/*`, `/exact`) into a `CompiledPath`.
/// Mirrors `matcher.py::compile_path` semantics.
pub fn compile_path(path: &str) -> CompiledPath {
    let mut p = if path.is_empty() {
        "/*".to_string()
    } else {
        path.to_string()
    };
    if !p.starts_with('/') {
        p = format!("/{p}");
    }
    if p == "/*" || p == "*" || p == "/**" {
        return CompiledPath {
            segments: Vec::new(),
            wildcard: false,
            catch_all: true,
        };
    }
    let mut wildcard = false;
    if let Some(stripped) = p.strip_suffix("/*") {
        wildcard = true;
        p = stripped.to_string();
    }
    let segments: Vec<Segment> = p
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            if seg.len() > 1 && seg.starts_with(':') {
                Segment::Param(seg[1..].to_string())
            } else {
                Segment::Literal(seg.to_string())
            }
        })
        .collect();
    CompiledPath {
        segments,
        wildcard,
        catch_all: false,
    }
}

impl CompiledPath {
    /// Match `path` against this compiled pattern; return captured params
    /// (`:name` -> value) on a match, `None` otherwise.
    pub fn match_path(&self, path: &str) -> Option<BTreeMap<String, String>> {
        if self.catch_all {
            return Some(BTreeMap::new());
        }
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let n = self.segments.len();
        if self.wildcard {
            // Prefix must match; anything (incl. nothing) may follow.
            if parts.len() < n {
                return None;
            }
        } else if parts.len() != n {
            return None;
        }
        let mut params = BTreeMap::new();
        for (i, seg) in self.segments.iter().enumerate() {
            match seg {
                Segment::Literal(lit) => {
                    if parts[i] != lit {
                        return None;
                    }
                }
                Segment::Param(name) => {
                    params.insert(name.clone(), parts[i].to_string());
                }
            }
        }
        Some(params)
    }
}

/// A rule compiled for the fast path (parsed criteria + precompiled path).
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub id: i64,
    pub priority: i64,
    pub enabled: bool,
    pub method: String, // upper-cased; "ANY" matches all
    pub path: CompiledPath,
    pub headers: BTreeMap<String, String>, // lower-cased names
    pub query: BTreeMap<String, String>,
    pub body_conditions: Vec<BodyCondition>,
    pub state_requirements: Vec<StateRequirement>,
    pub response: ResponseSpec,
    pub state_writes: Vec<StateWrite>,
    pub latency_ms: Option<i64>,
    pub rate_limit_per_min: Option<i64>,
    pub chaos_mode: Option<String>,
    pub webhook_action: Option<WebhookAction>,
}

impl CompiledRule {
    pub fn gates_on_state(&self) -> bool {
        !self.state_requirements.is_empty()
    }
}

/// Build a `CompiledRule` from already-parsed DB fields.
#[allow(clippy::too_many_arguments)]
pub fn compile_rule(
    id: i64,
    priority: i64,
    enabled: bool,
    match_: &MatchCriteria,
    response: ResponseSpec,
    state_writes: Vec<StateWrite>,
    latency_ms: Option<i64>,
    rate_limit_per_min: Option<i64>,
    chaos_mode: Option<String>,
    webhook_action: Option<WebhookAction>,
) -> CompiledRule {
    let headers = match_
        .headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    CompiledRule {
        id,
        priority,
        enabled,
        method: match_.method.to_ascii_uppercase(),
        path: compile_path(&match_.path),
        headers,
        query: match_.query.clone(),
        body_conditions: match_.body_conditions.clone(),
        state_requirements: match_.state_requirements.clone(),
        response,
        state_writes,
        latency_ms,
        rate_limit_per_min,
        chaos_mode,
        webhook_action,
    }
}

fn method_ok(rule: &CompiledRule, method: &str) -> bool {
    rule.method == "ANY" || rule.method == method.to_ascii_uppercase()
}

fn headers_ok(rule: &CompiledRule, headers: &BTreeMap<String, String>) -> bool {
    rule.headers
        .iter()
        .all(|(name, want)| headers.get(name) == Some(want))
}

fn query_ok(rule: &CompiledRule, query: &BTreeMap<String, String>) -> bool {
    rule.query
        .iter()
        .all(|(key, want)| query.get(key) == Some(want))
}

fn body_ok(rule: &CompiledRule, body: &str) -> bool {
    for cond in &rule.body_conditions {
        let got = jsonpath_lite(body, &cond.path);
        let want = cond.value.clone().unwrap_or_default();
        let ok = match cond.op.as_str() {
            "exists" => got.is_some(),
            "eq" => got.as_deref() == Some(want.as_str()),
            "neq" => got.as_deref() != Some(want.as_str()),
            "contains" => got.as_deref().map(|g| g.contains(&want)).unwrap_or(false),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

fn state_ok(rule: &CompiledRule, state: &BTreeMap<String, String>) -> bool {
    for req in &rule.state_requirements {
        let present = state.contains_key(&req.key);
        let got = state.get(&req.key);
        let want = req.value.clone().unwrap_or_default();
        let ok = match req.op.as_str() {
            "exists" => present,
            "absent" => !present,
            "eq" => present && got == Some(&want),
            // neq holds if absent OR differs.
            "neq" => !present || got != Some(&want),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// A matched rule plus its captured path params.
pub struct MatchResult<'a> {
    pub rule: &'a CompiledRule,
    pub path_params: BTreeMap<String, String>,
}

/// Return the first enabled, fully-matching rule (rules are pre-sorted by
/// `priority` then `id`) with its captured path params. `None` if none match.
pub fn select<'a>(
    rules: &'a [CompiledRule],
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    query: &BTreeMap<String, String>,
    body: &str,
    state: &BTreeMap<String, String>,
) -> Option<MatchResult<'a>> {
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if !method_ok(rule, method) {
            continue;
        }
        let params = match rule.path.match_path(path) {
            Some(p) => p,
            None => continue,
        };
        if !headers_ok(rule, headers) {
            continue;
        }
        if !query_ok(rule, query) {
            continue;
        }
        if !body_ok(rule, body) {
            continue;
        }
        if rule.gates_on_state() && !state_ok(rule, state) {
            continue;
        }
        return Some(MatchResult {
            rule,
            path_params: params,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, priority: i64, method: &str, path: &str) -> CompiledRule {
        let mc = MatchCriteria {
            method: method.into(),
            path: path.into(),
            ..Default::default()
        };
        compile_rule(
            id,
            priority,
            true,
            &mc,
            ResponseSpec::default(),
            vec![],
            None,
            None,
            None,
            None,
        )
    }

    fn empty() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn path_exact_param_and_wildcard() {
        assert!(compile_path("/users").match_path("/users").is_some());
        assert!(compile_path("/users").match_path("/users/1").is_none());
        let p = compile_path("/users/:id");
        let caps = p.match_path("/users/42").unwrap();
        assert_eq!(caps.get("id").map(String::as_str), Some("42"));
        assert!(p.match_path("/users").is_none());
        let w = compile_path("/v1/*");
        assert!(w.match_path("/v1").is_some());
        assert!(w.match_path("/v1/a/b/c").is_some());
        assert!(w.match_path("/v2/a").is_none());
        assert!(compile_path("/*").match_path("/anything/here").is_some());
    }

    #[test]
    fn order_is_priority_then_id() {
        let rules = vec![
            rule(2, 50, "ANY", "/*"),
            rule(1, 10, "ANY", "/*"),
            rule(3, 10, "ANY", "/*"),
        ];
        // caller pre-sorts; emulate priority,id order.
        let mut sorted = rules;
        sorted.sort_by_key(|r| (r.priority, r.id));
        let m = select(&sorted, "GET", "/x", &empty(), &empty(), "", &empty()).unwrap();
        assert_eq!(m.rule.id, 1); // priority 10, lowest id
    }

    #[test]
    fn disabled_and_method_filter() {
        let mut r = rule(1, 10, "POST", "/*");
        r.enabled = false;
        assert!(select(&[r], "POST", "/x", &empty(), &empty(), "", &empty()).is_none());
        let r2 = rule(1, 10, "POST", "/*");
        assert!(select(
            std::slice::from_ref(&r2),
            "GET",
            "/x",
            &empty(),
            &empty(),
            "",
            &empty()
        )
        .is_none());
        assert!(select(&[r2], "post", "/x", &empty(), &empty(), "", &empty()).is_some());
    }

    #[test]
    fn body_and_state_conditions() {
        let mut r = rule(1, 10, "ANY", "/*");
        r.body_conditions = vec![BodyCondition {
            path: "kind".into(),
            op: "eq".into(),
            value: Some("vip".into()),
        }];
        assert!(select(
            &[r.clone()],
            "POST",
            "/x",
            &empty(),
            &empty(),
            r#"{"kind":"vip"}"#,
            &empty()
        )
        .is_some());
        assert!(select(
            &[r],
            "POST",
            "/x",
            &empty(),
            &empty(),
            r#"{"kind":"std"}"#,
            &empty()
        )
        .is_none());

        let mut sr = rule(2, 10, "ANY", "/*");
        sr.state_requirements = vec![StateRequirement {
            key: "logged_in".into(),
            op: "eq".into(),
            value: Some("1".into()),
        }];
        // fail-closed: empty state -> skip.
        assert!(select(&[sr.clone()], "GET", "/x", &empty(), &empty(), "", &empty()).is_none());
        let mut state = BTreeMap::new();
        state.insert("logged_in".to_string(), "1".to_string());
        assert!(select(&[sr], "GET", "/x", &empty(), &empty(), "", &state).is_some());
    }
}
