//! Endpoint-ownership checks: the 404-never-403 route guard and the
//! extractor-free key check used by the feed and tunnel handshakes.

use sqlx::SqlitePool;

use super::owner_for_key;
use crate::error::ApiError;
use crate::repo::endpoints::{self, owner_of as endpoint_owner, EndpointRow};

/// `Ok(())` iff `owner_id` owns `token`; else 404 (never 403). Callers that
/// need the full row should select it themselves after this passes, or use
/// the route-layer query that already loads the row.
pub async fn assert_owns_endpoint(
    pool: &SqlitePool,
    token: &str,
    owner_id: &str,
) -> Result<(), ApiError> {
    match endpoint_owner(pool, token).await? {
        Some(o) if o == owner_id => Ok(()),
        _ => Err(ApiError::not_found("Endpoint not found.")),
    }
}

/// The endpoint row iff `owner_id` owns `token`; else 404 (never 403). One
/// query for the check and the row.
pub(crate) async fn require_owned_endpoint(
    pool: &SqlitePool,
    token: &str,
    owner_id: &str,
) -> Result<EndpointRow, ApiError> {
    match endpoints::find(pool, token).await? {
        Some(row) if row.owner_id == owner_id => Ok(row),
        _ => Err(ApiError::not_found("Endpoint not found.")),
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
