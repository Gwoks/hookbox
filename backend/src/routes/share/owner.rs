//! Owner-authenticated share-link routes (§5.1): mint, list, revoke — gated
//! by the `OwnerId` extractor exactly like every other management route.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use super::no_store;
use crate::auth::{require_owned_endpoint, OwnedEndpoint, OwnerId};
use crate::error::{ApiError, ApiJson};
use crate::ids::{gen_share_code, hash_secret};
use crate::models::{ShareLink, ShareLinkCreate, ShareLinkCreated};
use crate::repo::shares;
use crate::state::AppState;
use crate::util::time::rfc3339;
use crate::validate::share_label;

/// `url` is built from `PUBLIC_BASE_URL` only — never `mock_url`'s wildcard
/// form, never `Host`/`X-Forwarded-Host` (AC-99, AC-S14). A share URL on the
/// mock host would be swallowed by the interceptor and 404 as an unknown mock
/// path; deriving it from a request header would let host-header injection
/// hand the code to an attacker's domain.
fn share_url(state: &AppState, code: &str) -> String {
    format!("{}/s/{code}", state.cfg.public_base_url)
}

// === #19 POST /api/endpoints/{token}/shares (owner) ==========================

pub(crate) async fn create_share(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path(token): Path<String>,
    ApiJson(body): ApiJson<ShareLinkCreate>,
) -> Result<Response, ApiError> {
    // 1. OwnerId extractor already ran (401 handled by the extractor itself).
    // 2. 404 for unknown token *and* another owner's token.
    let endpoint = require_owned_endpoint(&state.pool, &token, &owner_id).await?;

    // 3. Reject a tombstoned endpoint — a link minted for a dead endpoint
    // would be dead on arrival.
    if endpoint.is_gone() {
        return Err(ApiError::not_found("Endpoint not found."));
    }

    // 4. Validate + trim the label; a label that trims to empty is NULL.
    let label = share_label(body.label.as_deref())?;

    // 5. Enforce the active-link cap.
    if shares::count_active(&state.pool, &token).await? >= state.cfg.share_max_per_endpoint {
        return Err(ApiError::validation(format!(
            "This endpoint already has the maximum of {} active share links. Revoke one first.",
            state.cfg.share_max_per_endpoint
        )));
    }

    // 6. Mint + store only the hash — the code appears in plaintext nowhere
    // in this database, ever.
    let code = gen_share_code(state.cfg.share_code_bytes);
    let row = shares::insert(&state.pool, &hash_secret(&code), &token, label.as_deref()).await?;

    // 7. `code` and `url` appear ONLY here, in ONLY this response, and are
    // never re-derivable (AC-104).
    let resp = ShareLinkCreated {
        id: row.id,
        url: share_url(&state, &code),
        code,
        label: row.label,
        created_at: rfc3339(&row.created_at),
        last_used_at: None,
    };
    Ok(no_store((StatusCode::CREATED, Json(resp)).into_response()))
}

// === #20 GET /api/endpoints/{token}/shares (owner) ============================

pub(crate) async fn list_shares(
    State(state): State<AppState>,
    OwnedEndpoint { endpoint }: OwnedEndpoint,
) -> Result<Json<Vec<ShareLink>>, ApiError> {
    // Revoked links never appear (AC-25). No `code`, no `url`, no code prefix.
    let rows = shares::list_active(&state.pool, &endpoint.token).await?;
    Ok(Json(rows.iter().map(ShareLink::from_row).collect()))
}

// === #21 DELETE /api/endpoints/{token}/shares/{id} (owner) ===================

pub(crate) async fn revoke_share(
    State(state): State<AppState>,
    OwnerId(owner_id): OwnerId,
    Path((token, id)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    require_owned_endpoint(&state.pool, &token, &owner_id).await?;
    // Soft revoke only (see `repo::shares::revoke`).
    if !shares::revoke(&state.pool, &token, id).await? {
        // Unknown id, id on another endpoint, and already-revoked are all
        // indistinguishable here, and idempotent from the caller's POV.
        return Err(ApiError::not_found("Share link not found."));
    }
    Ok(StatusCode::NO_CONTENT.into_response()) // 204, no body
}
