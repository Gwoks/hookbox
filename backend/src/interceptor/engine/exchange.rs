//! The per-request context threaded through every engine stage, and its one
//! exit: `finish` (identify → capture the body → trace off the path).

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::echo;
use super::respond::{capture_response_body, identified, insert_header};
use super::trace::Step;
use crate::interceptor::matcher::{self, CompiledRule, MatchResult};
use crate::models::{ChaosMode, DefaultMode, ServedBy};
use crate::repo;
use crate::rule_cache::CompiledEndpoint;
use crate::state::AppState;

/// "Client closed request": the status a chaos dropout answers with.
const DROPOUT_STATUS: u16 = 499;

/// The inbound mock request, normalized once.
pub(super) struct MockRequest {
    pub(super) token: String,
    /// Upper-cased.
    pub(super) method: String,
    pub(super) path: String,
    /// Lower-cased names.
    pub(super) headers: BTreeMap<String, String>,
    pub(super) query: BTreeMap<String, String>,
    pub(super) body: Vec<u8>,
    /// `body` decoded lossily (what rules / templates / traces see).
    pub(super) body_text: String,
    pub(super) started: Instant,
}

impl MockRequest {
    pub(super) fn new(
        token: &str,
        method: &str,
        path: &str,
        headers: BTreeMap<String, String>,
        query: BTreeMap<String, String>,
        body: Vec<u8>,
    ) -> Self {
        MockRequest {
            token: token.to_string(),
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            headers,
            query,
            body_text: String::from_utf8_lossy(&body).into_owned(),
            body,
            started: Instant::now(),
        }
    }
}

/// What a matched rule overrides of the endpoint's conditions.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct RuleOverrides {
    pub(super) id: Option<i64>,
    pub(super) latency_ms: Option<i64>,
    pub(super) rate_limit_per_min: Option<i64>,
    pub(super) chaos_mode: Option<ChaosMode>,
}

impl RuleOverrides {
    pub(super) fn of(rule: &CompiledRule) -> Self {
        RuleOverrides {
            id: Some(rule.id),
            latency_ms: rule.latency_ms,
            rate_limit_per_min: rule.rate_limit_per_min,
            chaos_mode: rule.chaos_mode,
        }
    }
}

/// One mock exchange in flight.
pub(super) struct Exchange<'s> {
    pub(super) state: &'s AppState,
    pub(super) req: MockRequest,
    pub(super) ep: Arc<CompiledEndpoint>,
    pub(super) trace: Vec<Step>,
    /// The state the rules saw (plus this request's writes) — the snapshot.
    pub(super) endpoint_state: BTreeMap<String, String>,
    pub(super) rule: RuleOverrides,
    pub(super) applied_latency_ms: i64,
}

impl<'s> Exchange<'s> {
    pub(super) fn new(state: &'s AppState, req: MockRequest, ep: Arc<CompiledEndpoint>) -> Self {
        Exchange {
            state,
            req,
            ep,
            trace: Vec::new(),
            endpoint_state: BTreeMap::new(),
            rule: RuleOverrides::default(),
            applied_latency_ms: 0,
        }
    }

    /// Record a trace step.
    pub(super) fn step(&mut self, name: &str, detail: impl Into<String>) {
        self.trace.push(Step::new(name, detail));
    }

    /// Read the endpoint state ONLY when some rule gates on it. A store
    /// error reads as empty, so state-gated rules fail closed.
    pub(super) async fn read_state_if_gated(&mut self) {
        if !self.ep.any_rule_gates_on_state {
            return;
        }
        match repo::state::read_state(&self.state.pool, &self.req.token).await {
            Ok(s) => {
                self.step("state_read", format!("{} key(s)", s.len()));
                self.endpoint_state = s;
            }
            Err(_) => self.step("state_read", "store error -> state empty (fail-closed)"),
        }
    }

    /// The first enabled rule of `ep` (this exchange's endpoint) that fully
    /// matches the request.
    pub(super) fn select_rule<'e>(&self, ep: &'e CompiledEndpoint) -> Option<MatchResult<'e>> {
        let r = &self.req;
        matcher::select(
            &ep.rules,
            &r.method,
            &r.path,
            &r.headers,
            &r.query,
            &r.body_text,
            &self.endpoint_state,
        )
    }

    /// The single exit of every served response: attach the identifying +
    /// CORS headers, buffer the exact bytes the client gets (F7), and spawn
    /// the trace. An echo default persists its redacted rebuild (AC-S3).
    pub(super) async fn finish(self, resp: Response, served_by: ServedBy) -> Response {
        let status = i64::from(resp.status().as_u16());
        let r = &self.req;
        let resp = identified(
            resp,
            &r.token,
            served_by,
            self.rule.id,
            &self.ep,
            &r.headers,
        );
        let (resp, captured) = capture_response_body(resp).await;
        let persisted = self.persisted_body(served_by, &captured);
        self.spawn_trace(status, served_by, resp.headers(), &persisted);
        resp
    }

    /// Chaos dropout (D5 / R-DROPOUT): the client gets no real response — an
    /// unidentified 499 + `Connection: close` — and the trace row keeps
    /// `status_code = 0`, empty response headers and no body.
    pub(super) fn finish_dropout(self) -> Response {
        self.spawn_trace(0, ServedBy::Chaos, &HeaderMap::new(), &[]);
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() =
            StatusCode::from_u16(DROPOUT_STATUS).unwrap_or(StatusCode::BAD_REQUEST);
        insert_header(resp.headers_mut(), "connection", "close");
        resp
    }

    /// AC-S3: a `default_mode = "echo"` row persists the echo rebuilt from
    /// REDACTED headers (`echo::redact_echo_persisted_headers`), while the
    /// client's own echo body keeps the raw ones; everything else persists
    /// the captured bytes as sent.
    fn persisted_body<'b>(&self, served_by: ServedBy, captured: &'b [u8]) -> Cow<'b, [u8]> {
        if served_by == ServedBy::Default && self.ep.default_mode == DefaultMode::Echo {
            Cow::Owned(echo::persisted_echo(&self.req))
        } else {
            Cow::Borrowed(captured)
        }
    }
}
