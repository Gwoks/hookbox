//! Uniform API error envelope — the **FLAT** `{ "error": "<code>", "detail": "<human>" }`
//! shape (AC-60). This is deliberately NOT the shortener's nested
//! `{ "error": { "code", "message" } }` shape — HookBox keeps its own flat
//! envelope across BOTH the management plane (P2) and the mock plane (P1).
//!
//! Ported from `app/auth.py` / `app/routes/api.py` / `app/interceptor/engine.py`,
//! where every non-2xx body is `{"error": code, "detail": human}`.

use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::net::headers::try_header;

mod api_json;
mod code;

pub use api_json::ApiJson;
pub use code::ErrorCode;

/// Internal error type, converted to the flat JSON envelope on response.
///
/// `code` is the stable machine string (e.g. `"unauthorized"`, `"not_found"`),
/// `detail` is the human, recovery-oriented message, `status` is the HTTP code.
/// Extra response headers (e.g. `WWW-Authenticate`, `Retry-After`,
/// `X-RateLimit-*`) ride along in `headers`.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: ErrorCode,
    pub detail: String,
    pub headers: Vec<(HeaderName, HeaderValue)>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: ErrorCode, detail: impl Into<String>) -> Self {
        ApiError {
            status,
            code,
            detail: detail.into(),
            headers: Vec::new(),
        }
    }

    /// Attach an extra response header (silently ignored if invalid).
    pub fn with_header(mut self, name: &'static str, value: impl AsRef<str>) -> Self {
        if let Some(pair) = try_header(name, value.as_ref()) {
            self.headers.push(pair);
        }
        self
    }

    /// `401 unauthorized` + `WWW-Authenticate: Bearer` (§5.1).
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        ApiError::new(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized, detail)
            .with_header("www-authenticate", "Bearer")
    }

    /// `403 csrf_rejected` — a cookie-authenticated (or cookie-setting) call
    /// without `x-hookbox: 1`, or from a foreign origin. Runs before any
    /// credential lookup, so it never reveals whether a cookie is valid.
    pub fn csrf_rejected() -> Self {
        ApiError::new(
            StatusCode::FORBIDDEN,
            ErrorCode::CsrfRejected,
            "This request must come from the HookBox app.",
        )
    }

    /// `401 invalid_key` — a recovery key that is malformed or unknown.
    pub fn invalid_key() -> Self {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidKey,
            "That recovery key is not valid.",
        )
    }

    /// `404 not_found` — used for both "absent" and "exists-but-not-mine"
    /// (a non-owner cannot distinguish the two; never 403). §5.1.
    pub fn not_found(detail: impl Into<String>) -> Self {
        ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, detail)
    }

    /// `410 endpoint_gone` — the endpoint was deleted (tombstoned). The mock
    /// plane and the owner routes share this exact body.
    pub fn endpoint_gone() -> Self {
        ApiError::new(
            StatusCode::GONE,
            ErrorCode::EndpointGone,
            "This endpoint was deleted or expired.",
        )
    }

    /// `422` validation failure (bad field / clamp).
    pub fn validation(detail: impl Into<String>) -> Self {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::ValidationError,
            detail,
        )
    }

    /// `422 invalid_collection` — a collection name outside the safe
    /// `^[A-Za-z0-9_-]{1,64}$` charset (§5.2 #17/#18).
    pub fn invalid_collection() -> Self {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCollection,
            "Invalid collection name.",
        )
    }

    /// `429 rate_limited` with `Retry-After` (and optional `X-RateLimit-*`).
    pub fn rate_limited(detail: impl Into<String>, retry_after: i64) -> Self {
        ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            detail,
        )
        .with_header("retry-after", retry_after.to_string())
    }

    /// The flat JSON body value `{"error": <code>, "detail": <detail>}`.
    pub fn body(&self) -> serde_json::Value {
        json!({ "error": self.code.as_str(), "detail": self.detail })
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, axum::Json(self.body())).into_response();
        for (name, value) in self.headers {
            response.headers_mut().insert(name, value);
        }
        response
    }
}

/// Any unclassified internal failure (e.g. a SQLite fault) maps to a stable
/// `503 store_unavailable` so the flat envelope stays consistent (OQ-3).
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("sqlite error: {e}");
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::StoreUnavailable,
            "The data store is temporarily unavailable. Please try again.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_envelope_shape() {
        let body = ApiError::not_found("nope").body();
        assert_eq!(body, json!({ "error": "not_found", "detail": "nope" }));
        // The flat shape must NOT nest under error.
        assert!(body["error"].is_string());
        assert!(body.get("detail").is_some());
    }

    #[test]
    fn unauthorized_carries_www_authenticate() {
        let e = ApiError::unauthorized("missing token");
        assert_eq!(e.status, StatusCode::UNAUTHORIZED);
        assert_eq!(e.code, "unauthorized");
        assert!(e
            .headers
            .iter()
            .any(|(n, v)| n.as_str() == "www-authenticate" && v == "Bearer"));
    }

    #[test]
    fn rate_limited_carries_retry_after() {
        let e = ApiError::rate_limited("slow down", 7);
        assert_eq!(e.status, StatusCode::TOO_MANY_REQUESTS);
        assert!(e
            .headers
            .iter()
            .any(|(n, v)| n.as_str() == "retry-after" && v == "7"));
    }

    #[test]
    fn csrf_rejected_is_403_and_invalid_key_is_401() {
        let e = ApiError::csrf_rejected();
        assert_eq!(e.status, StatusCode::FORBIDDEN);
        assert_eq!(e.code, "csrf_rejected");
        let e = ApiError::invalid_key();
        assert_eq!(e.status, StatusCode::UNAUTHORIZED);
        assert_eq!(e.code, "invalid_key");
    }

    #[test]
    fn validation_is_422() {
        assert_eq!(
            ApiError::validation("bad").status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}
