//! Mock-plane query strings.

use std::collections::BTreeMap;

use axum::http::Uri;

/// Parse the raw query string into a key→value map: `application/x-www-form-
/// urlencoded` decoding (`+` → space, `%XX` → byte, lossy UTF-8; a malformed
/// escape is kept literally), empty pairs skipped, a bare key maps to `""`,
/// and the LAST value of a repeated key wins (the Python
/// `dict(request.query_params)`). Pinned by `router::tests::parse_query_oracle`.
pub(crate) fn parse_query(uri: &Uri) -> BTreeMap<String, String> {
    uri.query()
        .map(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_default()
}
