//! Session, endpoint summary/detail, endpoint-create, and the state /
//! collection peek shapes (§5.3).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---- Session / owner ----
#[derive(Debug, Clone, Serialize)]
pub struct EndpointSummary {
    pub token: String,
    pub name: Option<String>,
    pub mock_url: String,
    pub path_url: String,
    pub created_at: String,
    pub last_hit: Option<String>,
    pub request_count: i64,
}

/// `GET|POST /api/session` and `POST /api/session/restore`: who the caller
/// is plus their live (non-tombstoned) endpoints. Deliberately carries NO
/// key — the key lives only in the HttpOnly cookie (or is revealed on
/// explicit request via `POST /api/session/key`).
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub owner_id: String,
    pub endpoints: Vec<EndpointSummary>,
    /// The oldest live endpoint; `null` when the owner has none.
    pub primary: Option<EndpointSummary>,
}

/// `POST /api/session/restore` body.
#[derive(Debug, Deserialize)]
pub struct RestoreBody {
    pub key: String,
}

/// `POST /api/session/key` and `POST /api/session/rotate`.
#[derive(Debug, Serialize)]
pub struct KeyResponse {
    pub key: String,
}

// ---- Endpoint config ----
#[derive(Debug, Deserialize)]
pub struct EndpointCreate {
    #[serde(default)]
    pub name: Option<String>,
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

// ---- Endpoint state / Auto-CRUD peek (#15, #17) ----

/// `GET /api/endpoints/{token}/state` → `{"state": {key: value}}`.
#[derive(Debug, Serialize)]
pub struct StateResponse {
    pub state: BTreeMap<String, String>,
}

/// `GET /api/endpoints/{token}/collections/{name}` → `{"items": [...]}` (the
/// stored array, passed through as parsed).
#[derive(Debug, Serialize)]
pub struct CollectionResponse {
    pub items: Value,
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
}
