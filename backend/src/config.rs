//! Env-driven configuration — PORT of `config.py` (HookBox spec §2/§5/§6).
//!
//! All values are env-driven with safe defaults (AC-44): the app must never
//! crash on a missing env var. An unset/misconfigured `MOCK_DOMAIN` degrades to
//! path-fallback-only mode (`path_fallback_only = true`) rather than failing.
//! Default serve port is `8080` (OQ-4, supersedes the Python `8000`).

use std::collections::HashSet;

/// Reads one raw setting by name; `None` when unset (or not valid unicode).
type Lookup<'a> = &'a dyn Fn(&str) -> Option<String>;

fn str_env(lookup: Lookup<'_>, name: &str, fallback: &str) -> String {
    match lookup(name) {
        Some(v) if !v.trim().is_empty() => v,
        _ => fallback.to_string(),
    }
}

fn int_env(lookup: Lookup<'_>, name: &str, fallback: i64) -> i64 {
    match lookup(name) {
        Some(v) if !v.trim().is_empty() => v.trim().parse::<i64>().unwrap_or(fallback),
        _ => fallback,
    }
}

fn bool_env(lookup: Lookup<'_>, name: &str, fallback: bool) -> bool {
    match lookup(name) {
        Some(v) if !v.trim().is_empty() => {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }
        _ => fallback,
    }
}

fn float_env(lookup: Lookup<'_>, name: &str, fallback: f64) -> f64 {
    match lookup(name) {
        Some(v) if !v.trim().is_empty() => v.trim().parse::<f64>().unwrap_or(fallback),
        _ => fallback,
    }
}

/// Hosts that always resolve to the UI plane (mirrors config.py APP_HOSTS):
/// the loopback names, the configured app host, and the bare mock apex (which
/// serves the UI, not the interceptor).
fn app_hosts(app_host: &str, mock_domain: &str) -> HashSet<String> {
    let mut hosts: HashSet<String> = ["localhost", "127.0.0.1", "[::1]"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if !app_host.is_empty() {
        hosts.insert(app_host.to_string());
    }
    if !mock_domain.is_empty() {
        hosts.insert(mock_domain.to_string());
    }
    hosts
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
    /// Whether the owner-key cookie is `Secure` (and therefore named
    /// `__Host-hookbox_key`). `SESSION_COOKIE_SECURE`, else derived from an
    /// `https://` `PUBLIC_BASE_URL`.
    pub session_cookie_secure: bool,

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
    /// Load from the process environment.
    pub fn from_env() -> Self {
        Self::from_source(|key| std::env::var(key).ok())
    }

    /// Load from any key -> value source. `from_env` is this over
    /// `std::env::var`; tests pass a fixed map so they never mutate the
    /// process-global environment.
    pub fn from_source(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let lookup: Lookup<'_> = &lookup;
        let mock_domain = str_env(lookup, "MOCK_DOMAIN", "mock.local")
            .trim()
            .to_string();
        let app_host = str_env(lookup, "APP_HOST", "localhost").trim().to_string();
        let app_hosts = app_hosts(&app_host, &mock_domain);

        // Derived: path-fallback-only when MOCK_DOMAIN is blank or has no dot.
        let path_fallback_only = mock_domain.is_empty() || !mock_domain.contains('.');

        // Normalized without a trailing slash so `{base}/e/<token>` joins cleanly.
        let public_base_url = str_env(lookup, "PUBLIC_BASE_URL", "")
            .trim()
            .trim_end_matches('/')
            .to_string();
        let session_cookie_secure = bool_env(
            lookup,
            "SESSION_COOKIE_SECURE",
            public_base_url.to_ascii_lowercase().starts_with("https://"),
        );

        Config {
            mock_domain,
            app_host,
            public_base_url,
            bind_host: str_env(lookup, "APP_BIND_HOST", "0.0.0.0"),
            // OQ-4: default 8080 (supersedes Python 8000). APP_PORT then PUBLIC_PORT.
            public_port: {
                let p = int_env(lookup, "APP_PORT", int_env(lookup, "PUBLIC_PORT", 8080));
                p as u16
            },
            app_hosts,
            path_fallback_only,

            sqlite_path: str_env(lookup, "DATABASE_PATH", "data/app.db"),
            static_dir: str_env(lookup, "STATIC_DIR", "dist"),

            endpoint_id_length: int_env(lookup, "ENDPOINT_ID_LENGTH", 10) as usize,
            // 256-bit floor: the owner key is the account (no other factor).
            owner_secret_bytes: int_env(lookup, "OWNER_SECRET_BYTES", 32).max(32) as usize,

            trace_cap: int_env(lookup, "TRACE_CAP", 100),
            trace_ttl_hours: int_env(lookup, "TRACE_TTL_HOURS", 24),
            retention_sweep_seconds: int_env(lookup, "RETENTION_SWEEP_SECONDS", 300) as u64,
            gone_ttl_hours: int_env(lookup, "GONE_TTL_HOURS", 168),

            state_ttl_seconds: int_env(lookup, "STATE_TTL_SECONDS", 24 * 3600),
            crud_ttl_seconds: int_env(lookup, "CRUD_TTL_SECONDS", 24 * 3600),

            latency_max_ms: int_env(lookup, "LATENCY_MAX_MS", 10_000),
            rate_limit_max_per_min: int_env(lookup, "RATE_LIMIT_MAX_PER_MIN", 100_000),
            chaos_max_pct: 100,
            chaos_drop_timeout_s: int_env(lookup, "CHAOS_DROP_TIMEOUT_S", 30) as u64,

            max_ingest_body_bytes: int_env(lookup, "MAX_INGEST_BODY_BYTES", 1_000_000) as usize,
            max_body_bytes: int_env(lookup, "MAX_BODY_BYTES", 256_000) as usize,
            template_max_size: int_env(lookup, "TEMPLATE_MAX_SIZE", 256_000) as usize,
            template_max_tags: int_env(lookup, "TEMPLATE_MAX_TAGS", 500) as usize,
            crud_max_items: int_env(lookup, "CRUD_MAX_ITEMS", 1000) as usize,
            crud_max_item_bytes: int_env(lookup, "CRUD_MAX_ITEM_BYTES", 64_000) as usize,

            mitm_timeout_s: int_env(lookup, "MITM_TIMEOUT_S", 10) as u64,
            mitm_max_body_bytes: int_env(lookup, "MITM_MAX_BODY_BYTES", 5_000_000) as usize,
            mitm_allow_private: bool_env(lookup, "MITM_ALLOW_PRIVATE", false),
            mitm_follow_redirects: bool_env(lookup, "MITM_FOLLOW_REDIRECTS", false),
            mitm_max_redirects: int_env(lookup, "MITM_MAX_REDIRECTS", 3) as usize,

            ws_max_conn_per_endpoint: int_env(lookup, "WS_MAX_CONN_PER_ENDPOINT", 50),
            ws_send_timeout_s: float_env(lookup, "WS_SEND_TIMEOUT_S", 5.0),

            tunnel_request_timeout_s: int_env(lookup, "TUNNEL_REQUEST_TIMEOUT_S", 30) as u64,

            session_rate_limit_per_min: int_env(lookup, "SESSION_RATE_LIMIT_PER_MIN", 30),
            session_cookie_secure,

            share_code_bytes: (int_env(lookup, "SHARE_CODE_BYTES", 24).max(16)) as usize,
            share_max_per_endpoint: int_env(lookup, "SHARE_MAX_PER_ENDPOINT", 10),
            share_rate_limit_per_min: int_env(lookup, "SHARE_RATE_LIMIT_PER_MIN", 120),
            share_rate_limit_global_per_min: int_env(
                lookup,
                "SHARE_RATE_LIMIT_GLOBAL_PER_MIN",
                1200,
            ),
        }
    }

    /// True when the session cookie is sent without `Secure` while the app
    /// claims a non-loopback host — a deployment that would ship the owner
    /// key over plain http. `main` warns about it at startup.
    pub fn insecure_cookie_on_public_host(&self) -> bool {
        !self.session_cookie_secure && !matches!(self.app_host.as_str(), "localhost" | "127.0.0.1")
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::config_from;

    #[test]
    fn session_cookie_secure_follows_https_base_url_unless_overridden() {
        assert!(!config_from(&[]).session_cookie_secure);
        assert!(!config_from(&[("PUBLIC_BASE_URL", "http://h.example")]).session_cookie_secure);
        assert!(config_from(&[("PUBLIC_BASE_URL", "HTTPS://h.example")]).session_cookie_secure);
        let forced_off = config_from(&[
            ("PUBLIC_BASE_URL", "https://h.example"),
            ("SESSION_COOKIE_SECURE", "false"),
        ]);
        assert!(!forced_off.session_cookie_secure);
        assert!(config_from(&[("SESSION_COOKIE_SECURE", "1")]).session_cookie_secure);
    }

    #[test]
    fn owner_secret_bytes_clamped_to_32_floor() {
        assert_eq!(
            config_from(&[("OWNER_SECRET_BYTES", "4")]).owner_secret_bytes,
            32
        );
        assert_eq!(
            config_from(&[("OWNER_SECRET_BYTES", "48")]).owner_secret_bytes,
            48
        );
    }

    #[test]
    fn insecure_cookie_warning_only_for_non_loopback_hosts() {
        assert!(!config_from(&[]).insecure_cookie_on_public_host()); // APP_HOST=localhost
        assert!(!config_from(&[("APP_HOST", "127.0.0.1")]).insecure_cookie_on_public_host());
        assert!(
            config_from(&[("APP_HOST", "hookbox.example.com")]).insecure_cookie_on_public_host()
        );
        let secure = config_from(&[
            ("APP_HOST", "hookbox.example.com"),
            ("PUBLIC_BASE_URL", "https://hookbox.example.com"),
        ]);
        assert!(!secure.insecure_cookie_on_public_host());
    }

    #[test]
    fn defaults_apply_without_env() {
        let cfg = config_from(&[]);
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
        let cfg = config_from(&[("SHARE_CODE_BYTES", "1")]);
        assert_eq!(cfg.share_code_bytes, 16);
    }

    #[test]
    fn blank_or_dotless_mock_domain_degrades_to_path_fallback() {
        assert!(config_from(&[("MOCK_DOMAIN", "localhost")]).path_fallback_only);
        // Blank falls back to the dotted default.
        assert!(!config_from(&[("MOCK_DOMAIN", "  ")]).path_fallback_only);
    }

    #[test]
    fn public_base_url_defaults_blank_and_strips_trailing_slash() {
        assert_eq!(config_from(&[]).public_base_url, "");
        let cfg = config_from(&[("PUBLIC_BASE_URL", "https://hookbox.example.com/ ")]);
        assert_eq!(cfg.public_base_url, "https://hookbox.example.com");
    }

    #[test]
    fn app_host_joins_the_ui_host_set() {
        let cfg = config_from(&[("APP_HOST", "app.local"), ("MOCK_DOMAIN", "mock.local")]);
        assert!(cfg.app_hosts.contains("app.local"));
        assert!(cfg.app_hosts.contains("mock.local"));
        assert!(cfg.app_hosts.contains("[::1]"));
    }

    #[test]
    fn malformed_numbers_and_bools_fall_back_to_defaults() {
        let cfg = config_from(&[
            ("TRACE_CAP", "lots"),
            ("MITM_ALLOW_PRIVATE", "maybe"),
            ("WS_SEND_TIMEOUT_S", "soon"),
        ]);
        assert_eq!(cfg.trace_cap, 100);
        assert!(!cfg.mitm_allow_private);
        assert_eq!(cfg.ws_send_timeout_s, 5.0);
        assert!(config_from(&[("MITM_ALLOW_PRIVATE", " YES ")]).mitm_allow_private);
    }
}
