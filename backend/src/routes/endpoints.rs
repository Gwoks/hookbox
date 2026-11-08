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
use serde_json::{json, Value};

use crate::auth::{require_owned_endpoint, OwnedEndpoint, OwnerId};
use crate::error::ApiError;
use crate::models::{EndpointCreate, EndpointDetail, EndpointSummary, Message};
use crate::repo::bind_value;
use crate::repo::endpoints::{self, EndpointRow};
use crate::state::AppState;

use crate::validate::validate_target_url;

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
    if let Some(n) = &body.name {
        if n.chars().count() > 100 {
            return Err(ApiError::validation("name must be at most 100 characters."));
        }
    }
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

pub(super) async fn patch_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(raw): Json<Value>,
) -> Result<Json<EndpointDetail>, ApiError> {
    let current = require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    let obj = raw
        .as_object()
        .ok_or_else(|| ApiError::validation("Request body must be a JSON object."))?;

    let mut sets: Vec<String> = Vec::new();
    let mut binds: Vec<Value> = Vec::new();

    // Validate + clamp each present field (mirrors pydantic exclude_unset).
    if let Some(v) = obj.get("name") {
        if !v.is_null() && v.as_str().map(|s| s.chars().count() > 100).unwrap_or(false) {
            return Err(ApiError::validation("name must be at most 100 characters."));
        }
        sets.push("name = ?".into());
        binds.push(v.clone());
    }
    if let Some(v) = obj.get("auto_crud") {
        let b = v
            .as_bool()
            .ok_or_else(|| ApiError::validation("auto_crud must be a boolean."))?;
        sets.push("auto_crud = ?".into());
        binds.push(json!(b as i64));
    }
    if let Some(v) = obj.get("target_url") {
        let s = if v.is_null() { None } else { v.as_str() };
        let validated = validate_target_url(s).map_err(ApiError::validation)?;
        sets.push("target_url = ?".into());
        binds.push(match validated {
            Some(u) => json!(u),
            None => Value::Null,
        });
    }
    if let Some(v) = obj.get("default_mode") {
        let m = v.as_str().unwrap_or("");
        if m != "mock_404" && m != "echo" {
            return Err(ApiError::validation(
                "default_mode must be 'mock_404' or 'echo'.",
            ));
        }
        sets.push("default_mode = ?".into());
        binds.push(json!(m));
    }
    if let Some(v) = obj.get("latency_ms") {
        let n = v
            .as_i64()
            .ok_or_else(|| ApiError::validation("latency_ms must be an integer."))?;
        sets.push("latency_ms = ?".into());
        binds.push(json!(n.clamp(0, state.cfg.latency_max_ms)));
    }
    if let Some(v) = obj.get("rate_limit_per_min") {
        let n = v
            .as_i64()
            .ok_or_else(|| ApiError::validation("rate_limit_per_min must be an integer."))?;
        sets.push("rate_limit_per_min = ?".into());
        binds.push(json!(n.clamp(0, state.cfg.rate_limit_max_per_min)));
    }
    if let Some(v) = obj.get("chaos_pct") {
        let n = v
            .as_i64()
            .ok_or_else(|| ApiError::validation("chaos_pct must be an integer."))?;
        sets.push("chaos_pct = ?".into());
        binds.push(json!(n.clamp(0, state.cfg.chaos_max_pct)));
    }
    if let Some(v) = obj.get("chaos_mode") {
        let m = v.as_str().unwrap_or("");
        if m != "error" && m != "dropout" {
            return Err(ApiError::validation(
                "chaos_mode must be 'error' or 'dropout'.",
            ));
        }
        sets.push("chaos_mode = ?".into());
        binds.push(json!(m));
    }
    if let Some(v) = obj.get("cors_enabled") {
        let b = v
            .as_bool()
            .ok_or_else(|| ApiError::validation("cors_enabled must be a boolean."))?;
        sets.push("cors_enabled = ?".into());
        binds.push(json!(b as i64));
    }

    if sets.is_empty() {
        return Ok(Json(endpoint_detail(&state, &current)));
    }
    let sql = format!("UPDATE endpoints SET {} WHERE token = ?", sets.join(", "));
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = bind_value(q, b);
    }
    q = q.bind(&token);
    state.write_through(&token, q.execute(&state.pool)).await?;

    let row = endpoints::find(&state.pool, &token)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
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
