//! Hand-rolled session-cookie plumbing (axum 0.7 ships no cookie jar, and
//! this needs only a handful of exact operations).
//!
//! The cookie holds the raw owner key. Its name depends on the deployment:
//! `__Host-hookbox_key` when `cfg.session_cookie_secure` (the `__Host-`
//! prefix makes browsers refuse it unless `Secure; Path=/` and no `Domain`),
//! else `hookbox_key` for plain-http local dev. Only the configured name is
//! ever READ; both names are always STRIPPED from anything that leaves the
//! management plane.

use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue};

use super::{COOKIE_MAX_AGE, COOKIE_NAME_PLAIN, COOKIE_NAME_SECURE};
use crate::config::Config;

/// The session cookie name this deployment reads and writes.
pub fn cookie_name(cfg: &Config) -> &'static str {
    if cfg.session_cookie_secure {
        COOKIE_NAME_SECURE
    } else {
        COOKIE_NAME_PLAIN
    }
}

/// One parsed cookie: the whitespace-trimmed raw `name=value` slice plus
/// its trimmed name and value.
struct CookiePair<'a> {
    raw: &'a [u8],
    name: &'a [u8],
    value: &'a [u8],
}

/// The cookies of one `Cookie` header value. Works on raw bytes so a
/// non-UTF-8 header can still be parsed and filtered rather than passed
/// through whole.
fn cookie_pairs(value: &[u8]) -> impl Iterator<Item = CookiePair<'_>> {
    value.split(|b| *b == b';').filter_map(|raw| {
        let raw = raw.trim_ascii();
        if raw.is_empty() {
            return None;
        }
        let (name, value) = match raw.iter().position(|b| *b == b'=') {
            Some(i) => (raw[..i].trim_ascii(), raw[i + 1..].trim_ascii()),
            None => (raw, &b""[..]),
        };
        Some(CookiePair { raw, name, value })
    })
}

/// The first non-empty value of the configured session cookie across EVERY
/// `Cookie` header (HTTP/2 may split cookies over several), exact name match.
pub fn read_session_cookie(headers: &HeaderMap, cfg: &Config) -> Option<String> {
    let name = cookie_name(cfg).as_bytes();
    headers
        .get_all(COOKIE)
        .iter()
        .flat_map(|v| cookie_pairs(v.as_bytes()))
        .find(|c| c.name == name && !c.value.is_empty())
        .and_then(|c| std::str::from_utf8(c.value).ok())
        .map(str::to_string)
}

fn attributes(cfg: &Config, max_age: i64) -> String {
    let secure = if cfg.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!("Path=/; Max-Age={max_age}; HttpOnly; SameSite=Strict{secure}")
}

/// The `Set-Cookie` value that stores `key` for ~400 days. No `Domain`, so
/// the cookie is host-only and never reaches a sibling mock subdomain.
pub fn set_cookie_value(cfg: &Config, key: &str) -> String {
    format!(
        "{}={key}; {}",
        cookie_name(cfg),
        attributes(cfg, COOKIE_MAX_AGE)
    )
}

/// The `Set-Cookie` value that deletes the session cookie (`Max-Age=0`).
pub fn clear_cookie_value(cfg: &Config) -> String {
    format!("{}=; {}", cookie_name(cfg), attributes(cfg, 0))
}

fn is_session_cookie_name(name: &[u8]) -> bool {
    name.eq_ignore_ascii_case(COOKIE_NAME_SECURE.as_bytes())
        || name.eq_ignore_ascii_case(COOKIE_NAME_PLAIN.as_bytes())
}

/// Remove BOTH session cookie names from every incoming `Cookie` header,
/// keeping every other cookie; a header left empty is dropped entirely.
pub fn strip_session_cookie(headers: &mut HeaderMap) {
    let kept: Vec<HeaderValue> = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| {
            let rest: Vec<&[u8]> = cookie_pairs(v.as_bytes())
                .filter(|c| !is_session_cookie_name(c.name))
                .map(|c| c.raw)
                .collect();
            if rest.is_empty() {
                return None;
            }
            HeaderValue::from_bytes(&rest.join(&b"; "[..])).ok()
        })
        .collect();
    headers.remove(COOKIE);
    for v in kept {
        headers.append(COOKIE, v);
    }
}

/// Whether a `Set-Cookie` line would set (or clear) one of our session
/// cookies — either name, compared case-insensitively.
pub fn is_session_cookie(set_cookie_line: &str) -> bool {
    let name = set_cookie_line
        .split(['=', ';'])
        .next()
        .unwrap_or("")
        .trim();
    is_session_cookie_name(name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::config_from;

    fn plain() -> Config {
        config_from(&[])
    }

    fn secure() -> Config {
        config_from(&[("PUBLIC_BASE_URL", "https://hookbox.example.com")])
    }

    fn with_cookies(values: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(COOKIE, v.parse().unwrap());
        }
        h
    }

    #[test]
    fn plain_cookie_has_strict_httponly_attributes_without_secure() {
        let v = set_cookie_value(&plain(), "KEY");
        assert_eq!(
            v,
            "hookbox_key=KEY; Path=/; Max-Age=34560000; HttpOnly; SameSite=Strict"
        );
        assert!(!v.contains("Domain"));
    }

    #[test]
    fn secure_cookie_uses_host_prefix_and_secure() {
        let v = set_cookie_value(&secure(), "KEY");
        assert_eq!(
            v,
            "__Host-hookbox_key=KEY; Path=/; Max-Age=34560000; HttpOnly; SameSite=Strict; Secure"
        );
        assert!(!v.contains("Domain"));
    }

    #[test]
    fn clear_cookie_has_max_age_zero() {
        assert_eq!(
            clear_cookie_value(&plain()),
            "hookbox_key=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict"
        );
        assert!(clear_cookie_value(&secure()).starts_with("__Host-hookbox_key=; "));
        assert!(clear_cookie_value(&secure()).ends_with("; Secure"));
    }

    #[test]
    fn reads_across_multiple_cookie_headers_with_exact_name() {
        let h = with_cookies(&["a=1; xhookbox_key=no", "b=2;  hookbox_key=KEY ; c=3"]);
        assert_eq!(read_session_cookie(&h, &plain()).as_deref(), Some("KEY"));
    }

    #[test]
    fn ignores_the_other_modes_cookie_name() {
        let h = with_cookies(&["__Host-hookbox_key=SECURE"]);
        assert_eq!(read_session_cookie(&h, &plain()), None);
        let h = with_cookies(&["hookbox_key=PLAIN"]);
        assert_eq!(read_session_cookie(&h, &secure()), None);
        let h = with_cookies(&["hookbox_key=PLAIN; __Host-hookbox_key=SECURE"]);
        assert_eq!(
            read_session_cookie(&h, &secure()).as_deref(),
            Some("SECURE")
        );
    }

    #[test]
    fn empty_value_reads_as_absent() {
        let h = with_cookies(&["hookbox_key=; other=1"]);
        assert_eq!(read_session_cookie(&h, &plain()), None);
    }

    #[test]
    fn strip_keeps_other_cookies_and_removes_both_names() {
        let mut h = with_cookies(&["a=1; hookbox_key=K1; b=2", "__Host-hookbox_key=K2; c=3=x"]);
        strip_session_cookie(&mut h);
        let vals: Vec<&str> = h
            .get_all(COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["a=1; b=2", "c=3=x"]);
    }

    #[test]
    fn strip_drops_a_header_left_empty() {
        let mut h = with_cookies(&["hookbox_key=K1", "other=1"]);
        strip_session_cookie(&mut h);
        let vals: Vec<&str> = h
            .get_all(COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["other=1"]);

        let mut only = with_cookies(&["hookbox_key=K1"]);
        strip_session_cookie(&mut only);
        assert!(only.get(COOKIE).is_none());
    }

    #[test]
    fn strip_handles_flag_style_and_empty_pairs() {
        let mut h = with_cookies(&["flag; ; hookbox_key=K; z="]);
        strip_session_cookie(&mut h);
        assert_eq!(h.get(COOKIE).unwrap(), "flag; z=");
    }

    #[test]
    fn is_session_cookie_matches_either_name_case_insensitively() {
        assert!(is_session_cookie("hookbox_key=x; Path=/"));
        assert!(is_session_cookie(" __Host-hookbox_key=; Max-Age=0"));
        assert!(is_session_cookie("__host-HOOKBOX_KEY=x"));
        assert!(is_session_cookie("hookbox_key"));
        assert!(!is_session_cookie("hookbox_key2=x"));
        assert!(!is_session_cookie("foo=hookbox_key"));
    }
}
