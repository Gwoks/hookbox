//! Trace history routes (§5.2 #12–#14): page an endpoint's request log, read
//! one full trace (ownership checked via the trace's endpoint), clear history.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::models::{Message, RequestDetail, RequestSummary};
use crate::state::AppState;
use crate::util::json::parse_json_or;
use crate::util::time::to_rfc3339;
use crate::validate::Paging;

fn request_summary(row: &sqlx::sqlite::SqliteRow) -> RequestSummary {
    RequestSummary {
        id: row.get("id"),
        token: row.get("token"),
        method: row.get("method"),
        path: row.get("path"),
        status_code: row.get("status_code"),
        served_by: row.get("served_by"),
        matched_rule_id: row.get("matched_rule_id"),
        duration_ms: row.get("duration_ms"),
        overhead_ms: row.get("overhead_ms"),
        timestamp: to_rfc3339(row.get("created_at")).unwrap_or_default(),
    }
}

// --- 12. GET /api/endpoints/{token}/requests?limit&offset --------------------

#[derive(Deserialize)]
pub(super) struct RequestsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

pub(super) async fn list_requests(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Query(q): Query<RequestsQuery>,
) -> Result<Json<Vec<RequestSummary>>, ApiError> {
    let paging = Paging::new(q.limit, q.offset)?;
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let rows =
        sqlx::query("SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(&token)
            .bind(paging.limit)
            .bind(paging.offset)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(rows.iter().map(request_summary).collect()))
}

// --- 13. GET /api/requests/{id} ----------------------------------------------

pub(super) async fn get_request(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(request_id): Path<i64>,
) -> Result<Json<RequestDetail>, ApiError> {
    let row = sqlx::query("SELECT * FROM request_logs WHERE id = ?")
        .bind(request_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Request not found."))?;
    let token: String = row.get("token");
    // Ownership via the trace's endpoint (404 for non-owner — AC-S2).
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let detail = RequestDetail {
        summary: request_summary(&row),
        request_headers: parse_json_or(row.get("request_headers"), json!({})),
        query_params: parse_json_or(row.get("query_params"), json!({})),
        request_body: row.get("request_body"),
        response_headers: parse_json_or(row.get("response_headers"), json!({})),
        response_body: row.get("response_body"),
        trace: parse_json_or(row.get("trace_json"), json!([])),
        state_snapshot: parse_json_or(row.get("state_snapshot"), json!({})),
    };
    Ok(Json(detail))
}

// --- 14. DELETE /api/endpoints/{token}/requests ------------------------------

pub(super) async fn clear_requests(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    sqlx::query("DELETE FROM request_logs WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    Ok(Json(Message::new("Trace history cleared.")))
}
