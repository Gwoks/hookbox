//! The single route table. `api_routes()` holds every `/api/**` management
//! route (owner-authenticated, plus the session and public share resolvers);
//! `router()` adds the P3 health / live-feed / tunnel / SPA routes on top.
//! `router::build_app` only layers plane dispatch + panic catching around it.

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

mod endpoints;
pub mod feed;
pub mod health;
mod requests;
mod rules;
mod session;
pub mod share;
pub mod spa;
mod state;
pub mod tunnel_ws;

pub use health::healthz;

/// Every `/api/**` route. `POST /api/session` and the two public share
/// routes are unauthenticated; all others use the `OwnerId` extractor (401
/// on a missing/unknown Bearer). The F4 share routes are merged into this one
/// router — never layered separately — so no `TraceLayer` or other logging
/// middleware can end up wrapping only them and start logging a share code
/// (AC-S6).
pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/api/session", post(session::create_session))
        .route(
            "/api/endpoints",
            get(endpoints::list_endpoints).post(endpoints::create_endpoint),
        )
        .route(
            "/api/endpoints/:token",
            get(endpoints::get_endpoint)
                .patch(endpoints::patch_endpoint)
                .delete(endpoints::delete_endpoint),
        )
        .route(
            "/api/endpoints/:token/rules",
            get(rules::list_rules).post(rules::create_rule),
        )
        .route(
            "/api/endpoints/:token/rules/:id",
            get(rules::get_rule)
                .patch(rules::patch_rule)
                .delete(rules::delete_rule),
        )
        .route(
            "/api/endpoints/:token/requests",
            get(requests::list_requests).delete(requests::clear_requests),
        )
        .route("/api/requests/:id", get(requests::get_request))
        .route(
            "/api/endpoints/:token/state",
            get(state::get_state).delete(state::clear_state),
        )
        .route(
            "/api/endpoints/:token/collections/:name",
            get(state::peek_collection).delete(state::clear_collection),
        )
        .merge(share::share_router())
}

/// The full inner router: `/api/**`, `/healthz`, the owner-gated live feed
/// (WS + SSE), the tunnel control channel, and the SPA fallback (R6 — the
/// fallback never shadows P1/P2, which plane dispatch resolves first).
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .merge(api_routes())
        .route("/healthz", get(healthz))
        .route("/ws/:token", get(feed::ws_handler))
        .route("/sse/:token", get(feed::sse_handler))
        .route("/ws/tunnel/:slug", get(tunnel_ws::tunnel_ws_handler))
        .fallback(spa::serve_spa)
}
