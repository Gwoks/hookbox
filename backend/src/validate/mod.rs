//! Field validators shared by the management routes and the mock-plane
//! stores, plus the `PatchObject` reader the PATCH routes parse through.

use crate::error::ApiError;

mod fields;
mod patch;

pub(crate) use fields::{
    chaos_mode, clamp_to, default_mode, endpoint_name, priority, response_spec, rule_name,
    CHAOS_MODE_MSG, DEFAULT_MODE_MSG, NAME_TYPE_MSG,
};
pub(crate) use patch::PatchObject;

/// `limit` when the caller sends none.
const DEFAULT_PAGE_LIMIT: i64 = 50;
/// Largest accepted `limit`.
const MAX_PAGE_LIMIT: i64 = 200;

/// A validated `?limit=&offset=` page of a trace list (owner #12 and the
/// public share feed): `limit` defaults to 50 and must be 1..=200; `offset`
/// defaults to 0 and must be >= 0. The 422 details are part of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Paging {
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Paging {
    pub(crate) fn new(limit: Option<i64>, offset: Option<i64>) -> Result<Self, ApiError> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
        let offset = offset.unwrap_or(0);
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(ApiError::validation("limit must be between 1 and 200."));
        }
        if offset < 0 {
            return Err(ApiError::validation("offset must be >= 0."));
        }
        Ok(Paging { limit, offset })
    }
}

/// An Auto-CRUD collection name that passed `is_safe_key` (#17/#18); anything
/// else is `422 invalid_collection`.
#[derive(Debug)]
pub(crate) struct CollectionName(String);

impl CollectionName {
    pub(crate) fn parse(raw: String) -> Result<Self, ApiError> {
        if is_safe_key(&raw) {
            Ok(CollectionName(raw))
        } else {
            Err(ApiError::invalid_collection())
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

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

/// Longest accepted share-link label (characters, after trimming).
const SHARE_LABEL_MAX_CHARS: usize = 80;

/// A share-link label: trimmed; blank (or absent) is `None`.
pub(crate) fn share_label(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(t) if t.chars().count() > SHARE_LABEL_MAX_CHARS => {
            Err(ApiError::validation("label must be at most 80 characters."))
        }
        Some(t) => Ok(Some(t.to_string())),
    }
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
    use axum::http::StatusCode;

    #[test]
    fn paging_defaults_and_bounds() {
        assert_eq!(
            Paging::new(None, None).unwrap(),
            Paging {
                limit: 50,
                offset: 0
            }
        );
        assert_eq!(Paging::new(Some(1), Some(0)).unwrap().limit, 1);
        assert_eq!(Paging::new(Some(200), Some(7)).unwrap().offset, 7);
        let too_big = Paging::new(Some(201), None).unwrap_err();
        assert_eq!(too_big.detail, "limit must be between 1 and 200.");
        assert!(Paging::new(Some(0), None).is_err());
        let negative = Paging::new(None, Some(-1)).unwrap_err();
        assert_eq!(negative.detail, "offset must be >= 0.");
        assert_eq!(negative.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn collection_name_parse() {
        assert_eq!(
            CollectionName::parse("widgets".into()).unwrap().as_str(),
            "widgets"
        );
        let err = CollectionName::parse("bad name".into()).unwrap_err();
        assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code, "invalid_collection");
    }

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
    fn share_label_trims_blanks_and_caps() {
        assert_eq!(share_label(None).unwrap(), None);
        assert_eq!(share_label(Some("   ")).unwrap(), None);
        assert_eq!(share_label(Some(" hi ")).unwrap(), Some("hi".to_string()));
        assert!(share_label(Some(&"x".repeat(80))).is_ok());
        let err = share_label(Some(&"x".repeat(81))).unwrap_err();
        assert_eq!(err.detail, "label must be at most 80 characters.");
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
