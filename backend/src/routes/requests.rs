//! Trace history routes (§5.2 #12–#14): page an endpoint's request log, read
//! one full trace (ownership checked via the trace's endpoint), clear history.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::models::{Message, RequestDetail, RequestSummary};
use crate::repo::requests;
use crate::state::AppState;
use crate::validate::Paging;

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
    let rows = requests::page(&state.pool, &token, paging).await?;
    Ok(Json(rows.iter().map(RequestSummary::from).collect()))
}

// --- 13. GET /api/requests/{id} ----------------------------------------------

pub(super) async fn get_request(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(request_id): Path<i64>,
) -> Result<Json<RequestDetail>, ApiError> {
    let row = requests::find(&state.pool, request_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Request not found."))?;
    // Ownership via the trace's endpoint (404 for non-owner — AC-S2).
    assert_owns_endpoint(&state.pool, &row.token, &owner_id).await?;
    Ok(Json(RequestDetail::from(&row)))
}

// --- 14. DELETE /api/endpoints/{token}/requests ------------------------------

pub(super) async fn clear_requests(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    requests::clear(&state.pool, &token).await?;
    Ok(Json(Message::new("Trace history cleared.")))
}
