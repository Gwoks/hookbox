//! Env-driven configuration — PORT of `config.py` (HookBox spec §2/§5/§6).
//!
//! All values are env-driven with safe defaults (AC-44): the app must never
//! crash on a missing env var. An unset/misconfigured `MOCK_DOMAIN` degrades to
//! path-fallback-only mode (`path_fallback_only = true`) rather than failing.
//! Default serve port is `8080` (OQ-4, supersedes the Python `8000`).

use std::collections::HashSet;

fn str_env(name: &str, fallback: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => fallback.to_string(),
    }
}

fn int_env(name: &str, fallback: i64) -> i64 {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v.trim().parse::<i64>().unwrap_or(fallback),
        _ => fallback,
    }
}

fn bool_env(name: &str, fallback: bool) -> bool {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }
        _ => fallback,
    }
}

fn float_env(name: &str, fallback: f64) -> f64 {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v.trim().parse::<f64>().unwrap_or(fallback),
        _ => fallback,
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    // --- Hosting / routing (LOCKED §2) ---
    pub mock_domain: String,
    pub app_host: String,
    pub bind_host: String,
    pub public_port: u16,
    /// Host names that always resolve to the UI/management plane (never P1).
    pub app_hosts: HashSet<String>,
    /// Derived: when MOCK_DOMAIN is blank/misconfigured serve path-fallback-only.
    pub path_fallback_only: bool,
    /// Absolute origin the app is publicly reachable at (e.g.
    /// `https://hookbox.example.com`). Prefixes `/e/<token>` URLs so the UI and
    /// API hand out copy-pasteable absolute mock URLs in path-fallback mode.
    /// Blank (the default) keeps the URLs relative.
    pub public_base_url: String,

    // --- Durable store ---
    pub sqlite_path: String,
    pub static_dir: String,

    // --- Token / secret entropy ---
    pub endpoint_id_length: usize,
    pub owner_secret_bytes: usize,

    // --- Data retention (§5.8) ---
    pub trace_cap: i64,
    pub trace_ttl_hours: i64,
    pub retention_sweep_seconds: u64,
    pub gone_ttl_hours: i64,

    // --- Ephemeral TTLs (§5.8) ---
    pub state_ttl_seconds: i64,
    pub crud_ttl_seconds: i64,

    // --- Simulated network condition bounds (§5.3) ---
    pub latency_max_ms: i64,
    pub rate_limit_max_per_min: i64,
    pub chaos_max_pct: i64,
    pub chaos_drop_timeout_s: u64,

    // --- Ingest / body caps ---
    pub max_ingest_body_bytes: usize,
    pub max_body_bytes: usize,
    pub template_max_size: usize,
    pub template_max_tags: usize,
    pub crud_max_items: usize,
    pub crud_max_item_bytes: usize,

    // --- MITM / proxy policy ---
    pub mitm_timeout_s: u64,
    pub mitm_max_body_bytes: usize,
    pub mitm_allow_private: bool,
    pub mitm_follow_redirects: bool,
    pub mitm_max_redirects: usize,

    // --- Real-time feed bounds ---
    pub ws_max_conn_per_endpoint: i64,
    pub ws_send_timeout_s: f64,

    // --- Tunnel ---
    pub tunnel_request_timeout_s: u64,

    // --- Session anti-enumeration rate limit ---
    pub session_rate_limit_per_min: i64,

    // --- F4 share links (§5.8) ---
    /// CSPRNG bytes per share code -> 32-char base64url -> 192 bits (AC-31).
    /// Clamped to >= 16 at load so a misconfigured SHARE_CODE_BYTES=1 cannot
    /// mint a guessable code (128-bit floor, AC-103).
    pub share_code_bytes: usize,
    /// Max **active** share links per endpoint (AC-27).
    pub share_max_per_endpoint: i64,
    /// Per-IP limit on the public share resolver (AC-38, AC-113).
    pub share_rate_limit_per_min: i64,
    /// Instance-wide ceiling on the public share resolver (AC-S15, AC-113).
    pub share_rate_limit_global_per_min: i64,
}

impl Config {
    pub fn from_env() -> Self {
        let mock_domain = str_env("MOCK_DOMAIN", "mock.local").trim().to_string();
        let app_host = str_env("APP_HOST", "localhost").trim().to_string();

        // Hosts that always resolve to the UI plane (mirrors config.py APP_HOSTS).
        let mut app_hosts: HashSet<String> = ["localhost", "127.0.0.1", "[::1]"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        if !app_host.is_empty() {
            app_hosts.insert(app_host.clone());
        }
        if !mock_domain.is_empty() {
            // the bare apex serves the UI, not the interceptor
            app_hosts.insert(mock_domain.clone());
        }

        // Derived: path-fallback-only when MOCK_DOMAIN is blank or has no dot.
        let path_fallback_only = mock_domain.is_empty() || !mock_domain.contains('.');

        // Normalized without a trailing slash so `{base}/e/<token>` joins cleanly.
        let public_base_url = str_env("PUBLIC_BASE_URL", "")
            .trim()
            .trim_end_matches('/')
            .to_string();

        Config {
            mock_domain,
            app_host,
            public_base_url,
            bind_host: str_env("APP_BIND_HOST", "0.0.0.0"),
            // OQ-4: default 8080 (supersedes Python 8000). APP_PORT then PUBLIC_PORT.
            public_port: {
                let p = int_env("APP_PORT", int_env("PUBLIC_PORT", 8080));
                p as u16
            },
            app_hosts,
            path_fallback_only,

            sqlite_path: str_env("DATABASE_PATH", "data/app.db"),
            static_dir: str_env("STATIC_DIR", "dist"),

            endpoint_id_length: int_env("ENDPOINT_ID_LENGTH", 10) as usize,
            owner_secret_bytes: int_env("OWNER_SECRET_BYTES", 32) as usize,

            trace_cap: int_env("TRACE_CAP", 100),
            trace_ttl_hours: int_env("TRACE_TTL_HOURS", 24),
            retention_sweep_seconds: int_env("RETENTION_SWEEP_SECONDS", 300) as u64,
            gone_ttl_hours: int_env("GONE_TTL_HOURS", 168),

            state_ttl_seconds: int_env("STATE_TTL_SECONDS", 24 * 3600),
            crud_ttl_seconds: int_env("CRUD_TTL_SECONDS", 24 * 3600),

            latency_max_ms: int_env("LATENCY_MAX_MS", 10_000),
            rate_limit_max_per_min: int_env("RATE_LIMIT_MAX_PER_MIN", 100_000),
            chaos_max_pct: 100,
            chaos_drop_timeout_s: int_env("CHAOS_DROP_TIMEOUT_S", 30) as u64,

            max_ingest_body_bytes: int_env("MAX_INGEST_BODY_BYTES", 1_000_000) as usize,
            max_body_bytes: int_env("MAX_BODY_BYTES", 256_000) as usize,
            template_max_size: int_env("TEMPLATE_MAX_SIZE", 256_000) as usize,
            template_max_tags: int_env("TEMPLATE_MAX_TAGS", 500) as usize,
            crud_max_items: int_env("CRUD_MAX_ITEMS", 1000) as usize,
            crud_max_item_bytes: int_env("CRUD_MAX_ITEM_BYTES", 64_000) as usize,

            mitm_timeout_s: int_env("MITM_TIMEOUT_S", 10) as u64,
            mitm_max_body_bytes: int_env("MITM_MAX_BODY_BYTES", 5_000_000) as usize,
            mitm_allow_private: bool_env("MITM_ALLOW_PRIVATE", false),
            mitm_follow_redirects: bool_env("MITM_FOLLOW_REDIRECTS", false),
            mitm_max_redirects: int_env("MITM_MAX_REDIRECTS", 3) as usize,

            ws_max_conn_per_endpoint: int_env("WS_MAX_CONN_PER_ENDPOINT", 50),
            ws_send_timeout_s: float_env("WS_SEND_TIMEOUT_S", 5.0),

            tunnel_request_timeout_s: int_env("TUNNEL_REQUEST_TIMEOUT_S", 30) as u64,

            session_rate_limit_per_min: int_env("SESSION_RATE_LIMIT_PER_MIN", 30),

            share_code_bytes: (int_env("SHARE_CODE_BYTES", 24).max(16)) as usize,
            share_max_per_endpoint: int_env("SHARE_MAX_PER_ENDPOINT", 10),
            share_rate_limit_per_min: int_env("SHARE_RATE_LIMIT_PER_MIN", 120),
            share_rate_limit_global_per_min: int_env("SHARE_RATE_LIMIT_GLOBAL_PER_MIN", 1200),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_without_env() {
        // Serialize against every other env-mutating test in the binary so the
        // remove_var -> from_env window can't observe another test's writes.
        let _guard = crate::testutil::env_lock();
        for k in [
            "MOCK_DOMAIN",
            "APP_HOST",
            "APP_PORT",
            "PUBLIC_PORT",
            "TRACE_CAP",
            "SESSION_RATE_LIMIT_PER_MIN",
            "MITM_ALLOW_PRIVATE",
            "SHARE_CODE_BYTES",
            "SHARE_MAX_PER_ENDPOINT",
            "SHARE_RATE_LIMIT_PER_MIN",
            "SHARE_RATE_LIMIT_GLOBAL_PER_MIN",
        ] {
            std::env::remove_var(k);
        }
        let cfg = Config::from_env();
        // OQ-4: default port is 8080, not the Python 8000.
        assert_eq!(cfg.public_port, 8080);
        assert_eq!(cfg.trace_cap, 100);
        assert_eq!(cfg.session_rate_limit_per_min, 30);
        assert_eq!(cfg.chaos_max_pct, 100);
        assert!(!cfg.mitm_allow_private);
        // default MOCK_DOMAIN=mock.local has a dot -> wildcard mode enabled.
        assert!(!cfg.path_fallback_only);
        assert!(cfg.app_hosts.contains("localhost"));
        assert!(cfg.app_hosts.contains("mock.local"));
        // §5.8 F4 share-link defaults (AC-27, AC-31, AC-38, AC-113).
        assert_eq!(cfg.share_code_bytes, 24);
        assert_eq!(cfg.share_max_per_endpoint, 10);
        assert_eq!(cfg.share_rate_limit_per_min, 120);
        assert_eq!(cfg.share_rate_limit_global_per_min, 1200);
    }

    #[test]
    fn share_code_bytes_clamped_to_16_floor() {
        // AC-103: a misconfigured SHARE_CODE_BYTES=1 must not mint a
        // guessable code — the loaded config clamps to the 128-bit floor.
        let _guard = crate::testutil::env_lock();
        std::env::set_var("SHARE_CODE_BYTES", "1");
        let cfg = Config::from_env();
        assert_eq!(cfg.share_code_bytes, 16);
        std::env::remove_var("SHARE_CODE_BYTES");
    }

    #[test]
    fn blank_or_dotless_mock_domain_degrades_to_path_fallback() {
        let _guard = crate::testutil::env_lock();
        std::env::set_var("MOCK_DOMAIN", "localhost");
        let cfg = Config::from_env();
        assert!(cfg.path_fallback_only);
        std::env::remove_var("MOCK_DOMAIN");
    }

    #[test]
    fn public_base_url_defaults_blank_and_strips_trailing_slash() {
        let _guard = crate::testutil::env_lock();
        std::env::remove_var("PUBLIC_BASE_URL");
        assert_eq!(Config::from_env().public_base_url, "");
        std::env::set_var("PUBLIC_BASE_URL", "https://hookbox.example.com/ ");
        assert_eq!(
            Config::from_env().public_base_url,
            "https://hookbox.example.com"
        );
        std::env::remove_var("PUBLIC_BASE_URL");
    }
}
