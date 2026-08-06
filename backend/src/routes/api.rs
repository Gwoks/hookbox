//! Management API (P2) — the 18 §5.2 endpoints with owner-capability auth.
//! PORT of `app/routes/api.py`. All responses are JSON; error bodies are the
//! flat `{"error":<code>,"detail":<human>}` envelope. Endpoint-scoped routes
//! return 404 (not 403) for a valid-but-non-owner capability (AC-S2/S3).
//!
//! The §5.3 `chaos_mode` (OQ-2) is a real column on endpoints + a nullable
//! per-rule override, and joins every endpoint/rule shape.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::helpers::{is_safe_key, validate_target_url};
use crate::ids::{gen_owner_secret, gen_token, hash_email, hash_secret};
use crate::models::*;
use crate::state::AppState;

// --- serialization helpers ---------------------------------------------------

fn mock_url(state: &AppState, token: &str) -> String {
    if state.cfg.path_fallback_only {
        // PUBLIC_BASE_URL makes this copy-pasteable (e.g.
        // `https://hookbox.example.com/e/<token>`); blank keeps it relative.
        format!("{}/e/{token}", state.cfg.public_base_url)
    } else {
        format!("https://{token}.{}", state.cfg.mock_domain)
    }
}

fn path_url(state: &AppState, token: &str) -> String {
    format!("{}/e/{token}", state.cfg.public_base_url)
}

/// Normalize a stored TEXT timestamp to an RFC3339 string. SQLite
/// `datetime('now')` yields `YYYY-MM-DD HH:MM:SS` (UTC, no zone); the contract
/// is RFC3339 UTC, so we present the `T`-separated `...Z` form.
fn to_rfc3339(value: Option<String>) -> Option<String> {
    let v = value?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    if v.contains('T') {
        return Some(v.to_string());
    }
    // "2026-06-21 12:00:00" -> "2026-06-21T12:00:00Z"
    Some(format!("{}Z", v.replacen(' ', "T", 1)))
}

fn endpoint_summary(state: &AppState, row: &sqlx::sqlite::SqliteRow) -> EndpointSummary {
    let token: String = row.get("token");
    EndpointSummary {
        mock_url: mock_url(state, &token),
        path_url: path_url(state, &token),
        name: row.get("name"),
        created_at: to_rfc3339(row.get("created_at")).unwrap_or_default(),
        last_hit: to_rfc3339(row.get("last_hit")),
        request_count: row.get("request_count"),
        token,
    }
}

fn endpoint_detail(
    state: &AppState,
    row: &sqlx::sqlite::SqliteRow,
    tunnel_active: bool,
) -> EndpointDetail {
    let token: String = row.get("token");
    EndpointDetail {
        mock_url: mock_url(state, &token),
        path_url: path_url(state, &token),
        name: row.get("name"),
        auto_crud: row.get::<i64, _>("auto_crud") != 0,
        target_url: row.get("target_url"),
        default_mode: row.get("default_mode"),
        latency_ms: row.get("latency_ms"),
        rate_limit_per_min: row.get("rate_limit_per_min"),
        chaos_pct: row.get("chaos_pct"),
        chaos_mode: row.get("chaos_mode"),
        cors_enabled: row.get::<i64, _>("cors_enabled") != 0,
        tunnel_active,
        created_at: to_rfc3339(row.get("created_at")).unwrap_or_default(),
        last_hit: to_rfc3339(row.get("last_hit")),
        request_count: row.get("request_count"),
        token,
    }
}

fn parse_json_value(s: Option<String>, fallback: Value) -> Value {
    match s {
        Some(t) if !t.is_empty() => serde_json::from_str(&t).unwrap_or(fallback),
        _ => fallback,
    }
}

fn rule_from_row(row: &sqlx::sqlite::SqliteRow) -> MockRule {
    let webhook: Option<String> = row.get("webhook_json");
    MockRule {
        id: row.get("id"),
        token: row.get("token"),
        name: row.get("name"),
        priority: row.get("priority"),
        enabled: row.get::<i64, _>("enabled") != 0,
        match_: parse_json_value(row.get("match_json"), json!({})),
        response: parse_json_value(row.get("response_json"), json!({})),
        state_writes: parse_json_value(row.get("state_writes_json"), json!([])),
        latency_ms: row.get("latency_ms"),
        rate_limit_per_min: row.get("rate_limit_per_min"),
        chaos_mode: row.get("chaos_mode"),
        webhook_action: webhook.and_then(|w| serde_json::from_str(&w).ok()),
        created_at: to_rfc3339(row.get("created_at")).unwrap_or_default(),
    }
}

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

/// Insert a fresh endpoint for an owner, retrying on the (astronomically rare)
/// token collision. Returns the new token.
async fn new_endpoint(
    state: &AppState,
    owner_id: &str,
    name: Option<&str>,
) -> Result<String, ApiError> {
    let mut token = gen_token(state.cfg.endpoint_id_length);
    for _ in 0..5 {
        let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM endpoints WHERE token = ?")
            .bind(&token)
            .fetch_optional(&state.pool)
            .await?;
        if exists.is_none() {
            break;
        }
        token = gen_token(state.cfg.endpoint_id_length);
    }
    sqlx::query("INSERT INTO endpoints (token, owner_id, name) VALUES (?, ?, ?)")
        .bind(&token)
        .bind(owner_id)
        .bind(name)
        .execute(&state.pool)
        .await?;
    Ok(token)
}

async fn fetch_endpoint(
    state: &AppState,
    token: &str,
) -> Result<sqlx::sqlite::SqliteRow, ApiError> {
    sqlx::query("SELECT * FROM endpoints WHERE token = ?")
        .bind(token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Endpoint not found."))
}

// --- 1. POST /api/session (no auth) ------------------------------------------

#[derive(Deserialize)]
struct SessionBody {
    email: Option<String>,
}

/// Minimal RFC-lite email check (the route only needs to reject obvious
/// non-emails with 422; the durable identity is `hash_email`).
fn valid_email(email: &str) -> bool {
    let e = email.trim();
    if e.len() < 3 || e.len() > 254 {
        return false;
    }
    let mut parts = e.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !e.contains(' ')
}

async fn create_session(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<SessionBody>,
) -> Result<Response, ApiError> {
    // Per-source anti-enumeration rate limit (AC-S5); fails open.
    let client_ip = connect_info
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    if let Some(retry) = session_rate_limited(&state, &client_ip) {
        return Ok(ApiError::rate_limited("Too many session requests.", retry).into_response());
    }

    let email = body
        .email
        .ok_or_else(|| ApiError::validation("A valid email is required."))?;
    if !valid_email(&email) {
        return Err(ApiError::validation("A valid email is required."));
    }
    let email_norm = email.trim().to_lowercase();
    let owner_id = hash_email(&email);
    let new_secret = gen_owner_secret(state.cfg.owner_secret_bytes);
    let secret_hash = hash_secret(&new_secret);

    // Upsert owner, rotating the secret (same shape new vs existing — AC-S5).
    sqlx::query(
        "INSERT INTO owners (owner_id, email, secret_hash, last_seen)
         VALUES (?, ?, ?, datetime('now'))
         ON CONFLICT(owner_id) DO UPDATE SET
             secret_hash = excluded.secret_hash,
             last_seen = datetime('now')",
    )
    .bind(&owner_id)
    .bind(&email_norm)
    .bind(&secret_hash)
    .execute(&state.pool)
    .await?;

    let mut rows =
        sqlx::query("SELECT * FROM endpoints WHERE owner_id = ? ORDER BY created_at, token")
            .bind(&owner_id)
            .fetch_all(&state.pool)
            .await?;
    if rows.is_empty() {
        let token = new_endpoint(&state, &owner_id, None).await?;
        rows = sqlx::query("SELECT * FROM endpoints WHERE token = ?")
            .bind(&token)
            .fetch_all(&state.pool)
            .await?;
    }

    let summaries: Vec<EndpointSummary> =
        rows.iter().map(|r| endpoint_summary(&state, r)).collect();
    let primary = endpoint_summary(&state, &rows[0]);
    let resp = SessionResponse {
        owner_id,
        owner_secret: new_secret,
        endpoints: summaries,
        primary,
    };
    Ok((StatusCode::OK, Json(resp)).into_response())
}

/// Minimal per-IP fixed-window session limiter (fail-open). Superseded by the
/// shared DashMap token bucket in sks.20; kept here so the §5.2 #1 429 contract
/// holds. Returns `Some(retry_after_secs)` when over the limit.
fn session_rate_limited(state: &AppState, ip: &str) -> Option<i64> {
    use dashmap::DashMap;
    use std::sync::OnceLock;
    static BUCKETS: OnceLock<DashMap<String, (u64, i64)>> = OnceLock::new();
    let buckets = BUCKETS.get_or_init(DashMap::new);
    let limit = state.cfg.session_rate_limit_per_min;
    if limit <= 0 {
        return None;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let window = now / 60;
    let mut entry = buckets.entry(ip.to_string()).or_insert((window, 0));
    if entry.0 != window {
        *entry = (window, 0);
    }
    entry.1 += 1;
    if entry.1 > limit {
        let retry = 60 - (now % 60) as i64;
        Some(retry.max(1))
    } else {
        None
    }
}

// --- 2. GET /api/endpoints ----------------------------------------------------

async fn list_endpoints(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
) -> Result<Json<Vec<EndpointSummary>>, ApiError> {
    let rows = sqlx::query("SELECT * FROM endpoints WHERE owner_id = ? ORDER BY created_at, token")
        .bind(&owner_id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(
        rows.iter().map(|r| endpoint_summary(&state, r)).collect(),
    ))
}

// --- 3. POST /api/endpoints ---------------------------------------------------

async fn create_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Json(body): Json<EndpointCreate>,
) -> Result<Response, ApiError> {
    if let Some(n) = &body.name {
        if n.chars().count() > 100 {
            return Err(ApiError::validation("name must be at most 100 characters."));
        }
    }
    let token = new_endpoint(&state, &owner_id, body.name.as_deref()).await?;
    let row = fetch_endpoint(&state, &token).await?;
    let active = state.tunnels.is_active(&token);
    Ok((
        StatusCode::CREATED,
        Json(endpoint_detail(&state, &row, active)),
    )
        .into_response())
}

// --- 4. GET /api/endpoints/{token} -------------------------------------------

async fn get_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<EndpointDetail>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let row = fetch_endpoint(&state, &token).await?;
    let active = state.tunnels.is_active(&token);
    Ok(Json(endpoint_detail(&state, &row, active)))
}

// --- 5. PATCH /api/endpoints/{token} -----------------------------------------

async fn patch_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(raw): Json<Value>,
) -> Result<Json<EndpointDetail>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
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

    if !sets.is_empty() {
        let sql = format!("UPDATE endpoints SET {} WHERE token = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        for b in &binds {
            q = bind_value(q, b);
        }
        q = q.bind(&token);
        q.execute(&state.pool).await?;
        state.rule_cache.invalidate(&token);
    }

    let row = fetch_endpoint(&state, &token).await?;
    let active = state.tunnels.is_active(&token);
    Ok(Json(endpoint_detail(&state, &row, active)))
}

/// Bind a JSON scalar onto a query (string/int/bool/null). Used by the
/// allow-listed PATCH builders.
fn bind_value<'q>(
    q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    v: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match v {
        Value::Null => q.bind(Option::<String>::None),
        Value::Bool(b) => q.bind(*b as i64),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else {
                q.bind(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => q.bind(s.as_str()),
        other => q.bind(other.to_string()),
    }
}

// --- 6. DELETE /api/endpoints/{token} (tombstone) ----------------------------

async fn delete_endpoint(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    // OQ-1: tombstone rather than hard-delete so the mock plane returns 410.
    // Clear live config (rules/state/crud cascade or empty) and set gone_at.
    sqlx::query("DELETE FROM mock_rules WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM endpoint_state WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM crud_collections WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE endpoints SET gone_at = datetime('now') WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    state.rule_cache.invalidate(&token);
    Ok(Json(Message::new("Endpoint deleted.")))
}

// --- 7. GET /api/endpoints/{token}/rules -------------------------------------

async fn list_rules(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Vec<MockRule>>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let rows = sqlx::query("SELECT * FROM mock_rules WHERE token = ? ORDER BY priority, id")
        .bind(&token)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows.iter().map(rule_from_row).collect()))
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

async fn create_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(data): Json<MockRuleCreate>,
) -> Result<Response, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    validate_rule(&state, &data)?;
    let latency = clamp_opt(data.latency_ms, 0, state.cfg.latency_max_ms);
    let rate = clamp_opt(data.rate_limit_per_min, 0, state.cfg.rate_limit_max_per_min);
    let match_json = serde_json::to_string(&data.r#match).unwrap();
    let response_json = serde_json::to_string(&data.response).unwrap();
    let writes_json = serde_json::to_string(&data.state_writes).unwrap();
    let webhook_json = data
        .webhook_action
        .as_ref()
        .map(|w| serde_json::to_string(w).unwrap());

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO mock_rules
            (token, name, priority, enabled, match_json, response_json,
             state_writes_json, latency_ms, rate_limit_per_min, chaos_mode, webhook_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&token)
    .bind(&data.name)
    .bind(data.priority)
    .bind(data.enabled as i64)
    .bind(&match_json)
    .bind(&response_json)
    .bind(&writes_json)
    .bind(latency)
    .bind(rate)
    .bind(&data.chaos_mode)
    .bind(&webhook_json)
    .fetch_one(&state.pool)
    .await?;

    state.rule_cache.invalidate(&token);
    let row = sqlx::query("SELECT * FROM mock_rules WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(rule_from_row(&row))).into_response())
}

// --- 9. GET /api/endpoints/{token}/rules/{id} --------------------------------

async fn get_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Json<MockRule>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let row = sqlx::query("SELECT * FROM mock_rules WHERE id = ? AND token = ?")
        .bind(rule_id)
        .bind(&token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Rule not found."))?;
    Ok(Json(rule_from_row(&row)))
}

// --- 10. PATCH /api/endpoints/{token}/rules/{id} -----------------------------

async fn patch_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
    Json(raw): Json<Value>,
) -> Result<Json<MockRule>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM mock_rules WHERE id = ? AND token = ?")
            .bind(rule_id)
            .bind(&token)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("Rule not found."));
    }
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

    if !sets.is_empty() {
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
    }

    let row = sqlx::query("SELECT * FROM mock_rules WHERE id = ?")
        .bind(rule_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(rule_from_row(&row)))
}

// --- 11. DELETE /api/endpoints/{token}/rules/{id} (204, no body) -------------

async fn delete_rule(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, rule_id)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM mock_rules WHERE id = ? AND token = ?")
            .bind(rule_id)
            .bind(&token)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("Rule not found."));
    }
    sqlx::query("DELETE FROM mock_rules WHERE id = ? AND token = ?")
        .bind(rule_id)
        .bind(&token)
        .execute(&state.pool)
        .await?;
    state.rule_cache.invalidate(&token);
    Ok(StatusCode::NO_CONTENT.into_response()) // 204 carries no body
}

// --- 12. GET /api/endpoints/{token}/requests?limit&offset --------------------

#[derive(Deserialize)]
struct RequestsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_requests(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Query(q): Query<RequestsQuery>,
) -> Result<Json<Vec<RequestSummary>>, ApiError> {
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    if !(1..=200).contains(&limit) {
        return Err(ApiError::validation("limit must be between 1 and 200."));
    }
    if offset < 0 {
        return Err(ApiError::validation("offset must be >= 0."));
    }
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    let rows =
        sqlx::query("SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(&token)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(rows.iter().map(request_summary).collect()))
}

// --- 13. GET /api/requests/{id} ----------------------------------------------

async fn get_request(
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
        request_headers: parse_json_value(row.get("request_headers"), json!({})),
        query_params: parse_json_value(row.get("query_params"), json!({})),
        request_body: row.get("request_body"),
        response_headers: parse_json_value(row.get("response_headers"), json!({})),
        response_body: row.get("response_body"),
        trace: parse_json_value(row.get("trace_json"), json!([])),
        state_snapshot: parse_json_value(row.get("state_snapshot"), json!({})),
    };
    Ok(Json(detail))
}

// --- 14. DELETE /api/endpoints/{token}/requests ------------------------------

async fn clear_requests(
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

// --- 15. GET /api/endpoints/{token}/state ------------------------------------

async fn get_state(
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

async fn clear_state(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Message>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    sqlx::query("DELETE FROM endpoint_state WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await?;
    Ok(Json(Message::new("State cleared.")))
}

// --- 17. GET /api/endpoints/{token}/collections/{name} -----------------------

async fn peek_collection(
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
    let parsed = parse_json_value(items, json!([]));
    Ok(Json(json!({ "items": parsed })))
}

// --- 18. DELETE /api/endpoints/{token}/collections/{name} --------------------

async fn clear_collection(
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

/// Build the `/api` management router (18 routes). Mounted under the P2 plane by
/// `router::build_app`. `POST /api/session` is the only no-auth route; all
/// others use the `OwnerId` extractor (401 on missing/unknown Bearer).
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/api/session", post(create_session))
        .route("/api/endpoints", get(list_endpoints).post(create_endpoint))
        .route(
            "/api/endpoints/:token",
            get(get_endpoint)
                .patch(patch_endpoint)
                .delete(delete_endpoint),
        )
        .route(
            "/api/endpoints/:token/rules",
            get(list_rules).post(create_rule),
        )
        .route(
            "/api/endpoints/:token/rules/:id",
            get(get_rule).patch(patch_rule).delete(delete_rule),
        )
        .route(
            "/api/endpoints/:token/requests",
            get(list_requests).delete(clear_requests),
        )
        .route("/api/requests/:id", get(get_request))
        .route(
            "/api/endpoints/:token/state",
            get(get_state).delete(clear_state),
        )
        .route(
            "/api/endpoints/:token/collections/:name",
            get(peek_collection).delete(clear_collection),
        )
}
