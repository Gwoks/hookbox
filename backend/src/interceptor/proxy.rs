//! MITM forward — PORT of `app/interceptor/proxy.py` (AC-28..32, AC-S1..S4).
//!
//! When a P1 request matches no rule (and Auto-CRUD/tunnel do not handle it) and
//! the endpoint has a `target_url`, forward to the real upstream via reqwest +
//! rustls and capture the response (served_by="mitm").
//!
//! SSRF guard (`ssrf.rs`) runs on EVERY resolved IP; the connection is PINNED to
//! the validated IP literal via reqwest's `resolve()` (no second resolution at
//! connect time) — preserving the `Host` header + TLS SNI for the original
//! hostname, defeating DNS rebinding. Redirects are off by default; each hop is
//! re-validated up to `mitm_max_redirects`. Timeout → 504; conn/DNS/SSRF → 502.
//! Body capped at `mitm_max_body_bytes` (+`X-HookBox-Truncated`). Hop-by-hop +
//! sensitive request headers are stripped before forward; upstream Set-Cookie /
//! CORS / framing headers are stripped from the response.

use std::collections::BTreeMap;
use std::time::Duration;

use url::Url;

use crate::config::Config;
use crate::helpers::strip_forward_headers;
use crate::interceptor::ssrf::resolve_and_check;

/// Response headers never copied back from upstream (hop-by-hop + Set-Cookie +
/// upstream CORS + framing — we inject our own P1 CORS and re-send the body).
const STRIP_RESPONSE_HEADERS: [&str; 16] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "content-encoding",
    "set-cookie",
    "access-control-allow-origin",
    "access-control-allow-credentials",
    "access-control-expose-headers",
    "access-control-max-age",
    "vary",
];

/// The captured upstream response (or an error mapped to 502/504 by the caller).
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncated: bool,
}

/// Why a forward failed (mapped to the §5.5 status by the engine).
#[derive(Debug)]
pub enum ProxyError {
    /// SSRF block / connection / DNS error → 502 upstream_unreachable.
    Unreachable(String),
    /// Timeout → 504 upstream_timeout.
    Timeout,
}

/// Build `target_url + mock_path + ?query` (arch §4.4).
fn build_target_url(
    target_url: &str,
    mock_path: &str,
    query: &BTreeMap<String, String>,
) -> Result<Url, ProxyError> {
    let base = Url::parse(target_url)
        .map_err(|e| ProxyError::Unreachable(format!("bad target_url: {e}")))?;
    let base_path = base.path().trim_end_matches('/');
    let mp = if mock_path.starts_with('/') {
        mock_path.to_string()
    } else {
        format!("/{mock_path}")
    };
    let full_path = format!("{base_path}{mp}");
    let full_path = if full_path.is_empty() {
        "/".to_string()
    } else {
        full_path
    };
    let mut url = base.clone();
    url.set_path(&full_path);
    if query.is_empty() {
        // keep base query (if any)
    } else {
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        for (k, v) in query {
            pairs.append_pair(k, v);
        }
        drop(pairs);
    }
    Ok(url)
}

fn safe_response_headers(resp: &reqwest::Response) -> Vec<(String, String)> {
    resp.headers()
        .iter()
        .filter(|(k, _)| {
            !STRIP_RESPONSE_HEADERS.contains(&k.as_str().to_ascii_lowercase().as_str())
        })
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|vs| (k.as_str().to_string(), vs.to_string()))
        })
        .collect()
}

/// Forward an unmatched request to `target_url`. Returns the captured upstream
/// response, or a `ProxyError` the engine maps to 502/504.
pub async fn forward(
    cfg: &Config,
    target_url: &str,
    method: &str,
    mock_path: &str,
    query: &BTreeMap<String, String>,
    headers_lower: &BTreeMap<String, String>,
    body: &[u8],
) -> Result<ProxyResponse, ProxyError> {
    let target_url = target_url.trim();
    if target_url.is_empty() {
        return Err(ProxyError::Unreachable(
            "no upstream target configured".into(),
        ));
    }
    let redirects_allowed = if cfg.mitm_follow_redirects {
        cfg.mitm_max_redirects
    } else {
        0
    };
    let fwd_headers = strip_forward_headers(headers_lower);

    let mut current_url = build_target_url(target_url, mock_path, query)?;
    let mut current_method = method.to_ascii_uppercase();
    let mut current_body = body.to_vec();
    let mut redirects_left = redirects_allowed;

    loop {
        let host = current_url
            .host_str()
            .ok_or_else(|| ProxyError::Unreachable("target has no host".into()))?
            .to_string();
        let port = current_url
            .port_or_known_default()
            .ok_or_else(|| ProxyError::Unreachable("target has no port".into()))?;

        // SSRF guard on every resolved IP; PIN to the validated address.
        let validated = resolve_and_check(&host, cfg.mitm_allow_private)
            .map_err(|e| ProxyError::Unreachable(e.0))?;
        let pinned = std::net::SocketAddr::new(validated[0], port);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.mitm_timeout_s))
            .redirect(reqwest::redirect::Policy::none()) // we re-validate each hop
            .resolve(&host, pinned) // pin DNS to the validated IP (no rebinding)
            .build()
            .map_err(|e| ProxyError::Unreachable(format!("client build: {e}")))?;

        let m = reqwest::Method::from_bytes(current_method.as_bytes())
            .map_err(|_| ProxyError::Unreachable("bad method".into()))?;
        let mut req = client.request(m, current_url.clone());
        for (k, v) in &fwd_headers {
            req = req.header(k, v);
        }
        if !current_body.is_empty() {
            req = req.body(current_body.clone());
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() => return Err(ProxyError::Timeout),
            Err(e) => return Err(ProxyError::Unreachable(format!("{e}"))),
        };

        let status = resp.status();
        // Manual redirect handling so each hop is re-validated (AC-S4 redirects).
        if status.is_redirection() && redirects_left > 0 {
            if let Some(loc) = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
            {
                redirects_left -= 1;
                current_url = current_url
                    .join(loc)
                    .map_err(|e| ProxyError::Unreachable(format!("bad redirect: {e}")))?;
                let code = status.as_u16();
                if (code == 301 || code == 302 || code == 303)
                    && current_method != "GET"
                    && current_method != "HEAD"
                {
                    current_method = "GET".into();
                    current_body.clear();
                } else {
                    current_body.clear();
                }
                continue;
            }
        }

        let out_status = status.as_u16();
        let mut out_headers = safe_response_headers(&resp);
        let full = resp
            .bytes()
            .await
            .map_err(|e| ProxyError::Unreachable(format!("read body: {e}")))?;
        let mut body = full.to_vec();
        let truncated = body.len() > cfg.mitm_max_body_bytes;
        if truncated {
            body.truncate(cfg.mitm_max_body_bytes);
            out_headers.push(("X-HookBox-Truncated".into(), "true".into()));
        }
        return Ok(ProxyResponse {
            status: out_status,
            headers: out_headers,
            body,
            truncated,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::from_env()
    }

    #[test]
    fn builds_target_url_with_path_and_query() {
        let mut q = BTreeMap::new();
        q.insert("a".to_string(), "1".to_string());
        let url = build_target_url("https://up.example.com/base/", "/users/5", &q).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("up.example.com"));
        assert_eq!(url.path(), "/base/users/5");
        assert_eq!(url.query(), Some("a=1"));
    }

    #[tokio::test]
    async fn ssrf_blocked_target_is_unreachable() {
        // A loopback target must be refused before any connection (502).
        let res = forward(
            &cfg(),
            "http://127.0.0.1:9/x",
            "GET",
            "/x",
            &BTreeMap::new(),
            &BTreeMap::new(),
            b"",
        )
        .await;
        assert!(matches!(res, Err(ProxyError::Unreachable(_))));
    }
}
