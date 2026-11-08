//! Endpoint-ownership checks: the 404-never-403 route guard (as a function
//! and as the `OwnedEndpoint` extractor) and the extractor-free key check
//! used by the feed and tunnel handshakes.

use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use sqlx::SqlitePool;

use super::{owner_for_key, OwnerId};
use crate::error::ApiError;
use crate::repo::endpoints::{self, owner_of as endpoint_owner, EndpointRow};
use crate::state::AppState;

/// The route parameter every endpoint-scoped route names its token by.
const TOKEN_PARAM: &str = "token";

fn endpoint_not_found() -> ApiError {
    ApiError::not_found("Endpoint not found.")
}

/// The endpoint row iff `owner_id` owns `token`; else 404 (never 403) — a
/// non-owner cannot tell "absent" from "someone else's". One query for the
/// check and the row.
pub(crate) async fn require_owned_endpoint(
    pool: &SqlitePool,
    token: &str,
    owner_id: &str,
) -> Result<EndpointRow, ApiError> {
    match endpoints::find(pool, token).await? {
        Some(row) if row.owner_id == owner_id => Ok(row),
        _ => Err(endpoint_not_found()),
    }
}

/// `require_owned_endpoint`, then 410 `endpoint_gone` when the owner's
/// endpoint is tombstoned. A non-owner still gets the 404 (ownership is
/// checked first), so a tombstone reveals nothing to strangers.
pub(crate) async fn require_live_endpoint(
    pool: &SqlitePool,
    token: &str,
    owner_id: &str,
) -> Result<EndpointRow, ApiError> {
    require_owned_endpoint(pool, token, owner_id)
        .await?
        .require_live()
}

impl EndpointRow {
    /// 410 `endpoint_gone` for a tombstoned endpoint.
    pub(crate) fn require_live(self) -> Result<Self, ApiError> {
        if self.is_gone() {
            Err(ApiError::endpoint_gone())
        } else {
            Ok(self)
        }
    }
}

/// Extractor for an endpoint-scoped owner route (`/api/endpoints/:token…`):
/// authenticates the owner (`OwnerId`: 401/403), reads the `:token` path
/// parameter, then requires a live owned endpoint (404 not yours / 410
/// tombstoned). Handlers whose own validation must run BEFORE the ownership
/// check (a typed `Path`/`Query`/JSON body rejection today precedes the 404)
/// use `OwnerId` + `require_live_endpoint` explicitly instead, so that order
/// is kept.
pub(crate) struct OwnedEndpoint {
    pub(crate) endpoint: EndpointRow,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for OwnedEndpoint {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let OwnerId(owner_id) = OwnerId::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let token = params
            .iter()
            .find_map(|(name, value)| (name == TOKEN_PARAM).then_some(value))
            .ok_or_else(|| endpoint_not_found().into_response())?;
        let endpoint = require_live_endpoint(&state.pool, token, &owner_id)
            .await
            .map_err(IntoResponse::into_response)?;
        Ok(OwnedEndpoint { endpoint })
    }
}

/// True iff `key` is a valid owner key whose owner owns `token`.
///
/// Used by the WS/SSE feed gate (§5.4) and the tunnel bind handshake (§5.8),
/// which run *before* any extractor. A missing/blank key or token returns
/// false without a DB hit. Never errors — a DB hiccup denies.
pub async fn key_owns_token(pool: &SqlitePool, token: &str, key: Option<&str>) -> bool {
    let Some(key) = key.filter(|k| !k.is_empty()) else {
        return false;
    };
    if token.is_empty() {
        return false;
    }
    let Ok(Some(owner_id)) = owner_for_key(pool, key).await else {
        return false;
    };
    matches!(endpoint_owner(pool, token).await, Ok(Some(o)) if o == owner_id)
}
