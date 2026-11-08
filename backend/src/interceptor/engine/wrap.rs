//! The conditions wrapped around the resolved response (§5.5 step 5), in the
//! frozen order: rate limit (429) → chaos (5xx / drop) → latency (sleep).

use axum::response::{IntoResponse, Response};

use super::exchange::Exchange;
use crate::error::ApiError;
use crate::interceptor::conditions::{self, Chaos};
use crate::interceptor::mock_error::MockError;
use crate::limiter::Limiter;
use crate::models::ServedBy;

/// The mock rate-limit window.
const RATE_WINDOW_SECS: i64 = 60;

/// What the conditions decided.
pub(super) enum Wrapped {
    /// Serve the resolved response (after any simulated latency).
    Continue,
    /// Serve this response instead (429 / chaos 5xx).
    Replace(Response, ServedBy),
    /// Chaos dropout: no real response at all.
    Dropout,
}

pub(super) async fn apply(ex: &mut Exchange<'_>) -> Wrapped {
    if let Some(limited) = rate_limit(ex) {
        return Wrapped::Replace(limited, ServedBy::RateLimit);
    }
    if let Some(chaos) = chaos(ex) {
        return chaos;
    }
    latency(ex).await;
    Wrapped::Continue
}

/// A matched rule's own limit gets its own bucket; otherwise the endpoint's.
fn rate_limit(ex: &mut Exchange<'_>) -> Option<Response> {
    let state = ex.state;
    let limit = ex
        .rule
        .rate_limit_per_min
        .unwrap_or(ex.ep.rate_limit_per_min);
    let limit = conditions::clamp_rate(Some(limit), &state.cfg);
    if limit <= 0 {
        return None;
    }
    let rule_bucket = ex.rule.id.filter(|_| ex.rule.rate_limit_per_min.is_some());
    let key = Limiter::key(&ex.req.token, rule_bucket);
    let verdict = state.limiter.check(&key, limit, RATE_WINDOW_SECS);
    if verdict.allowed {
        return None;
    }
    ex.step("rate_limit", format!("429 (limit {limit}/min)"));
    let error = ApiError::rate_limited("Rate limit exceeded.", verdict.retry_after)
        .with_header("x-ratelimit-limit", verdict.limit.to_string())
        .with_header("x-ratelimit-remaining", verdict.remaining.to_string());
    Some(error.into_response())
}

fn chaos(ex: &mut Exchange<'_>) -> Option<Wrapped> {
    let pct = conditions::clamp_chaos(Some(ex.ep.chaos_pct), &ex.state.cfg);
    let mode = ex.rule.chaos_mode.unwrap_or(ex.ep.chaos_mode);
    match conditions::roll_chaos(pct, mode) {
        Chaos::Drop => {
            ex.step("chaos", "dropout (connection closed)");
            Some(Wrapped::Dropout)
        }
        Chaos::Status(code) => {
            ex.step("chaos", code.to_string());
            let resp = MockError::Chaos(code as u16).into_response();
            Some(Wrapped::Replace(resp, ServedBy::Chaos))
        }
        Chaos::None => None,
    }
}

async fn latency(ex: &mut Exchange<'_>) {
    let ms = ex.rule.latency_ms.unwrap_or(ex.ep.latency_ms);
    if ms > 0 {
        ex.applied_latency_ms = conditions::apply_latency(ms, &ex.state.cfg).await;
        ex.step("latency", format!("{}ms", ex.applied_latency_ms));
    }
}
