//! Session, endpoint summary/detail and endpoint-create/patch shapes (§5.3).

use serde::{Deserialize, Serialize};

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
