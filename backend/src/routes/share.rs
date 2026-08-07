//! F4 share links — the whole unauthenticated attack surface in one file.
//!
//! Two trust boundaries, five routes:
//!   * Owner-authenticated (§5.1): mint / list / revoke, under
//!     `/api/endpoints/{token}/shares[/{id}]`, gated by the `OwnerId`
//!     extractor exactly like every other management route.
//!   * PUBLIC, no authentication at all (§5.2): `/api/share/{code}/requests`
//!     and `/api/share/{code}/requests/{id}`. Together with `POST
//!     /api/session` these are the ONLY unauthenticated routes in HookBox.
//!
//! **Invariants that make the public half safe:**
//!   * **No mutation.** Both public handlers only ever `SELECT` (plus one
//!     fire-and-forget `last_used_at` touch off the response path). Nothing a
//!     viewer can send changes any row a viewer can read.
//!   * **No owner secret, no token, ever.** The public projections
//!     (`PublicRequestSummary`/`PublicRequestDetail`/`PublicShareFeed`) are
//!     STANDALONE structs built field-by-field in this file — never
//!     `#[serde(flatten)]`ed off an owner struct, never `#[serde(skip)]`ed off
//!     `RequestDetail` — so a future owner-shape field cannot leak here by
//!     default (AC-34, AC-102). This is a LIVE-CAPABILITY boundary, not just a
//!     naming one: recovering the token from a share link hands a viewer
//!     write access to `/e/<token>` (AC-S2, AC-43, hookbox-mun.34), so
//!     `filter_public_request_headers`/`filter_public_response_headers` also
//!     strip the structural `host`/`origin`/`referer` request headers (the
//!     wildcard mock host IS `<token>.<MOCK_DOMAIN>`) and mask any remaining
//!     header value that contains the resolved token, e.g. a CORS
//!     `access-control-allow-origin` echo of that same wildcard `Origin`.
//!     `response_body` itself is passed through as stored and NOT re-filtered
//!     here (§5.11) — the third channel (hookbox-mun.36) was a
//!     `default_mode = "echo"` row's persisted `headers` sub-object carrying
//!     the same host/origin/referer verbatim; that is closed on the persist
//!     path instead, in `interceptor::engine::redact_echo_persisted_headers`,
//!     so this file's projection never needs to know about body-shaped data.
//!   * **One 404 for every negative outcome.** `share_not_found()` is the
//!     ONLY 404 the public resolver may emit — unknown code, revoked code,
//!     tombstoned endpoint, unknown request id and cross-endpoint request id
//!     are byte-identical (AC-36), so a scanner learns nothing from the
//!     difference.
//!   * **The frozen check order is load-bearing.** Rate limit, THEN parameter
//!     validation, THEN code-shape, THEN code resolution — in that order, on
//!     every public request, before any other work (§5.2). Validating
//!     parameters before resolving the code is what stops `?limit=999` from
//!     being a live/dead code oracle (AC-101).
//!   * **The code never appears in a log.** No `tracing::*` call in this file
//!     receives the code or the full path; log `share_links.id` instead
//!     (AC-S6, AC-S16).

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::auth::{assert_owns_endpoint, OwnerId};
use crate::error::ApiError;
use crate::ids::{gen_share_code, hash_secret};
use crate::models::{
    PublicEndpointInfo, PublicRequestDetail, PublicRequestSummary, PublicShareFeed, ShareLink,
    ShareLinkCreate, ShareLinkCreated,
};
use crate::routes::api::effective_client_ip;
use crate::state::AppState;

/// Dropped entirely from the PUBLIC projection: internal markers that identify
/// the endpoint or the matched rule. Prefix rule, not an enumeration, so a
/// future x-hookbox-* header cannot leak by omission. (AC-S1)
const PUBLIC_RESPONSE_HEADER_DROP_PREFIX: &str = "x-hookbox-";

/// Value replaced with "<redacted>" in the PUBLIC projection: credential-bearing
/// response headers. Key is KEPT so the viewer can see one was sent. (AC-S1)
const PUBLIC_RESPONSE_HEADER_REDACT: [&str; 5] = [
    "set-cookie",
    "set-cookie2",
    "authorization",
    "proxy-authenticate",
    "www-authenticate",
];

/// Dropped entirely from the PUBLIC projection's `request_headers` (AC-43,
/// AC-S2, hookbox-mun.34): in wildcard mock-host mode the `Host` header IS
/// `<token>.<MOCK_DOMAIN>` on EVERY row, and `Origin`/`Referer` echo it back
/// on a cross-origin browser request. Unlike `REDACT_HEADERS`
/// (`helpers.rs`) — a capability scrubber applied at persist time — this is
/// a token-disclosure scrubber applied ONLY at the public projection;
/// storage and the owner Inspector keep these headers verbatim (§5.11).
const PUBLIC_REQUEST_HEADER_DROP: [&str; 3] = ["host", "origin", "referer"];

/// Mask `value` to `"<redacted>"` iff it is a JSON string containing `token`
/// verbatim — the second half of the AC-S2/AC-43 fix, applied to BOTH
/// `request_headers` and `response_headers` in the PUBLIC projection so a
/// server-generated echo of the token (e.g. a CORS
/// `access-control-allow-origin` reflecting the wildcard mock `Origin`)
/// cannot survive the name-based filters above. Never applied to `path`,
/// `query_params`, or either body — those are caller-supplied and a caller
/// choosing to paste the token into their own body is out of scope (AC-S2).
fn mask_token_in_value(token: &str, value: Value) -> Value {
    match &value {
        Value::String(s) if !token.is_empty() && s.contains(token) => json!("<redacted>"),
        _ => value,
    }
}

/// The rate-limit namespace, and the global-ceiling bucket key inside it
/// (AC-S7): every per-IP key is `share:<ip>`; `share:__global__` cannot
/// collide with a real IP literal, but still lives in the same `share:`
/// namespace so both are subject to the limiter's one shared eviction bound.
const SHARE_RATE_LIMIT_GLOBAL_KEY: &str = "share:__global__";

// --- small local helpers (deliberately NOT reused from routes::api, which
// keeps this file's two upstream edits to exactly the pub(crate) visibility
// change + the tombstone statement) --------------------------------------

/// Normalize a stored TEXT timestamp to an RFC3339 string, mirroring
/// `routes::api::to_rfc3339`. SQLite `datetime('now')` yields
/// `YYYY-MM-DD HH:MM:SS` (UTC, no zone); the contract is RFC3339 UTC.
fn to_rfc3339(value: Option<String>) -> Option<String> {
    let v = value?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    if v.contains('T') {
        return Some(v.to_string());
    }
    Some(format!("{}Z", v.replacen(' ', "T", 1)))
}

fn parse_json_value(s: Option<String>, fallback: Value) -> Value {
    match s {
        Some(t) if !t.is_empty() => serde_json::from_str(&t).unwrap_or(fallback),
        _ => fallback,
    }
}

/// `url` is built from `PUBLIC_BASE_URL` only — never `mock_url`'s wildcard
/// form, never `Host`/`X-Forwarded-Host` (AC-99, AC-S14). A share URL on the
/// mock host would be swallowed by the interceptor and 404 as an unknown mock
/// path; deriving it from a request header would let host-header injection
/// hand the code to an attacker's domain.
fn share_url(state: &AppState, code: &str) -> String {
    format!("{}/s/{code}", state.cfg.public_base_url)
}

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

/// Wrap any response (success or error) with `Cache-Control: no-store` —
/// every handler-produced response on the public routes carries it (AC-37),
/// including a 503 from a bubbled `sqlx::Error`, which is why this wraps the
/// *outermost* Result rather than being attached per branch.
fn no_store(mut resp: Response) -> Response {
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
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

/// Filter `response_headers` for the PUBLIC projection ONLY (§5.11, AC-S1,
/// AC-S2): drop every `x-hookbox-*` key entirely, mask the five
/// credential-bearing keys to `<redacted>` (key kept, value replaced), then
/// mask any surviving value that contains `token` — closes the CORS-echo
/// channel where `access-control-allow-origin` reflects a wildcard `Origin:
/// https://<token>.<MOCK_DOMAIN>` sent by the caller (hookbox-mun.34).
/// Storage, the owner Inspector and F5's CSV all stay verbatim — this filter
/// is applied nowhere else in the codebase.
fn filter_public_response_headers(raw: Value, token: &str) -> Value {
    let map = match raw {
        Value::Object(m) => m,
        _ => return json!({}),
    };
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        let kl = k.to_ascii_lowercase();
        if kl.starts_with(PUBLIC_RESPONSE_HEADER_DROP_PREFIX) {
            continue;
        }
        if PUBLIC_RESPONSE_HEADER_REDACT.contains(&kl.as_str()) {
            out.insert(k, json!("<redacted>"));
        } else {
            out.insert(k, mask_token_in_value(token, v));
        }
    }
    Value::Object(out)
}

/// Filter `request_headers` for the PUBLIC projection ONLY (AC-43, AC-S2,
/// hookbox-mun.34): drop `host`/`origin`/`referer` entirely — structural
/// carriers of the token in wildcard mock-host mode — then mask any
/// surviving value that contains `token`. Storage and the owner Inspector
/// keep `request_headers` verbatim (§5.11); this filter is applied nowhere
/// else in the codebase.
fn filter_public_request_headers(raw: Value, token: &str) -> Value {
    let map = match raw {
        Value::Object(m) => m,
        _ => return json!({}),
    };
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        let kl = k.to_ascii_lowercase();
        if PUBLIC_REQUEST_HEADER_DROP.contains(&kl.as_str()) {
            continue;
        }
        out.insert(k, mask_token_in_value(token, v));
    }
    Value::Object(out)
}

fn public_request_summary(row: &SqliteRow) -> PublicRequestSummary {
    PublicRequestSummary {
        id: row.get("id"),
        method: row.get("method"),
        path: row.get("path"),
        status_code: row.get("status_code"),
        served_by: row.get("served_by"),
        duration_ms: row.get("duration_ms"),
        timestamp: to_rfc3339(row.get("created_at")).unwrap_or_default(),
    }
}

/// `token` is the resolved share row's OWN endpoint token (never taken from
/// the caller) — used only to mask its own reappearance in the projection
/// (AC-43, AC-S2).
fn public_request_detail(row: &SqliteRow, token: &str) -> PublicRequestDetail {
    PublicRequestDetail {
        id: row.get("id"),
        method: row.get("method"),
        path: row.get("path"),
        status_code: row.get("status_code"),
        served_by: row.get("served_by"),
        duration_ms: row.get("duration_ms"),
        timestamp: to_rfc3339(row.get("created_at")).unwrap_or_default(),
        request_headers: filter_public_request_headers(
            parse_json_value(row.get("request_headers"), json!({})),
            token,
        ),
        query_params: parse_json_value(row.get("query_params"), json!({})),
        request_body: row.get("request_body"),
        response_headers: filter_public_response_headers(
            parse_json_value(row.get("response_headers"), json!({})),
            token,
        ),
        response_body: row.get("response_body"),
    }
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

// === #19 POST /api/endpoints/{token}/shares (owner) ==========================

async fn create_share(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    Json(body): Json<ShareLinkCreate>,
) -> Result<Response, ApiError> {
    // 1. OwnerId extractor already ran (401 handled by the extractor itself).
    // 2. 404 for unknown token *and* another owner's token.
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;

    // 3. Reject a tombstoned endpoint — a link minted for a dead endpoint
    // would be dead on arrival.
    let gone_at: Option<String> =
        sqlx::query_scalar("SELECT gone_at FROM endpoints WHERE token = ?")
            .bind(&token)
            .fetch_one(&state.pool)
            .await?;
    if gone_at.is_some() {
        return Err(ApiError::not_found("Endpoint not found."));
    }

    // 4. Validate + trim the label; a label that trims to empty is NULL.
    let label = match body.label {
        None => None,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.chars().count() > 80 {
                return Err(ApiError::validation("label must be at most 80 characters."));
            } else {
                Some(trimmed.to_string())
            }
        }
    };

    // 5. Enforce the active-link cap.
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM share_links WHERE token = ? AND revoked_at IS NULL",
    )
    .bind(&token)
    .fetch_one(&state.pool)
    .await?;
    if active_count >= state.cfg.share_max_per_endpoint {
        return Err(ApiError::validation(format!(
            "This endpoint already has the maximum of {} active share links. Revoke one first.",
            state.cfg.share_max_per_endpoint
        )));
    }

    // 6. Mint + store only the hash — the code appears in plaintext nowhere
    // in this database, ever.
    let code = gen_share_code(state.cfg.share_code_bytes);
    let code_hash = hash_secret(&code);
    let row = sqlx::query(
        "INSERT INTO share_links (code_hash, token, label) VALUES (?, ?, ?) RETURNING id, created_at",
    )
    .bind(&code_hash)
    .bind(&token)
    .bind(&label)
    .fetch_one(&state.pool)
    .await?;
    let id: i64 = row.get("id");
    let created_at: Option<String> = row.get("created_at");

    // 7. `code` and `url` appear ONLY here, in ONLY this response, and are
    // never re-derivable (AC-104).
    let resp = ShareLinkCreated {
        id,
        code: code.clone(),
        url: share_url(&state, &code),
        label,
        created_at: to_rfc3339(created_at).unwrap_or_default(),
        last_used_at: None,
    };
    Ok(no_store((StatusCode::CREATED, Json(resp)).into_response()))
}

// === #20 GET /api/endpoints/{token}/shares (owner) ============================

async fn list_shares(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
) -> Result<Json<Vec<ShareLink>>, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    // Revoked links never appear (AC-25). No `code`, no `url`, no code prefix.
    let rows = sqlx::query(
        "SELECT id, label, created_at, last_used_at FROM share_links
          WHERE token = ? AND revoked_at IS NULL ORDER BY created_at DESC, id DESC",
    )
    .bind(&token)
    .fetch_all(&state.pool)
    .await?;
    let list = rows
        .iter()
        .map(|r| ShareLink {
            id: r.get("id"),
            label: r.get("label"),
            created_at: to_rfc3339(r.get("created_at")).unwrap_or_default(),
            last_used_at: to_rfc3339(r.get("last_used_at")),
        })
        .collect();
    Ok(Json(list))
}

// === #21 DELETE /api/endpoints/{token}/shares/{id} (owner) ===================

async fn revoke_share(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, id)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    assert_owns_endpoint(&state.pool, &token, &owner_id).await?;
    // Soft revoke only — never a row delete, so a revoked code_hash can never
    // be re-minted and the UNIQUE constraint keeps enforcing global
    // uniqueness against revoked codes too.
    let result = sqlx::query(
        "UPDATE share_links SET revoked_at = datetime('now') WHERE id = ? AND token = ? AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(&token)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        // Unknown id, id on another endpoint, and already-revoked are all
        // indistinguishable here, and idempotent from the caller's POV.
        return Err(ApiError::not_found("Share link not found."));
    }
    Ok(StatusCode::NO_CONTENT.into_response()) // 204, no body
}

// === #22 GET /api/share/{code}/requests?limit&offset (PUBLIC) ================

#[derive(Deserialize)]
struct PublicRequestsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn public_list_requests_inner(
    state: &AppState,
    ip: &str,
    code: &str,
    q: PublicRequestsQuery,
) -> Result<Json<PublicShareFeed>, ApiError> {
    // 1. Rate limit — before any DB read.
    check_share_rate_limit(state, ip)?;

    // 2. Parameter validation MUST precede code resolution (AC-101): if
    // `limit` were validated after resolving the code, `?limit=999` would
    // return 422 for a live code and 404 for a dead one — a boolean
    // existence oracle that defeats AC-36.
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

async fn public_list_requests(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Path(code): Path<String>,
    Query(q): Query<PublicRequestsQuery>,
) -> Response {
    let ip = effective_client_ip(connect_info, &headers);
    match public_list_requests_inner(&state, &ip, &code, q).await {
        Ok(v) => no_store(v.into_response()),
        Err(e) => no_store(e.into_response()),
    }
}

// === #23 GET /api/share/{code}/requests/{id} (PUBLIC) =========================

async fn public_get_request_inner(
    state: &AppState,
    ip: &str,
    code: &str,
    request_id: i64,
) -> Result<Json<PublicRequestDetail>, ApiError> {
    // 1. Rate limit — before any DB read.
    check_share_rate_limit(state, ip)?;

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

async fn public_get_request(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Path((code, request_id)): Path<(String, i64)>,
) -> Response {
    let ip = effective_client_ip(connect_info, &headers);
    match public_get_request_inner(&state, &ip, &code, request_id).await {
        Ok(v) => no_store(v.into_response()),
        Err(e) => no_store(e.into_response()),
    }
}

/// Build the F4 share-link router: three owner routes + two public routes.
/// Merged alongside `api_router()` in `router::build_app`. No `TraceLayer` is
/// added anywhere near this router (AC-S6) — the app's own logging must never
/// see a share code or the full path.
pub fn share_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/endpoints/:token/shares",
            get(list_shares).post(create_share),
        )
        .route("/api/endpoints/:token/shares/:id", delete(revoke_share))
        .route("/api/share/:code/requests", get(public_list_requests))
        .route("/api/share/:code/requests/:id", get(public_get_request))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn filter_drops_x_hookbox_prefix_and_redacts_credential_headers() {
        let raw = json!({
            "x-hookbox-endpoint": "tok1234567",
            "x-hookbox-rule-id": "1",
            "set-cookie": "sid=abc",
            "content-type": "application/json",
        });
        let out = filter_public_response_headers(raw, "tok1234567");
        let obj = out.as_object().unwrap();
        assert!(!obj.contains_key("x-hookbox-endpoint"));
        assert!(!obj.contains_key("x-hookbox-rule-id"));
        assert_eq!(obj["set-cookie"], json!("<redacted>"));
        assert_eq!(obj["content-type"], json!("application/json"));
    }

    #[test]
    fn filter_response_headers_masks_any_value_containing_the_token() {
        // hookbox-mun.34 channel A: a CORS echo of the wildcard mock Origin
        // survives the name-based filter and must still be masked.
        let raw = json!({
            "access-control-allow-origin": "https://sx37Uac9ty.mock.local",
            "content-type": "application/json",
        });
        let out = filter_public_response_headers(raw, "sx37Uac9ty");
        let obj = out.as_object().unwrap();
        assert_eq!(obj["access-control-allow-origin"], json!("<redacted>"));
        assert_eq!(obj["content-type"], json!("application/json"));
    }

    #[test]
    fn filter_request_headers_drops_host_origin_referer_and_masks_token_value() {
        // hookbox-mun.34 channel B: Host structurally embeds the token in
        // wildcard mode; Origin/Referer can echo it too.
        let raw = json!({
            "host": "Q3L3jRQ7oY.mock.local",
            "origin": "https://Q3L3jRQ7oY.mock.local",
            "referer": "https://Q3L3jRQ7oY.mock.local/x",
            "x-forwarded-for": "https://Q3L3jRQ7oY.mock.local/smuggled",
            "accept": "application/json",
        });
        let out = filter_public_request_headers(raw, "Q3L3jRQ7oY");
        let obj = out.as_object().unwrap();
        assert!(!obj.contains_key("host"));
        assert!(!obj.contains_key("origin"));
        assert!(!obj.contains_key("referer"));
        assert_eq!(obj["x-forwarded-for"], json!("<redacted>"));
        assert_eq!(obj["accept"], json!("application/json"));
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
