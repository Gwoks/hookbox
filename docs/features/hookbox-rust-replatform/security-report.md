# Security review (REVIEW): HookBox — Rust/Axum + SQLite Re-platform

- **Slug:** `hookbox-rust-replatform`
- **Mode:** REVIEW (Stage B — review the implemented code; runs after QA passed)
- **Date:** 2026-06-22
- **Gate:** `hookbox-sks.35` (security review gate — code-level)
- **Verdict:** **PASS — no unresolved security findings.** Every design-time threat
  (F1–F13) and every required AC (AC-S1…AC-S19) is satisfied with file:line
  evidence below. No bd bugs filed.
- **Inputs:** `security.md` (F1–F13, AC-S1…S19), `prd.md` §4/§5 (frozen contract),
  and the implemented backend (`backend/src/**`) + frontend (`src/**`).

> Method: static review (grep + read) of the actual code. No long-running server
> was started. The 19 AC-S claims were each traced to concrete code with the
> exploit/why-safe reasoning shown.

---

## AC-S verification (file:line evidence)

| AC | Verdict | Evidence |
|----|---------|----------|
| **AC-S1** SSRF on every resolved IP incl. `169.254.169.254`, IPv4/IPv6, `::ffff:` | **PASS** | `ssrf.rs:19-59` classifies loopback/private/link-local/multicast/broadcast/unspecified/documentation/CGNAT/`0.0.0.0/8` + the metadata constant `ssrf.rs:17`; v6 `ssrf.rs:33-51` maps `::ffff:` → embedded v4, plus `fc00::/7` + `fe80::/10`. `resolve_and_check` `ssrf.rs:65-90` rejects if **any** resolved addr is blocked. Engine maps the block to `502 upstream_unreachable` (`engine.rs:276-279`). |
| **AC-S2** Pin to validated IP, no second resolution, preserve Host+SNI | **PASS** | `proxy.rs:114-123`: `resolve_and_check` → `validated[0]` → `reqwest::Client::builder().resolve(&host, pinned)` installs a static DNS override so reqwest does not re-resolve at connect; the original `host` is preserved on the request URL (`proxy.rs:128`) so Host + TLS SNI bind to the original name. Rebinding (public-then-private) cannot reach the private addr. |
| **AC-S3** Redirects off by default; per-hop re-validation; bounded | **PASS** | `mitm_follow_redirects` defaults **false** (`config.rs:164`); `redirects_allowed=0` unless enabled (`proxy.rs:97`); `redirect::Policy::none()` (`proxy.rs:121`); manual follow loop joins the new URL and `continue`s to the top where `resolve_and_check` re-runs on the new host (`proxy.rs:144-158, 106-117`); bounded by `redirects_left` from `mitm_max_redirects`. |
| **AC-S4** Strip sensitive/hop-by-hop before forward; strip framing/Set-Cookie/CORS from response | **PASS** | `helpers.rs:11-18` (`SENSITIVE_FORWARD_HEADERS` incl. `authorization`,`cookie`,`x-owner-id`,`x-user-id`,`x-hookbox-cap`; `HOP_BY_HOP` incl. `host`); `strip_forward_headers` `helpers.rs:41-50` applied at `proxy.rs:98`. Response strip list `proxy.rs:27-33` (set-cookie, transfer-encoding, content-length/-encoding, ACAO/ACAC/expose/max-age/vary) applied `proxy.rs:74-80`. |
| **AC-S5** No SSTI; hand-written scanner; probes verbatim | **PASS** | `templating.rs` is a single-pass byte scanner (`render` `templating.rs:187-245`); resolution is a closed `match` over fixed verbs (`resolve_tag` `templating.rs:156-183`) — no eval, no format-over-user-text, **no template-engine crate** (Cargo grep: no minijinja/tera/handlebars/askama/liquid). Unknown tags → literal (`templating.rs:238`). Test `ssti_probes_returned_verbatim` `templating.rs:269-274`. |
| **AC-S6** Rotation invalidates old secret (overwrite) | **PASS** | Lookup is by `secret_hash` only (`auth.rs:42-48`); session rotation is an `UPDATE owners SET secret_hash` (overwrite, not append) — verified in test `rotation_invalidates_old_secret` `auth.rs:190-212`; seed uses `ON CONFLICT … DO UPDATE SET secret_hash` (`seed.rs:25-31`). |
| **AC-S7** `owner_id` is not a credential | **PASS** | Only `sha256(secret)` is looked up (`auth.rs:42-48`); test `owner_id_is_never_a_credential` `auth.rs:179-188` confirms presenting `owner_id` as Bearer → 401. |
| **AC-S8** 401 vs 404; WWW-Authenticate; uniform `{error,detail}` | **PASS** | Missing/malformed/unknown → `ApiError::unauthorized` 401 (`auth.rs:23-48`); non-owner of `{token}` → `ApiError::not_found` 404 (`assert_owns_endpoint` `auth.rs:55-69`, never 403); tests `auth.rs:154-250`, `router.rs:377-389`. `WWW-Authenticate: Bearer` is set by `ApiError::unauthorized` (error.rs). |
| **AC-S9** Feed owner-gate BEFORE any frame; channel isolation; WS 4401/SSE 401 | **PASS** | WS: `verify_cap_owns_token` runs before `on_upgrade`; inside, unauth → accept-then-close `4401` before subscribe (`feed.rs/routes`: `feed.rs:46-58`). SSE: cap verified before streaming → `401` (`feed.rs:121-123`). Channel isolation via per-token broadcast (`feed.rs:67-101`, test `feed.rs:122-135`). |
| **AC-S10** Tunnel bind auth before accept; only owner; 4401 | **PASS** | `tunnel_ws.rs:104-118`: bearer-or-`?cap=` → `verify_cap_owns_token(&slug,…)` runs before `on_upgrade`; unauth → close `4401` with no `bind()`/registration. |
| **AC-S11** No CORS on P2 `/api/**` | **PASS** | No `CorsLayer`/`tower_http::cors` anywhere (grep clean); `cors.rs` is only ever called from `engine::identified` on P1 (`engine.rs:340-342`). |
| **AC-S12** P1 never credentialed; reflect Origin / `*` only when absent | **PASS** | `cors.rs:26-60` reflects `Origin` (or `*` when absent) and **never** emits `Access-Control-Allow-Credentials`; test `never_emits_allow_credentials` `cors.rs:70-76`. |
| **AC-S13** Cap/Authorization/Cookie/X-Owner-Id redacted in traces+feed; never in logs incl `?cap=` | **PASS** | `redact` `helpers.rs:22-37` applied to request headers before persist AND feed (`engine.rs:404`). No `tower_http::trace`/access-log layer exists; only explicit `tracing::info/warn` strings (`main.rs`) — no header maps or query strings logged; `?cap=` never reaches a log sink. |
| **AC-S14** Cap only via Bearer/`?cap=`, never a cookie | **PASS** | Server reads cap only from `Authorization` (`auth.rs:124-127`) or `?cap=` (feed/tunnel). FE holds it in localStorage/memory, never a cookie, and sends `credentials: 'omit'` (`src/api/session.ts:4-16`, `src/api/client.ts:6-9,79-88`). CSRF closed. |
| **AC-S15** Ingest 413 before buffering | **PASS (with note)** | Content-Length checked before `collect()` → 413 (`router.rs:104-109`); post-buffer backstop `router.rs:114-116` and engine backstop `engine.rs:65-68`. **Note:** a chunked request with no Content-Length is buffered then rejected; it remains bounded by Axum's default 2 MB body limit (no `DefaultBodyLimit::disable` anywhere) so memory is bounded > the 1 MB target but never unbounded. Accepted (low/info). |
| **AC-S16** No crafted Host/path reaches `/api`/UI from a mock host | **PASS** | `planes.rs` resolves a mock host (incl. `/api`,`/static`) to P1 only (`planes.rs:126-131`); router short-circuits P1 to the interceptor and never enters the inner router (`router.rs:145-150`). Tests cover multi-label, app-host collision, percent-encoded label, `/api` under mock host (`planes.rs:181-271`, `router.rs:332-342`). |
| **AC-S17** Parameterized SQL only; charset enforced | **PASS** | All values are bound `?` params (sqlx). The two `format!` SQL builders (`api.rs:396`, `api.rs:654`) interpolate only **server-hard-coded** column fragments (`"chaos_mode = ?"` etc.); all user data flows through `bind_value` (`api.rs:413-430`). `is_safe_key` `^[A-Za-z0-9_-]{1,64}$` (`helpers.rs:100-107`) enforced for collection names → 422 (`api.rs` peek/clear), CRUD segments (`crud.rs:48-65`), and state writes (`engine.rs:140` Ok(false)=unsafe skipped). |
| **AC-S18** Bounded fail-open limiter | **PASS** | `limiter.rs`: in-memory `DashMap`, `MAX_BUCKETS=100_000` with idle eviction (`limiter.rs:38,81-83,117-143`); `limit<=0` ⇒ unlimited; the limiter returns *allowed* on the unlimited path and never wedges the request; over-limit → 429 with Retry-After (`engine.rs:166-179`). |
| **AC-S19** WS conn cap + bounded broadcast + lag-drop + send-timeout | **PASS** | Conn cap `ws_max_conn_per_endpoint` enforced (WS `1013` / SSE `503`) `feed.rs(routes):47-56,124-126`; broadcast channel bounded `CHANNEL_CAP=256` (`feed.rs:37,71`); lagged receiver drops + continues (`routes/feed.rs:106,147`); per-send `ws_send_timeout_s` drops slow clients (`routes/feed.rs:100-104`). |

**Tally: 19 / 19 PASS** (AC-S15 passes with an accepted low/info note on chunked-body buffering).

---

## Threat-surface checklist (F1–F13)

- **F1 SSRF (CRITICAL)** — COVERED. Guard on every resolved IP + IP-pin (no second
  resolution) + redirects-off-default with per-hop re-validation; `MITM_ALLOW_PRIVATE`
  defaults false. (`ssrf.rs`, `proxy.rs`)
- **F2 Plane isolation (HIGH)** — COVERED. P1 short-circuits before the inner router;
  mock host captures `/api`/`/static` as its own path; host-parse edge cases tested.
  (`planes.rs`, `router.rs`)
- **F3 Cap rotation & 401/404 (HIGH)** — COVERED. Rotate-overwrite, hash-only lookup,
  owner_id-not-a-credential, 401-vs-404. (`auth.rs`, `seed.rs`)
- **F4 WS/SSE + tunnel auth before frame (HIGH)** — COVERED. Cap verified before
  upgrade/registration; channel isolation; `?cap=` never logged. (`routes/feed.rs`,
  `routes/tunnel_ws.rs`)
- **F5 Templating SSTI (HIGH)** — COVERED. Hand-written scanner, no engine crate,
  unknown→literal. (`templating.rs`)
- **F6 DoS / no-Redis limits (HIGH)** — COVERED. Ingest 413, body truncation,
  write-time trace prune, bounded limiter map, bounded broadcast, per-send timeout.
  (`router.rs`, `limiter.rs`, `feed.rs`, `db.rs`)
- **F7 Sensitive-header stripping (HIGH)** — COVERED. Forward + response strip lists.
  (`helpers.rs`, `proxy.rs`)
- **F8 Secret never logged/reflected/persisted (HIGH)** — COVERED. Trace + feed
  redaction; no access-log layer; echo reflects only the *caller's own* headers back
  to that same caller (not a cross-party leak), and the echo body is not persisted.
  (`engine.rs`, `helpers.rs`, `main.rs`)
- **F9 Auto-CORS P1-only / never credentialed (MED→HIGH)** — COVERED. (`cors.rs`)
- **F10 SQLite injection & key safety (MED)** — COVERED. Parameterized SQL only;
  safe-key charset. (`api.rs`, `helpers.rs`, `crud.rs`)
- **F11 CSRF (LOW, accepted)** — COVERED. Bearer-only, cap never in a cookie,
  `credentials: 'omit'`. (`src/api/*`)
- **F12 410 tombstone integrity (LOW)** — COVERED. `gone_at` backs 410; indeterminate
  → 404; not logged as a trace. (`engine.rs:346-355`, `rule_cache`)
- **F13 Tunnel → operator localhost (LOW/INFO, accepted)** — COVERED. Owner-gated bind
  is the only control; forwarding public traffic to the operator's own localhost is the
  product. (`routes/tunnel_ws.rs`)

**Frontend:** XSS-inert — no `dangerouslySetInnerHTML`/`innerHTML`/`eval`/`new Function`
(grep clean); captured request/response data renders as React text nodes
(`json-tree.tsx`, `key-value-rows.tsx`). Cap held in localStorage/memory, never a
cookie; never placed in a logged URL except the `?cap=` feed query (which the server
does not log).

---

## Accepted (non-blocking) notes

- **N1 (low/info) — chunked ingest buffering.** A P1 request with no Content-Length is
  fully buffered before the size check; it is still bounded by Axum's default 2 MB body
  limit (no `DefaultBodyLimit::disable`), so memory cannot grow unbounded. The X-HookBox
  `413` envelope only fires for Content-Length-declared or post-buffer cases. Accepted:
  no unbounded-memory path; faithfulness-only gap, not a vulnerability.
- **N2 (info) — seed prints the secret to stderr.** `bin/seed.rs:25` prints the demo
  owner secret to **stderr** during a one-time operator-run seed (not into any HTTP
  response or server log). Equivalent to a generated-once admin credential. Accepted.
- **N3 (info) — feed channel map growth.** `FeedHub.channels` adds one entry per token
  and is never evicted, but tokens are owner-created (require a cap), so an anonymous
  attacker cannot grow it. Accepted.

These are explicitly accepted and do **not** block the gate.

---

## Conclusion

All 19 AC-S verified PASS against the implemented code; all design-time threats
F1–F13 are covered. **No exploitable vulnerability found; no bd bug filed; the gate
`hookbox-sks.35` is cleared.**

### Post-review hardening (2026-06-22)

The two low/info notes were additionally addressed (defense-in-depth; neither was
exploitable):
- **N1 — P1 ingest body bound.** `run_interceptor` now reads the mock body through
  `http_body_util::Limited(MAX_INGEST_BODY_BYTES)`, so a chunked request with no
  `Content-Length` is rejected with `413` before buffering past the cap (no longer
  relying on Axum's 2 MB default). `router.rs`.
- **N3 — feed channel map bound.** `FeedHub` now evicts a token's broadcast channel
  when its last subscriber drops (increment under the entry lock; `remove_if` to
  stay race-free), bounding the map to currently-live subscriptions. `feed.rs`
  (+ unit test `channel_is_evicted_when_last_subscriber_drops`).
