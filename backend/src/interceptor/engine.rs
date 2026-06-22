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

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use serde_json::{json, Value};

use crate::db::{self, TraceRecord};
use crate::feed::FeedEvent;
use crate::helpers::{redact, value_to_string};
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
        let payload = json!({
            "method": method, "path": mock_path, "query": query,
            "headers": headers_lower, "body": body_text,
        });
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

fn json_response(status: u16, body: &Value, content_type: &str) -> Response {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    let mut r = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .body(Body::from(bytes))
        .unwrap();
    insert_header(r.headers_mut(), "content-type", content_type);
    r
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
    trace: &[Step],
    state_snapshot: &BTreeMap<String, String>,
) {
    let duration_ms = t0.elapsed().as_millis() as i64;
    let overhead_ms = (duration_ms - applied_latency_ms).max(0);

    // Capture response headers (the §5.4 trace records them) — redacted set.
    let resp_headers: BTreeMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|vs| (k.as_str().to_string(), vs.to_string()))
        })
        .collect();

    let cap = state.cfg.max_body_bytes;
    let truncate = |s: &str| -> String {
        if s.len() > cap {
            s[..cap].to_string()
        } else {
            s.to_string()
        }
    };

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
        request_body: if req_body.is_empty() {
            None
        } else {
            Some(truncate(req_body))
        },
        response_headers: serde_json::to_string(&resp_headers).unwrap_or_else(|_| "{}".into()),
        response_body: None, // body already consumed into the Response; not re-read (off-path)
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
