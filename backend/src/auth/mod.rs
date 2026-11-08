//! Owner-key auth (§5.1, AC-S14a–b).
//!
//! An owner is a non-secret random `owner_id` plus a **secret** owner key
//! (256-bit CSPRNG, stored only as sha256). The key arrives one of two ways:
//!   * `Authorization: Bearer <key>` — the tunnel CLI, curl scripting, tests.
//!     A browser never attaches it on its own, so no CSRF check applies.
//!   * the HttpOnly session cookie — the SPA. Browsers attach it
//!     automatically, so it is honored only after `session::csrf_guard`
//!     (`x-hookbox: 1` + a same-origin request).
//!
//! Precedence is strict: when an `Authorization` header is present it is the
//! ONLY credential considered (an invalid Bearer is 401, never a silent
//! fallback to the cookie). Outcomes:
//!   * missing / malformed / unknown key        → 401 + `WWW-Authenticate: Bearer`
//!   * cookie without the CSRF header / foreign origin → 403 `csrf_rejected`
//!   * valid key that does not own `{token}`   → 404 (never 403)
//!
//! The public `owner_id` is **never** a credential — only `sha256(key)` is
//! looked up, via the unique `owners.secret_hash` index.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::error::ApiError;
use crate::ids::hash_secret;
use crate::repo::owners;
use crate::session::{csrf_guard, read_session_cookie};
use crate::state::AppState;

mod bearer;
mod ownership;

pub(crate) use bearer::parse_bearer;
pub(crate) use ownership::require_owned_endpoint;
pub use ownership::{assert_owns_endpoint, key_owns_token};

/// How the caller presented its owner key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Bearer(String),
    Cookie(String),
}

impl Credential {
    /// The raw owner key.
    pub fn key(&self) -> &str {
        match self {
            Credential::Bearer(k) | Credential::Cookie(k) => k,
        }
    }

    pub fn is_cookie(&self) -> bool {
        matches!(self, Credential::Cookie(_))
    }
}

/// An authenticated owner plus the credential that proved it.
#[derive(Debug, Clone)]
pub struct AuthedOwner {
    pub owner_id: String,
    pub credential: Credential,
}

impl AuthedOwner {
    /// The raw owner key the caller presented.
    pub fn key(&self) -> &str {
        self.credential.key()
    }
}

fn unauthorized() -> ApiError {
    ApiError::unauthorized("Valid owner capability required.")
}

/// The owner holding `key`, if any (looked up by `sha256(key)`).
pub(crate) async fn owner_for_key(
    pool: &SqlitePool,
    key: &str,
) -> Result<Option<String>, sqlx::Error> {
    owners::find_by_secret_hash(pool, &hash_secret(key)).await
}

/// The credential a request presents, applying Bearer-over-cookie
/// precedence. A present-but-malformed `Authorization` header yields 401;
/// a cookie is only returned after the CSRF guard passes.
fn presented_credential(headers: &HeaderMap, cfg: &Config) -> Result<Credential, ApiError> {
    if let Some(auth) = headers.get(AUTHORIZATION) {
        let key = auth
            .to_str()
            .ok()
            .and_then(parse_bearer)
            .ok_or_else(unauthorized)?;
        return Ok(Credential::Bearer(key.to_string()));
    }
    let key = read_session_cookie(headers, cfg).ok_or_else(unauthorized)?;
    csrf_guard(headers, cfg)?;
    Ok(Credential::Cookie(key))
}

/// Resolve the authenticated owner from a request's headers (see the module
/// docs for precedence and outcomes).
pub async fn resolve_owner(
    pool: &SqlitePool,
    cfg: &Config,
    headers: &HeaderMap,
) -> Result<AuthedOwner, ApiError> {
    let credential = presented_credential(headers, cfg)?;
    let owner_id = owner_for_key(pool, credential.key())
        .await?
        .ok_or_else(unauthorized)?;
    Ok(AuthedOwner {
        owner_id,
        credential,
    })
}

/// The owner key a live-feed handshake presents: the Bearer when an
/// `Authorization` header is present, else the session cookie. No custom
/// header is required — WebSocket and EventSource cannot set one; the feed
/// routes check `origin_allowed` instead.
pub fn feed_key(headers: &HeaderMap, cfg: &Config) -> Option<String> {
    match headers.get(AUTHORIZATION) {
        Some(auth) => auth
            .to_str()
            .ok()
            .and_then(parse_bearer)
            .map(str::to_string),
        None => read_session_cookie(headers, cfg),
    }
}

#[axum::async_trait]
impl FromRequestParts<AppState> for AuthedOwner {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        resolve_owner(&state.pool, &state.cfg, &parts.headers).await
    }
}

/// Axum extractor: just the authenticated owner id (Bearer or cookie+CSRF).
/// Yields `ApiError` (401/403) on failure — the flat envelope reaches the
/// client unchanged.
pub struct OwnerId(pub String);

#[axum::async_trait]
impl FromRequestParts<AppState> for OwnerId {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let owner = AuthedOwner::from_request_parts(parts, state).await?;
        Ok(OwnerId(owner.owner_id))
    }
}

/// Axum extractor for the unauthenticated routes that set or clear the
/// session cookie (mint / restore / sign-out): passes only when
/// `session::csrf_guard` does, so a foreign page can neither plant its own
/// key in the owner's browser nor sign them out.
pub struct CsrfGuard;

#[axum::async_trait]
impl FromRequestParts<AppState> for CsrfGuard {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        csrf_guard(&parts.headers, &state.cfg).map(|()| CsrfGuard)
    }
}

#[cfg(test)]
mod tests;
