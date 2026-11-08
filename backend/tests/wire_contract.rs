//! Wire-contract golden tests — the S0 safety net for refactor program
//! hookbox-a73. Each scenario records status, response headers and body bytes
//! into `tests/golden/<scenario>.txt`; behavior-preserving refactor phases must
//! pass these with ZERO edits to the golden files. Intentional contract changes
//! regenerate the affected files in the same commit (`UPDATE_GOLDEN=1`).
//!
//! Rendering rules (deterministic across runs):
//! - the endpoint token is masked as `<token>` in header values and bodies;
//! - volatile header values (`date`, `retry-after`, …) render as `<volatile>`;
//! - 2xx JSON objects render as their sorted top-level keys (they carry
//!   timestamps/ids); every other body renders as exact bytes.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{HeaderMap, Request};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use hookbox::config::Config;
use hookbox::db;
use hookbox::router::build_app;
use hookbox::state::AppState;

const APP_HOST: &str = "app.local";
const VOLATILE_HEADERS: [&str; 3] = ["date", "retry-after", "x-ratelimit-reset"];

// --- harness ------------------------------------------------------------------

struct Harness {
    app: axum::Router,
    secret: String,
}

async fn harness() -> Harness {
    harness_with(|_| {}).await
}

async fn harness_with(tweak: impl FnOnce(&mut Config)) -> Harness {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", APP_HOST);
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    tweak(&mut cfg);
    let app = build_app(AppState::new(pool, cfg));
    let session = Req::post("/api/session")
        .json(json!({"email": "golden@e.com"}))
        .send(&app)
        .await;
    let secret = session.json()["owner_secret"].as_str().unwrap().to_string();
    Harness { app, secret }
}

impl Harness {
    fn authed(&self, req: Req) -> Req {
        req.bearer(&self.secret)
    }

    async fn endpoint(&self) -> String {
        let c = self
            .authed(Req::post("/api/endpoints").json(json!({"name": "golden"})))
            .send(&self.app)
            .await;
        assert_eq!(c.status, 201, "endpoint create");
        c.json()["token"].as_str().unwrap().to_string()
    }

    async fn patch_endpoint(&self, token: &str, body: Value) -> Captured {
        self.authed(Req::patch(&format!("/api/endpoints/{token}")).json(body))
            .send(&self.app)
            .await
    }

    async fn rule(&self, token: &str, body: Value) -> Captured {
        self.authed(Req::post(&format!("/api/endpoints/{token}/rules")).json(body))
            .send(&self.app)
            .await
    }
}

struct Req {
    method: &'static str,
    path: String,
    host: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Req {
    fn new(method: &'static str, path: &str) -> Self {
        Req {
            method,
            path: path.to_string(),
            host: APP_HOST.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
    fn get(path: &str) -> Self {
        Self::new("GET", path)
    }
    fn post(path: &str) -> Self {
        Self::new("POST", path)
    }
    fn patch(path: &str) -> Self {
        Self::new("PATCH", path)
    }
    fn delete(path: &str) -> Self {
        Self::new("DELETE", path)
    }
    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
    fn bearer(self, secret: &str) -> Self {
        self.header("authorization", &format!("Bearer {secret}"))
    }
    fn json(self, body: Value) -> Self {
        self.raw("application/json", serde_json::to_vec(&body).unwrap())
    }
    fn raw(mut self, content_type: &str, body: Vec<u8>) -> Self {
        self.body = body;
        self.header("content-type", content_type)
    }
    async fn send(self, app: &axum::Router) -> Captured {
        let builder = self.headers.iter().fold(
            Request::builder()
                .method(self.method)
                .uri(&self.path)
                .header("host", &self.host),
            |b, (k, v)| b.header(k.as_str(), v.as_str()),
        );
        let resp = app
            .clone()
            .oneshot(builder.body(Body::from(self.body)).unwrap())
            .await
            .unwrap();
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let body = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        Captured {
            status,
            headers,
            body,
        }
    }
}

struct Captured {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Captured {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    fn render(&self, label: &str, token: &str) -> String {
        let mask = |s: &str| {
            if token.is_empty() {
                s.to_string()
            } else {
                s.replace(token, "<token>")
            }
        };
        let mut names: Vec<&str> = self.headers.keys().map(|k| k.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        let headers: String = names
            .iter()
            .flat_map(|name| {
                self.headers.get_all(*name).iter().map(move |v| {
                    let value = if VOLATILE_HEADERS.contains(name) {
                        "<volatile>".to_string()
                    } else {
                        mask(v.to_str().unwrap_or("<binary>"))
                    };
                    format!("  {name}: {value}\n")
                })
            })
            .collect();
        format!(
            "## {label}\nstatus: {}\nheaders:\n{headers}body: {}\n\n",
            self.status,
            mask(&self.body_repr())
        )
    }

    fn body_repr(&self) -> String {
        if self.body.is_empty() {
            return "<empty>".to_string();
        }
        if (200..300).contains(&self.status) {
            if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&self.body) {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort_unstable();
                let keys: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                return format!("json-keys [{}]", keys.join(", "));
            }
        }
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn assert_golden(scenario: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{scenario}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {scenario}; run with UPDATE_GOLDEN=1"));
    assert_eq!(
        expected, actual,
        "wire contract drift in scenario `{scenario}`"
    );
}

// --- management plane -----------------------------------------------------------

#[tokio::test]
async fn golden_management_auth_and_not_found() {
    let h = harness().await;
    let mut out = String::new();
    let c = Req::get("/api/endpoints").send(&h.app).await;
    out += &c.render("no credential", "");
    let c = Req::get("/api/endpoints").bearer("nope").send(&h.app).await;
    out += &c.render("unknown bearer", "");
    let c = h
        .authed(Req::get("/api/endpoints/zzzzzzzzzzzz"))
        .send(&h.app)
        .await;
    out += &c.render("foreign or missing endpoint", "");
    let c = Req::post("/api/session")
        .json(json!({"email": "nope"}))
        .send(&h.app)
        .await;
    out += &c.render("session invalid email", "");
    assert_golden("management_auth_and_not_found", &out);
}

#[tokio::test]
async fn golden_endpoint_patch_validation() {
    let h = harness().await;
    let token = h.endpoint().await;
    let cases = [
        ("non-object body", json!([1])),
        ("name too long", json!({"name": "x".repeat(101)})),
        ("bad chaos_mode", json!({"chaos_mode": "bogus"})),
        ("bad default_mode", json!({"default_mode": "bogus"})),
        (
            "bad target_url scheme",
            json!({"target_url": "ftp://example.com"}),
        ),
        ("latency clamp", json!({"latency_ms": 999_999})),
        (
            "valid patch",
            json!({"name": "renamed", "cors_enabled": true}),
        ),
    ];
    let mut out = String::new();
    for (label, body) in cases {
        out += &h.patch_endpoint(&token, body).await.render(label, &token);
    }
    assert_golden("endpoint_patch_validation", &out);
}

#[tokio::test]
async fn golden_rule_create_and_patch_validation() {
    let h = harness().await;
    let token = h.endpoint().await;
    let mut out = String::new();
    let creates = [
        ("create name too long", json!({"name": "x".repeat(121)})),
        ("create bad priority", json!({"priority": -5})),
        (
            "create bad status",
            json!({"response": {"status_code": 99}}),
        ),
        ("create bad chaos", json!({"chaos_mode": "bogus"})),
        ("create malformed json shape", json!({"priority": "high"})),
    ];
    for (label, body) in creates {
        out += &h.rule(&token, body).await.render(label, &token);
    }
    let ok = h.rule(&token, json!({"priority": 10})).await;
    out += &ok.render("create ok", &token);
    let id = ok.json()["id"].as_i64().unwrap();
    let rule_path = format!("/api/endpoints/{token}/rules/{id}");
    let patches = [
        ("patch bad priority", json!({"priority": -1})),
        (
            "patch bad status",
            json!({"response": {"status_code": 1000}}),
        ),
        ("patch bad chaos", json!({"chaos_mode": "bogus"})),
    ];
    for (label, body) in patches {
        let c = h
            .authed(Req::patch(&rule_path).json(body))
            .send(&h.app)
            .await;
        out += &c.render(label, &token);
    }
    let missing = format!("/api/endpoints/{token}/rules/999999");
    let c = h.authed(Req::get(&missing)).send(&h.app).await;
    out += &c.render("get missing rule", &token);
    let c = h.authed(Req::delete(&rule_path)).send(&h.app).await;
    out += &c.render("delete rule", &token);
    assert_golden("rule_create_and_patch_validation", &out);
}

#[tokio::test]
async fn golden_requests_state_collections_shapes() {
    let h = harness().await;
    let token = h.endpoint().await;
    let base = format!("/api/endpoints/{token}");
    let steps = [
        (
            "list requests bad limit",
            Req::get(&format!("{base}/requests?limit=500")),
        ),
        ("clear requests", Req::delete(&format!("{base}/requests"))),
        ("get state", Req::get(&format!("{base}/state"))),
        ("clear state", Req::delete(&format!("{base}/state"))),
        (
            "peek collection",
            Req::get(&format!("{base}/collections/items")),
        ),
        (
            "peek bad collection",
            Req::get(&format!("{base}/collections/bad%20name")),
        ),
        (
            "clear collection",
            Req::delete(&format!("{base}/collections/items")),
        ),
        ("get missing request", Req::get("/api/requests/999999")),
    ];
    let mut out = String::new();
    for (label, req) in steps {
        out += &h.authed(req).send(&h.app).await.render(label, &token);
    }
    let c = h.authed(Req::delete(&base)).send(&h.app).await;
    out += &c.render("delete endpoint", &token);
    assert_golden("requests_state_collections_shapes", &out);
}

#[tokio::test]
async fn golden_public_share_not_found() {
    let h = harness().await;
    let c = Req::get("/api/share/zzzzzzzzzzzzzzzzzzzzzz/requests")
        .send(&h.app)
        .await;
    assert_golden("public_share_not_found", &c.render("unknown code", ""));
}

// --- mock plane -----------------------------------------------------------------

#[tokio::test]
async fn golden_mock_unknown_gone_and_default() {
    let h = harness().await;
    let token = h.endpoint().await;
    let mut out = String::new();
    let c = Req::get("/e/zzzzzzzzzzzz/x").send(&h.app).await;
    out += &c.render("unknown token path-fallback", &token);
    let mut wildcard = Req::get("/x");
    wildcard.host = "zzzzzzzzzzzz.mock.local".to_string();
    out += &wildcard
        .send(&h.app)
        .await
        .render("unknown token wildcard", &token);
    let c = Req::get(&format!("/e/{token}/nothing")).send(&h.app).await;
    out += &c.render("default no_match", &token);
    h.authed(Req::delete(&format!("/api/endpoints/{token}")))
        .send(&h.app)
        .await;
    let c = Req::get(&format!("/e/{token}/x")).send(&h.app).await;
    out += &c.render("gone endpoint", &token);
    assert_golden("mock_unknown_gone_and_default", &out);
}

#[tokio::test]
async fn golden_mock_rule_hit_echo_and_cors() {
    let h = harness().await;
    let token = h.endpoint().await;
    let rule = json!({
        "priority": 10,
        "match": {"method": "GET", "path": "/hello"},
        "response": {"status_code": 202, "content_type": "text/plain",
                     "headers": {"x-custom": "1"}, "body_template": "hi {{request.method}}"}
    });
    h.rule(&token, rule).await;
    let mut out = String::new();
    let c = Req::get(&format!("/e/{token}/hello")).send(&h.app).await;
    out += &c.render("rule hit", &token);
    h.patch_endpoint(
        &token,
        json!({"default_mode": "echo", "cors_enabled": true}),
    )
    .await;
    let c = Req::post(&format!("/e/{token}/echo?a=1"))
        .json(json!({"k": "v"}))
        .send(&h.app)
        .await;
    out += &c.render("echo", &token);
    let c = Req::new("OPTIONS", &format!("/e/{token}/anything"))
        .header("origin", "https://client.example")
        .header("access-control-request-method", "POST")
        .send(&h.app)
        .await;
    out += &c.render("cors preflight", &token);
    assert_golden("mock_rule_hit_echo_and_cors", &out);
}

#[tokio::test]
async fn golden_mock_conditions() {
    let h = harness_with(|cfg| cfg.max_ingest_body_bytes = 16).await;
    let token = h.endpoint().await;
    let mut out = String::new();
    let c = Req::post(&format!("/e/{token}/big"))
        .raw("text/plain", vec![b'a'; 64])
        .header("content-length", "64")
        .send(&h.app)
        .await;
    out += &c.render("payload too large (content-length)", &token);
    let c = Req::post(&format!("/e/{token}/big"))
        .raw("text/plain", vec![b'a'; 64])
        .send(&h.app)
        .await;
    out += &c.render("payload too large (streamed)", &token);
    h.patch_endpoint(&token, json!({"rate_limit_per_min": 1}))
        .await;
    Req::get(&format!("/e/{token}/a")).send(&h.app).await;
    let c = Req::get(&format!("/e/{token}/a")).send(&h.app).await;
    out += &c.render("rate limited", &token);
    let chaos = h.endpoint().await;
    h.patch_endpoint(&chaos, json!({"chaos_pct": 100, "chaos_mode": "dropout"}))
        .await;
    let c = Req::get(&format!("/e/{chaos}/a")).send(&h.app).await;
    out += &c.render("chaos dropout", &chaos);
    h.patch_endpoint(&chaos, json!({"chaos_mode": "error"}))
        .await;
    let c = Req::get(&format!("/e/{chaos}/a")).send(&h.app).await;
    assert!(
        [502, 503, 504].contains(&c.status),
        "chaos status {}",
        c.status
    );
    let code = c.status.to_string();
    let masked = Captured {
        status: 0,
        headers: c.headers.clone(),
        body: String::from_utf8_lossy(&c.body)
            .replace(&code, "<status>")
            .into_bytes(),
    };
    out += &masked.render("chaos error (status masked)", &chaos);
    assert_golden("mock_conditions", &out);
}

#[tokio::test]
async fn golden_mock_auto_crud() {
    let h = harness().await;
    let token = h.endpoint().await;
    h.patch_endpoint(&token, json!({"auto_crud": true})).await;
    let base = format!("/e/{token}/items");
    let mut out = String::new();
    let created = Req::post(&base).json(json!({"a": 1})).send(&h.app).await;
    out += &created.render("create item", &token);
    let id = created.json()["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let c = Req::get(&format!("{base}/{id}")).send(&h.app).await;
    out += &c.render("get item", &token);
    let c = Req::get(&format!("{base}/nope")).send(&h.app).await;
    out += &c.render("get missing item", &token);
    let c = Req::post(&base)
        .raw("application/json", b"{not json".to_vec())
        .send(&h.app)
        .await;
    out += &c.render("create invalid json", &token);
    let c = Req::delete(&format!("{base}/{id}")).send(&h.app).await;
    out += &c.render("delete item", &token);
    assert_golden("mock_auto_crud", &out);
}

#[tokio::test]
async fn golden_mock_mitm_blocked_upstream() {
    let h = harness().await;
    let token = h.endpoint().await;
    let mut out = String::new();
    let c = h
        .patch_endpoint(&token, json!({"target_url": "http://127.0.0.1:9"}))
        .await;
    out += &c.render("patch loopback target", &token);
    let c = Req::get(&format!("/e/{token}/proxied")).send(&h.app).await;
    out += &c.render("proxied request", &token);
    assert_golden("mock_mitm_blocked_upstream", &out);
}
