//! Credential resolution: Bearer vs cookie precedence, CSRF gating, key
//! rotation, and the 404-never-403 ownership checks.

use axum::http::header::{AUTHORIZATION, COOKIE, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use sqlx::SqlitePool;

use super::*;
use crate::ids::gen_owner_secret;
use crate::repo::owners;
use crate::session::CSRF_HEADER;
use crate::testing::{memory_pool, seed_endpoint, seed_owner, test_config, SeededOwner};

async fn setup() -> (SqlitePool, String, String) {
    let pool = memory_pool().await;
    let SeededOwner { owner_id, key } = seed_owner(&pool).await;
    (pool, owner_id, key)
}

/// Headers built from `(name, value)` pairs, always with an app `Host`.
fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("host", "app.local".parse().unwrap());
    for (k, v) in pairs {
        h.append(
            axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            v.parse().unwrap(),
        );
    }
    h
}

fn bearer(key: &str) -> String {
    format!("Bearer {key}")
}

fn cookie(key: &str) -> String {
    format!("hookbox_key={key}")
}

async fn status_of(pool: &SqlitePool, h: &HeaderMap) -> Result<String, StatusCode> {
    resolve_owner(pool, &test_config(), h)
        .await
        .map(|o| o.owner_id)
        .map_err(|e| e.status)
}

#[tokio::test]
async fn bearer_resolves_owner_and_unknown_is_401() {
    let (pool, owner_id, key) = setup().await;
    let h = headers(&[("authorization", &bearer(&key))]);
    let owner = resolve_owner(&pool, &test_config(), &h).await.unwrap();
    assert_eq!(owner.owner_id, owner_id);
    assert_eq!(owner.credential, Credential::Bearer(key.clone()));

    // Missing / malformed / unknown all 401.
    for h in [
        headers(&[]),
        headers(&[("authorization", "Basic xyz")]),
        headers(&[("authorization", "Bearer not-a-real-secret")]),
    ] {
        assert_eq!(status_of(&pool, &h).await, Err(StatusCode::UNAUTHORIZED));
    }
}

#[tokio::test]
async fn valid_bearer_needs_no_csrf_header() {
    let (pool, owner_id, key) = setup().await;
    let h = headers(&[
        ("authorization", &bearer(&key)),
        ("origin", "https://evil.example"),
    ]);
    assert_eq!(status_of(&pool, &h).await, Ok(owner_id));
}

#[tokio::test]
async fn cookie_with_csrf_header_resolves() {
    let (pool, owner_id, key) = setup().await;
    let h = headers(&[("cookie", &cookie(&key)), (CSRF_HEADER, "1")]);
    let owner = resolve_owner(&pool, &test_config(), &h).await.unwrap();
    assert_eq!(owner.owner_id, owner_id);
    assert!(owner.credential.is_cookie());
    assert_eq!(owner.key(), key);
}

#[tokio::test]
async fn cookie_without_csrf_header_or_from_foreign_origin_is_403() {
    let (pool, _owner_id, key) = setup().await;
    let no_header = headers(&[("cookie", &cookie(&key))]);
    assert_eq!(
        status_of(&pool, &no_header).await,
        Err(StatusCode::FORBIDDEN)
    );
    let foreign = headers(&[
        ("cookie", &cookie(&key)),
        (CSRF_HEADER, "1"),
        ("origin", "https://evil.example"),
    ]);
    assert_eq!(status_of(&pool, &foreign).await, Err(StatusCode::FORBIDDEN));
}

#[tokio::test]
async fn unknown_cookie_is_401() {
    let (pool, _owner_id, _key) = setup().await;
    let h = headers(&[("cookie", &cookie("nope-nope-nope")), (CSRF_HEADER, "1")]);
    assert_eq!(status_of(&pool, &h).await, Err(StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn invalid_bearer_never_falls_back_to_a_valid_cookie() {
    let (pool, _owner_id, key) = setup().await;
    let h = headers(&[
        ("authorization", "Bearer wrong"),
        ("cookie", &cookie(&key)),
        (CSRF_HEADER, "1"),
    ]);
    assert_eq!(status_of(&pool, &h).await, Err(StatusCode::UNAUTHORIZED));
    let malformed = headers(&[
        ("authorization", "Basic abc"),
        ("cookie", &cookie(&key)),
        (CSRF_HEADER, "1"),
    ]);
    assert_eq!(
        status_of(&pool, &malformed).await,
        Err(StatusCode::UNAUTHORIZED)
    );
}

#[tokio::test]
async fn owner_id_is_never_a_credential() {
    let (pool, owner_id, _key) = setup().await;
    let as_bearer = headers(&[("authorization", &bearer(&owner_id))]);
    assert_eq!(
        status_of(&pool, &as_bearer).await,
        Err(StatusCode::UNAUTHORIZED)
    );
    let as_cookie = headers(&[("cookie", &cookie(&owner_id)), (CSRF_HEADER, "1")]);
    assert_eq!(
        status_of(&pool, &as_cookie).await,
        Err(StatusCode::UNAUTHORIZED)
    );
}

#[tokio::test]
async fn rotation_invalidates_old_key() {
    let (pool, owner_id, old_key) = setup().await;
    let old = headers(&[("authorization", &bearer(&old_key))]);
    assert!(status_of(&pool, &old).await.is_ok());

    let new_key = gen_owner_secret(32);
    owners::set_key_hash(&pool, &owner_id, &hash_secret(&new_key))
        .await
        .unwrap();

    assert_eq!(status_of(&pool, &old).await, Err(StatusCode::UNAUTHORIZED));
    let new = headers(&[("authorization", &bearer(&new_key))]);
    assert_eq!(status_of(&pool, &new).await, Ok(owner_id));
}

#[test]
fn feed_key_prefers_bearer_and_falls_back_to_cookie() {
    let cfg = test_config();
    let mut h = HeaderMap::new();
    h.insert(COOKIE, "hookbox_key=C".parse().unwrap());
    assert_eq!(feed_key(&h, &cfg).as_deref(), Some("C"));
    h.insert(AUTHORIZATION, "Bearer B".parse().unwrap());
    assert_eq!(feed_key(&h, &cfg).as_deref(), Some("B"));
    h.insert(AUTHORIZATION, "Basic x".parse().unwrap());
    assert_eq!(
        feed_key(&h, &cfg),
        None,
        "a bad Authorization never falls back"
    );
    // No custom header is needed (WS/EventSource can't send one), and the
    // Origin check is the feed route's job, not feed_key's.
    let mut h = HeaderMap::new();
    h.insert(COOKIE, "hookbox_key=C".parse().unwrap());
    h.insert(ORIGIN, "https://evil.example".parse().unwrap());
    assert_eq!(feed_key(&h, &cfg).as_deref(), Some("C"));
}

#[tokio::test]
async fn non_owner_of_token_is_404_not_403() {
    let (pool, owner_id, key) = setup().await;
    seed_endpoint(&pool, &owner_id, "tokAAAAAAA").await;
    let SeededOwner {
        owner_id: b_id,
        key: b_key,
    } = seed_owner(&pool).await;

    assert!(require_owned_endpoint(&pool, "tokAAAAAAA", &owner_id)
        .await
        .is_ok());
    // Owner B sees 404 (never 403) for a token that exists but is not theirs.
    let err = require_owned_endpoint(&pool, "tokAAAAAAA", &b_id)
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::NOT_FOUND);
    assert_eq!(err.code, "not_found");
    // A token that does not exist is also 404 (indistinguishable).
    let err2 = require_owned_endpoint(&pool, "doesnotexi", &owner_id)
        .await
        .unwrap_err();
    assert_eq!(err2.status, StatusCode::NOT_FOUND);

    // Feed/tunnel key gate: right key+token true; wrong-owner key false.
    assert!(key_owns_token(&pool, "tokAAAAAAA", Some(&key)).await);
    assert!(!key_owns_token(&pool, "tokAAAAAAA", Some(&b_key)).await);
    assert!(!key_owns_token(&pool, "tokAAAAAAA", None).await);
    assert!(!key_owns_token(&pool, "tokAAAAAAA", Some("")).await);
    assert!(!key_owns_token(&pool, "", Some(&key)).await);
}
