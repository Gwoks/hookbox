//! Simulated network conditions — clamps · latency · chaos (§5.5, AC-37/38/40).
//!
//! The engine applies these *around* the resolved body in the frozen order
//! (rate-limit → chaos → latency). Rate-limit lives in `limiter.rs`; this module
//! owns the clamps, the cooperative latency sleep, and the chaos roll.
//!
//! Determinism (R6): the chaos roll is split into a pure `chaos_outcome` (testable
//! with an injected `roll`/`pick`) and a thin `roll_chaos` wrapper using the
//! thread RNG, mirroring the Python `set_rng` seam.

use rand::Rng;

use crate::config::Config;

/// Chaos failure statuses (§5.5 / OQ-2 default = random 5xx).
pub const CHAOS_STATUSES: [i64; 3] = [502, 503, 504];

/// The outcome of a chaos roll.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Chaos {
    /// Return a `5xx` JSON `{error:"chaos"}` body.
    Status(i64),
    /// Opt-in connection drop (`chaos_mode == "dropout"`), bounded by timeout.
    Drop,
    /// Chaos did not fire.
    None,
}

/// Clamp latency to `[0, latency_max_ms]`. `None` → 0.
pub fn clamp_latency(ms: Option<i64>, cfg: &Config) -> i64 {
    ms.unwrap_or(0).clamp(0, cfg.latency_max_ms)
}

/// Clamp a rate limit to `[0, rate_limit_max_per_min]`. `None`/<=0 → 0 (unlimited).
pub fn clamp_rate(limit: Option<i64>, cfg: &Config) -> i64 {
    match limit {
        Some(n) if n > 0 => n.min(cfg.rate_limit_max_per_min),
        _ => 0,
    }
}

/// Clamp a chaos percentage to `[0, chaos_max_pct]`. `None` → 0.
pub fn clamp_chaos(pct: Option<i64>, cfg: &Config) -> i64 {
    pct.unwrap_or(0).clamp(0, cfg.chaos_max_pct)
}

/// Apply the (clamped) simulated latency cooperatively and return the ms applied
/// so the caller can record `overhead_ms = duration - applied_latency` (AC-38).
pub async fn apply_latency(ms: i64, cfg: &Config) -> i64 {
    let applied = clamp_latency(Some(ms), cfg);
    if applied > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(applied as u64)).await;
    }
    applied
}

/// Pure chaos decision (testable). `pct` is the clamped percentage; `roll` is a
/// value in `1..=100`; `pick` selects a status index `0..3` when in error mode.
/// `chaos_pct=0` never fires; `chaos_pct=100` always fires (roll<=100).
pub fn chaos_outcome(pct: i64, dropout: bool, roll: u8, pick: usize) -> Chaos {
    if pct <= 0 {
        return Chaos::None;
    }
    if (roll as i64) > pct {
        return Chaos::None;
    }
    if dropout {
        Chaos::Drop
    } else {
        Chaos::Status(CHAOS_STATUSES[pick % CHAOS_STATUSES.len()])
    }
}

/// Roll chaos for an endpoint using the thread RNG. `chaos_pct` already clamped
/// by the caller (or clamp here defensively). `chaos_mode == "dropout"` opts into
/// the connection-drop variant.
pub fn roll_chaos(chaos_pct: i64, chaos_mode: &str) -> Chaos {
    if chaos_pct <= 0 {
        return Chaos::None;
    }
    let mut rng = rand::thread_rng();
    let roll: u8 = rng.gen_range(1..=100);
    let pick: usize = rng.gen_range(0..CHAOS_STATUSES.len());
    let dropout = chaos_mode.eq_ignore_ascii_case("dropout");
    chaos_outcome(chaos_pct, dropout, roll, pick)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        let _guard = crate::testutil::env_lock();
        for k in ["LATENCY_MAX_MS", "RATE_LIMIT_MAX_PER_MIN"] {
            std::env::remove_var(k);
        }
        Config::from_env()
    }

    #[test]
    fn clamps() {
        let c = cfg();
        assert_eq!(clamp_latency(Some(999_999), &c), 10_000);
        assert_eq!(clamp_latency(Some(-5), &c), 0);
        assert_eq!(clamp_latency(None, &c), 0);
        assert_eq!(clamp_rate(Some(0), &c), 0); // 0 = unlimited
        assert_eq!(clamp_rate(Some(-1), &c), 0);
        assert_eq!(clamp_rate(Some(999_999_999), &c), 100_000);
        assert_eq!(clamp_chaos(Some(150), &c), 100);
        assert_eq!(clamp_chaos(Some(-1), &c), 0);
    }

    #[test]
    fn chaos_edges_deterministic() {
        // pct=0 never fires regardless of roll.
        assert_eq!(chaos_outcome(0, false, 1, 0), Chaos::None);
        // pct=100 always fires (roll<=100).
        assert_eq!(chaos_outcome(100, false, 100, 0), Chaos::Status(502));
        assert_eq!(chaos_outcome(100, false, 100, 1), Chaos::Status(503));
        assert_eq!(chaos_outcome(100, true, 1, 0), Chaos::Drop);
        // roll above pct -> no fire.
        assert_eq!(chaos_outcome(30, false, 31, 0), Chaos::None);
        assert_eq!(chaos_outcome(30, false, 30, 2), Chaos::Status(504));
    }

    #[tokio::test]
    async fn latency_applies_clamped() {
        let c = cfg();
        let applied = apply_latency(5, &c).await;
        assert_eq!(applied, 5);
        assert_eq!(apply_latency(0, &c).await, 0);
    }
}
