//! Integration tests over the in-process app (§5 parity guard, AC-55).
//!
//! Mirrors the reference `../shortener-link/backend/tests/` shape: build the real
//! `router::build_app(state)` over an in-memory SQLite DB and drive it with
//! `tower::ServiceExt::oneshot`. One test binary; the cases live in
//! `tests/api/*.rs` by area, and the shared harness (`TestApp`, a
//! from_source `Config` that never touches the process env, and the API
//! helpers) in `tests/common/mod.rs`.

mod common;

mod api {
    mod capture;
    mod feed_auth;
    mod management;
    mod mock_plane;
    mod session;
    mod share_leaks;
    mod share_owner;
    mod share_public;
}
