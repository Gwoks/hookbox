//! Per-endpoint state + Auto-CRUD collection peek/clear routes (§5.2 #15–#18).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::models::Message;
use crate::state::AppState;
use crate::util::json::parse_json_or;
use crate::validate::is_safe_key;

// --- 15. GET /api/endpoints/{token}/state ------------------------------------

pub(super) async fn get_state(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Value>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let rows = sqlx::query(
        "SELECT key, value FROM endpoint_state WHERE token = ? AND expires_at > datetime('now')",
    )
    .bind(&token)
    .fetch_all(&state.pool)
    .await?;
    let mut map = serde_json::Map::new();
    for r in &rows {
        let k: String = r.get("key");
        let v: String = r.get("value");
        map.insert(k, json!(v));
    }
    Ok(Json(json!({ "state": Value::Object(map) })))
}

// --- 16. DELETE /api/endpoints/{token}/state ---------------------------------

pub(super) async fn clear_state(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    crate::repo::state::clear_state(&state.pool, &token).await?;
    Ok(Json(Message::new("State cleared.")))
}

// --- 17. GET /api/endpoints/{token}/collections/{name} -----------------------

pub(super) async fn peek_collection(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, name)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    if !is_safe_key(&name) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_collection",
            "Invalid collection name.",
        ));
    }
    let items: Option<String> = sqlx::query_scalar(
        "SELECT items_json FROM crud_collections WHERE token = ? AND name = ? AND expires_at > datetime('now')",
    )
    .bind(&token)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?;
    let parsed = parse_json_or(items, json!([]));
    Ok(Json(json!({ "items": parsed })))
}

// --- 18. DELETE /api/endpoints/{token}/collections/{name} --------------------

pub(super) async fn clear_collection(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, name)): Path<(String, String)>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    if !is_safe_key(&name) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_collection",
            "Invalid collection name.",
        ));
    }
    sqlx::query("DELETE FROM crud_collections WHERE token = ? AND name = ?")
        .bind(&token)
        .bind(&name)
        .execute(&state.pool)
        .await?;
    Ok(Json(Message::new("Collection cleared.")))
}
