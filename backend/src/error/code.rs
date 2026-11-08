//! `ErrorCode`: every machine-readable `error` string HookBox emits, on
//! both planes. The strings are frozen wire contract.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // management plane (P2)
    Unauthorized,
    CsrfRejected,
    InvalidKey,
    NotFound,
    ValidationError,
    InvalidCollection,
    RateLimited,
    StoreUnavailable,
    TooManyConnections,
    PayloadTooLarge,
    BadRequest,
    // mock plane (P1)
    UnknownEndpoint,
    EndpointGone,
    NoMatch,
    Chaos,
    NoTunnel,
    UpstreamTimeout,
    UpstreamUnreachable,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 18] = [
        ErrorCode::Unauthorized,
        ErrorCode::CsrfRejected,
        ErrorCode::InvalidKey,
        ErrorCode::NotFound,
        ErrorCode::ValidationError,
        ErrorCode::InvalidCollection,
        ErrorCode::RateLimited,
        ErrorCode::StoreUnavailable,
        ErrorCode::TooManyConnections,
        ErrorCode::PayloadTooLarge,
        ErrorCode::BadRequest,
        ErrorCode::UnknownEndpoint,
        ErrorCode::EndpointGone,
        ErrorCode::NoMatch,
        ErrorCode::Chaos,
        ErrorCode::NoTunnel,
        ErrorCode::UpstreamTimeout,
        ErrorCode::UpstreamUnreachable,
    ];

    /// The wire string (`{"error": "<this>"}`).
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Unauthorized => "unauthorized",
            ErrorCode::CsrfRejected => "csrf_rejected",
            ErrorCode::InvalidKey => "invalid_key",
            ErrorCode::NotFound => "not_found",
            ErrorCode::ValidationError => "validation_error",
            ErrorCode::InvalidCollection => "invalid_collection",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::StoreUnavailable => "store_unavailable",
            ErrorCode::TooManyConnections => "too_many_connections",
            ErrorCode::PayloadTooLarge => "payload_too_large",
            ErrorCode::BadRequest => "bad_request",
            ErrorCode::UnknownEndpoint => "unknown_endpoint",
            ErrorCode::EndpointGone => "endpoint_gone",
            ErrorCode::NoMatch => "no_match",
            ErrorCode::Chaos => "chaos",
            ErrorCode::NoTunnel => "no_tunnel",
            ErrorCode::UpstreamTimeout => "upstream_timeout",
            ErrorCode::UpstreamUnreachable => "upstream_unreachable",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lets tests (and callers) compare a code to its wire string directly.
impl PartialEq<&str> for ErrorCode {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_strings_are_unique_snake_case() {
        let mut seen: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        assert!(seen
            .iter()
            .all(|s| s.chars().all(|c| c.is_ascii_lowercase() || c == '_')));
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), ErrorCode::ALL.len());
        assert_eq!(ErrorCode::EndpointGone.to_string(), "endpoint_gone");
        assert_eq!(ErrorCode::NoTunnel, "no_tunnel");
    }
}
