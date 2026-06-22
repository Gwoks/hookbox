//! `/healthz` liveness probe (P3, no auth). PORT of the FastAPI `/healthz`.

use axum::http::StatusCode;

pub async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
