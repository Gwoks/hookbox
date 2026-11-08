//! F4 share links — the whole unauthenticated attack surface in one module.
//!
//! Two trust boundaries, five routes:
//!   * Owner-authenticated (§5.1): mint / list / revoke, under
//!     `/api/endpoints/{token}/shares[/{id}]`, gated by the `OwnerId`
//!     extractor exactly like every other management route.
//!   * PUBLIC, no authentication at all (§5.2): `/api/share/{code}/requests`
//!     and `/api/share/{code}/requests/{id}`. Together with `POST
//!     /api/session` these are the ONLY unauthenticated routes in HookBox.
//!
//! **Invariants that make the public half safe:**
//!   * **No mutation.** Both public handlers only ever `SELECT` (plus one
//!     fire-and-forget `last_used_at` touch off the response path). Nothing a
//!     viewer can send changes any row a viewer can read.
//!   * **No owner secret, no token, ever.** The public projections
//!     (`PublicRequestSummary`/`PublicRequestDetail`/`PublicShareFeed`) are
//!     STANDALONE structs built field-by-field in this module — never
//!     `#[serde(flatten)]`ed off an owner struct, never `#[serde(skip)]`ed off
//!     `RequestDetail` — so a future owner-shape field cannot leak here by
//!     default (AC-34, AC-102). This is a LIVE-CAPABILITY boundary, not just a
//!     naming one: recovering the token from a share link hands a viewer
//!     write access to `/e/<token>` (AC-S2, AC-43, hookbox-mun.34), so
//!     `filter_public_request_headers`/`filter_public_response_headers` also
//!     strip the structural `host`/`origin`/`referer` request headers (the
//!     wildcard mock host IS `<token>.<MOCK_DOMAIN>`) and mask any remaining
//!     header value that contains the resolved token, e.g. a CORS
//!     `access-control-allow-origin` echo of that same wildcard `Origin`.
//!     `response_body` itself is passed through as stored and NOT re-filtered
//!     here (§5.11) — the third channel (hookbox-mun.36) was a
//!     `default_mode = "echo"` row's persisted `headers` sub-object carrying
//!     the same host/origin/referer verbatim; that is closed on the persist
//!     path instead, in `interceptor::engine::redact_echo_persisted_headers`,
//!     so this module's projection never needs to know about body-shaped data.
//!   * **One 404 for every negative outcome.** `share_not_found()` is the
//!     ONLY 404 the public resolver may emit — unknown code, revoked code,
//!     tombstoned endpoint, unknown request id and cross-endpoint request id
//!     are byte-identical (AC-36), so a scanner learns nothing from the
//!     difference.
//!   * **The frozen check order is load-bearing.** Rate limit, THEN parameter
//!     validation, THEN code-shape, THEN code resolution — in that order, on
//!     every public request, before any other work (§5.2). Validating
//!     parameters before resolving the code is what stops `?limit=999` from
//!     being a live/dead code oracle (AC-101).
//!   * **The code never appears in a log.** No `tracing::*` call in this module
//!     receives the code or the full path; log `share_links.id` instead
//!     (AC-S6, AC-S16).

mod owner;
mod projection;
mod public;

pub(crate) use owner::{create_share, list_shares, revoke_share};
pub(crate) use public::{public_get_request, public_list_requests};

// Every handler-produced response on the public routes carries
// `Cache-Control: no-store` (AC-37), including a 503 from a bubbled
// `sqlx::Error`, which is why it wraps the *outermost* Result rather than
// being attached per branch.
use crate::net::headers::no_store;
