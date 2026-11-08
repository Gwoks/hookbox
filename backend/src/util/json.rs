//! JSON helpers shared by the stored-column readers.

use serde::de::DeserializeOwned;

/// Parse an optional stored JSON TEXT column, falling back to `fallback` when
/// the column is NULL, empty, or not valid JSON for `T`. Never errors — a
/// corrupt column degrades to the documented empty shape instead of failing
/// the whole read.
pub(crate) fn parse_json_or<T: DeserializeOwned>(raw: Option<String>, fallback: T) -> T {
    match raw {
        Some(text) if !text.is_empty() => serde_json::from_str(&text).unwrap_or(fallback),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn parses_valid_json() {
        let v: Value = parse_json_or(Some(r#"{"a":1}"#.into()), json!({}));
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn null_empty_or_corrupt_falls_back() {
        assert_eq!(parse_json_or(None, json!([])), json!([]));
        assert_eq!(parse_json_or(Some(String::new()), json!({})), json!({}));
        assert_eq!(parse_json_or(Some("{oops".into()), json!({})), json!({}));
    }

    #[test]
    fn typed_target_falls_back_on_shape_mismatch() {
        let items: Vec<Value> = parse_json_or(Some(r#"{"not":"an array"}"#.into()), Vec::new());
        assert!(items.is_empty());
        let items: Vec<Value> = parse_json_or(Some("[1,2]".into()), Vec::new());
        assert_eq!(items, vec![json!(1), json!(2)]);
    }
}
