//! Route module tree + router builders for the management (P2) and health (P3)
//! surfaces. The interceptor (P1), feed/SPA (P3) routers are added by their own
//! tasks and merged in `router::build_app`.

pub mod api;
pub mod feed;
pub mod health;
pub mod share;
pub mod spa;
pub mod tunnel_ws;

pub use api::api_router;
pub use health::healthz;
pub use share::share_router;
