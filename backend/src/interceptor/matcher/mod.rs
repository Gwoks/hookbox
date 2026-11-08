//! Rule matcher — PORT of `app/interceptor/matcher.py` (AC-11, AC-13, §5.3/§5.5).
//!
//! Selects the first **enabled** rule that fully matches a request, with rules
//! pre-sorted by `priority` (lower first) then `id` — deterministic ordering.
//! Matching covers method / path (exact · `:param` · trailing `/*`, see
//! `path`) / required headers (case-insensitive name) / required query /
//! `body_conditions` and `state_requirements` (see `predicates`).
//!
//! State-gated rules **fail closed** (AC-11/AC-57): when `state` is empty because
//! the store was unreadable, an `eq`/`contains`/`exists` requirement does not
//! hold → the rule is skipped, never silently matched.

use std::collections::BTreeMap;

use crate::models::{ChaosMode, MatchCriteria, ResponseSpec, StateWrite, WebhookAction};
use crate::repo::rules::RuleRow;

mod path;
mod predicates;

pub use path::{compile_path, CompiledPath};
pub use predicates::{BodyPredicate, StatePredicate};

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
    pub body_conditions: Vec<BodyPredicate>,
    pub state_requirements: Vec<StatePredicate>,
    pub response: ResponseSpec,
    pub state_writes: Vec<StateWrite>,
    pub latency_ms: Option<i64>,
    pub rate_limit_per_min: Option<i64>,
    /// Per-rule override (`None` inherits the endpoint's mode).
    pub chaos_mode: Option<ChaosMode>,
    pub webhook_action: Option<WebhookAction>,
}

impl CompiledRule {
    /// Compile a stored rule. A JSON column that does not parse degrades to
    /// its default (an unparseable match = the catch-all default criteria,
    /// exactly as the rule was always cold-loaded).
    pub(crate) fn from_row(row: &RuleRow) -> Self {
        let criteria: MatchCriteria = serde_json::from_str(&row.match_json).unwrap_or_default();
        let headers = criteria
            .headers
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        CompiledRule {
            id: row.id,
            priority: row.priority,
            enabled: row.enabled,
            method: criteria.method.to_ascii_uppercase(),
            path: compile_path(&criteria.path),
            headers,
            query: criteria.query,
            body_conditions: criteria
                .body_conditions
                .into_iter()
                .map(Into::into)
                .collect(),
            state_requirements: criteria
                .state_requirements
                .into_iter()
                .map(Into::into)
                .collect(),
            response: serde_json::from_str(&row.response_json).unwrap_or_default(),
            state_writes: serde_json::from_str(&row.state_writes_json).unwrap_or_default(),
            latency_ms: row.latency_ms,
            rate_limit_per_min: row.rate_limit_per_min,
            chaos_mode: row.chaos_mode.as_deref().map(ChaosMode::from_db),
            webhook_action: row
                .webhook_json
                .as_deref()
                .and_then(|w| serde_json::from_str::<WebhookAction>(w).ok()),
        }
    }

    pub fn gates_on_state(&self) -> bool {
        !self.state_requirements.is_empty()
    }

    fn method_ok(&self, method: &str) -> bool {
        self.method == "ANY" || self.method == method.to_ascii_uppercase()
    }

    fn headers_ok(&self, headers: &BTreeMap<String, String>) -> bool {
        self.headers
            .iter()
            .all(|(name, want)| headers.get(name) == Some(want))
    }

    fn query_ok(&self, query: &BTreeMap<String, String>) -> bool {
        self.query
            .iter()
            .all(|(key, want)| query.get(key) == Some(want))
    }

    fn body_ok(&self, body: &str) -> bool {
        self.body_conditions.iter().all(|c| c.holds(body))
    }

    fn state_ok(&self, state: &BTreeMap<String, String>) -> bool {
        self.state_requirements.iter().all(|r| r.holds(state))
    }
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
    rules.iter().find_map(|rule| {
        if !rule.enabled || !rule.method_ok(method) {
            return None;
        }
        let path_params = rule.path.match_path(path)?;
        let matches = rule.headers_ok(headers)
            && rule.query_ok(query)
            && rule.body_ok(body)
            && (!rule.gates_on_state() || rule.state_ok(state));
        matches.then_some(MatchResult { rule, path_params })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BodyCondition, StateRequirement};

    fn row(id: i64, priority: i64, match_json: String) -> RuleRow {
        RuleRow {
            id,
            token: "tok".into(),
            name: None,
            priority,
            enabled: true,
            match_json,
            response_json: "{}".into(),
            state_writes_json: "[]".into(),
            latency_ms: None,
            rate_limit_per_min: None,
            chaos_mode: None,
            webhook_json: None,
            created_at: String::new(),
        }
    }

    fn rule(id: i64, priority: i64, method: &str, path: &str) -> CompiledRule {
        let criteria = MatchCriteria {
            method: method.into(),
            path: path.into(),
            ..Default::default()
        };
        CompiledRule::from_row(&row(
            id,
            priority,
            serde_json::to_string(&criteria).unwrap(),
        ))
    }

    fn empty() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn from_row_upper_cases_the_method_and_reads_chaos_leniently() {
        assert_eq!(rule(1, 10, "get", "/x").method, "GET");
        let r = CompiledRule::from_row(&RuleRow {
            chaos_mode: Some("Dropout".into()),
            ..row(1, 10, "{}".into())
        });
        assert_eq!(r.chaos_mode, Some(ChaosMode::Dropout));
    }

    #[test]
    fn from_row_degrades_corrupt_json_to_defaults() {
        let corrupt = RuleRow {
            enabled: false,
            response_json: "[]".into(),
            state_writes_json: "nope".into(),
            latency_ms: Some(3),
            webhook_json: Some(r#"{"url":5}"#.into()),
            ..row(2, 5, "{oops".into())
        };
        let r = CompiledRule::from_row(&corrupt);
        assert_eq!(r.method, "ANY");
        assert!(r.path.match_path("/anything").is_some());
        assert_eq!(r.response.status_code, 200);
        assert!(r.state_writes.is_empty());
        assert!(r.webhook_action.is_none());
        assert_eq!(r.latency_ms, Some(3));
        assert!(!r.enabled);
    }

    #[test]
    fn order_is_priority_then_id() {
        let mut sorted = vec![
            rule(2, 50, "ANY", "/*"),
            rule(1, 10, "ANY", "/*"),
            rule(3, 10, "ANY", "/*"),
        ];
        // caller pre-sorts; emulate priority,id order.
        sorted.sort_by_key(|r| (r.priority, r.id));
        let m = select(&sorted, "GET", "/x", &empty(), &empty(), "", &empty()).unwrap();
        assert_eq!(m.rule.id, 1); // priority 10, lowest id
    }

    #[test]
    fn disabled_and_method_filter() {
        let mut r = rule(1, 10, "POST", "/*");
        r.enabled = false;
        assert!(select(&[r], "POST", "/x", &empty(), &empty(), "", &empty()).is_none());
        let r2 = [rule(1, 10, "POST", "/*")];
        assert!(select(&r2, "GET", "/x", &empty(), &empty(), "", &empty()).is_none());
        assert!(select(&r2, "post", "/x", &empty(), &empty(), "", &empty()).is_some());
    }

    #[test]
    fn body_and_state_conditions() {
        let mut r = rule(1, 10, "ANY", "/*");
        r.body_conditions = vec![BodyPredicate::from(BodyCondition {
            path: "kind".into(),
            op: "eq".into(),
            value: Some("vip".into()),
        })];
        let r = [r];
        let vip = r#"{"kind":"vip"}"#;
        assert!(select(&r, "POST", "/x", &empty(), &empty(), vip, &empty()).is_some());
        let std_body = r#"{"kind":"std"}"#;
        assert!(select(&r, "POST", "/x", &empty(), &empty(), std_body, &empty()).is_none());

        let mut sr = rule(2, 10, "ANY", "/*");
        sr.state_requirements = vec![StatePredicate::from(StateRequirement {
            key: "logged_in".into(),
            op: "eq".into(),
            value: Some("1".into()),
        })];
        let sr = [sr];
        // fail-closed: empty state -> skip.
        assert!(select(&sr, "GET", "/x", &empty(), &empty(), "", &empty()).is_none());
        let state: BTreeMap<String, String> = [("logged_in".to_string(), "1".to_string())].into();
        assert!(select(&sr, "GET", "/x", &empty(), &empty(), "", &state).is_some());
    }
}
