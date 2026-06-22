//! HookBox backend — single Axum binary over WAL SQLite (Rust re-platform).
//! Library crate so `main.rs` and the `tunnel` / `seed` bins share modules.
//!
//! Scaffold (hookbox-sks.11): config, db, error, state + the in-process
//! component placeholders (rule_cache, limiter, feed, tunnels) that later tasks
//! flesh out behind the same `AppState` fields. Routing/interceptor modules
//! (planes, router, routes/*, interceptor/*) are added by tasks sks.12–sks.24.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod helpers;
pub mod ids;
pub mod interceptor;
pub mod models;
pub mod planes;
pub mod router;
pub mod routes;
pub mod state;

// SQLite-backed stores (replace two Redis responsibilities).
pub mod crud_store;
pub mod state_store;

// In-process components backing AppState.
pub mod feed;
pub mod limiter;
pub mod rule_cache;
pub mod tunnels;

// Background retention sweep.
pub mod tasks;
pub mod seed;

// Test-only helpers (e.g. a process-global lock to serialize env-var mutation).
#[cfg(test)]
pub mod testutil;
