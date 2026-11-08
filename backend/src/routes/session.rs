//! `/api/session*` — anonymous owner sessions (§5.2 #1, AC-S14a).
//!
//! The owner key is minted server-side and handed to the browser ONLY as an
//! HttpOnly cookie; the summary bodies never carry it. The CLI and scripts
//! present the same key as `Authorization: Bearer`.
//!
//! | Route                       | Auth              | Result                               |
//! |-----------------------------|-------------------|--------------------------------------|
//! | `POST /api/session`         | CsrfGuard + limit | 201 + cookie + summary (new owner);  |
//! |                             |                   | 200 + summary if a valid cookie sent |
//! | `POST /api/session/restore` | CsrfGuard + limit | 200 + cookie + summary; no rotation  |
//! | `GET /api/session`          | owner             | 200 summary; refreshes the cookie    |
//! | `POST /api/session/key`     | owner             | 200 `{key}` (the presented key)      |
//! | `POST /api/session/rotate`  | owner             | 200 `{key}` + cookie; revokes tunnels|
//! | `DELETE /api/session`       | CsrfGuard         | 204 + cleared cookie; key stays valid|
//!
//! Every response — success or error, including extractor rejections —
//! carries `Cache-Control: no-store` (`no_store_response`, layered over
//! these routes in `routes::session_routes`).

use std::net::SocketAddr;

use axum::{
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::auth::{owner_for_key, AuthedOwner, Credential, CsrfGuard};
use crate::error::ApiError;
use crate::ids::{gen_owner_id, gen_owner_secret, hash_secret};
use crate::models::{EndpointSummary, KeyResponse, RestoreBody, SessionSummary};
use crate::net::client_ip::effective_client_ip;
use crate::net::headers::no_store;
use crate::repo::owners;
use crate::routes::endpoints::{endpoint_summary, new_endpoint};
use crate::routes::tunnel_ws::publish_endpoint_updated;
use crate::session::{clear_cookie_value, read_session_cookie, set_cookie_value};
use crate::state::AppState;

/// Accepted owner-key shape: base64url, 22 chars (128-bit) to 128 chars.
const KEY_MIN_LEN: usize = 22;
const KEY_MAX_LEN: usize = 128;

/// Rate-limit window for the session namespace.
const SESSION_RATE_WINDOW_SECS: i64 = 60;

// --- helpers -----------------------------------------------------------------

/// `map_response` adapter for the session routes' `no-store` layer.
pub(super) async fn no_store_response(resp: Response) -> Response {
    no_store(resp)
}

/// Per-source limit on mint/restore (AC-S5), in its own `session:` limiter
/// namespace so an unauthenticated flood can never evict mock-plane or share
/// buckets. Fails open like every limiter call.
fn rate_limit_session(
    state: &AppState,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let ip = effective_client_ip(connect_info, headers);
    let verdict = state.limiter.check(
        &format!("session:{ip}"),
        state.cfg.session_rate_limit_per_min,
        SESSION_RATE_WINDOW_SECS,
    );
    if verdict.allowed {
        Ok(())
    } else {
        Err(ApiError::rate_limited(
            "Too many session requests.",
            verdict.retry_after,
        ))
    }
}

/// The owner's id plus live endpoints (`primary` = the oldest, or null).
async fn summary(state: &AppState, owner_id: &str) -> Result<SessionSummary, ApiError> {
    let rows = owners::live_endpoints(&state.pool, owner_id).await?;
    let endpoints: Vec<EndpointSummary> = rows.iter().map(|r| endpoint_summary(state, r)).collect();
    Ok(SessionSummary {
        owner_id: owner_id.to_string(),
        primary: endpoints.first().cloned(),
        endpoints,
    })
}

/// Append one `Set-Cookie` line.
fn with_cookie(mut resp: Response, set_cookie: &str) -> Response {
    if let Ok(v) = HeaderValue::from_str(set_cookie) {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

/// (Re-)issue the session cookie for `key` — only when the caller is the
/// cookie-holding browser, never for a Bearer client.
fn with_cookie_if(
    state: &AppState,
    credential: &Credential,
    key: &str,
    resp: Response,
) -> Response {
    if credential.is_cookie() {
        with_cookie(resp, &set_cookie_value(&state.cfg, key))
    } else {
        resp
    }
}

fn is_key_shape(key: &str) -> bool {
    (KEY_MIN_LEN..=KEY_MAX_LEN).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The key from a restore body; anything malformed is `401 invalid_key`.
fn parse_restore_key(body: &[u8]) -> Result<String, ApiError> {
    let RestoreBody { key } = serde_json::from_slice(body).map_err(|_| ApiError::invalid_key())?;
    let key = key.trim();
    if is_key_shape(key) {
        Ok(key.to_string())
    } else {
        Err(ApiError::invalid_key())
    }
}

/// The owner behind an already-present, valid session cookie.
async fn cookie_owner(state: &AppState, headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    match read_session_cookie(headers, &state.cfg) {
        Some(key) => Ok(owner_for_key(&state.pool, &key).await?),
        None => Ok(None),
    }
}

// --- POST /api/session ---------------------------------------------------------

/// Mint a fresh anonymous owner + primary endpoint (body ignored). A caller
/// that already holds a valid cookie gets its existing summary instead.
pub(super) async fn create_session(
    _csrf: CsrfGuard,
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    rate_limit_session(&state, connect_info, &headers)?;
    if let Some(owner_id) = cookie_owner(&state, &headers).await? {
        return Ok(Json(summary(&state, &owner_id).await?).into_response());
    }
    let key = gen_owner_secret(state.cfg.owner_secret_bytes);
    let owner_id = gen_owner_id();
    owners::insert(&state.pool, &owner_id, &hash_secret(&key)).await?;
    new_endpoint(&state, &owner_id, None).await?;
    let resp = (StatusCode::CREATED, Json(summary(&state, &owner_id).await?)).into_response();
    Ok(with_cookie(resp, &set_cookie_value(&state.cfg, &key)))
}

// --- POST /api/session/restore -------------------------------------------------

/// Adopt an existing key (recovery / legacy-secret migration). Does NOT
/// rotate: the same key keeps working everywhere it is already used.
pub(super) async fn restore_session(
    _csrf: CsrfGuard,
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    rate_limit_session(&state, connect_info, &headers)?;
    let key = parse_restore_key(&body)?;
    let owner_id = owner_for_key(&state.pool, &key)
        .await?
        .ok_or_else(ApiError::invalid_key)?;
    owners::touch_last_seen(&state.pool, &owner_id).await?;
    let resp = Json(summary(&state, &owner_id).await?).into_response();
    Ok(with_cookie(resp, &set_cookie_value(&state.cfg, &key)))
}

// --- GET /api/session ----------------------------------------------------------

/// Who am I. Re-issues the cookie (sliding expiry) for cookie callers.
pub(super) async fn get_session(
    State(state): State<AppState>,
    owner: AuthedOwner,
) -> Result<Response, ApiError> {
    owners::touch_last_seen(&state.pool, &owner.owner_id).await?;
    let resp = Json(summary(&state, &owner.owner_id).await?).into_response();
    Ok(with_cookie_if(&state, &owner.credential, owner.key(), resp))
}

// --- POST /api/session/key -----------------------------------------------------

/// Reveal the key the caller presented (the server stores only its hash).
pub(super) async fn reveal_key(owner: AuthedOwner) -> Json<KeyResponse> {
    Json(KeyResponse {
        key: owner.key().to_string(),
    })
}

// --- POST /api/session/rotate --------------------------------------------------

/// Replace the owner key. The old key — in other browsers, scripts and the
/// tunnel CLI — stops working at once: every bound tunnel is dropped and every
/// live feed connection is closed so it must re-handshake with the new key.
pub(super) async fn rotate_key(
    State(state): State<AppState>,
    owner: AuthedOwner,
) -> Result<Response, ApiError> {
    // Read everything fallible BEFORE the key changes, so a failure can never
    // rotate the key without handing the caller the new one.
    let tokens = owners::endpoint_tokens(&state.pool, &owner.owner_id).await?;
    let key = gen_owner_secret(state.cfg.owner_secret_bytes);
    owners::set_key_hash(&state.pool, &owner.owner_id, &hash_secret(&key)).await?;
    for token in &tokens {
        state.feed_hub.revoke(token);
        if state.tunnels.revoke(token) {
            publish_endpoint_updated(&state, token);
        }
    }
    let resp = Json(KeyResponse { key: key.clone() }).into_response();
    Ok(with_cookie_if(&state, &owner.credential, &key, resp))
}

// --- DELETE /api/session -------------------------------------------------------

/// Sign out this browser: clear the cookie. The key itself stays valid
/// (there is no server-side session to end), so it can be restored later.
pub(super) async fn sign_out(_csrf: CsrfGuard, State(state): State<AppState>) -> Response {
    with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        &clear_cookie_value(&state.cfg),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_shape_is_base64url_22_to_128() {
        assert!(is_key_shape(&"a".repeat(22)));
        assert!(is_key_shape(&"A-_9".repeat(32)));
        assert!(!is_key_shape(&"a".repeat(21)));
        assert!(!is_key_shape(&"a".repeat(129)));
        assert!(!is_key_shape(&format!("{}=", "a".repeat(30))));
        assert!(!is_key_shape(&format!("{} ", "a".repeat(30))));
    }

    #[test]
    fn restore_body_parsing_maps_every_failure_to_invalid_key() {
        let ok = format!("{{\"key\":\"  {}  \"}}", "k".repeat(43));
        assert_eq!(parse_restore_key(ok.as_bytes()).unwrap(), "k".repeat(43));
        for bad in [
            &b"not json"[..],
            b"{}",
            b"{\"key\":5}",
            b"{\"key\":\"short\"}",
        ] {
            let err = parse_restore_key(bad).unwrap_err();
            assert_eq!(err.code, "invalid_key");
            assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        }
    }
}
