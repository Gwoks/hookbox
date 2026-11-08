//! Mock-rule management routes (§5.2 #7–#11). A rule row's
//! `match`/`response`/`state_writes`/`webhook_action` JSON columns come back
//! as parsed values so the exact stored shape round-trips.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::models::{MatchCriteria, MockRule, MockRuleCreate, ResponseSpec, StateWrite};
use crate::repo::{bind_value, rules};
use crate::state::AppState;

fn rule_not_found() -> ApiError {
    ApiError::not_found("Rule not found.")
}

// --- 7. GET /api/endpoints/{token}/rules -------------------------------------

pub(super) async fn list_rules(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Vec<MockRule>>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let rows = rules::list(&state.pool, &token).await?;
    Ok(Json(rows.iter().map(MockRule::from_row).collect()))
}

// --- 8. POST /api/endpoints/{token}/rules ------------------------------------

fn clamp_opt(v: Option<i64>, lo: i64, hi: i64) -> Option<i64> {
    v.map(|n| n.clamp(lo, hi))
}

fn validate_rule(state: &AppState, r: &MockRuleCreate) -> Result<(), ApiError> {
    if let Some(n) = &r.name {
        if n.chars().count() > 120 {
            return Err(ApiError::validation("name must be at most 120 characters."));
        }
    }
    if !(0..=100000).contains(&r.priority) {
        return Err(ApiError::validation(
            "priority must be between 0 and 100000.",
        ));
    }
    if r.response.status_code < 100 || r.response.status_code > 599 {
        return Err(ApiError::validation(
            "response.status_code must be 100..599.",
        ));
    }
    if r.response.body_template.len() > state.cfg.template_max_size {
        return Err(ApiError::validation("response.body_template is too large."));
    }
    if let Some(m) = &r.chaos_mode {
        if m != "error" && m != "dropout" {
            return Err(ApiError::validation(
                "chaos_mode must be 'error' or 'dropout'.",
            ));
        }
    }
    Ok(())
}

pub(super) async fn create_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(data): Json<MockRuleCreate>,
) -> Result<Response, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    validate_rule(&state, &data)?;
    let data = MockRuleCreate {
        latency_ms: clamp_opt(data.latency_ms, 0, state.cfg.latency_max_ms),
        rate_limit_per_min: clamp_opt(data.rate_limit_per_min, 0, state.cfg.rate_limit_max_per_min),
        ..data
    };
    let row = rules::insert(&state.pool, &token, &data).await?;
    state.rule_cache.invalidate(&token);
    Ok((StatusCode::CREATED, Json(MockRule::from_row(&row))).into_response())
}

// --- 9. GET /api/endpoints/{token}/rules/{id} --------------------------------

pub(super) async fn get_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Json<MockRule>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let row = rules::find(&state.pool, &token, rule_id)
        .await?
        .ok_or_else(rule_not_found)?;
    Ok(Json(MockRule::from_row(&row)))
}

// --- 10. PATCH /api/endpoints/{token}/rules/{id} -----------------------------

pub(super) async fn patch_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
    Json(raw): Json<Value>,
) -> Result<Json<MockRule>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let current = rules::find(&state.pool, &token, rule_id)
        .await?
        .ok_or_else(rule_not_found)?;
    let obj = raw
        .as_object()
        .ok_or_else(|| ApiError::validation("Request body must be a JSON object."))?;

    let mut sets: Vec<String> = Vec::new();
    let mut binds: Vec<Value> = Vec::new();

    if let Some(v) = obj.get("name") {
        sets.push("name = ?".into());
        binds.push(v.clone());
    }
    if let Some(v) = obj.get("priority") {
        let n = v
            .as_i64()
            .ok_or_else(|| ApiError::validation("priority must be an integer."))?;
        if !(0..=100000).contains(&n) {
            return Err(ApiError::validation(
                "priority must be between 0 and 100000.",
            ));
        }
        sets.push("priority = ?".into());
        binds.push(json!(n));
    }
    if let Some(v) = obj.get("enabled") {
        let b = v
            .as_bool()
            .ok_or_else(|| ApiError::validation("enabled must be a boolean."))?;
        sets.push("enabled = ?".into());
        binds.push(json!(b as i64));
    }
    if let Some(v) = obj.get("match") {
        let m: MatchCriteria = serde_json::from_value(v.clone())
            .map_err(|_| ApiError::validation("match is malformed."))?;
        sets.push("match_json = ?".into());
        binds.push(json!(serde_json::to_string(&m).unwrap()));
    }
    if let Some(v) = obj.get("response") {
        let r: ResponseSpec = serde_json::from_value(v.clone())
            .map_err(|_| ApiError::validation("response is malformed."))?;
        if r.status_code < 100 || r.status_code > 599 {
            return Err(ApiError::validation(
                "response.status_code must be 100..599.",
            ));
        }
        sets.push("response_json = ?".into());
        binds.push(json!(serde_json::to_string(&r).unwrap()));
    }
    if let Some(v) = obj.get("state_writes") {
        let w: Vec<StateWrite> = serde_json::from_value(v.clone())
            .map_err(|_| ApiError::validation("state_writes is malformed."))?;
        sets.push("state_writes_json = ?".into());
        binds.push(json!(serde_json::to_string(&w).unwrap()));
    }
    if let Some(v) = obj.get("latency_ms") {
        let n = if v.is_null() {
            None
        } else {
            Some(
                v.as_i64()
                    .ok_or_else(|| ApiError::validation("latency_ms must be an integer."))?,
            )
        };
        sets.push("latency_ms = ?".into());
        binds.push(match clamp_opt(n, 0, state.cfg.latency_max_ms) {
            Some(x) => json!(x),
            None => Value::Null,
        });
    }
    if let Some(v) = obj.get("rate_limit_per_min") {
        let n =
            if v.is_null() {
                None
            } else {
                Some(v.as_i64().ok_or_else(|| {
                    ApiError::validation("rate_limit_per_min must be an integer.")
                })?)
            };
        sets.push("rate_limit_per_min = ?".into());
        binds.push(match clamp_opt(n, 0, state.cfg.rate_limit_max_per_min) {
            Some(x) => json!(x),
            None => Value::Null,
        });
    }
    if let Some(v) = obj.get("chaos_mode") {
        if !v.is_null() {
            let m = v.as_str().unwrap_or("");
            if m != "error" && m != "dropout" {
                return Err(ApiError::validation(
                    "chaos_mode must be 'error' or 'dropout'.",
                ));
            }
        }
        sets.push("chaos_mode = ?".into());
        binds.push(v.clone());
    }
    if let Some(v) = obj.get("webhook_action") {
        sets.push("webhook_json = ?".into());
        binds.push(match v {
            Value::Null => Value::Null,
            other => json!(other.to_string()),
        });
    }

    if sets.is_empty() {
        return Ok(Json(MockRule::from_row(&current)));
    }
    let sql = format!(
        "UPDATE mock_rules SET {} WHERE id = ? AND token = ?",
        sets.join(", ")
    );
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = bind_value(q, b);
    }
    q = q.bind(rule_id).bind(&token);
    q.execute(&state.pool).await?;
    state.rule_cache.invalidate(&token);

    let row = rules::find(&state.pool, &token, rule_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    Ok(Json(MockRule::from_row(&row)))
}

// --- 11. DELETE /api/endpoints/{token}/rules/{id} (204, no body) -------------

pub(super) async fn delete_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    if !rules::delete(&state.pool, &token, rule_id).await? {
        return Err(rule_not_found());
    }
    state.rule_cache.invalidate(&token);
    Ok(StatusCode::NO_CONTENT.into_response()) // 204 carries no body
}
