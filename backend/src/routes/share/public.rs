//! The PUBLIC, unauthenticated share resolver (§5.2): the frozen
//! rate-limit -> parameter-validation -> code-shape -> code-resolution order,
//! one byte-identical 404 for every negative outcome, `no-store` on every
//! response, and a coalesced off-path `last_used_at` touch.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use sqlx::{Row, SqlitePool};

use super::no_store;
use super::projection::{public_request_detail, public_request_summary};
use crate::error::ApiError;
use crate::ids::hash_secret;
use crate::models::{
    PublicEndpointInfo, PublicRequestDetail, PublicRequestSummary, PublicShareFeed,
};
use crate::net::client_ip::effective_client_ip;
use crate::state::AppState;
use crate::util::time::to_rfc3339;

/// The rate-limit namespace, and the global-ceiling bucket key inside it
/// (AC-S7): every per-IP key is `share:<ip>`; `share:__global__` cannot
/// collide with a real IP literal, but still lives in the same `share:`
/// namespace so both are subject to the limiter's one shared eviction bound.
const SHARE_RATE_LIMIT_GLOBAL_KEY: &str = "share:__global__";

/// The ONLY 404 the public resolver may emit. Unknown code, revoked code,
/// tombstoned endpoint, unknown request id and cross-endpoint request id all
/// return this exact value, so a scanner learns nothing from the difference
/// (AC-36).
fn share_not_found() -> ApiError {
    ApiError::not_found("This share link is not available.")
        .with_header("cache-control", "no-store")
}

/// base64url-no-pad shape gate (no `regex` dependency in this crate).
/// `SHARE_CODE_BYTES` is env-tunable, so accept the whole plausible band
/// rather than one exact length: 32 chars = the 24-byte default, 64 chars
/// covers up to 48 bytes.
fn is_share_code_shape(code: &str) -> bool {
    let n = code.len(); // ASCII-only charset ⇒ len() == char count
    (32..=64).contains(&n)
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Rate limit `share:<ip>` per-IP, THEN the instance-wide ceiling — both
/// BEFORE any DB read (§5.2 step 1). The `share:` prefix keeps this key space
/// (attacker-controlled, since it is unauthenticated) from ever colliding
/// with or evicting the mock plane's `rl:<token>` buckets (AC-S7).
fn check_share_rate_limit(state: &AppState, ip: &str) -> Result<(), ApiError> {
    let per_ip = state.limiter.check(
        &format!("share:{ip}"),
        state.cfg.share_rate_limit_per_min,
        60,
    );
    if !per_ip.allowed {
        return Err(ApiError::rate_limited(
            "Too many requests. Please slow down.",
            per_ip.retry_after,
        ));
    }
    let global = state.limiter.check(
        SHARE_RATE_LIMIT_GLOBAL_KEY,
        state.cfg.share_rate_limit_global_per_min,
        60,
    );
    if !global.allowed {
        return Err(ApiError::rate_limited(
            "Too many requests. Please slow down.",
            global.retry_after,
        ));
    }
    Ok(())
}

/// Best-effort, coalesced, off the response path (§5.2's `last_used_at`
/// recipe) — an unauthenticated GET can never contend on the single-writer
/// WAL lock in front of a viewer's response (AC-S10).
fn touch_last_used(state: &AppState, share_id: i64) {
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE share_links SET last_used_at = datetime('now')
              WHERE id = ? AND (last_used_at IS NULL
                                OR last_used_at < datetime('now','-60 seconds'))",
        )
        .bind(share_id)
        .execute(&pool)
        .await;
    });
}

/// The resolved liveness-checked share row (§5.2's resolution statement).
struct ResolvedShare {
    share_id: i64,
    token: String,
    endpoint_name: Option<String>,
    endpoint_created_at: Option<String>,
    request_count: i64,
}

/// One round trip, joining liveness so a tombstone is indistinguishable from
/// an unknown/revoked code (AC-S9): `s.revoked_at IS NULL AND e.gone_at IS
/// NULL`, both read fresh on every request (never cached — `rule_cache` is
/// keyed by token for the mock plane only and gets no share entries, which is
/// what makes revocation take effect on the very next request, AC-37).
async fn resolve_share(pool: &SqlitePool, code: &str) -> Result<Option<ResolvedShare>, ApiError> {
    let code_hash = hash_secret(code); // sha256(code) — never the code itself
    let row = sqlx::query(
        "SELECT s.id, s.token, e.name, e.created_at AS endpoint_created_at, e.request_count
           FROM share_links s
           JOIN endpoints  e ON e.token = s.token
          WHERE s.code_hash = ?
            AND s.revoked_at IS NULL
            AND e.gone_at   IS NULL",
    )
    .bind(&code_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| ResolvedShare {
        share_id: r.get("id"),
        token: r.get("token"),
        endpoint_name: r.get("name"),
        endpoint_created_at: r.get("endpoint_created_at"),
        request_count: r.get("request_count"),
    }))
}

// === #22 GET /api/share/{code}/requests?limit&offset (PUBLIC) ================

struct PublicRequestsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

/// Parse `limit`/`offset` out of the RAW `?key=value` map (hookbox-mun.41).
/// The route used to take `Query<PublicRequestsQuery>` directly as an axum
/// extractor argument — but a fallible typed extractor runs BEFORE the
/// handler body, so `?limit=abc` 400'd at the axum layer and
/// `check_share_rate_limit` never ran at all, making the only unauthenticated
/// surface unmetered for any malformed query. Taking
/// `Query<HashMap<String, String>>` instead can never fail to extract (every
/// query string parses into string→string pairs), so the rate limit check in
/// `public_list_requests_inner` below always runs first; a non-numeric value
/// now fails HERE, inside the handler, with the SAME validation error as an
/// out-of-range one — consistent, and counted against the limiter either way.
fn parse_public_requests_query(
    raw: &std::collections::HashMap<String, String>,
) -> Result<PublicRequestsQuery, ApiError> {
    let limit = match raw.get("limit") {
        None => None,
        Some(s) => Some(
            s.trim()
                .parse::<i64>()
                .map_err(|_| ApiError::validation("limit must be between 1 and 200."))?,
        ),
    };
    let offset = match raw.get("offset") {
        None => None,
        Some(s) => Some(
            s.trim()
                .parse::<i64>()
                .map_err(|_| ApiError::validation("offset must be >= 0."))?,
        ),
    };
    Ok(PublicRequestsQuery { limit, offset })
}

async fn public_list_requests_inner(
    state: &AppState,
    ip: &str,
    code: &str,
    raw_q: &std::collections::HashMap<String, String>,
) -> Result<Json<PublicShareFeed>, ApiError> {
    // 1. Rate limit — before any DB read, and before `limit`/`offset` are
    // even parsed (hookbox-mun.41): a malformed query must still be counted.
    check_share_rate_limit(state, ip)?;

    // 2. Parameter validation MUST precede code resolution (AC-101): if
    // `limit` were validated after resolving the code, `?limit=999` would
    // return 422 for a live code and 404 for a dead one — a boolean
    // existence oracle that defeats AC-36.
    let q = parse_public_requests_query(raw_q)?;
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    if !(1..=200).contains(&limit) {
        return Err(ApiError::validation("limit must be between 1 and 200."));
    }
    if offset < 0 {
        return Err(ApiError::validation("offset must be >= 0."));
    }

    // 3. Code shape — no DB read on a malformed code.
    if !is_share_code_shape(code) {
        return Err(share_not_found());
    }

    // 4. Code resolution.
    let resolved = resolve_share(&state.pool, code)
        .await?
        .ok_or_else(share_not_found)?;

    let rows =
        sqlx::query("SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(&resolved.token)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;
    let requests: Vec<PublicRequestSummary> = rows.iter().map(public_request_summary).collect();

    let resp = PublicShareFeed {
        endpoint: PublicEndpointInfo {
            name: resolved.endpoint_name.clone(),
            created_at: to_rfc3339(resolved.endpoint_created_at.clone()).unwrap_or_default(),
            request_count: resolved.request_count,
        },
        requests,
    };

    // 6. Fire-and-forget, AFTER the response value is built.
    touch_last_used(state, resolved.share_id);

    Ok(Json(resp))
}

pub(crate) async fn public_list_requests(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Path(code): Path<String>,
    Query(raw_q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let ip = effective_client_ip(connect_info, &headers);
    match public_list_requests_inner(&state, &ip, &code, &raw_q).await {
        Ok(v) => no_store(v.into_response()),
        Err(e) => no_store(e.into_response()),
    }
}

// === #23 GET /api/share/{code}/requests/{id} (PUBLIC) =========================

async fn public_get_request_inner(
    state: &AppState,
    ip: &str,
    code: &str,
    request_id_raw: &str,
) -> Result<Json<PublicRequestDetail>, ApiError> {
    // 1. Rate limit — before any DB read, and before the id is even parsed
    // (hookbox-mun.41's same root cause applied here too: `Path<(String,
    // i64)>` is a fallible extractor that used to run BEFORE this handler,
    // so a non-numeric `{id}` 400'd at the axum layer and never reached
    // `check_share_rate_limit`). Taking the id as a raw `String` instead
    // means this route can never fail to extract, so the rate limit check
    // always runs first.
    check_share_rate_limit(state, ip)?;

    // A non-numeric id can never resolve to a real row — collapsed into the
    // SAME 404 as an unknown/cross-endpoint one (AC-36's one negative
    // outcome), not a distinct error that would tell a scanner it guessed a
    // syntactically-plausible id.
    let request_id: i64 = request_id_raw.parse().map_err(|_| share_not_found())?;

    // 3. Code shape (no `limit`/`offset`, so no step 2 here).
    if !is_share_code_shape(code) {
        return Err(share_not_found());
    }

    // 4. Code resolution.
    let resolved = resolve_share(&state.pool, code)
        .await?
        .ok_or_else(share_not_found)?;

    // The `AND token = ?` (scoped to the share's OWN endpoint, resolved from
    // the code, never from the path) is the whole of AC-35: cross-endpoint
    // trace enumeration is impossible because the id is scoped inside the
    // same statement.
    let row = sqlx::query("SELECT * FROM request_logs WHERE id = ? AND token = ?")
        .bind(request_id)
        .bind(&resolved.token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(share_not_found)?;
    let detail = public_request_detail(&row, &resolved.token);

    touch_last_used(state, resolved.share_id);

    Ok(Json(detail))
}

pub(crate) async fn public_get_request(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Path((code, request_id_raw)): Path<(String, String)>,
) -> Response {
    let ip = effective_client_ip(connect_info, &headers);
    match public_get_request_inner(&state, &ip, &code, &request_id_raw).await {
        Ok(v) => no_store(v.into_response()),
        Err(e) => no_store(e.into_response()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn share_code_shape_accepts_the_default_band() {
        let code = "a".repeat(32);
        assert!(is_share_code_shape(&code));
        let code64 = "A1_-".repeat(16); // 64 chars, mixed charset
        assert!(is_share_code_shape(&code64));
        assert!(!is_share_code_shape(&"a".repeat(31))); // too short
        assert!(!is_share_code_shape(&"a".repeat(65))); // too long
        assert!(!is_share_code_shape(&format!("{}!", "a".repeat(31)))); // bad char
    }

    #[test]
    fn share_not_found_carries_no_store() {
        let e = share_not_found();
        assert_eq!(e.status, StatusCode::NOT_FOUND);
        assert!(e
            .headers
            .iter()
            .any(|(n, v)| n.as_str() == "cache-control" && v == "no-store"));
    }
}
