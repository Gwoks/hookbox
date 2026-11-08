//! Anonymous owner sessions: the owner key rides an HttpOnly,
//! `SameSite=Strict` cookie for the SPA (and `Authorization: Bearer` for the
//! CLI / scripts). This module owns the cookie wire format, the CSRF guard
//! every cookie-authenticated request passes, and the mock-plane hardening
//! that keeps the cookie from ever leaking through a mock endpoint.

pub mod cookie;
pub mod csrf;

pub use cookie::{
    clear_cookie_value, cookie_name, is_session_cookie, read_session_cookie, set_cookie_value,
    strip_session_cookie,
};
pub use csrf::{csrf_guard, origin_allowed};

/// Cookie name when `SESSION_COOKIE_SECURE` (https deployments).
pub const COOKIE_NAME_SECURE: &str = "__Host-hookbox_key";

/// Cookie name for plain-http local development.
pub const COOKIE_NAME_PLAIN: &str = "hookbox_key";

/// The custom request header (value `1`) the SPA sends on every owner call.
pub const CSRF_HEADER: &str = "x-hookbox";

/// Cookie lifetime: 400 days, the longest Max-Age browsers honor.
pub const COOKIE_MAX_AGE: i64 = 34_560_000;
