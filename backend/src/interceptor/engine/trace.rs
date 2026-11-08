//! The trace write + feed publish, spawned OFF the response path (never
//! awaited) so a slow / failed trace never delays or fails the mock response
//! (AC-46/47/57, AC-73(a)).

use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::{json, Value};

use super::exchange::Exchange;
use crate::feed::FeedEvent;
use crate::models::ServedBy;
use crate::net::headers::{redact, response_headers_for_trace};
use crate::repo::requests::{insert_trace, TraceRecord};
use crate::util::text::body_for_trace;
use crate::util::time::now_iso;

/// One step of the per-request trace (`trace_json`).
#[derive(Debug, Clone)]
pub(super) struct Step {
    step: String,
    detail: String,
}

impl Step {
    pub(super) fn new(step: &str, detail: impl Into<String>) -> Self {
        Step {
            step: step.to_string(),
            detail: detail.into(),
        }
    }

    fn to_json(&self) -> Value {
        json!({"step": self.step, "detail": self.detail})
    }
}

/// Serialize a JSON trace column, falling back to its empty shape.
fn to_json<T: Serialize + ?Sized>(value: &T, empty: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| empty.to_string())
}

/// Wall time of the exchange, and the part of it that was not simulated
/// latency (AC-38).
#[derive(Clone, Copy)]
struct Timing {
    duration_ms: i64,
    overhead_ms: i64,
}

impl Exchange<'_> {
    /// Persist the exchange and publish its `new_request` feed event, off the
    /// response path. `resp_body` is the captured response (F7); empty ⇒ NULL
    /// (AC-69).
    pub(super) fn spawn_trace(
        &self,
        status: i64,
        served_by: ServedBy,
        resp_headers: &HeaderMap,
        resp_body: &[u8],
    ) {
        let duration_ms = self.req.started.elapsed().as_millis() as i64;
        let timing = Timing {
            duration_ms,
            overhead_ms: (duration_ms - self.applied_latency_ms).max(0),
        };
        let record = self.record(status, served_by, timing, resp_headers, resp_body);
        let summary = self.feed_summary(status, served_by, timing);
        let pool = self.state.pool.clone();
        let hub = self.state.feed_hub.clone();
        let trace_cap = self.state.cfg.trace_cap;
        let token = self.req.token.clone();
        tokio::spawn(async move {
            let row_id = match insert_trace(&pool, &record, trace_cap).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("trace write failed (feed still notified): {e}");
                    0
                }
            };
            let mut summary = summary;
            summary["id"] = json!(row_id);
            hub.publish(&token, FeedEvent::new("new_request", summary));
        });
    }

    fn record(
        &self,
        status: i64,
        served_by: ServedBy,
        timing: Timing,
        resp_headers: &HeaderMap,
        resp_body: &[u8],
    ) -> TraceRecord {
        let r = &self.req;
        let cap = self.state.cfg.max_body_bytes;
        let steps: Vec<Value> = self.trace.iter().map(Step::to_json).collect();
        TraceRecord {
            token: r.token.clone(),
            method: r.method.clone(),
            path: r.path.clone(),
            status_code: status,
            served_by: served_by.as_str().to_string(),
            matched_rule_id: self.rule.id,
            duration_ms: timing.duration_ms,
            overhead_ms: timing.overhead_ms,
            // Redact owner cap / cookies before persist AND before any feed payload.
            request_headers: to_json(&redact(&r.headers), "{}"),
            query_params: to_json(&r.query, "{}"),
            // F7: both body columns go through the identical projection helper
            // (AC-70(d)) — NULL <=> zero-length, never an empty string (AC-69).
            request_body: body_for_trace(r.body_text.as_bytes(), cap),
            response_headers: to_json(&response_headers_for_trace(resp_headers), "{}"),
            response_body: body_for_trace(resp_body, cap),
            trace_json: to_json(&steps, "[]"),
            state_snapshot: to_json(&self.endpoint_state, "{}"),
        }
    }

    fn feed_summary(&self, status: i64, served_by: ServedBy, timing: Timing) -> Value {
        let r = &self.req;
        json!({
            "token": r.token, "method": r.method, "path": r.path, "status_code": status,
            "served_by": served_by.as_str(), "matched_rule_id": self.rule.id,
            "duration_ms": timing.duration_ms, "overhead_ms": timing.overhead_ms,
            "timestamp": now_iso(),
        })
    }
}
