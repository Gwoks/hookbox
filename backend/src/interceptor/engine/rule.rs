//! A matched rule (§5.5 step 3): its state writes, then its response
//! template, rendered with one shared context.

use axum::body::Body;
use axum::response::Response;
use serde_json::json;

use super::exchange::{Exchange, RuleOverrides};
use super::respond::{insert_header, with_status};
use crate::feed::FeedEvent;
use crate::interceptor::matcher::{CompiledRule, MatchResult};
use crate::interceptor::templating::{self, TemplateContext};
use crate::models::StateWrite;
use crate::repo;

/// The content type a rule with a blank `content_type` responds with.
const DEFAULT_CONTENT_TYPE: &str = "application/json";

pub(super) async fn respond(ex: &mut Exchange<'_>, matched: MatchResult<'_>) -> Response {
    let rule = matched.rule;
    ex.rule = RuleOverrides::of(rule);
    ex.step(
        "match",
        format!("rule {} (priority {})", rule.id, rule.priority),
    );
    let r = &ex.req;
    let mut ctx = TemplateContext {
        method: r.method.clone(),
        path: r.path.clone(),
        query: r.query.clone(),
        headers: r.headers.clone(),
        path_params: matched.path_params,
        body: r.body_text.clone(),
        state: ex.endpoint_state.clone(),
    };
    // 3a. State writes BEFORE the body, so {{state.k}} sees the value this
    // very request wrote (AC-19).
    for write in &rule.state_writes {
        apply_state_write(ex, &mut ctx, write).await;
    }
    render(ex, rule, &ctx)
}

async fn apply_state_write(ex: &mut Exchange<'_>, ctx: &mut TemplateContext, w: &StateWrite) {
    let state = ex.state;
    let cfg = &state.cfg;
    let value = templating::render(&w.value, ctx, cfg.template_max_size, cfg.template_max_tags);
    let token = &ex.req.token;
    let ttl = cfg.state_ttl_seconds;
    match repo::state::write_state(&state.pool, token, &w.key, &value, ttl).await {
        Ok(true) => {
            let event = json!({"token": token, "key": w.key, "value": value});
            state
                .feed_hub
                .publish(token, FeedEvent::new("state_changed", event));
            ctx.state.insert(w.key.clone(), value.clone());
            ex.endpoint_state.insert(w.key.clone(), value.clone());
            ex.step("state_write", format!("{}={}", w.key, value));
        }
        Ok(false) => ex.step("state_write", format!("{}: unsafe key (skipped)", w.key)),
        Err(_) => ex.step("state_write", format!("{}: store error (skipped)", w.key)),
    }
}

fn render(ex: &mut Exchange<'_>, rule: &CompiledRule, ctx: &TemplateContext) -> Response {
    let cfg = &ex.state.cfg;
    let spec = &rule.response;
    let body = templating::render(
        &spec.body_template,
        ctx,
        cfg.template_max_size,
        cfg.template_max_tags,
    );
    ex.step("template", format!("rendered {} bytes", body.len()));
    let content_type = if spec.content_type.is_empty() {
        DEFAULT_CONTENT_TYPE
    } else {
        spec.content_type.as_str()
    };
    let mut resp = with_status(spec.status_code.clamp(100, 599) as u16, Body::from(body));
    insert_header(resp.headers_mut(), "content-type", content_type);
    for (k, v) in &spec.headers {
        insert_header(resp.headers_mut(), k, v);
    }
    resp
}
