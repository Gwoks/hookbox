//! Small shared helpers — PORT of `app/utils/helpers.py` (the subset needed by
//! the management routes; the templating/MITM tasks extend this module).

use std::collections::BTreeMap;

use serde_json::Value;

/// Sensitive inbound headers that must never be forwarded upstream (AC-S9) on top
/// of the hop-by-hop set. The owner capability rides `Authorization` to the
/// management plane only; it is dropped here so it never reaches an MITM target.
pub const SENSITIVE_FORWARD_HEADERS: [&str; 5] =
    ["authorization", "cookie", "x-owner-id", "x-user-id", "x-hookbox-cap"];

/// Hop-by-hop headers (RFC 7230 §6.1) — never forwarded/echoed upstream.
pub const HOP_BY_HOP: [&str; 9] = [
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailers", "transfer-encoding", "upgrade", "host",
];

/// Owner-capability / cookie headers redacted before a trace is persisted OR
/// published to the feed (AC-S15 / engine `_redact`). Case-insensitive names.
pub const REDACT_HEADERS: [&str; 3] = ["authorization", "cookie", "x-owner-id"];

/// Redact sensitive headers (lower-cased names) before persisting/publishing a
/// trace. PORT of `engine.py::_redact`.
pub fn redact(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            if REDACT_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
                (k.clone(), "<redacted>".to_string())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}

/// Headers safe to forward to an MITM upstream: hop-by-hop + sensitive removed
/// (AC-S9). PORT of `helpers.py::strip_forward_headers`.
pub fn strip_forward_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter(|(k, _)| {
            let kl = k.to_ascii_lowercase();
            !HOP_BY_HOP.contains(&kl.as_str()) && !SENSITIVE_FORWARD_HEADERS.contains(&kl.as_str())
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Dot-path getter into a JSON body (`a.b.0.c`), §5.7 / AC-21. PORT of
/// `helpers.py::jsonpath_lite`. `body` is the raw JSON string; returns the value
/// as a string, or `None` on any miss / non-JSON / out-of-range index. Never
/// panics.
pub fn jsonpath_lite(body: &str, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(body).ok()?;
    let mut cur = &parsed;
    for seg in path.split('.') {
        match cur {
            Value::Object(map) => {
                cur = map.get(seg)?;
            }
            Value::Array(arr) => {
                // Index must be a (possibly negative-signed) integer.
                let trimmed = seg.strip_prefix('-').unwrap_or(seg);
                if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let idx: i64 = seg.parse().ok()?;
                if idx < 0 || (idx as usize) >= arr.len() {
                    return None;
                }
                cur = &arr[idx as usize];
            }
            _ => return None,
        }
    }
    Some(value_to_string(cur))
}

/// Stringify a JSON value the way jsonpath-lite / templating expects: `null`→"",
/// bool→"true"/"false", numbers/strings→their text, objects/arrays→compact JSON.
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "true".into() } else { "false".into() },
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// `^[A-Za-z0-9_-]{1,64}$` — the safe charset for state keys and collection
/// names (PORT of `is_safe_key`). Used for #17/#18 collection-name validation
/// and before any `endpoint_state` write.
pub fn is_safe_key(key: &str) -> bool {
    let len = key.chars().count();
    if !(1..=64).contains(&len) {
        return false;
    }
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Validate a MITM `target_url`: http(s) scheme with a host, else error
/// (AC-S6). `""`/`None` clears the target (returns `Ok(None)`). PORT of
/// `models.py::_validate_target_url`.
pub fn validate_target_url(v: Option<&str>) -> Result<Option<String>, String> {
    let raw = match v {
        None => return Ok(None),
        Some(s) => s.trim(),
    };
    if raw.is_empty() {
        return Ok(None);
    }
    let parsed = url::Url::parse(raw)
        .map_err(|_| "target_url must use the http or https scheme".to_string())?;
    let scheme = parsed.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("target_url must use the http or https scheme".to_string());
    }
    if parsed.host_str().map(|h| h.is_empty()).unwrap_or(true) {
        return Err("target_url must include a host".to_string());
    }
    Ok(Some(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_key_charset() {
        assert!(is_safe_key("users"));
        assert!(is_safe_key("a_b-C9"));
        assert!(is_safe_key(&"x".repeat(64)));
        assert!(!is_safe_key(""));
        assert!(!is_safe_key(&"x".repeat(65)));
        assert!(!is_safe_key("has space"));
        assert!(!is_safe_key("dots.here"));
        assert!(!is_safe_key("slash/here"));
    }

    #[test]
    fn jsonpath_lite_resolves_and_misses() {
        let body = r#"{"a":{"b":[10,20,{"c":"deep"}]},"flag":true,"n":5,"z":null}"#;
        assert_eq!(jsonpath_lite(body, "a.b.0"), Some("10".into()));
        assert_eq!(jsonpath_lite(body, "a.b.2.c"), Some("deep".into()));
        assert_eq!(jsonpath_lite(body, "flag"), Some("true".into()));
        assert_eq!(jsonpath_lite(body, "n"), Some("5".into()));
        assert_eq!(jsonpath_lite(body, "z"), Some("".into())); // null -> ""
        assert_eq!(jsonpath_lite(body, "a.b.9"), None); // out of range
        assert_eq!(jsonpath_lite(body, "nope"), None);
        assert_eq!(jsonpath_lite("not json", "a"), None);
        assert_eq!(jsonpath_lite(body, ""), None);
    }

    #[test]
    fn redact_masks_sensitive_headers() {
        let mut h = std::collections::BTreeMap::new();
        h.insert("authorization".to_string(), "Bearer secret".to_string());
        h.insert("cookie".to_string(), "sid=1".to_string());
        h.insert("x-owner-id".to_string(), "abc".to_string());
        h.insert("accept".to_string(), "application/json".to_string());
        let out = redact(&h);
        assert_eq!(out["authorization"], "<redacted>");
        assert_eq!(out["cookie"], "<redacted>");
        assert_eq!(out["x-owner-id"], "<redacted>");
        assert_eq!(out["accept"], "application/json");
    }

    #[test]
    fn strip_forward_drops_sensitive_and_hop_by_hop() {
        let mut h = std::collections::BTreeMap::new();
        h.insert("authorization".to_string(), "Bearer x".to_string());
        h.insert("host".to_string(), "victim".to_string());
        h.insert("x-hookbox-cap".to_string(), "cap".to_string());
        h.insert("accept".to_string(), "*/*".to_string());
        let out = strip_forward_headers(&h);
        assert!(!out.contains_key("authorization"));
        assert!(!out.contains_key("host"));
        assert!(!out.contains_key("x-hookbox-cap"));
        assert_eq!(out["accept"], "*/*");
    }

    #[test]
    fn target_url_scheme_allow_list() {
        assert_eq!(validate_target_url(Some("https://api.example.com")).unwrap(),
                   Some("https://api.example.com".to_string()));
        assert_eq!(validate_target_url(Some("http://h/x")).unwrap(),
                   Some("http://h/x".to_string()));
        // "" / None clears.
        assert_eq!(validate_target_url(Some("  ")).unwrap(), None);
        assert_eq!(validate_target_url(None).unwrap(), None);
        // Bad scheme / missing host -> Err (422 at the route).
        assert!(validate_target_url(Some("ftp://h/x")).is_err());
        assert!(validate_target_url(Some("file:///etc/passwd")).is_err());
        assert!(validate_target_url(Some("not a url")).is_err());
    }
}
