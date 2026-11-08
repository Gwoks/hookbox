//! The rich mock-rule shapes (§5.3): match criteria, response spec, state
//! writes, webhook action, and the serialized rule row.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::repo::rules::RuleRow;
use crate::util::json::parse_json_or;
use crate::util::time::rfc3339;

fn default_method() -> String {
    "ANY".to_string()
}
fn default_match_path() -> String {
    "/*".to_string()
}
fn default_priority() -> i64 {
    100
}
fn default_true() -> bool {
    true
}
fn default_status_code() -> i64 {
    200
}
fn default_content_type() -> String {
    "application/json".to_string()
}

// ---- Mock rule (rich) ----
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyCondition {
    pub path: String,
    #[serde(default = "default_eq")]
    pub op: String,
    #[serde(default)]
    pub value: Option<String>,
}
fn default_eq() -> String {
    "eq".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRequirement {
    pub key: String,
    #[serde(default = "default_eq")]
    pub op: String,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCriteria {
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_match_path")]
    pub path: String,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub query: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub body_conditions: Vec<BodyCondition>,
    #[serde(default)]
    pub state_requirements: Vec<StateRequirement>,
}

impl Default for MatchCriteria {
    fn default() -> Self {
        MatchCriteria {
            method: default_method(),
            path: default_match_path(),
            headers: Default::default(),
            query: Default::default(),
            body_conditions: Vec::new(),
            state_requirements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateWrite {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSpec {
    #[serde(default = "default_status_code")]
    pub status_code: i64,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub body_template: String,
    #[serde(default = "default_content_type")]
    pub content_type: String,
}

impl Default for ResponseSpec {
    fn default() -> Self {
        ResponseSpec {
            status_code: default_status_code(),
            headers: Default::default(),
            body_template: String::new(),
            content_type: default_content_type(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookAction {
    pub url: String,
    #[serde(default)]
    pub body_template: String,
}

#[derive(Debug, Deserialize)]
pub struct MockRuleCreate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub r#match: MatchCriteria,
    #[serde(default)]
    pub response: ResponseSpec,
    #[serde(default)]
    pub state_writes: Vec<StateWrite>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
    #[serde(default)]
    pub rate_limit_per_min: Option<i64>,
    #[serde(default)]
    pub chaos_mode: Option<String>, // OQ-2 per-rule override
    #[serde(default)]
    pub webhook_action: Option<WebhookAction>,
}

/// The serialized rule (`MockRuleCreate` + id/token/created_at). Built from a DB
/// row; `match`/`response`/`state_writes`/`webhook_action` come back as parsed
/// JSON values so the exact stored shape round-trips.
#[derive(Debug, Serialize)]
pub struct MockRule {
    pub id: i64,
    pub token: String,
    pub name: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    #[serde(rename = "match")]
    pub match_: Value,
    pub response: Value,
    pub state_writes: Value,
    pub latency_ms: Option<i64>,
    pub rate_limit_per_min: Option<i64>,
    pub chaos_mode: Option<String>, // OQ-2
    pub webhook_action: Option<Value>,
    pub created_at: String,
}

impl MockRule {
    pub(crate) fn from_row(row: &RuleRow) -> Self {
        MockRule {
            id: row.id,
            token: row.token.clone(),
            name: row.name.clone(),
            priority: row.priority,
            enabled: row.enabled,
            match_: parse_json_or(Some(row.match_json.as_str()), json!({})),
            response: parse_json_or(Some(row.response_json.as_str()), json!({})),
            state_writes: parse_json_or(Some(row.state_writes_json.as_str()), json!([])),
            latency_ms: row.latency_ms,
            rate_limit_per_min: row.rate_limit_per_min,
            chaos_mode: row.chaos_mode.clone(),
            webhook_action: row
                .webhook_json
                .as_deref()
                .and_then(|w| serde_json::from_str(w).ok()),
            created_at: rfc3339(&row.created_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_create_defaults_match_contract() {
        // Empty body fills all §5.3 defaults.
        let r: MockRuleCreate = serde_json::from_str("{}").unwrap();
        assert_eq!(r.priority, 100);
        assert!(r.enabled);
        assert_eq!(r.r#match.method, "ANY");
        assert_eq!(r.r#match.path, "/*");
        assert_eq!(r.response.status_code, 200);
        assert_eq!(r.response.content_type, "application/json");
        assert!(r.state_writes.is_empty());
        assert!(r.latency_ms.is_none());
        assert!(r.chaos_mode.is_none());
    }

    #[test]
    fn mock_rule_serializes_match_key() {
        let rule = MockRule {
            id: 1,
            token: "tok1234567".into(),
            name: None,
            priority: 100,
            enabled: true,
            match_: serde_json::json!({"method": "GET"}),
            response: serde_json::json!({"status_code": 200}),
            state_writes: serde_json::json!([]),
            latency_ms: None,
            rate_limit_per_min: None,
            chaos_mode: None,
            webhook_action: None,
            created_at: "2026-06-21T12:00:00Z".into(),
        };
        let v = serde_json::to_value(&rule).unwrap();
        // The field is named "match" on the wire (Rust keyword renamed).
        assert_eq!(v["match"], serde_json::json!({"method": "GET"}));
        assert!(v.as_object().unwrap().contains_key("webhook_action"));
        assert!(v["webhook_action"].is_null());
    }
}
