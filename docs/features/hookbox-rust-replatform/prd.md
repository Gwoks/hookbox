# PRD: HookBox — Rust/Axum + SQLite Re-platform (slug: hookbox-rust-replatform)

- **Status:** REVISED — lockable. §5 lifted verbatim from `architecture.md` (FROZEN); §9 empty.
- **Date:** 2026-06-21
- **Authoritative inputs:** `docs/superpowers/specs/2026-06-21-hookbox-rust-replatform-design.md` (APPROVED, locked north star), `architecture.md` (AUTHORITATIVE — owns the frozen §5 contract), `journey.md`, `ux.md`, `design.md`, `copy.md`, `security.md`, `FEATURES.md` (the 13 canonical feature areas), the current Python implementation under `app/` (verified, cited per behavior), and `../shortener-link/` (the reference Rust+SQLite+Vite/React project whose conventions we mirror).
- **Downstream consumers:** approval → BREAKDOWN → FE ∥ BE lanes → QA gate → security review → sync.

> **§5 is FROZEN.** It is lifted verbatim from `architecture.md` §5 (the system-architect owns it). The architect resolved OQ-1 (`gone_at` tombstone, `410` vs `404`, 7d hard-delete), OQ-2 (`chaos_mode "error"|"dropout"` promoted to a first-class column + per-rule override), OQ-4 (serve port `8080`); security resolved OQ-3 (single SQLite store: state-gated rules fail CLOSED, in-memory rate limiter fails OPEN but bounded, CRUD/store error → stable-enveloped 5xx). All open questions are resolved — §9 is empty.

---

## 1. Problem & goal

HookBox today is a Python **FastAPI + Jinja + SQLite + Redis + WebSockets** Beeceptor-class mock/intercept platform (verified across `app/`, `templates/`, `config.py`). It is operationally heavy for a self-host single-node tool: it needs Redis alongside SQLite, ships a server-rendered Jinja dashboard with hardcoded-hex styling (`templates/base.html` lines 27–44, 124–127), and carries a two-service Docker topology. The goal is to recreate HookBox as a **feather-weight, self-contained** application on the exact platform `../shortener-link` uses: a **single Rust (Axum) binary over one WAL-mode SQLite file** that also serves a **Vite + React + TypeScript SPA**. **No Redis, no Postgres, Docker optional.** Single-instance by design — every responsibility Redis carries today (per-endpoint state, Auto-CRUD storage, rate-limit buckets, the live-feed pub/sub) moves to **in-process structures + SQLite**. The recreation is **full feature parity across all 13 HookBox feature areas** with **byte-faithful preservation** of the externally observable contract (resolution order, `X-HookBox-*` headers, status codes, caps/limits, management-API shapes — §5), while the frontend gets a **fresh, light, instrument-grade visual identity and a distinct product voice** (`design.md` + `copy.md`), not a 1:1 visual port of the current dashboard.

## 2. Non-goals

- **Multi-instance / horizontal scale.** Deliberately traded away with Redis (spec §2). In-process broadcast/buckets/registries are single-process; the self-host sweet spot is vertical scale on one node.
- **Keeping the Python codebase runnable.** `app/`, `templates/`, `config.py`, `requirements*.txt`, `tunnel/`, `Dockerfile`, `docker-compose.yml` are retired (kept in git history). They are the *parity reference*, not a deliverable.
- **Email delivery / verification / passwords / registration.** The email-keyed, capability-backed model is preserved exactly (`app/auth.py`, `app/database.py`): email → non-secret `owner_id` + a minted 256-bit owner capability that rotates on each submit. No new identity surface.
- **New product features.** This is a re-platform at parity, not a feature expansion. `webhook_action` remains accepted-and-stored-but-no-op (`app/models.py::WebhookAction`); the SPA renders it as a visible-but-disabled "Stored, not yet sent" control (not omitted) so the data shape round-trips.
- **A documented public REST API as a product.** `/api/**` exists for the SPA; it is not a versioned public surface.
- **A template render-preview in the rule builder.** A live preview would need the *same* closed scanner or it would lie about behavior; intentionally absent (`copy.md` §6.1). The Templating tab inserts tags; it does not render them.
- **New collection-browser / state-viewer screens beyond a minimal peek.** Settings exposes a lightweight collection peek/clear and state clear (parity with the API peek/clear routes); a full data-browser is out of scope (resolved §9 note J9).

## 3. Users & context

A developer self-hosting HookBox to mock, intercept, and inspect HTTP traffic. They reach HookBox through three hard-isolated request planes decided purely from `Host` + path (`app/planes.py::resolve_plane`):

- **P1 — mock plane** (`<token>.<MOCK_DOMAIN>/<path>` and `/e/<token>/<path>`): fully hostile, unauthenticated, internet-facing; the entire interceptor pipeline runs here.
- **P2 — management API** (`/api/**`): capability-gated except `POST /api/session`.
- **P3 — dashboard** (`/`, `/d/<token>`, static assets, `/ws/*`, `/sse/*`, `/healthz`): the React SPA + the cap-gated WS/SSE live feed.

SPA screens the SPA must deliver (grounded in `journey.md` + `ux.md`, parity with `templates/index.html`, `templates/dashboard.html`, `templates/partials/*`): **landing / email gate** (`/`), **split-screen dashboard** (`/d/:token`: live feed + deep inspector), **rules manager** + **5-tab rule builder**, **endpoint settings** (incl. Danger zone, collection peek, state/history clear), and a public **tunnel / CLI** page (`/cli`). The `tunnel` CLI itself is terminal-only (no GUI), with the dashboard reflecting `tunnel_active`. All user-facing strings are the keys in `copy.md` §4–§5, wired 1:1 by the FE.

---

## 4. Acceptance criteria — all 13 parity areas + security (AC-S*) + visual (AC-D*) + journey-state (AC-J*)

Each AC is testable and observable. **[existing — verified at `<path>`]** = ported faithfully (byte-level external contract preserved); **[new]** = Rust/React artifact to create. Caps reference `config.py` (verified) and become env-driven config. The QA lane (AC-55) validates every AC and the full §5 contract on both FE and BE sides; the security review gate validates AC-S1…AC-S19 against the implemented code.

### Area 1 — Email-keyed access (no password) — `FEATURES.md §1`
- **AC-1** `POST /api/session {email}` returns `200` with `{owner_id, owner_secret, endpoints[], primary}`; the response shape and status are **identical for a brand-new and an existing email** (anti-enumeration). [existing — verified at `app/routes/api.py::create_session`, shape in `app/models.py::SessionResponse`]
- **AC-2** Each `POST /api/session` **rotates** the owner capability: a freshly minted 256-bit secret is returned and stored hashed (sha256); the previously issued secret stops authenticating (`401`). [existing — verified at `app/auth.py`, `app/database.py::gen_owner_secret/hash_secret`]
- **AC-3** If the owner has no endpoints, `POST /api/session` auto-provisions one and returns it as `primary`. [existing — verified at `app/routes/api.py::create_session` → `_new_endpoint`]
- **AC-4** `owner_id` is `sha256(lower(trim(email)))[:16]` and is **never** accepted as a credential — only the hashed secret is looked up. [existing — verified at `app/database.py::hash_email`, `app/auth.py::require_owner`]
- **AC-5** `POST /api/session` is per-source-IP rate-limited (`SESSION_RATE_LIMIT_PER_MIN`, default 30); over-limit returns `429` with `Retry-After`. [existing — verified at `app/routes/api.py::create_session`]

### Area 2 — Wildcard mock surface + 3 isolated planes — `FEATURES.md §2`
- **AC-6** A request to `<token>.<MOCK_DOMAIN>/<path>` resolves to the **mock plane (P1)** with that token, and everything on that host (including `/api`, `/static`) is treated as the mock's own path. [existing — verified at `app/planes.py::resolve_plane/subdomain_of`]
- **AC-7** The path-fallback `/e/<token>/<rest>` resolves to P1 with `mock_path="/<rest>"` (and `/e/<token>` → `mock_path="/"`); a malformed `/e` with no token is a UI 404. [existing — verified at `app/planes.py::_path_fallback_token`]
- **AC-8** On the app host, `/api/**` is the **management plane (P2)** and `/`, `/d/<token>`, static assets, `/ws/*`, `/sse/*`, `/healthz` are the **dashboard plane (P3)**; the bare apex, `localhost`, `127.0.0.1`, `[::1]`, `<APP_HOST>` resolve to P3. The mock catch-all can **never** shadow P2/P3. [existing — verified at `app/planes.py`, `APP_HOSTS` in `config.py`]
- **AC-9** Endpoint tokens are generated from an ambiguity-stripped alphabet (no `0 O 1 l I`), default length 10, independent of the owner id; the subdomain label's **case is preserved** when resolving the token. [existing — verified at `app/database.py::gen_token`, `app/planes.py::subdomain_of`]
- **AC-10** When `MOCK_DOMAIN` is unset/misconfigured the binary serves **path-fallback-only mode** (no wildcard surface) and logs a warning at startup rather than crashing; `mock_url` in API responses surfaces the `/e/<token>` form. [existing — verified at `config.py::PATH_FALLBACK_ONLY`, `app/routes/api.py::_mock_url`]

### Area 3 — Rule-driven mock responses — `FEATURES.md §3`
- **AC-11** Rules are evaluated **first-by-priority-then-by-id**, deterministically; only the first enabled fully-matching rule is served. [existing — verified at `app/interceptor/matcher.py::select`, `app/routes/api.py::list_rules` ORDER BY]
- **AC-12** Match criteria cover: `method` (`ANY` or exact verb), `path` (exact, `:param` capture segments, trailing `/*` wildcard), required `headers` (case-insensitive name), required `query`, JSON `body_conditions` (`eq/neq/contains/exists` via jsonpath-lite), and `state_requirements` (`eq/neq/exists/absent`). [existing — verified at `app/interceptor/matcher.py`, `app/models.py::MatchCriteria`]
- **AC-13** A served rule response uses the rule's `status_code` (100–599), `headers` (with `Content-Type` defaulting from `content_type`), and templated `body_template`. [existing — verified at `app/interceptor/engine.py` rule branch, `app/models.py::ResponseSpec`]
- **AC-14** The rule builder exposes the five tabs Matching · Response · Templating · Actions · Throttling, mapping 1:1 to `MatchCriteria`, `ResponseSpec`, template tags, `state_writes`/`webhook_action`, and per-rule `latency_ms`/`rate_limit_per_min`/`chaos_mode`. All strings come from `copy.md` `rule.*`. [new — React; data shapes existing at `app/models.py::MockRuleCreate`]
- **AC-15** Per-rule `latency_ms` / `rate_limit_per_min` / `chaos_mode` overrides, when set (non-null), take precedence over the endpoint-level values for that served request; a null per-rule `chaos_mode` inherits the endpoint value. [existing semantics — verified at `app/interceptor/engine.py` (`rule_latency_override`/`rule_rate_override`); `chaos_mode` per-rule override is the OQ-2 addition]

### Area 4 — Dynamic response templating (sandboxed) — `FEATURES.md §4`
- **AC-16** A **hand-written single-pass scanner** resolves the closed tag set: `{{now 'iso'|'unix'|'epoch_ms'}}`, `{{random 'uuid'}}`, `{{random 'int' <lo> <hi>}}`, `{{random 'hex' <len>}}`, `{{request.method|path|body}}`, `{{request.query.<k>}}`, `{{request.path.<name>}}`, `{{request.header.<name>}}`, `{{request.body.<jsonpath>}}`, `{{state.<k>}}`. [existing — verified at `app/interceptor/templating.py`]
- **AC-17** There is **no `eval`/`exec`, no general template engine, and no string-format over user text.** SSTI probes (`{{ 7*7 }}`, `{{ config }}`, `{{ ''.__class__ }}`, `{{ self }}`) are unknown tags → returned **verbatim**, executing zero code. [existing — verified at `app/interceptor/templating.py::_resolve_tag`]
- **AC-18** Unknown/malformed tags are **left literal** and never error the mock path; a template over `TEMPLATE_MAX_SIZE` (256 KB) is returned unrendered; at most `TEMPLATE_MAX_TAGS` (500) substitutions are performed. [existing — verified at `app/interceptor/templating.py::render`, `config.py`]
- **AC-19** `state_writes` are rendered with the same template context and applied **before** the body renders, so `{{state.k}}` in the same response observes a just-written value. [existing — verified at `app/interceptor/engine.py::_apply_state_writes`]

### Area 5 — Stateful / multi-step transactions — `FEATURES.md §5`
- **AC-20** A rule can write per-endpoint state (`state_writes`); a later rule with a `state_requirement` matches only when that state holds (e.g. `POST /login` sets `authenticated=true`; `/dashboard` requires it). State is **per-endpoint, shared across all callers**. [existing — verified at `app/interceptor/matcher.py::_state_ok`, `engine.py`]
- **AC-21** State is persisted with a 24h TTL (`STATE_TTL_SECONDS`) and is read **lazily** — only when some enabled rule gates on state. [existing — verified at `app/interceptor/engine.py` (`ep.any_rule_gates_on_state`), `config.py`]
- **AC-22** `GET /api/endpoints/{token}/state` returns `{state: {...}}` and `DELETE …/state` clears it; both owner-gated. [existing — verified at `app/routes/api.py::get_state/clear_state`]
- **AC-23** A `state_write`/`state_requirement` key must pass the safe charset (`^[A-Za-z0-9_-]{1,64}$`); an unsafe key is skipped/ignored, never persisted. [existing — verified at `app/utils/helpers.py::is_safe_key` usage]

### Area 6 — Instant Auto-CRUD — `FEATURES.md §6`
- **AC-24** With `auto_crud` enabled and **no rule matching**, the endpoint serves a REST DB over a per-collection JSON array: `POST /<coll>`→`201` (server-assigned uuid `id`), `GET /<coll>`→`200` (array), `GET/PUT/PATCH/DELETE /<coll>/<id>`→`200`/`200`/`200`/`204` or `404`, `HEAD` mirrors `GET`. [existing — verified at `app/interceptor/crud.py::handle`]
- **AC-25** `<coll>` and `<id>` must match `^[A-Za-z0-9_-]{1,64}$`; a 3+-segment or unsafe path is **not CRUD** and falls through to tunnel/MITM/default. A write body must be a JSON **object** (else `400`); item-count cap `CRUD_MAX_ITEMS` (1000) and per-item byte cap `CRUD_MAX_ITEM_BYTES` (64 KB) yield `400` when exceeded. [existing — verified at `app/interceptor/crud.py`, `config.py`]
- **AC-26** Concurrent writes to one collection do not lose updates — the read-modify-write is **atomic** (today Redis WATCH/MULTI; in the re-platform, one SQLite `BEGIN IMMEDIATE` transaction). A server-assigned uuid `id` means two concurrent POSTs never collide and both persist with distinct ids. [existing semantics — verified at `app/interceptor/crud.py` (`crud_cas`); architecture.md transaction design]
- **AC-27** `GET /api/endpoints/{token}/collections/{name}` peeks (`{items: [...]}`), `DELETE …/collections/{name}` clears; both owner-gated; an unsafe `name` → `422`. [existing — verified at `app/routes/api.py::peek_collection/clear_collection`]

### Area 7 — Proxy / partial mocking (MITM) — `FEATURES.md §7`
- **AC-28** With a `target_url` set and no rule/CRUD/tunnel handling the request, the request is forwarded to the upstream and the real response (status, safe headers, capped body) is returned labeled `served_by="mitm"`. A matching local rule **always wins** over forwarding. [existing — verified at `app/interceptor/proxy.py::forward`, `engine.py::_resolve_unmatched`]
- **AC-29** **SSRF guard (evaluated on the resolved IP, not the hostname):** the target is resolved and **every** resolved address is rejected if loopback / private / link-local / multicast / reserved / unspecified / cloud-metadata `169.254.169.254`; a blocked target → `502`. The connection is **pinned to the validated IP** (Host header + TLS SNI preserved) so a DNS-rebinding swap between check and connect is impossible. (See AC-S1/S2.) [existing — verified at `app/interceptor/proxy.py::_resolve_and_check/_pin_target`]
- **AC-30** Redirects are **not followed by default** (`MITM_FOLLOW_REDIRECTS=false`); when enabled, each hop re-applies the SSRF check, bounded by `MITM_MAX_REDIRECTS`. Timeout → `504`; connection/DNS/SSRF error → `502 upstream_unreachable`; body capped at `MITM_MAX_BODY_BYTES` (5 MB) with `X-HookBox-Truncated: true` when truncated. [existing — verified at `app/interceptor/proxy.py`, `config.py`]
- **AC-31** The owner capability and hop-by-hop/sensitive request headers (`authorization`, `cookie`, `x-owner-id`, `x-user-id`, `x-hookbox-cap`, hop-by-hop) are **stripped before forwarding** (never sent upstream); upstream hop-by-hop / `Set-Cookie` / upstream-CORS / `content-length` / `content-encoding` / `transfer-encoding` headers are stripped from the captured response. [existing — verified at `app/interceptor/proxy.py::_STRIP_RESPONSE_HEADERS`, `strip_forward_headers`]
- **AC-32** `target_url` is validated at the API boundary to `http`/`https` with a host (else `422` on PATCH); empty/null clears it. [existing — verified at `app/models.py::_validate_target_url`]

### Area 8 — Auto-CORS engine — `FEATURES.md §8`
- **AC-33** On the **mock plane only (P1)**, when `cors_enabled`, an `OPTIONS` preflight to any mock path returns `204` with reflected `Access-Control-Allow-Origin`, reflected `Access-Control-Request-Headers` (else `*`), `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`, `Access-Control-Max-Age: 600`, `Vary: Origin` — with no user-defined rule. [existing — verified at `app/interceptor/cors.py::preflight_headers`]
- **AC-34** Every non-preflight P1 response carries `Access-Control-Allow-Origin` (reflected Origin, or `*` when absent), `Access-Control-Expose-Headers: *`, `Vary: Origin`. `Access-Control-Allow-Credentials` is **never** emitted (no credentialed wildcard). [existing — verified at `app/interceptor/cors.py::response_headers`]
- **AC-35** The **management API (P2) emits no wildcard CORS** — these headers are produced only on P1. [existing — verified at `app/interceptor/cors.py` docstring + `engine._identified` call site]
- **AC-36** When `cors_enabled=false`, the per-response CORS set is empty and `OPTIONS` still returns a deterministic `204` (a preflight never falls through to a rule/404). [existing — verified at `app/interceptor/cors.py::preflight_response`]

### Area 9 — Simulated network conditions (latency · rate-limit · chaos) — `FEATURES.md §9`
- **AC-37** Conditions are applied around the served response in the **frozen order rate-limit → chaos → latency**. [existing — verified at `app/interceptor/engine.py` step 5, `conditions.py`]
- **AC-38** **Latency** (`latency_ms`, clamped 0–10000) sleeps cooperatively (never blocks other endpoints); the applied ms is recorded so `overhead_ms = duration − applied_latency` is observable. [existing — verified at `app/interceptor/conditions.py::apply_latency`, `engine.py`]
- **AC-39** **Rate limit** (`rate_limit_per_min`, `0`=unlimited, clamped ≤ `RATE_LIMIT_MAX_PER_MIN`=100000) is a token bucket keyed per endpoint (or per rule on override), applied to **every** served path including MITM forwards and Auto-CRUD writes; over-limit → `429` with `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`; the limiter **fails open** on internal error. (See AC-S18.) [existing — verified at `app/interceptor/conditions.py::check_rate_limit`, `engine.py::_rate_limit`]
- **AC-40** **Chaos** (`chaos_pct` 0–100) injects on a hit a random `502/503/504` when `chaos_mode="error"` (default); `chaos_pct=0` never fires, `100` always fires. `chaos_mode="dropout"` closes the connection (no body), bounded by `CHAOS_DROP_TIMEOUT_S` so it cannot hang a worker. `chaos_mode` is a first-class field on endpoint + per-rule override (OQ-2, frozen in §5.3). [existing behavior — verified at `app/interceptor/conditions.py::roll_chaos`; `chaos_mode` promoted per OQ-2]

### Area 10 — Real-time split-screen dashboard — `FEATURES.md §10`
- **AC-41** Every served mock request is logged and published; subscribed clients receive a `new_request` event (the `RequestSummary` shape, in the §5.4 WS wire envelope) in near-real-time over WebSocket, with an SSE fallback delivering the identical stream. [existing — verified at `app/websocket.py`, `engine._persist_and_publish`]
- **AC-42** The feed is **owner-gated**: the capability is presented as `?cap=<owner_secret>` and verified **before any frame** — an anonymous/wrong/cross-owner subscribe gets **zero** events (WS close `4401` / SSE `401`). Channel isolation: a client authed for `tokenA` receives only `tokenA` events. (See AC-S9.) [existing — verified at `app/websocket.py::feed_ws/feed_sse`, `app/auth.py::verify_cap_owns_token`]
- **AC-43** Concurrent feed connections per endpoint are bounded by `WS_MAX_CONN_PER_ENDPOINT` (50); excess are refused (WS accept-then-close `1013` / SSE `503`). Per-client send is bounded by `WS_SEND_TIMEOUT_S`; dead/slow clients are dropped, never stalling fan-out; a lagged broadcast receiver drops frames (client reconciles via the management API) and is not errored. (See AC-S19.) [existing — verified at `app/websocket.py` (`at_capacity`, `broadcast`)]
- **AC-44** On connect the server sends a `hello {token, server_time}` frame; the deep inspector shows Headers · Query · Body · Response Served · State & Tracing for a selected trace via `GET /api/requests/{id}`. Strings come from `copy.md` `insp.*`. [existing — verified at `app/websocket.py`, `app/routes/api.py::get_request`, `app/models.py::RequestDetail`]
- **AC-45** The split-screen dashboard (live feed + deep inspector), rule builder, rules manager, and endpoint settings are rebuilt as a fresh React SPA over the §5 contract, applying `design.md` tokens/components and `copy.md` strings 1:1. [new — React `src/`]

### Area 11 — Data retention — `FEATURES.md §11`
- **AC-46** A hard **per-endpoint trace cap of `TRACE_CAP`=100** is enforced **at write time** (prune to newest 100 by id on every insert) so it never drifts between sweeps. [existing — verified at `app/database.py::TraceWriter._PRUNE_SQL/insert_trace`]
- **AC-47** A background sweep (interval `RETENTION_SWEEP_SECONDS`=300) enforces the 100-cap **and** a **24h TTL** (`TRACE_TTL_HOURS`=24) on traces; expired `endpoint_state` / `crud_collections` rows (24h TTL) are reaped, and tombstoned endpoints older than `GONE_TTL_HOURS` (168h/7d) are hard-deleted. [existing semantics — verified at `config.py`; sweep re-homed to a `tokio::time::interval` task — architecture.md]
- **AC-48** `GET /api/endpoints/{token}/requests?limit&offset` lists traces (newest first, `limit` 1–200, default 50, `offset`≥0 default 0; out-of-range → `422`) and `DELETE …/requests` clears history; both owner-gated. [existing — verified at `app/routes/api.py::list_requests/clear_requests`]

### Area 12 — Local tunnel CLI — `FEATURES.md §12`
- **AC-49** A second Rust binary (`tunnel`) reverse-tunnels public traffic for an endpoint to the operator's localhost: it opens one authenticated WebSocket to `/ws/tunnel/{slug}` presenting the owner capability as `Authorization: Bearer <secret>` (or `?cap=`); an unauthenticated/wrong-owner bind is refused with WS close `4401` and never registered. UX parity: `--port --endpoint --secret`. (See AC-S10.) [existing — verified at `app/routes/tunnel.py`; CLI was `python -m tunnel` → now `tunnel` bin]
- **AC-50** In the resolution order, **tunnel is consulted after Auto-CRUD and before MITM**; tunneled traffic is forwarded down the bound socket and the CLI's response replayed to the public caller, labeled `served_by="tunnel"` in the feed. A second correctly-authenticated owner bind **takes over** (last-bind-wins): the prior socket gets `{"t":"err","message":"rebound elsewhere"}` then close `4409`. [existing — verified at `app/routes/tunnel.py::TunnelRegistry.bind`, `engine._resolve_unmatched`]
- **AC-51** No tunnel connected / drop / per-request timeout (`TUNNEL_REQUEST_TIMEOUT_S`=30) resolves to a deterministic `504 {error:"no_tunnel"}` (never a hang); tunneled traffic is subject to the same ingest/rate caps as every other P1 path; control frames use the JSON `{t:"req|res|err|ping|pong", id, ...}` shape with base64 bodies (`bound` greeting on connect); backoff reconnect on the CLI side. [existing — verified at `app/routes/tunnel.py::forward_to_tunnel`, framing in module docstring]
- **AC-52** **Tunnel CLI stdout contract (operator UX).** The `tunnel` binary prints the `copy.md` `cli.tty.*` lines for each lifecycle moment: connecting, `bound` (`Tunnel up. Forwarding {token} → http://localhost:{port}`), per-request line, **`4401` → print the auth-rejected message and STOP (no retry loop)**, **`4409` → print "another tunnel took over" and exit cleanly**, disconnect → "Reconnecting in {seconds}s…" with backoff, reconnected, and shutdown. The dashboard reflects bind/unbind via the `endpoint_updated` event flipping `tunnel_active` (shown via `copy.md` `dash.tunnel.active`). [new — CLI; strings at `copy.md` §5.11–§5.12; behavior parity with `app/routes/tunnel.py`]

### Area 13 — Deployment — `FEATURES.md §13`
- **AC-53** `scripts/start.sh` runs `pnpm build` (Vite → `dist/`) → `cargo build --release` → applies SQLite migrations on startup → seeds demo data on first run → serves on `http://localhost:8080` from the **single binary** (SPA + API + mock plane + WS/SSE + in-process sweep/buckets). Default serve port is **8080** (OQ-4). [new — mirrors `../shortener-link/scripts/start.sh`; spec §11]
- **AC-54** **Docker is optional**: if kept, a **single app container** (no Redis, no Postgres) with a persistent volume for `data/app.db`; the old two-service `docker-compose.yml` (app + Redis with healthcheck wait) is re-authored to the single-container shape. [new — supersedes verified `Dockerfile`/`docker-compose.yml`; spec §3]
- **AC-55** `cargo test` (backend integration, mirroring `../shortener-link/backend/tests/`) and Playwright e2e (frontend) gate the build; the QA lane validates every AC and the full §5 contract on both FE and BE sides. [new — spec §11]

### Cross-cutting parity invariants
- **AC-56** Every mock response carries `X-HookBox-Endpoint: <token>` and `X-HookBox-Served-By: <rule|crud|mitm|tunnel|default|cors|chaos|ratelimit>`, plus `X-HookBox-Rule-Id: <id>` when a rule matched, and `X-HookBox-Truncated: true` on MITM body truncation. [existing — verified at `app/interceptor/engine.py::_identified`, `proxy.py`]
- **AC-57** An unknown endpoint token on P1 returns `404 {error:"unknown_endpoint"}`; a deleted/tombstoned token returns `410 {error:"endpoint_gone"}`; after `GONE_TTL_HOURS` (7d) the tombstone is reaped and the token degrades to `404`; neither is logged as a trace. (OQ-1: `gone_at` column backs the distinction; see §5.6.) [existing semantics — verified at `app/interceptor/engine.py::_unknown_or_gone`, `app/routes/api.py::delete_endpoint`]
- **AC-58** P1 request bodies over `MAX_INGEST_BODY_BYTES` (1 MB) are rejected with `413` before buffering; captured request/response bodies in traces are truncated at `MAX_BODY_BYTES` (256 KB). (See AC-S15.) [existing — verified at `app/interceptor/engine.py::_read_body_capped`, `_spawn_trace`]
- **AC-59** The trace write + feed publish run **off the response path** (fire-and-forget via a spawned task); a slow/failed trace never delays or fails the served mock response. [existing — verified at `app/interceptor/engine.py::_spawn_trace` (`create_task`, never awaited)]
- **AC-60** Owner-capability auth: every `/api/**` route except `POST /api/session` requires `Authorization: Bearer <owner_secret>`; missing/malformed/unknown → `401` with `WWW-Authenticate: Bearer`; valid-but-non-owner of `{token}` → **`404`** (never `403`). Error bodies are uniformly the flat `{error, detail}`. [existing — verified at `app/auth.py`, `app/routes/api.py`]
- **AC-61** The owner capability and `Cookie`/`Authorization`/`X-Owner-Id` headers are **redacted** before a trace is persisted (never stored) and before any feed payload; the inspector surfaces redacted values as a "redacted" pill (`copy.md` `insp.headers.redacted`). (See AC-S13.) [existing — verified at `app/interceptor/engine.py::_redact`]

### Security ACs (from `security.md`; each testable, validated by the security gate)
- **AC-S1** A MITM `target_url` whose host resolves to any loopback/private/link-local/multicast/reserved/unspecified address or `169.254.169.254` (IPv4 and IPv6, incl. IPv4-mapped `::ffff:` forms) is refused with `502`; the guard runs on **every** resolved address, not the hostname. [reinforces AC-29]
- **AC-S2** The MITM forward connects to the **validated IP literal** (no second resolution), preserving `Host` + TLS SNI; a host that resolves public-then-private between check and connect cannot reach the private address. [reinforces AC-29]
- **AC-S3** MITM redirects are not followed by default; when enabled, each hop re-runs AC-S1/S2, bounded by `MITM_MAX_REDIRECTS`. [reinforces AC-30]
- **AC-S4** `authorization`, `cookie`, `x-owner-id`, `x-user-id`, `x-hookbox-cap` and hop-by-hop headers are stripped before any MITM forward; the owner capability is never sent upstream. Upstream `set-cookie` / CORS / `transfer-encoding` / `content-length` / `content-encoding` are stripped from the captured response. [reinforces AC-31]
- **AC-S5** The SSTI probe set (`{{7*7}}`, `{{config}}`, `{{''.__class__}}`, `{{self}}`, handlebars/tera-style helpers) is returned **verbatim**, executing zero code; templating uses a hand-written scanner with no general template-engine crate over user text. [reinforces AC-17]
- **AC-S6** After a second `POST /api/session` for the same email, the previously returned secret authenticates with `401` (overwrite, not append). [reinforces AC-2]
- **AC-S7** Presenting `owner_id` (or any non-secret) as a bearer token returns `401`. [reinforces AC-4]
- **AC-S8** Missing/malformed/unknown cap → `401` + `WWW-Authenticate: Bearer`; a valid cap addressing a `{token}` it does not own → `404` (never `403`); bodies are uniform `{error, detail}`. [reinforces AC-60]
- **AC-S9** An anonymous / wrong / cross-owner WS or SSE subscribe receives **zero** events (WS close `4401` / SSE `401`), verified before registration/first frame; a cap valid for `tokenA` receives only `tokenA` events. [reinforces AC-42]
- **AC-S10** An unauthenticated / wrong-owner bind to `/ws/tunnel/{slug}` is refused (`4401`) and never registered; only the slug's owner can bind. [reinforces AC-49]
- **AC-S11** No `Access-Control-Allow-*` headers are emitted on any `/api/**` response (P2 has no CORS layer). [reinforces AC-35]
- **AC-S12** No P1 response ever emits `Access-Control-Allow-Credentials`; Origin is reflected (or `*` only when absent). [reinforces AC-34]
- **AC-S13** The capability and `Authorization`/`Cookie`/`X-Owner-Id` headers are redacted in stored traces and feed payloads; the cap never appears in `tracing` logs (incl. the `?cap=` query string of `/ws|/sse`) nor in any error body. [reinforces AC-61]
- **AC-S14** The API never reads the capability from a cookie; the SPA does not store it in a cookie (holds it in memory/`localStorage`), closing CSRF without tokens.
- **AC-S15** A P1 request whose `Content-Length` or streamed size exceeds `MAX_INGEST_BODY_BYTES` is rejected `413` without fully buffering. [reinforces AC-58]
- **AC-S16** No crafted `Host`/path (multi-label subdomain, percent-encoded path, a token equal to the app host, `/api` under a mock host) causes the P1 catch-all to serve `/api/**` or the UI/feed; `/api/**` is unreachable from any mock host. [reinforces AC-6/7/8]
- **AC-S17** No SQL is built by string interpolation (parameterized `sqlx` binds only); the `^[A-Za-z0-9_-]{1,64}$` charset is enforced for state keys (skip/ignore), CRUD collection names (`422`), and CRUD ids. [reinforces AC-23/25/27]
- **AC-S18** The in-memory token-bucket map is bounded (entry cap / idle eviction) so it cannot grow without limit; on an internal limiter error the request is **allowed** (fails open) while the body/size caps still bound it. [reinforces AC-39; OQ-3 fail-open property]
- **AC-S19** The per-endpoint WS connection cap, a bounded broadcast channel with lag-drop, per-send timeout, and slow-client drop are all enforced so a flood of connections/endpoints cannot exhaust process memory. [reinforces AC-43]

> **OQ-3 security-load-bearing fail-directions (frozen, security-signed-off):** a `state_requirement` that cannot be evaluated (unreadable state) **fails closed** — the gated rule does not match (preserves the Python fail-closed intent without a Redis trigger). A genuine SQLite fault on the state/CRUD path surfaces as a stable-enveloped `5xx` (CRUD store error → `503 {error:"store_unavailable"}`), not a routine degradation mode. The two rate limiters (per-endpoint and `POST /api/session`) **fail open** but are bounded (AC-S18). This is reflected in §5.5/§5.6.

### Visual ACs (from `design.md`; testable, validated by QA against `design.md` §2/§9)
- **AC-D11** No component file under `src/components/**` contains a raw hex color (grep `#[0-9a-fA-F]{3,6}` → 0 matches); all color resolves through semantic Tailwind tokens (`design.md` §2). [fixes the `templates/base.html` inline-hex anti-pattern]
- **AC-D12** Toggling `.dark` on `<html>` re-themes the entire SPA via tokens with no component-level color change; both themes pass automated contrast checks for every token pair in `design.md` §9.
- **AC-D13** A selected `FeedRow` is distinguishable from unselected **with color removed** (3px leading accent rail + `bg-active` fill present in grayscale) and carries `aria-selected`.
- **AC-D14** `MethodBadge`, `StatusCode`, `ServedByChip`, and `ConnectionPill` are each identifiable in a grayscale screenshot (text label and/or icon present); status class is conveyed by digits + underline/dot, not hue alone.
- **AC-D15** Every interactive element shows a visible 2px focus-visible ring (≥3:1 against adjacent fill) on keyboard focus, including feed rows, JsonTree disclosure triangles, slider thumbs, and copy buttons.
- **AC-D16** Under `prefers-reduced-motion: reduce`, feed-row arrival is an instant insert (no slide/flash), skeletons are static blocks, and no auto-pulse/spin is the sole state cue.
- **AC-D17** `ConnectionPill` state (connecting/live/reconnecting/sse/offline/unauthorized/busy) is conveyed by its **text label** independent of icon animation and dot color (strings at `copy.md` `feed.conn.*`).
- **AC-D18** The live feed maintains vertical column alignment (tabular numerics for status/latency) as rows prepend; columns do not reflow on new-row arrival.
- **AC-D19** Mock-URL chips render in `text-primary` mono (not `--accent`/link color) and expose only a copy affordance — no navigation.
- **AC-D20** `--text-on-accent` on the primary button's `--accent-fill` measures ≥ 4.5:1 in both themes (apply the `design.md` §11.4 `--teal-600` fallback in light if the rendered ratio dips below 4.5:1).
- **AC-D21** Every status/method/served-by chip's `fg` on its `bg` measures ≥ 4.5:1 in both themes.
- **AC-D22** The landing email gate is visually and copy-identical for a brand-new vs. existing email (no "welcome back" divergence) through submit/success (no returning-user string exists — `copy.md` §4.2 note). [reinforces AC-1]
- **AC-D23** The `body_template` byte counter turns `--warning-fg` as it approaches 256 KB and `--danger-fg` when exceeded (text + color, not color alone; `copy.md` `rule.resp.body.counter*`).
- **AC-D24** The landing page first-paints in the **light** theme for a new visitor with no stored preference and no `prefers-color-scheme: dark`; an explicit toggle persists and overrides. (Resolves `design.md` gap #1 / `ux.md` gap #10: light is the first-paint default; both themes AA.)

### Journey-state ACs (from `journey.md` PRD gaps; per-screen state matrix — all strings from `copy.md`)
- **AC-J1 (Landing states).** The email gate renders: idle; submitting (`landing.email.submitting`, input disabled, button loading); field-error on `422` (`landing.error.email.invalid`); banner on `429` with Retry-After (`landing.error.rateLimit {seconds}`); network-error banner (`landing.error.network`); no-endpoint banner (`landing.error.noEndpoint`); storage-unavailable warn (`landing.warn.storage`, still allows submit, no redirect loop); and auto-resume (stored owner+token → redirect to `/d/:token` with no form shown). [journey.md §"Primary"/§"Error", §"Required states"]
- **AC-J2 (Dashboard shell states).** `/d/:token` renders: loading (`dash.state.loading.aria`); not-signed-in → bounce to `/`; `404` not-found card (`dash.state.notFound.*` + `dash.state.backToStart`); `410` gone card (`dash.state.gone.*`, distinct from 404 per OQ-1); offline (`dash.state.offline.*`); loaded. [journey.md gap #1]
- **AC-J3 (Feed states).** The live feed renders: loading skeleton (6–8 rows shaped like the grid); empty (`feed.empty.*` + copy-only mock-URL CodeBlock + static `curl {mock_url}/ping` sample, never executed); streaming (newest-first, capped at 100, `feed.count`); paused with a buffered "N new" pill (`feed.newCount {n}`) that flushes on resume preserving read position; first `new_request` transitions empty→streaming with no reload. [journey.md gap #1, edge cases]
- **AC-J4 (Inspector states).** The inspector renders: empty/no-selection (`insp.empty.*`); loading (`insp.loading.aria`); **pending** — a freshly-streamed `new_request` whose `GET /api/requests/{id}` 404s because the fire-and-forget trace (AC-59) is not yet persisted shows `insp.pending.*` + Retry (NOT a hard 404); unauthorized on `401` (`insp.unauthorized.*`); error + Retry (`insp.error.*`); ready (5 tabs, each with its own empty sub-state: `insp.headers.empty`, `insp.query.empty`, `insp.body.empty`, `insp.response.empty`, `insp.trace.empty/stateEmpty`). [journey.md gap #2]
- **AC-J5 (WS resilience).** The feed client implements: exponential backoff with jitter (250→8000ms) on drop; pill states connecting/live/reconnecting(n)/degraded/unauthorized/offline; heartbeat ping + pong-grace to detect a half-open socket and force reconnect (SSE emits `: ping` ~25s); reconnection paused while the tab is hidden, resumed with a back-fill on focus; after `MAX_WS_FAILS_BEFORE_SSE` (6) failures → **SSE fallback** to `/sse/{token}?cap=` (same owner gate), pill `Live (SSE)` (`feed.conn.sse`). On reconnect the client reconciles authoritative state via the §5.2 GET routes (per the at-most-once broadcast contract). [journey.md gap #3]
- **AC-J6 (Connection-cap UX).** A WS `1013` / SSE `503` refusal maps to a **distinct** "Feed busy" pill state (`feed.conn.busy` / `.tooltip` with the `{max}` slot), NOT an endless "Reconnecting" — the client does not hammer the gate. [journey.md gap #4]
- **AC-J7 (Secret rotation mid-session).** (a) On re-submitting the email, the active tab overwrites its stored secret and survives; (b) a stale tab holding the old secret bounces to `/` on the next `/api/*` `401` (`common.error.401`); (c) the feed on a stale tab shows the unauthorized pill (`feed.conn.unauthorized`), does **not** hammer the gate, and only retries if the stored secret changes. [journey.md gap #5; reinforces AC-2/AC-60]
- **AC-J8 (Endpoint delete journey).** The Settings Danger zone exposes endpoint deletion behind a typed-token confirm (`set.confirm.delete.*`); a successful `DELETE /api/endpoints/{token}` (`200 Message`) tombstones the endpoint (gone → `410` on P1, AC-57) and routes the SPA back to a primary/landing; subsequent traffic to the dead token shows the `410` "deleted" copy (`dash.state.gone.*`). [journey.md gap #7; resolved by OQ-1]
- **AC-J9 (Auto-CRUD / state visibility).** Settings exposes a lightweight collection peek (`set.crud.peek.*` via `GET …/collections/{name}`) and clear, plus state clear and history clear (`set.retention.*`) — API-backed, no full data-browser screen (resolved §9 J9). [journey.md gap #9]
- **AC-J10 (echo + webhook honesty).** `default_mode="echo"` returns the reflected request as JSON and surfaces in the feed/inspector with the normal `default` served-by chip (no special "echoed" label); the rule builder Actions tab renders the `webhook_action` section as a **visible-but-disabled** "Stored, not yet sent" control (`rule.act.webhook.badge`/`.helper`), not omitted, so the data shape round-trips. [journey.md gaps #8/#10; copy.md §6.2/§6.5]
- **AC-J11 (Concurrency observable contracts).** QA can observe: (a) many feed subscribers with a slow/dead client dropped (per-client `WS_SEND_TIMEOUT_S`) without stalling fan-out; (b) per-endpoint state shared across concurrent callers with a `state_write` applied before the body so `{{state.k}}` sees the new value in the same response; (c) the last-bind-wins tunnel takeover (second authed bind wins; prior gets `4409`). [journey.md gap #13; reinforces AC-19/26/43/50]
- **AC-J12 (First-run / seeded demo).** AC-53 seeds demo data on first run; the SPA's first-time operator lands on the seeded `primary` endpoint pre-populated with at least one rule and sample traces, so genuine empty states are reachable only after the operator clears history/rules or creates a fresh endpoint. [journey.md gap #11; resolved §9 J12]
- **AC-J13 (Global offline).** When the browser goes fully offline, the feed pill shows `offline` (`feed.conn.offline`) and management calls surface a non-alarming offline banner (`common.error.network` / `dash.state.offline.*`); on reconnect the client back-fills and resumes without a reload. [journey.md gap #12]

---

## 5. Frozen interface contract — FROZEN (lifted verbatim from `architecture.md` §5)

> **FROZEN.** FE (`src/`) and BE (`backend/`) implement against exactly these shapes, status codes, headers, and orders. Neither lane may change a shape without a §5 amendment in `architecture.md`. All `/api/**` bodies are JSON; every error body is the **flat** envelope `{"error": "<code>", "detail": "<human>"}`. All timestamps are RFC3339 UTC strings (e.g. `2026-06-21T12:00:00Z` / `...+00:00`). [existing — verified at `app/routes/api.py`, `app/models.py`]

### 5.1 Auth (P2 management plane)
- Every `/api/**` route **except** `POST /api/session` requires `Authorization: Bearer <owner_secret>`.
- Missing / malformed / unknown secret → `401 {"error":"unauthorized","detail":...}` with header `WWW-Authenticate: Bearer`. [existing — `app/auth.py::require_owner`]
- Valid secret that does **not** own the addressed `{token}`/resource → `404 {"error":"not_found","detail":...}` (never `403` — a non-owner cannot distinguish "exists but not mine" from "does not exist"). [existing — `app/auth.py::assert_owns_endpoint`]
- The public `owner_id` is **never** a credential — only `sha256(secret)` is looked up. [existing — `app/auth.py`]
- Feed (WS/SSE) subscribe auth: `?cap=<owner_secret>` query param. Tunnel bind auth: `Authorization: Bearer <owner_secret>` header (with `?cap=` fallback). Both verified **before any frame**. [existing — `app/websocket.py`, `app/routes/tunnel.py`]
- **Security notes (frozen):** the `capability_hash` is **rotate-overwrite** (a new `POST /api/session` overwrites, never appends — old secret 401s); the cap is a CSPRNG 256-bit value (`OWNER_SECRET_BYTES`=32), compared by hash (no early-return string compare on the raw secret); the cap is accepted **only** from `Authorization: Bearer` (P2) and `?cap=` (feed/tunnel), **never** from a cookie.

### 5.2 Management API endpoints (`/api/**`)

| # | Method | Path | Request body | Success | Errors |
|---|--------|------|--------------|---------|--------|
| 1 | POST | `/api/session` | `SessionCreate` | `200 SessionResponse` | `429` over rate-limit (`Retry-After`); `422` invalid email. No auth. |
| 2 | GET | `/api/endpoints` | — | `200 EndpointSummary[]` | `401` |
| 3 | POST | `/api/endpoints` | `EndpointCreate` | `201 EndpointDetail` | `401`, `422` |
| 4 | GET | `/api/endpoints/{token}` | — | `200 EndpointDetail` | `401`, `404` |
| 5 | PATCH | `/api/endpoints/{token}` | `EndpointConfigPatch` | `200 EndpointDetail` | `401`, `404`, `422` (bad `target_url`/clamp) |
| 6 | DELETE | `/api/endpoints/{token}` | — | `200 Message` | `401`, `404` (tombstones → `410` on P1) |
| 7 | GET | `/api/endpoints/{token}/rules` | — | `200 MockRule[]` | `401`, `404` — ORDER BY priority, id |
| 8 | POST | `/api/endpoints/{token}/rules` | `MockRuleCreate` | `201 MockRule` | `401`, `404`, `422` |
| 9 | GET | `/api/endpoints/{token}/rules/{id}` | — | `200 MockRule` | `401`, `404` (endpoint or rule) |
| 10 | PATCH | `/api/endpoints/{token}/rules/{id}` | `MockRulePatch` | `200 MockRule` | `401`, `404`, `422` |
| 11 | DELETE | `/api/endpoints/{token}/rules/{id}` | — | `204` (no body) | `401`, `404` |
| 12 | GET | `/api/endpoints/{token}/requests?limit&offset` | — | `200 RequestSummary[]` | `401`, `404`; `limit` 1–200 (default 50), `offset` ≥ 0 (default 0); out-of-range → `422` |
| 13 | GET | `/api/requests/{id}` | — | `200 RequestDetail` | `401`, `404` (owner resolved via the trace's endpoint) |
| 14 | DELETE | `/api/endpoints/{token}/requests` | — | `200 Message` | `401`, `404` |
| 15 | GET | `/api/endpoints/{token}/state` | — | `200 {"state": {<k>:<v>}}` | `401`, `404` |
| 16 | DELETE | `/api/endpoints/{token}/state` | — | `200 Message` | `401`, `404` |
| 17 | GET | `/api/endpoints/{token}/collections/{name}` | — | `200 {"items": [ <obj> ]}` | `401`, `404`, `422` (unsafe `name`) |
| 18 | DELETE | `/api/endpoints/{token}/collections/{name}` | — | `200 Message` | `401`, `404`, `422` |
| 19 | GET | `/healthz` | — | `200` (P3 plane, no auth) | — |

[existing — verified verbatim at `app/routes/api.py` (the 18 `/api` routes) + the `/healthz` liveness probe]

**Notes that are part of the contract:**
- `204` responses (#11) carry **no body**.
- `name` charset for #17/#18 collections: `^[A-Za-z0-9_-]{1,64}$`; otherwise `422 {"error":"invalid_collection",...}`. [existing — `app/routes/api.py::peek_collection`, `app/utils/helpers.py::is_safe_key`]
- `POST /api/session` is rate-limited per source IP (`SESSION_RATE_LIMIT_PER_MIN`, default 30/min); over-limit → `429 {"error":"rate_limited",...}` + `Retry-After`. [existing — `app/routes/api.py::create_session`]
- New vs existing email return **identical shape/status** (anti-enumeration). [existing]
- **No `Access-Control-Allow-*` headers on any `/api/**` response** (P2 carries no CORS layer; AC-S11).

### 5.3 Shared data models (Rust `serde` structs ⇄ TS `zod` types, serialize to these exact JSON shapes)

Field names, types, defaults, and validation clamps are FROZEN. Optional fields serialize as `null` when absent (Python `Optional[...]` → Rust `Option<T>` with `serde(default)`; the JSON key is present with `null`, matching pydantic's default serialization). [existing — verified verbatim at `app/models.py`]

```
SessionCreate        { email: string }                                   // valid email; else 422
EndpointSummary      { token: string, name: string|null, mock_url: string,
                       path_url: string, created_at: datetime,
                       last_hit: datetime|null, request_count: int }
SessionResponse      { owner_id: string, owner_secret: string,           // owner_secret returned ONCE
                       endpoints: EndpointSummary[], primary: EndpointSummary }

EndpointCreate       { name: string|null }                               // name ≤ 100 chars
EndpointConfigPatch  { name?: string|null,                               // all optional → partial update
                       auto_crud?: bool,
                       target_url?: string|null,                         // http|https w/ host, else 422; ""/null clears
                       default_mode?: "mock_404"|"echo",
                       latency_ms?: int,                                 // clamp 0..10000
                       rate_limit_per_min?: int,                         // clamp 0..100000; 0 = unlimited
                       chaos_pct?: int,                                  // clamp 0..100
                       chaos_mode?: "error"|"dropout",                   // [new] OQ-2
                       cors_enabled?: bool }
EndpointDetail       { token, name|null, mock_url, path_url,
                       auto_crud: bool, target_url: string|null,
                       default_mode: "mock_404"|"echo",
                       latency_ms: int, rate_limit_per_min: int,
                       chaos_pct: int, chaos_mode: "error"|"dropout",     // [new] OQ-2
                       cors_enabled: bool, tunnel_active: bool,
                       created_at: datetime, last_hit: datetime|null, request_count: int }

BodyCondition        { path: string, op: "eq"|"neq"|"contains"|"exists", value: string|null }   // path = jsonpath-lite
StateRequirement     { key: string, op: "eq"|"neq"|"exists"|"absent",    value: string|null }
MatchCriteria        { method: string = "ANY",                           // "ANY" | exact verb (case-insens.)
                       path: string = "/*",                              // exact | ":param" | trailing "/*"
                       headers: { <name>: <value> } = {},                // required headers (name case-insens.)
                       query:   { <key>:  <value> } = {},                // required query
                       body_conditions: BodyCondition[] = [],
                       state_requirements: StateRequirement[] = [] }
StateWrite           { key: string, value: string }                      // value may contain template tags
ResponseSpec         { status_code: int = 200,                           // 100..599
                       headers: { <name>: <value> } = {},
                       body_template: string = "",                       // ≤ 256000 chars
                       content_type: string = "application/json" }
WebhookAction        { url: string, body_template: string = "" }         // stored, no-op (parity)
MockRuleCreate       { name?: string|null,                               // ≤ 120
                       priority: int = 100,                              // 0..100000; lower = first
                       enabled: bool = true,
                       match: MatchCriteria = {default},
                       response: ResponseSpec = {default},
                       state_writes: StateWrite[] = [],
                       latency_ms?: int|null,                            // 0..10000 override
                       rate_limit_per_min?: int|null,                    // 0..100000 override
                       chaos_mode?: "error"|"dropout"|null,              // [new] OQ-2 per-rule override
                       webhook_action?: WebhookAction|null }
MockRulePatch        = MockRuleCreate, all fields optional
MockRule             = MockRuleCreate + { id: int, token: string, created_at: datetime }

RequestSummary       { id: int, token: string, method: string, path: string,
                       status_code: int,
                       served_by: "rule"|"crud"|"mitm"|"tunnel"|"default"|"cors"|"chaos"|"ratelimit",
                       matched_rule_id: int|null,
                       duration_ms: int, overhead_ms: int, timestamp: datetime }
TraceEvent           { step: string, detail: string }
RequestDetail        = RequestSummary + { request_headers: {<k>:<v>}, query_params: {<k>:<v>},
                       request_body: string|null, response_headers: {<k>:<v>},
                       response_body: string|null, trace: TraceEvent[], state_snapshot: {<k>:<v>} }

Message              { message: string, success: bool = true }
```

[existing — verified verbatim at `app/models.py`; `chaos_mode` is the only **[new]** addition, OQ-2]

### 5.4 WebSocket + SSE live feed (P3)
- **WS:** `GET /ws/{token}?cap=<owner_secret>` (Upgrade). **SSE:** `GET /sse/{token}?cap=<owner_secret>` (`text/event-stream`).
- **Auth before any frame:** verify cap owns token. On failure → WS **accept-then-close `4401`** (so the close code reaches the client, not a 1006); SSE → `401 {"error":"unauthorized",...}`. [existing — `app/websocket.py` (the accept-then-close rationale is load-bearing)]
- **Connection cap** per endpoint = `WS_MAX_CONN_PER_ENDPOINT` (50). Over cap → WS accept-then-close `1013`; SSE → `503 {"error":"too_many_connections",...}`. [existing]
- **Channel isolation:** a client authed for `tokenA` receives only `tokenA` events. [existing]
- **Server → client events** (JSON; WS as a JSON text frame, SSE as `event: <type>\ndata: <json>\n\n`):
  - `hello` `{ "token": string, "server_time": datetime }` — first frame on connect.
  - `new_request` `{ ...RequestSummary... }` — one per served mock request.
  - `state_changed` `{ "token": string, "key": string, "value": string }`.
  - `endpoint_updated` `{ "token": string, "fields": string[] }`.
  - **Wire envelope:** WS frames are `{"type": "<event>", "data": {<payload>}}`. SSE frames put `<event>` in the `event:` line and `<payload>` (the inner `data` object) in the `data:` line. [existing — `app/websocket.py::broadcast` / `_sse_format`, `app/interceptor/engine.py::_persist_and_publish`]
- **Client → server:** WS text `"ping"` → server replies text `"pong"`. SSE: server emits a `: ping` comment heartbeat every ~25s (`_SSE_HEARTBEAT_S`). [existing]
- A dead/slow per-client send is bounded by `WS_SEND_TIMEOUT_S` (5s) and the client is dropped, never stalling fan-out. [existing]
- **Best-effort liveness contract (frozen):** the broadcast channel is at-most-once; on a lagged receiver the subscriber drops missed frames and **reconciles** authoritative state via the §5.2 GET routes. The management API is the source of truth.
- **Logging note (frozen):** the access-log / `tracing` layer must never emit the `?cap=` query string of `/ws|/sse` (AC-S13).

### 5.5 Mock plane (P1) behavior — FROZEN resolution & wrapping
- **Resolution order** (first that produces a response wins): `OPTIONS preflight → matching rule (priority,id) → Auto-CRUD → tunnel → MITM → default`. [existing — `app/interceptor/engine.py::handle_mock` / `_resolve_unmatched`]
- **Conditions wrap** (applied around the resolved body, in this exact order): `rate-limit (429) → chaos (5xx / connection-drop) → latency (sleep)`. Rate-limit and chaos short-circuit (return before latency); applied latency is recorded so `overhead_ms = duration − applied_latency`. [existing]
- **Default mode:** `mock_404` → `404 {"error":"no_match","detail":...}`; `echo` → `200 {"method","path","query","headers","body"}` (headers are the lower-cased request headers). [existing — `app/interceptor/engine.py::_echo_response`]
- **Identifying headers** on every P1 response: `X-HookBox-Endpoint: <token>`, `X-HookBox-Served-By: <rule|crud|mitm|tunnel|default|cors|chaos|ratelimit>`, `X-HookBox-Rule-Id: <id>` (only when a rule matched), `X-HookBox-Truncated: true` (MITM body truncation only). [existing — `app/interceptor/engine.py::_identified`, `app/interceptor/proxy.py`]
- **Auto-CORS** (P1 only, when `cors_enabled`): every non-preflight response also carries `Access-Control-Allow-Origin` (reflected `Origin`, else `*`), `Access-Control-Expose-Headers: *`, `Vary: Origin`. `OPTIONS` preflight → `204` with reflected Origin, reflected `Access-Control-Request-Headers` (else `*`), `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`, `Access-Control-Max-Age: 600`, `Vary: Origin`. `Access-Control-Allow-Credentials` is **never** emitted. When `cors_enabled=false` the per-response CORS set is empty but `OPTIONS` is still answered with a deterministic `204` (never falls through). The management plane (P2) emits **no** wildcard CORS. [existing — `app/interceptor/cors.py`]
- **Auto-CRUD lifecycle** (when `auto_crud` and no rule matched and path is a valid 1- or 2-segment CRUD path with safe segments): `POST /<coll>`→`201` (server uuid `id`), `GET /<coll>`→`200` (array), `GET /<coll>/<id>`→`200`|`404`, `PUT /<coll>/<id>`→`200`|`404`, `PATCH /<coll>/<id>`→`200`|`404`, `DELETE /<coll>/<id>`→`204`|`404`, `HEAD` mirrors `GET`. Write body must be a JSON **object** (else `400 {"error":"bad_request",...}`); `id` is server-assigned/immutable; caps `CRUD_MAX_ITEMS` (1000) and `CRUD_MAX_ITEM_BYTES` (64 KB) → `400`. A 3+-segment or unsafe-segment path is **not CRUD** → falls through to tunnel/MITM/default. A genuine SQLite fault → `503 {"error":"store_unavailable",...}` (OQ-3). [existing — `app/interceptor/crud.py`]
- **MITM** (target_url set, nothing else handled): forward, return upstream status/safe-headers/capped-body labeled `mitm`; SSRF-blocked / connection / DNS error → `502 {"error":"upstream_unreachable",...}`; timeout → `504 {"error":"upstream_timeout",...}`; body capped at `MITM_MAX_BODY_BYTES` (5 MB) with `X-HookBox-Truncated: true`. SSRF guard on **every** resolved IP + connection pinned to the validated IP (no second resolution; preserve Host + TLS SNI); redirects off by default and re-validate per hop; forward/response header strip lists per AC-S4; `MITM_ALLOW_PRIVATE` defaults **false**. [existing — `app/interceptor/proxy.py`; security §5 notes]
- **Tunnel** (CLI bound): forward down the socket, replay response labeled `tunnel`; no tunnel / drop / timeout → `504 {"error":"no_tunnel",...}`. [existing — `app/routes/tunnel.py::forward_to_tunnel`]
- **Caps:** ingest body > `MAX_INGEST_BODY_BYTES` (1 MB) → `413 {"error":"payload_too_large",...}` before buffering; trace request/response bodies truncated at `MAX_BODY_BYTES` (256 KB); template `TEMPLATE_MAX_SIZE` (256 KB) / `TEMPLATE_MAX_TAGS` (500). All caps env-driven with safe defaults. The in-memory rate-limit bucket map is bounded (entry cap / idle eviction) and the broadcast channel is bounded with lag-drop (AC-S18/S19 — the no-Redis additions). [existing + security additions]
- **Unknown / gone token:** unknown → `404 {"error":"unknown_endpoint",...}`; deleted/tombstoned → `410 {"error":"endpoint_gone",...}`; both carry `X-HookBox-Endpoint` and **neither is logged as a trace**. (OQ-1 tombstone backs the `410`; `404` when indeterminate.) [existing — `app/interceptor/engine.py::_unknown_or_gone`]
- **Rate-limit 429** body: `{"error":"rate_limited","detail":...}` + headers `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`. **Chaos 5xx** body: `{"error":"chaos","detail":...}`. Chaos `dropout` closes the connection (no body), bounded by `CHAOS_DROP_TIMEOUT_S`. [existing]
- **State-gating fail direction (frozen):** a `state_requirement` that cannot be evaluated (unreadable state) **fails closed** — the gated rule does not match (OQ-3). The rate limiter **fails open** on internal error (AC-39/AC-S18).

### 5.6 SQLite schema — FROZEN (DDL replaces both the old SQLite tables **and** all four Redis responsibilities)

Applied as numbered migrations under `backend/migrations/` on startup (`sqlx::migrate!`), WAL + `foreign_keys=ON` + `busy_timeout` (mirrors `../shortener-link/backend/src/db.rs`). Timestamps are TEXT RFC3339 (`datetime('now')` default). **Parameterized SQL only — no string interpolation (AC-S17).** [existing tables verified at `app/database.py::_DDL`; the three Redis-replacement tables + `gone_at`/`chaos_mode` are **[new]**]

```sql
-- migrations/0001_init.sql  --------------------------------------------------

CREATE TABLE owners (                                      -- [existing]
    owner_id    TEXT PRIMARY KEY,                          -- sha256(lower(trim(email)))[:16], non-secret
    email       TEXT UNIQUE NOT NULL,
    secret_hash TEXT NOT NULL,                             -- sha256(owner_secret); rotates each /api/session
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE TABLE endpoints (                                   -- [existing] + chaos_mode + gone_at [new]
    token              TEXT PRIMARY KEY,                   -- gen_token: 10-char ambiguity-stripped, case-sensitive
    owner_id           TEXT NOT NULL,
    name               TEXT,
    auto_crud          INTEGER NOT NULL DEFAULT 0,
    target_url         TEXT,
    default_mode       TEXT NOT NULL DEFAULT 'mock_404',   -- 'mock_404' | 'echo'
    latency_ms         INTEGER NOT NULL DEFAULT 0,
    rate_limit_per_min INTEGER NOT NULL DEFAULT 0,
    chaos_pct          INTEGER NOT NULL DEFAULT 0,
    chaos_mode         TEXT NOT NULL DEFAULT 'error',      -- [new] OQ-2: 'error' | 'dropout'
    cors_enabled       INTEGER NOT NULL DEFAULT 1,
    gone_at            TEXT,                                -- [new] OQ-1: tombstone; non-null ⇒ 410 endpoint_gone
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    last_hit           TEXT,
    request_count      INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (owner_id) REFERENCES owners(owner_id)
);
CREATE INDEX idx_endpoints_owner ON endpoints(owner_id);

CREATE TABLE mock_rules (                                  -- [existing] + chaos_mode [new]
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL,
    name        TEXT,
    priority    INTEGER NOT NULL DEFAULT 100,
    enabled     INTEGER NOT NULL DEFAULT 1,
    match_json  TEXT NOT NULL DEFAULT '{}',                -- serialized MatchCriteria
    response_json TEXT NOT NULL DEFAULT '{}',              -- serialized ResponseSpec
    state_writes_json TEXT NOT NULL DEFAULT '[]',          -- serialized StateWrite[]
    latency_ms  INTEGER,                                   -- null ⇒ inherit endpoint
    rate_limit_per_min INTEGER,                            -- null ⇒ inherit endpoint
    chaos_mode  TEXT,                                      -- [new] OQ-2: null ⇒ inherit endpoint
    webhook_json TEXT,                                     -- serialized WebhookAction | null
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_rules_token ON mock_rules(token, priority, id);

CREATE TABLE request_logs (                                -- [existing] (traces)
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    token           TEXT NOT NULL,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    status_code     INTEGER NOT NULL,
    served_by       TEXT NOT NULL,
    matched_rule_id INTEGER,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    overhead_ms     INTEGER NOT NULL DEFAULT 0,
    request_headers TEXT, query_params TEXT, request_body TEXT,
    response_headers TEXT, response_body TEXT,
    trace_json      TEXT, state_snapshot TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_logs_token_id ON request_logs(token, id DESC);
CREATE INDEX idx_logs_created ON request_logs(created_at);

-- NEW: replaces Redis hash state:<token> ------------------------------------
CREATE TABLE endpoint_state (                              -- [new]
    token      TEXT NOT NULL,
    key        TEXT NOT NULL,                              -- ^[A-Za-z0-9_-]{1,64}$ enforced before write
    value      TEXT NOT NULL,
    expires_at TEXT NOT NULL,                              -- now + STATE_TTL_SECONDS (24h); checked at read + sweep
    PRIMARY KEY (token, key),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_state_expires ON endpoint_state(expires_at);

-- NEW: replaces Redis list crud:<token>:<collection> ------------------------
CREATE TABLE crud_collections (                            -- [new]
    token      TEXT NOT NULL,
    name       TEXT NOT NULL,                              -- ^[A-Za-z0-9_-]{1,64}$
    items_json TEXT NOT NULL DEFAULT '[]',                 -- JSON array of objects, each with uuid "id"
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,                              -- now + CRUD_TTL_SECONDS (24h)
    PRIMARY KEY (token, name),
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);
CREATE INDEX idx_crud_expires ON crud_collections(expires_at);
```

- **Rate-limit buckets and the tunnel registry are NOT in SQLite** — they are in-process (`DashMap` / `HashMap<String, TunnelConnection>`) by design (spec §3). They do not survive a restart; this is the accepted single-instance tradeoff.
- **OQ-1 tombstone:** `endpoints.gone_at`. A deleted endpoint is **not** hard-deleted; it is tombstoned (`gone_at = datetime('now')`, live config cleared) so the mock plane returns `410`. Tombstones are reaped by the sweep after `GONE_TTL_HOURS` (default 168h / 7d) — after which the token resolves to `404`. `#6 DELETE` is a tombstone-update, not a row delete; the response `200 Message` is unchanged.

### 5.7 Sandboxed templating grammar — FROZEN
Hand-written single-pass scanner; closed tag set; unknown/malformed → left **literal**; never errors the mock path; **no eval / no general template-engine crate over user text / no format-string over user text (AC-S5).** Tags (FROZEN): `{{now 'iso'|'unix'|'epoch_ms'}}`, `{{random 'uuid'}}`, `{{random 'int' <lo> <hi>}}`, `{{random 'hex' <len>}}`, `{{request.method|path|body}}`, `{{request.query.<k>}}`, `{{request.path.<name>}}`, `{{request.header.<name>}}`, `{{request.body.<jsonpath>}}`, `{{state.<k>}}`. DoS bounds: template > `TEMPLATE_MAX_SIZE` returned unrendered; at most `TEMPLATE_MAX_TAGS` substitutions. [existing — `app/interceptor/templating.py`]

### 5.8 Tunnel control protocol — FROZEN
- WS endpoint `GET /ws/tunnel/{slug}`; bind auth `Authorization: Bearer <owner_secret>` (or `?cap=`), verified before `accept()`/registration.
- Server greets bound CLI with `{"t":"bound","slug":<slug>}`.
- Frames (JSON text, multiplexed by integer `id`):
  - `→ client {"t":"req","id":int,"method":str,"path":str,"query":{<k>:<v>},"headers":{<k>:<v>},"body_b64":str}`
  - `← client {"t":"res","id":int,"status":int,"headers":{<k>:<v>},"body_b64":str}`
  - `← client {"t":"err","id"?:int,"message":str}`
  - `↔ {"t":"ping"}` / `{"t":"pong"}` keepalive.
- Close codes: `4401` unauthorized bind; `4409` rebound (last-authenticated-bind-wins takeover — prior socket gets `{"t":"err","message":"rebound elsewhere"}` then close `4409`).
- Miss / drop / per-request timeout (`TUNNEL_REQUEST_TIMEOUT_S`, 30s) → public caller gets `504 {"error":"no_tunnel",...}` (never a hang). Tunneled traffic is subject to the same ingest/rate caps as any P1 path. [existing — verified at `app/routes/tunnel.py`]

---

## 6. Affected files (existing — verified; retired by the re-platform but the parity reference)

The Python tree is the byte-level parity reference; it is retired from the working tree (kept in git history). Each cited path is the source of truth for the behavior it backs:
- `app/planes.py` — three-plane host+path dispatch (AC-6/7/8/9, §5.5).
- `app/auth.py`, `app/database.py` — capability auth, rotation, `gen_token`/`hash_email`/`hash_secret`, the existing SQLite DDL (§5.1, §5.6).
- `app/models.py` — the annotated "FROZEN §5.3 interface contract" (§5.3).
- `app/routes/api.py` — the 18 `/api` routes + session (§5.2).
- `app/routes/tunnel.py` — tunnel WS server, registry, framing, close codes (§5.8, AC-49/50/51).
- `app/websocket.py` — feed WS/SSE, conn cap, heartbeat, broadcast (§5.4, AC-41/42/43).
- `app/interceptor/{engine,matcher,templating,crud,proxy,cors,conditions}.py` — the P1 pipeline (§5.5, AC-11..AC-40, AC-56..AC-61).
- `app/redis_state.py`, `app/pubsub.py`, `app/rule_cache.py` — the Redis responsibilities being re-homed to SQLite + in-process structures (§5.6).
- `app/utils/helpers.py` — `is_safe_key`, jsonpath-lite, header strip/redact (AC-23/25/27/31/61, AC-S4/S13/S17).
- `config.py` — every cap/limit/default (§5.5 caps; ported to a Rust `config` module).
- `templates/base.html`, `templates/dashboard.html`, `templates/index.html`, `templates/partials/*` — behavioral parity reference for the SPA (NOT a code source); the hardcoded-hex `<style>` is explicitly not carried (AC-D11).
- `Dockerfile`, `docker-compose.yml` — re-authored to single-container (AC-54).

## 7. New files (to be created — full inventory in `architecture.md` "Component & file design")

- **BE lane** (`backend/`, `migrations/`, `Cargo.toml`): the `0001_init.sql` migration (§5.6); `hookbox` / `tunnel` / `seed` bins; `config.rs`, `state.rs`, `error.rs`, `db.rs`, `ids.rs`, `auth.rs`, `planes.rs`, `router.rs`; `routes/{api,health,feed,tunnel_ws,spa}.rs`; `interceptor/{engine,matcher,templating,crud,proxy,cors,conditions}.rs`; `state_store.rs`, `crud_store.rs`, `rule_cache.rs`, `feed.rs`, `limiter.rs`, `ssrf.rs`, `helpers.rs`; `tasks/sweep.rs`; `tests/api.rs`.
- **FE lane** (`src/`, `public/`, `dist/`, Vite config): the React SPA — landing/email gate, split-screen dashboard, 5-tab rule builder, rules manager, endpoint settings, tunnel/CLI page; the `design.md` tokens in `src/globals.css` + `tailwind.config.ts`; the `copy.md` string table wired 1:1; `useRequestStream` WS+SSE hook; zod schemas mirroring §5.3.
- **Deploy:** `scripts/start.sh`; single-container Docker assets.

## 8. Risks & assumptions

- **R1 — Behavioral drift.** The value is *faithful* parity; "close" silently breaks consumers' mocks. Mitigation: §5 lifted verbatim from verified code; QA validates status/headers/shapes/order/caps; port the Python tests' intent into `cargo test`.
- **R2 — Concurrency model change.** Python's single-threaded asyncio made the tunnel registry / state writes race-free for free; multi-threaded tokio needs explicit `Send + Sync` (`DashMap` buckets, `Mutex`/`DashMap` tunnel registry, `BEGIN IMMEDIATE`+`busy_timeout` for CRUD atomicity, `Mutex`-guarded per-request-id oneshot map). Mitigation: architecture.md R2/R3; AC-J11 pins the observable contracts.
- **R3 — SSRF re-proof on the Rust DNS/TLS stack (CRITICAL).** `reqwest` resolves the URL host itself; a naive port has a TOCTOU rebinding hole. Mitigation: AC-S1/S2/S3 + §5.5 freeze the resolved-IP guard, IP-pinning (no second resolution), redirects-off-and-revalidate; the security gate probes a rebinding host, `169.254.169.254`, `[::1]`, `::ffff:169.254.169.254`, redirect→internal.
- **R4 — Loss of the Redis backstop for resource limits (DoS).** Everything is now one process; a missing cap is a single-binary OOM. Mitigation: AC-S18/S19 + §5.5 — bounded bucket map, bounded broadcast channel with lag-drop, ingest `413` before buffering, WS conn cap, per-send timeout.
- **R5 — SSTI via a convenience template crate.** Reaching for `tera`/`handlebars`/`minijinja` reintroduces SSTI. Mitigation: AC-S5 + §5.7 freeze the hand-written scanner; the security gate runs the SSTI probe set.
- **A1** The architect adopts `../shortener-link`'s crate/framework palette (Axum 0.7, sqlx-sqlite, reqwest+rustls, tokio, tracing) and React/Radix/Tailwind/CVA foundation unless documented otherwise (architecture.md A1).
- **A2** "Faithful parity" = the externally observable contract (status, headers, shapes, order, caps) is preserved; internal structure is idiomatic Rust.
- **A3** Default serve port is `8080` (OQ-4, frozen).

## 9. Open Questions

**None — all resolved during discovery.** Recorded resolutions (frozen into §5 / §4):

- **OQ-1 (architect):** `410 endpoint_gone` vs `404` — kept via a `endpoints.gone_at` tombstone column; `DELETE` tombstones (clears live config), the sweep hard-deletes tombstones older than `GONE_TTL_HOURS` (7d), after which the token degrades to `404`; security-safe default `404` when indeterminate (security F12/OSQ-1). Frozen in §5.6; backs AC-57, AC-J2, AC-J8.
- **OQ-2 (architect):** `chaos_mode "error"|"dropout"` promoted to a first-class `endpoints` column (`NOT NULL DEFAULT 'error'`) + nullable per-rule override (`mock_rules.chaos_mode`); joins §5.3 (`EndpointConfigPatch`/`EndpointDetail`/`MockRuleCreate/Patch/MockRule`). Backs AC-15/40; UI controls at `copy.md` `set.cond.chaosMode.*` / `rule.thr.chaosMode.*`.
- **OQ-3 (security):** post-Redis degradation — single SQLite store; state-gated rule evaluation **fails closed** when state is unreadable; CRUD/store SQLite fault → stable `503 store_unavailable`; the two rate limiters **fail open** but bounded (AC-S18). Security-signed-off. Frozen in §5.5/§5.6.
- **OQ-4 (product):** default serve port standardized on **8080** (spec + `../shortener-link`); `APP_PORT`/`PUBLIC_PORT` env override retained. Backs AC-53.
- **Security OSQ-1..OSQ-5:** OSQ-1 → OQ-1 (404 when indeterminate). OSQ-2 → SPA holds the cap non-cookie (memory/`localStorage`); AC-S14 + §5.1. OSQ-3 → IP-pinning preserves TLS SNI/cert against the original hostname; AC-S2 + §5.5. OSQ-4 → bounded bucket map + bounded broadcast channel; AC-S18/S19 + §5.5. OSQ-5 → `tracing` deny-list for `authorization`/`cookie`/`cap`/`x-owner-id` and never log `/ws|/sse` query strings; AC-S13 + §5.4.
- **Copy §6 questions (decisions recorded, aligned to "fresh, beautiful, light"):**
  - C1 — Template render-preview: **omitted** (a preview would need the real scanner or it lies); `rule.tmpl.honesty` copy ships instead. (Non-goal §2.)
  - C2 — `webhook_action`: **visible-but-disabled** "Stored, not yet sent" control, not omitted (AC-J10); disclosure wording accepted.
  - C3 — `410` long-tail: after the 7d window a deleted token shows the `404` "doesn't exist" copy; **no extra long-tail state** (keep it light). (AC-J2.)
  - C4 — `chaos_mode` phrasing accepted ("Dropout — drop the connection"); per-rule "Inherit from endpoint" option ships.
  - C5 — `echo` feed representation: **normal `default` chip + 200**, no distinct "echoed" label (AC-J10).
  - C6 — CLI secret: **reveal-on-demand** with the firm `cli.secret.warning`; plaintext reveal accepted (operator needs the value to run the command).
  - C7 — connection-cap: distinct `feed.conn.busy` state, FE maps `1013`/`503` to it (AC-J6).
  - C8 — fixed cap literals in copy ("last 100", 1000, 64 KB, 256 KB, 1 MB): **fixed in copy**; a changed cap is a copy edit, not interpolation.
  - C9 — sign-out: **confirm step** kept (`shell.account.signOut.*`).
  - C10 — endpoint delete: **typed-token confirm** kept (`set.confirm.delete.*`, AC-J8).
- **Design §12 / UX gaps (decisions recorded):**
  - D1 — first-paint theme: **light default** for new visitors, then `prefers-color-scheme`, then persisted toggle; both themes AA (AC-D24).
  - D2 — brand mark + accent: **teal** accent + inline-SVG hook-mark accepted (distinct from old `#58a6ff`/reference indigo); isolated to `--teal-*`/`--accent*` + the SVG.
  - D3 — visual ACs folded into Area 10 as AC-D11..AC-D24.
  - D4 — light accent-fill contrast: `--teal-600` fallback accepted if rendered ratio < 4.5:1 (AC-D20).
  - D5 — `webhook_action` warning-variant `InlineAlert` accepted (AC-J10).
  - D6 — decorative landing texture: purely-CSS, `aria-hidden`, reduced-motion-safe — **in scope** for the "beautiful & light" bar.
- **Journey J9 / J12 (decisions recorded):**
  - J9 — Auto-CRUD/state visibility: **Settings peek/clear only** (API-backed), no full data-browser screen (AC-J9).
  - J12 — first-run seeded demo: the seeded `primary` endpoint ships with ≥1 rule + sample traces so the operator sees a live dashboard, not raw empty states (AC-J12).

---

## 10. Task graph (beads)

**Feature epic:** `hookbox-sks` (label `feature:hookbox-rust-replatform`). 23 task issues (15 BE + 8 FE) → QA gate → security gate → sync. `bd dep cycles` clean.

**Backend lane** (`backend/`, `migrations/`, `Cargo.toml`):

| Issue | Title | AC index |
|-------|-------|----------|
| `hookbox-sks.11` | scaffold: Cargo.toml (3 bins) · Config · AppState · error envelope · pool + `0001_init.sql` | AC-53, AC-S17, §5.6 |
| `hookbox-sks.12` | ids + auth + capability (rotate/hash · Bearer · 401-vs-404 · cap-owns-token) | AC-1..5, AC-60, AC-S6/S7/S8, §5.1 |
| `hookbox-sks.13` | plane router — `resolve_plane` + P1/P2/P3 dispatch middleware | AC-6..10, AC-S16 |
| `hookbox-sks.14` | the 18 `/api/**` routes + `/healthz` | AC-1..5/22/27/32/48/60, §5.2/§5.3 |
| `hookbox-sks.15` | rule engine + matcher + conditions (order · priority,id · criteria) | AC-11..13/15/37/38/40/56/57, §5.5 |
| `hookbox-sks.16` | sandboxed templating scanner + state_writes (no eval) | AC-16..19, AC-S5, §5.7 |
| `hookbox-sks.17` | state_store + Auto-CRUD store (TTL/lazy · `BEGIN IMMEDIATE` CAS) | AC-20..27, AC-S17, OQ-3 |
| `hookbox-sks.18` | MITM proxy + SSRF guard (resolved-IP + pinning) + header strip | AC-28..32, AC-S1/S2/S3/S4 |
| `hookbox-sks.19` | Auto-CORS engine (P1-only · no Allow-Credentials) | AC-33..36, AC-S11/S12 |
| `hookbox-sks.20` | rate-limit token bucket (bounded · fail-open) + rule_cache | AC-39/15/11, AC-S18 |
| `hookbox-sks.21` | FeedHub + WS/SSE endpoints (broadcast · owner-gate · close codes · heartbeat · cap) | AC-41..44, AC-S9/S13/S19, §5.4 |
| `hookbox-sks.22` | trace persistence (off-path · write-time prune) + retention sweep | AC-46..48/58/59/61/57, AC-S13/S15 |
| `hookbox-sks.23` | tunnel WS server + `tunnel` CLI bin (bind auth · last-bind-wins · 504 no_tunnel) | AC-49..52, AC-S10, §5.8 |
| `hookbox-sks.24` | seed bin + SPA serving/fallback + `scripts/start.sh` + Docker | AC-53/54/J12 |
| `hookbox-sks.25` | cargo integration tests (`tests/api.rs`) over the full §5 surface | AC-55 |

**Frontend lane** (`src/`, `public/`, Vite config):

| Issue | Title | AC index |
|-------|-------|----------|
| `hookbox-sks.26` | Vite/React/TS scaffold + Tailwind semantic tokens + ui/ + HookBox primitives | AC-45, AC-D11/D12/D14/D15/D17/D20/D21/D24 |
| `hookbox-sks.27` | API client + zod types + auth/capability (Bearer · 401 bounce · rotation) | AC-1..5 (FE), AC-60/S14/J7, §5.2/§5.3 |
| `hookbox-sks.28` | live-feed hook — WS+SSE (backoff · heartbeat · fallback · N-new · cap · reconcile) | AC-41/42 (FE), AC-43/J5/J6/J13/D16/D17/D18, §5.4 |
| `hookbox-sks.29` | landing / email gate (`/`) — all states · copy 1:1 · anti-enumeration | AC-1 (FE), AC-D22, AC-J1 |
| `hookbox-sks.30` | split-screen dashboard — feed + inspector (5 tabs) · all states | AC-44/45, AC-D13/D18/D19, AC-J2/J3/J4/J10 |
| `hookbox-sks.31` | 5-tab rule builder + rules manager | AC-14 (+11/12/13/15 FE), AC-D23, AC-J10 |
| `hookbox-sks.32` | endpoint settings (Danger zone · peek/clear · state/history) + `/cli` page | AC-J8/J9, AC-49..52 (FE), AC-J10 |
| `hookbox-sks.33` | Playwright e2e (journeys + visual/state ACs) | AC-55 (FE), AC-D11..D24, AC-J1..J13 |

**Gates:** `hookbox-sks.34` QA gate (blocked by .11–.33) → `hookbox-sks.35` security review gate (blocked by .34) → `hookbox-sks.36` sync (blocked by .35). Epic `hookbox-sks` closes at sync.
