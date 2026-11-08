//! Field validators shared by the management routes and the mock-plane stores.

/// `^[A-Za-z0-9_-]{1,64}$` — the safe charset for state keys and collection
/// names (PORT of `is_safe_key`). Used for #17/#18 collection-name validation,
/// Auto-CRUD path segments, and before any `endpoint_state` write.
pub(crate) fn is_safe_key(key: &str) -> bool {
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
pub(crate) fn validate_target_url(v: Option<&str>) -> Result<Option<String>, String> {
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
    fn target_url_scheme_allow_list() {
        assert_eq!(
            validate_target_url(Some("https://api.example.com")).unwrap(),
            Some("https://api.example.com".to_string())
        );
        assert_eq!(
            validate_target_url(Some("http://h/x")).unwrap(),
            Some("http://h/x".to_string())
        );
        // "" / None clears.
        assert_eq!(validate_target_url(Some("  ")).unwrap(), None);
        assert_eq!(validate_target_url(None).unwrap(), None);
        // Bad scheme / missing host -> Err (422 at the route).
        assert!(validate_target_url(Some("ftp://h/x")).is_err());
        assert!(validate_target_url(Some("file:///etc/passwd")).is_err());
        assert!(validate_target_url(Some("not a url")).is_err());
    }
}
