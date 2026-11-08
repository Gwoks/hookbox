//! The mock plane's fixed error responses (§5.5), as one table. Each renders
//! through `ApiError` — the same flat `{"detail","error"}` body bytes and
//! `content-type: application/json` as every management error.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::error::{ApiError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MockError {
    /// 404 — no endpoint has this token.
    UnknownEndpoint,
    /// 410 — the endpoint was deleted (tombstoned).
    EndpointGone,
    /// 413 — the request body exceeds `MAX_INGEST_BODY_BYTES`.
    PayloadTooLarge,
    /// 404 — `default_mode = mock_404` and nothing handled the request.
    NoMatch,
    /// 503 — the Auto-CRUD store failed.
    CrudStoreUnavailable,
    /// 504 — the bound tunnel CLI did not answer.
    NoTunnel,
    /// 504 — the MITM upstream timed out.
    UpstreamTimeout,
    /// 502 — the MITM upstream was unreachable (or SSRF-blocked).
    UpstreamUnreachable,
    /// The injected chaos failure, with its rolled 5xx status.
    Chaos(u16),
}

impl MockError {
    fn parts(self) -> (StatusCode, ErrorCode, &'static str) {
        match self {
            MockError::UnknownEndpoint => (
                StatusCode::NOT_FOUND,
                ErrorCode::UnknownEndpoint,
                "No such endpoint.",
            ),
            MockError::EndpointGone => (
                StatusCode::GONE,
                ErrorCode::EndpointGone,
                "This endpoint was deleted or expired.",
            ),
            MockError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::PayloadTooLarge,
                "Request body exceeds the configured limit.",
            ),
            MockError::NoMatch => (
                StatusCode::NOT_FOUND,
                ErrorCode::NoMatch,
                "No rule matched this request.",
            ),
            MockError::CrudStoreUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::StoreUnavailable,
                "Auto-CRUD store unavailable.",
            ),
            MockError::NoTunnel => (
                StatusCode::GATEWAY_TIMEOUT,
                ErrorCode::NoTunnel,
                "Tunnel error.",
            ),
            MockError::UpstreamTimeout => (
                StatusCode::GATEWAY_TIMEOUT,
                ErrorCode::UpstreamTimeout,
                "Upstream did not respond in time.",
            ),
            MockError::UpstreamUnreachable => (
                StatusCode::BAD_GATEWAY,
                ErrorCode::UpstreamUnreachable,
                "Could not reach the upstream target.",
            ),
            MockError::Chaos(status) => (
                StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
                ErrorCode::Chaos,
                "Injected chaos failure.",
            ),
        }
    }

    pub(crate) fn api_error(self) -> ApiError {
        let (status, code, detail) = self.parts();
        ApiError::new(status, code, detail)
    }
}

impl IntoResponse for MockError {
    fn into_response(self) -> Response {
        self.api_error().into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn renders_the_flat_envelope_bytes() {
        let resp = MockError::NoMatch.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(resp.headers()["content-type"], "application/json");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            &bytes[..],
            br#"{"detail":"No rule matched this request.","error":"no_match"}"#
        );
        assert_eq!(MockError::Chaos(504).api_error().status.as_u16(), 504);
        assert_eq!(
            MockError::EndpointGone.api_error().body(),
            ApiError::endpoint_gone().body()
        );
    }
}
