//! Endpoint-ownership checks: the 404-never-403 route guard and the
//! extractor-free capability check used by the feed and tunnel handshakes.

use sqlx::SqlitePool;

use crate::error::ApiError;
use crate::ids::hash_secret;

/// Return the endpoint row's owner check result: `Ok(())` iff `owner_id` owns
/// `token`; else 404 (never 403). Mirrors `assert_owns_endpoint`. Callers that
/// need the full row should select it themselves after this passes, or use the
/// route-layer query that already loads the row.
pub async fn assert_owns_endpoint(
    pool: &SqlitePool,
    token: &str,
    owner_id: &str,
) -> Result<(), ApiError> {
    let row_owner: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM endpoints WHERE token = ?")
            .bind(token)
            .fetch_optional(pool)
            .await?;
    match row_owner {
        Some(o) if o == owner_id => Ok(()),
        _ => Err(ApiError::not_found("Endpoint not found.")),
    }
}

/// Return true iff `secret` is a valid owner capability that owns `token`.
///
/// Used by the WS/SSE feed gate (§5.4) and the tunnel bind handshake (§5.8),
/// which run *before* any extractor and therefore cannot use `require_owner`.
/// A missing/blank secret or token returns false without a credential DB hit.
/// Never errors — a DB hiccup denies (returns false).
pub async fn verify_cap_owns_token(pool: &SqlitePool, token: &str, secret: Option<&str>) -> bool {
    let secret = match secret {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    if token.is_empty() {
        return false;
    }
    let secret_hash = hash_secret(secret);
    let owner_id: Option<String> =
        match sqlx::query_scalar("SELECT owner_id FROM owners WHERE secret_hash = ?")
            .bind(&secret_hash)
            .fetch_optional(pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return false,
        };
    let owner_id = match owner_id {
        Some(o) => o,
        None => return false,
    };
    let ep_owner: Option<String> =
        match sqlx::query_scalar("SELECT owner_id FROM endpoints WHERE token = ?")
            .bind(token)
            .fetch_optional(pool)
            .await
        {
            Ok(v) => v,
            Err(_) => return false,
        };
    matches!(ep_owner, Some(o) if o == owner_id)
}
