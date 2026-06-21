# Security review (design): HookBox — Rust/Axum + SQLite Re-platform

- **Slug:** `hookbox-rust-replatform`
- **Mode:** DESIGN (Stage A — threat-model the design; no code yet)
- **Date:** 2026-06-21
- **Inputs read:** `prd.md` (esp. §4 ACs, §5 contract, §9 OQs), the locked spec
  (`docs/superpowers/specs/2026-06-21-hookbox-rust-replatform-design.md`),
  `FEATURES.md`, and the **verified Python posture** — `app/auth.py`,
  `app/planes.py`, `app/websocket.py`, `app/redis_state.py`,
  `app/interceptor/{proxy,cors,templating,engine,crud}.py`,
  `app/utils/helpers.py`, `app/routes/{api,tunnel}.py`, `config.py`.

> **Posture note.** The Python baseline is already security-hardened (owner
> capability, resolved-IP SSRF guard + IP pinning, sandboxed scanner, redaction,
> caps). The re-platform's **dominant risk is not designing new controls — it is
> *silently dropping an existing control* during a faithful re-write in a new
> language with a different concurrency and DNS/TLS stack.** Every "[existing —
> verified]" AC is a control that must be re-proven on Rust, not assumed.

---

## Threat model — assets, trust boundaries, who can reach what

### Assets
1. **Owner capabilities** (256-bit bearer secrets) — the *only* credential.
   Stored as `sha256` at rest (`capability_hash`). Compromise = full account
   takeover (read all traces, mutate rules, bind tunnels, drive the MITM proxy).
2. **`owner_id`** (`sha256(lower(trim(email)))[:16]`) — non-secret, must never be
   accepted as a credential.
3. **Captured traffic** (`traces`: request/response headers, bodies, query) —
   contains third parties' secrets by nature (this is an inspector). Cross-owner
   read = data breach.
4. **Endpoint config** (`target_url`, rules, state, CRUD collections).
5. **The host's network position** — the MITM proxy and (any) backup/replay
   fetcher are an SSRF cannon pointed at the operator's internal network and
   cloud metadata.
6. **The single process** — no Redis to absorb load; in-process buckets/broadcast/
   tunnel-registry/SQLite are the only line against resource exhaustion.

### Trust boundaries (the three planes, decided from `Host` + path)
- **P1 mock plane** — `<token>.<MOCK_DOMAIN>/<path>` + `/e/<token>/<path>`.
  **Fully hostile, unauthenticated, internet-facing.** Arbitrary method/path/
  headers/body. The entire interceptor pipeline runs here. **This is the primary
  attack surface.**
- **P2 management API** — `/api/**`, capability-gated (except `POST /api/session`).
  Anonymous can only mint sessions.
- **P3 dashboard** — SPA assets + WS/SSE feed + `/healthz`. Feed is cap-gated.
- **Tunnel control channel** — `/ws/tunnel/{slug}`, cap-gated bind.

The **critical isolation invariant**: the P1 catch-all must *never* shadow P2/P3.
A bug that lets a mock token named `api` or a crafted `Host`/path serve the mock
plane on `/api/**` would expose the management surface or let P1 traffic be
mistaken for authenticated calls. The Python `resolve_plane` ordering (mock host →
`/e/` fallback → `/api` → UI) and `subdomain_of` (single-label, app-host
exclusion, case-preserved) encode this; the Rust router must reproduce it
**exactly**, including the host-parsing edge cases below.

### Reachability summary
| Actor | Can reach | Must NOT reach |
|---|---|---|
| Anonymous internet | P1 (all), `POST /api/session`, SPA static, `/healthz` | any other `/api/**`, any feed, any tunnel bind, cross-owner data |
| Cap holder (owner) | own endpoints' `/api/**`, own feed, own tunnel bind | other owners' tokens (→ 404, not 403) |
| Leaked old cap (post-rotation) | nothing (must 401) | everything |
| MITM target chosen by owner | only public, non-internal IPs | loopback/private/link-local/metadata/cloud IMDS |

---

## Findings & risks

Severity reflects impact **if the control is dropped/regressed in the Rust port**.
Nothing here is "the design is wrong" — these are the controls the frozen contract
must *carry forward verbatim* and the re-review must prove.

### F1 — SSRF guard must be re-proven on the Rust DNS/TLS stack (CRITICAL)
- **What / where:** §5.5 MITM forward, AC-29/30/31; spec §8 "SSRF guard on the
  resolved IP." Python (`proxy.py::_resolve_and_check`/`_pin_target`) resolves the
  hostname, blocks every resolved IP (loopback/private/link-local/multicast/
  reserved/unspecified, incl. `169.254.169.254`), then **pins the connection to
  the validated IP literal** while preserving `Host` + TLS SNI — defeating
  DNS-rebinding TOCTOU.
- **Why exploitable if regressed:** `reqwest` resolves the URL host itself at
  connect time. A naive Rust port that validates `target_url`'s hostname and then
  hands the *same hostname* to `reqwest` has a **TOCTOU rebinding hole** (attacker
  DNS returns a public IP for the check, `127.0.0.1`/`169.254.169.254` for the
  connect) → read cloud IMDS credentials, hit internal services. This is the
  single highest-impact control in the product.
- **Rust-specific hazards to call out:** (a) pinning requires a custom
  `reqwest` resolver / `ClientBuilder::resolve(host, addr)` or connecting to the
  IP literal with SNI+Host overridden — must verify SNI/cert still bind to the
  original hostname for `https`; (b) **redirects**: `reqwest`'s built-in redirect
  follower bypasses per-hop revalidation — must set `redirect::Policy::none()`
  and follow manually, re-running the guard on every hop (AC-30); (c) IPv6
  (`::1`, `fe80::/10`, `fc00::/7`, IPv4-mapped `::ffff:127.0.0.1`) and zone-id
  stripping; (d) `MITM_ALLOW_PRIVATE` opt-out must default **false**.
- **Fix / contract requirement:** §5.5 must state the guard is evaluated on
  **every resolved IP** and the connection is **pinned to the validated IP**
  (no second resolution), redirects default off and re-validate per hop. Re-review
  will probe with a rebinding host, `169.254.169.254`, `[::1]`, `::ffff:169.254.169.254`,
  and a redirect→internal.

### F2 — Plane isolation under a new host parser (HIGH)
- **What / where:** AC-6/7/8/9, spec §5; `planes.py`. The mock catch-all must
  never shadow `/api/**` or the UI/feed.
- **Why exploitable if regressed:** Host-header parsing is fiddly and a frequent
  source of bypass. Risks specific to a Rust re-write: (a) Axum gives the path
  already-decoded — a percent-encoded `/e/%2e%2e` or `/api` under a mock host must
  resolve **on the mock host as the mock's own path** (not leak to P2); (b)
  multi-label subdomain `a.b.<MOCK_DOMAIN>` must be **non-token → UI**, not a
  token of `a.b`; (c) the app-host / apex / `localhost` / `127.0.0.1` / `[::1]`
  exclusions (`APP_HOSTS`) must hold so a token literally equal to the app host
  can't hijack the UI; (d) **token case is preserved** (mixed-case alphabet) while
  the domain suffix matches case-insensitively — lower-casing the label breaks
  addressing *and* could collide tokens.
- **Fix / contract requirement:** §5 freezes the resolution order and the
  `subdomain_of` rules (single-label, app-host-excluded, case-preserved suffix-
  insensitive). Re-review probes crafted `Host`/path combinations and confirms
  `/api/**` is unreachable from any mock host.

### F3 — Capability rotation & 401-vs-404 semantics (HIGH)
- **What / where:** AC-2/4/60, §5.1; `auth.py`, `database.py`.
- **Why it matters:** (a) On each `POST /api/session` the old hash must be
  **overwritten** so a leaked prior secret 401s immediately — a port that inserts
  a new row / keeps history leaves the old secret valid; (b) `owner_id` must never
  authenticate — only `capability_hash` is looked up; (c) **404 (not 403)** for a
  valid cap that doesn't own `{token}`, so a non-owner can't enumerate which
  tokens exist; (d) the hash lookup is by `secret_hash` — fine, but the Rust port
  must still avoid a **timing oracle** on the email/owner path (the `404` vs `401`
  distinction is by design, but DB-error vs not-found must be uniform).
- **Fix / contract requirement:** §5.1 freezes: rotate-overwrite, hash-only
  lookup, `401` for missing/malformed/unknown cap with `WWW-Authenticate: Bearer`,
  `404` for valid-but-non-owner, uniform `{error, detail}` bodies. Capability is
  generated from a **CSPRNG** (`OWNER_SECRET_BYTES`=32 → 256-bit), compared by
  hash (constant-time not strictly required since we compare hashes, but the port
  must not introduce an early-return string compare on the raw secret).

### F4 — WS/SSE feed + tunnel bind auth before any frame (HIGH)
- **What / where:** AC-42/49, §5.4/§5.8; `websocket.py`, `tunnel.py`.
- **Why exploitable if regressed:** WS upgrades commonly **skip the route-auth
  layer**. The cap arrives as `?cap=` (feed) / `Authorization: Bearer` (tunnel)
  and must be verified **before any data frame** and before broadcast
  registration. A port that accepts then registers, then checks, leaks the
  `hello`/`new_request` stream (cross-owner trace disclosure) or lets an
  unauthenticated tunnel bind hijack a slug. The owner check must confirm the cap
  **owns that specific token** (channel isolation) — not merely "is a valid cap."
- **Rust-specific:** Axum WS handlers run the closure *after* the 101 upgrade;
  the cap check must happen inside the handler before subscribing to the
  `broadcast` channel, and a refusal should send the close code on an accepted
  socket (the Python notes 4401/4409/1013 must survive — a pre-accept close in
  some stacks collapses to a 1006 the client can't distinguish).
- **`?cap=` in query is a leak vector:** caps in URLs land in access logs / proxy
  logs / browser history. **Contract note:** server logs must never log the query
  string of `/ws|/sse` with `cap`, and the access-log layer must redact `cap`.

### F5 — Templating sandbox: no SSTI, closed grammar (HIGH if regressed)
- **What / where:** AC-16/17/18, §5.7; `templating.py`.
- **Why it matters:** This is the one place attacker-controlled text becomes a
  "template." The whole safety property is that it is a **hand-written single-pass
  scanner over a closed tag set** — no `eval`, no general engine, unknown →
  literal. **The Rust port must NOT reach for a crate like `tera`/`handlebars`/
  `minijinja` for convenience** — that would reintroduce SSTI
  (`{{7*7}}`, `{{config}}`, object traversal) and is a critical regression.
- **Fix / contract requirement:** §5.7 freezes "hand-written scanner, no template
  engine over user text, unknown/malformed tags left literal, never errors the
  mock path, bounded by `TEMPLATE_MAX_SIZE`/`TEMPLATE_MAX_TAGS`." Re-review runs
  the SSTI probe set and confirms verbatim echo.

### F6 — Resource limits with NO Redis backstop (HIGH — DoS)
- **What / where:** AC-39/43/46/47/58, §5.5 caps; `engine.py`, `conditions.py`,
  `websocket.py`, `crud.py`, `config.py`.
- **Why it matters more now:** Redis previously absorbed buckets/feed memory.
  Now everything is **in one process**; a missing cap is a single-binary OOM/CPU
  kill. Controls that must survive: ingest body cap → `413` **before buffering**
  (Content-Length check + read backstop, `MAX_INGEST_BODY_BYTES`); trace
  truncation `MAX_BODY_BYTES`; per-endpoint trace cap (write-time prune, not just
  sweep); WS conn cap per endpoint (`WS_MAX_CONN_PER_ENDPOINT`) + per-send timeout
  + slow-client drop; CRUD item/size caps; latency clamp 0–10000; chaos
  drop-timeout bound; tunnel per-request timeout. **New Rust risk:** the in-memory
  rate-limit `DashMap` and the broadcast registry are themselves unbounded
  growth surfaces — a flood of distinct endpoints/rules can grow the bucket map;
  the broadcast channel's lagged-receiver behavior must drop, not buffer
  unboundedly.
- **Fix / contract requirement:** §5.5 caps are frozen and env-driven with safe
  defaults. Add explicit caps for: bucket-map entry count / TTL eviction (so the
  limiter map can't grow forever), and broadcast channel bounded capacity with
  lag-drop. Body cap must reject **before** full buffering.

### F7 — Sensitive-header stripping before MITM forward (HIGH)
- **What / where:** AC-31; `helpers.py::strip_forward_headers`, `proxy.py`.
- **Why exploitable if regressed:** The owner capability rides `Authorization` on
  P2 — but a mock request on P1 may also carry attacker/victim headers. Before any
  MITM forward, `authorization`, `cookie`, `x-owner-id`, `x-user-id`,
  `x-hookbox-cap` and hop-by-hop headers must be stripped so the capability and
  caller cookies never leak to the (owner-chosen, but possibly logging) upstream.
  Conversely upstream `set-cookie` / upstream-CORS / `transfer-encoding` /
  `content-length`/`-encoding` must be stripped from the captured response.
- **Fix / contract requirement:** §5.5 freezes the forward/response strip lists.

### F8 — Secret never logged / reflected / persisted (HIGH)
- **What / where:** AC-61, §5.1; `engine.py::_redact`, `redis_state` (no longer).
- **Why it matters:** Caps must never appear in: traces (`_redact` of
  `authorization`/`cookie`/`x-owner-id` before persist), the live feed payloads,
  error bodies, or **`tracing` logs**. Rust's `tracing` makes it easy to
  `?req.headers` a whole header map into a span — that would log caps. The `?cap=`
  query (F4) is the other leak path.
- **Fix / contract requirement:** §5 mandates redaction of the cap + auth/cookie
  headers everywhere they could surface; a structured-logging deny-list for
  `authorization`/`cookie`/`cap`/`x-owner-id`. Echo/`request.header.*` templating
  and the `echo` default mode must NOT expose the cap (it isn't sent to P1 by the
  owner, but a malicious caller could send `Authorization` to P1 — the trace
  redaction and feed must still scrub it so the owner viewing the feed never sees
  a third party's bearer token verbatim either).

### F9 — Auto-CORS must stay P1-only and never credentialed (MEDIUM→HIGH)
- **What / where:** AC-33/34/35/36; `cors.py`.
- **Why it matters:** The management API (P2) must emit **no** wildcard CORS — a
  port that slaps a permissive `tower-http::cors` layer on the whole router would
  let any web origin drive authenticated `/api/**` calls (if a cap is ever in a
  cookie/localStorage reachable cross-origin). P1 reflects the Origin (not `*`
  when present) and **never** emits `Access-Control-Allow-Credentials`
  (reflected-origin + credentials, or `*` + credentials, is the classic
  account-takeover CORS misconfig).
- **Fix / contract requirement:** §5.6 freezes: CORS headers produced **only** on
  P1; `Allow-Credentials` never emitted; reflect Origin, fall back to `*` only
  when absent; P2 has no CORS layer.

### F10 — SQLite injection & key-namespace safety (MEDIUM)
- **What / where:** AC-23/25/27; `helpers.py::is_safe_key`, all DB access.
- **Why it matters:** Tokens, collection names, state keys, ids are user-influenced
  and now flow into SQLite (`endpoint_state`, `crud_collections`) instead of Redis.
  Must use **parameterized queries** everywhere (`sqlx` bound params, never
  `format!` into SQL). The `^[A-Za-z0-9_-]{1,64}$` safe-key check (state keys, CRUD
  names/ids) must be preserved; an unsafe key is skipped (state) / `422` (CRUD
  name). With Redis gone the keys are no longer a namespace-escape risk, but the
  charset check still guards path/parameter sanity and the 64-char cap.
- **Fix / contract requirement:** §5 mandates parameterized SQL only; safe-key
  charset preserved for keys/collection-names/ids.

### F11 — CSRF posture for state-changing `/api/**` (LOW — accepted, but state it)
- **What / where:** all mutating `/api/**` (DELETE/PATCH/POST endpoints).
- **Why low:** Auth is a **Bearer token**, not an ambient cookie — a cross-site
  page cannot read the cap to set the `Authorization` header, so classic
  cookie-CSRF does not apply *provided the cap is never stored in a cookie and the
  API never accepts the cap from a cookie*. **This is the load-bearing
  assumption.** If the SPA stores the cap in a cookie or the server ever reads
  `cap` from a cookie, CSRF reopens.
- **Fix / contract requirement:** §5.1 states the cap is accepted **only** from
  `Authorization: Bearer` (P2) and `?cap=` (feed/tunnel) — **never** from a
  cookie; the SPA holds it in memory/`localStorage`, not a cookie. With that, no
  CSRF tokens are required.

### F12 — `410 gone` tombstone integrity (LOW — ties to OQ-1)
- **What / where:** AC-57, OQ-1; today `redis_state.mark_gone/is_gone`.
- **Why minor security note:** the 404-vs-410 distinction is an info-leak knob (it
  tells a stranger a token *once existed*). The Python default already fails to
  `404` (safer) when the gone-marker is unavailable. Whatever the architect picks
  for OQ-1 (a `gone_at` column / tombstone table, or dropping 410→404), the
  security-acceptable default is **404 when in doubt**. Neither path is logged as
  a trace (AC-57) — preserve that.

### F13 — Tunnel forwards attacker-controlled bytes to operator localhost (LOW/INFO)
- **What / where:** AC-49/50/51; `tunnel.py`.
- **Why noted:** the tunnel deliberately replays public traffic to the operator's
  localhost server — that is the feature. The control is that **only the
  capability owner can bind** (cap-gated, `4401`), traffic is subject to the same
  P1 ingest/rate caps, base64 framing avoids smuggling, and per-request timeout
  prevents hangs. Re-review confirms bind auth + caps; the localhost exposure is
  accepted (it's the product).

---

## Required security ACs (fold into PRD §4 during REVISE)

These are testable and map to re-review probes. Most reinforce existing ACs with
the **explicit "re-proven on Rust"** obligation.

- **AC-S1 — SSRF resolved-IP guard.** A MITM `target_url` whose host resolves to
  any loopback/private/link-local/multicast/reserved/unspecified address or
  `169.254.169.254` (IPv4 and IPv6, incl. IPv4-mapped `::ffff:` forms) is refused
  with `502`; the guard runs on **every** resolved address, not the hostname.
- **AC-S2 — DNS-rebinding pin.** The forward connects to the **validated IP
  literal** (no second resolution), preserving `Host` + TLS SNI; a host that
  resolves public-then-private between check and connect cannot reach the private
  address.
- **AC-S3 — MITM redirects.** Redirects are not followed by default; when enabled,
  each hop re-runs AC-S1/S2, bounded by `MITM_MAX_REDIRECTS`.
- **AC-S4 — Forward header hygiene.** `authorization`, `cookie`, `x-owner-id`,
  `x-user-id`, `x-hookbox-cap` and hop-by-hop headers are stripped before any MITM
  forward; the owner capability is never sent upstream. Upstream `set-cookie` /
  CORS / `transfer-encoding` / `content-length` / `content-encoding` are stripped
  from the captured response.
- **AC-S5 — No SSTI.** The SSTI probe set (`{{7*7}}`, `{{config}}`,
  `{{''.__class__}}`, `{{self}}`, handlebars/tera-style helpers) is returned
  **verbatim**, executing zero code; templating is a hand-written scanner with no
  general template engine over user text.
- **AC-S6 — Cap rotation invalidates the old secret.** After a second
  `POST /api/session` for the same email, the previously returned secret
  authenticates with `401` (overwrite, not append).
- **AC-S7 — `owner_id` is not a credential.** Presenting `owner_id` (or any
  non-secret) as a bearer token returns `401`.
- **AC-S8 — 401 vs 404 authz.** Missing/malformed/unknown cap → `401` +
  `WWW-Authenticate: Bearer`; a valid cap addressing a `{token}` it does not own →
  `404` (never `403`); bodies are uniform `{error, detail}`.
- **AC-S9 — Feed owner-gate before any frame.** An anonymous / wrong / cross-owner
  WS or SSE subscribe receives **zero** events (WS close `4401` / SSE `401`),
  verified before registration/first frame; a cap valid for `tokenA` receives only
  `tokenA` events.
- **AC-S10 — Tunnel bind auth.** An unauthenticated / wrong-owner bind to
  `/ws/tunnel/{slug}` is refused (`4401`) and never registered; only the slug's
  owner can bind.
- **AC-S11 — Management API has no wildcard CORS.** No `Access-Control-Allow-*`
  headers are emitted on any `/api/**` response.
- **AC-S12 — P1 CORS never credentialed.** No P1 response ever emits
  `Access-Control-Allow-Credentials`; Origin is reflected (or `*` only when
  absent).
- **AC-S13 — Cap never persisted/logged/reflected.** The capability and
  `Authorization`/`Cookie`/`X-Owner-Id` headers are redacted in stored traces and
  in feed payloads; the cap never appears in `tracing` logs (incl. the `?cap=`
  query string of `/ws|/sse`) nor in any error body.
- **AC-S14 — Cap accepted only via Bearer / `?cap=`, never a cookie.** The API
  never reads the capability from a cookie; the SPA does not store it in a cookie
  (closes CSRF without tokens).
- **AC-S15 — Ingest body cap before buffering.** A P1 request whose
  `Content-Length` or streamed size exceeds `MAX_INGEST_BODY_BYTES` is rejected
  `413` without fully buffering.
- **AC-S16 — Plane isolation.** No crafted `Host`/path (multi-label subdomain,
  percent-encoded path, a token equal to the app host, `/api` under a mock host)
  causes the P1 catch-all to serve `/api/**` or the UI/feed; `/api/**` is
  unreachable from any mock host.
- **AC-S17 — Parameterized SQL only.** No SQL is built by string interpolation;
  the `^[A-Za-z0-9_-]{1,64}$` charset is enforced for state keys (skip/ignore),
  CRUD collection names (`422`), and CRUD ids.
- **AC-S18 — In-process limiter is bounded and fails open.** The token-bucket map
  is bounded (entry cap / idle eviction) so it cannot grow without limit; on an
  internal limiter error the request is **allowed** (fails open) while the body/
  size caps still bound it.
- **AC-S19 — Feed/registry memory bounds.** Per-endpoint WS connection cap, bounded
  broadcast channel with lag-drop, per-send timeout and slow-client drop are all
  enforced so a flood of connections/endpoints cannot exhaust process memory.

## §5 contract notes (the system-architect must specify when freezing)

- **§5.1 (auth):** rotate-overwrite of `capability_hash`; hash-only lookup;
  CSPRNG 256-bit cap; cap accepted **only** from `Authorization: Bearer` (P2) and
  `?cap=` (feed/tunnel), **never** a cookie; `401` (with `WWW-Authenticate:
  Bearer`) vs `404` semantics frozen; uniform `{error, detail}`.
- **§5.5 (MITM):** SSRF on every resolved IP + IP-pin (no second resolution) +
  redirects-off-default-and-revalidate-per-hop + forward/response header strip
  lists + `MITM_ALLOW_PRIVATE` defaults false; timeouts→504, block/conn→502.
- **§5.6 (CORS):** P1-only; never `Allow-Credentials`; reflect Origin; P2 carries
  no CORS layer.
- **§5.7 (templating):** hand-written scanner, **no template-engine crate over
  user text**, unknown→literal, never errors mock path, size/tag caps.
- **§5.5 caps:** all caps env-driven with safe defaults; ingest `413` before
  buffering; trace truncation; write-time trace prune; WS conn cap; bounded
  limiter map + bounded broadcast channel (NEW — the no-Redis additions).
- **§5.6 DB:** parameterized SQL only; safe-key charset for keys/collections/ids;
  WAL + `foreign_keys=ON`.
- **§5.4 logging:** access-log/tracing deny-list for `authorization`/`cookie`/
  `cap`/`x-owner-id`; never log `/ws|/sse` query strings raw.

## OQ-3 resolution — post-Redis degradation contract (SECURITY SIGN-OFF)

**Context.** In the Python build three behaviors were *Redis-failure* branches:
state-gated rules **fail closed** (matched state treated as empty → gated rules
don't fire), Auto-CRUD → **503** when Redis is down, and the rate limiter **fails
open** (`RateLimitResult(..., degraded=True)`, `redis_state.py::rate_limit_check`).

**Resolution (recommended, accepted):** With SQLite as the **single local
datastore**, state and CRUD are now ordinary in-process transactional reads/writes
— there is no remote dependency to "be down," so their dedicated fail-closed/503
branches **collapse into normal SQLite error handling** and should be removed as
distinct contract behaviors:

- **State (`endpoint_state`):** reads/writes are reliable local calls. A genuine
  SQLite error (disk full, corruption) is a **5xx on the management API** path and,
  on the **mock path**, must **fail closed for security-relevant gating** — i.e. a
  `state_requirement` that cannot be evaluated must **not** match (a rule gated on
  `authenticated=true` must never serve when state is unreadable). This preserves
  the *intent* of the old fail-closed behavior without a Redis-specific branch.
- **Auto-CRUD (`crud_collections`):** a SQLite error surfaces as `500` on that
  served path (no longer a Redis-specific `503`); atomicity is provided by a single
  SQLite transaction (AC-26), not WATCH/MULTI. No data-integrity regression.
- **Rate limiter (in-memory `DashMap` bucket):** **keep fail-open** as an explicit
  property of the in-memory bucket (AC-39 / AC-S18). Rationale: the limiter is a
  traffic-shaping/abuse knob, not an authorization control; failing it open avoids
  turning a transient internal error into a self-inflicted outage, and the
  body/size caps (`MAX_INGEST_BODY_BYTES`, trace caps) plus the WS connection cap
  remain hard bounds regardless. **Security caveat:** the limiter map itself must
  be bounded (AC-S18) so "fail open" can never be weaponized into unbounded memory
  growth; and the **session anti-enumeration limiter** (`SESSION_RATE_LIMIT_PER_MIN`
  on `POST /api/session`) also fails open today — that is acceptable because the
  endpoint is already constant-shape/anti-enumeration (AC-1) and minting a session
  is not a privileged action, but note it so the architect doesn't assume it is a
  hard control.

**Net:** the only *security-load-bearing* fail-direction to preserve is
**state-gated rule evaluation fails closed** when state is unreadable; everything
else is either ordinary local error handling (state/CRUD → 5xx) or an explicit,
bounded **fail-open** (the two rate limiters). Architect to confirm and freeze in
§5; security signs off on this contract.

## Open security questions (for PM/architect before lock)

- **OSQ-1 (→ architect, ties OQ-1):** confirm the `410`-vs-`404` tombstone
  mechanism; whichever is chosen, the security-safe default is **404 when the
  gone-state is indeterminate**, and neither is logged as a trace.
- **OSQ-2 (→ architect):** where does the SPA hold the capability? Must be
  **non-cookie** (memory/`localStorage`) to keep CSRF closed (F11/AC-S14). Confirm.
- **OSQ-3 (→ architect):** confirm the `reqwest` IP-pinning mechanism preserves
  TLS SNI + cert verification against the original hostname (F1) — this is the
  subtlest correctness/security interaction in the port.
- **OSQ-4 (→ architect):** confirm bounds for the in-memory rate-limit map and the
  `tokio::broadcast` channel (entry cap / lag-drop) so the no-Redis move doesn't
  create an unbounded-growth DoS (F6/AC-S18/S19).
- **OSQ-5 (→ architect):** confirm structured-logging (`tracing`) deny-list so
  caps / auth / cookie / `?cap=` query strings are never emitted to logs (F8/F4).
