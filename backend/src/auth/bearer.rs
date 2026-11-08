//! `Authorization: Bearer <key>` parsing, shared by owner resolution (which
//! maps `None` to 401), the live-feed handshake and the tunnel bind
//! handshake (Bearer-only).

/// The raw secret from a `Bearer <secret>` header value: the scheme matches
/// case-insensitively and the secret is trimmed. `None` for any other scheme
/// or a blank secret.
pub(crate) fn parse_bearer(header: &str) -> Option<&str> {
    let mut parts = header.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    let secret = parts.next().unwrap_or("").trim();
    if scheme.eq_ignore_ascii_case("bearer") && !secret.is_empty() {
        Some(secret)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bearer_case_insensitively_and_trims() {
        assert_eq!(parse_bearer("Bearer abc"), Some("abc"));
        assert_eq!(parse_bearer("bearer   abc  "), Some("abc"));
        assert_eq!(parse_bearer("BEARER abc"), Some("abc"));
    }

    #[test]
    fn rejects_other_schemes_and_blank_secrets() {
        assert_eq!(parse_bearer("Basic abc"), None);
        assert_eq!(parse_bearer("Bearer"), None);
        assert_eq!(parse_bearer("Bearer    "), None);
        assert_eq!(parse_bearer(""), None);
    }
}
