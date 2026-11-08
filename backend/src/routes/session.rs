//! `POST /api/session` (§5.2 #1) — the only unauthenticated management
//! route besides the public share resolver: upsert the owner for an email,
//! rotate its secret, and return it with the owner's endpoints (minting a
//! primary endpoint on first sign-in). Rate limited per source (AC-S5).

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::ids::{gen_owner_secret, hash_email, hash_secret};
use crate::models::{EndpointSummary, SessionResponse};
use crate::net::client_ip::effective_client_ip;
use crate::routes::endpoints::{endpoint_summary, new_endpoint};
use crate::state::AppState;

// --- 1. POST /api/session (no auth) ------------------------------------------

#[derive(Deserialize)]
pub(super) struct SessionBody {
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

pub(super) async fn create_session(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<SessionBody>,
) -> Result<Response, ApiError> {
    // Per-source anti-enumeration rate limit (AC-S5); fails open.
    let client_ip = effective_client_ip(connect_info, &headers);
    if let Some(retry) = session_rate_limited(&state, &client_ip) {
        return Err(ApiError::rate_limited("Too many session requests.", retry));
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
