//! §5.2 management surface: session shape, endpoint/rule CRUD, request
//! paging, state + collection peeks, owner scoping, and the flat error envelope.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{
    add_rule, call, call_raw, create_owner, list_requests, mint_session, new_endpoint,
    patch_endpoint, TestApp, APP_HOST,
};

#[tokio::test]
async fn mock_url_uses_public_base_url_in_path_fallback_mode() {
    let TestApp { app, .. } = TestApp::with_config(|cfg| {
        cfg.path_fallback_only = true;
        cfg.public_base_url = "https://hookbox.example.com".to_string();
    })
    .await;

    let (s, body, _) = mint_session(&app).await;
    assert_eq!(s, StatusCode::CREATED);
    let ep = &body["primary"];
    let token = ep["token"].as_str().unwrap();
    assert_eq!(
        ep["mock_url"],
        json!(format!("https://hookbox.example.com/e/{token}"))
    );
    assert_eq!(
        ep["path_url"],
        json!(format!("https://hookbox.example.com/e/{token}"))
    );
}

#[tokio::test]
async fn mock_url_stays_relative_in_path_fallback_mode_without_base_url() {
    let TestApp { app, .. } = TestApp::with_config(|cfg| {
        cfg.path_fallback_only = true;
        cfg.public_base_url = String::new();
    })
    .await;

    let (s, body, _) = mint_session(&app).await;
    assert_eq!(s, StatusCode::CREATED);
    let ep = &body["primary"];
    let token = ep["token"].as_str().unwrap();
    assert_eq!(ep["mock_url"], json!(format!("/e/{token}")));
    assert_eq!(ep["path_url"], json!(format!("/e/{token}")));
}

#[tokio::test]
async fn session_shape_and_flat_error_envelope() {
    let TestApp { app, .. } = TestApp::new().await;
    // Unauthed /api -> 401 flat envelope + WWW-Authenticate.
    let (s, body, h) = call(&app, "GET", "app.local", "/api/endpoints", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], json!("unauthorized"));
    assert!(body["detail"].is_string());
    assert_eq!(h.get("www-authenticate").unwrap(), "Bearer");
    // A session call without the SPA's CSRF header -> 403 flat envelope.
    let (s, body, _) = call(&app, "POST", "app.local", "/api/session", None, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], json!("csrf_rejected"));
    assert!(body["detail"].is_string());
}

#[tokio::test]
async fn rules_ordered_and_204_delete() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let r1 = add_rule(
        &app,
        &secret,
        &token,
        json!({"priority": 50, "match": {"method":"GET"}}),
    )
    .await;
    add_rule(&app, &secret, &token, json!({"priority": 10})).await;
    let (s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/rules"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr[0]["priority"], json!(10)); // priority,id order
                                               // DELETE -> 204 no body.
    let (s, body, _) = call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}/rules/{r1}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(body.is_null());
}

// --- moved from router.rs unit tests (unique assertions only) ---

#[tokio::test]
async fn endpoint_crud_round_trip() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let create = json!({"name": "demo"});
    let (s, body, _) = call(
        &app,
        "POST",
        APP_HOST,
        "/api/endpoints",
        Some(&secret),
        Some(create),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let token = body["token"].as_str().unwrap().to_string();
    assert_eq!(body["chaos_mode"], json!("error")); // OQ-2 default present
    assert_eq!(body["default_mode"], json!("mock_404"));
    assert!(body.as_object().unwrap().contains_key("last_hit")); // null key present

    // List: the session's primary endpoint plus this one.
    let (s, body, h) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("x-hookbox-plane").unwrap(), "api");
    let listed = body.as_array().unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|e| e["token"] == json!(token)));

    // PATCH clamps latency and validates target_url.
    let patch = json!({"latency_ms": 999999, "chaos_mode": "dropout", "target_url": "https://up.example.com"});
    let (s, body, _) = patch_endpoint(&app, &secret, &token, patch).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["latency_ms"], json!(10000)); // clamped
    assert_eq!(body["chaos_mode"], json!("dropout"));
    assert_eq!(body["target_url"], json!("https://up.example.com"));
    let (s, _b, _) =
        patch_endpoint(&app, &secret, &token, json!({"target_url": "ftp://bad"})).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

    // DELETE tombstones and answers with the 200 Message envelope.
    let path = format!("/api/endpoints/{token}");
    let (s, body, _) = call(&app, "DELETE", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
}

#[tokio::test]
async fn rule_create_shape_and_get_after_delete_is_404() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let rules = format!("/api/endpoints/{token}/rules");
    let rule = json!({"priority": 50, "match": {"method": "GET"}});
    let (s, created, _) = call(&app, "POST", APP_HOST, &rules, Some(&secret), Some(rule)).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(created["match"]["method"], json!("GET"));
    assert!(created.as_object().unwrap().contains_key("webhook_action"));
    let path = format!("{rules}/{}", created["id"]);
    let (s, _b, _) = call(&app, "DELETE", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _b, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn fresh_endpoint_requests_state_and_collections_are_empty() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let base = format!("/api/endpoints/{token}");
    let path = format!("{base}/requests?limit=10");
    let (s, body, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!([]));
    let path = format!("{base}/state");
    let (s, body, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({"state": {}}));
    let path = format!("{base}/collections/widgets");
    let (s, body, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({"items": []}));
}

#[tokio::test]
async fn foreign_endpoint_is_404_not_403() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let stranger = create_owner(&app).await;
    let foreign = new_endpoint(&app, &stranger).await;
    let path = format!("/api/endpoints/{foreign}");
    let (s, body, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("not_found"));
}

/// B1: PATCH /api/endpoints/{token} types every field like create does — a
/// non-string `name` or `target_url` is a 422 that changes nothing (it used
/// to be stored stringified / silently clear the target).
#[tokio::test]
async fn endpoint_patch_rejects_mistyped_fields_without_writing() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let target = json!({"name": "kept", "target_url": "https://up.example.com"});
    let (s, _, _) = patch_endpoint(&app, &secret, &token, target).await;
    assert_eq!(s, StatusCode::OK);
    let cases = [
        (json!({"name": 5}), "name must be a string or null."),
        (json!({"name": {"a": 1}}), "name must be a string or null."),
        (
            json!({"target_url": 5}),
            "target_url must be a string or null.",
        ),
    ];
    for (body, detail) in cases {
        let (s, err, _) = patch_endpoint(&app, &secret, &token, body).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err, json!({"error": "validation_error", "detail": detail}));
    }
    let path = format!("/api/endpoints/{token}");
    let (_, ep, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    assert_eq!(ep["name"], json!("kept"));
    assert_eq!(ep["target_url"], json!("https://up.example.com"));
}

/// B1: PATCH on a rule enforces the create rules (name string-or-null and
/// <= 120 chars, the body_template cap) and stores `webhook_action` typed.
#[tokio::test]
async fn rule_patch_validates_like_create_and_types_webhook_action() {
    let TestApp { app, secret, state } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let id = add_rule(&app, &secret, &token, json!({"name": "r"})).await;
    let path = format!("/api/endpoints/{token}/rules/{id}");
    let big = "x".repeat(state.cfg.template_max_size + 1);
    let cases = [
        (json!({"name": 7}), "name must be a string or null."),
        (
            json!({"name": "x".repeat(121)}),
            "name must be at most 120 characters.",
        ),
        (
            json!({"response": {"body_template": big}}),
            "response.body_template is too large.",
        ),
        (
            json!({"webhook_action": {"url": 5}}),
            "webhook_action is malformed.",
        ),
    ];
    for (body, detail) in cases {
        let (s, err, _) = call(&app, "PATCH", APP_HOST, &path, Some(&secret), Some(body)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{detail}");
        assert_eq!(err["detail"], json!(detail));
    }
    let hook = json!({"webhook_action": {"url": "https://hook.example.com"}, "ignored": 1});
    let (s, rule, _) = call(&app, "PATCH", APP_HOST, &path, Some(&secret), Some(hook)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(rule["name"], json!("r"));
    assert_eq!(
        rule["webhook_action"],
        json!({"url": "https://hook.example.com", "body_template": ""})
    );
}

/// B5: once tombstoned, the endpoint is gone for its owner — every owner
/// route answers 410 `endpoint_gone` (a stranger still gets the 404), the
/// list drops it, and minting a share link stays the plain 404.
#[tokio::test]
async fn owner_routes_on_a_tombstoned_endpoint_are_410() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let stranger = create_owner(&app).await;
    let token = new_endpoint(&app, &secret).await;
    let rule = add_rule(&app, &secret, &token, json!({})).await;
    let mock_host = format!("{token}.mock.local");
    call(&app, "GET", &mock_host, "/traced", None, None).await;
    // The trace is written off the response path — wait for it.
    let mut trace = json!(null);
    for _ in 0..100 {
        trace = list_requests(&app, &secret, &token).await[0]["id"].clone();
        if !trace.is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(trace.is_i64(), "trace written");
    let base = format!("/api/endpoints/{token}");
    let (s, _, _) = call(&app, "DELETE", APP_HOST, &base, Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);

    let owner_routes = [
        ("GET", base.clone()),
        ("PATCH", base.clone()),
        ("DELETE", base.clone()),
        ("GET", format!("{base}/rules")),
        ("POST", format!("{base}/rules")),
        ("GET", format!("{base}/rules/{rule}")),
        ("PATCH", format!("{base}/rules/{rule}")),
        ("DELETE", format!("{base}/rules/{rule}")),
        ("GET", format!("{base}/requests")),
        ("DELETE", format!("{base}/requests")),
        ("GET", format!("/api/requests/{trace}")),
        ("GET", format!("{base}/state")),
        ("DELETE", format!("{base}/state")),
        ("GET", format!("{base}/collections/items")),
        ("DELETE", format!("{base}/collections/items")),
        ("GET", format!("{base}/shares")),
        ("DELETE", format!("{base}/shares/1")),
    ];
    for (method, path) in &owner_routes {
        let (s, body, _) = call(&app, method, APP_HOST, path, Some(&secret), Some(json!({}))).await;
        assert_eq!(s, StatusCode::GONE, "{method} {path}: {body}");
        assert_eq!(body["error"], json!("endpoint_gone"), "{method} {path}");
        let (s, _, _) = call(
            &app,
            method,
            APP_HOST,
            path,
            Some(&stranger),
            Some(json!({})),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND, "stranger {method} {path}");
    }

    let (_, list, _) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["token"] != json!(token)));
    let shares = format!("{base}/shares");
    let (s, _, _) = call(
        &app,
        "POST",
        APP_HOST,
        &shares,
        Some(&secret),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

/// B2: a body the create routes cannot decode gets the flat JSON envelope
/// (it used to be axum's text/plain message), keeping axum's status.
#[tokio::test]
async fn create_routes_reject_bad_json_with_the_flat_envelope() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let rules = format!("/api/endpoints/{token}/rules");
    let shares = format!("/api/endpoints/{token}/shares");
    let cases = [
        (
            rules.as_str(),
            JSON,
            r#"{"priority":"high"}"#,
            422,
            "validation_error",
        ),
        ("/api/endpoints", JSON, "{not json", 400, "bad_request"),
        (shares.as_str(), "text/plain", "{}", 415, "bad_request"),
    ];
    for (path, content_type, body, status, code) in cases {
        let (s, err, h) = call_raw(&app, "POST", path, &secret, content_type, body).await;
        assert_eq!(s.as_u16(), status, "{path}: {err}");
        assert_eq!(h.get("content-type").unwrap(), JSON);
        assert_eq!(err["error"], json!(code), "{path}");
        assert!(err["detail"].as_str().is_some_and(|d| !d.is_empty()));
    }
}

const JSON: &str = "application/json";

/// B7: a stored value that cannot decode into its column type (a BLOB in a
/// TEXT column keeps its storage class; 'high' in an INTEGER column stays
/// TEXT) is a stable 503 `store_unavailable` on every read path that loads
/// the row — never a panic turned into a bare 500.
#[tokio::test]
async fn row_decode_mismatch_is_503_store_unavailable() {
    let TestApp { app, secret, state } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(&app, &secret, &token, json!({"priority": 5})).await;
    sqlx::query("UPDATE endpoints SET name = x'00ff' WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE mock_rules SET priority = 'high' WHERE token = ?")
        .bind(&token)
        .execute(&state.pool)
        .await
        .unwrap();
    let paths = [
        "/api/session".to_string(),
        "/api/endpoints".to_string(),
        format!("/api/endpoints/{token}"),
        format!("/api/endpoints/{token}/rules"),
    ];
    for path in paths {
        let (s, body, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
        assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE, "{path}: {body}");
        assert_eq!(body["error"], json!("store_unavailable"), "{path}");
    }
}
