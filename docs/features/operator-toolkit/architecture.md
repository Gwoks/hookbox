# Architecture: Operator Toolkit — six operator affordances + one enabling backend fix (slug: `operator-toolkit`)

**Status:** AUTHORITATIVE on §5. §2 of this document *replaces* `prd.md` §5 verbatim on the PM's
REVISE pass. Where this document contradicts the PRD, this document wins and the contradiction is
listed explicitly in §0 so the PM can reconcile.

**Stack, grounded (not the generic template).** This repo is **Rust + axum 0.7.9 + sqlx 0.8/SQLite
(WAL) + tokio**, with a **React 18 + Vite + zod + react-router SPA**, served behind nginx in the
deploy target. There is no FastAPI, no `aiosqlite`, no pydantic and no Jinja anywhere in this
codebase — every contract below is expressed as `serde` structs ⇄ `zod` schemas over the existing
flat JSON envelope.

Verification legend (unchanged from the PRD):
- **[existing — verified at `path:line`]** — I opened the file and read that line range during this pass.
- **[new]** — does not exist yet.

---

## 0. What I verified, and the 14 places the PRD needs correcting

The PM flagged 8 unverified implementation details. All 8 are resolved below, plus 6 further
discrepancies I found while reading the code. **Items marked `MUST-FIX` change §5 shapes or ACs and
need the PM's REVISE pass.** Items marked `CLARIFY` are wording/precision fixes.

| # | PM question / finding | Verdict |
|---|---|---|
| **D1** | Is `axum::body::to_bytes` available? | **YES.** `axum = "0.7"` resolves to **0.7.9** **[existing — verified at `backend/Cargo.lock:90-92`]** and `pub async fn to_bytes(body: Body, limit: usize) -> Result<Bytes, axum_core::Error>` exists **[verified at `~/.cargo/registry/src/…/axum-0.7.9/src/body/mod.rs:48-54`]**. Bonus: `axum::body::HttpBody` is re-exported (`pub use http_body::Body as HttpBody;` **[verified at `axum-0.7.9/src/body/mod.rs:4`]**), so `body.size_hint()` is reachable **without adding `http-body` to `Cargo.toml`** — §5.10's "Cargo.toml untouched" survives. |
| **D2** | Does `identified()` ever rewrite a body? | **NO.** It only calls `insert_header` on `resp.headers_mut()` and returns the same `Response` **[existing — verified at `backend/src/interceptor/engine.rs:576-593`]**. Capturing the body **after** `identified()` is therefore safe and is what I specify (so the trace's `response_headers` still includes the `X-HookBox-*` set). |
| **D3** | `forward_to_tunnel` body shape + error/timeout path | Read in full **[existing — verified at `backend/src/routes/tunnel_ws.rs:35-98`]**. There is exactly **one** response-construction site (`:73-76`, `Body::from(body_bytes)` where `body_bytes` is the base64-decoded CLI reply at `:70-72`). Every failure path — no registered tunnel (`:44`), send failure (`:55-58`), timeout / dropped waiter (`:60-66`) — returns `Err(())` **before any Response exists**; the engine then builds its own `json_response(504, no_tunnel)` **[verified at `engine.rs:448-458`]**. **Consequence: mechanism (ii) would need a signature change here; mechanism (i) needs none.** |
| **D4** | `ProxyResponse.body` type / extra clone? | `pub body: Vec<u8>` **[existing — verified at `backend/src/interceptor/proxy.rs:47-52`]**, moved into `Body::from(pr.body)` **[verified at `engine.rs:478-481`]**. `Bytes::from(Vec<u8>)` takes ownership (O(1)); `to_bytes` on a single-frame body hits `BufList::copy_to_bytes`'s `front.remaining() == len` fast path, which is `Bytes::split_to` = a refcount bump **[verified at `http-body-util-0.1.3/src/util.rs:74-83` and `src/collected.rs:37-39`]**. **So capture needs zero extra full-size clone** — no change to `proxy.rs`. |
| **D5** | Chaos-dropout throwaway `Response` | Confirmed **[existing — verified at `engine.rs:299-321`]**: `spawn_trace` is handed `&Response::new(Body::empty())` at `:314`, then the real 499 + `Connection: close` is built at `:319-321`. **Decision: do not fix.** F7 passes `&[]` at this call site, which yields `response_body = NULL` — exactly what AC-69 demands. Reordering to pass the real 499 would silently change two *already-persisted* values (`response_headers` `{}` → `{"connection":"close"}`, and it still could not fix `status_code = 0`), which is outside F7's frozen "value change to `response_body` only" scope. Recorded as **R-DROPOUT** in §7. |
| **D6** | `request_body` truncation convention | Confirmed **silent, marker-free byte cut**: `if s.len() > cap { s[..cap].to_string() }` **[existing — verified at `engine.rs:646-653`]**, with `cap = state.cfg.max_body_bytes` (default `256_000` **[verified at `backend/src/config.rs:170`]**). It **panics** on a mid-character `cap` (R12 confirmed). `response_body` mirrors it exactly through the shared helper in §2.11. |
| **D7** | R11 slot-in point for response-header redaction | **Named seam specified.** §2.11 requires the response-header map to be built by a new `helpers::response_headers_for_trace(&HeaderMap) -> BTreeMap<String,String>` (today an identity projection), replacing the inline collect at **`engine.rs:636-644`**. A future security requirement becomes a one-function change in `backend/src/helpers.rs` next to the existing `redact()` **[existing — verified at `backend/src/helpers.rs:34-49`]** — no engine edit, no shape change, no migration. |
| **D8** | AC-73(d)'s `max(2 ms, 10 %)` / `5 ms` numbers | **`MUST-FIX` — not measurable as written.** `duration_ms = t0.elapsed().as_millis() as i64` **[existing — verified at `engine.rs:632`]** and `overhead_ms = (duration_ms - applied_latency_ms).max(0)` **[`:633`]**, i.e. the metric is quantised to whole milliseconds. For the stated workload (rule, 64 KB template, `latency_ms = 0`) the baseline `overhead_ms` is **0 or 1**, so "10 %" is degenerate and "p95 ≤ 5 ms" passes even if capture were 400 % slower. The real cost of capture is a refcount bump plus one ≤256 KB `memcpy` (single-digit µs). §2.11 replaces (d) with a µs-resolution harness measurement plus a bucket-stability assertion. |
| **D9** | **Share-code storage: plaintext vs hashed** (PRD R3, left to me) | **`MUST-FIX` — I am choosing HASHED.** `code_hash = sha256(code)` with a separate non-secret `id`. Full reasoning + trade-off table in §3.2. The decisive fact is D10, not at-rest paranoia. |
| **D10** | `DELETE /api/endpoints/{token}/shares/{code}` puts a **live bearer credential in the owner-side nginx access log** | **`MUST-FIX`, new finding.** §5.9.3 correctly turns `access_log off` for `/api/share/` and `/s/`, but the owner *revoke* route sits under `location /api/` **[existing — verified at `deploy/nginx.conf:18-25`]**, which logs the full request line. A plaintext code in that path is logged in cleartext — the exact class of leak commit `47a267c` fixed for `?cap=`. There is no nginx prefix-location that can exclude `/api/endpoints/*/shares/*` without disabling logging for **all** endpoint routes. **Fix: revoke by non-secret `id`, not by code** (§2.1). This makes hashing (D9) almost free, since the owner list no longer needs to echo the code back. |
| **D11** | AC-25 "each row showing … the URL, a copy action" | **`MUST-FIX` (consequence of D9/D10).** With hashed storage the code exists in plaintext only in the 201 response. The list shows `id` + `label` + `created_at` + `last_used_at`; the URL + copy affordance live on the just-minted link only. `journey.md`/`ux.md` do **not** exist yet in this feature folder, so this is the cheapest possible moment to make the change — no downstream doc rework. |
| **D12** | AC-55's "(quoted per AC-54)" | **`CLARIFY`.** `=cmd\|' /c calc'!A1` contains no comma, `"`, CR or LF, so under AC-54's own rule it is **not** quoted. The correct expected cell is the bare `'=cmd\|' /c calc'!A1`. §2.7 freezes guard-then-quote ordering and drops the misleading parenthetical. |
| **D13** | AC-13/§5.5.6 "built by composing the existing `endpointConfigPatchSchema`" | **`CLARIFY`.** `endpointConfigPatchSchema` makes all nine fields `.optional()` and is not `.strict()` **[existing — verified at `src/api/schemas.ts:48-58`]**, so composing it would accept a bundle missing every endpoint field and would silently pass validation. §2.6 specifies a separate `.strict()` object with all nine fields **required** (nullable where the API is nullable), plus a compile-time assignability check to `EndpointConfigPatch`. Rule objects keep the non-strict `mockRuleCreateSchema` (zod strips unknown keys) so a stray `id` is tolerated, matching AC-16's "unknown **top-level** keys" wording. |
| **D14** | AC-37 "**every** public response carries `Cache-Control: no-store`" | **`CLARIFY`.** axum's `MethodRouter` auto-405 (required by AC-39) is generated by the framework with an empty body and no custom headers; it cannot carry `no-store` without an extra layer. Narrow AC-37 to the handler-produced statuses (200/404/422/429/503). A 405 carries no user data, so this is not a leak. Same note applies to the 405 body not being the flat error envelope — consistent with every other route in the app. |
| **D15** | `SHARE_RATE_LIMIT_PER_MIN` default `60` | **`CLARIFY`.** Arithmetic: one viewer polling at 5 s (AC-45) burns **12 req/min** on the list route alone, before any detail clicks. Two viewers behind one corporate NAT egress IP (the common case for "show the vendor") exceed 60/min while doing nothing unusual. §2.10 raises the default to **120**. |
| **D16** | No regex crate | **`CLARIFY`.** `Cargo.toml` has no `regex` dependency **[existing — verified at `backend/Cargo.toml`]**, so §5.2's `^[A-Za-z0-9_-]{32,64}$` must be a hand-rolled char check (the codebase precedent is `is_safe_key` **[existing — verified at `backend/src/helpers.rs:118-125`]**). §2.2 specifies `is_share_code_shape`. |

Everything else in the PRD's §5 survives review and is carried forward below.

---

## 1. Approach

**Three independent workstreams, deliberately decoupled by the contract so they can land in any
order.**

1. **F1 / F2 / F3 / F5 / F6 are pure frontend work against routes that already exist.** Not one line
   of Rust changes for them. F1 calls the existing `DELETE /api/endpoints/{token}/requests`
   **[existing — verified at `backend/src/routes/api.rs:878-889`, routed at `:1000-1002`]**; F3 and F6
   orchestrate the existing `PATCH /api/endpoints/{token}` **[`:380-481`]** and
   `POST /api/endpoints/{token}/rules` **[`:584-628`]**; F5 fans out over the existing
   `GET /api/requests/{id}` **[`:850-874`]**. The only new *logic* is pure and unit-testable, so it
   goes into three new leaf modules (`src/lib/csv.ts`, `src/lib/config-bundle.ts`,
   `src/lib/request-export.ts`) rather than growing the screens. F2 is a two-line deletion.

2. **F4 is the one new surface, and it gets its own module.** All five routes (three owner, two
   public) live in **one new file** `backend/src/routes/share.rs` so the unauthenticated attack
   surface is auditable in a single `cargo` file, and so `api.rs` (already 1063 lines) does not grow.
   `api.rs` is touched in exactly **two** places: `effective_client_ip` becomes `pub(crate)`, and
   `delete_endpoint` gains the tombstone-revokes-shares statement. The share router is merged
   alongside `api_router()` in `router::build_app` **[existing — verified at
   `backend/src/router.rs:178-193`]**, which keeps `api.rs`'s "18 §5.2 endpoints" doc-comment true.

   **Two security decisions I am making, not deferring** (both detailed in §3.2): the code is stored
   **hashed** (`sha256`, mirroring `owners.secret_hash` **[existing — verified at
   `backend/migrations/0001_init.sql:11`]**), and **revoke addresses a link by non-secret `id`, never
   by code**, so no share code ever appears in a URL that the owner-side nginx `location /api/` logs.
   The code appears in a URL exactly once, on the public resolver, where `access_log off` applies.

3. **F7 is a ~40-line change in one file, using mechanism (i) (buffer-once), and it is provably
   allocation-free for the payload.** The PRD offered two shapes; **(i) wins decisively**. It is one
   helper called at four sites in `engine.rs`, covering all eight `served_by` values through a single
   code path, and it touches neither `tunnel_ws.rs` nor `proxy.rs` (which mechanism (ii) would). The
   reason it is free rather than merely acceptable: every mock body is constructed from an owned
   `String`/`Vec<u8>`/`Bytes` (`Body::from(body_out.clone())` **[`engine.rs:216-219`]**,
   `Body::from(pr.body)` **[`:478-481`]**, `serde_json::to_vec` **[`:565-573`]**,
   `Body::from(body_bytes)` **[`tunnel_ws.rs:70-76`]**, or `Body::empty()`), which becomes a
   **single-frame** body; `to_bytes` on a single-frame body returns the *same* `Bytes` buffer via
   `Bytes::split_to` (verified in D4), and rebuilding with `Bytes::clone` is a refcount bump. The only
   real work is the truncated `String` the row needs anyway — and it is bounded at
   `MAX_BODY_BYTES` **before** anything is moved into the spawned task, which is the memory mitigation
   R4 asked for.

**What deliberately does not change:** no new WS/SSE event (§2.9); no change to any of the 18 existing
route shapes; no change to the mock plane's client-visible bytes (AC-72); no new dependency in
`Cargo.toml` or `package.json`; migration `0001_init.sql` is untouched.

---

## 2. Frozen interface contract (authoritative §5)

Inherited and unchanged: flat error envelope `{"error":"<code>","detail":"<human>"}`
**[existing — verified at `backend/src/error.rs:72-74`]**; owner routes require
`Authorization: Bearer <owner_secret>` via the `OwnerId` extractor and return 401 + `WWW-Authenticate:
Bearer` or **404-not-403** **[existing — verified at `backend/src/auth.rs:37-49`, `:55-69`,
`:114-131`]**; any `sqlx::Error` maps to `503 store_unavailable` **[existing — verified at
`backend/src/error.rs:97-106`]**; timestamps are RFC3339 UTC via `to_rfc3339` **[existing — verified
at `backend/src/routes/api.rs:49-60`]**; `Option<T>` serialises as **present-with-`null`**
**[existing — verified at `backend/src/models.rs:1-8` and the test at `:292-319`]**.

### 2.1 New HTTP endpoints — owner-authenticated (F4)

| # | Method | Path | Auth | Request | Success | Errors |
|---|---|---|---|---|---|---|
| 19 | POST | `/api/endpoints/{token}/shares` | Bearer owner | `ShareLinkCreate` (§2.5.1) | **201** `ShareLinkCreated` (§2.5.3) + `Cache-Control: no-store` | 401 `unauthorized` · 404 `not_found` · 422 `validation_error` · 503 `store_unavailable` |
| 20 | GET | `/api/endpoints/{token}/shares` | Bearer owner | — | **200** `ShareLink[]` (§2.5.2) | 401 · 404 · 503 |
| 21 | DELETE | `/api/endpoints/{token}/shares/{id}` | Bearer owner | — | **204**, no body | 401 · 404 `not_found` · 503 |

**`{id}` is a positive integer** (`share_links.id`), **not** the share code — see D10. A non-integer
`{id}` is rejected by axum's `Path<(String, i64)>` extraction before the handler runs (400 from the
extractor; the handler never sees it — same as the existing `/rules/:id` routes
**[existing — verified at `backend/src/routes/api.rs:632-645`]**).

Exact semantics, in handler order:

**#19 POST**
1. `OwnerId` extractor → 401 on missing/malformed/unknown Bearer (AC-29).
2. `assert_owns_endpoint(pool, token, owner_id)` → **404** for unknown token *and* for another
   owner's token (AC-28) **[existing — verified at `backend/src/auth.rs:55-69`]**.
3. Reject a tombstoned endpoint: `SELECT gone_at FROM endpoints WHERE token = ?`; non-null → **404**
   `not_found` ("Endpoint not found."). *(New constraint, not in the PRD: minting a share for a
   deleted endpoint would produce a link that is dead on arrival. Cheap to enforce, so enforce it.)*
4. Validate `label`: `None`/absent is allowed; a present label is trimmed, and
   `label.chars().count() > 80` → **422** `validation_error` "label must be at most 80 characters."
   A label that trims to empty is stored as `NULL`.
5. Enforce the cap: `SELECT COUNT(*) FROM share_links WHERE token = ? AND revoked_at IS NULL`;
   `>= cfg.share_max_per_endpoint` → **422** `validation_error`
   "This endpoint already has the maximum of {n} active share links. Revoke one first." (AC-27).
6. `let code = ids::gen_share_code(cfg.share_code_bytes);` → `INSERT INTO share_links (code_hash,
   token, label) VALUES (?, ?, ?)` with `code_hash = ids::hash_secret(&code)`; `RETURNING id, created_at`.
7. **201** with `ShareLinkCreated { id, code, url, label, created_at, last_used_at: null }`.
   `code` and `url` appear **only** here, in **only** this response, and are never re-derivable.

**#20 GET** — `OwnerId` → `assert_owns_endpoint` → `SELECT id, label, created_at, last_used_at FROM
share_links WHERE token = ? AND revoked_at IS NULL ORDER BY created_at DESC, id DESC` → **200**
`ShareLink[]`. Revoked links never appear (AC-25). A tombstoned endpoint still lists (its links are
already revoked by #22, so the list is `[]`).

**#21 DELETE** — `OwnerId` → `assert_owns_endpoint` → `UPDATE share_links SET revoked_at =
datetime('now') WHERE id = ? AND token = ? AND revoked_at IS NULL`. `rows_affected() == 0` → **404**
`not_found` "Share link not found." (covers unknown id, id on another endpoint, and already-revoked —
indistinguishable, and idempotent from the caller's point of view). Otherwise **204** with no body,
matching `delete_rule` **[existing — verified at `backend/src/routes/api.rs:812`]**.
**Soft revoke only — never a row delete**, so a revoked `code_hash` can never be re-minted and the
`code_hash` PRIMARY KEY keeps enforcing global uniqueness against revoked codes too.

### 2.2 New HTTP endpoints — PUBLIC, no authentication (F4)

Mounted under `/api/` so nginx's existing `location /api/` proxy reaches them (§2.12) and so
`resolve_plane` classifies them as `Plane::Api` **[existing — verified at
`backend/src/planes.rs:162-164`]**. Together with `POST /api/session` **[existing — verified at
`backend/src/routes/api.rs:983`]** these are the **only** unauthenticated routes in HookBox.

| # | Method | Path | Auth | Request | Success | Errors |
|---|---|---|---|---|---|---|
| 22 | GET | `/api/share/{code}/requests?limit&offset` | **none** | `limit` 1..200 (default 50), `offset` ≥ 0 | **200** `PublicShareFeed` (§2.5.4) | **404** `not_found` (unknown / revoked / tombstoned — byte-identical) · 422 `validation_error` · 429 `rate_limited` + `Retry-After` · 503 `store_unavailable` |
| 23 | GET | `/api/share/{code}/requests/{id}` | **none** | — | **200** `PublicRequestDetail` (§2.5.5) | **404** (as above, plus unknown request id, plus a request id not belonging to this share's endpoint) · 429 · 503 |

Every handler-produced response carries **`Cache-Control: no-store`** (AC-37, narrowed per D14). No
other verb is routed for these two paths, so axum returns **405** (AC-39).

**The single 404 constructor.** AC-36 requires byte-identical bodies *and* status lines for unknown /
revoked / tombstoned. That is guaranteed structurally by funnelling every negative outcome through one
function:

```rust
/// The ONLY 404 the public resolver may emit. Unknown code, revoked code,
/// tombstoned endpoint, unknown request id and cross-endpoint request id all
/// return this exact value, so a scanner learns nothing from the difference.
fn share_not_found() -> ApiError {
    ApiError::not_found("This share link is not available.")
        .with_header("cache-control", "no-store")
}
```

**Frozen check order for #22** (the order is load-bearing — see the note below):

1. **Rate limit** — `state.limiter.check(&format!("share:{ip}"), cfg.share_rate_limit_per_min, 60)`
   using the existing token bucket **[existing — verified at `backend/src/limiter.rs:80-128`]** and
   the existing proxy-aware IP resolver **[existing — verified at
   `backend/src/routes/api.rs:273-299`]**. Over limit → **429** `rate_limited` + `Retry-After`
   (AC-38). The key prefix is `share:` so it can never collide with the mock plane's `rl:<token>`
   keys **[existing — verified at `backend/src/limiter.rs:71-76`]**; `limiter.rs` needs no change.
2. **Parameter validation** — `limit` must be `1..=200`, `offset` must be `>= 0`, else **422**
   `validation_error` with the same wording as the owner route **[existing — verified at
   `backend/src/routes/api.rs:829-836`]**.
3. **Code shape** — `is_share_code_shape(&code)`; false → `share_not_found()` **with no DB read**.
4. **Code resolution** — one statement (below); miss → `share_not_found()`.
5. **200** + `no-store`.
6. **Fire-and-forget** `last_used_at` touch (below), after the response value is built.

> **Why 2 before 4.** If `limit` were validated *after* resolving the code, then `?limit=999` would
> return 422 for a live code and 404 for a dead one — a boolean oracle that defeats AC-36. Validating
> parameters first makes the response depend only on the parameters. The existing owner route already
> orders it this way **[verified at `backend/src/routes/api.rs:829-837`]**; the public route must too,
> and a test must assert it (`?limit=999` returns 422 for **both** a valid and an invalid code).

**The resolution statement** (one round trip, joins liveness so a tombstone is indistinguishable):

```sql
SELECT s.id, s.token, e.name, e.created_at AS endpoint_created_at, e.request_count
  FROM share_links s
  JOIN endpoints  e ON e.token = s.token
 WHERE s.code_hash = ?          -- sha256(code) — never the code itself
   AND s.revoked_at IS NULL
   AND e.gone_at   IS NULL
```

**Trace list** (after resolution), reusing the owner route's shape and ordering
**[existing — verified at `backend/src/routes/api.rs:838-845`]**:

```sql
SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?
```

**#23 detail** — steps 1, 3, 4 as above (no `limit`/`offset`, so no step 2), then:

```sql
SELECT * FROM request_logs WHERE id = ? AND token = ?   -- token from the resolved share row
```

Miss → `share_not_found()`. **The `AND token = ?` is the whole of AC-35** — cross-endpoint trace
enumeration is impossible because the id is scoped by the share's endpoint inside the same statement.

**`last_used_at`, made safe.** Best-effort, coalesced, and off the response path so an unauthenticated
GET can never contend on the write lock in front of a viewer's response:

```rust
// after the 200 value is built, mirroring engine::spawn_trace's fire-and-forget shape
let pool = state.pool.clone();
tokio::spawn(async move {
    let _ = sqlx::query(
        "UPDATE share_links SET last_used_at = datetime('now')
          WHERE id = ? AND (last_used_at IS NULL
                            OR last_used_at < datetime('now','-60 seconds'))",
    ).bind(share_id).execute(&pool).await;
});
```

At most one write per link per minute regardless of poll rate. This is a write to `share_links` only;
AC-39's asserted-unchanged tables (`request_logs`, `mock_rules`, `endpoints`, `endpoint_state`,
`crud_collections`) are untouched, and the AC's row-count assertions hold verbatim.

**Code shape check** (no regex crate — D16):

```rust
/// base64url-no-pad shape gate. `SHARE_CODE_BYTES` is env-tunable, so accept the
/// whole plausible band rather than one exact length: 32 chars = the 24-byte
/// default, 64 chars covers up to 48 bytes.
fn is_share_code_shape(code: &str) -> bool {
    let n = code.len();                 // ASCII-only charset ⇒ len() == char count
    (32..=64).contains(&n)
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
```

### 2.3 Changed behavior on existing endpoints (F4 only)

`DELETE /api/endpoints/{token}` **[existing — verified at `backend/src/routes/api.rs:506-532`]** —
**request and response shapes unchanged** (200 `Message`). One statement is added, in the same handler,
immediately after the `crud_collections` delete at `:522-525` and before the `gone_at` update at
`:526-529`:

```sql
UPDATE share_links SET revoked_at = datetime('now') WHERE token = ? AND revoked_at IS NULL
```

That satisfies AC-30 for the *tombstone* window. The eventual hard delete by the retention sweep
**[existing — verified at `backend/src/tasks/sweep.rs`, step (c)]** removes the rows via
`ON DELETE CASCADE`. No other existing route changes for any feature. `effective_client_ip`
**[existing — verified at `backend/src/routes/api.rs:273-299`]** changes visibility from private to
`pub(crate)` — no behavior change, and its five existing unit tests **[verified at `:1018-1061`]**
stay where they are.

### 2.4 DB schema — one new migration (F4 only)

`backend/migrations/0002_share_links.sql` **[new]**. Additive only; `0001_init.sql` is byte-untouched
so its recorded `sqlx` checksum stays valid **[existing — verified at `backend/src/db.rs:31-34`:
`sqlx::migrate!("./migrations")`]**.

```sql
-- migrations/0002_share_links.sql -------------------------------------------
-- F4 public read-only share links. The code is a URL-borne bearer credential:
-- stored ONLY as sha256 (mirroring owners.secret_hash), surfaced in plaintext
-- exactly once in the 201 response, and addressed thereafter by the non-secret
-- integer id so no code ever lands in an owner-route URL (and therefore never
-- in nginx's access log for `location /api/`).

CREATE TABLE share_links (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,  -- non-secret handle: list + revoke
    code_hash    TEXT NOT NULL UNIQUE,               -- sha256(code) hex, 64 chars; the lookup key
    token        TEXT NOT NULL,                      -- owning endpoint
    label        TEXT,                               -- optional operator note, <= 80 chars, NULL when blank
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at   TEXT,                               -- non-null => dead; never un-set, never deleted
    last_used_at TEXT,                               -- best-effort, coalesced to >= 60s granularity
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);

-- Public resolver: exact-match on the UNIQUE code_hash index (one B-tree probe).
-- Owner list + the active-count cap check: covered by this composite index.
CREATE INDEX idx_share_token_active ON share_links(token, revoked_at, created_at DESC);
```

No other table, column or index changes. **F1, F2, F3, F5, F6 and F7 require zero schema change** —
`request_logs.response_body TEXT` already exists **[existing — verified at
`backend/migrations/0001_init.sql:65`]** and is already bound on insert **[existing — verified at
`backend/src/db.rs:85`]**; F7 changes only the *value* written (§2.11).

### 2.5 Shared data models

Rust side: `backend/src/models.rs` **[existing — verified]**. TS side: `src/api/schemas.ts`
**[existing — verified]**. Every `Option<T>` / `.nullable()` field is **present-with-`null`**, never
omitted.

**2.5.1 `ShareLinkCreate`** (request body for #19):

```
Rust:  #[derive(Deserialize)] pub struct ShareLinkCreate { #[serde(default)] pub label: Option<String> }
TS:    shareLinkCreateSchema = z.object({ label: z.string().max(80).nullable().optional() })
Wire:  { "label": "Acme vendor" }  |  { "label": null }  |  {}
```

**2.5.2 `ShareLink`** (owner list item — **carries no secret material**):

```
{ id: number, label: string | null, created_at: string, last_used_at: string | null }
```

**2.5.3 `ShareLinkCreated`** (201 body for #19 — the **only** place `code`/`url` ever appear):

```
{ id: number, code: string, url: string,
  label: string | null, created_at: string, last_used_at: null }
```

`url` is built by a new `share_url(state, code)` helper that mirrors how `path_url` is built
**[existing — verified at `backend/src/routes/api.rs:42-44`]**:

```rust
fn share_url(state: &AppState, code: &str) -> String {
    format!("{}/s/{code}", state.cfg.public_base_url)   // blank base ⇒ "/s/{code}", SPA absolutizes
}
```

> **`share_url` must never use `mock_url`'s wildcard form.** `mock_url` returns
> `https://{token}.{mock_domain}` when wildcard mode is on **[existing — verified at
> `backend/src/routes/api.rs:32-40`]**, and `resolve_plane` sends **everything** on a mock host to P1
> **[existing — verified at `backend/src/planes.rs:137-148`]** — a share URL on the mock host would be
> swallowed by the interceptor and 404 as an unknown mock path. The share URL is always on the **app**
> origin. The SPA absolutizes a relative value with the existing helper **[existing — verified at
> `src/lib/url.ts:8-11`]**.

**2.5.4 `PublicShareFeed`** (200 body for #22):

```
{ endpoint: { name: string | null, created_at: string, request_count: number },
  requests: PublicRequestSummary[] }
```

**2.5.5 Public trace projections** — reduced from the owner shapes **[existing — verified at
`src/api/schemas.ts:149-172`, `backend/src/models.rs:244-269`]**:

```
PublicRequestSummary = { id: number, method: string, path: string, status_code: number,
                         served_by: ServedBy, duration_ms: number, timestamp: string }

PublicRequestDetail  = PublicRequestSummary & {
                         request_headers:  Record<string,string>,
                         query_params:     Record<string,string>,
                         request_body:     string | null,
                         response_headers: Record<string,string>,
                         response_body:    string | null }
```

**Complete omission list vs. the owner shapes (AC-34/AC-44a): `token`, `matched_rule_id`,
`overhead_ms`, `trace`, `state_snapshot`.** The five body/header fields above are **present keys**
(present-with-`null` when the column is `NULL`), so a future narrowing is a deliberate contract change
and not a silent regression.

**Implementation requirement (this is how AC-34 becomes structural rather than aspirational):** the
public structs are **standalone `#[derive(Serialize)]` structs in `backend/src/models.rs`**, built
field-by-field from the row in `share.rs`. They must **not** be produced by `#[serde(skip)]`-ing
fields off `RequestDetail`, and must not `#[serde(flatten)]` an owner struct — either would make a
future field added to the owner shape leak into the public projection by default. A test asserts the
serialised JSON key set is exactly the allow-list and that the body does not contain the endpoint
token as a substring.

**2.5.6 `ConfigBundle`** — the F3 **file** format. A client/file contract; never a request body; no
Rust counterpart.

```
{ hookbox_config_version: 1,                 // z.literal(1)
  exported_at: string,                       // RFC3339 UTC, from new Date().toISOString()
  endpoint: {                                // ALL NINE REQUIRED (D13), .strict()
    name:               string | null,
    auto_crud:          boolean,
    target_url:         string | null,
    default_mode:       "mock_404" | "echo",
    latency_ms:         number,              // int
    rate_limit_per_min: number,              // int
    chaos_pct:          number,              // int
    chaos_mode:         "error" | "dropout",
    cors_enabled:       boolean },
  rules: MockRuleCreate[] }                  // mockRuleCreateSchema, non-strict (unknown keys stripped)
```

`configBundleSchema` is `.strict()` at the **top level** (rejects any key other than the three above)
and on `endpoint` (rejects `token`, `mock_url`, `path_url`, `created_at`, `last_hit`,
`request_count`, `tunnel_active` — AC-13). Import limits enforced **before any network write**
(AC-16): file size ≤ **5 MB** (checked on `File.size` before `text()`), `rules.length` ≤ **200**,
`hookbox_config_version === 1`. Rule objects reuse `mockRuleCreateSchema` **[existing — verified at
`src/api/schemas.ts:115-126`]** so a stray `id`/`token`/`created_at` inside a rule is stripped rather
than fatal.

A compile-time guard keeps the bundle and the API in lockstep:

```ts
// src/lib/config-bundle.ts
const _assignable: EndpointConfigPatch = {} as ConfigBundle['endpoint']  // fails typecheck on drift
```

**2.5.7 Default catch-all rule payload (F6)** — the exact frozen `MockRuleCreate` body:

```json
{
  "name": "Catch-all (default)",
  "priority": 1000,
  "enabled": true,
  "match": { "method": "ANY", "path": "/*", "headers": {}, "query": {},
             "body_conditions": [], "state_requirements": [] },
  "response": { "status_code": 200, "headers": {}, "content_type": "application/json",
                "body_template": "{\n  \"status\": \"ok\",\n  \"message\": \"Default catch-all response from HookBox. Edit this rule to change it.\"\n}" },
  "state_writes": [], "latency_ms": null, "rate_limit_per_min": null,
  "chaos_mode": null, "webhook_action": null
}
```

Backend confirmation for AC-63 — **no matcher or engine change is needed**: `"ANY"` already matches
every method **[existing — verified at `backend/src/interceptor/matcher.rs:182`]**, `/*` already
compiles to a pure catch-all **[existing — verified at `backend/src/interceptor/matcher.rs:53`]**,
those are already the serde defaults **[existing — verified at `backend/src/models.rs:135-138`]**,
`priority = 1000` is inside the accepted `0..=100000` band **[existing — verified at
`backend/src/routes/api.rs:561-565`]**, and the list order is `ORDER BY priority, id`
**[existing — verified at `backend/src/routes/api.rs:542`]** so the catch-all sorts last and loses to
every default-priority (100) rule.

### 2.6 F3 import orchestration (frozen client behavior)

`PATCH /api/endpoints/{token}` first, then one `POST /api/endpoints/{token}/rules` per rule in array
order (AC-17). **Stop-at-first-failure, no rollback, add-never-replace** (AC-18/AC-19). Failure report
must state, in one message: that the config step succeeded (or that it failed and **zero** rules were
attempted), `k-1` of `n` rules created, the 1-based index and `name` of the failing rule, the server's
`detail`, and that no rule after it was attempted. The PATCH body is the bundle's `endpoint` object
verbatim (all nine keys present ⇒ every field is applied, no `exclude_unset` ambiguity). Progress is
determinate ("Creating rule {i} of {n}…"), both Export and Import are disabled while in flight, and
on completion the screen re-fetches `GET /api/endpoints/{token}` (AC-20).

### 2.7 CSV artifact format (F5) — frozen

- Media type `text/csv;charset=utf-8`, UTF-8, **no BOM**, **CRLF** record separator, RFC 4180 quoting,
  **including a trailing CRLF after the final record** (frozen so fixtures are byte-stable).
- Filename `hookbox-requests-<token>-<YYYYMMDDTHHMMSSZ>.csv`, where the stamp is
  `new Date().toISOString()` with `-`, `:` and the `.mmm` fraction removed
  (`2026-08-07T11:22:33.444Z` → `20260807T112233Z`).
- Header row, then one row per **visible feed row, newest first** (AC-51), exactly 10 columns:

```
timestamp,method,path,status_code,served_by,duration_ms,request_headers,request_body,response_headers,response_body
```

**Cell derivation — frozen, so a failed detail fetch can never shift a column:**

| Column | Source | On detail failure |
|---|---|---|
| `timestamp` | `row.timestamp` (the feed `RequestSummary`) | unchanged |
| `method` | `row.method` | unchanged |
| `path` | `row.path` | unchanged |
| `status_code` | `row.status_code` — bare integer, unquoted | unchanged |
| `served_by` | `row.served_by` | unchanged |
| `duration_ms` | `row.duration_ms` — bare integer, unquoted | unchanged |
| `request_headers` | `JSON.stringify(detail.request_headers)` — compact object | `pending` / `unavailable` |
| `request_body` | `detail.request_body ?? ''` | `pending` / `unavailable` |
| `response_headers` | `JSON.stringify(detail.response_headers)` — compact object | `pending` / `unavailable` |
| `response_body` | `detail.response_body ?? ''` | `pending` / `unavailable` |

The six summary columns always come from the feed row — **never** from the detail response — even when
the detail succeeds. That makes a row's summary cells independent of the detail fetch and keeps AC-52
trivially true.

**Sentinels (AC-52).** `pending` when the detail fetch returned **404** (the documented
just-streamed-trace case **[existing — verified at `src/screens/dashboard/inspector.tsx:63-66`]**);
`unavailable` for any other per-row failure (5xx, network, `contract_mismatch` from the zod parse
**[existing — verified at `src/api/client.ts:128-134`]**). An **empty cell** is semantically distinct:
it means the detail fetch **succeeded** and the stored column is `NULL` (AC-56a/AC-69). Completion
toast: "Exported {n} rows ({m} without detail)."

**A 401 is the one aborting case** (AC-53): the client already clears the session and bounces
**[existing — verified at `src/api/client.ts:99-105`]**, so the export stops and no file is written.

**Escaping, frozen order: formula guard, then quote.**
1. **Guard** — if the cell's first character is `=`, `+`, `-`, `@`, TAB (U+0009) or CR (U+000D),
   prefix a single `'`. Applied uniformly to all ten columns (it can never fire on the two integer
   columns).
2. **Quote** — if the guarded value contains `,`, `"`, CR or LF, wrap in `"` and double every embedded
   `"`.

Per D12, `=cmd|' /c calc'!A1` becomes `'=cmd|' /c calc'!A1` **unquoted** (it contains no `,`, `"`, CR
or LF); the PRD's "(quoted per AC-54)" parenthetical is wrong and must be dropped from AC-55.

**No client-side redaction, ever** (AC-56). `request_headers` arrive already server-redacted
(`authorization`, `cookie`, `x-owner-id` → `<redacted>`) **[existing — verified at
`backend/src/helpers.rs:34-49`, applied to request headers only at
`backend/src/interceptor/engine.rs:674`]**; `response_headers` arrive **verbatim** **[existing —
verified at `backend/src/interceptor/engine.rs:636-644` — no `redact()` call]**. The exporter emits
both exactly as received. If security.md later requires response-header redaction, that changes these
cell *values* on the server (via the D7 seam) and not this format.

**Fetch mechanism (AC-47/AC-48).** Up to `FEED_CAP` = 100 rows **[existing — verified at
`src/feed/use-feed.ts:42`]**, `GET /api/requests/{id}` per row **[existing — verified at
`src/api/client.ts:217-219`]**, **exactly 4 in flight**, one shared `AbortController`:

```ts
// src/lib/request-export.ts  [new]
export const EXPORT_CONCURRENCY = 4
export async function fetchDetails(
  rows: readonly RequestSummary[],
  signal: AbortSignal,
  onProgress: (done: number, total: number) => void,
): Promise<ReadonlyArray<DetailCell>>   // index-aligned with `rows`
```

A fixed pool of `EXPORT_CONCURRENCY` workers pulls from a shared cursor over `rows`; results are
written into a pre-sized array **by index**, so completion order never affects row order. `onProgress`
fires once per settled row. On abort: no file, no object URL. On success: `new Blob([csv], { type:
'text/csv;charset=utf-8' })` → `URL.createObjectURL` → programmatic anchor click →
`URL.revokeObjectURL` in a `finally` (AC-49).

### 2.8 WebSocket / SSE messages

**No changes.** No new event type; the `hello` / `new_request` / `state_changed` / `endpoint_updated`
envelope is untouched **[existing — verified at `src/feed/events.ts`,
`backend/src/routes/feed.rs`]**. **F7 does not widen it**: the `new_request` summary is built from
`summary_base`, which contains no body field **[existing — verified at
`backend/src/interceptor/engine.rs:692-697`]**, and stays that way (AC-75). F4's viewer opens **no**
WebSocket and **no** EventSource (AC-45), so `/ws/{token}` and `/sse/{token}` remain owner-gated by
`?cap=`.

### 2.9 New configuration (env, safe defaults)

Added to `Config` following the established never-crash-on-a-missing-var rule **[existing — verified
at `backend/src/config.rs:10-41`, `:110-189`]**:

| Field | Env var | Default | Meaning |
|---|---|---|---|
| `share_code_bytes: usize` | `SHARE_CODE_BYTES` | `24` | CSPRNG bytes per code → 32-char base64url → **192 bits** (AC-31) |
| `share_max_per_endpoint: i64` | `SHARE_MAX_PER_ENDPOINT` | `10` | Max **active** links per endpoint (AC-27) |
| `share_rate_limit_per_min: i64` | `SHARE_RATE_LIMIT_PER_MIN` | **`120`** | Per-IP limit on the public resolver (AC-38) — raised from the PRD's 60, see D15 |

`share_code_bytes` is clamped to `>= 16` at load so a misconfigured `SHARE_CODE_BYTES=1` cannot mint a
guessable code (128-bit floor, AC-31). **F7 adds no configuration**: it reuses `max_body_bytes`
**[existing — verified at `backend/src/config.rs:170`]** as the persisted cap for **both** body
columns, and `mitm_max_body_bytes` **[`:177`]** continues to bound what a MITM response can contain
before truncation. No feature flag for capture — a flag would make `response_body = null` ambiguous
("empty" vs "capture off") and fork the CSV/public-detail contract.

**Deployment-level opt-out instead of a `SHARE_LINKS_ENABLED` flag.** An operator who does not want a
public surface blocks it in nginx (`location /api/share/ { return 404; }` and
`location /s/ { return 404; }`). A server-side flag would have to be discoverable by the SPA to hide
the Share control, which would require a shape change to `GET /api/endpoints/{token}` — explicitly out
of scope. Recorded for security.md to accept or override.

### 2.10 F7 response-body capture — the exact mechanism

**Mechanism (i), "buffer once", is chosen.** One helper, four call sites, one file.

#### 2.10.1 The capture helper — `backend/src/interceptor/engine.rs` [existing, +~25 lines]

```rust
use axum::body::{to_bytes, Bytes, HttpBody};   // HttpBody re-exported by axum 0.7 (D1)

/// Buffer a finished mock response's body so the trace can persist the exact
/// bytes the client will receive, returning an equivalent response plus those
/// bytes.
///
/// This is allocation-free for the payload. Every mock-plane body is built from
/// an owned buffer — the rendered template (`:216-219`), `serde_json::to_vec`
/// (`:565-573`), the MITM `pr.body` (`:478-481`), the tunnel's decoded reply
/// (`routes/tunnel_ws.rs:70-76`) or `Body::empty()` — so it is a SINGLE-FRAME
/// body: `to_bytes` hands back the same `Bytes` buffer and `Bytes::clone` is a
/// refcount bump, not a copy. Nothing here can block: the frame is already
/// resolved, so the `.await` never yields.
///
/// The `size_hint().exact()` guard is a forward-compatibility fuse: if a
/// streaming mock body is ever introduced, this returns the response untouched
/// and captures nothing rather than buffering an unbounded stream. Revisit THIS
/// function (and only this function) if that day comes.
async fn capture_response_body(resp: Response) -> (Response, Bytes) {
    let (parts, body) = resp.into_parts();
    let Some(len) = body.size_hint().exact() else {
        tracing::warn!("mock response body has no exact size; response_body not captured");
        return (Response::from_parts(parts, body), Bytes::new());
    };
    if len == 0 {
        // 204 CORS preflight, empty 204 CRUD, chaos dropout — nothing to capture.
        return (Response::from_parts(parts, body), Bytes::new());
    }
    match to_bytes(body, len).await {
        Ok(bytes) => (Response::from_parts(parts, Body::from(bytes.clone())), bytes),
        Err(e) => {
            // Unreachable for an in-memory body (`len` is exact, so the limit
            // cannot trip, and these bodies are infallible). If it ever fires the
            // body is already lost, so the only honest option is an empty one.
            tracing::error!("failed to buffer mock response body: {e}");
            (Response::from_parts(parts, Body::empty()), Bytes::new())
        }
    }
}
```

The `len == 0` fast path is what makes AC-69's three genuinely-empty cases (204 preflight, empty 204
CRUD, chaos dropout) **structurally** `NULL` rather than incidentally so.

#### 2.10.2 The shared body projection — `backend/src/helpers.rs` [existing, +~30 lines]

`engine.rs`'s local `truncate` closure **[existing — verified at `engine.rs:646-653`]** is **deleted**
and replaced by two `pub fn`s in `helpers.rs`, used by **both** body columns (AC-70(d)):

```rust
/// Cut `s` to at most `cap` bytes, floored to a UTF-8 character boundary.
/// Replaces the panicking `s[..cap]` (R12): `is_char_boundary` is O(1) and the
/// loop backs off at most 3 bytes.
pub fn truncate_utf8(s: &str, cap: usize) -> &str {
    if s.len() <= cap { return s; }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

/// The persisted projection shared by `request_body` and `response_body` (§2.11).
/// Empty (before OR after truncation) ⇒ `None`; never an empty string. Wire bytes
/// are lossy-UTF-8 decoded — the same conversion the request path already uses at
/// `engine.rs:73` — then cut per `truncate_utf8`, with no marker and no flag.
pub fn body_for_trace(bytes: &[u8], cap: usize) -> Option<String> {
    if bytes.is_empty() { return None; }
    let decoded = String::from_utf8_lossy(bytes);   // Cow::Borrowed for valid UTF-8: no copy
    let cut = truncate_utf8(&decoded, cap);
    if cut.is_empty() { None } else { Some(cut.to_string()) }
}
```

The trailing `cut.is_empty()` guard keeps "`NULL` ⟺ zero-length" true even under a pathological
`MAX_BODY_BYTES=0`.

#### 2.10.3 The response-header seam (R11) — `backend/src/helpers.rs` [existing, +~10 lines]

Replaces the inline collect at **`engine.rs:636-644`**:

```rust
use axum::http::HeaderMap;

/// Response headers as persisted on a trace. Today an identity projection
/// (mirrors today's behavior exactly, R11).
///
/// THIS IS THE SLOT-IN POINT. If security.md requires response-header redaction
/// (e.g. `set-cookie`, `authorization`), the whole change is this function body
/// plus a `REDACT_RESPONSE_HEADERS` const next to `REDACT_HEADERS` above — one
/// function, one call site (`engine.rs:636`), no engine edit, no shape change,
/// no migration, no contract change. Note MITM already strips upstream
/// `Set-Cookie` before the response is built (`interceptor/proxy.rs:27-44`), so
/// the exposure surface is tunnel replies and rule-authored response headers.
pub fn response_headers_for_trace(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers.iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|vs| (k.as_str().to_string(), vs.to_string())))
        .collect()
}
```

#### 2.10.4 `spawn_trace` signature — one new parameter

```rust
fn spawn_trace(
    state: &AppState, token: &str, method: &str, path: &str,
    status_code: i64, served_by: &str, matched_rule_id: Option<i64>,
    t0: Instant, applied_latency_ms: i64,
    req_headers: &BTreeMap<String, String>,
    query: &BTreeMap<String, String>,
    req_body: &str,
    resp: &Response,
    resp_body: &[u8],                       // NEW — captured bytes; empty ⇒ NULL (AC-69)
    trace: &[Step],
    state_snapshot: &BTreeMap<String, String>,
)
```

and inside, replacing `engine.rs:646-653` + `:677-683`:

```rust
let cap = state.cfg.max_body_bytes;
// ...
request_body:      helpers::body_for_trace(req_body.as_bytes(), cap),
response_headers:  serde_json::to_string(&helpers::response_headers_for_trace(resp.headers()))
                       .unwrap_or_else(|_| "{}".into()),
response_body:     helpers::body_for_trace(resp_body, cap),
```

Both body columns now go through the identical helper — that *is* AC-70(d). `TraceRecord`
**[existing — verified at `backend/src/db.rs:40-56`]** and `insert_trace` **[`:61-106`]** are
unchanged; `response_body` was already bound at `:85`.

#### 2.10.5 The four call sites (and the one that stays a literal `&[]`)

| Site | Where | Edit |
|---|---|---|
| `cors` (204 preflight) | `engine.rs:84-102` | after `identified(...)`: `let (resp, rb) = capture_response_body(resp).await;` → pass `&rb`. `len == 0` ⇒ `NULL` ✓ |
| `ratelimit` (429) | `engine.rs:275-293` | same shape, on `r` |
| `chaos` `Status` | `engine.rs:330-348` | same shape, on `r` |
| **main path** (rule / crud / tunnel / mitm / default) | `engine.rs:359-387` | keep `let status = resp.status()…` at `:359` **before** `identified`; capture **after** `identified` (safe per D2); pass `&rb`; return the rebuilt `resp` |
| `chaos` `Drop` | `engine.rs:299-321` | pass a literal `&[]` alongside the existing throwaway `&Response::new(Body::empty())`. **No capture call.** ⇒ `NULL` ✓ (D5 / R-DROPOUT) |

`resolve_unmatched` **[`engine.rs:391-534`]**, `routes/tunnel_ws.rs` and `interceptor/proxy.rs` are
**not touched**. The pre-existing 413 ingest-cap path still returns without calling `spawn_trace`
**[existing — verified at `engine.rs:65-72`]** and so still writes no row at all — unchanged, not a
regression.

#### 2.10.6 Frozen value semantics (this *is* a contract statement)

| Surface | Field | Before | After F7 |
|---|---|---|---|
| `GET /api/requests/{id}` **[`backend/src/routes/api.rs:850-874`]** | `response_body: string \| null` **[`backend/src/models.rs:266`, `src/api/schemas.ts:169`]** | always `null` | the captured body, or `null` for an empty-bodied response |
| `GET /api/share/{code}/requests/{id}` (§2.2 #23) | `response_body: string \| null` | — | same values |
| F5 CSV `response_body` (§2.7) | cell | always empty | the captured body; empty **only** when the stored value is `NULL` |

1. `NULL` ⟺ zero-length body. **Never an empty string.**
2. Cut to at most `MAX_BODY_BYTES` **bytes of the decoded string**, floored to a character boundary,
   with **no marker and no flag** — a consumer cannot distinguish "exactly cap" from "truncated"
   (accepted, R4).
3. Lossy UTF-8 decoding of the wire bytes; the TEXT column always holds valid UTF-8. The client still
   receives the original raw bytes (AC-72).
4. **No** redaction of either body, request or response — unchanged from today. Only *request headers*
   are redacted (AC-56, R11, D7).
5. Rows written before F7 keep `response_body = NULL` forever; no back-fill.

**No new endpoint, model, table, column, migration, env var, WS event or TS schema edit for F7.**
`Cargo.toml`, `package.json` and `src/api/schemas.ts` are untouched by F7 (D1 confirms the
`HttpBody` re-export makes this true).

#### 2.10.7 AC-73 restated so it is measurable (replaces the PRD's (d))

(a), (b), (c) stand as written. **(d) is replaced:**

- **(d1) Harness-timed, µs resolution.** In `backend/tests/api.rs`, 200 sequential mock requests
  against a rule with a 64 KB `body_template`, `latency_ms = 0`, `chaos_pct = 0`, each timed with
  `std::time::Instant` around the `oneshot` call. Record median and p95 **wall time per request in
  µs** in the PR body, alongside the same numbers from the parent commit.
  **Pass: median ≤ baseline + max(0.5 ms, 20 %).**
- **(d2) Bucket stability on the reported metric.** For the same 200 requests, the distribution of the
  trace-reported `overhead_ms` **[existing — verified at `engine.rs:632-633`]** must not shift: at
  least **95 %** of requests must land in the same integer millisecond bucket as the baseline. This is
  the coarse "did someone add a blocking call" guard, stated in terms the 1 ms quantisation can
  actually express.
- **(d3) Direct helper bound.** A unit test asserts `capture_response_body` on a 5 MB body (the
  `MITM_MAX_BODY_BYTES` worst case) completes in **< 1 ms**, and that the rebuilt response's bytes are
  byte-equal to the input — which is only achievable if no full copy happens.

Rationale recorded for QA: the PRD's `max(2 ms, 10 %)` on `overhead_ms` is unmeasurable because the
baseline is 0–1 ms and the metric is quantised to 1 ms (D8); (a)–(c) plus (d1)–(d3) are the real gates.

### 2.11 Deploy surface (F4) — grounded constraint

`deploy/nginx.conf` proxies **only** `/api/`, `= /healthz`, `/ws/`, `/sse/` and `/e/`; everything else
is `location / { try_files $uri $uri/ /index.html; }` **[existing — verified at
`deploy/nginx.conf:9-71`]**. Three consequences:

1. The public resolver **must** live under `/api/` (§2.2) — any other prefix is swallowed by the SPA
   fallback and never reaches the backend.
2. `/s/:code` needs **no** proxy rule: it falls through `try_files` to `index.html`, and the plane
   resolver already routes unknown app-host paths to the UI plane **[existing — verified at
   `backend/src/planes.rs:178-179`]**, where `serve_spa` falls back to `index.html` **[existing —
   verified at `backend/src/routes/spa.rs:37-60`]**. So it also works without nginx (dev / `cargo run`).
3. **Share codes must never reach an access log.** Two new locations, both *longer* prefixes than the
   existing ones so nginx's longest-prefix rule selects them:

```nginx
# The share code is a bearer credential in the URL path — same class of leak as
# ?cap= on /ws/ and /sse/ (commit 47a267c). Longer prefix than `location /api/`,
# so nginx picks this one. X-Real-IP MUST stay: the public resolver's per-IP
# limiter depends on it (backend/src/routes/api.rs:273-299 only trusts it from a
# loopback peer, which nginx always is).
location /api/share/ {
    proxy_pass http://127.0.0.1:{{APP_PORT}}/api/share/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    access_log off;
}

# The viewer page itself: SPA fallback, but the code is in the path.
location /s/ {
    try_files $uri $uri/ /index.html;
    access_log off;
}
```

The app's own logging must never log the code either: `share.rs` must not `tracing::*` the code or the
full path. Log the resolved `share_links.id` when logging is needed. **This is exactly why revoke is
by `id`** (D10) — the owner route stays under the logging `location /api/`, and it now carries no
secret.

---

## 3. Data model & storage

### 3.1 Access pattern (sqlx, not `aiosqlite`)

Every statement is a **parameterised `sqlx::query*`** against the shared WAL pool
(`max_connections = 8`, `busy_timeout = 5s`, `foreign_keys = ON` **[existing — verified at
`backend/src/db.rs:10-28`]**), reached through `State(state): State<AppState>` **[existing — verified
at `backend/src/state.rs:20-32`]**. No string interpolation into SQL anywhere in the new code (the
existing PATCH builders interpolate only allow-listed **column names**, never values **[existing —
verified at `backend/src/routes/api.rs:467-476`]**; the share routes need no dynamic SQL at all).
Migrations run on startup via `sqlx::migrate!("./migrations")` **[existing — verified at
`backend/src/db.rs:31-34`, called from `backend/src/main.rs`]**, so `0002_share_links.sql` needs no
wiring beyond existing.

Statement counts per request, for the record:

| Route | Statements |
|---|---|
| #19 POST shares | 1 ownership + 1 gone-check + 1 count + 1 insert = **4** |
| #20 GET shares | 1 ownership + 1 select = **2** |
| #21 DELETE shares/{id} | 1 ownership + 1 update = **2** |
| #22 GET share/{code}/requests | 1 resolve (joined) + 1 list = **2** (+1 coalesced `last_used_at` off-path) |
| #23 GET share/{code}/requests/{id} | 1 resolve (joined) + 1 select = **2** (+1 off-path) |
| mock request (F7) | **unchanged** — still the single `insert_trace` + its prune **[existing — verified at `backend/src/db.rs:61-106`]** (AC-73(b)) |

`share_links` is **never** cached. The `rule_cache` is keyed by endpoint token for the mock plane only
**[existing — verified at `backend/src/rule_cache.rs:1-8`]** and gets no share entries, which is what
makes revocation take effect on the very next request (AC-37).

### 3.2 The share-code storage decision (PRD R3 — my call: **hashed + non-secret `id`**)

Five options considered:

| Option | At-rest posture | Re-copy an existing link | Code in an owner-route URL (→ nginx log) | Verdict |
|---|---|---|---|---|
| **A. Plaintext PK, revoke by code** (the PRD's default) | code readable from any DB dump/backup/replica | ✅ yes | ❌ **yes — logged in cleartext** | **Rejected** on D10 alone |
| **B. Plaintext PK + `id`, revoke by id** | same as A | ✅ yes | ✅ no | Fixes D10 but keeps a plaintext bearer credential at rest, inconsistent with `owners.secret_hash` — an easy target for a security override *after* lock |
| **C. `code_hash` PK + `code_prefix` for display** | hashed | ❌ no | ✅ no | Works, but leaks 48 bits of the code into the owner list for no benefit once `id` exists |
| **D. `code_hash` + non-secret `id`** | hashed, **zero** secret material outside the 201 response | ❌ no (re-mint instead) | ✅ no | **CHOSEN** |
| **E. Any of the above + `SHARE_LINKS_ENABLED` flag** | — | — | — | Rejected: the SPA cannot discover the flag without a shape change to `GET /api/endpoints/{token}`; nginx `return 404` is the equivalent, contract-free opt-out (§2.9) |

**Why D.** (1) D10 forces a non-secret handle regardless, and once you have `id` the *only* thing
plaintext storage buys is "re-copy an existing link" — which "revoke + mint again" covers, with a
`SHARE_MAX_PER_ENDPOINT` of 10 to work in. (2) `sha256` of a 192-bit CSPRNG value is not brute-forcible,
so hashing costs nothing at lookup: the resolver is still a single exact-match probe on a UNIQUE index.
(3) It matches the codebase's existing precedent for bearer credentials — `owners.secret_hash`
**[existing — verified at `backend/migrations/0001_init.sql:11`, `backend/src/auth.rs:42-48`]** — and
reuses `ids::hash_secret` verbatim **[existing — verified at `backend/src/ids.rs:57-60`]**, so a
security review has nothing left to override. (4) It removes a whole failure class: a leaked backup,
replica or `.db` file copied off the box yields **no working URL**. (5) The cost is a UX change to
AC-25 that is free *right now*, because `journey.md` and `ux.md` for this feature do not exist yet.

**Code generation (AC-31/AC-32)** — one new function in `backend/src/ids.rs`, reusing the verified
CSPRNG path **[existing — verified at `backend/src/ids.rs:41-45`]**:

```rust
/// Public share-link code: `n_bytes` of CSPRNG, base64url no-pad. Default 24
/// bytes ⇒ 32 chars ⇒ 192 bits. Deliberately independent of the endpoint token,
/// owner id, owner secret, endpoint name and the clock (AC-32) — it is pure
/// randomness with no derivation. Stored only as `hash_secret(code)`.
pub fn gen_share_code(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes.max(16)];        // 128-bit floor
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}
```

Unit tests: length ≥ 32 for the default; charset ⊆ `[A-Za-z0-9_-]`; 10 000 codes are all distinct;
`gen_share_code(1)` still yields ≥ 22 chars (the clamp holds); a generated code shares no ≥4-char
substring with a `gen_token(10)` value (AC-32).

### 3.3 Storage growth (R4 confirmation against the retention sweep)

F7 lets a trace row hold up to **2 × `MAX_BODY_BYTES`** of body text instead of 1 ×. Bounded by
`TRACE_CAP = 100` rows per endpoint and `TRACE_TTL_HOURS = 24` **[existing — verified at
`backend/src/config.rs:156-157`]**, enforced both at write time **[existing — verified at
`backend/src/db.rs:91-103`]** and by the periodic sweep **[existing — verified at
`backend/src/tasks/sweep.rs`, steps (a)]**. Worst case per endpoint therefore roughly doubles from
~25 MB to ~50 MB, and the existing sweep already reclaims it with no change. `share_links` adds ≤ 10
rows × ~200 bytes per endpoint — negligible, and hard-deleted by CASCADE when the sweep reaps a
tombstone.

---

## 4. Component & file design

### 4.1 Backend — new files

| File | Responsibility |
|---|---|
| `backend/migrations/0002_share_links.sql` **[new]** | §2.4 DDL. Additive; `0001` untouched. |
| `backend/src/routes/share.rs` **[new, ~320 lines]** | **The entire F4 route surface in one auditable file.** `share_router() -> Router<AppState>`; the three owner handlers (#19–21); the two public handlers (#22–23); `share_url`, `share_not_found`, `is_share_code_shape`, the `last_used_at` touch, and the row→`Public*` projection functions. Module doc states the trust boundary, the frozen check order, and the "no mutation, no owner secret, no token" invariants. |

### 4.2 Backend — changed files

| File | Change | Feature |
|---|---|---|
| `backend/src/interceptor/engine.rs` **[existing]** | Add `capture_response_body` (§2.10.1); delete the local `truncate` closure at `:646-653`; `spawn_trace` gains `resp_body: &[u8]` (§2.10.4); four call sites capture, one passes `&[]` (§2.10.5); `response_body: None` at `:683` becomes `helpers::body_for_trace(resp_body, cap)`; `resp_headers` at `:636-644` becomes `helpers::response_headers_for_trace(resp.headers())`. **The only file in `backend/src/interceptor/` that F7 touches.** | F7 |
| `backend/src/helpers.rs` **[existing]** | Add `truncate_utf8`, `body_for_trace` (§2.10.2) and `response_headers_for_trace` (§2.10.3, the R11 seam). Adds `use axum::http::HeaderMap;`. | F7 |
| `backend/src/models.rs` **[existing]** | Add `ShareLinkCreate`, `ShareLink`, `ShareLinkCreated`, `PublicShareFeed`, `PublicEndpointInfo`, `PublicRequestSummary`, `PublicRequestDetail` as **standalone** structs (§2.5). Nothing changes for F7. | F4 |
| `backend/src/ids.rs` **[existing]** | Add `gen_share_code` (§3.2). | F4 |
| `backend/src/config.rs` **[existing]** | Add the three §2.9 fields + defaults + the `>= 16` clamp. | F4 |
| `backend/src/routes/api.rs` **[existing]** | **Exactly two edits.** (1) `fn effective_client_ip` → `pub(crate) fn` **[`:273`]**. (2) `delete_endpoint` gains the revoke-shares statement **[`:522-526`]**. `api_router()` is **not** changed. | F4 |
| `backend/src/routes/mod.rs` **[existing]** | `pub mod share;` + `pub use share::share_router;`. | F4 |
| `backend/src/router.rs` **[existing]** | `.merge(share_router())` next to `.merge(api_router())` **[`:180`]**. | F4 |
| `backend/tests/api.rs` **[existing]** | New integration coverage (§4.5). | F4, F7 |

**Untouched, explicitly:** `backend/Cargo.toml`, `backend/src/db.rs`, `backend/src/limiter.rs`,
`backend/src/auth.rs`, `backend/src/error.rs`, `backend/src/planes.rs`, `backend/src/rule_cache.rs`,
`backend/src/tasks/sweep.rs`, `backend/src/routes/tunnel_ws.rs`, `backend/src/interceptor/proxy.rs`,
`backend/src/interceptor/matcher.rs`, and every other file under `backend/src/interceptor/`.

### 4.3 Frontend — new files

| File | Responsibility |
|---|---|
| `src/lib/csv.ts` **[new, ~70 lines]** | **Pure**, DOM-free RFC 4180 serializer: `escapeCell(value: string): string` (guard-then-quote, §2.7), `toCsv(header: readonly string[], rows: readonly (readonly string[])[]): string` (CRLF + trailing CRLF). Unit-testable without a browser (AC-54/AC-55). |
| `src/lib/request-export.ts` **[new, ~120 lines]** | The F5 orchestrator: `EXPORT_CONCURRENCY = 4`, `fetchDetails(...)` (§2.7), `buildRequestCsv(rows, details)`, `exportFilename(token, now)`, and `downloadCsv(csv, filename)` (Blob → object URL → click → `revokeObjectURL`). Keeps `dashboard.tsx` from growing. **Not in the PRD's §7 — added, see §8.** |
| `src/lib/config-bundle.ts` **[new, ~120 lines]** | `configBundleSchema` + `CONFIG_BUNDLE_VERSION = 1`, `buildBundle(endpoint: EndpointDetail, rules: MockRule[]): ConfigBundle` (the `MockRule → MockRuleCreate` projection, AC-14), `parseBundle(text: string): Result<ConfigBundle, string>` returning a human message naming the first failing field/index (AC-16), `MAX_BUNDLE_BYTES = 5_000_000`, `MAX_BUNDLE_RULES = 200`, and the `_assignable` type guard (§2.5.6). |
| `src/screens/share-view.tsx` **[new, ~280 lines]** | The public `/s/:code` viewer. **Imports neither `AppShell` nor `session`.** Six documented states (AC-44): loading · empty · list+detail · unavailable(404) · rate-limited(429, shows `Retry-After` seconds) · error(+Retry). 5 s polling gated on `document.visibilityState === 'visible'`, cleared on `visibilitychange` → hidden and on unmount (AC-45). Reuses the read-only presentational primitives — `MethodBadge`, `StatusCode`, `ServedByChip`, `KeyValueRows`, `JsonTree`, `CodeBlock` — so every value is a text node (AC-67). |
| `src/components/hookbox/share-dialog.tsx` **[new, ~220 lines]** | Owner mint/list/revoke dialog on the existing `Dialog` primitives **[existing — verified at `src/components/ui/dialog.tsx`]**. Lists `ShareLink[]` (id/label/created/last-used); "Create share link" → on 201 shows the **one-time** URL with a copy affordance and an explicit "this link is shown only once" note; Revoke per row → `DELETE …/shares/{id}` → optimistic row removal; surfaces the 422 cap message inline (AC-27). |

### 4.4 Frontend — changed files

| File | Change | Feature |
|---|---|---|
| `src/api/schemas.ts` **[existing]** | Add `shareLinkCreateSchema`, `shareLinkSchema`, `shareLinkCreatedSchema`, `publicRequestSummarySchema`, `publicRequestDetailSchema`, `publicShareFeedSchema` + inferred types. **No change for F7** — `response_body: z.string().nullable()` already exists **[`:169`]**. | F4 |
| `src/api/client.ts` **[existing]** | Add five methods: `createShare(token, payload)`, `listShares(token)`, `revokeShare(token, id)` (Bearer, default path) and `getSharedRequests(code, params)`, `getSharedRequest(code, id)` with **`noAuth: true`** (AC-42 — this is what stops a share 401 from clearing a real session at `:99-105`). Public paths use `encodeURIComponent(code)`. | F4 |
| `src/router.tsx` **[existing]** | `{ path: "/s/:code", element: <ShareView /> }` before the `*` fallback **[`:35-49`]**. | F4 |
| `src/feed/use-feed.ts` **[existing]** | Add `clearRows(): void` to `UseFeedResult` **[`:55-66`]**: `setRows([])`, `buffer.current = []`, `setNewCount(0)` — one function covering AC-4 and AC-5 including the paused case. | F1 |
| `src/screens/dashboard.tsx` **[existing]** | `FeedPane` header action group **[`:325-350`]** gains **Clear all** (leftmost, `variant="danger"` confirm dialog) and **Export CSV**, both `disabled={rows.length === 0}`. `DashboardLoaded` wires `clearRows` and passes `token` down to `FeedPane`. | F1, F5 |
| `src/components/hookbox/app-shell.tsx` **[existing]** | Delete the `dash.pathUrl.label` chip **[`:101`]**; add the Share control + `<ShareDialog>` mount immediately after the Mock URL chip **[`:100`]**; fix the module doc-comment **[`:4-5`]** which still says "mock URL + local path". | F2, F4 |
| `src/screens/settings.tsx` **[existing]** | New `<Section title={t("set.config.title")}>` (the existing `Section` helper **[verified at `:707-720`]**) with Export config + Import config. The Identity block's Local path `CodeBlock` **[`:256-262`]** is **byte-unchanged** (AC-8). | F3 |
| `src/screens/rules-manager.tsx` **[existing]** | "Add default rule" in the toolbar next to "New rule" **[`:191-193`]** **and** in the empty state **[`:218-227`]**; disabled + tooltip when `rules.some(r => r.enabled && r.match.method === 'ANY' && r.match.path === '/*')` (AC-61); on 201 reload + success toast. | F6 |
| `src/lib/copy.ts` **[existing]** | New keys for F1/F3/F4/F5/F6. `dash.pathUrl.*` **[`:65-66`]** stay (they mirror `copy.md` 1:1 and simply become unreferenced — AC-10). | all FE |
| `src/api/index.ts` **[existing]** | Nothing (it already re-exports `* from './schemas'`). | — |

### 4.5 Tests & deploy

| File | Change |
|---|---|
| `backend/tests/api.rs` **[existing]** | **F4:** mint/list/revoke happy path; 401 (missing/malformed/unknown Bearer) × 3 routes; 404-not-403 for a foreign token × 3 routes; the cap 422; the label 422; tombstone-revokes-all + every prior URL 404s; **byte-identical 404** for unknown vs revoked vs tombstoned (compare status line + body bytes + headers); cross-endpoint detail id → 404; `limit=999` → 422 for **both** a valid and an invalid code (the D14/§2.2 oracle test); 429 + `Retry-After` past the limit; `no-store` on 200/404/422/429; POST/PATCH/PUT/DELETE on both public paths → 405 with the five row counts unchanged; the public JSON key-set allow-list + "token is not a substring" assertion. **F7:** the eight-row `served_by` capture matrix (AC-68); the three `NULL` cases (AC-69); truncation (a)–(d) incl. the mid-multibyte no-panic case (AC-70); the `0x80 0xFF 0xFE` lossy case with byte-exact client bytes (AC-71); golden status+sorted-headers+body fixtures for 7 paths (AC-72); (d1)/(d2)/(d3) from §2.10.7; the `RequestDetail` key-set-unchanged assertion (AC-74); the `new_request` payload has no body keys (AC-75). |
| `backend/src/helpers.rs` `#[cfg(test)]` **[existing]** | Unit tests for `truncate_utf8` (boundary floor, no panic, exact-cap, `cap = 0`) and `body_for_trace` (empty ⇒ `None`, never `Some("")`). |
| `backend/src/ids.rs` `#[cfg(test)]` **[existing]** | `gen_share_code` entropy/charset/uniqueness/clamp/non-derivation (AC-31/32). |
| `e2e/mock-backend.ts` **[existing]** | Route stubs for `POST/GET /api/endpoints/:token/shares`, `DELETE …/shares/:id`, `GET /api/share/:code/requests[/:id]`, plus fault-injection switches for the F3 partial-failure and F5 per-row-404 paths. Add to the `**/api/**` handler **[`:238`]**. |
| `e2e/visual.spec.ts`, `e2e/states.spec.ts`, `e2e/journeys.spec.ts` **[existing]** | Update the sub-header chip-count assertions (AC-7); add the `/s/:code` state specs; add the "zero `Authorization` header from `/s/:code`" and "zero `ws://`/`wss://`/`text/event-stream` from `/s/:code`" network assertions (AC-42/AC-45). |
| `deploy/nginx.conf` **[existing]** | The two new locations from §2.11. |

---

## 5. Sequences

### S1 — F7: a rule-served mock request (the hot path)

```
client → nginx /e/{token}/hook → plane_dispatch (router.rs:152-173) → Plane::Mock
  → run_interceptor buffers the capped request body (router.rs:96-136)
  → engine::handle_mock (engine.rs:39)
     1  rule_cache.get → Resolved::Live(ep)                     [no DB read on a warm cache]
     2  body_text = String::from_utf8_lossy(&body)              [engine.rs:73]
     3  matcher::select → Some(rule)
     4  body_out = templating::render(...)                      [String, owned]
     5  resp = Response::builder().body(Body::from(body_out.clone()))   [single-frame Bytes]
     6  conditions: rate-limit pass → chaos None → latency 0
     7  status = resp.status()                                  [engine.rs:359, BEFORE identified]
     8  resp = identified(resp, ...)                            [headers only — D2]
  ►  9  (resp, rb) = capture_response_body(resp).await          [NEW: refcount bump, no copy]
    10  spawn_trace(..., &resp, &rb, ...)
          duration_ms/overhead_ms                               [engine.rs:632-633]
          request_headers  = redact(req_headers)                [unchanged]
          response_headers = helpers::response_headers_for_trace(resp.headers())   [R11 seam]
          request_body     = helpers::body_for_trace(req_body.as_bytes(), cap)     [same helper]
          response_body    = helpers::body_for_trace(&rb, cap)                     [NEW]
          tokio::spawn { db::insert_trace (1 stmt + prune) ; feed_hub.publish(new_request) }
    11  return resp                                             [byte-identical to before — AC-72]
```

Nothing is awaited between step 9 and step 11 that can block; step 10's DB work stays inside the
existing `tokio::spawn` **[existing — verified at `engine.rs:699-704`]**, so AC-73(a)/(c) hold by
construction.

### S2 — F4: mint a share link

```
Share dialog → POST /api/endpoints/{token}/shares  { label: "Acme vendor" }
  OwnerId extractor (auth.rs:114-131)                         → 401 on a bad Bearer
  assert_owns_endpoint (auth.rs:55-69)                         → 404 for unknown / foreign token
  SELECT gone_at                                               → 404 if tombstoned
  label trim + <= 80 chars                                     → 422 otherwise
  SELECT COUNT(*) … revoked_at IS NULL >= share_max_per_endpoint → 422 (AC-27)
  code = ids::gen_share_code(cfg.share_code_bytes)              [192 bits, plaintext, in memory only]
  INSERT (code_hash = hash_secret(code), token, label) RETURNING id, created_at
  201 { id, code, url: share_url(state, &code), label, created_at, last_used_at: null }
      + Cache-Control: no-store
  → dialog shows the URL + copy + "shown only once"; the plaintext code is now unrecoverable server-side
```

### S3 — F4: the viewer resolves a share (list, then detail)

```
GET /api/share/{code}/requests?limit=50            [no Authorization header — AC-42]
  1 limiter.check("share:{effective_client_ip}", 120, 60)      → 429 + Retry-After
  2 limit in 1..200 && offset >= 0                             → 422   [BEFORE the code lookup — no oracle]
  3 is_share_code_shape(code)                                  → share_not_found(), no DB read
  4 SELECT s.id, s.token, e.name, e.created_at, e.request_count
      FROM share_links s JOIN endpoints e ON e.token = s.token
     WHERE s.code_hash = sha256(code) AND s.revoked_at IS NULL AND e.gone_at IS NULL
                                                               → share_not_found() on any miss
  5 SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?
  6 200 PublicShareFeed + Cache-Control: no-store              [projection built field-by-field]
  7 tokio::spawn { UPDATE share_links SET last_used_at … coalesced to 60s }   [off the response path]

GET /api/share/{code}/requests/{id}
  steps 1, 3, 4 as above, then
  5' SELECT * FROM request_logs WHERE id = ? AND token = ?      ← AC-35 in one statement
  6' 200 PublicRequestDetail + no-store   (token / matched_rule_id / overhead_ms / trace /
                                           state_snapshot are never constructed, let alone serialised)
```

### S4 — F4: revoke takes effect on the next request

```
Share dialog → DELETE /api/endpoints/{token}/shares/{id}      [id, NOT the code — D10]
  OwnerId → assert_owns_endpoint
  UPDATE share_links SET revoked_at = datetime('now') WHERE id=? AND token=? AND revoked_at IS NULL
  rows_affected == 0 → 404 ; else 204
  → the very next public GET fails step 4 (revoked_at IS NOT NULL) and returns the identical 404.
    No cache is involved: share_links is read per request and rule_cache holds no share state
    [verified at backend/src/rule_cache.rs:1-8]. Every public response is Cache-Control: no-store.
```

### S5 — F3: import with a mid-sequence rule failure

```
file input → File.size <= 5 MB → text() → JSON.parse → configBundleSchema.safeParse
   any failure → one human message naming the first bad field/index; ZERO network calls (AC-16)
valid →
   PATCH /api/endpoints/{token}  <bundle.endpoint>     (all nine keys)
       failure → "Config was not applied; no rules were created." + server detail; STOP
   for i in 0..n:  POST /api/endpoints/{token}/rules  <bundle.rules[i]>
       progress "Creating rule {i+1} of {n}…"
       failure at k → STOP. Message states: config applied · k-1 of n rules created ·
                      failing index k+1 and its name · the server's `detail` ·
                      "no rule after it was attempted" · nothing was rolled back   (AC-19)
   done → re-fetch GET /api/endpoints/{token}; rules count now old + created  (AC-18/AC-20)
```

### S6 — F5: CSV export

```
Export CSV (enabled only when rows.length > 0)
  rows = the visible feed rows, newest first (already the useFeed order)
  controller = new AbortController()
  fetchDetails(rows, controller.signal, onProgress):
      4 workers pull from a shared cursor; each result is written to results[index]
      per-row outcome: ok(detail) | pending(404) | unavailable(other)
      401 → rethrow → abort the whole export, no file  (AC-53)
      onProgress(done, total) → "Exporting {done} of {total}…" + Cancel
  csv = toCsv(HEADER, rows.map((r, i) => cells(r, results[i])))     [§2.7 derivation table]
  downloadCsv(csv, exportFilename(token, new Date()))               [revokeObjectURL in finally]
  toast "Exported {n} rows ({m} without detail)."
Cancel → controller.abort() → no Blob, no object URL, no download   (AC-48)
```

### S7 — F1: clear all logs

```
Clear all (disabled when rows.length === 0)
  → confirm Dialog (DialogHeader / DialogBody naming the endpoint / ghost Cancel / danger Confirm)
  Cancel · Esc · overlay click → zero network calls, rows untouched                (AC-3)
  Confirm → DELETE /api/endpoints/{token}/requests   [existing, api.rs:878-889]
      2xx  → close dialog · success toast · feed.clearRows()  (rows [] + buffer [] + newCount 0,
             so the empty state renders immediately without waiting for a poll/WS) (AC-4/AC-5)
      else → dialog stays open · danger toast · rows untouched                     (AC-6)
```

---

## 6. FE / BE work split

Each lane builds against §2 alone. **No lane can change a shape another lane depends on.**

### Backend lane (Rust) — 3 independent issues

| Issue | Scope | ACs | Files | Depends on |
|---|---|---|---|---|
| **BE-1 · F4 share links (server)** | Migration `0002`, `share.rs` (all 5 routes), `models.rs` public+owner structs, `ids::gen_share_code`, the three config vars, `api.rs`'s two edits, `mod.rs`/`router.rs` wiring, integration tests | AC-23…AC-40 (server halves) | §4.1, §4.2 | nothing |
| **BE-2 · F7 response-body capture** | `capture_response_body`, the three `helpers.rs` functions, `spawn_trace`'s new param, the five call sites, the full test matrix | AC-68…AC-75 | `engine.rs`, `helpers.rs`, `backend/tests/api.rs` | nothing |
| **BE-3 · deploy** | The two nginx locations | §2.11 | `deploy/nginx.conf` | nothing (can land before BE-1) |

**BE-2 has no frontend counterpart at all** (AC-74). Both `GET /api/requests/{id}` consumers already
render `response_body` when non-null and its empty state when null **[existing — verified at
`src/screens/dashboard/inspector.tsx:246-253`]**, and the e2e mock backend already stubs a value
**[existing — verified at `e2e/mock-backend.ts:139`]**. BE-1 and BE-2 touch **disjoint** files.

### Frontend lane (React/TS) — 6 independent issues

| Issue | Scope | ACs | Files | Depends on |
|---|---|---|---|---|
| **FE-1 · F1 clear all** | `clearRows` in `use-feed.ts`; Clear all button + confirm dialog in `FeedPane`; copy keys | AC-1…AC-6 | `use-feed.ts`, `dashboard.tsx`, `copy.ts` | nothing (route exists) |
| **FE-2 · F2 remove the path chip** | Delete `app-shell.tsx:101`; fix the doc-comment; update the affected e2e assertions | AC-7…AC-10 | `app-shell.tsx`, `e2e/*.spec.ts` | nothing |
| **FE-3 · F3 export/import** | `config-bundle.ts`; the Configuration `Section` in `settings.tsx`; copy keys | AC-11…AC-22 | `config-bundle.ts` **[new]**, `settings.tsx`, `copy.ts` | nothing (routes exist) |
| **FE-4 · F4 share UI** | schemas + 5 client methods; `share-dialog.tsx`; `share-view.tsx`; the `/s/:code` route; the Share control in `app-shell.tsx`; copy keys; e2e stubs + specs | AC-41…AC-45 + the owner-dialog halves of AC-23…AC-27 | `schemas.ts`, `client.ts`, `router.tsx`, `share-dialog.tsx` **[new]**, `share-view.tsx` **[new]**, `app-shell.tsx`, `copy.ts`, `e2e/*` | §2 contract only — **can be built against `e2e/mock-backend.ts` stubs before BE-1 lands** |
| **FE-5 · F5 CSV export** | `csv.ts`; `request-export.ts`; the Export CSV button + progress/cancel in `FeedPane`; copy keys | AC-46…AC-56a | `csv.ts` **[new]**, `request-export.ts` **[new]**, `dashboard.tsx`, `copy.ts` | nothing — the `response_body` **column** works whether BE-2 has landed or not (null ⇒ empty cell) |
| **FE-6 · F6 default rule** | "Add default rule" in the toolbar + empty state; the duplicate guard; copy keys | AC-57…AC-63 | `rules-manager.tsx`, `copy.ts` | nothing (route exists) |

**Coordination points, and only these three:**
1. **FE-1 and FE-5 both edit `FeedPane`'s action group** (`dashboard.tsx:325-350`). Sequence them, or
   land FE-1 first and have FE-5 rebase — it is a ~10-line region.
2. **FE-2 and FE-4 both edit `app-shell.tsx`'s sub-header** (`:98-118`). Same treatment; FE-2 is a
   deletion and should land first.
3. **FE-4 ⇄ BE-1 share only §2.** FE-4 develops against `e2e/mock-backend.ts` stubs. The QA gate owns
   the real integration.
4. **AC-56a and AC-44a are end-to-end assertions** (real content in the CSV / the public detail) and
   belong to the **QA gate**, which is blocked on BE-1 + BE-2 + FE-4 + FE-5 — not on any single lane.

---

## 7. Technical risks

**R1 — F4 is a new internet-reachable read surface (highest).** Mitigations are structural, not
intentions: 192-bit CSPRNG codes stored hashed (§3.2); the projection is built field-by-field from
standalone structs so a future owner-shape field cannot leak by default (§2.5.5); AC-35 is enforced
inside a single `WHERE id = ? AND token = ?`; all five negative outcomes funnel through one
`share_not_found()`; parameter validation precedes code resolution so `?limit=999` is not an existence
oracle (§2.2); revocation is a per-request DB read with no cache and `no-store`; the resolver is
per-IP rate limited; no verb but GET is routed; the code never appears in an owner-route URL or a log.
security.md owns the full threat model.

**R2 — Shared links expose payloads by design, and F7 widens what "payload" means.** A link now
exposes request bodies, request headers (redacted only for `authorization` / `cookie` /
`x-owner-id` — a bearer token in a custom header or a JSON body is shared verbatim), response headers
**un-redacted** (R11), **and** response bodies including anything the operator hard-coded into a rule
`body_template` or that a MITM/tunnel upstream returned. ux.md must make this explicit **at the mint
step**, naming bodies *and* responses. Partial comfort: MITM already strips upstream `Set-Cookie`
before the response exists **[existing — verified at `backend/src/interceptor/proxy.rs:27-44`]**, so
the un-redacted exposure is narrower than it first looks — it is tunnel replies and rule-authored
response headers.

**R3 — resolved, not a risk.** §3.2 settles the storage question (hashed + non-secret `id`) with a
recorded trade-off table. If security.md still wants a change, it is a `code_hash`-only tweak; the
`id`-based revoke route must survive regardless (D10).

**R4 — F7 on the mock plane's hot path (second-highest).**
- *Latency*: capture is a `size_hint` read, a refcount bump and a `Response::from_parts` — the only
  copy is the ≤256 KB truncated `String` the row needed anyway. AC-73(a)/(b)/(c) plus the rewritten
  (d1)/(d2)/(d3) (§2.10.7) are the gates.
- *Memory*: **no extra full-size retention.** The `Bytes` is shared with the outgoing response, and
  what moves into the spawned task is the already-truncated `Option<String>` (≤ `MAX_BODY_BYTES`), so
  the 5 MB MITM worst case is not held past the response — this is precisely what R4 asked the
  architect to specify. The one transient extra allocation is `String::from_utf8_lossy` for a body
  that is **not** valid UTF-8 (valid UTF-8 borrows), bounded by `MITM_MAX_BODY_BYTES` and freed before
  the response returns.
- *Storage*: ~2× per trace row, bounded by `TRACE_CAP` × `TRACE_TTL_HOURS`; the existing sweep
  reclaims it with no change (§3.3).
- *Exposure*: feeds R2 and R11; security.md owns the call, and D7 names the exact one-function seam.
- *Silent truncation is ambiguous (accepted)*: symmetric-and-silent beats asymmetric-and-explicit for
  a narrow fix; `len == MAX_BODY_BYTES` is a usable heuristic until someone builds a real "truncated"
  affordance.

**R-DROPOUT — the chaos-dropout trace is a pre-existing low-fidelity row, and F7 does not fix it.**
`spawn_trace` receives a throwaway empty `Response`, so the row records `status_code = 0` and
`response_headers = {}` while the client actually receives `499` + `Connection: close`
**[existing — verified at `engine.rs:299-321`]**. F7 passes `&[]` here, so `response_body` is `NULL`,
which is the correct answer per AC-69. Fixing the other two fields would change already-persisted
values outside F7's frozen scope and would still not make `status_code` truthful (0 is a deliberate
"connection dropped" marker). Recommendation: leave it, add a code comment, and file a separate issue.
**PM decision needed only if you disagree.**

**R5 — F3 import is not transactional.** SQLite gives no cross-request transaction and there is no
bulk endpoint, so a mid-import failure leaves a deterministic prefix applied. "Add, never replace,
never roll back, report precisely" is safer than any client-side rollback that could destroy
pre-existing rules.

**R6 — F5 issues up to 100 authenticated detail fetches per click.** Concurrency 4 + a Cancel path
keeps it from looking like a self-inflicted DoS on a small self-hosted box. Assumption verified: only
`POST /api/session` is rate limited today **[existing — verified at
`backend/src/routes/api.rs:211-214`, `:304-326`]**; if a limiter is ever added to
`GET /api/requests/{id}`, `request-export.ts` must grow 429 + `Retry-After` handling — the single
place to change.

**R7 — F1 is irreversible.** `clear_requests` hard-deletes **[existing — verified at
`backend/src/routes/api.rs:884-887`]**. A plain confirm is proportionate: traces are already
ephemeral (100-row cap, 24 h TTL) and the typed-token confirm is reserved for endpoint deletion
**[existing — verified at `src/screens/settings.tsx:575-654`]**.

**R8 — F6 changes an endpoint's default answer from 404 to 200**, which can mask a real client
misconfiguration. Opt-in per endpoint, never auto-created, `priority = 1000` so every real rule wins,
an obviously-placeholder body, plus the duplicate guard.

**R9 — F2 removes an affordance.** Mitigated by keeping it on Settings → Identity, byte-unchanged
**[existing — verified at `src/screens/settings.tsx:256-262`]**.

**R10 — copy is placeholder-quality.** Keys are additive; design/ux own final wording; a wording
change is not a contract change.

**R11 — response headers are persisted un-redacted, and F4 now publishes them.** Verified asymmetry:
`redact()` is applied to request headers only **[`engine.rs:674`]**; the response map is collected
verbatim **[`engine.rs:636-644`]**. Default carried forward: keep today's behavior, documented
truthfully. **If security.md overrides, the entire change is the body of
`helpers::response_headers_for_trace` plus a `REDACT_RESPONSE_HEADERS` const (§2.10.3)** — one
function, one call site, no §2 shape change, no migration, no re-architecture. That was the explicit
design requirement and it is met.

**R12 — today's truncation can panic on a multibyte boundary.** `s[..cap]` panics when byte `cap`
falls inside a UTF-8 character **[existing — verified at `engine.rs:646-653`]**. Reachable today for
`request_body`; F7 would make it reachable for upstream/CLI-controlled `response_body`.
`helpers::truncate_utf8` fixes it for both columns and is tested explicitly (AC-70(b)/(d)). This is a
**required** part of F7, not optional cleanup.

**R13 — new: axum's auto-405 is not our envelope.** AC-39's 405s come from `MethodRouter` and carry an
empty body and no `no-store` (D14). Accepted: consistent with every other route in the app, and a 405
discloses nothing. Alternative (a `map_response` layer over the share router) is available if
security.md wants uniformity.

**R14 — new: a NAT'd audience shares one rate-limit bucket.** `effective_client_ip` resolves the first
`X-Forwarded-For` hop / `X-Real-IP` **[existing — verified at `backend/src/routes/api.rs:273-299`]**,
so several viewers behind one corporate egress IP share a bucket. Default raised to 120/min (D15).
The viewer's 429 state shows the `Retry-After` seconds (AC-44) rather than looking broken.

---

## 8. Gaps the PM must resolve on REVISE

Ordered by blocking-ness. Items 1–5 change ACs or §5 shapes.

1. **D9 + D10 + D11 — the share-code storage and revoke-route change.** §5.4 becomes the §2.4 DDL
   (`id` + `code_hash`); §5.1's DELETE becomes `…/shares/{id}`; §5.5.2 drops `code`/`url`; §5.5.3 adds
   `id` and is the sole carrier of `code`/`url`; **AC-25 must be rewritten** (list rows show
   id/label/created-at/last-used, no URL, no copy) and **AC-26 must say "by id"**. AC-31/AC-32 survive
   verbatim; add an AC that the plaintext code is stored nowhere and appears in exactly one response.
2. **D8 — AC-73(d).** Replace the `max(2 ms, 10 %)` / `p95 ≤ 5 ms` conditions with §2.10.7's
   (d1)/(d2)/(d3). As written the AC cannot fail, which is worse than having no AC.
3. **D13 — AC-13 / §5.5.6.** Drop "built by composing the existing `endpointConfigPatchSchema`"; the
   bundle's `endpoint` object is a separate `.strict()` schema with all nine fields **required**.
4. **D12 — AC-55.** Drop "(quoted per AC-54)": the expected cell is the unquoted
   `'=cmd|' /c calc'!A1`. Also freeze guard-then-quote ordering and the trailing CRLF.
5. **D14 — AC-37 / AC-39.** Narrow "every public response carries `Cache-Control: no-store`" to
   handler-produced responses (200/404/422/429/503); note that the auto-405 carries neither the header
   nor the flat envelope, and that this is accepted.
6. **D15 — §5.8.** `SHARE_RATE_LIMIT_PER_MIN` default 60 → **120**, with the arithmetic recorded.
7. **New ACs I recommend adding** (each is already specified above and testable):
   (a) minting a share on a **tombstoned** endpoint returns 404 (§2.1 #19 step 3);
   (b) `?limit=999` returns **422 for both a valid and an invalid code** — the anti-oracle ordering
   test (§2.2);
   (c) the public detail projection is built from **standalone** structs, asserted by a key-set test,
   so a future owner-shape field cannot leak (§2.5.5);
   (d) `last_used_at` writes are coalesced to ≥ 60 s and happen off the response path (§2.2);
   (e) `SHARE_CODE_BYTES` is clamped to ≥ 16 so a misconfiguration cannot mint a guessable code (§2.9).
8. **§7 file list.** Add `src/lib/request-export.ts` **[new]** (the F5 orchestrator; `csv.ts` stays a
   pure serializer). §6's "possibly affected by F7" list resolves to: `backend/src/helpers.rs`
   **is** touched (three functions), `backend/src/routes/tunnel_ws.rs` is **not** (mechanism (i)).
9. **R-DROPOUT.** Confirm "leave the chaos-dropout trace's `status_code = 0` / `response_headers = {}`
   as-is, file a follow-up" — or tell me to fix it and I will re-scope §2.10.5.
10. **§2.9's no-feature-flag decision.** Confirm that the deploy-level opt-out
    (`location /api/share/ { return 404; }`) is the accepted way to disable public sharing on an
    instance, rather than a `SHARE_LINKS_ENABLED` env var (which would require a shape change to
    `GET /api/endpoints/{token}` for the SPA to hide the control).
11. **For ux.md, flagged not decided:** `PublicShareFeed.endpoint.name` is operator-authored and is
    shown to an unauthenticated viewer (e.g. "acme-prod-billing-webhook"). It is in the PRD's §5.5.4
    and I have kept it, but the mint step should say that the endpoint **name** is visible too.
    Also: because links are identified by `label` + `created_at` in the list (D11), ux.md should treat
    `label` as strongly encouraged rather than incidental.
