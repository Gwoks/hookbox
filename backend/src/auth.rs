//! Owner-capability auth (§5.1) — PORT of `app/auth.py`.
//!
//! A non-secret `owner_id` (= `hash_email`) plus a **secret** bearer
//! `owner_secret` (256-bit CSPRNG, stored sha256-hashed). Every `/api/**` route
//! except `POST /api/session` requires `Authorization: Bearer <owner_secret>`:
//!   * missing / malformed / unknown secret      → 401 + `WWW-Authenticate: Bearer`
//!   * valid secret that does not own `{token}`   → 404 (never 403 — a non-owner
//!     cannot distinguish "exists but not mine" from "does not exist")
//!
//! The public `owner_id` is **never** accepted as a credential — only
//! `sha256(secret)` is looked up. The secret is compared by hash (a constant
//! indexed lookup), never by a raw early-return string compare (AC-S6/S7/S8).

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::SqlitePool;

use crate::error::ApiError;
use crate::ids::hash_secret;
use crate::state::AppState;

/// Parse a `Bearer <secret>` header, returning the raw secret or 401.
fn parse_bearer(authorization: Option<&str>) -> Result<String, ApiError> {
    let unauthorized = || ApiError::unauthorized("Valid owner capability required.");
    let header = authorization.ok_or_else(unauthorized)?;
    let mut parts = header.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    let token = parts.next().unwrap_or("").trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(unauthorized());
    }
    Ok(token.to_string())
}

/// Resolve the authenticated `owner_id` from the bearer secret (PORT of
/// `require_owner`). Looks up `sha256(secret)`; missing/malformed/unknown → 401.
pub async fn require_owner(
    pool: &SqlitePool,
    authorization: Option<&str>,
) -> Result<String, ApiError> {
    let secret = parse_bearer(authorization)?;
    let secret_hash = hash_secret(&secret);
    let owner_id: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM owners WHERE secret_hash = ?")
            .bind(&secret_hash)
            .fetch_optional(pool)
            .await?;
    owner_id.ok_or_else(|| ApiError::unauthorized("Valid owner capability required."))
}

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

/// Axum extractor: an authenticated owner id resolved from the `Authorization`
/// header against `AppState`. Yields `ApiError` (401) on failure — the flat
/// envelope reaches the client unchanged.
pub struct OwnerId(pub String);

#[axum::async_trait]
impl FromRequestParts<AppState> for OwnerId {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        let owner_id = require_owner(&state.pool, auth).await?;
        Ok(OwnerId(owner_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::ids::{gen_owner_secret, hash_email, hash_secret};

    async fn setup() -> (SqlitePool, String, String) {
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        let owner_id = hash_email("a@b.com");
        let secret = gen_owner_secret(32);
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)")
            .bind(&owner_id)
            .bind("a@b.com")
            .bind(hash_secret(&secret))
            .execute(&pool)
            .await
            .unwrap();
        (pool, owner_id, secret)
    }

    #[tokio::test]
    async fn bearer_resolves_owner_and_unknown_is_401() {
        let (pool, owner_id, secret) = setup().await;
        let header = format!("Bearer {secret}");
        let resolved = require_owner(&pool, Some(&header)).await.unwrap();
        assert_eq!(resolved, owner_id);

        // Missing / malformed / unknown all 401.
        assert_eq!(
            require_owner(&pool, None).await.unwrap_err().status,
            axum::http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            require_owner(&pool, Some("Basic xyz")).await.unwrap_err().status,
            axum::http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            require_owner(&pool, Some("Bearer not-a-real-secret"))
                .await
                .unwrap_err()
                .status,
            axum::http::StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn owner_id_is_never_a_credential() {
        let (pool, owner_id, _secret) = setup().await;
        // Presenting the public owner_id as the bearer must 401 (it is not a secret).
        let header = format!("Bearer {owner_id}");
        assert_eq!(
            require_owner(&pool, Some(&header)).await.unwrap_err().status,
            axum::http::StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn rotation_invalidates_old_secret() {
        let (pool, owner_id, old_secret) = setup().await;
        let old_header = format!("Bearer {old_secret}");
        assert!(require_owner(&pool, Some(&old_header)).await.is_ok());

        // Rotate (re-submit email overwrites secret_hash).
        let new_secret = gen_owner_secret(32);
        sqlx::query("UPDATE owners SET secret_hash = ? WHERE owner_id = ?")
            .bind(hash_secret(&new_secret))
            .bind(&owner_id)
            .execute(&pool)
            .await
            .unwrap();

        // Old secret now 401s; new secret works.
        assert_eq!(
            require_owner(&pool, Some(&old_header)).await.unwrap_err().status,
            axum::http::StatusCode::UNAUTHORIZED
        );
        let new_header = format!("Bearer {new_secret}");
        assert_eq!(require_owner(&pool, Some(&new_header)).await.unwrap(), owner_id);
    }

    #[tokio::test]
    async fn non_owner_of_token_is_404_not_403() {
        let (pool, owner_id, secret) = setup().await;
        // Owner A's endpoint.
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES (?, ?)")
            .bind("tokAAAAAAA")
            .bind(&owner_id)
            .execute(&pool)
            .await
            .unwrap();
        // Owner B.
        let b_id = hash_email("b@c.com");
        let b_secret = gen_owner_secret(32);
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)")
            .bind(&b_id)
            .bind("b@c.com")
            .bind(hash_secret(&b_secret))
            .execute(&pool)
            .await
            .unwrap();

        // Owner A owns it.
        assert!(assert_owns_endpoint(&pool, "tokAAAAAAA", &owner_id).await.is_ok());
        // Owner B sees 404 (never 403) for a token that exists but is not theirs.
        let err = assert_owns_endpoint(&pool, "tokAAAAAAA", &b_id).await.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(err.code, "not_found");
        // A token that does not exist is also 404 (indistinguishable).
        let err2 = assert_owns_endpoint(&pool, "doesnotexi", &owner_id).await.unwrap_err();
        assert_eq!(err2.status, axum::http::StatusCode::NOT_FOUND);

        // Feed/tunnel cap gate: correct cap+token true; wrong-owner cap false.
        assert!(verify_cap_owns_token(&pool, "tokAAAAAAA", Some(&secret)).await);
        assert!(!verify_cap_owns_token(&pool, "tokAAAAAAA", Some(&b_secret)).await);
        assert!(!verify_cap_owns_token(&pool, "tokAAAAAAA", None).await);
        assert!(!verify_cap_owns_token(&pool, "tokAAAAAAA", Some("")).await);
    }
}
