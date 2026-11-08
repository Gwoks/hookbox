//! Timestamp formatting shared by every wire shape that carries a time.

/// Normalize a stored TEXT timestamp to an RFC3339 string. SQLite
/// `datetime('now')` yields `YYYY-MM-DD HH:MM:SS` (UTC, no zone); the contract
/// is RFC3339 UTC, so we present the `T`-separated `...Z` form. A value that
/// already contains a `T` is passed through; blank/absent is `None`.
pub(crate) fn to_rfc3339(value: Option<String>) -> Option<String> {
    let v = value?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    if v.contains('T') {
        return Some(v.to_string());
    }
    // "2026-06-21 12:00:00" -> "2026-06-21T12:00:00Z"
    Some(format!("{}Z", v.replacen(' ', "T", 1)))
}

/// The current UTC instant as RFC3339 with microseconds and a `Z` suffix —
/// the format of every server-generated live timestamp (feed `hello`,
/// `new_request` summaries, the `{{now 'iso'}}` template tag).
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_datetime_becomes_rfc3339_utc() {
        assert_eq!(
            to_rfc3339(Some("2026-06-21 12:00:00".into())),
            Some("2026-06-21T12:00:00Z".into())
        );
    }

    #[test]
    fn already_rfc3339_passes_through_trimmed() {
        assert_eq!(
            to_rfc3339(Some(" 2026-06-21T12:00:00Z ".into())),
            Some("2026-06-21T12:00:00Z".into())
        );
    }

    #[test]
    fn blank_or_absent_is_none() {
        assert_eq!(to_rfc3339(None), None);
        assert_eq!(to_rfc3339(Some("   ".into())), None);
    }

    #[test]
    fn now_iso_is_utc_with_microseconds() {
        let now = now_iso();
        assert!(now.ends_with('Z'), "{now}");
        // `YYYY-MM-DDTHH:MM:SS.ffffffZ` = 27 chars.
        assert_eq!(now.len(), 27, "{now}");
    }
}
