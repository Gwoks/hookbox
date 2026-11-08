//! Endpoint management routes (§5.2 #2–#6): list/create/get/patch/delete an
//! owner's endpoints, plus the endpoint presentation shared with the session
//! route. Endpoint-scoped routes 404 (never 403) for a non-owner (AC-S2/S3).
//! The §5.3 `chaos_mode` (OQ-2) is a real column joined into every shape.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;

use crate::auth::{require_owned_endpoint, OwnedEndpoint, OwnerId};
use crate::error::ApiError;
use crate::models::{EndpointCreate, EndpointDetail, EndpointPatch, EndpointSummary, Message};
use crate::repo::endpoints::{self, EndpointRow};
use crate::state::AppState;
use crate::validate;

// --- presentation -------------------------------------------------------------

/// The summary shape (shared with the session route).
pub(super) fn endpoint_summary(state: &AppState, row: &EndpointRow) -> EndpointSummary {
    EndpointSummary::from_row(row, &state.cfg)
}

fn endpoint_detail(state: &AppState, row: &EndpointRow) -> EndpointDetail {
    EndpointDetail::from_row(row, &state.cfg, state.tunnels.is_active(&row.token))
}

/// Insert a fresh endpoint for an owner (shared with the session mint).
/// Returns the new token.
pub(super) async fn new_endpoint(
    state: &AppState,
    owner_id: &str,
    name: Option<&str>,
) -> Result<String, ApiError> {
    Ok(create(state, owner_id, name).await?.token)
}

async fn create(
    state: &AppState,
    owner_id: &str,
    name: Option<&str>,
) -> Result<EndpointRow, sqlx::Error> {
    let len = state.cfg.endpoint_id_length;
    endpoints::create_with_fresh_token(&state.pool, owner_id, name, len).await
}

// --- 2. GET /api/endpoints ----------------------------------------------------

pub(super) async fn list_endpoints(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
) -> Result<Json<Vec<EndpointSummary>>, ApiError> {
    let rows = endpoints::list_by_owner(&state.pool, &owner_id).await?;
    Ok(Json(
        rows.iter().map(|r| endpoint_summary(&state, r)).collect(),
    ))
}

// --- 3. POST /api/endpoints ---------------------------------------------------

pub(super) async fn create_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Json(body): Json<EndpointCreate>,
) -> Result<Response, ApiError> {
    validate::endpoint_name(body.name.as_deref())?;
    let row = create(&state, &owner_id, body.name.as_deref()).await?;
    Ok((StatusCode::CREATED, Json(endpoint_detail(&state, &row))).into_response())
}

// --- 4. GET /api/endpoints/{token} -------------------------------------------

pub(super) async fn get_endpoint(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Json<EndpointDetail> {
    Json(endpoint_detail(&state, &endpoint))
}

// --- 5. PATCH /api/endpoints/{token} -----------------------------------------

/// Only the fields present in the body change (pydantic `exclude_unset`);
/// each is validated exactly as on create (`EndpointPatch::parse`).
pub(super) async fn patch_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(raw): Json<Value>,
) -> Result<Json<EndpointDetail>, ApiError> {
    let current = require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    let patch = EndpointPatch::parse(&raw, &state.cfg)?;
    let update = endpoints::update(&state.pool, &token, patch);
    let row = state
        .write_through(&token, update)
        .await?
        .unwrap_or(current);
    Ok(Json(endpoint_detail(&state, &row)))
}

// --- 6. DELETE /api/endpoints/{token} (tombstone) ----------------------------

pub(super) async fn delete_endpoint(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Result<Json<Message>, ApiError> {
    let token = &endpoint.token;
    state
        .write_through(token, endpoints::tombstone(&state.pool, token))
        .await?;
    Ok(Json(Message::new("Endpoint deleted.")))
}
