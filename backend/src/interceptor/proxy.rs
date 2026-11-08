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
use crate::interceptor::ssrf::resolve_and_check;
use crate::net::headers::{header_pairs, is_mitm_response_stripped, strip_forward_headers};

/// The captured upstream response (or an error mapped to 502/504 by the caller).
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncated: bool,
}

/// Why a forward failed (mapped to the §5.5 status by the engine).
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// SSRF block / connection / DNS error → 502 upstream_unreachable.
    #[error("upstream unreachable: {0}")]
    Unreachable(String),
    /// Timeout → 504 upstream_timeout.
    #[error("upstream timed out")]
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

/// Upstream response headers safe to copy back: `is_mitm_response_stripped`
/// drops hop-by-hop + Set-Cookie + upstream CORS + framing (we inject our own
/// P1 CORS and re-send the body).
fn safe_response_headers(resp: &reqwest::Response) -> Vec<(String, String)> {
    header_pairs(resp.headers())
        .filter(|(k, _)| !is_mitm_response_stripped(k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// One request hop: where, how, and with what body.
struct Hop {
    url: Url,
    method: String,
    body: Vec<u8>,
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
    let fwd_headers = strip_forward_headers(headers_lower);
    let mut hop = Hop {
        url: build_target_url(target_url, mock_path, query)?,
        method: method.to_ascii_uppercase(),
        body: body.to_vec(),
    };
    let mut redirects_left = if cfg.mitm_follow_redirects {
        cfg.mitm_max_redirects
    } else {
        0
    };
    loop {
        let resp = send(cfg, &hop, &fwd_headers).await?;
        // Manual redirect handling so each hop is re-validated (AC-S4 redirects).
        if redirects_left > 0 {
            if let Some(next) = next_hop(&hop, &resp)? {
                redirects_left -= 1;
                hop = next;
                continue;
            }
        }
        return read_capped(resp, cfg).await;
    }
}

/// A client for one hop whose DNS is PINNED to the SSRF-validated address of
/// `url`'s host (no second resolution at connect time, so no rebinding);
/// `Host` / TLS SNI still carry the original hostname. Redirects are off —
/// `forward` re-validates each hop itself.
fn pinned_client(cfg: &Config, url: &Url) -> Result<reqwest::Client, ProxyError> {
    let host = url
        .host_str()
        .ok_or_else(|| ProxyError::Unreachable("target has no host".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ProxyError::Unreachable("target has no port".into()))?;
    // SSRF guard on every resolved IP; PIN to the validated address.
    let validated = resolve_and_check(host, cfg.mitm_allow_private)
        .map_err(|e| ProxyError::Unreachable(e.0))?;
    let ip = validated
        .first()
        .copied()
        .ok_or_else(|| ProxyError::Unreachable("target resolved to no address".into()))?;
    reqwest::Client::builder()
        .timeout(Duration::from_secs(cfg.mitm_timeout_s))
        .redirect(reqwest::redirect::Policy::none())
        .resolve(host, std::net::SocketAddr::new(ip, port))
        .build()
        .map_err(|e| ProxyError::Unreachable(format!("client build: {e}")))
}

/// Send one hop (timeout → 504, anything else → 502).
async fn send(
    cfg: &Config,
    hop: &Hop,
    headers: &BTreeMap<String, String>,
) -> Result<reqwest::Response, ProxyError> {
    let client = pinned_client(cfg, &hop.url)?;
    let method = reqwest::Method::from_bytes(hop.method.as_bytes())
        .map_err(|_| ProxyError::Unreachable("bad method".into()))?;
    let mut req = client.request(method, hop.url.clone());
    for (k, v) in headers {
        req = req.header(k, v);
    }
    if !hop.body.is_empty() {
        req = req.body(hop.body.clone());
    }
    req.send().await.map_err(|e| {
        if e.is_timeout() {
            ProxyError::Timeout
        } else {
            ProxyError::Unreachable(format!("{e}"))
        }
    })
}

/// The hop a redirect response points to, if it is one with a `Location`.
/// The body is always dropped; 301/302/303 also turn a non-GET/HEAD into a
/// GET.
fn next_hop(hop: &Hop, resp: &reqwest::Response) -> Result<Option<Hop>, ProxyError> {
    let status = resp.status();
    if !status.is_redirection() {
        return Ok(None);
    }
    let Some(location) = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
    else {
        return Ok(None);
    };
    let url = hop
        .url
        .join(location)
        .map_err(|e| ProxyError::Unreachable(format!("bad redirect: {e}")))?;
    let see_other = matches!(status.as_u16(), 301..=303);
    let method = if see_other && hop.method != "GET" && hop.method != "HEAD" {
        "GET".to_string()
    } else {
        hop.method.clone()
    };
    Ok(Some(Hop {
        url,
        method,
        body: Vec::new(),
    }))
}

/// Capture the upstream status, safe headers and body, capped at
/// `mitm_max_body_bytes` (+ `X-HookBox-Truncated: true`).
async fn read_capped(resp: reqwest::Response, cfg: &Config) -> Result<ProxyResponse, ProxyError> {
    let status = resp.status().as_u16();
    let mut headers = safe_response_headers(&resp);
    let full = resp
        .bytes()
        .await
        .map_err(|e| ProxyError::Unreachable(format!("read body: {e}")))?;
    let mut body = full.to_vec();
    let truncated = body.len() > cfg.mitm_max_body_bytes;
    if truncated {
        body.truncate(cfg.mitm_max_body_bytes);
        headers.push(("X-HookBox-Truncated".into(), "true".into()));
    }
    Ok(ProxyResponse {
        status,
        headers,
        body,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        crate::testing::test_config()
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
