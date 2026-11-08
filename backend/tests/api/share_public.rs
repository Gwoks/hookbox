//! F4 public share resolver (§5.2): the single byte-identical 404, check order
//! (rate limit -> params -> code shape -> resolution), rate limits, no-store on
//! every outcome, and read-only routes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use hookbox::ids;

use crate::common::{
    add_rule, call, list_requests, list_shares_of, mint_share, new_endpoint, public_detail,
    public_list, revoke_share, TestApp,
};

#[tokio::test]
async fn share_404_identity_unknown_revoked_tombstoned_byte_identical_incl_head() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;

    // Unknown code: correctly shaped, never minted.
    let unknown_code = ids::gen_share_code(24);

    // Revoked code.
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let revoked_id = body["id"].as_i64().unwrap();
    let revoked_code = body["code"].as_str().unwrap().to_string();
    revoke_share(&app, &secret, &token, revoked_id).await;

    // Tombstoned-endpoint code.
    let token2 = new_endpoint(&app, &secret).await;
    let (_s, body2, _) = mint_share(&app, &secret, &token2, None).await;
    let tombstoned_code = body2["code"].as_str().unwrap().to_string();
    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token2}"),
        Some(&secret),
        None,
    )
    .await;

    let mut bodies = Vec::new();
    for code in [&unknown_code, &revoked_code, &tombstoned_code] {
        let (s, b, h) = public_list(&app, code, "").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(h.get("cache-control").unwrap(), "no-store");
        bodies.push(b);
    }
    assert_eq!(bodies[0], bodies[1]);
    assert_eq!(bodies[1], bodies[2]);

    // HEAD is auto-implemented and rate-limited too; same status + headers.
    let req = Request::builder()
        .method("HEAD")
        .uri(format!("/api/share/{unknown_code}/requests"))
        .header("host", "app.local")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_detail_cross_endpoint_request_id_404() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token_a = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token_a,
        json!({"match":{"method":"GET","path":"/p"},"response":{"status_code":200}}),
    )
    .await;
    let host_a = format!("{token_a}.mock.local");
    call(&app, "GET", &host_a, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let list_a = list_requests(&app, &secret, &token_a).await;
    let request_id_a = list_a.as_array().unwrap()[0]["id"].as_i64().unwrap();

    let token_b = new_endpoint(&app, &secret).await;
    let (_s, body_b, _) = mint_share(&app, &secret, &token_b, None).await;
    let code_b = body_b["code"].as_str().unwrap().to_string();

    // request_id_a belongs to token_a; code_b's share is scoped to token_b —
    // the `AND token = ?` inside the resolved statement is the whole of AC-35.
    let (s, _b, _) = public_detail(&app, &code_b, request_id_a).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_limit_validation_precedes_code_resolution() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let valid_code = body["code"].as_str().unwrap().to_string();
    let invalid_code = ids::gen_share_code(24); // correctly shaped, but unknown

    // `?limit=999` -> 422 for BOTH a valid and an invalid code (AC-101): if
    // this were checked after code resolution, the two would differ (422 vs
    // 404) and that difference would be a live/dead-code oracle.
    for code in [&valid_code, &invalid_code] {
        let (s, body, _) = public_list(&app, code, "limit=999").await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], json!("validation_error"));
    }
    let (s, body, _) = public_list(&app, &valid_code, "offset=-1").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));
}

// hookbox-mun.41: a malformed query string (`?limit=abc`) used to be
// rejected by axum's `Query<PublicRequestsQuery>` extractor with a 400
// BEFORE the handler — and therefore `check_share_rate_limit` — ever ran,
// so it was completely unmetered. Assert both halves of the fix: (a) a
// malformed `limit` gets the SAME 422 validation error as an out-of-range
// one (not axum's generic 400), and (b) it is counted against the limiter
// exactly like a well-formed request.
#[tokio::test]
async fn share_malformed_query_returns_422_and_is_still_rate_limited() {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.share_rate_limit_per_min = 2;
    })
    .await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    // A malformed `limit` is a 422 validation error, not axum's bare 400 —
    // consistent with the existing out-of-range message, never a live/dead
    // code oracle (AC-36/AC-101 unaffected: it fails identically before any
    // code resolution).
    let (s1, body1, _) = public_list(&app, &code, "limit=abc").await;
    assert_eq!(s1, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body1["error"], json!("validation_error"));

    // The FIRST malformed request above already consumed one unit of the
    // `share_rate_limit_per_min = 2` bucket. One more malformed request
    // consumes the second...
    let (s2, _b2, _) = public_list(&app, &code, "limit=abc").await;
    assert_eq!(s2, StatusCode::UNPROCESSABLE_ENTITY);
    // ...and the THIRD — still malformed — must now be rate limited, proving
    // the limiter counted the two malformed requests above rather than
    // letting them through for free.
    let (s3, body3, h3) = public_list(&app, &code, "limit=abc").await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body3["error"], json!("rate_limited"));
    assert!(h3.get("retry-after").is_some());
}

// Same root cause as hookbox-mun.41, on the SIBLING public route: `Path<
// (String, i64)>` on GET /api/share/{code}/requests/{id} is just as fallible
// as the query extractor was — a non-numeric `{id}` used to 400 at the axum
// layer before `check_share_rate_limit` ever ran. Assert it is now counted,
// and collapses into the SAME 404 as an unknown id (AC-36), not a distinct
// error that would tell a scanner it guessed a syntactically-plausible id.
#[tokio::test]
async fn share_non_numeric_request_id_is_404_and_still_rate_limited() {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.share_rate_limit_per_min = 2;
    })
    .await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (s1, _b1, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/share/{code}/requests/not-a-number"),
        None,
        None,
    )
    .await;
    assert_eq!(
        s1,
        StatusCode::NOT_FOUND,
        "a non-numeric id must 404 like any other unresolvable id, never axum's bare 400"
    );

    let (s2, _b2, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/share/{code}/requests/not-a-number"),
        None,
        None,
    )
    .await;
    assert_eq!(s2, StatusCode::NOT_FOUND);

    let (s3, _b3, h3) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/share/{code}/requests/not-a-number"),
        None,
        None,
    )
    .await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert!(h3.get("retry-after").is_some());
}

#[tokio::test]
async fn share_rate_limit_429_per_ip_with_retry_after() {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.share_rate_limit_per_min = 2;
    })
    .await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (s1, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s1, StatusCode::OK);
    let (s2, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s2, StatusCode::OK);
    let (s3, body3, h3) = public_list(&app, &code, "").await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body3["error"], json!("rate_limited"));
    assert!(h3.get("retry-after").is_some());
    assert_eq!(h3.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_rate_limit_429_global_ceiling() {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.share_rate_limit_per_min = 1_000; // effectively unlimited per source
        cfg.share_rate_limit_global_per_min = 2; // tiny instance-wide ceiling
    })
    .await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    for i in 0..2 {
        let (s, _b, _) = public_list(&app, &code, "").await;
        assert_eq!(s, StatusCode::OK, "request {i} should pass");
    }
    let (s, _b, h) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_public_routes_503_carries_no_store() {
    let TestApp { app, state, secret } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();
    // Close the pool under the live app: every DB read now fails, so the
    // public resolver bubbles a 503 — which must still carry no-store.
    state.pool.close().await;

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/share/{code}/requests"))
        .header("host", "app.local")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_public_routes_405_for_other_verbs_and_tables_unchanged() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    for method in ["POST", "PATCH", "PUT", "DELETE"] {
        let req = Request::builder()
            .method(method)
            .uri(format!("/api/share/{code}/requests"))
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} should 405"
        );
    }
    // The share itself is still there (untouched — no accidental mutation).
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn share_code_never_appears_in_error_detail() {
    let TestApp { app, .. } = TestApp::new().await;
    let code = ids::gen_share_code(24);
    let (s, body, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let detail = body["detail"].as_str().unwrap();
    assert!(!detail.contains(&code));
}
