# PRD: HookBox — Beeceptor-class API Mocking & Interception Platform  (slug: beeceptor-rewrite)

> **Mode:** REVISE. Ground-up rewrite that **replaces** the toy webhook-catcher in `app/` + `templates/`. Governed by `docs/features/beeceptor-rewrite/_decisions.md` (LOCKED) and the full product spec `prompt.txt` (6 sections); where they conflict, `_decisions.md` wins.
>
> **§5 is FROZEN.** It is lifted **verbatim** from the system-architect's `architecture.md` §5 (the authoritative contract). FE (`templates/`, `static/`) and BE (`app/`, `config.py`, `requirements.txt`, Docker, `tunnel/`) implement against exactly those shapes and never need each other to change. Security-engineer's §5 contract notes are folded in (they match the architect's §5; cross-references noted inline).
>
> **§4 incorporates** the security-engineer's 27 required ACs (SEC-AC-1…27), the design-agent's visual criteria (VC-1…VC-17), the user-journey's edge cases/states, and `ux.md`'s structure/copy/accessibility.
>
> **Locked stack (do not substitute):** Python 3.12+ async · FastAPI on `uvicorn[standard]` · `aiosqlite` (SQLite WAL) · **Redis** (state KV + pub/sub + rate-limit) · `httpx` (MITM forward) · server-rendered **Jinja2 + HTMX + Alpine.js + Tailwind**. **No React/JSX/Vite/Node build.** The spec's `useRequestStream` React hook is delivered as an **Alpine store + `static/js/request-stream.js`** with identical responsibilities (open pipe, exponential-backoff reconnect, dedupe, non-blocking feed).

---

## 1. Problem & goal

Developers need to mock, intercept, inspect, and virtualize HTTP APIs without standing up a backend, and existing self-hosted options are heavy or incomplete. The current HookBox is a single-purpose webhook *catcher*: it records inbound requests and can replay one canned mock per HTTP method (`app/main.py` `/hook/{user_id}/{endpoint_id}` `[existing — verified at app/main.py:41]`), with an in-process WebSocket feed and no Redis, no proxying, no templating, no rules engine. The goal is to rebuild HookBox into a **Beeceptor-class platform**: a low-overhead interceptor (**<10 ms of our own overhead** on the mock fast path) that serves rule-driven mock responses with dynamic templating, persists per-endpoint state, can auto-act as a CRUD backend, can forward unmatched traffic to a real upstream (MITM) and capture it, auto-handles CORS, simulates adverse network conditions, and streams every transaction to a real-time split-screen debugging dashboard — all behind a **no-password, email-keyed** session and shipped as a `docker compose up` deployment with a working local-tunnel CLI.

## 2. Non-goals

- **No registration / password / OTP wall.** Email is the access key only (LOCKED §5). We will **not** add login credentials, email verification, or sessions beyond the localStorage-persisted owner identity. (Hardened — not weakened — by SEC-AC-1…5: a real server-stored capability backs the email.)
- **No React / SPA / Node build / Vite / JSX** anywhere (LOCKED §1a). No client framework other than Alpine.js + HTMX + vanilla JS. The `useRequestStream` hook becomes `static/js/request-stream.js` + an Alpine store.
- **No production-grade Go tunnel binary.** We ship a **Python reference** `mock-tunnel` CLI as the spec's "blueprint + simple CLI" (LOCKED §8); a Go rewrite is explicitly future work.
- **Not preserving the old surface.** The `/status` crypto-trading-bot route (`app/main.py:123-268`), the GitHub auto-deploy webhook (`app/routes/webhook.py`), the SMTP email-backup (`app/routes/backup.py`), the `/login` / `/register` pages, and the `/hook/{user_id}/{endpoint_id}` URL shape are **removed/superseded**, not migrated.
- **Export / restore is CUT for this milestone** (RESOLVED OQ-7 + architect §6.6 + security SOQ-7). The SMTP backup and the `/backup` page/route/nav are removed entirely. Re-introducing JSON export/restore is additive future work (`GET/POST /api/owner/export`) and does not disturb §5; it must enforce owner-from-capability on every imported row if it ever returns.
- **No "Webhook Actions" (outbound webhook-on-match) behavior in v1** (RESOLVED OQ-9 + architect §6.1). The `webhook_action` field stays in the frozen §5 schema (accepted-and-stored) so the contract never changes later, but BE ships it as a **no-op stub** and FE renders the tab **disabled with a "coming per spec" note**. No outbound request is made on match in v1.
- **No multi-region, no clustering, no horizontal scale-out** of the app container in this scope (single app instance + single Redis + single SQLite file). Redis pub/sub + the `cfg:<token>` cache-invalidation bus keep this *not architecturally blocked* later, but multi-instance is out of scope to validate.
- **No paid plans, quotas, billing, org/team accounts, or RBAC** beyond single-owner ownership.
- **No gRPC / GraphQL / WebSocket *mocking*.** We mock HTTP request/response only; the dashboard itself uses WS, but mocking non-HTTP protocols is out of scope.
- **No request replay / edit-and-resend** in v1 (RESOLVED journey gap 16). Beeceptor offers it; HookBox v1 does not. Explicitly deferred so it is not silently assumed.
- **Redis data is best-effort durable, not guaranteed.** State, Auto-CRUD collections, and rate buckets live in Redis; compose runs Redis with `--appendonly yes` on a named volume, but a Redis wipe loses live state/collections (never config/rules/history, which are in SQLite). See §8 R7.

## 3. Users & context

| User | When / where | What they do |
| --- | --- | --- |
| **Frontend/app developer (D)** | Landing `/`, then dashboard `/d/<token>` | Enters email → gets `https://<token>.<MOCK_DOMAIN>` (and a `/e/<token>` path-fallback URL). Points their app at it, watches requests stream into the live feed, inspects each one. |
| **Integration/QA engineer (Q)** | Rule builder modal + endpoint settings panel | Writes mock rules (match → templated response), enables Auto-CRUD, sets a real-API MITM target, dials latency/rate-limit/chaos, traces which rule matched and how state changed. |
| **Developer behind a firewall (F)** | Terminal | Runs `mock-tunnel --port 3000 --endpoint <slug>` so public traffic to `<slug>.<MOCK_DOMAIN>` reverse-tunnels to `localhost:3000`. |
| **Operator / self-hoster (O)** | Shell / Docker host | `docker compose up`; relies on healthchecks, named volumes (SQLite + Redis), an internal network. Configures `MOCK_DOMAIN`, Redis URL, retention caps. |

**Primary screens / flows (server-rendered Jinja2):** Entry/landing (`/`), Dashboard split-screen (`/d/<token>`), and — as **overlays on `/d/<token>`** (RESOLVED ux gap 15) — the Settings panel and the multi-tab Rule-builder modal. Three hard-isolated request planes (LOCKED §2, architect §3): **(P1)** wildcard mock interception on `*.<MOCK_DOMAIN>/<path>` + path-fallback `/e/<token>/<path>`; **(P2)** management API `/api/*` (app host only); **(P3)** UI + static `/`, `/d/<token>`, `/static/*`, `/ws/*` (app host). Plane confusion is itself a security boundary (security §1.2).

**Multi-endpoint IA (RESOLVED journey gap 4 / ux gap 1):** `POST /api/session` **auto-provisions the owner's first endpoint** and returns it as `primary`; the browser routes to `/d/<primary.token>`. An owner may hold **>1 endpoint**; the endpoint **list/switcher lives in the dashboard endpoint bar** (a switcher fed by `GET /api/endpoints`), and a new endpoint is created via `POST /api/endpoints`. There is no separate list page (the old `/` list home is gone).

**Canonical `MOCK_DOMAIN` & local-dev recipe (RESOLVED OQ-14, architect §3.2):** the documented default is `mock.local`; the UI shows the subdomain chip `https://<token>.mock.local/…` **and** the local-fallback chip `/e/<token>/…`. README documents `*.localhost` → `127.0.0.1` and the `http://<token>.127.0.0.1.nip.io:8000` recipe. When `MOCK_DOMAIN` is unset/misconfigured the app **logs a clear warning and serves path-fallback-only mode** (it does not crash); the dashboard then renders only the `/e/<token>` chip.

## 4. Acceptance criteria

> Every AC is testable and observable. "FE" = frontend lane (`templates/`, `static/`), "BE" = backend lane (`app/`, `config.py`, `requirements.txt`, Docker, `tunnel/`). `<token>` = endpoint token; mock surface = `<token>.<MOCK_DOMAIN>/<path>` or `/e/<token>/<path>`. `<cap>` = the owner capability (`owner_secret`, §5.1). Status codes, payloads, headers, and the resolution order are pinned in **§5** — ACs reference them, never contradict them.

### A. Routing, isolation & access model (prompt §3.2, UX-1; LOCKED §2,§5; arch §3,§5.1,§5.2)

- **AC-1 (Email → instant session).** `POST /api/session` with a valid email returns `200 SessionResponse` containing `owner_id`, `owner_secret`, `endpoints[]`, and `primary` (an `EndpointSummary` with `mock_url` + `path_url`) — **no** password step. A malformed email returns `422`. *(BE)*
- **AC-2 (Resume by email).** Submitting the **same** email again returns the **same** `owner_id` and resolves the owner's existing endpoint(s); **no duplicate owner** is created. The response **rotates** `owner_secret` (old secret stops working; new one returned). *(BE)*
- **AC-3 (Session persisted client-side).** After entering an email on `/`, `{owner_id, owner_secret, token, mock_url}` is written to `localStorage` key `hookbox_owner` and the browser routes to `/d/<primary.token>`; reloading `/d/<token>` with that localStorage present shows the dashboard without re-entering email. *(FE)*
- **AC-3a (Auto-resume + identity switch).** Opening `/` with `hookbox_owner` present routes straight to `/d/<last token>` without re-entering email; the nav offers a **Logout** that clears `hookbox_owner` and returns to `/`, and re-entering a different email switches owner (RESOLVED ux gap 17). If `localStorage` is unavailable (private mode), the app shows "enable storage or re-enter email each visit" rather than an infinite redirect loop (RESOLVED journey edge case). *(FE)*
- **AC-3b (Multi-endpoint switcher).** When an owner has >1 endpoint, the dashboard endpoint bar shows a switcher (from `GET /api/endpoints`); selecting one navigates to its `/d/<token>`; `+ New endpoint` calls `POST /api/endpoints` and routes to the new token (RESOLVED journey gap 4, ux gap 1). *(FE + BE)*
- **AC-4 (Wildcard subdomain interception).** A request to `http(s)://<token>.<MOCK_DOMAIN>/<any/path>` is dispatched to the interceptor engine for `<token>`, verified by the response header `X-HookBox-Endpoint: <token>` (§5.5). *(BE)*
- **AC-5 (Path-based local fallback).** A request to `/e/<token>/<any/path>` on the app host reaches the **same** interceptor with **mock path = `/<any/path>`** (the `/e/<token>` prefix stripped, arch §3.2) and produces the same result as AC-4 for an equivalent rule. *(BE)*
- **AC-6 (Plane isolation — mock never shadows API/UI).** On the **app host**, `/api/...`, `/`, `/d/...`, `/static/...`, `/ws/...` are served by the management/UI planes (correct content type/status), never the interceptor — even when an endpoint has a catch-all rule. On a **mock host** (`<token>.<MOCK_DOMAIN>`), `/api/...` and `/static/...` are treated as **mock traffic** for that endpoint, not management (arch §3.1, security §4.8). *(BE)*
- **AC-6a (Reserved subdomains / apex).** The bare apex (`https://<MOCK_DOMAIN>/`, no subdomain), `localhost`, `127.0.0.1`, and `<APP_HOST>` resolve to the **UI plane**, not the interceptor; a generated `<token>` is drawn from the ambiguity-stripped alphabet (`generate_endpoint_id` `[existing — verified at app/utils/helpers.py]`) so it never collides with the app host (RESOLVED journey gap 3). *(BE)*
- **AC-7 (Unknown token).** A mock request for a non-existent token returns `404 {error:"unknown_endpoint", detail:"..."}` with header `X-HookBox-Endpoint: <token>`, and is **not** logged against any real endpoint (§5.5). *(BE)*
- **AC-7a (Known-but-expired/deleted endpoint → 410).** A mock request to a token whose endpoint was deleted (owner delete) or pruned/expired returns **`410 {error:"endpoint_gone", detail:"..."}`** (distinct from the `404 unknown_endpoint` of a never-existed token, AC-7), carries header `X-HookBox-Endpoint: <token>`, and is **not** logged against any endpoint (§5.5, RESOLVED OQ-1). The owner viewing their own deleted/expired endpoint in the dashboard sees an "endpoint not found / re-create" state, not a dead live pill (RESOLVED journey gap 2 / edge case). *(BE + FE)*

### A-SEC. Access-model security (security F1–F3, F16; SEC-AC-1…5; arch §5.1)

- **AC-S1 (Capability is a real, separate secret).** `POST /api/session` returns a CSPRNG `owner_secret` (≥128-bit; `secrets.token_urlsafe(32)`) that is **distinct from** the public `owner_id` and is **stored server-side hashed** (sha256, `owners.secret_hash`) and verified on every `/api/*` call. The public `owner_id` (= `hash_email`) is **never accepted as an authorization credential** — a request with a valid `owner_id` but absent/wrong `Authorization: Bearer <cap>` returns `401` (SEC-AC-1). *(BE)*
- **AC-S2 (No IDOR — cross-tenant read).** With owner B's capability, `GET /api/endpoints/<A's token>`, `/rules`, `/requests`, and `GET /api/requests/<A's request id>` all return **404** (never owner A's data) (SEC-AC-2). *(BE)*
- **AC-S3 (No IDOR — cross-tenant mutate/delete).** With owner B's capability, `PATCH`/`DELETE /api/endpoints/<A's token>` and all `/rules` mutations on `<A's token>` return `404` and leave A's data unchanged (SEC-AC-3). *(BE)*
- **AC-S4 (Unauthenticated `/api/*` rejected).** Any `/api/endpoints*`, `/rules*`, `/requests*` call with **no** capability returns `401` — not `200`, not a silent default user (SEC-AC-4, closes the `X-User-Id` header-trust hole `[existing — verified at app/routes/api.py:24]`). *(BE)*
- **AC-S5 (Enumeration-resistant session).** `POST /api/session` is **constant in shape/status** for new vs existing emails (no "welcome back" vs "created" signal observable to the client) and is **rate-limited per source**; a malformed email returns `422` (SEC-AC-5). The public mock `<token>` does **not** embed or reveal the `owner_id` (RESOLVED journey gap 1 — old `/hook/{user_id}/…` leaked it; the new token is independent of the owner id). *(BE)*

### B. Stateful / multi-step transactions (prompt §1.1; LOCKED §3.1; arch §4.2,§5.3,§5.8)

- **AC-8 (State mutate).** A rule with `state_writes:[{key:"authenticated", value:"true"}]` matching `POST /login` writes to the Redis hash `state:<token>` (TTL `STATE_TTL`, default 24h); the value is then readable (observable via the trace tab's `state_snapshot` and `{{state.authenticated}}` templating). *(BE)*
- **AC-9 (State-gated match).** A rule with `match.state_requirements:[{key:"authenticated", op:"eq", value:"true"}]` is **selected only when** the condition holds: before AC-8's login the request falls through to CRUD/MITM/default; after login it returns the rule's response (arch §4.2). *(BE)*
- **AC-10 (State scoping, reset & isolation).** State is **per-endpoint** (`state:<token>`, RESOLVED OQ + architect §6.5 + journey gap 6 — shared across all callers of that mock, matching the login→dashboard example). Endpoint A's state is never visible to endpoint B (keyspace prefixed by token). A user-facing **"Clear state"** action in the Settings danger zone calls `DELETE /api/endpoints/{token}/state`, clearing the hash and returning gated rules to pre-condition behavior. *(BE + FE)*
- **AC-10a (State key safety).** User-supplied state keys are validated against a safe charset (`^[A-Za-z0-9_-]{1,64}$`) so a crafted key cannot inject a Redis separator to reach another endpoint's namespace; A's state stays invisible to B even with a crafted key (SEC-AC-24, security F12). *(BE)*

### C. Instant Auto-CRUD (prompt §1.2; LOCKED §3.2; arch §4.3,§5.5)

- **AC-11 (Toggle on).** Setting `auto_crud:true` via `PATCH /api/endpoints/{token}` (and the dashboard toggle) makes the endpoint behave as a REST DB backend over a Redis-backed JSON array per collection (`crud:<token>:<collection>`), **no mock rules required**. *(BE config, FE toggle)*
- **AC-12 (CRUD lifecycle).** Against an Auto-CRUD endpoint (arch §4.3): `POST /<collection>` with JSON creates an object, returns it with a generated `id` and `201`; `GET /<collection>` returns the array (`200`); `GET /<collection>/<id>` returns one object (`200`) or `404`; `PUT`/`PATCH /<collection>/<id>` updates/merges and returns it (`200`) or `404`; `DELETE /<collection>/<id>` returns `204` and a subsequent `GET /<collection>/<id>` returns `404`. *(BE)*
- **AC-12a (CRUD unhappy paths).** `POST` with invalid/empty/non-JSON body → `400` (not `500`); `PUT`/`PATCH`/`DELETE` on a missing id → `404`; `collection`/`:id` are validated against `^[A-Za-z0-9_-]{1,64}$` (SEC-AC-24); concurrent `POST`s generate collision-safe ids (uuid4, arch §4.3) (RESOLVED journey gap 7). *(BE)*
- **AC-12b (CRUD bounded).** Each collection has a configurable cap on item count and per-item/total size (`config.py`/env); exceeding it is rejected (not grown unboundedly) (SEC-AC-21, security F8c). *(BE)*
- **AC-13 (CRUD precedence vs rules).** Resolution order is **frozen**: `OPTIONS preflight → matching rule → Auto-CRUD → tunnel → MITM → default` (§5.5). A rule matching `/<collection>` wins over CRUD; the trace `served_by` shows which path served the request (arch §4.1/§4.3). *(BE)*

### D. Proxy / partial mocking — MITM (prompt §1.3; LOCKED §3.3; arch §4.4,§5.5; security F4)

- **AC-14 (Forward on no-match).** With `target_url` set and no matching rule (and Auto-CRUD/tunnel not handling it), the request is forwarded via the shared `httpx.AsyncClient` to `target_url + mock_path + querystring`; the upstream status, safe headers, and body are returned to the client (arch §4.4). *(BE)*
- **AC-15 (Capture forwarded response).** A forwarded request and its **real** upstream response are written to `request_logs` with `served_by:"mitm"` and shown in the inspector labeled "Proxied" (§5.3 `RequestDetail`). *(BE)*
- **AC-16 (Match wins over forward).** When a rule matches on a MITM-enabled endpoint, the **local mock** is returned and **no** upstream call is made (observable: upstream receives zero requests; trace `served_by:"rule"`). *(BE)*
- **AC-17 (Upstream failure handling).** Upstream connection error → `502 {error:"upstream_unreachable"}`; timeout → `504`; both logged (arch §4.4). Redirects are **not** followed by default (`follow_redirects=False`). *(BE)*
- **AC-S6 (Scheme allow-list).** Setting `target_url` to any non-`http(s)` scheme (`file:`,`gopher:`,`ftp:`,`data:`,…) is rejected at config time with `422` and never fetched (SEC-AC-6). *(BE)*
- **AC-S7 (Private/link-local/metadata blocked — SSRF).** A MITM forward whose target resolves to loopback (`127.0.0.0/8`,`::1`), private (`10/8`,`172.16/12`,`192.168/16`,`fc00::/7`), or link-local (`169.254.0.0/16` incl. **`169.254.169.254`**, `fe80::/10`) is **blocked** (`502`) and **logged**, evaluated **on the resolved IP** not just the hostname string, unless `MITM_ALLOW_PRIVATE=true` (SEC-AC-7). Negative test: a target of `http://169.254.169.254/...` is refused. *(BE)*
- **AC-S8 (Redirects re-validated / capped).** If redirect-following is enabled by config, MITM follows at most a small N and **re-applies AC-S7 to every hop's resolved IP**; a redirect into a blocked range is refused (SEC-AC-8). *(BE)*
- **AC-S9 (Timeout + response-size cap + header stripping).** MITM enforces `MITM_TIMEOUT_S` and `MITM_MAX_BODY_BYTES`; exceeding either yields a deterministic error and is logged. Hop-by-hop and sensitive headers are stripped before forwarding (reuse `format_headers` `[existing — verified at app/utils/helpers.py:18]`); the owner capability is **never** forwarded upstream; upstream `Set-Cookie`/CORS/hop-by-hop headers are stripped/normalized so our auto-CORS isn't broken (SEC-AC-9, security §4.3, RESOLVED journey gap 8). *(BE)*

### E. Auto-CORS engine (prompt §1.4; LOCKED §3.4; arch §5.6; security F10)

- **AC-18 (Preflight handled).** An `OPTIONS` to any mock path (when `cors_enabled`) returns `204` with `Access-Control-Allow-Origin: <reflected Origin or *>`, `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`, `Access-Control-Allow-Headers: <reflected Access-Control-Request-Headers or *>`, `Access-Control-Max-Age: 600`, `Vary: Origin` — **without** a user-defined rule (§5.6). *(BE)*
- **AC-19 (CORS on every response).** Every intercepted non-preflight response (mock/CRUD/MITM/tunnel/default) carries `Access-Control-Allow-Origin: <reflected Origin or *>`, `Access-Control-Expose-Headers: *`, `Vary: Origin` (§5.6). *(BE)*
- **AC-S16 (Wide-open CORS is P1-only).** Wildcard CORS appears on **mock-surface (P1)** responses only; `/api/*` (P2) responses carry **no** wildcard CORS (SEC-AC-16, RESOLVED OQ-12). Verify both planes. *(BE)*
- **AC-S17 (No credentialed wildcard).** No response ever sends `Access-Control-Allow-Origin: *` together with `Access-Control-Allow-Credentials: true`; `Allow-Credentials` is **omitted** (§5.6, SEC-AC-17, RESOLVED OQ-12). *(BE)*

### F. Dynamic response templating (prompt §1.5; LOCKED §3.5; arch §5.7; security F5)

- **AC-20 (Core tags render).** A body containing `{{now 'iso'}}` and `{{random 'uuid'}}` returns a valid ISO-8601 timestamp and a valid UUIDv4 respectively (two calls yield different UUIDs) (§5.7). *(BE)*
- **AC-21 (Request echo).** `{{request.query.<k>}}`, `{{request.path.<name>}}` (a `:name` segment from the rule's match path), `{{request.header.<name>}}`, `{{request.body}}`, and `{{request.body.<jsonpath>}}` (dot path, e.g. `a.b.0.c`) are replaced with the corresponding incoming values; a JSONPath miss / non-JSON body yields empty string (§5.7). *(BE)*
- **AC-22 (State echo).** `{{state.<k>}}` renders the current per-endpoint state value (empty if unset; ties AC-8) (§5.7). *(BE)*
- **AC-23 (Unknown/invalid tag fails safe).** An unknown or malformed tag is **left literal** (raw `{{...}}` stays); it never 500s the mock path and never leaks server internals (§5.7). *(BE)*
- **AC-S10 (Sandboxed grammar, no code exec — SSTI).** The engine evaluates **only** the §5.7 allow-list via a hand-written single-pass scanner — **no** Jinja `render_template_string`/`Template().render` over user text, **no** `eval`/`exec`/`str.format` on attacker input. A rule body of `{{ 7*7 }}`, `{{ config }}`, `{{ ''.__class__.__mro__ }}`, or `{{ self }}` renders as inert literal/empty and executes **no** Python (SEC-AC-10). *(BE)*
- **AC-S11 (Templating fails safe & quiet + bounded).** Templating failures never 500 and never leak stack traces into the response; the engine enforces a max template size / max tag count as a DoS bound (SEC-AC-11, security §4.4). *(BE)*

### G. Simulated network conditions (prompt §1.6; LOCKED §3.6; arch §4.1,§5.5; security F8/F9)

- **AC-24 (Latency).** A configured `latency_ms` (0–10000, endpoint-level, or per-rule override) delays the response by approximately that amount (wall-clock ≥ configured, within tolerance) via `asyncio.sleep` (never blocking other endpoints' event loop) before returning (§5.5). *(BE config + FE slider)*
- **AC-25 (Rate limit).** With `rate_limit_per_min > 0`, requests beyond the limit in the window receive `429 {error:"rate_limited"}` + `Retry-After: <sec>` + `X-RateLimit-Limit`/`X-RateLimit-Remaining`, enforced via a **Redis Lua token bucket** keyed per endpoint (`rl:<token>` / `rl:<token>:<rule_id>`); `0` = unlimited (§5.5). *(BE config + FE input)*
- **AC-26 (Chaos injection — default 5xx; drop opt-in; both bounded).** **Default chaos = random 5xx:** with `chaos_pct=100` every request returns a random failure from {502,503,504} (JSON `{error:"chaos"}`); with `chaos_pct=0` none do. The **connection-drop (`dropout`) mode is opt-in** per rule/endpoint; when enabled and rolled, the connection is closed without a response. **Both** chaos modes are bounded by the **same global rate/size caps** as every other P1 path (AC-S18/AC-S19) and the `dropout` close is bounded by a server-side drop timeout, so neither is an unbounded abuse vector (RESOLVED OQ-2; honors prompt.txt §1.6 "dropouts **or** random HTTP errors"). Determinism for tests: `conditions.py` takes an injectable seeded RNG (arch §9 R6) (§5.5). *(BE config + FE dial)*
- **AC-S18 (Ingest body cap).** A P1 request body over the configured max is rejected with `413` before being fully buffered/processed; the cap is env-configurable (SEC-AC-18, security F15). *(BE)*
- **AC-S19 (Rate limit covers MITM & CRUD writes).** The token-bucket limit is enforced on **MITM forwards** and **Auto-CRUD writes**, not only rule hits, so those paths cannot bypass throttling or reflect at a third party; buckets are namespaced per token (no cross-tenant eviction) (SEC-AC-19). *(BE)*
- **AC-S20 (Limiter fails safe on Redis loss — fail-OPEN, bounded).** If Redis is unavailable, the **rate limiter fails OPEN** (the request is allowed) **but** the in-process global body/size caps (not Redis-backed) still apply, so it is **not** unbounded; the fast path never silently makes latency/MITM/CRUD unbounded (SEC-AC-20; per the §5.11 degradation table, RESOLVED OQ-3). *(BE)*
- **AC-27c (Validation clamps).** `latency_ms` clamped 0–10000, `rate_limit_per_min` ≥0 bounded, `chaos_pct` 0–100, both server-side (pydantic, §5.3) and surfaced in the UI; out-of-range values are clamped/rejected, not stored raw (security §4.7, RESOLVED journey edge case). *(BE + FE)*

### H. Real-time split-screen dashboard (prompt §2,§5; LOCKED §1a; arch §4.5,§5.4; ux §2,§3; design §3,§6)

- **AC-27 (Live feed).** When a request hits `<token>`'s mock surface, a new row appears in the dashboard left-column feed in near-real-time **without a page reload**, pushed over the WS pipe fanned out via Redis pub/sub `trace:<token>` (arch §4.5). *(FE + BE)*
- **AC-27a (First-paint server-render).** The feed's most recent traces (up to the 100 cap) are **server-rendered on first load** (progressive enhancement via `partials/feed_row.html`), not blank-until-JS; on WS open the client back-fills/reconciles via `GET /api/endpoints/{token}/requests` (RESOLVED ux gap 2, journey step 5). *(FE)*
- **AC-28 (Feed row content).** Each feed row shows a color-coded HTTP method badge, a truncated path, the served status code, a served-by chip, latency, and relative time (ux §3.2, design §3.3). *(FE)*
- **AC-VC1 (Method-badge palette).** Method badges use the frozen color map (design §2.4): GET `#388bfd`, POST `#3fb950`, PUT `#d29922`, PATCH `#a371f7`, DELETE `#f85149`, OPTIONS/HEAD/ANY desaturated neutral; the **literal method text always renders** (color is never the sole signal) (VC-1, RESOLVED OQ-15). *(FE)*
- **AC-VC9 (Status-code color map).** Status renders as colored text/number in distinct hues: 2xx green `#3fb950`, 3xx amber `#d29922`, 4xx `#e3a008`, 5xx red `#f85149`, with the literal number always shown; 4xx and 5xx are **visibly distinct** (amber vs red) (VC-9, design §2.5). *(FE)*
- **AC-29 (Connection health — three states + degraded + unauthorized).** The WS pill reflects exactly three discrete states with text labels: **"Live"** (connected), **"Reconnecting…"** (+ attempt count after N tries), **"Offline"** (disconnected); plus a **"Realtime degraded"** treatment when the pipe is unhealthy due to Redis (AC-30d/§5.11), and an **"Unauthorized"** treatment when the feed gate refuses the capability (WS close `4401`, §5.4/AC-S12, RESOLVED OQ-4) — observable so a non-owner attempt is visibly refused rather than silently looping. The label is always present (never color-only) (ux §2.4, design §3.8, RESOLVED ux gap 13, journey gap 11). *(FE)*
- **AC-VC4 (Liveness motion).** Connected dot = `#3fb950` with a ~2s pulse/breathe; reconnecting = amber slower throb; offline = static red. Under `prefers-reduced-motion: reduce` all three are **static** colored dots and still legible by color + label (VC-4, design §6.1). *(FE)*
- **AC-30 (Resilient stream — `request-stream.js`, presents the capability).** The pipe is an Alpine store + `static/js/request-stream.js` (the `useRequestStream` substitute) that opens **`/ws/<token>?cap=<owner_secret>`** (reading `owner_secret` from the `hookbox_owner` localStorage; §5.4 owner-gate, RESOLVED OQ-4), **reconnects with exponential backoff** (250ms→500→1000→2000→4000→8000ms) **+ jitter** (cap documented), **dedupes** by `request_id`, and feeds rows without locking the DOM. On a **WS close `4401`** (unauthorized) the client does **not** blindly retry — it surfaces the **"Unauthorized"** health treatment (AC-29) and re-checks the stored capability rather than hammering the gate. Observable: kill the WS server-side → client retries with increasing delay and resumes on recovery (arch §4.5/§5.4). *(FE)*
- **AC-30a (Feed cap = retention cap).** The client feed is capped at the **100-trace retention cap**; rows beyond 100 are trimmed so the DOM never grows unbounded, and the footer states "Showing {n} of last 100" (ties AC-35; RESOLVED ux gaps 10/11, journey gap 10). *(FE)*
- **AC-30b (Burst coalescing + arrival flash).** Bursts are batched via `requestAnimationFrame` so a flood doesn't thrash layout; a newly-arrived row shows a `--success-bg` highlight decaying ≤~900ms (VC-5). Under `prefers-reduced-motion` the row inserts with **no** flash/slide and the "N new" counter still updates (VC-5/VC-17, design §6.2). *(FE)*
- **AC-30c (Pause / auto-follow).** A Pause toggle stops auto-prepend/auto-scroll so a developer can read a row while traffic streams; a "N new" pill buffers and, on resume/click, flushes (RESOLVED ux gap 10, journey alt-flow). *(FE)*
- **AC-30d (Heartbeat + half-open detection + visibility).** The client sends `"ping"` keepalive, detects half-open sockets after tab-sleep, **pauses reconnection while `document.hidden`** and resumes on focus, and resumes/back-fills so events during a gap aren't silently lost (arch §4.5/§5.4, RESOLVED ux gap/journey gap 10). *(FE)*
- **AC-31 (Deep inspector tabs).** Clicking a feed row opens a right-column inspector with tabs **Headers · Query Params · Body** (collapsible JSON/XML tree; non-JSON/XML → raw `<pre>` fallback) **· Response Served · State & Tracing Logs**, each rendering the corresponding captured data from `RequestDetail` (§5.3). The Body tree falls back to raw text on pathological nesting / binary / oversized bodies ("binary, truncated") (ux §3.4, RESOLVED journey edge cases). *(FE)*
- **AC-31a (Inspector detail availability under fire-and-forget).** Because traces are written fire-and-forget (AC-39), a clicked row whose detail is not yet in SQLite shows a brief "detail pending" / retry state rather than a hard 404 (RESOLVED journey gap 15). Full detail is **lazy-loaded** by HTMX/JS via `GET /api/requests/{id}` on row click (RESOLVED OQ-11 + ux gap 3 — the WS event carries only the `RequestSummary`). *(FE)*
- **AC-31b (Served-by taxonomy + identifying headers surfaced).** The Response-Served and State&Tracing tabs render the frozen `served_by` enum (`rule|crud|mitm|tunnel|default|cors|chaos|ratelimit`, §5.3) and the `X-HookBox-Endpoint`/`X-HookBox-Served-By`/`X-HookBox-Rule-Id` headers (§5.5); the served-by chip colors map by intent (matched=success, CRUD=info, proxied=warn, default=muted, chaos=danger, rate-limited=client-err) (VC-8, RESOLVED OQ-10, ux gap 4, design §3.7). *(FE)*
- **AC-31c (Trace step list).** State&Tracing renders the ordered `trace: list[TraceEvent]` (§5.3) as a vertical step list (matched ●/skipped ○/state-write ◆/chaos ✕ glyphs + color, before→after state diffs), not raw JSON (VC-11, RESOLVED ux gap 5, design §3.4). *(FE)*
- **AC-32 (Channel scoping).** A dashboard open on `<tokenA>` receives only `<tokenA>`'s events, never `<tokenB>`'s (channel `trace:<token>`, arch §4.5). *(BE + FE)*
- **AC-S12 (Real-time channel requires capability — FROZEN, owner-gated).** Opening the WS/SSE pipe for `<tokenA>` **requires owner A's capability**, presented as the **`?cap=<owner_secret>` query parameter** (§5.4) and verified **server-side BEFORE `accept()`/before channel-join** (WS) and before the first `data:` frame (SSE). The server resolves `<tokenA>`, hashes `cap`, and confirms the resolved owner **owns** `<tokenA>`; otherwise it refuses **before any frame** — **WS close `4401`** / **SSE `401`**. An anonymous subscribe, a wrong-capability subscribe, and owner B's capability on `<tokenA>` are **all refused** and receive **zero** events (no `hello`, no `new_request`) (SEC-AC-12, security F6, RESOLVED OQ-4; closes the no-auth WS `[existing — verified at app/main.py:29]`). Negative tests QA + security run: (a) `/ws/<tokenA>` with no `cap` → close `4401`, no frame; (b) `/ws/<tokenA>?cap=<B's secret>` → close `4401`, no frame; (c) `/sse/<tokenA>` with no/wrong `cap` → `401`, no `data:`. *(BE)*
- **AC-S13 (Channel isolation — security).** A pipe authenticated for `<tokenA>` receives **zero** `<tokenB>` events (strengthens AC-32) (SEC-AC-13). *(BE)*
- **AC-S22 (WS connection cap).** Concurrent real-time connections per endpoint (and/or per source) are bounded; excess are refused — no unbounded fan-out memory growth (SEC-AC-22, security F8). *(BE)*
- **AC-S14 (Captured data escaped everywhere — stored XSS).** Sending a request whose path/header/query/body contains `<script>` / `"><img onerror=...>` and then opening the inspector does **not** execute script: every captured field is HTML-escaped (Jinja autoescape ON for server-render; JS inserts via `textContent`/a real escaper, never raw `innerHTML`; no `| safe` on captured data) (SEC-AC-14, fixes `dashboard.html:213-235` `[existing — verified]`). *(FE)*
- **AC-S15 (Inspector JSON/headers not executable).** Body/headers rendered as formatted JSON are inserted as text nodes/escaped so a JSON string value of `</script><script>...` cannot break out (SEC-AC-15). *(FE)*

### I. Rule builder UI (prompt §2.3; arch §5.2,§5.3; ux §2.3,§3.5; design §3.9)

- **AC-33 (Create-rule modal).** A multi-tab "Create Rule" modal organizes fields into **Matching · Response · Templating · Actions · Throttling** and on submit persists the rule via `POST /api/endpoints/{token}/rules` (serializing `MockRuleCreate`, §5.3) such that the interceptor uses it on the next request. The five tabs share one form; a single Save submits everything (ux §2.3/§3.5). *(FE + BE)*
- **AC-33a (Webhook Actions deferred but present).** The Actions tab's **Webhook** sub-section is rendered **disabled** with a "coming per spec — confirm scope" note; the `webhook_action` field is accepted-and-stored by the API (no-op in v1) so the contract does not change later (RESOLVED OQ-9, architect §6.1, §2 non-goal). *(FE + BE)*
- **AC-34 (Edit/disable/delete rule).** Rules can be listed (`GET .../rules`), edited (`PATCH`), enabled/disabled (optimistic toggle + `PATCH`, revert + toast on failure), and deleted (`DELETE` + confirm); the interceptor honors the change on the next request (picks it up via the `cfg:<token>` cache-invalidation, arch §4.5). Disable ≠ delete (a disabled rule round-trips without losing its body). *(FE + BE)*
- **AC-33b (Rule precedence & multi-match).** With path/header/body matching, multiple rules can match; the matcher selects the **first enabled rule by `priority` (lower first), then `id`** (deterministic, §5.3 ordering + arch §3.3/§4.1), and the trace tab surfaces the matched rule id (RESOLVED journey gap 9). *(BE)*
- **AC-VC14 (Validation surfacing).** Required = method + path; invalid JSON in body/headers shows an inline error on that field and marks the owning tab's rail dot red; status code constrained 100–599; the footer summarizes count needing attention and **disables Save** until resolved; server-side validation errors return the partial with errors rendered (VC-14, ux §3.5, design §3.9). *(FE)*

### J. Data retention (prompt §3.3; LOCKED §6; arch §5.8)

- **AC-35 (100-trace cap).** After more than 100 traces for one endpoint, only the most recent 100 are retained (oldest pruned), enforced **both** at write-time (in `persist_and_publish`) **and** by the sweep (arch §5.8) (RESOLVED OQ-13). *(BE)*
- **AC-36 (24h TTL).** Traces older than 24 hours are swept and removed (`DELETE FROM request_logs WHERE created_at < datetime('now','-24 hours')`, arch §5.8). *(BE)*
- **AC-37 (Both enforced by a background task on a documented interval).** The sweep runs both caps every `RETENTION_SWEEP_SECONDS` (default **300s**); `TRACE_CAP=100`, `TRACE_TTL_HOURS=24`, and the interval are env-configurable and documented (arch §5.8/`config.py`, RESOLVED OQ-13). The open dashboard feed drops a row pruned by the cap to stay consistent (RESOLVED journey edge case). *(BE)*

### K. Performance (prompt §3.1; LOCKED §7; arch §4.1,§7,§9 R1)

- **AC-38 (Fast-path overhead <10 ms).** On the mock fast path (rule resolved from the in-process `rule_cache`, templating + serialization, **no** simulated latency, MITM/tunnel not involved), HookBox's own added overhead is **< 10 ms at the median** under the defined local load, measured as `overhead_ms = duration_ms − applied_latency` (the `request_logs.overhead_ms` column makes this directly observable, §5.3/§5.8). The benchmark is a `pytest`/`locust` profile hitting a no-latency matched rule asserting median <10ms (arch §9 R1, RESOLVED OQ-4). *(BE)*
- **AC-39 (Non-blocking logging + publish).** The trace SQLite write **and** the Redis publish happen in a fire-and-forget `asyncio.create_task(persist_and_publish(...))` that is **never awaited** on the response path; responses return even if the logging path is artificially slowed; the interceptor fast path opens **no per-request DB connection** (reads from `rule_cache`, writes from a single long-lived background connection, arch §4.1/§7). *(BE)*

### L. Local tunnel CLI (prompt §6.1; LOCKED §8; arch §4.6,§5.5; security F11)

- **AC-40 (Tunnel forwards public → local).** Running `mock-tunnel --port 3000 --endpoint <slug> --server <wss-url> --secret <owner_secret>` connects over the WS control channel `/ws/tunnel/<slug>` presenting `Authorization: Bearer <owner_secret>` (auth/takeover semantics in AC-S27/§5.12); a public request to `<slug>.<MOCK_DOMAIN>` (or path fallback) that reaches the tunnel branch (rule > CRUD > **tunnel** > MITM > default, §5.5/§5.12/arch §4.6) is framed to the CLI (`{t:"req",id,method,path,query,headers,body_b64}`), replayed to `localhost:3000`, and the response (`{t:"res",id,status,headers,body_b64}`) is returned to the public caller (arch §4.6). Tunneled traffic still appears in the live feed labeled `served_by:"tunnel"` and is subject to the same ingest/rate caps (AC-S18/S19) (RESOLVED journey gap 14). *(BE)*
- **AC-40a (No tunnel connected).** When `<slug>` has no tunnel registered, public callers get a deterministic `504 {error:"no_tunnel"}` (not a hang) (arch §4.6, RESOLVED journey gap 14). *(BE)*
- **AC-41 (Tunnel reconnect).** If the control channel drops, the CLI reconnects with backoff and resumes forwarding; an in-flight public request during the gap errors (504) rather than waiting forever (arch §4.6). *(BE)*
- **AC-S27 (Tunnel bind is authenticated; last authenticated bind wins — FROZEN).** Binding `mock-tunnel` to `<slug>` requires that endpoint's owner capability presented as **`Authorization: Bearer <owner_secret>` over the WS control-channel handshake** (`/ws/tunnel/{slug}`), verified server-side to **own `<slug>` before registration/`accept()`** (§5.12, RESOLVED OQ-5). An **unauthenticated or wrong-owner** bind is **rejected** (WS close `4401`, no registration). **Slug contention = last authenticated bind wins (takeover):** a second *correctly-authenticated owner* CLI binding an already-bound `<slug>` **takes over** — the server registers the new tunnel and **closes the prior connection** with a clear "rebound elsewhere" message; subsequent public traffic goes to the new tunnel. Because binding is capability-gated, a **cross-owner hijack is impossible** (a non-owner can never take over) (SEC-AC-27, security F11, RESOLVED journey gap 14). Negative tests: (a) bind with no/garbage bearer → close `4401`; (b) bind with owner B's secret to A's `<slug>` → close `4401`, A's tunnel untouched; (c) second owner-A bind → A's first tunnel closed "rebound elsewhere", new one live. *(BE)*

### M. Deployment & cleanup (prompt §6.2; LOCKED §9,§10; arch §2 infra; security F14/F8)

- **AC-42 (Compose up brings up app + Redis).** `docker compose up` starts `app` + `redis` on an internal network, each with a healthcheck (`redis-cli ping`; app `/healthz`); `app` waits for Redis via `depends_on: redis: condition: service_healthy` (arch §2 infra). *(BE)*
- **AC-43 (Persistent volumes).** SQLite data (`hookbox_data`) and Redis data (`hookbox_redis`, `--appendonly yes`) are on **named volumes**; an endpoint's config/rules/history survive `docker compose down && up` (without `-v`) (arch §7 R7). *(BE)*
- **AC-44 (Config via env).** `MOCK_DOMAIN`, `REDIS_URL`, `DATABASE_PATH`, retention caps/interval, MITM policy, latency/rate/chaos bounds, ingest body cap, and ports are env-configurable and documented in README (arch `config.py`). *(BE)*
- **AC-45 (Crypto/auto-deploy/SMTP cruft removed).** The `/status` crypto route, all `openclaw`/`holdings`/`fear_greed`/`restart_service` references, the GitHub auto-deploy webhook (`app/routes/webhook.py`), and the SMTP backup (`app/routes/backup.py`) are **absent** from shipped code (grep for `openclaw`, `holdings`, `fear_greed`, `restart_service`, `/status`, `git pull`, `smtplib` returns nothing) (LOCKED §10). *(BE)*
- **AC-S25 (No secret/internal leakage).** The owner capability, Redis URL/creds, any `.env` value, and Python stack traces never appear in any HTTP response, rendered template, error body, or captured-trace API output; unhandled errors return a generic message (SEC-AC-25, security F14). *(BE)*
- **AC-S26 (Insecure cruft absent — security restatement).** Restates AC-45 as a security gate because these are RCE/secret-exposure class (`os.system`/`git pull`/`pkill` in `app/routes/webhook.py` `[existing — verified]`, the `/status` file-read+reflected-XSS, SMTP creds) (SEC-AC-26). *(BE)*
- **AC-S23 (Parameterized SQL only).** No SQL query interpolates request-derived values via f-string/`%`/`.format`; all values are bound parameters (SEC-AC-23, security F13; REVIEW gate greps for f-string/`%`-built SQL — note prior art `backup.py:43,53` hand-builds `IN (...)`). *(BE)*

### N. Universal per-screen states (ux §3.6; journey "Required states" + gap 18; design §5)

- **AC-46 (Per-screen loading/empty/error/success states).** Every NEW async surface implements all applicable states (lift of the journey "Required states" matrix + ux §3.6 + design §5): **Landing** (submit→"Setting up…"; `422` inline field error that does **not** reveal email-exists; network-error banner; success→redirect); **Dashboard shell** ("Endpoint not found/expired"; owner-mismatch→re-enter email); **Live feed** (skeleton rows loading; "No requests yet" empty with copyable URL + test-request hint; "reconnecting"/"realtime degraded" errors); **Inspector** ("Loading detail…"/"detail pending"; "Select a request" empty; binary/oversized fallback); **Rule modal** (saving spinner; blank-tabs-with-defaults; per-field validation; save-failed toast); **Settings** (loading; defaults; invalid-MITM-URL/out-of-range errors; persisted success); **Rule list** ("No rules yet"); **Tunnel CLI** ("Connecting…"; "Auth failed / slug taken / :port refused"; "Tunnel live" + per-request log lines); **Operator/compose** ("Waiting for Redis healthy…"; "Redis unhealthy / MOCK_DOMAIN unset" / all-green). *(FE for screens; BE for CLI/compose)*
- **AC-VC2 (Contrast AA).** Every method-badge text/fill pair and every status-code-text/surface pair meets **≥3:1** (bold/UI bar); body and secondary text meet **≥4.5:1** (`--text-faint` restricted to non-text/decoration only) (VC-2/VC-2a/VC-2b, design §8.1, RESOLVED ux gap 18 / OQ-15). *(FE)*
- **AC-VC16 (Focus-visible).** Every interactive element (feed row, tab, button, toggle, chip, copy button, tree node) shows a visible focus ring on keyboard focus; the modal **traps focus** and `Esc` restores focus to the trigger (VC-16, ux §5, design §8.2). *(FE)*
- **AC-VC17 (Reduced motion).** Under `prefers-reduced-motion: reduce`, the WS pulse, row flash/slide, skeleton shimmer, and panel transitions are disabled; all states stay legible by color + text + glyph (VC-17, design §8.4). *(FE)*
- **AC-47 (Keyboard + ARIA operability).** The dashboard is fully keyboard-operable: feed rows are real `<button>`/`role=button`+`tabindex=0` with Enter/Space; `↑/↓` move feed selection, `Enter` inspects; inspector & modal tabs use `role=tablist`/`tab`/`tabpanel` with arrow-key nav; `#feed` is `aria-live="polite"` (paused with the feed); the WS pill is `role="status" aria-live="polite"`; copy buttons have `aria-label`s (ux §5, fixes the unnamed `📋` `[existing — verified at index.html:95]`). *(FE)*
- **AC-48 (Mock-URL chips are copy-only, neutral).** The mock-URL chips (subdomain + local-fallback) are **copyable, non-anchor** `code` chips rendered **neutral/muted (never link-blue)** so the dashboard origin is never confused with the mock origin (VC-7, ux §2.0/gap 14, design §3.7). *(FE)*

### O. Redis-down degradation (journey gap 5 — biggest infra gap; security F9/SEC-AC-20)

- **AC-49 (Mock fast path survives Redis loss — per the §5.11 table).** With Redis down, the mock fast path **still serves** matched rules and `default_mode` responses (matching reads the in-process `rule_cache`, not Redis; arch §9 R4). Each Redis-dependent feature degrades **exactly per the frozen §5.11 degradation table (RESOLVED OQ-3)**: **state-gated rules fail CLOSED** (the state condition does not match → rule skipped; never a silent state match without state); **Auto-CRUD returns `503`** (no fabricated/lost data); the **rate limiter fails OPEN** but bounded (AC-S20); the real-time feed degrades to the **"Realtime degraded"** health treatment (AC-29/AC-30d) and may fall back to polling, while mock serving + SQLite trace logging are unaffected. Trace persistence/publish failures are swallowed-and-logged, never blocking the response (arch §4.5/§9 R4, RESOLVED journey gap 5). *(BE + FE)*

## 5. Frozen interface contract  (authoritative — lifted verbatim from `architecture.md` §5)

> Everything below is **frozen** (all blocking open questions resolved 2026-06-18; §9 is empty). FE and BE implement against exactly these shapes. All `/api/*` JSON. All timestamps ISO-8601 UTC strings. All management mutations require owner auth (§5.1). Security-engineer's §4 contract notes (auth model, status matrix, SSRF policy, SSTI grammar, real-time gate, CORS, validation, plane-dispatch) are satisfied by the shapes below; the WS/SSE capability gate the security-engineer required (and the architect had not pinned) is now folded into **§5.4** as the owner gate (RESOLVED OQ-4).

### 5.1 Auth model (replaces `X-User-Id` header-trust)

- On `POST /api/session`, the server returns `{ owner_id, owner_secret, ... }`. `owner_id` = `hash_email(email)` (sha256→16 hex, **non-secret** identifier) `[reuses app/database.py:102]`. `owner_secret` = `secrets.token_urlsafe(32)` (**secret**, 256-bit), stored **hashed** (sha256) in `owners.secret_hash`.
- Every `/api/*` route **except** `POST /api/session` requires header **`Authorization: Bearer <owner_secret>`**. The server hashes the presented secret and looks up the owner; mismatch → **401**.
- Endpoint-scoped routes (`/api/endpoints/{token}/...`) additionally verify the resolved owner **owns** `{token}` → else **404** (not 403, to avoid confirming token existence to non-owners).
- The browser stores `{owner_id, owner_secret, token, mock_url}` in `localStorage` key `hookbox_owner`. The `owner_secret` is the recovery key surfaced as the access token; re-entering the same email reissues a working secret (rotates; see §5.2). **No password, no registration, no verification** (`_decisions.md` §5).

### 5.2 HTTP endpoints — management API (P2, app host only)

> Status codes: `200` ok, `201` created, `204` no content, `400` bad request, `401` unauthorized, `404` not found / not owned, `409` conflict, `422` validation (FastAPI/pydantic), `429` rate-limited. Error body shape everywhere: `{ "error": "<machine_code>", "detail": "<human>" }`.

| # | Method | Path | Auth | Request body | Success | Errors |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | POST | `/api/session` | none | `SessionCreate` | `200 SessionResponse` | `422` bad email |
| 2 | GET | `/api/endpoints` | bearer | — | `200 EndpointSummary[]` (owner-scoped) | `401` |
| 3 | POST | `/api/endpoints` | bearer | `EndpointCreate` | `201 EndpointDetail` | `401`, `422` |
| 4 | GET | `/api/endpoints/{token}` | bearer+own | — | `200 EndpointDetail` | `401`,`404` |
| 5 | PATCH | `/api/endpoints/{token}` | bearer+own | `EndpointConfigPatch` (partial) | `200 EndpointDetail` | `401`,`404`,`422` |
| 6 | DELETE | `/api/endpoints/{token}` | bearer+own | — | `200 Message` | `401`,`404` |
| 7 | GET | `/api/endpoints/{token}/rules` | bearer+own | — | `200 MockRule[]` (ordered by `priority,id`) | `401`,`404` |
| 8 | POST | `/api/endpoints/{token}/rules` | bearer+own | `MockRuleCreate` | `201 MockRule` | `401`,`404`,`422` |
| 9 | GET | `/api/endpoints/{token}/rules/{rule_id}` | bearer+own | — | `200 MockRule` | `401`,`404` |
| 10 | PATCH | `/api/endpoints/{token}/rules/{rule_id}` | bearer+own | `MockRulePatch` (partial) | `200 MockRule` | `401`,`404`,`422` |
| 11 | DELETE | `/api/endpoints/{token}/rules/{rule_id}` | bearer+own | — | `204` | `401`,`404` |
| 12 | GET | `/api/endpoints/{token}/requests` | bearer+own | query `?limit=50&offset=0` (limit ≤200) | `200 RequestSummary[]` (newest first) | `401`,`404`,`422` |
| 13 | GET | `/api/requests/{request_id}` | bearer+own | — | `200 RequestDetail` | `401`,`404` |
| 14 | DELETE | `/api/endpoints/{token}/requests` | bearer+own | — | `200 Message` (clears trace history) | `401`,`404` |
| 15 | GET | `/api/endpoints/{token}/state` | bearer+own | — | `200 {state: {k:v,...}}` | `401`,`404` |
| 16 | DELETE | `/api/endpoints/{token}/state` | bearer+own | — | `200 Message` (clears Redis state hash) | `401`,`404` |
| 17 | GET | `/api/endpoints/{token}/collections/{name}` | bearer+own | — | `200 {items:[...]}` (Auto-CRUD store peek) | `401`,`404` |
| 18 | DELETE | `/api/endpoints/{token}/collections/{name}` | bearer+own | — | `200 Message` | `401`,`404` |
| 19 | GET | `/healthz` | none | — | `200 {status:"ok", redis:bool, db:bool}` | — |

**Total management REST endpoints defined: 19.** (Plus the mock-plane catch-all, which is behavioral, not a fixed path — §5.5.)

### 5.3 Pydantic v2 models (field names + types — authoritative)

```python
# ---- Session / owner ----
class SessionCreate(BaseModel):
    email: EmailStr

class EndpointSummary(BaseModel):
    token: str
    name: str | None
    mock_url: str                 # https://<token>.<MOCK_DOMAIN>
    path_url: str                 # /e/<token>
    created_at: datetime
    last_hit: datetime | None
    request_count: int

class SessionResponse(BaseModel):
    owner_id: str                 # hash_email, non-secret
    owner_secret: str             # bearer token (only returned here)
    endpoints: list[EndpointSummary]
    primary: EndpointSummary      # convenience: first/created endpoint to route the browser to

# ---- Endpoint config ----
class EndpointCreate(BaseModel):
    name: str | None = Field(None, max_length=100)

class EndpointConfigPatch(BaseModel):       # all optional → partial update
    name: str | None = None
    auto_crud: bool | None = None
    target_url: str | None = None           # MITM upstream; "" or null clears
    default_mode: Literal["mock_404","echo"] | None = None
    latency_ms: int | None = Field(None, ge=0, le=10000)
    rate_limit_per_min: int | None = Field(None, ge=0, le=100000)   # 0 = unlimited
    chaos_pct: int | None = Field(None, ge=0, le=100)
    cors_enabled: bool | None = None

class EndpointDetail(BaseModel):
    token: str
    name: str | None
    mock_url: str
    path_url: str
    auto_crud: bool
    target_url: str | None
    default_mode: Literal["mock_404","echo"]
    latency_ms: int
    rate_limit_per_min: int
    chaos_pct: int
    cors_enabled: bool
    tunnel_active: bool
    created_at: datetime
    last_hit: datetime | None
    request_count: int

# ---- Mock rule (rich) ----
class MatchCriteria(BaseModel):
    method: str = "ANY"                      # "ANY" | GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS
    path: str = "/*"                         # exact, ":param" segments, or trailing "/*" wildcard
    headers: dict[str, str] = {}             # header name -> required value (case-insensitive name)
    query: dict[str, str] = {}               # query key -> required value
    body_conditions: list["BodyCondition"] = []   # jsonpath-lite equals checks
    state_requirements: list["StateRequirement"] = []

class BodyCondition(BaseModel):
    path: str                                # jsonpath-lite e.g. "user.role"
    op: Literal["eq","neq","contains","exists"] = "eq"
    value: str | None = None

class StateRequirement(BaseModel):
    key: str
    op: Literal["eq","neq","exists","absent"] = "eq"
    value: str | None = None

class StateWrite(BaseModel):
    key: str
    value: str                               # may contain template tags (rendered with same ctx)

class ResponseSpec(BaseModel):
    status_code: int = Field(200, ge=100, le=599)
    headers: dict[str, str] = {}
    body_template: str = ""                  # response body WITH template tags
    content_type: str = "application/json"

class MockRuleCreate(BaseModel):
    name: str | None = Field(None, max_length=120)
    priority: int = Field(100, ge=0, le=100000)   # lower = evaluated first
    enabled: bool = True
    match: MatchCriteria = MatchCriteria()
    response: ResponseSpec = ResponseSpec()
    state_writes: list[StateWrite] = []
    latency_ms: int | None = Field(None, ge=0, le=10000)        # overrides endpoint latency if set
    rate_limit_per_min: int | None = Field(None, ge=0, le=100000)
    webhook_action: "WebhookAction | None" = None              # see scope note §6 / PRD OQ-9

class WebhookAction(BaseModel):              # fire-and-forget outbound POST on match (optional feature)
    url: str
    body_template: str = ""

class MockRulePatch(BaseModel):              # all-optional mirror of MockRuleCreate for PATCH
    name: str | None = None
    priority: int | None = None
    enabled: bool | None = None
    match: MatchCriteria | None = None
    response: ResponseSpec | None = None
    state_writes: list[StateWrite] | None = None
    latency_ms: int | None = None
    rate_limit_per_min: int | None = None
    webhook_action: WebhookAction | None = None

class MockRule(MockRuleCreate):
    id: int
    token: str
    created_at: datetime

# ---- Traces ----
class RequestSummary(BaseModel):
    id: int
    token: str
    method: str
    path: str                                # the mock path (e.g. /users/5)
    status_code: int                         # served status
    served_by: Literal["rule","crud","mitm","tunnel","default","cors","chaos","ratelimit"]
    matched_rule_id: int | None
    duration_ms: int                         # total wall-clock (incl. applied latency)
    overhead_ms: int                         # our overhead = duration - applied latency - upstream
    timestamp: datetime

class TraceEvent(BaseModel):                 # the "State & Tracing Logs" tab payload, ordered
    step: str                                # e.g. "match", "state_read", "state_write", "template", "forward", "chaos"
    detail: str

class RequestDetail(RequestSummary):
    request_headers: dict[str, str]
    query_params: dict[str, str]
    request_body: str | None
    response_headers: dict[str, str]
    response_body: str | None
    trace: list[TraceEvent]
    state_snapshot: dict[str, str]           # state at time of request

# ---- Generic ----
class Message(BaseModel):
    message: str
    success: bool = True
```

### 5.4 WebSocket / SSE messages (real-time pipe)  **[OWNER-GATED — RESOLVED OQ-4, supersedes the architect's prior opportunistic-watch wording]**

- **Live feed WS:** `GET ws(s)://<app-or-mock-host>/ws/{token}?cap=<owner_secret>`. **Auth (FROZEN): the observability feed is owner-gated.** The `owner_secret` capability (§5.1) is presented on connect as the **`?cap=<owner_secret>` query parameter**; the server (1) resolves the endpoint for `{token}`, (2) hashes `cap` and verifies the resolved owner **owns** `{token}`, and (3) **only then calls `accept()`/joins the `trace:<token>` channel**. If the capability is absent, malformed, or does not own `{token}`, the server **closes the socket with WS close code `4401`** (application "unauthorized") **before `accept()` and before sending any frame** (no `hello`, no `new_request`). The capability is verified **server-side before channel-join** — an anonymous or wrong-owner subscribe never receives a single event. *(Chosen transport: query param over subprotocol/first-message because the gate must run **before** `accept()`; the `?cap=` value is available at the ASGI handshake. Per AC-S25 the `cap` query value is **never** written into a trace, log line, or rendered template.)*
- **SSE fallback:** `GET /sse/{token}?cap=<owner_secret>` (text/event-stream), **same owner-gate**: the capability is verified before the stream is opened; a missing/wrong capability returns **`401`** (JSON `{error:"unauthorized"}`) **before any `data:` frame**. Same payloads as `data:` events once authorized.
- **Mock interception plane stays public.** Only this observability feed (WS + SSE) is owner-gated; the P1 mock surface (§5.5) remains fully public/unauthenticated so clients can hit the mock URL.
- **Direction:** server → client only (feed). Client → server: none required; server ignores inbound text except `"ping"`.

| Direction | Event `type` | Payload (`data`) | When |
| --- | --- | --- | --- |
| s→c | `new_request` | `RequestSummary` (§5.3) | a request was served on `{token}`'s mock surface |
| s→c | `endpoint_updated` | `{ token, fields:[...] }` | endpoint config changed (so dashboard refreshes settings) |
| s→c | `state_changed` | `{ token, key, value }` | a rule mutated state |
| s→c | `hello` | `{ token, server_time }` | sent on connect (lets client sync + confirm channel) |
| c→s | `ping` | `"ping"` (raw text) | client keepalive; server replies WS pong frame |

**Total WebSocket/SSE message types defined: 5** (4 server→client events + 1 client→server keepalive). The 5 event **shapes** are unchanged by the OQ-4 resolution; only the **handshake auth** (now an owner gate via `?cap=`) changed.

> **Security override RESOLVED (OQ-4 — human-approved 2026-06-18, resolved in favor of SECURITY):** §5.4 is now **owner-gated**. The architect's prior "opportunistic, no-secret" WS auth is **superseded**: the capability is verified server-side **before `accept()`/channel-join** (WS) and before the first `data:` frame (SSE), because the live feed carries captured secrets in real time (security F6, SEC-AC-12). The chosen transport is the `?cap=<owner_secret>` query parameter (rejected with WS close `4401` / SSE `401`). This is FROZEN; see AC-S12, AC-29, AC-30, and §9 resolution log OQ-4.

### 5.5 Mock-surface behavior contract (P1 catch-all — behavioral, not a fixed path)
- **Reachable as:** any method, any path on `<token>.<MOCK_DOMAIN>/<path>` **or** `/<app-host>/e/<token>/<path>`.
- **Resolution order (frozen):** `OPTIONS preflight` → **matching rule** (by `priority`,`id`) → **Auto-CRUD** (if `auto_crud` and path looks like `/<collection>[/<id>]`) → **tunnel** (if `tunnel_active`) → **MITM** (if `target_url`) → **default** (`default_mode`: `mock_404` returns `404 {error:"no_match"}`; `echo` returns `200` echoing the request).
- **Every response** carries auto-CORS headers (§5.6 when `cors_enabled`) **plus** identifying headers: `X-HookBox-Endpoint: <token>`, `X-HookBox-Served-By: <served_by>`, `X-HookBox-Rule-Id: <id-or-absent>`.
- **Unknown vs expired token (FROZEN — RESOLVED OQ-1):** a mock request to a token that **never existed** returns `404 {error:"unknown_endpoint", detail:"..."}`; a mock request to a token that **did exist but was deleted (owner delete) or pruned/expired** returns `410 {error:"endpoint_gone", detail:"..."}`. **Both** carry header `X-HookBox-Endpoint: <token>` and **neither** is logged against any endpoint. The `410` lets the owner dashboard distinguish "expired — re-create" from "typo/never-existed."
- **Conditions order:** rate-limit (→`429 {error:"rate_limited"}` + `Retry-After: <sec>` + `X-RateLimit-Limit/Remaining`) → chaos (→ default: random `502|503|504` JSON `{error:"chaos"}`; opt-in `dropout` variant: the connection is closed without a response, bounded by the same global rate/size caps and a server-side drop timeout) → latency (`sleep` before returning). **(RESOLVED OQ-2:** chaos default = random 5xx; connection-drop is **opt-in** per rule/endpoint; **both** are bounded by the same global caps as every other path — no unbounded abuse vector.**)**

### 5.6 Auto-CORS header set (frozen)
- **Preflight** (`OPTIONS`, when `cors_enabled`): status `204`, headers:
  `Access-Control-Allow-Origin: <reflected Origin or *>`,
  `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`,
  `Access-Control-Allow-Headers: <reflected Access-Control-Request-Headers or *>`,
  `Access-Control-Max-Age: 600`,
  `Vary: Origin`.
- **Every non-preflight response** (when `cors_enabled`): `Access-Control-Allow-Origin: <reflected Origin or *>`, `Access-Control-Expose-Headers: *`, `Vary: Origin`.
- **`Access-Control-Allow-Credentials`:** **omitted** (not sent). Sending `*` for origin and `true` for credentials is invalid per the Fetch spec; since the spec demands "wide-open", we reflect the origin and **do not** claim credentials support. (Recorded as the resolution of PRD OQ-12.)
- **Plane scope (security §4.6, SEC-AC-16):** wide-open CORS is attached in the plane-dispatch path so it is emitted on **P1 only**; **P2 `/api/*` emits no wildcard CORS.**

### 5.7 Templating tag grammar + sandboxed engine (frozen)
- **Syntax:** `{{ <expr> }}`. The engine is a **single-pass scanner** (`app/interceptor/templating.py`) — it finds `{{`…`}}`, parses the inner expr against a fixed allow-list, substitutes the string result. **No `eval`/`exec`, no Jinja, no arbitrary attribute access.** Anything not on the allow-list is **left literal** (the raw `{{...}}` stays) — never 500s, never leaks internals (resolution of PRD OQ-8, AC-23).
- **Allow-list (exact):**

| Tag | Result |
| --- | --- |
| `{{now}}` / `{{now 'iso'}}` | ISO-8601 UTC timestamp |
| `{{now 'unix'}}` | epoch seconds |
| `{{now 'epoch_ms'}}` | epoch milliseconds |
| `{{random 'uuid'}}` | UUID v4 |
| `{{random 'int' MIN MAX}}` | random int in `[MIN,MAX]` |
| `{{random 'hex' N}}` | N random hex chars |
| `{{request.method}}` | request method |
| `{{request.path}}` | mock path |
| `{{request.query.<k>}}` | query param `k` (empty string if absent) |
| `{{request.path.<name>}}` | value captured by a `:name` segment in the rule's match path |
| `{{request.header.<name>}}` | request header (case-insensitive) |
| `{{request.body}}` | raw request body string |
| `{{request.body.<jsonpath>}}` | jsonpath-lite into a JSON body (dot path, `a.b.0.c`); empty if not JSON / not found |
| `{{state.<k>}}` | per-endpoint Redis state value (empty if unset) |

- **Quoting:** single quotes for literal args; bare tokens for numeric args. Whitespace-tolerant.
- **Failure mode:** malformed/unknown tag → left literal. JSON path miss → empty string. This is deterministic and testable.
- **DoS bound (security §4.4, SEC-AC-11):** the engine enforces a max template size / max tag count.

### 5.8 Data model — SQLite DDL (durable) + Redis keyspace (ephemeral)

**SQLite (`aiosqlite`, WAL).** Set on init: `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;`

```sql
CREATE TABLE IF NOT EXISTS owners (
    owner_id    TEXT PRIMARY KEY,             -- hash_email (non-secret)
    email       TEXT UNIQUE NOT NULL,
    secret_hash TEXT NOT NULL,                -- sha256(owner_secret)
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE TABLE IF NOT EXISTS endpoints (
    token              TEXT PRIMARY KEY,       -- generate_endpoint_id() (ambiguity-stripped)
    owner_id           TEXT NOT NULL,
    name               TEXT,
    auto_crud          INTEGER NOT NULL DEFAULT 0,
    target_url         TEXT,                   -- MITM upstream (nullable)
    default_mode       TEXT NOT NULL DEFAULT 'mock_404',  -- 'mock_404' | 'echo'
    latency_ms         INTEGER NOT NULL DEFAULT 0,
    rate_limit_per_min INTEGER NOT NULL DEFAULT 0,         -- 0 = unlimited
    chaos_pct          INTEGER NOT NULL DEFAULT 0,
    cors_enabled       INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    last_hit           TEXT,
    request_count      INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (owner_id) REFERENCES owners(owner_id)
);
CREATE INDEX IF NOT EXISTS idx_endpoints_owner ON endpoints(owner_id);

CREATE TABLE IF NOT EXISTS mock_rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL,
    name        TEXT,
    priority    INTEGER NOT NULL DEFAULT 100,
    enabled     INTEGER NOT NULL DEFAULT 1,
    match_json  TEXT NOT NULL DEFAULT '{}',    -- MatchCriteria serialized
    response_json TEXT NOT NULL DEFAULT '{}',  -- ResponseSpec serialized
    state_writes_json TEXT NOT NULL DEFAULT '[]',
    latency_ms  INTEGER,                       -- nullable override
    rate_limit_per_min INTEGER,                -- nullable override
    webhook_json TEXT,                         -- WebhookAction or null
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_rules_token ON mock_rules(token, priority, id);

CREATE TABLE IF NOT EXISTS request_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    token           TEXT NOT NULL,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    served_by       TEXT NOT NULL,             -- rule|crud|mitm|tunnel|default|cors|chaos|ratelimit
    matched_rule_id INTEGER,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    overhead_ms     INTEGER NOT NULL DEFAULT 0,
    request_headers TEXT,                      -- JSON
    query_params    TEXT,                      -- JSON
    request_body    TEXT,                      -- truncated to MAX_BODY_BYTES
    response_headers TEXT,                     -- JSON
    response_body   TEXT,                      -- truncated
    trace_json      TEXT,                      -- [TraceEvent]
    state_snapshot  TEXT,                      -- JSON of state at request time
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_logs_token_id ON request_logs(token, id DESC);
CREATE INDEX IF NOT EXISTS idx_logs_created ON request_logs(created_at);
```

**Redis keyspace (ephemeral):**

| Key | Type | Purpose | TTL |
| --- | --- | --- | --- |
| `state:<token>` | hash | per-endpoint state KV (AC-8/9/22) | `STATE_TTL` (default 24h, refreshed on write) |
| `crud:<token>:<collection>` | list | Auto-CRUD JSON elements | `CRUD_TTL` (default 24h, refreshed on write) |
| `rl:<token>` / `rl:<token>:<rule_id>` | string (token bucket via Lua) | rate-limit counter/window | window length |
| `trace:<token>` | pub/sub channel | live-feed fan-out (not a stored key) | — |
| `cfg:<token>` | pub/sub channel | rule-cache invalidation signal | — |

**Retention sweep (`app/utils/cleanup.py`, both caps, every `RETENTION_SWEEP_SECONDS`=300):**
```sql
-- 24h TTL
DELETE FROM request_logs WHERE created_at < datetime('now','-24 hours');
-- 100-cap per endpoint (keep newest 100 by id)
DELETE FROM request_logs WHERE id IN (
  SELECT id FROM request_logs r
  WHERE r.token = request_logs.token
  AND r.id NOT IN (SELECT id FROM request_logs r2 WHERE r2.token = r.token ORDER BY r2.id DESC LIMIT 100)
);
```
Plus **write-time prune** in `persist_and_publish` (delete rows beyond newest 100 for the just-written token) so the cap holds between sweeps. Both caps + interval are env-configurable (resolution of PRD OQ-13, AC-35/36/37).

### 5.9 Owner identity & email access (frozen)
- `POST /api/session` upserts `owners` by `owner_id=hash_email(email)`; on every call it **rotates** `owner_secret` (new secret, new `secret_hash`) and returns it — so an email holder can always recover access, and an old leaked secret stops working. (Security-engineer reviews entropy/enumeration in their pass; the contract guarantees the secret is 256-bit and never returned except on `/api/session`.)

### 5.10 Security-derived contract constraints (security §4 — fold-ins on the frozen shapes above)
> These do not change any shape in §5.1–§5.9; they pin behavior the implementation must satisfy and that QA will probe. They mirror security.md §4.
- **Validation (security §4.7):** `email→EmailStr` (422); `target_url`→validated URL enforcing the http(s) scheme + resolved-IP host policy (AC-S6/S7), not raw `str`; `latency_ms` clamped 0–10000; `rate_limit_per_min` ≥0 bounded; `chaos_pct` 0–100; `collection`/CRUD `:id`/state-keys → `^[A-Za-z0-9_-]{1,64}$`; rule `response.body_template` and request body → max-size bound (AC-S10a/AC-S18). Numeric clamps are both UX and DoS controls.
- **SSRF policy (security §4.3, resolves OQ-3):** http(s) only; block-list applied to the **resolved IP(s)** (loopback/private/link-local/ULA/metadata `169.254.169.254` + IPv6 equivalents) at request time **and on each redirect hop**; small max-redirects; connect+read timeout; max response bytes; strip hop-by-hop + sensitive inbound headers and **never forward the owner capability** upstream. Default: block-list on, no allow-list (`MITM_ALLOW_PRIVATE=false`).
- **Plane-dispatch (security §4.8, LOCKED §2):** the Host+path → P1/P2/P3 decision (arch §3.1) is security-critical and ordered so the P1 mock catch-all can never match `/api`, `/d`, `/static`, `/ws` on the app host, and `/api` under a mock host is mock traffic (AC-6).
- **Status matrix (security §4.2):** as pinned in §5.2 (`401` no/invalid cap; `404` valid-cap-not-owner, uniformly), §5.5 (`404` unknown token / `410` known-but-deleted-or-expired, **neither** logged; `429`+`Retry-After`; MITM `502`/`504`), AC-S18 (`413` ingest oversize), §5.4 (WS close `4401` / SSE `401` on a missing/wrong feed capability before any frame).

### 5.11 Redis-down per-feature degradation table (FROZEN — RESOLVED OQ-3, human-approved)
> The exact per-feature fail-open vs fail-closed policy is authoritative below (verbatim from the human-approved OQ resolutions). It does not change any shape in §5.1–§5.9; it pins behavior QA + security probe (AC-49, AC-S20). The mock fast-path **match read** never touches Redis (it reads the in-process `rule_cache`), so static mock matching survives a Redis outage.

| Feature | Redis-down behavior |
|---|---|
| Static mock matching | **Survives** — served from the in-process rule cache (no Redis on the match read path). |
| State-gated rules (read/require/mutate) | **Fail-CLOSED** — the state condition does not match (rule is skipped); never silently "match" a state-gated rule without state. |
| Auto-CRUD (data in Redis) | **503** — do not fabricate/lose data; surface a clear degraded error. |
| Rate limiter (token bucket) | **Fail-OPEN** — allow the request, but the global body/size caps (in-process, not Redis-backed) still apply, so it is not unbounded. |
| Real-time feed (pub/sub) | Mock serving + SQLite trace logging **unaffected**; the dashboard shows a **"degraded" pill** and may fall back to polling. |

### 5.12 Tunnel control-channel protocol (FROZEN — RESOLVED OQ-5; supplements arch §4.6)
> Lifted from arch §4.6 with the human-approved OQ-5 bind/contention semantics folded in. Frames and resolution precedence are unchanged from arch §4.6; the **authenticated bind transport** and **slug-contention (takeover)** are now pinned.

- **Bind transport (FROZEN):** the `mock-tunnel` CLI connects to `GET ws(s)://<app-host>/ws/tunnel/{slug}` and presents the endpoint's **owner capability over the WebSocket control channel** — `Authorization: Bearer <owner_secret>` on the WS handshake (same capability model as §5.1). The server verifies the resolved owner **owns** `{slug}` **before** registering the tunnel / before `accept()`; an unauthenticated or wrong-owner bind is **rejected** (WS close `4401`, no registration). Because binding is capability-gated, cross-owner hijack is impossible.
- **Slug contention (FROZEN): last authenticated bind wins (takeover).** A second *correctly-authenticated* (owner) CLI binding an already-bound `{slug}` **takes over**: the server registers the new tunnel and **closes the prior tunnel connection** with a clear "rebound elsewhere" message (a `{t:"err", message:"rebound elsewhere"}` frame then WS close). The new tunnel serves subsequent public traffic.
- **Frames (unchanged, arch §4.6):** `→client {t:"req", id, method, path, query, headers, body_b64}` · `←client {t:"res", id, status, headers, body_b64}` · `↔ {t:"ping"}/{t:"pong"}` · `←client {t:"err", id, message}`.
- **No tunnel connected:** public callers get `504 {error:"no_tunnel"}` (not a hang) (arch §4.6).
- **Resolution precedence with tunnel (unchanged):** `rule > CRUD > tunnel (if active) > MITM (if target) > default` (§5.5/arch §4.6).
- **Public tunnel traffic is bounded:** requests entering the tunnel branch are subject to the **same** ingest body cap (AC-S18) and rate-limit (AC-S19) as every other P1 path.

## 6. Affected files (existing — verified; superseded or rewritten by this work)

> Action verbs mirror `architecture.md` §2 (REPLACE = rewrite contents; DELETE = remove).

- `app/main.py` **[existing — verified]** — REPLACE: app factory + lifespan (init_db, Redis pool, pub/sub relay, retention task, warm rule cache), mount `/static`, register `PlaneDispatchMiddleware`, include api/ui routers, WS routes (`/ws/{token}`, `/ws/tunnel/{token}`), mock catch-all mounted **last**. **Remove** `/hook/{user_id}/{endpoint_id}` (lines 41–92), **`/status` crypto (lines 123–268)**, `/login`/`/register`/`/backup`.
- `app/models.py` **[existing — verified]** — REPLACE: pydantic v2 models for §5.3; drop `UserRegister`/`UserLogin`.
- `app/database.py` **[existing — verified]** — REPLACE: WAL pragma, §5.8 DDL (`owners`,`endpoints`,`mock_rules`,`request_logs`), keep `hash_email` (line 102), add `gen_owner_secret`, `gen_token`, fire-and-forget trace insert + write-time prune.
- `app/routes/api.py` **[existing — verified]** — REPLACE: §5.2 endpoints with real owner-capability auth (replaces `get_current_user` header-trust, line 24).
- `app/routes/backup.py` **[existing — verified]** — **DELETE** (SMTP removed; export/restore cut — §2 non-goal).
- `app/routes/webhook.py` **[existing — verified]** — **DELETE** (GitHub auto-deploy `git pull`+`os.system`/`pkill`, RCE-class — LOCKED §10).
- `app/websocket.py` **[existing — verified]** — REPLACE: `ConnectionManager` local registry now **fed by `app/pubsub.py`** (Redis relay); add SSE generator; per-client send timeout drops slow clients (extends the disconnect-on-error pattern, line 31).
- `app/utils/helpers.py` **[existing — verified]** — REPLACE/extend: keep `generate_endpoint_id` (ambiguity-stripped) + `format_headers` (hop-by-hop strip, line 18); add `gen_owner_secret`, jsonpath-lite getter, RNG seam for chaos.
- `app/utils/cleanup.py` **[existing — verified]** — REPLACE: both caps (100-trace + 24h) every `RETENTION_SWEEP_SECONDS` (§5.8).
- `config.py` **[existing — verified]** — REPLACE/extend: `MOCK_DOMAIN`, `APP_HOST/PORT`, `REDIS_URL`, `DATABASE_PATH`, retention caps/interval, bounds, MITM policy, token entropy, ingest body cap.
- `requirements.txt` **[existing — verified]** — REPLACE: pin fastapi/`uvicorn[standard]`/aiosqlite/`pydantic>=2`/python-multipart; **add** `redis>=5`, `httpx`, `jinja2`, `itsdangerous`, `websockets`, `typer`/argparse (CLI).
- `Dockerfile` **[existing — verified]** — REPLACE: `python:3.12-slim`, install reqs, `CMD ["uvicorn","app.main:app",...]`, non-root user, `HEALTHCHECK /healthz`.
- `docker-compose.yml` **[existing — verified]** — REPLACE: `app` + `redis`, healthchecks, `depends_on: service_healthy`, named volumes (`hookbox_data`, `hookbox_redis` `--appendonly yes`), internal network.
- `templates/base.html` **[existing — verified]** — REPLACE: Tailwind Play CDN + Alpine.js + HTMX + `defer` `static/js/*`; de-emoji brand; slim nav variant; `{% block fullbleed %}`; localStorage owner→Alpine store; remove `/backup` nav.
- `templates/index.html` **[existing — verified]** — REPLACE: email entry → `POST /api/session` → localStorage → `/d/<token>` (collapses today's list + login/register).
- `templates/dashboard.html` **[existing — verified]** — REPLACE: split-screen (left feed / right inspector tabs), WS pill, settings/rule-modal mounts.
- `templates/mock.html`, `login.html`, `register.html`, `backup.html` **[existing — verified]** — **DELETE** (folded into modal/settings; no password/registration/backup wall).
- `app/__init__.py`, `app/routes/__init__.py`, `app/utils/__init__.py` **[existing — verified]** — update exports as modules change.
- `README.md`, `FEATURES.md` **[existing — verified]** — REPLACE: new platform docs, env vars, `*.localhost`/`nip.io` recipe, tunnel usage, compose.
- `reset_db.sh` **[existing — verified at repo root]** — documents the dev wipe (ground-up replacement, not migration; arch §7).

## 7. New files (to be created) — per `architecture.md` §2

**Backend lane (`app/`, `config.py`, `tunnel/`):**
- `app/middleware.py` **[new]** — `PlaneDispatchMiddleware` (Host+path → plane/token; AC-6/6a).
- `app/planes.py` **[new]** — pure plane-resolution (host→token, `/e/<token>` parse), unit-testable.
- `app/interceptor/engine.py` **[new]** — ordered pipeline `handle_mock` (AC-13/§5.5 resolution order; AC-38/39).
- `app/interceptor/matcher.py` **[new]** — method/path/header/query/body/state selection, first-by-`priority,id` (AC-33b).
- `app/interceptor/templating.py` **[new]** — sandboxed `{{...}}` scanner (AC-20–23, AC-S10/S11; §5.7).
- `app/interceptor/crud.py` **[new]** — Redis-backed Auto-CRUD (AC-11/12/12a/12b).
- `app/interceptor/proxy.py` **[new]** — `httpx` MITM forward + SSRF guard + caps (AC-14–17, AC-S6–S9).
- `app/interceptor/conditions.py` **[new]** — latency/chaos/rate-limit (AC-24/25/26, AC-S19; seeded RNG).
- `app/interceptor/cors.py` **[new]** — auto-CORS preflight + header set (AC-18/19, AC-S16/S17; §5.6).
- `app/redis_state.py` **[new]** — Redis facade: state KV, CRUD store, token bucket, pub/sub, TTLs (AC-8–10, 25, 27, 32, AC-S20/S24).
- `app/pubsub.py` **[new]** — subscribe `trace:*`+`cfg:*`; relay to WS/SSE; invalidate rule cache (AC-27/32, AC-34).
- `app/rule_cache.py` **[new]** — in-process `dict[token]→CompiledEndpoint` (AC-38/39, AC-49).
- `app/auth.py` **[new]** — `require_owner` + `assert_owns_endpoint` (AC-S1–S5; §5.1).
- `app/routes/ui.py` **[new]** — server-rendered `/`, `/d/{token}` (P3).
- `app/routes/tunnel.py` **[new]** — tunnel WS control-channel server (AC-40/40a/41, AC-S27; arch §4.6).
- `tunnel/mock_tunnel.py` + `tunnel/__init__.py` + `tunnel/README.md` **[new]** — `mock-tunnel` reference CLI (AC-40/41, LOCKED §8).

**Frontend lane (`templates/`, `static/`):**
- `static/css/app.css` **[new]** — design tokens `:root` + method-badge `.m-*` map + keyframes (`hb-pulse`,`hb-flash`,skeleton) + `prefers-reduced-motion` block + `--shadow-*`/`--ring` (design §9). (Or these live in `base.html` `<style>` per architect's CDN choice; either is fine.)
- `static/js/request-stream.js` **[new]** — `useRequestStream` substitute: WS open, exponential backoff + jitter, dedupe, cap, health pill, heartbeat, visibility pause, SSE fallback (AC-30/30a–d, LOCKED §1a).
- `static/js/stores.js` **[new]** — Alpine stores: `feed`, `inspector` (lazy `GET /api/requests/{id}`), `endpoint`, `rules`.
- `static/js/rule-builder.js` **[new]** — multi-tab modal logic; serializes `MockRuleCreate`/`MockRulePatch` (AC-33/34).
- `static/js/util.js` **[new]** — shared `showToast`/`copyToClipboard`/`formatDate`/escaper (de-duplicate from old templates; AC-S14/S15).
- `templates/partials/feed_row.html` **[new]** — one feed row; the unit `request-stream.js` clones (AC-28, AC-VC1/VC9, AC-27a).
- `templates/partials/inspector.html` + `inspector_body_tree.html` **[new]** — tabbed inspector + recursive JSON/XML tree (AC-31/31a–c, AC-S14/S15).
- `templates/partials/rule_row.html`, `rule_modal.html`, `endpoint_settings.html` **[new]** — rule list/modal/settings (AC-33/34, AC-10/11/14, AC-48).

> **Infra Dockerfile count (RESOLVED draft §7 ARCH-GAP):** the architect chose a **single app Dockerfile + the stock Redis image** (no FE build step) — not split FE/BE Dockerfiles (arch §2 infra).

## 8. Risks & assumptions

- **R1 — <10 ms vs Python.** Mitigated by the in-process `rule_cache` (no per-request SQLite read on the matched path), single-pass templating, and **fire-and-forget** trace+publish (AC-38/39). Residual awaited I/O = optional state read + optional rate-limit `EVAL` against local Redis (sub-ms). Benchmark harness per AC-38 (arch §9 R1).
- **R2 — Auth IDOR.** Old `X-User-Id` trust replaced by hashed-secret bearer + per-route ownership → `404` for non-owners (AC-S1–S5; arch §5.1; security F2/F3). Closed in design; QA + security gates verify.
- **R3 — MITM SSRF.** `proxy.forward` vets the resolved IP, every hop, http(s) only, caps body+timeout, no-redirect default (AC-S6–S9; security §4.3). Highest external attack surface.
- **R4 — SSTI in templating.** Hand-written allow-list scanner, no Jinja/eval over user text (AC-S10/S11; security F5).
- **R5 — Stored XSS in inspector.** Confirmed in prior art (`dashboard.html:213-235`); fixed by Jinja autoescape + `textContent`/escaper, no raw `innerHTML`, no `|safe` on captured data (AC-S14/S15; security F7).
- **R6 — Redis down.** Mock fast path survives (matching reads cache); state/CRUD/rate-limit degrade per a documented policy; dashboard shows "Realtime degraded" (AC-49, AC-S20). **The exact per-feature fail-open/closed table is still OPEN (OQ-3).**
- **R7 — Redis durability for state/CRUD.** AOF on a named volume; contract treats Redis as best-effort durable; a wipe loses live state/collections, never config/rules/history (arch §7 R7). Rate-bucket reset on restart = a free burst window — accepted (security SOQ-6).
- **R8 — WS fan-out backpressure.** Per-client send timeout drops slow clients; feed capped client-side at 100 (AC-30a; arch §9 R8); connection cap per token (AC-S22).
- **R9 — Tunnel abuse / slug-hijack.** Bind requires `owner_secret` (AC-S27); single-instance scope; Go binary deferred (arch §9 R9). **Slug-contention + bearer-over-WS acceptance still OPEN (OQ-5).**
- **R10 — Email enumeration.** Constant-shape session + per-source rate-limit + non-secret-id + secret-rotation (AC-S5; arch §5.9; security F1).
- **R11 — Chaos/rate-limit determinism for tests.** Injectable seeded RNG; `chaos_pct` 100/0 framing; atomic Redis Lua bucket (AC-26; arch §9 R6).
- **R12 — Background-task loss on crash.** Fire-and-forget traces can be lost if the process dies mid-task; acceptable (debug telemetry, not transactional); response never at risk (arch §9 R4).
- **Assumptions:** single app instance; one Redis; one SQLite file on a volume; Tailwind via Play CDN (architect's choice); `MOCK_DOMAIN` configured by operator (default `mock.local`); HTTP/1.1 + WS sufficient; a fresh data volume per `_decisions.md` §0 (no migration from the toy schema).

## 9. Open Questions  (MUST be empty before lock)

**None. §9 is empty — all blocking open questions are resolved; the §5 contract is FROZEN.**

The five formerly-blocking questions were resolved by the human on 2026-06-18 (`docs/features/beeceptor-rewrite/_oq-resolutions.md`, authoritative) and folded into §4/§5 in this REVISE-FINALIZE pass. They are recorded in the resolution log below; nothing remains open.

### §9 Resolution log (formerly-blocking OQ-1…OQ-5 — RESOLVED, recorded for the lock/QA/security trace)

- **OQ-1 (expired-vs-deleted mock status) — RESOLVED.** Decision: **`410 {error:"endpoint_gone"}`** for a known-but-deleted/pruned/expired endpoint; **`404 {error:"unknown_endpoint"}`** for a never-existed token; neither logged. Folded into **§5.5**, **§5.10 status matrix**, and **AC-7 / AC-7a**.
- **OQ-2 (chaos `dropout` semantics) — RESOLVED.** Decision: default chaos = random **5xx** (502/503/504); raw **connection-drop is opt-in** per rule/endpoint; **both** bounded by the same global rate/size caps (and a server-side drop timeout). Folded into **§5.5** and **AC-26**.
- **OQ-3 (Redis-down per-feature degradation table) — RESOLVED.** Decision: the authoritative per-feature table — static matching **survives**; state-gated rules **fail-CLOSED**; Auto-CRUD **503**; rate limiter **fail-OPEN but bounded**; real-time feed **degraded pill / polling fallback** while mock+SQLite are unaffected. Folded verbatim into **§5.11** and **AC-49 / AC-S20**.
- **OQ-4 (real-time pipe capability gate) — RESOLVED in favor of SECURITY.** Decision: the WS **and** SSE observability feed is **owner-gated** — capability presented via **`?cap=<owner_secret>`** and verified server-side **before `accept()`/channel-join** (WS) / before the first `data:` frame (SSE); reject **WS close `4401` / SSE `401`** before any frame; the mock interception plane stays public. Supersedes the architect's prior opportunistic-watch §5.4 wording. Folded into **§5.4**, **AC-S12**, **AC-29**, **AC-30**.
- **OQ-5 (tunnel bind transport + slug contention) — RESOLVED.** Decision: bind authenticates with the **owner capability (`Authorization: Bearer <owner_secret>`) over the WS control channel**, verified to own `<slug>` before registration; **slug contention = last authenticated bind wins (takeover)** — a second correctly-authenticated owner CLI takes over and the prior connection is closed "rebound elsewhere"; cross-owner hijack is impossible. Folded into **§5.12**, **AC-S27**, **AC-40**.

**Resolved earlier in this revision (recorded for the lock checklist; no longer open):**
- OQ-7 export/restore → **CUT** (§2 non-goal). · OQ-8 templating grammar/failure → §5.7. · OQ-9 "Webhook Actions" scope → **deferred, field retained** (AC-33a, §2). · OQ-10 served-by enum + identifying headers → §5.3/§5.5, AC-31b. · OQ-11 WS vs SSE + inline-vs-lazy detail → §5.4 (WS primary, SSE fallback; detail lazy-loaded, AC-31a). · OQ-12 CORS header set + credentials + chaos/rate-limit details → §5.5/§5.6 (note chaos `dropout` granularity moved to OQ-2). · OQ-13 retention mechanism + interval → §5.8 (write-time + sweep @300s). · OQ-14 `MOCK_DOMAIN` canonical example + fallback → §3 (default `mock.local`, path-only fallback when unset). · OQ-15 design palette/motion/contrast → AC-VC1/VC2/VC4/VC9/VC16/VC17. · Draft §5 `[ARCH-GAP]`s (auth, status, MITM, CRUD storage, state reset, precedence, tag grammar, WS path, Dockerfile count) → all filled by architecture §5 and §7. · Design-agent gaps 1–4,7 → AC-VC*/§3 viewport commit. · UX gaps 2,3,4,5,6,9,10,11,13,15,16,17,18 → ACs above. · Journey gaps 1–18 → ACs/non-goals above (gap 16 replay = non-goal; gaps 5/8 = AC-49/AC-S6–S9; gap 6 state = AC-10/§6.5; remaining residuals are OQ-1/2/3/5).

## 10. Task graph (beads)

**Feature epic:** `hookbox-wrd` (HookBox: Beeceptor-class API mocking & interception platform — full rewrite). Built in BREAKDOWN (`hookbox-wrd.9`) after the human approved this PRD and resolved the 5 blocking OQs. **19 build tasks (13 backend + 6 frontend), each `feature:beeceptor-rewrite` + lane-tagged**, all independently completable against the frozen §5; every one of the 100 ACs maps to ≥1 task. Gates: **QA** (`hookbox-wrd.29`) blocked by all 19 tasks → **security review** (`hookbox-wrd.30`) blocked by QA → **sync** (`hookbox-wrd.31`) blocked by security. `bd dep cycles` = none.

### Issue → AC index (traceability)

**Backend lane (`app/`, `config.py`, `requirements.txt`, Docker, `tunnel/`):**

| Issue | Task | §5 contract part | ACs covered |
| --- | --- | --- | --- |
| `hookbox-wrd.10` | BE1 Data layer (SQLite WAL + DDL + conn mgmt + fire-and-forget insert + write-time prune) | §5.8, §5.3 | AC-35, AC-39, AC-S23 |
| `hookbox-wrd.11` | BE2 Redis layer (state KV, Lua token bucket, pub/sub, TTLs, validated keys) | §5.8, §5.11 | AC-8, AC-10a, AC-25, AC-S20 |
| `hookbox-wrd.12` | BE3 Capability/auth + ownership + management `/api/*` | §5.1, §5.2, §5.3, §5.9 | AC-1, AC-2, AC-10, AC-S1, AC-S2, AC-S3, AC-S4, AC-S5, AC-S25 |
| `hookbox-wrd.13` | BE4 3-plane Host+path dispatch & isolation | §5.5 (unknown 404 / gone 410) | AC-4, AC-5, AC-6, AC-6a, AC-7, AC-7a |
| `hookbox-wrd.14` | BE5 Interceptor engine + matcher + precedence + rule cache | §5.5, §5.11 | AC-9, AC-13, AC-33b, AC-34, AC-38, AC-49 |
| `hookbox-wrd.15` | BE6 Sandboxed templating engine | §5.7 | AC-20, AC-21, AC-22, AC-23, AC-S10, AC-S11 |
| `hookbox-wrd.16` | BE7 MITM forward + SSRF guard | §5.5, §5.10 | AC-14, AC-15, AC-16, AC-17, AC-S6, AC-S7, AC-S8, AC-S9 |
| `hookbox-wrd.17` | BE8 Auto-CRUD engine | §5.5 | AC-11, AC-12, AC-12a, AC-12b |
| `hookbox-wrd.18` | BE9 Auto-CORS engine | §5.6 | AC-18, AC-19, AC-S16, AC-S17 |
| `hookbox-wrd.19` | BE10 Simulated network conditions (latency/rate/chaos; OQ-2) | §5.5 | AC-24, AC-25, AC-26, AC-27c, AC-S18, AC-S19 |
| `hookbox-wrd.20` | BE11 WS+SSE feed (OQ-4 gate) + pub/sub fan-out + retention sweep | §5.4, §5.8 | AC-27, AC-32, AC-S12, AC-S13, AC-S22, AC-36, AC-37 |
| `hookbox-wrd.21` | BE12 Tunnel server + `mock-tunnel` CLI (OQ-5) | §5.12, §5.5 | AC-40, AC-40a, AC-41, AC-S27 |
| `hookbox-wrd.22` | BE13 Docker + compose + config env + cruft removal | §5.8, infra | AC-42, AC-43, AC-44, AC-45, AC-S26 |

**Frontend lane (`templates/`, `static/`):**

| Issue | Task | §5 contract part | ACs covered |
| --- | --- | --- | --- |
| `hookbox-wrd.23` | FE1 Base template + Tailwind + design tokens/dark theme | (styling; design.md) | AC-VC2, AC-VC17 |
| `hookbox-wrd.24` | FE2 Email entry / landing | §5.2 #1, §5.3 SessionResponse, §5.1 | AC-3, AC-3a |
| `hookbox-wrd.25` | FE3 Dashboard shell + live feed + `request-stream.js` (OQ-4 cap) | §5.4, §5.3 RequestSummary | AC-27, AC-27a, AC-28, AC-VC1, AC-VC9, AC-29, AC-VC4, AC-30, AC-30a, AC-30b, AC-30c, AC-30d, AC-32 |
| `hookbox-wrd.26` | FE4 Deep inspector tabs + XSS-safe rendering | §5.3 RequestDetail/TraceEvent, §5.5 headers, §5.2 #13 | AC-31, AC-31a, AC-31b, AC-31c, AC-S14, AC-S15 |
| `hookbox-wrd.27` | FE5 Multi-tab Create-Rule modal + validation | §5.3 MockRuleCreate/Patch, §5.2 #7–11 | AC-33, AC-33a, AC-34, AC-VC14 |
| `hookbox-wrd.28` | FE6 Endpoint settings + multi-endpoint switcher + per-screen states + a11y | §5.3 EndpointConfigPatch/Detail/Summary, §5.2 #2,#3,#5,#16 | AC-3b, AC-10, AC-11, AC-24, AC-25, AC-27c, AC-46, AC-47, AC-VC16, AC-48 |

**Gates:** `hookbox-wrd.29` QA (validates all 100 ACs + frozen §5; depends on `.10`–`.28`) · `hookbox-wrd.30` Security review (code-level, depends on QA) · `hookbox-wrd.31` Sync (close epic · export · push; depends on security).

> Some ACs are split FE↔BE by lane and so appear under both a BE and an FE task against the same frozen §5 shape (each lane independently completable): **AC-10** (BE3 state API ↔ FE6 Clear-state button), **AC-11** (BE8 CRUD engine ↔ FE6 toggle), **AC-24/AC-25/AC-27c** (BE10 enforcement ↔ FE6/FE3 controls), **AC-27/AC-32** (BE11 publish/scope ↔ FE3 render), **AC-3b** (BE3 `GET/POST /api/endpoints` ↔ FE6 switcher). The SEC-AC-24 key-safety requirement is covered via **AC-10a** (BE2) + **AC-12a** (BE8).
