//! The P1 mock-plane interceptor pipeline (§5.5).
//!
//! `engine::handle_mock` owns the frozen resolution order
//! (OPTIONS → rule → Auto-CRUD → tunnel → MITM → default) and the conditions
//! wrap (rate-limit → chaos → latency). The sibling modules each own one stage:
//!   * `matcher`     — compile_path + select (priority,id) + predicate checks
//!   * `templating`  — sandboxed single-pass response/state-write rendering
//!   * `conditions`  — clamps + latency + chaos roll
//!   * `cors`        — auto-CORS header sets (P1 only)
//!   * `crud`        — Auto-CRUD over crud_collections (in-txn CAS)
//!   * `proxy`/`ssrf`— MITM forward + SSRF guard
//!
//! Modules land across hookbox-sks.15–.22; each is added as its task closes.

pub mod conditions;
pub mod cors;
pub mod crud;
pub mod engine;
pub mod matcher;
pub mod proxy;
pub mod ssrf;
pub mod templating;
