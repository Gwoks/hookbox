//! F4 share links, owner side (§5.1): mint / list / revoke, auth and owner
//! scoping, caps and labels, tombstone revocation, `last_used_at`, share URL.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common::{
    call, create_owner, list_shares_of, mint_share, new_endpoint, public_list, revoke_share,
    TestApp,
};

#[tokio::test]
async fn share_mint_list_revoke_happy_path() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;

    let (s, body, h) = mint_share(&app, &secret, &token, Some("Acme vendor")).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
    let id = body["id"].as_i64().unwrap();
    let code = body["code"].as_str().unwrap().to_string();
    assert!(body["url"]
        .as_str()
        .unwrap()
        .ends_with(&format!("/s/{code}")));
    assert_eq!(body["label"], json!("Acme vendor"));
    assert!(body["last_used_at"].is_null());
    assert!(code.len() >= 32);

    // List: the id shows up, but never the code or url (AC-25).
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], json!(id));
    assert_eq!(arr[0]["label"], json!("Acme vendor"));
    assert!(arr[0].as_object().unwrap().get("code").is_none());
    assert!(arr[0].as_object().unwrap().get("url").is_none());

    // The freshly minted code resolves publicly.
    let (s, feed, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(feed["requests"], json!([]));

    // Revoke -> 204, then the list is empty and the code 404s (AC-37: takes
    // effect on the very next request — share_links is never cached).
    let (s, body, _) = revoke_share(&app, &secret, &token, id).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(body.is_null());
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 0);
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_owner_routes_401_missing_malformed_unknown_bearer() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    // Missing Authorization entirely, across all three owner routes.
    for (method, path) in [
        ("POST", format!("/api/endpoints/{token}/shares")),
        ("GET", format!("/api/endpoints/{token}/shares")),
        ("DELETE", format!("/api/endpoints/{token}/shares/{id}")),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(&path)
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} without Authorization"
        );
        assert_eq!(resp.headers().get("www-authenticate").unwrap(), "Bearer");
    }

    // Malformed scheme (Basic, not Bearer).
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/endpoints/{token}/shares"))
        .header("host", "app.local")
        .header("authorization", "Basic xyz")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A syntactically valid but unknown secret, across all three routes.
    let (s, _b, _) = mint_share(&app, "not-a-real-secret", &token, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _b, _) = list_shares_of(&app, "not-a-real-secret", &token).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _b, _) = revoke_share(&app, "not-a-real-secret", &token, id).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn share_owner_routes_404_not_403_for_foreign_token() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    // A second, unrelated owner.
    let stranger_secret = create_owner(&app).await;

    let (s, body, _) = mint_share(&app, &stranger_secret, &token, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("not_found"));
    let (s, _b, _) = list_shares_of(&app, &stranger_secret, &token).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _b, _) = revoke_share(&app, &stranger_secret, &token, id).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_active_cap_422() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    // Default cap (SHARE_MAX_PER_ENDPOINT) is 10.
    for _ in 0..10 {
        let (s, _b, _) = mint_share(&app, &secret, &token, None).await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let (s, body, _) = mint_share(&app, &secret, &token, None).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));
}

#[tokio::test]
async fn share_label_422_over_80_chars_and_blank_trims_to_null() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let long = "x".repeat(81);
    let (s, body, _) = mint_share(&app, &secret, &token, Some(&long)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));

    // Exactly 80 is fine; a label that trims to empty stores as NULL.
    let ok = "y".repeat(80);
    let (s, _b, _) = mint_share(&app, &secret, &token, Some(&ok)).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, body, _) = mint_share(&app, &secret, &token, Some("   ")).await;
    assert_eq!(s, StatusCode::CREATED);
    assert!(body["label"].is_null());
}

#[tokio::test]
async fn share_mint_on_tombstoned_endpoint_404() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;
    let (s, body, _) = mint_share(&app, &secret, &token, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("not_found"));
}

#[tokio::test]
async fn share_tombstone_revokes_all_and_belt_and_braces_gone_at_check() {
    let TestApp { app, state, secret } = TestApp::new().await;
    let pool = state.pool.clone();
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;

    // The share_links revoke ran as part of the tombstone -> already 404.
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // AC-S9 belt-and-braces: simulate the revoke statement having failed by
    // directly clearing revoked_at back to NULL, as if the UPDATE never took
    // effect. The resolver's OWN `e.gone_at IS NULL` check in the same joined
    // statement still 404s regardless.
    sqlx::query("UPDATE share_links SET revoked_at = NULL WHERE token = ?")
        .bind(&token)
        .execute(&pool)
        .await
        .unwrap();
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_last_used_at_written_off_path_and_coalesced() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert!(list.as_array().unwrap()[0]["last_used_at"].is_null());

    public_list(&app, &code, "").await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    let first_touch = list.as_array().unwrap()[0]["last_used_at"].clone();
    assert!(!first_touch.is_null());

    // A second poll within 60s does not move it again (coalesced, AC-S10).
    public_list(&app, &code, "").await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(list.as_array().unwrap()[0]["last_used_at"], first_touch);
}

#[tokio::test]
async fn share_url_built_from_public_base_url_never_mock_host() {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.public_base_url = "https://hookbox.example.com".to_string();
    })
    .await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();
    assert_eq!(
        body["url"],
        json!(format!("https://hookbox.example.com/s/{code}"))
    );
    assert!(!body["url"].as_str().unwrap().contains("mock.local"));
}

#[tokio::test]
async fn share_revoke_without_auth_401_and_nothing_revoked() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    let (s, _b, _) = call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}/shares/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(list.as_array().unwrap().len(), 1); // still active, nothing revoked
}
