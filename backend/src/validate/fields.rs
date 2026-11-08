//! Endpoint / rule field validators, shared by the create routes (typed JSON
//! bodies) and the PATCH routes (`PatchObject`), so both enforce the same
//! rules with the same 422 `detail` strings.

use std::ops::RangeInclusive;

use crate::error::ApiError;
use crate::models::{ChaosMode, DefaultMode, ResponseSpec};

/// Longest endpoint name (characters).
const ENDPOINT_NAME_MAX_CHARS: usize = 100;
/// Longest rule name (characters).
const RULE_NAME_MAX_CHARS: usize = 120;
/// Accepted rule priorities (lower runs first).
const PRIORITY_RANGE: RangeInclusive<i64> = 0..=100_000;
/// Accepted response status codes.
const STATUS_RANGE: RangeInclusive<i64> = 100..=599;

/// `name` on a PATCH body that is neither a string nor null.
pub(crate) const NAME_TYPE_MSG: &str = "name must be a string or null.";
pub(crate) const CHAOS_MODE_MSG: &str = "chaos_mode must be 'error' or 'dropout'.";
pub(crate) const DEFAULT_MODE_MSG: &str = "default_mode must be 'mock_404' or 'echo'.";

fn max_chars(value: Option<&str>, max: usize, detail: &str) -> Result<(), ApiError> {
    match value {
        Some(v) if v.chars().count() > max => Err(ApiError::validation(detail)),
        _ => Ok(()),
    }
}

pub(crate) fn endpoint_name(name: Option<&str>) -> Result<(), ApiError> {
    let detail = "name must be at most 100 characters.";
    max_chars(name, ENDPOINT_NAME_MAX_CHARS, detail)
}

pub(crate) fn rule_name(name: Option<&str>) -> Result<(), ApiError> {
    let detail = "name must be at most 120 characters.";
    max_chars(name, RULE_NAME_MAX_CHARS, detail)
}

pub(crate) fn priority(priority: i64) -> Result<(), ApiError> {
    if PRIORITY_RANGE.contains(&priority) {
        Ok(())
    } else {
        Err(ApiError::validation(
            "priority must be between 0 and 100000.",
        ))
    }
}

/// Status code range first, then the body-template size cap.
pub(crate) fn response_spec(spec: &ResponseSpec, template_max_size: usize) -> Result<(), ApiError> {
    if !STATUS_RANGE.contains(&spec.status_code) {
        return Err(ApiError::validation(
            "response.status_code must be 100..599.",
        ));
    }
    if spec.body_template.len() > template_max_size {
        return Err(ApiError::validation("response.body_template is too large."));
    }
    Ok(())
}

/// Exactly one of the `ChaosMode` wire strings (case-sensitive).
pub(crate) fn chaos_mode(mode: &str) -> Result<(), ApiError> {
    mode.parse::<ChaosMode>()
        .map(drop)
        .map_err(|_| ApiError::validation(CHAOS_MODE_MSG))
}

/// Exactly one of the `DefaultMode` wire strings (case-sensitive).
pub(crate) fn default_mode(mode: &str) -> Result<(), ApiError> {
    mode.parse::<DefaultMode>()
        .map(drop)
        .map_err(|_| ApiError::validation(DEFAULT_MODE_MSG))
}

/// Clamp an optional integer knob into `[0, max]`.
pub(crate) fn clamp_to(value: Option<i64>, max: i64) -> Option<i64> {
    value.map(|n| n.clamp(0, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_capped_by_characters_not_bytes() {
        assert!(endpoint_name(Some(&"é".repeat(100))).is_ok());
        assert!(endpoint_name(Some(&"x".repeat(101))).is_err());
        assert!(endpoint_name(None).is_ok());
        assert!(rule_name(Some(&"x".repeat(120))).is_ok());
        let err = rule_name(Some(&"x".repeat(121))).unwrap_err();
        assert_eq!(err.detail, "name must be at most 120 characters.");
    }

    #[test]
    fn priority_and_response_bounds() {
        assert!(priority(0).is_ok() && priority(100_000).is_ok());
        assert!(priority(-1).is_err() && priority(100_001).is_err());
        let spec = ResponseSpec {
            status_code: 99,
            ..ResponseSpec::default()
        };
        let err = response_spec(&spec, 10).unwrap_err();
        assert_eq!(err.detail, "response.status_code must be 100..599.");
        let spec = ResponseSpec {
            body_template: "x".repeat(11),
            ..ResponseSpec::default()
        };
        let err = response_spec(&spec, 10).unwrap_err();
        assert_eq!(err.detail, "response.body_template is too large.");
    }

    #[test]
    fn mode_sets_are_exact() {
        assert!(chaos_mode("error").is_ok() && chaos_mode("dropout").is_ok());
        assert!(chaos_mode("Error").is_err());
        assert!(default_mode("mock_404").is_ok() && default_mode("echo").is_ok());
        assert!(default_mode("mock404").is_err());
        assert_eq!(clamp_to(Some(-5), 10), Some(0));
        assert_eq!(clamp_to(Some(50), 10), Some(10));
        assert_eq!(clamp_to(None, 10), None);
    }
}
