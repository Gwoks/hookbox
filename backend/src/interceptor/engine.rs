//! Core interceptor engine — PORT of `app/interceptor/engine.py` (§5.5).
//!
//! `handle_mock` owns the frozen resolution order
//! (OPTIONS → rule → Auto-CRUD → tunnel → MITM → default) and the conditions
//! wrap (rate-limit (429) → chaos (5xx/drop) → latency (sleep)). The endpoint
//! config + compiled rules come from the in-process `rule_cache` (no per-request
//! DB read on a match); per-endpoint state is read only when some rule gates on
//! it (lazy). The trace write + feed publish run OFF the response path (spawned,
//! never awaited) so a slow/failed trace never delays/fails the mock response.
//!
//! Identifying headers on every P1 response: `X-HookBox-Endpoint`,
//! `X-HookBox-Served-By`, `X-HookBox-Rule-Id` (only when a rule matched),
//! `X-HookBox-Truncated` (MITM only) + auto-CORS (P1, when enabled).

use std::collections::BTreeMap;
use std::time::Instant;

use axum::body::{to_bytes, Body, Bytes, HttpBody};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use serde_json::{json, Value};

use crate::db::{self, TraceRecord};
use crate::feed::FeedEvent;
use crate::helpers::{self, redact, value_to_string};
use crate::interceptor::{conditions, cors, crud, matcher, proxy, templating};
use crate::rule_cache::{CompiledEndpoint, Resolved};
use crate::state::AppState;

/// Trace step accumulated for the off-path persist.
#[derive(Clone)]
struct Step {
    step: String,
    detail: String,
}

/// Build the §5.5 P1 response for `(token, mock_path)`. `headers`/`query`/`body`
/// are the inbound request parts (headers names lower-cased upstream).
pub async fn handle_mock(
    state: &AppState,
    token: &str,
    mock_path: &str,
    method: &str,
    headers: HeaderMap,
    query: BTreeMap<String, String>,
    body: Vec<u8>,
) -> Response {
    let t0 = Instant::now();
    let method = method.to_ascii_uppercase();
    let headers_lower = lower_headers(&headers);

    // 0. Endpoint config + compiled rules from the cache (no DB read on a match).
    let resolved = match state.rule_cache.get(&state.pool, token).await {
        Ok(Some(r)) => r,
        Ok(None) => return unknown_or_gone(token, false),
        Err(_) => return unknown_or_gone(token, false),
    };
    let ep = match resolved {
        Resolved::Gone => return unknown_or_gone(token, true),
        Resolved::Live(ep) => ep,
    };

    // 0a. Ingest body cap (413 before buffering further — body already read, so
    // we reject oversize here as a backstop; the router enforces the limit too).
    if body.len() > state.cfg.max_ingest_body_bytes {
        let resp = json_response(
            413,
            &json!({"error":"payload_too_large","detail":"Request body exceeds the configured limit."}),
            "application/json",
        );
        return identified(resp, token, "default", None, &ep, &headers_lower);
    }
    let body_text = String::from_utf8_lossy(&body).to_string();

    let mut trace: Vec<Step> = Vec::new();
    let mut applied_latency_ms = 0i64;

    // 1. OPTIONS preflight short-circuit.
    if method == "OPTIONS" {
        let mut resp = Response::builder().status(204).body(Body::empty()).unwrap();
        for (k, v) in cors::preflight_headers(&headers_lower, ep.cors_enabled) {
            insert_header(resp.headers_mut(), &k, &v);
        }
        let resp = identified(resp, token, "cors", None, &ep, &headers_lower);
        let (resp, rb) = capture_response_body(resp).await;
        spawn_trace(
            state,
            token,
            &method,
            mock_path,
            204,
            "cors",
            None,
            t0,
            0,
            &headers_lower,
            &query,
            &body_text,
            &resp,
            &rb,
            &trace,
            &BTreeMap::new(),
        );
        return resp;
    }

    // 2. Lazily read state ONLY if some enabled rule gates on it.
    let mut endpoint_state: BTreeMap<String, String> = BTreeMap::new();
    if ep.any_rule_gates_on_state {
        match crate::state_store::read_state(&state.pool, token).await {
            Ok(s) => {
                trace.push(step("state_read", &format!("{} key(s)", s.len())));
                endpoint_state = s;
            }
            Err(_) => {
                // Fail closed: empty state -> gated rules skipped.
                trace.push(step(
                    "state_read",
                    "store error -> state empty (fail-closed)",
                ));
            }
        }
    }

    // 3. Match a rule.
    let served_by;
    let mut matched_rule_id: Option<i64> = None;
    let mut rule_latency_override: Option<i64> = None;
    let mut rule_rate_override: Option<i64> = None;
    let mut rule_chaos_override: Option<String> = None;
    let mut resp: Response;

    let selection = matcher::select(
        &ep.rules,
        &method,
        mock_path,
        &headers_lower,
        &query,
        &body_text,
        &endpoint_state,
    );

    if let Some(m) = selection {
        let rule = m.rule;
        matched_rule_id = Some(rule.id);
        served_by = "rule".into();
        rule_latency_override = rule.latency_ms;
        rule_rate_override = rule.rate_limit_per_min;
        rule_chaos_override = rule.chaos_mode.clone();
        trace.push(step(
            "match",
            &format!("rule {} (priority {})", rule.id, rule.priority),
        ));

        let mut ctx = templating::TemplateContext {
            method: method.clone(),
            path: mock_path.to_string(),
            query: query.clone(),
            headers: headers_lower.clone(),
            path_params: m.path_params.clone(),
            body: body_text.clone(),
            state: endpoint_state.clone(),
        };

        // 3a. State writes (rendered with the same ctx) BEFORE the body so
        // {{state.k}} sees the just-written value (AC-19).
        for w in &rule.state_writes {
            let value = templating::render(
                &w.value,
                &ctx,
                state.cfg.template_max_size,
                state.cfg.template_max_tags,
            );
            match crate::state_store::write_state(
                &state.pool,
                token,
                &w.key,
                &value,
                state.cfg.state_ttl_seconds,
            )
            .await
            {
                Ok(true) => {
                    ctx.state.insert(w.key.clone(), value.clone());
                    endpoint_state.insert(w.key.clone(), value.clone());
                    trace.push(step("state_write", &format!("{}={}", w.key, value)));
                    state.feed_hub.publish(
                        token,
                        FeedEvent::new(
                            "state_changed",
                            json!({"token": token, "key": w.key, "value": value}),
                        ),
                    );
                }
                Ok(false) => trace.push(step(
                    "state_write",
                    &format!("{}: unsafe key (skipped)", w.key),
                )),
                Err(_) => trace.push(step(
                    "state_write",
                    &format!("{}: store error (skipped)", w.key),
                )),
            }
        }

        let body_out = templating::render(
            &rule.response.body_template,
            &ctx,
            state.cfg.template_max_size,
            state.cfg.template_max_tags,
        );
        let status = rule.response.status_code.clamp(100, 599) as u16;
        let content_type = if rule.response.content_type.is_empty() {
            "application/json".to_string()
        } else {
            rule.response.content_type.clone()
        };
        resp = Response::builder()
            .status(status)
            .body(Body::from(body_out.clone()))
            .unwrap();
        insert_header(resp.headers_mut(), "content-type", &content_type);
        for (k, v) in &rule.response.headers {
            insert_header(resp.headers_mut(), k, v);
        }
        trace.push(step(
            "template",
            &format!("rendered {} bytes", body_out.len()),
        ));
    } else {
        // 4. No rule -> Auto-CRUD -> tunnel -> MITM -> default.
        let (r, sb) = resolve_unmatched(
            state,
            &ep,
            token,
            &method,
            mock_path,
            &query,
            &headers_lower,
            &body_text,
            &body,
            &mut trace,
        )
        .await;
        resp = r;
        served_by = sb;
    }

    // 5. Conditions: rate-limit -> chaos -> latency.
    let eff_rate = rule_rate_override.unwrap_or(ep.rate_limit_per_min);
    let eff_latency = rule_latency_override.unwrap_or(ep.latency_ms);
    let eff_chaos_mode = rule_chaos_override
        .clone()
        .unwrap_or_else(|| ep.chaos_mode.clone());

    let eff_rate = conditions::clamp_rate(Some(eff_rate), &state.cfg);
    if eff_rate > 0 {
        let key = crate::limiter::Limiter::key(
            token,
            matched_rule_id.filter(|_| rule_rate_override.is_some()),
        );
        let rl = state.limiter.check(&key, eff_rate, 60);
        if !rl.allowed {
            trace.push(step("rate_limit", &format!("429 (limit {eff_rate}/min)")));
            let mut r = json_response(
                429,
                &json!({"error":"rate_limited","detail":"Rate limit exceeded."}),
                "application/json",
            );
            insert_header(r.headers_mut(), "retry-after", &rl.retry_after.to_string());
            insert_header(r.headers_mut(), "x-ratelimit-limit", &rl.limit.to_string());
            insert_header(
                r.headers_mut(),
                "x-ratelimit-remaining",
                &rl.remaining.to_string(),
            );
            let r = identified(r, token, "ratelimit", matched_rule_id, &ep, &headers_lower);
            let (r, rb) = capture_response_body(r).await;
            spawn_trace(
                state,
                token,
                &method,
                mock_path,
                429,
                "ratelimit",
                matched_rule_id,
                t0,
                0,
                &headers_lower,
                &query,
                &body_text,
                &r,
                &rb,
                &trace,
                &endpoint_state,
            );
            return r;
        }
    }

    let chaos_pct = conditions::clamp_chaos(Some(ep.chaos_pct), &state.cfg);
    match conditions::roll_chaos(chaos_pct, &eff_chaos_mode) {
        conditions::Chaos::Drop => {
            trace.push(step("chaos", "dropout (connection closed)"));
            // D5 / R-DROPOUT: the client never gets a real response (a 499 +
            // Connection: close is synthesized below), so there is nothing to
            // capture — pass a literal `&[]`, with NO capture call. The trace
            // row keeps `status_code = 0` / `response_headers = {}` as-is;
            // this pre-existing low-fidelity row is deliberately not fixed by
            // F7 (a follow-up issue is filed for it separately).
            spawn_trace(
                state,
                token,
                &method,
                mock_path,
                0,
                "chaos",
                matched_rule_id,
                t0,
                0,
                &headers_lower,
                &query,
                &body_text,
                &Response::new(Body::empty()),
                &[],
                &trace,
                &endpoint_state,
            );
            // Signal a connection drop: empty body + Connection: close.
            let mut r = Response::builder().status(499).body(Body::empty()).unwrap();
            insert_header(r.headers_mut(), "connection", "close");
            return r;
        }
        conditions::Chaos::Status(code) => {
            trace.push(step("chaos", &code.to_string()));
            let r = json_response(
                code as u16,
                &json!({"error":"chaos","detail":"Injected chaos failure."}),
                "application/json",
            );
            let r = identified(r, token, "chaos", matched_rule_id, &ep, &headers_lower);
            let (r, rb) = capture_response_body(r).await;
            spawn_trace(
                state,
                token,
                &method,
                mock_path,
                code,
                "chaos",
                matched_rule_id,
                t0,
                0,
                &headers_lower,
                &query,
                &body_text,
                &r,
                &rb,
                &trace,
                &endpoint_state,
            );
            return r;
        }
        conditions::Chaos::None => {}
    }

    if eff_latency > 0 {
        applied_latency_ms = conditions::apply_latency(eff_latency, &state.cfg).await;
        trace.push(step("latency", &format!("{applied_latency_ms}ms")));
    }

    // 6. Identifying + CORS headers.
    let status = resp.status().as_u16() as i64;
    let resp = identified(
        resp,
        token,
        &served_by,
        matched_rule_id,
        &ep,
        &headers_lower,
    );

    // 6a. F7: buffer the finished body AFTER `identified()` (safe — that only
    // ever calls `insert_header`) so the trace can persist the exact bytes
    // the client is about to receive.
    let (resp, rb) = capture_response_body(resp).await;

    // AC-S3: the `default_mode = "echo"` payload's persisted `response_body`
    // must not carry the caller's raw headers, even though the client's echo
    // body still does (that response is already built, above, in `resp`/
    // `rb`). Rebuild ONLY the bytes handed to `spawn_trace` from
    // `redact_echo_persisted_headers(&headers_lower, token)` (hookbox-mun.36:
    // drops the structural `host`/`origin`/`referer` headers; hookbox-mun.37:
    // additionally MASKS any other header whose VALUE contains the endpoint
    // token, e.g. `x-forwarded-host`/`forwarded`/`x-original-uri`, mirroring
    // `routes::share::mask_token_in_value`; hookbox-mun.38: that comparison
    // is case-insensitive — see that function's doc comment).
    let persisted_body: std::borrow::Cow<'_, [u8]> =
        if served_by == "default" && ep.default_mode == "echo" {
            let redacted = echo_payload(
                &method,
                mock_path,
                &query,
                &redact_echo_persisted_headers(&headers_lower, token),
                &body_text,
            );
            std::borrow::Cow::Owned(serde_json::to_vec(&redacted).unwrap_or_default())
        } else {
            std::borrow::Cow::Borrowed(rb.as_ref())
        };

    // 7. Fire-and-forget trace + publish (never awaited).
    spawn_trace(
        state,
        token,
        &method,
        mock_path,
        status,
        &served_by,
        matched_rule_id,
        t0,
        applied_latency_ms,
        &headers_lower,
        &query,
        &body_text,
        &resp,
        &persisted_body,
        &trace,
        &endpoint_state,
    );
    resp
}

#[allow(clippy::too_many_arguments)]
async fn resolve_unmatched(
    state: &AppState,
    ep: &CompiledEndpoint,
    token: &str,
    method: &str,
    mock_path: &str,
    query: &BTreeMap<String, String>,
    headers_lower: &BTreeMap<String, String>,
    body_text: &str,
    body: &[u8],
    trace: &mut Vec<Step>,
) -> (Response, String) {
    // 4a. Auto-CRUD.
    if ep.auto_crud && crud::matches(mock_path) {
        match crud::handle(state, token, method, mock_path, body_text).await {
            Ok(cr) => {
                trace.push(step("crud", &format!("{method} {mock_path}")));
                let resp = match cr.body {
                    Some(b) => json_response(cr.status, &b, "application/json"),
                    None => Response::builder()
                        .status(cr.status)
                        .body(Body::empty())
                        .unwrap(),
                };
                return (resp, "crud".into());
            }
            Err(_) => {
                trace.push(step("crud", "store error -> 503"));
                return (
                    json_response(
                        503,
                        &json!({"error":"store_unavailable","detail":"Auto-CRUD store unavailable."}),
                        "application/json",
                    ),
                    "crud".into(),
                );
            }
        }
    }

    // 4b. Tunnel (if a CLI is bound).
    if state.tunnels.is_active(token) {
        match crate::routes::tunnel_ws::forward_to_tunnel(
            state,
            token,
            method,
            mock_path,
            query,
            headers_lower,
            body,
        )
        .await
        {
            Ok(resp) => {
                trace.push(step("tunnel", "forwarded to bound CLI"));
                return (resp, "tunnel".into());
            }
            Err(_) => {
                trace.push(step("tunnel", "no_tunnel"));
                return (
                    json_response(
                        504,
                        &json!({"error":"no_tunnel","detail":"Tunnel error."}),
                        "application/json",
                    ),
                    "tunnel".into(),
                );
            }
        }
    }

    // 4c. MITM forward.
    if let Some(target) = ep.target_url.as_deref() {
        if !target.trim().is_empty() {
            match proxy::forward(
                &state.cfg,
                target,
                method,
                mock_path,
                query,
                headers_lower,
                body,
            )
            .await
            {
                Ok(pr) => {
                    trace.push(step("forward", &format!("MITM -> {target}")));
                    let mut resp = Response::builder()
                        .status(pr.status)
                        .body(Body::from(pr.body))
                        .unwrap();
                    for (k, v) in &pr.headers {
                        insert_header(resp.headers_mut(), k, v);
                    }
                    return (resp, "mitm".into());
                }
                Err(proxy::ProxyError::Timeout) => {
                    trace.push(step("forward", "MITM timeout -> 504"));
                    return (
                        json_response(
                            504,
                            &json!({"error":"upstream_timeout","detail":"Upstream did not respond in time."}),
                            "application/json",
                        ),
                        "mitm".into(),
                    );
                }
                Err(proxy::ProxyError::Unreachable(_)) => {
                    trace.push(step("forward", "MITM unreachable -> 502"));
                    return (
                        json_response(
                            502,
                            &json!({"error":"upstream_unreachable","detail":"Could not reach the upstream target."}),
                            "application/json",
                        ),
                        "mitm".into(),
                    );
                }
            }
        }
    }

    // 4d. Default mode.
    if ep.default_mode == "echo" {
        trace.push(step("default", "echo"));
        let payload = echo_payload(method, mock_path, query, headers_lower, body_text);
        return (
            json_response(200, &payload, "application/json"),
            "default".into(),
        );
    }
    trace.push(step("default", "mock_404"));
    (
        json_response(
            404,
            &json!({"error":"no_match","detail":"No rule matched this request."}),
            "application/json",
        ),
        "default".into(),
    )
}

// --- helpers -----------------------------------------------------------------

fn step(s: &str, detail: &str) -> Step {
    Step {
        step: s.to_string(),
        detail: detail.to_string(),
    }
}

fn lower_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|vs| (k.as_str().to_ascii_lowercase(), vs.to_string()))
        })
        .collect()
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(n), Ok(v)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(n, v);
    }
}

/// Structural request headers dropped ENTIRELY from the echo payload's
/// persisted `headers` sub-object (hookbox-mun.36, channel C of AC-43/
/// AC-S2). In wildcard mock-host mode `Host` IS `<token>.<MOCK_DOMAIN>` on
/// every request, and a cross-origin browser echoes it back via `Origin`/
/// `Referer` — exactly the set `routes::share::PUBLIC_REQUEST_HEADER_DROP`
/// removes from the public projection's `request_headers` column.
///
/// This is a NAME-based drop list and, on its own, only closes the
/// structural carriers. Do NOT extend this list to chase the next proxy
/// header (hookbox-mun.37) — `redact_echo_persisted_headers` below also
/// applies a VALUE-based mask to every surviving header, which is what
/// actually generalizes the fix.
const ECHO_PERSIST_HEADER_DROP: [&str; 3] = ["host", "origin", "referer"];

/// The AC-S3 persist-path redaction (`redact()`, masking
/// authorization/cookie/x-owner-id) PLUS TWO follow-ups that must be applied
/// together (hookbox-mun.37 + hookbox-mun.38):
///   1. hookbox-mun.36: drop `ECHO_PERSIST_HEADER_DROP` (`host`/`origin`/
///      `referer`) entirely.
///   2. hookbox-mun.37: mask any SURVIVING header whose VALUE contains
///      `token` to `"<redacted>"`, mirroring
///      `routes::share::mask_token_in_value` — a reverse proxy's
///      `X-Forwarded-Host`, `X-Forwarded-Server`, `Forwarded`,
///      `X-Original-URI`, `X-Forwarded-Uri` or `X-Envoy-Original-Path` (all
///      added by default by Apache/Caddy/Traefik/ingress-nginx/Envoy) is not
///      in the fixed 3-name drop list above, so without this the endpoint
///      token re-enters an anonymous share viewer's response through the
///      echo BODY instead of the header maps — the same class as channel
///      A/B/C, closed generically instead of by enumerating one more name.
///
/// Both this function and `mask_token_in_value` share `helpers::contains_ci`,
/// which is case-insensitive (hookbox-mun.38: nginx's `$host` variable is
/// documented as lowercase, so a case-sensitive compare lets a case-folded
/// token through) — so the two copies of this check cannot silently drift
/// apart again.
///
/// Applied ONLY to the `headers` sub-object rebuilt for the persisted
/// `response_body` of a `default_mode = "echo"` row (`handle_mock`, below)
/// — the client's own echo body still carries the raw headers (AC-72, the
/// §2 non-goal), and every other row shape / column (including the stored
/// `request_headers` column, the owner Inspector and F5's CSV) is
/// untouched.
fn redact_echo_persisted_headers(
    headers: &BTreeMap<String, String>,
    token: &str,
) -> BTreeMap<String, String> {
    redact(headers)
        .into_iter()
        .filter(|(k, _)| !ECHO_PERSIST_HEADER_DROP.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| {
            if helpers::contains_ci(&v, token) {
                (k, "<redacted>".to_string())
            } else {
                (k, v)
            }
        })
        .collect()
}

/// The `default_mode = "echo"` payload shape, factored out so the client
/// build (unredacted `headers`) and the AC-S3 persisted rebuild (redacted
/// `headers`) can never drift apart.
fn echo_payload(
    method: &str,
    path: &str,
    query: &BTreeMap<String, String>,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Value {
    json!({ "method": method, "path": path, "query": query, "headers": headers, "body": body })
}

fn json_response(status: u16, body: &Value, content_type: &str) -> Response {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    let mut r = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .body(Body::from(bytes))
        .unwrap();
    insert_header(r.headers_mut(), "content-type", content_type);
    r
}

/// F7 (architecture.md §2.10.1): buffer a finished mock response's body so the
/// trace can persist the exact bytes the client will receive, returning an
/// equivalent response plus those bytes.
///
/// Allocation-free for the payload: every mock-plane body is built from an
/// owned buffer (a rendered template, `serde_json::to_vec`, the MITM reply,
/// the tunnel's decoded reply, or `Body::empty()`), so it is a SINGLE-FRAME
/// body — `to_bytes` hands back the same `Bytes` buffer and `Bytes::clone` is
/// a refcount bump, not a copy. Nothing here can block: the frame is already
/// resolved, so the `.await` never yields.
///
/// The `size_hint().exact()` guard is a forward-compatibility fuse: if a
/// streaming mock body is ever introduced, this returns the response
/// untouched and captures nothing rather than buffering an unbounded stream.
async fn capture_response_body(resp: Response) -> (Response, Bytes) {
    let (parts, body) = resp.into_parts();
    let Some(len) = body.size_hint().exact() else {
        tracing::warn!("mock response body has no exact size; response_body not captured");
        return (Response::from_parts(parts, body), Bytes::new());
    };
    if len == 0 {
        // 204 CORS preflight, empty 204 CRUD, chaos dropout — nothing to capture.
        return (Response::from_parts(parts, body), Bytes::new());
    }
    match to_bytes(body, len as usize).await {
        Ok(bytes) => (
            Response::from_parts(parts, Body::from(bytes.clone())),
            bytes,
        ),
        Err(e) => {
            // Unreachable for an in-memory body (`len` is exact, so the limit
            // cannot trip, and these bodies are infallible). If it ever fires
            // the body is already lost, so the only honest option is an
            // empty one.
            tracing::error!("failed to buffer mock response body: {e}");
            (Response::from_parts(parts, Body::empty()), Bytes::new())
        }
    }
}

/// Attach `X-HookBox-*` identifying headers + auto-CORS (P1).
fn identified(
    mut resp: Response,
    token: &str,
    served_by: &str,
    rule_id: Option<i64>,
    ep: &CompiledEndpoint,
    headers_lower: &BTreeMap<String, String>,
) -> Response {
    insert_header(resp.headers_mut(), "x-hookbox-endpoint", token);
    insert_header(resp.headers_mut(), "x-hookbox-served-by", served_by);
    if let Some(id) = rule_id {
        insert_header(resp.headers_mut(), "x-hookbox-rule-id", &id.to_string());
    }
    for (k, v) in cors::response_headers(headers_lower, ep.cors_enabled) {
        insert_header(resp.headers_mut(), &k, &v);
    }
    resp
}

fn unknown_or_gone(token: &str, gone: bool) -> Response {
    let (status, code, detail) = if gone {
        (
            410,
            "endpoint_gone",
            "This endpoint was deleted or expired.",
        )
    } else {
        (404, "unknown_endpoint", "No such endpoint.")
    };
    let mut r = json_response(
        status,
        &json!({"error": code, "detail": detail}),
        "application/json",
    );
    insert_header(r.headers_mut(), "x-hookbox-endpoint", token);
    r
}

#[allow(clippy::too_many_arguments)]
fn spawn_trace(
    state: &AppState,
    token: &str,
    method: &str,
    path: &str,
    status_code: i64,
    served_by: &str,
    matched_rule_id: Option<i64>,
    t0: Instant,
    applied_latency_ms: i64,
    req_headers: &BTreeMap<String, String>,
    query: &BTreeMap<String, String>,
    req_body: &str,
    resp: &Response,
    resp_body: &[u8], // F7 — captured bytes; empty ⇒ NULL (AC-69)
    trace: &[Step],
    state_snapshot: &BTreeMap<String, String>,
) {
    let duration_ms = t0.elapsed().as_millis() as i64;
    let overhead_ms = (duration_ms - applied_latency_ms).max(0);

    // Response headers as persisted (R11 seam; today an identity projection).
    let resp_headers = helpers::response_headers_for_trace(resp.headers());

    let cap = state.cfg.max_body_bytes;

    let trace_arr: Vec<Value> = trace
        .iter()
        .map(|s| json!({"step": s.step, "detail": s.detail}))
        .collect();
    let state_obj: serde_json::Map<String, Value> = state_snapshot
        .iter()
        .map(|(k, v)| (k.clone(), json!(v)))
        .collect();

    let rec = TraceRecord {
        token: token.to_string(),
        method: method.to_string(),
        path: path.to_string(),
        status_code,
        served_by: served_by.to_string(),
        matched_rule_id,
        duration_ms,
        overhead_ms,
        // Redact owner cap / cookies before persist AND before any feed payload.
        request_headers: serde_json::to_string(&redact(req_headers))
            .unwrap_or_else(|_| "{}".into()),
        query_params: serde_json::to_string(query).unwrap_or_else(|_| "{}".into()),
        // F7: both body columns go through the identical projection helper
        // (AC-70(d)) — NULL <=> zero-length, never an empty string (AC-69).
        request_body: helpers::body_for_trace(req_body.as_bytes(), cap),
        response_headers: serde_json::to_string(&resp_headers).unwrap_or_else(|_| "{}".into()),
        response_body: helpers::body_for_trace(resp_body, cap),
        trace_json: serde_json::to_string(&trace_arr).unwrap_or_else(|_| "[]".into()),
        state_snapshot: serde_json::to_string(&state_obj).unwrap_or_else(|_| "{}".into()),
    };

    let pool = state.pool.clone();
    let hub = state.feed_hub.clone();
    let trace_cap = state.cfg.trace_cap;
    let token_s = token.to_string();
    let summary_base = json!({
        "token": token, "method": method, "path": path, "status_code": status_code,
        "served_by": served_by, "matched_rule_id": matched_rule_id,
        "duration_ms": duration_ms, "overhead_ms": overhead_ms,
        "timestamp": now_iso(),
    });

    tokio::spawn(async move {
        let row_id = db::insert_trace(&pool, &rec, trace_cap).await.unwrap_or(0);
        let mut summary = summary_base;
        summary["id"] = json!(row_id);
        hub.publish(&token_s, FeedEvent::new("new_request", summary));
    });
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// Read a value out of a JSON body via jsonpath-lite (used by tests/helpers).
#[allow(dead_code)]
fn jp(body: &str, path: &str) -> String {
    crate::helpers::jsonpath_lite(body, path).unwrap_or_default()
}

/// Stringify a JSON value (re-export for symmetry with the Python helper).
#[allow(dead_code)]
fn vstr(v: &Value) -> String {
    value_to_string(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // F7 AC-73(d3): the capture helper on a 5 MB body (the MITM_MAX_BODY_BYTES
    // worst case) must be zero-copy, and the rebuilt response's bytes must be
    // byte-equal to the input.
    //
    // Zero-copy is proven by timing, not by a fixed millisecond budget — a
    // fixed `< 1ms` bound (hookbox-mun.32) has under 1x margin against warm
    // ~0.3ms runs and flakes ~6% of the time on a cold/loaded/shared runner,
    // because it is racing wall-clock scheduling noise, not the code path.
    //
    // Instead we compare `capture_response_body` against a same-process,
    // same-buffer, same-run explicit `Vec::clone` (a real memcpy) of the
    // identical 5 MB payload. That comparison is scale-free: a slow or
    // loaded runner slows both timings down together, so the RATIO is stable
    // even when the absolute numbers are not. A full-copy implementation of
    // `capture_response_body` would cost about the same as the memcpy
    // control, so requiring capture to be well under it (< 25%) still fails
    // a regression that reintroduces a copy, while a min-of-N sample (a
    // warm-up run discarded, then the fastest of several timed runs kept)
    // removes first-invocation/cold-cache noise from BOTH sides.
    #[tokio::test]
    async fn capture_response_body_5mb_is_faster_than_a_full_copy_and_byte_equal() {
        const ITERATIONS: usize = 20;
        let payload = vec![b'x'; 5_000_000];

        // Warm-up: pay for allocator/CPU-frequency-scaling/page-fault costs
        // once, outside any measured sample, on both code paths.
        {
            let resp = Response::builder()
                .status(200)
                .body(Body::from(payload.clone()))
                .unwrap();
            let _ = capture_response_body(resp).await;
            let warm_control: Vec<u8> = payload.clone();
            std::hint::black_box(&warm_control);
        }

        let mut capture_min = Duration::MAX;
        let mut captured_bytes = Bytes::new();
        let mut rebuilt_bytes = Bytes::new();
        for _ in 0..ITERATIONS {
            let resp = Response::builder()
                .status(200)
                .body(Body::from(payload.clone()))
                .unwrap();
            let started = Instant::now();
            let (rebuilt, bytes) = capture_response_body(resp).await;
            let elapsed = started.elapsed();
            if elapsed < capture_min {
                capture_min = elapsed;
                captured_bytes = bytes;
                rebuilt_bytes = to_bytes(rebuilt.into_body(), usize::MAX).await.unwrap();
            }
        }

        let mut copy_min = Duration::MAX;
        for _ in 0..ITERATIONS {
            let started = Instant::now();
            let control: Vec<u8> = payload.clone();
            let elapsed = started.elapsed();
            std::hint::black_box(&control);
            copy_min = copy_min.min(elapsed);
        }

        assert_eq!(captured_bytes.len(), payload.len());
        assert_eq!(captured_bytes.as_ref(), payload.as_slice());
        // The rebuilt response's own body is byte-equal too.
        assert_eq!(rebuilt_bytes.as_ref(), payload.as_slice());

        // A full-copy implementation would cost roughly `copy_min`; the
        // zero-copy path must be clearly (>= 4x) faster than that, on the
        // same machine, same run, same buffer — so this cannot pass a
        // full-copy regression and cannot flake from an absolute-time budget.
        assert!(
            capture_min.saturating_mul(4) < copy_min,
            "capture_response_body best-of-{ITERATIONS} ({capture_min:?}) is not clearly \
             faster than a best-of-{ITERATIONS} explicit 5MB Vec::clone ({copy_min:?}); \
             expected < 25% of the copy control, which suggests a full copy crept back in"
        );
    }

    #[tokio::test]
    async fn capture_response_body_empty_is_none_and_untouched() {
        let resp = Response::builder().status(204).body(Body::empty()).unwrap();
        let (rebuilt, bytes) = capture_response_body(resp).await;
        assert!(bytes.is_empty());
        assert_eq!(rebuilt.status(), StatusCode::NO_CONTENT);
    }

    // hookbox-mun.37 + hookbox-mun.38: a proxy header whose NAME is not in
    // `ECHO_PERSIST_HEADER_DROP` but whose VALUE carries the endpoint token
    // (in ANY letter case) must be masked, not just dropped-by-name headers.
    #[test]
    fn redact_echo_persisted_headers_masks_any_value_containing_the_token() {
        let token = "oPp8tASu3i";
        let mut headers = BTreeMap::new();
        headers.insert("host".to_string(), format!("{token}.mock.local"));
        headers.insert(
            "x-forwarded-host".to_string(),
            format!("{token}.mock.local"),
        );
        headers.insert(
            "x-forwarded-server".to_string(),
            format!("{token}.mock.local"),
        );
        headers.insert(
            "forwarded".to_string(),
            format!("for=203.0.113.9;host={token}.mock.local;proto=https"),
        );
        headers.insert(
            "x-envoy-original-path".to_string(),
            format!("/e/{token}/ingress"),
        );
        headers.insert("accept".to_string(), "application/json".to_string());
        let out = redact_echo_persisted_headers(&headers, token);

        // Structural name-drop headers are absent entirely.
        assert!(!out.contains_key("host"));
        // Every OTHER header whose value carries the token is masked, not
        // just the 3-name list — this is the generic fix, not a longer
        // enumeration.
        assert_eq!(out["x-forwarded-host"], "<redacted>");
        assert_eq!(out["x-forwarded-server"], "<redacted>");
        assert_eq!(out["forwarded"], "<redacted>");
        assert_eq!(out["x-envoy-original-path"], "<redacted>");
        // A header carrying no token value is untouched.
        assert_eq!(out["accept"], "application/json");
    }

    #[test]
    fn redact_echo_persisted_headers_masks_a_case_folded_token() {
        // hookbox-mun.38: nginx's `$host` variable is documented as
        // lowercase, so the value arrives case-folded relative to the
        // mixed-case token.
        let token = "ixaU3viom4";
        let mut headers = BTreeMap::new();
        headers.insert(
            "x-forwarded-host".to_string(),
            "ixau3viom4.mock.local".to_string(),
        );
        let out = redact_echo_persisted_headers(&headers, token);
        assert_eq!(out["x-forwarded-host"], "<redacted>");
    }

    #[test]
    fn echo_payload_shape_is_stable() {
        let mut query = BTreeMap::new();
        query.insert("q".to_string(), "1".to_string());
        let mut headers = BTreeMap::new();
        headers.insert("authorization".to_string(), "Bearer secret".to_string());
        let v = echo_payload("GET", "/p", &query, &headers, "body-text");
        assert_eq!(v["method"], json!("GET"));
        assert_eq!(v["path"], json!("/p"));
        assert_eq!(v["query"]["q"], json!("1"));
        assert_eq!(v["headers"]["authorization"], json!("Bearer secret"));
        assert_eq!(v["body"], json!("body-text"));
    }
}
