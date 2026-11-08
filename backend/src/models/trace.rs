//! Owner-facing trace shapes (§5.3): the request summary + full detail.

use serde::Serialize;
use serde_json::Value;

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
