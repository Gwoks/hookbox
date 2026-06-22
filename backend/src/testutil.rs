//! Test-only support.
//!
//! `std::env::{set_var,remove_var}` mutate process-global state shared by every
//! test in the binary, which `cargo test` runs in parallel by default. Any test
//! that touches the environment (e.g. `MOCK_DOMAIN`, `TRACE_CAP`) must hold the
//! `env_lock()` guard for the whole set -> `Config::from_env()` -> assert window
//! so reads see only its own writes. This is what keeps the AC-55 `cargo test`
//! gate deterministic without forcing `--test-threads=1`.

use std::sync::{Mutex, MutexGuard, OnceLock};

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquire the process-global environment lock. Hold the returned guard until
/// every env read for the test has completed. The lock is intentionally
/// poison-tolerant: a panicking test should not wedge the rest of the suite.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
