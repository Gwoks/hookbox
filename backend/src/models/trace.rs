//! Owner-facing trace shapes (§5.3): the request summary + full detail.

use serde::Serialize;
use serde_json::{json, Value};

use crate::repo::requests::RequestRow;
use crate::util::json::parse_json_or;
use crate::util::time::rfc3339;

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

impl From<&RequestRow> for RequestSummary {
    fn from(row: &RequestRow) -> Self {
        RequestSummary {
            id: row.id,
            token: row.token.clone(),
            method: row.method.clone(),
            path: row.path.clone(),
            status_code: row.status_code,
            served_by: row.served_by.clone(),
            matched_rule_id: row.matched_rule_id,
            duration_ms: row.duration_ms,
            overhead_ms: row.overhead_ms,
            timestamp: rfc3339(&row.created_at),
        }
    }
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

impl From<&RequestRow> for RequestDetail {
    fn from(row: &RequestRow) -> Self {
        RequestDetail {
            summary: RequestSummary::from(row),
            request_headers: parse_json_or(row.request_headers.as_deref(), json!({})),
            query_params: parse_json_or(row.query_params.as_deref(), json!({})),
            request_body: row.request_body.clone(),
            response_headers: parse_json_or(row.response_headers.as_deref(), json!({})),
            response_body: row.response_body.clone(),
            trace: parse_json_or(row.trace_json.as_deref(), json!([])),
            state_snapshot: parse_json_or(row.state_snapshot.as_deref(), json!({})),
        }
    }
}
