//! The FROZEN §5.3 interface contract as serde structs (PORT of `app/models.py`,
//! + the OQ-2 `chaos_mode` additions from architecture.md §5.3).
//!
//! These serialize to the EXACT §5.3 JSON shapes: field names/types/defaults
//! are frozen. `Option<T>` fields serialize as `null` when absent (the JSON key
//! is present with `null`, matching pydantic's default) — we deliberately do NOT
//! `skip_serializing_if`. Collection/string defaults use `#[serde(default)]` so
//! a partial request body fills the documented defaults.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

// ---- Session / owner ----
#[derive(Debug, Deserialize)]
pub struct SessionCreate {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointSummary {
    pub token: String,
    pub name: Option<String>,
    pub mock_url: String,
    pub path_url: String,
    pub created_at: String,
    pub last_hit: Option<String>,
    pub request_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub owner_id: String,
    pub owner_secret: String,
    pub endpoints: Vec<EndpointSummary>,
    pub primary: EndpointSummary,
}

// ---- Endpoint config ----
#[derive(Debug, Deserialize)]
pub struct EndpointCreate {
    #[serde(default)]
    pub name: Option<String>,
}

/// All optional → partial update. We track presence via `Option` plus, for
/// nullable-clearing fields, `Option<Option<T>>` is overkill here; the route
/// builds the SET clause from whichever keys are present using a manual
/// `serde_json::Map` pass (mirrors pydantic `exclude_unset`). For typed clamp
/// validation we still deserialize into this struct.
#[derive(Debug, Default, Deserialize)]
pub struct EndpointConfigPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub auto_crud: Option<bool>,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub default_mode: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
    #[serde(default)]
    pub rate_limit_per_min: Option<i64>,
    #[serde(default)]
    pub chaos_pct: Option<i64>,
    #[serde(default)]
    pub chaos_mode: Option<String>, // OQ-2: "error" | "dropout"
    #[serde(default)]
    pub cors_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct EndpointDetail {
    pub token: String,
    pub name: Option<String>,
    pub mock_url: String,
    pub path_url: String,
    pub auto_crud: bool,
    pub target_url: Option<String>,
    pub default_mode: String,
    pub latency_ms: i64,
    pub rate_limit_per_min: i64,
    pub chaos_pct: i64,
    pub chaos_mode: String, // OQ-2
    pub cors_enabled: bool,
    pub tunnel_active: bool,
    pub created_at: String,
    pub last_hit: Option<String>,
    pub request_count: i64,
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

// ---- Traces ----
#[derive(Debug, Serialize)]
pub struct RequestSummary {
    pub id: i64,
    pub token: String,
    pub method: String,
    pub path: String,
    pub status_code: i64,
    pub served_by: String,
    pub matched_rule_id: Option<i64>,
    pub duration_ms: i64,
    pub overhead_ms: i64,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct RequestDetail {
    #[serde(flatten)]
    pub summary: RequestSummary,
    pub request_headers: Value,
    pub query_params: Value,
    pub request_body: Option<String>,
    pub response_headers: Value,
    pub response_body: Option<String>,
    pub trace: Value,
    pub state_snapshot: Value,
}

// ---- F4 share links (§5.5.1-§5.5.5) ----

/// Request body for `POST /api/endpoints/{token}/shares` (§5.5.1).
#[derive(Debug, Deserialize)]
pub struct ShareLinkCreate {
    #[serde(default)]
    pub label: Option<String>,
}

/// Owner list item (`GET /api/endpoints/{token}/shares`) — carries **no**
/// secret material: no `code`, no `url`, no code prefix (§5.5.2, AC-25).
#[derive(Debug, Serialize)]
pub struct ShareLink {
    pub id: i64,
    pub label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// 201 body for `POST /api/endpoints/{token}/shares` — the **only** place
/// `code`/`url` ever appear, in the **only** response that ever shows them
/// (§5.5.3, AC-104).
#[derive(Debug, Serialize)]
pub struct ShareLinkCreated {
    pub id: i64,
    pub code: String,
    pub url: String,
    pub label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// `endpoint` sub-object of `PublicShareFeed` (§5.5.4). `name` is
/// operator-authored text rendered to strangers — intentionally disclosed.
#[derive(Debug, Serialize)]
pub struct PublicEndpointInfo {
    pub name: Option<String>,
    pub created_at: String,
    pub request_count: i64,
}

/// 200 body for `GET /api/share/{code}/requests` (§5.5.4).
#[derive(Debug, Serialize)]
pub struct PublicShareFeed {
    pub endpoint: PublicEndpointInfo,
    pub requests: Vec<PublicRequestSummary>,
}

/// Public trace projection — reduced from `RequestSummary` (§5.5.5). Built
/// standalone, field-by-field, from the row: never `#[serde(flatten)]`ed off
/// an owner struct, so a future owner-shape field cannot leak here by default
/// (AC-34, AC-102). Omits `token`, `matched_rule_id`, `overhead_ms`.
#[derive(Debug, Serialize)]
pub struct PublicRequestSummary {
    pub id: i64,
    pub method: String,
    pub path: String,
    pub status_code: i64,
    pub served_by: String,
    pub duration_ms: i64,
    pub timestamp: String,
}

/// 200 body for `GET /api/share/{code}/requests/{id}` (§5.5.5). `response_headers`
/// is a FILTERED map (§5.11) — the one security-driven contract delta (AC-S1).
/// All five body/header fields are present keys (present-with-`null`), never
/// omitted, so a future narrowing is a deliberate contract change (AC-34).
#[derive(Debug, Serialize)]
pub struct PublicRequestDetail {
    pub id: i64,
    pub method: String,
    pub path: String,
    pub status_code: i64,
    pub served_by: String,
    pub duration_ms: i64,
    pub timestamp: String,
    pub request_headers: Value,
    pub query_params: Value,
    pub request_body: Option<String>,
    pub response_headers: Value,
    pub response_body: Option<String>,
}

// ---- Generic ----
#[derive(Debug, Serialize)]
pub struct Message {
    pub message: String,
    pub success: bool,
}

impl Message {
    pub fn new(message: impl Into<String>) -> Self {
        Message {
            message: message.into(),
            success: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_detail_emits_null_keys_for_optionals() {
        let d = EndpointDetail {
            token: "tok1234567".into(),
            name: None,
            mock_url: "https://tok1234567.mock.local".into(),
            path_url: "/e/tok1234567".into(),
            auto_crud: false,
            target_url: None,
            default_mode: "mock_404".into(),
            latency_ms: 0,
            rate_limit_per_min: 0,
            chaos_pct: 0,
            chaos_mode: "error".into(),
            cors_enabled: true,
            tunnel_active: false,
            created_at: "2026-06-21T12:00:00Z".into(),
            last_hit: None,
            request_count: 0,
        };
        let v = serde_json::to_value(&d).unwrap();
        // Optional absent fields are present with null (pydantic parity).
        assert!(v.as_object().unwrap().contains_key("name"));
        assert!(v["name"].is_null());
        assert!(v["last_hit"].is_null());
        assert!(v["target_url"].is_null());
        // OQ-2 chaos_mode present.
        assert_eq!(v["chaos_mode"], serde_json::json!("error"));
    }

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
