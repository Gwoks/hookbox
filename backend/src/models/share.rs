//! F4 share-link shapes (§5.5.1–§5.5.5): owner mint/list bodies and the
//! standalone PUBLIC projection structs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::repo::shares::ShareRow;
use crate::util::time::{rfc3339, to_rfc3339};

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

impl ShareLink {
    pub(crate) fn from_row(row: &ShareRow) -> Self {
        ShareLink {
            id: row.id,
            label: row.label.clone(),
            created_at: rfc3339(&row.created_at),
            last_used_at: to_rfc3339(row.last_used_at.as_deref()),
        }
    }
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
