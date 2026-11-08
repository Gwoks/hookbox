//! `Authorization: Bearer <secret>` parsing.

use crate::error::ApiError;

/// Parse a `Bearer <secret>` header, returning the raw secret or 401.
pub(super) fn parse_bearer(authorization: Option<&str>) -> Result<String, ApiError> {
    let unauthorized = || ApiError::unauthorized("Valid owner capability required.");
    let header = authorization.ok_or_else(unauthorized)?;
    let mut parts = header.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    let token = parts.next().unwrap_or("").trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(unauthorized());
    }
    Ok(token.to_string())
}
