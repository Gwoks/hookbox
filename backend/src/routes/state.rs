//! Per-endpoint state + Auto-CRUD collection peek/clear routes (§5.2 #15–#18).

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::json;

use crate::auth::OwnedEndpoint;
use crate::error::ApiError;
use crate::models::{CollectionResponse, Message, StateResponse};
use crate::repo;
use crate::state::AppState;
use crate::util::json::parse_json_or;
use crate::validate::CollectionName;

// --- 15. GET /api/endpoints/{token}/state ------------------------------------

pub(super) async fn get_state(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Result<Json<StateResponse>, ApiError> {
    let map = repo::state::read_state(&state.pool, &endpoint.token).await?;
    Ok(Json(StateResponse { state: map }))
}

// --- 16. DELETE /api/endpoints/{token}/state ---------------------------------

pub(super) async fn clear_state(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Result<Json<Message>, ApiError> {
    repo::state::clear_state(&state.pool, &endpoint.token).await?;
    Ok(Json(Message::new("State cleared.")))
}

// --- 17. GET /api/endpoints/{token}/collections/{name} -----------------------

pub(super) async fn peek_collection(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
    Path((_, name)): Path<(String, String)>,
) -> Result<Json<CollectionResponse>, ApiError> {
    let name = CollectionName::parse(name)?;
    let items = repo::crud::read_items_json(&state.pool, &endpoint.token, name.as_str()).await?;
    Ok(Json(CollectionResponse {
        items: parse_json_or(items.as_deref(), json!([])),
    }))
}

// --- 18. DELETE /api/endpoints/{token}/collections/{name} --------------------

pub(super) async fn clear_collection(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
    Path((_, name)): Path<(String, String)>,
) -> Result<Json<Message>, ApiError> {
    let name = CollectionName::parse(name)?;
    repo::crud::clear(&state.pool, &endpoint.token, name.as_str()).await?;
    Ok(Json(Message::new("Collection cleared.")))
}
