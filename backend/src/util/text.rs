//! UTF-8-safe text helpers for persisted bodies.

/// Cut `s` to at most `cap` bytes, floored to a UTF-8 character boundary.
/// Replaces a panicking bare `s[..cap]` slice (R12): `is_char_boundary` is
/// O(1) and the loop backs off at most 3 bytes (F7, AC-70(b)/(d)).
pub(crate) fn truncate_utf8(s: &str, cap: usize) -> &str {
    if s.len() <= cap {
        return s;
    }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The persisted projection shared by BOTH `request_body` and `response_body`
/// (F7, AC-70(d)). Empty (before OR after truncation) ⇒ `None`, never an
/// empty string (AC-69). Wire bytes are lossy-UTF-8 decoded — the same
/// conversion the request path already uses — then cut per `truncate_utf8`,
/// with no marker and no flag (AC-70).
pub(crate) fn body_for_trace(bytes: &[u8], cap: usize) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let decoded = String::from_utf8_lossy(bytes); // Cow::Borrowed for valid UTF-8: no copy
    let cut = truncate_utf8(&decoded, cap);
    if cut.is_empty() {
        None
    } else {
        Some(cut.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // F7: truncate_utf8 (AC-70(b)) and body_for_trace (AC-69).

    #[test]
    fn truncate_utf8_floors_to_char_boundary_no_panic() {
        // Under the cap: untouched.
        assert_eq!(truncate_utf8("hello", 10), "hello");
        // cap = 0: always empty, never panics.
        assert_eq!(truncate_utf8("hello", 0), "");
        // Exactly at the cap: untouched.
        assert_eq!(truncate_utf8("hello", 5), "hello");
        // Multi-byte char straddling the cap: backs off to the last whole
        // char, never slices mid-codepoint (which would panic).
        let s = "a€b"; // 'a' (1 byte) + '€' (3 bytes) + 'b' (1 byte) = 5 bytes
        assert_eq!(truncate_utf8(s, 2), "a"); // cap lands inside '€' -> back off
        assert_eq!(truncate_utf8(s, 4), "a€"); // cap lands exactly on the boundary
        assert_eq!(truncate_utf8(s, 5), "a€b");
    }

    #[test]
    fn body_for_trace_empty_before_or_after_truncation_is_none_never_some_empty() {
        assert_eq!(body_for_trace(&[], 100), None);
        // A cap of 0 truncates any non-empty body down to "" -> still None,
        // never Some("") (AC-69's "never an empty string" rule, even under a
        // pathological MAX_BODY_BYTES=0).
        assert_eq!(body_for_trace(b"hello", 0), None);
        assert_eq!(body_for_trace(b"hello", 100), Some("hello".to_string()));
        // Lossy UTF-8 decode of invalid bytes never panics.
        let lossy = body_for_trace(&[0x80, 0xFF, 0xFE], 100).unwrap();
        assert!(lossy.contains('\u{FFFD}'));
    }
}
