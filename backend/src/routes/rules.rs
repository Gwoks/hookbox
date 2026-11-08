//! Mock-rule management routes (§5.2 #7–#11). A rule row's
//! `match`/`response`/`state_writes`/`webhook_action` JSON columns come back
//! as parsed values so the exact stored shape round-trips.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;

use crate::auth::{require_owned_endpoint, OwnedEndpoint, OwnerId};
use crate::config::Config;
use crate::error::ApiError;
use crate::models::{MockRule, MockRuleCreate, RulePatch};
use crate::repo::rules;
use crate::state::AppState;
use crate::validate::{self, clamp_to};

fn rule_not_found() -> ApiError {
    ApiError::not_found("Rule not found.")
}

// --- 7. GET /api/endpoints/{token}/rules -------------------------------------

pub(super) async fn list_rules(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Result<Json<Vec<MockRule>>, ApiError> {
    let rows = rules::list(&state.pool, &endpoint.token).await?;
    Ok(Json(rows.iter().map(MockRule::from_row).collect()))
}

// --- 8. POST /api/endpoints/{token}/rules ------------------------------------

/// The create-side checks, in the frozen order (shared with PATCH).
fn validate_rule(cfg: &Config, r: &MockRuleCreate) -> Result<(), ApiError> {
    validate::rule_name(r.name.as_deref())?;
    validate::priority(r.priority)?;
    validate::response_spec(&r.response, cfg.template_max_size)?;
    r.chaos_mode
        .as_deref()
        .map(validate::chaos_mode)
        .transpose()?;
    Ok(())
}

pub(super) async fn create_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(data): Json<MockRuleCreate>,
) -> Result<Response, ApiError> {
    require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    validate_rule(&state.cfg, &data)?;
    let data = MockRuleCreate {
        latency_ms: clamp_to(data.latency_ms, state.cfg.latency_max_ms),
        rate_limit_per_min: clamp_to(data.rate_limit_per_min, state.cfg.rate_limit_max_per_min),
        ..data
    };
    let insert = rules::insert(&state.pool, &token, &data);
    let row = state.write_through(&token, insert).await?;
    Ok((StatusCode::CREATED, Json(MockRule::from_row(&row))).into_response())
}

// --- 9. GET /api/endpoints/{token}/rules/{id} --------------------------------

pub(super) async fn get_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Json<MockRule>, ApiError> {
    require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    let row = rules::find(&state.pool, &token, rule_id)
        .await?
        .ok_or_else(rule_not_found)?;
    Ok(Json(MockRule::from_row(&row)))
}

// --- 10. PATCH /api/endpoints/{token}/rules/{id} -----------------------------

/// 404 for an unknown rule BEFORE the body is validated; then only the
/// fields present change, each validated exactly as on create.
pub(super) async fn patch_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
    Json(raw): Json<Value>,
) -> Result<Json<MockRule>, ApiError> {
    require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    let current = rules::find(&state.pool, &token, rule_id)
        .await?
        .ok_or_else(rule_not_found)?;
    let patch = RulePatch::parse(&raw, &state.cfg)?;
    let update = rules::update(&state.pool, &token, rule_id, patch);
    let row = state
        .write_through(&token, update)
        .await?
        .unwrap_or(current);
    Ok(Json(MockRule::from_row(&row)))
}

// --- 11. DELETE /api/endpoints/{token}/rules/{id} (204, no body) -------------

pub(super) async fn delete_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    let delete = rules::delete(&state.pool, &token, rule_id);
    if !state.write_through(&token, delete).await? {
        return Err(rule_not_found());
    }
    Ok(StatusCode::NO_CONTENT.into_response()) // 204 carries no body
}
