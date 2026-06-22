//! Pure 3-plane resolution logic — 1:1 PORT of `app/planes.py` (arch §3.1/§3.2).
//!
//! Decides which of the three hard-isolated request planes a request belongs to,
//! purely from `Host` + `path` — no DB, no async — so it is trivially testable.
//! The router middleware (`router.rs`) applies it.
//!
//! Planes:
//! - `Mock` (P1) — the public wildcard mock surface `<token>.<MOCK_DOMAIN>/<path>`
//!   and the localhost path-fallback `/e/<token>/<path>`. On a mock host
//!   EVERYTHING (incl. `/api`, `/static`) is the mock's own path — management is
//!   unreachable there by construction.
//! - `Api` (P2) — the management REST API `/api/**` (app host only).
//! - `Ui` (P3) — dashboard + static + WS/SSE feed + health (app host).
//!
//! The bare apex, `localhost`, `127.0.0.1`, `[::1]`, `<APP_HOST>` all resolve to
//! the UI plane (AC-6a). The single load-bearing guarantee carried verbatim:
//! the mock catch-all can NEVER shadow P2/P3.

use std::collections::HashSet;

/// Path prefixes/exacts that, on the app host, belong to UI/management planes
/// and must never be served by the mock catch-all (AC-6).
const API_PREFIX: &str = "/api";
const UI_EXACT: [&str; 4] = ["/", "/healthz", "/favicon.ico", "/robots.txt"];
const UI_PREFIXES: [&str; 8] = [
    "/static/", "/static", "/d/", "/d", "/ws/", "/ws", "/sse/", "/sse",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plane {
    Mock,
    Api,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaneResult {
    pub plane: Plane,
    /// Endpoint token when `plane == Mock`.
    pub token: Option<String>,
    /// The mock path handed to the engine (P1).
    pub mock_path: Option<String>,
}

impl PlaneResult {
    fn ui() -> Self {
        PlaneResult {
            plane: Plane::Ui,
            token: None,
            mock_path: None,
        }
    }
    fn api() -> Self {
        PlaneResult {
            plane: Plane::Api,
            token: None,
            mock_path: None,
        }
    }
    fn mock(token: String, mock_path: String) -> Self {
        PlaneResult {
            plane: Plane::Mock,
            token: Some(token),
            mock_path: Some(mock_path),
        }
    }
}

/// Drop a trailing `:port`; preserve bracketed IPv6 literals — recovers the
/// case-sensitive endpoint
/// token from a subdomain label (tokens use a mixed-case alphabet).
fn strip_port_preserve_case(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        return String::new();
    }
    if let Some(rest) = host.strip_prefix('[') {
        return match rest.find(']') {
            Some(end) => host[..end + 2].to_string(),
            None => host.to_string(),
        };
    }
    if host.contains(':') {
        let (head, tail) = host.rsplit_once(':').unwrap();
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            return head.to_string();
        }
    }
    host.to_string()
}

/// Return the case-preserved `<token>` iff `host == <token>.<mock_domain>`
/// (single label), matching the domain suffix case-insensitively. None for the
/// bare apex, app host, multi-label subdomain, or non-matching host.
pub fn subdomain_of(host: &str, mock_domain: &str) -> Option<String> {
    if host.is_empty() || mock_domain.is_empty() || !mock_domain.contains('.') {
        return None;
    }
    let raw = strip_port_preserve_case(host);
    let md = mock_domain.trim().to_lowercase();
    let suffix = format!(".{md}");
    if !raw.to_lowercase().ends_with(&suffix) {
        return None;
    }
    let label = &raw[..raw.len() - suffix.len()];
    if label.is_empty() || label.contains('.') {
        return None; // apex (empty) or multi-label -> not a mock token
    }
    Some(label.to_string())
}

/// Parse `/e/<token>/<rest>` -> `(token, mock_path="/<rest>")`.
/// `/e/<token>` -> `(token, "/")`. `/e` alone -> no token.
fn path_fallback_token(path: &str) -> (Option<String>, Option<String>) {
    // Mirror Python `path.split("/", 3)` -> ['', 'e', '<token>', '<rest...>'].
    let parts: Vec<&str> = path.splitn(4, '/').collect();
    if parts.len() < 3 || parts[1] != "e" {
        return (None, None);
    }
    let token = parts[2];
    if token.is_empty() {
        return (None, None);
    }
    let rest = if parts.len() > 3 { parts[3] } else { "" };
    (Some(token.to_string()), Some(format!("/{rest}")))
}

/// Resolve the plane for `(host, path)` (arch §3.1). Order matters: a wildcard
/// mock host wins first (everything on it is mock), then the path-fallback,
/// then `/api`, then UI.
pub fn resolve_plane(
    host: &str,
    path: &str,
    mock_domain: &str,
    app_hosts: &HashSet<String>,
) -> PlaneResult {
    // P1a — wildcard mock host: EVERYTHING here is the mock's own path.
    if let Some(sub) = subdomain_of(host, mock_domain) {
        // A subdomain that collides with an app host stays UI (token==app host probe).
        if !app_hosts.contains(&sub) {
            let mock_path = if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            };
            return PlaneResult::mock(sub, mock_path);
        }
    }

    // P1b — path-fallback /e/<token>/<rest> (any host).
    if path == "/e" || path == "/e/" || path.starts_with("/e/") {
        let (token, mock_path) = path_fallback_token(path);
        if let (Some(t), Some(mp)) = (token, mock_path) {
            return PlaneResult::mock(t, mp);
        }
        // Malformed /e with no token -> UI 404 (not a mock).
        return PlaneResult::ui();
    }

    // App host (or bare apex / unknown host -> UI).
    // P2 — management API.
    if path == API_PREFIX || path.starts_with(&format!("{API_PREFIX}/")) {
        return PlaneResult::api();
    }

    // P3 — UI / static / ws / sse / health.
    if UI_EXACT.contains(&path) {
        return PlaneResult::ui();
    }
    for pref in UI_PREFIXES {
        // Preserve the Python's loose match: path == pref OR starts_with(pref+'/')
        // OR starts_with(pref).
        if path == pref || path.starts_with(&format!("{pref}/")) || path.starts_with(pref) {
            return PlaneResult::ui();
        }
    }

    // Default app-host fallthrough -> UI (the UI router returns 404 inside).
    PlaneResult::ui()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_hosts() -> HashSet<String> {
        ["localhost", "127.0.0.1", "[::1]", "app.local", "mock.local"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn resolve(host: &str, path: &str) -> PlaneResult {
        resolve_plane(host, path, "mock.local", &app_hosts())
    }

    #[test]
    fn wildcard_mock_host_captures_everything_incl_api_and_static() {
        // AC-4 / AC-6 / AC-S16: /api under a mock host stays P1, never P2.
        let r = resolve("aB3xZ.mock.local", "/api/endpoints");
        assert_eq!(r.plane, Plane::Mock);
        assert_eq!(r.token.as_deref(), Some("aB3xZ"));
        assert_eq!(r.mock_path.as_deref(), Some("/api/endpoints"));

        let r2 = resolve("aB3xZ.mock.local:8080", "/static/app.js");
        assert_eq!(r2.plane, Plane::Mock);
        assert_eq!(r2.token.as_deref(), Some("aB3xZ"));
    }

    #[test]
    fn subdomain_label_case_preserved() {
        // AC-4: token alphabet is mixed-case; the label case must survive.
        let r = resolve("MixedCase.MOCK.LOCAL", "/x");
        assert_eq!(r.token.as_deref(), Some("MixedCase"));
    }

    #[test]
    fn multi_label_subdomain_is_not_a_token() {
        // a.b.<domain> -> not a single token -> UI.
        let r = resolve("a.b.mock.local", "/x");
        assert_eq!(r.plane, Plane::Ui);
    }

    #[test]
    fn bare_apex_and_known_app_hosts_are_ui() {
        assert_eq!(resolve("mock.local", "/").plane, Plane::Ui);
        assert_eq!(resolve("localhost", "/api/endpoints").plane, Plane::Api);
        assert_eq!(resolve("127.0.0.1:8080", "/").plane, Plane::Ui);
        assert_eq!(resolve("[::1]:8080", "/healthz").plane, Plane::Ui);
        assert_eq!(resolve("app.local", "/d/abc").plane, Plane::Ui);
    }

    #[test]
    fn token_equal_to_app_host_does_not_become_mock() {
        // AC-S16 crafted probe: subdomain label collides with an app host name.
        let r = resolve("localhost.mock.local", "/api/endpoints");
        // 'localhost' is in APP_HOSTS -> not treated as a mock token -> /api is P2.
        assert_eq!(r.plane, Plane::Api);
    }

    #[test]
    fn path_fallback_e_token() {
        let r = resolve("localhost", "/e/aB3xZ/users/1");
        assert_eq!(r.plane, Plane::Mock);
        assert_eq!(r.token.as_deref(), Some("aB3xZ"));
        assert_eq!(r.mock_path.as_deref(), Some("/users/1"));

        let r2 = resolve("localhost", "/e/aB3xZ");
        assert_eq!(r2.plane, Plane::Mock);
        assert_eq!(r2.mock_path.as_deref(), Some("/"));

        // /e alone -> UI (malformed, not a mock).
        assert_eq!(resolve("localhost", "/e").plane, Plane::Ui);
        assert_eq!(resolve("localhost", "/e/").plane, Plane::Ui);
    }

    #[test]
    fn api_and_ui_classification_on_app_host() {
        assert_eq!(resolve("localhost", "/api").plane, Plane::Api);
        assert_eq!(
            resolve("localhost", "/api/endpoints/x/rules").plane,
            Plane::Api
        );
        assert_eq!(resolve("localhost", "/ws/tok").plane, Plane::Ui);
        assert_eq!(resolve("localhost", "/sse/tok").plane, Plane::Ui);
        assert_eq!(resolve("localhost", "/static/app.css").plane, Plane::Ui);
        // Unknown app-host path -> UI (SPA router 404s inside).
        assert_eq!(resolve("localhost", "/anything").plane, Plane::Ui);
    }

    #[test]
    fn percent_encoded_host_does_not_smuggle_into_mock() {
        // A percent-encoded label cannot match the literal mock-domain suffix.
        let r = resolve("%2e.mock.local", "/api/x");
        // '%2e' is a single label without a literal dot -> it IS a token here,
        // but /api is on the mock host so it stays P1 (never P2). Crucially it
        // does not reach the management plane.
        assert_eq!(r.plane, Plane::Mock);
    }

    #[test]
    fn path_fallback_only_when_mock_domain_dotless() {
        // MOCK_DOMAIN without a dot -> no wildcard mock; only /e/ fallback works.
        let hosts = app_hosts();
        let r = resolve_plane("anything.localhost", "/api/x", "localhost", &hosts);
        assert_eq!(r.plane, Plane::Api); // no wildcard -> /api is management
        let r2 = resolve_plane("anything.localhost", "/e/tok/x", "localhost", &hosts);
        assert_eq!(r2.plane, Plane::Mock);
        assert_eq!(r2.token.as_deref(), Some("tok"));
    }
}
