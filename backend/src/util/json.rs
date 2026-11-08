//! JSON helpers: stored-column readers and the jsonpath-lite body getter.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Parse an optional stored JSON TEXT column, falling back to `fallback` when
/// the column is NULL, empty, or not valid JSON for `T`. Never errors — a
/// corrupt column degrades to the documented empty shape instead of failing
/// the whole read.
pub(crate) fn parse_json_or<T: DeserializeOwned>(raw: Option<&str>, fallback: T) -> T {
    match raw {
        Some(text) if !text.is_empty() => serde_json::from_str(text).unwrap_or(fallback),
        _ => fallback,
    }
}

/// Dot-path getter into a JSON body (`a.b.0.c`), §5.7 / AC-21. PORT of
/// `helpers.py::jsonpath_lite`. `body` is the raw JSON string; returns the value
/// as a string, or `None` on any miss / non-JSON / out-of-range index. Never
/// panics.
pub(crate) fn jsonpath_lite(body: &str, path: &str) -> Option<String> {
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
fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_valid_json() {
        let v: Value = parse_json_or(Some(r#"{"a":1}"#), json!({}));
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn null_empty_or_corrupt_falls_back() {
        assert_eq!(parse_json_or(None, json!([])), json!([]));
        assert_eq!(parse_json_or(Some(""), json!({})), json!({}));
        assert_eq!(parse_json_or(Some("{oops"), json!({})), json!({}));
    }

    #[test]
    fn typed_target_falls_back_on_shape_mismatch() {
        let items: Vec<Value> = parse_json_or(Some(r#"{"not":"an array"}"#), Vec::new());
        assert!(items.is_empty());
        let items: Vec<Value> = parse_json_or(Some("[1,2]"), Vec::new());
        assert_eq!(items, vec![json!(1), json!(2)]);
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
}
