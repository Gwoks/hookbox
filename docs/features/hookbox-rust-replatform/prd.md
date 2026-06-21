# PRD: HookBox — Rust/Axum + SQLite Re-platform (slug: hookbox-rust-replatform)

- **Status:** DRAFT (product-manager, pipeline seed)
- **Date:** 2026-06-21
- **Authoritative inputs:** `docs/superpowers/specs/2026-06-21-hookbox-rust-replatform-design.md` (APPROVED, locked — platform/§3, layout/§4, planes/§5, data model/§6, pipeline/§7-§8, auth/§9, frontend/§10 are FROZEN), `FEATURES.md` (the 13 canonical feature areas), the current Python implementation under `app/` (verified, cited per behavior), and `../shortener-link/` (the reference Rust+SQLite+Vite/React project whose conventions we mirror).
- **Downstream consumers:** user-journey, ui-ux, system-architect (owns the final §5 contract + `architecture.md`), design-agent (`design.md`), copywriter-engineer (`copy.md`), security-engineer (`security.md`), then REVISE → BREAKDOWN.

> **§5 is DRAFT here.** It is lifted verbatim from the *verified current Python behavior* (the faithful-parity baseline) and re-expressed on the Rust/React stack. The **system-architect owns and finalizes §5** in `architecture.md`; REVISE will lift it back to match exactly. Where the Python contract is precise it is reproduced; where the re-platform forces a genuine choice it is flagged in §9.

---

## 1. Goal

Recreate HookBox — today a Python **FastAPI + Jinja + SQLite + Redis + WebSockets** app (verified across `app/`, `templates/`, `config.py`) — as a **feather-weight, self-contained** application on the exact platform `../shortener-link` uses: a **single Rust (Axum) binary over SQLite (WAL)** that also serves a **Vite + React + TypeScript SPA**. **No Redis, no Postgres, Docker optional.** Single-instance by design — every responsibility Redis carries today (per-endpoint state, Auto-CRUD storage, rate-limit buckets, the live-feed pub/sub) moves to **in-process structures + SQLite**.

The recreation is **full feature parity across all 13 HookBox feature areas** (§4), with **byte-faithful preservation** of the externally observable contract: the resolution order, the `X-HookBox-*` headers, the status codes, the caps/limits, and the management-API shapes (§5). The frontend gets a **fresh visual identity and a distinct product voice** (design-agent + copywriter-engineer own this) — not a 1:1 visual port of the current Jinja dashboard.

## 2. Non-goals

- **Multi-instance / horizontal scale.** Deliberately traded away with Redis (spec §2). In-process broadcast/buckets/registries are single-process; the self-host sweet spot is vertical scale on one node.
- **Keeping the Python codebase runnable.** `app/`, `templates/`, `config.py`, `requirements*.txt`, `tunnel/`, `Dockerfile`, `docker-compose.yml` are retired (they remain in git history). They are the *parity reference*, not a deliverable.
- **Email delivery / verification / passwords / registration.** The email-keyed, capability-backed model is preserved exactly as today (`app/auth.py`, `app/database.py`): email → non-secret `owner_id` + a minted 256-bit owner capability that rotates on each submit. No new identity surface.
- **New product features.** This is a re-platform at parity, not a feature expansion. `webhook_action` remains accepted-and-stored-but-no-op as it is today (`app/models.py` `WebhookAction`, current OQ-9).
- **A documented public REST API as a product.** `/api/**` exists for the SPA; it is not a versioned public surface.

## 3. Background & platform

### 3.1 What exists today (verified)
HookBox is a Beeceptor-class mock/intercept platform behind a no-password, email-keyed session (`FEATURES.md`). The mock surface, the management API, and the dashboard are **three hard-isolated request planes** decided purely from `Host` + path in `app/planes.py` (`resolve_plane`). Durable data lives in SQLite via `aiosqlite` (`app/database.py`: `owners`, `endpoints`, `mock_rules`, `request_logs`). **Ephemeral/shared data lives in Redis** (`app/redis_state.py`): per-endpoint state hash `state:<token>`, Auto-CRUD lists `crud:<token>:<collection>`, rate-limit token buckets `rl:<token>[:<rule_id>]`, and the live-feed pub/sub channels `trace:<token>` / `cfg:<token>`. The real-time dashboard fan-out is Redis pub/sub → `ConnectionManager.broadcast` over WS/SSE (`app/websocket.py`).

### 3.2 The re-platform: Redis → in-process + SQLite (single-instance)
Every Redis responsibility is re-homed:

| Redis responsibility today (verified) | Re-platform target (new, Rust) |
|---|---|
| `state:<token>` hash, 24h TTL (`redis_state.get/set/clear_state`) | SQLite `endpoint_state(token, key, value, expires_at)` table; 24h TTL enforced at read + by the sweep (spec §6) |
| `crud:<token>:<collection>` JSON-array list, atomic CAS (`crud_cas`) | SQLite `crud_collections(token, name, items_json)`; atomic read-modify-write inside one SQLite transaction (spec §6) |
| `rl:<token>[:<rule_id>]` Lua token bucket (`rate_limit_check`) | In-memory token bucket (e.g. `DashMap`) keyed by endpoint(+rule); `0` = unlimited; **fails open** (spec §5/§8) |
| `trace:<token>` / `cfg:<token>` pub/sub fan-out | In-process `tokio::sync::broadcast` channel → owner-gated WS subscribe + SSE fallback (spec §5/§8) |
| Redis-down degradation contract (`RedisUnavailable`) | **Mostly eliminated.** State/CRUD are now SQLite (the single datastore), so the old "state-gated rules fail closed / CRUD→503 when Redis down" branches collapse to ordinary SQLite error handling (see §8, §9 OQ-3) |

**Single-instance tradeoff (accepted):** the broadcast channel, the rate-limit buckets, and the tunnel registry are per-process. Two binaries do **not** share live feed or buckets. This matches the spec's locked decision and the `../shortener-link` precedent (one binary, in-process tasks). Horizontal scale is explicitly out of scope (§2).

### 3.3 Target layout (mirrors `../shortener-link`, spec §4)
```
backend/        Rust crate — Cargo.toml bins: hookbox (server), tunnel (CLI), seed
  migrations/   SQLite schema migrations (applied on startup)
  src/          axum router, planes, interceptor, templating, proxy, ws/sse, ...
  tests/        cargo integration tests
src/            Vite + React + TS SPA
public/         static assets
dist/           built SPA (served by the binary with SPA fallback)
data/app.db     SQLite (WAL), gitignored
scripts/start.sh  pnpm build → cargo build --release → migrate → seed → serve :8080
```
Reference conventions to mirror (verified in `../shortener-link`): Axum 0.7 + tokio + tower-http; `sqlx` (sqlite, migrate, chrono); `reqwest` with `rustls-tls`; `governor`/`moka` for limiting/caching; `tracing`; React 18 + Radix + Tailwind + CVA + react-router + zod (`../shortener-link/package.json`); `scripts/start.sh` build-then-run shape.

---

## 4. Features & Acceptance Criteria — all 13 areas at parity

Each AC is testable and observable. Behaviors tagged **[existing — verified at `<path>`]** are ported faithfully (byte-level contract preserved); **[new]** marks Rust/React artifacts to be created. Caps reference `config.py` (verified) and become env-driven config in the Rust binary.

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
- **AC-14** The rule builder exposes the five tabs Matching · Response · Templating · Actions · Throttling, mapping 1:1 to `MatchCriteria`, `ResponseSpec`, template tags, `state_writes`/`webhook_action`, and per-rule `latency_ms`/`rate_limit_per_min`. [new — React; data shapes existing at `app/models.py::MockRuleCreate`]
- **AC-15** Per-rule `latency_ms` / `rate_limit_per_min` overrides, when set, take precedence over the endpoint-level values for that served request. [existing — verified at `app/interceptor/engine.py` (`rule_latency_override`/`rule_rate_override`)]

### Area 4 — Dynamic response templating (sandboxed) — `FEATURES.md §4`
- **AC-16** A **hand-written single-pass scanner** resolves the closed tag set: `{{now 'iso'|'unix'|'epoch_ms'}}`, `{{random 'uuid'}}`, `{{random 'int' <lo> <hi>}}`, `{{random 'hex' <len>}}`, `{{request.method|path|body}}`, `{{request.query.<k>}}`, `{{request.path.<name>}}`, `{{request.header.<name>}}`, `{{request.body.<jsonpath>}}`, `{{state.<k>}}`. [existing — verified at `app/interceptor/templating.py`]
- **AC-17** There is **no `eval`/`exec`, no general template engine, and no string-format over user text.** SSTI probes (`{{ 7*7 }}`, `{{ config }}`, `{{ ''.__class__ }}`, `{{ self }}`) are unknown tags → returned **verbatim**, executing zero code. [existing — verified at `app/interceptor/templating.py::_resolve_tag`]
- **AC-18** Unknown/malformed tags are **left literal** and never error the mock path; a template over `TEMPLATE_MAX_SIZE` (256 KB) is returned unrendered; at most `TEMPLATE_MAX_TAGS` (500) substitutions are performed. [existing — verified at `app/interceptor/templating.py::render`, `config.py`]
- **AC-19** `state_writes` are rendered with the same template context and applied **before** the body renders, so `{{state.k}}` in the same response observes a just-written value. [existing — verified at `app/interceptor/engine.py::_apply_state_writes`]

### Area 5 — Stateful / multi-step transactions — `FEATURES.md §5`
- **AC-20** A rule can write per-endpoint state (`state_writes`); a later rule with a `state_requirement` matches only when that state holds (e.g. `POST /login` sets `authenticated=true`; `/dashboard` requires it). State is **per-endpoint, shared across all callers**. [existing — verified at `app/interceptor/matcher.py::_state_ok`, `engine.py`]
- **AC-21** State is persisted with a 24h TTL (`STATE_TTL_SECONDS`) and is read **lazily** — only when some enabled rule gates on state. [existing — verified at `app/interceptor/engine.py` (`ep.any_rule_gates_on_state`), `config.py`]
- **AC-22** `GET /api/endpoints/{token}/state` returns `{state: {...}}` and `DELETE …/state` clears it; both are owner-gated. [existing — verified at `app/routes/api.py::get_state/clear_state`]
- **AC-23** A `state_write`/`state_requirement` key must pass the safe charset (`^[A-Za-z0-9_-]{1,64}$`); an unsafe key is skipped/ignored, never persisted. [existing — verified at `app/utils/helpers.py::is_safe_key` usage]

### Area 6 — Instant Auto-CRUD — `FEATURES.md §6`
- **AC-24** With `auto_crud` enabled and **no rule matching**, the endpoint serves a REST DB over a per-collection JSON array: `POST /<coll>`→`201` (server-assigned uuid `id`), `GET /<coll>`→`200` (array), `GET/PUT/PATCH/DELETE /<coll>/<id>`→`200`/`200`/`200`/`204` or `404`, `HEAD` mirrors `GET`. [existing — verified at `app/interceptor/crud.py::handle`]
- **AC-25** `<coll>` and `<id>` must match `^[A-Za-z0-9_-]{1,64}$`; a 3+-segment or unsafe path is **not CRUD** and falls through to tunnel/MITM/default. A write body must be a JSON **object** (else `400`); item-count cap `CRUD_MAX_ITEMS` (1000) and per-item byte cap `CRUD_MAX_ITEM_BYTES` (64 KB) yield `400` when exceeded. [existing — verified at `app/interceptor/crud.py`, `config.py`]
- **AC-26** Concurrent writes to one collection do not lose updates — the read-modify-write is **atomic** (today Redis WATCH/MULTI; in the re-platform, one SQLite transaction). A server-assigned uuid `id` means two POSTs never collide. [existing — verified at `app/interceptor/crud.py` (`crud_cas`), spec §6]
- **AC-27** `GET /api/endpoints/{token}/collections/{name}` peeks (`{items: [...]}`), `DELETE …/collections/{name}` clears; both owner-gated; an unsafe `name` → `422`. [existing — verified at `app/routes/api.py::peek_collection/clear_collection`]

### Area 7 — Proxy / partial mocking (MITM) — `FEATURES.md §7`
- **AC-28** With a `target_url` set and no rule/CRUD/tunnel handling the request, the request is forwarded to the upstream and the real response (status, safe headers, capped body) is returned labeled `served_by="mitm"`. A matching local rule **always wins** over forwarding. [existing — verified at `app/interceptor/proxy.py::forward`, `engine.py::_resolve_unmatched`]
- **AC-29** **SSRF guard (evaluated on the resolved IP, not the hostname):** the target is resolved and **every** resolved address is rejected if loopback / private / link-local / multicast / reserved / unspecified / cloud-metadata `169.254.169.254`; a blocked target → `502`. The connection is **pinned to the validated IP** (Host header + TLS SNI preserved) so a DNS-rebinding swap between check and connect is impossible. [existing — verified at `app/interceptor/proxy.py::_resolve_and_check/_pin_target`]
- **AC-30** Redirects are **not followed by default** (`MITM_FOLLOW_REDIRECTS=false`); when enabled, each hop re-applies the SSRF check, bounded by `MITM_MAX_REDIRECTS`. Timeout → `504`; connection/DNS/SSRF error → `502 upstream_unreachable`; body capped at `MITM_MAX_BODY_BYTES` (5 MB) with `X-HookBox-Truncated: true` when truncated. [existing — verified at `app/interceptor/proxy.py`, `config.py`]
- **AC-31** The owner capability and hop-by-hop/sensitive request headers are **stripped before forwarding** (never sent upstream); upstream hop-by-hop / `Set-Cookie` / upstream-CORS / `content-length` / `content-encoding` headers are stripped from the captured response. [existing — verified at `app/interceptor/proxy.py::_STRIP_RESPONSE_HEADERS`, `strip_forward_headers`]
- **AC-32** `target_url` is validated at the API boundary to `http`/`https` with a host (else `422` on PATCH); empty/null clears it. [existing — verified at `app/models.py::_validate_target_url`]

### Area 8 — Auto-CORS engine — `FEATURES.md §8`
- **AC-33** On the **mock plane only (P1)**, when `cors_enabled`, an `OPTIONS` preflight to any mock path returns `204` with reflected `Access-Control-Allow-Origin`, reflected `Access-Control-Request-Headers`, `Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`, `Max-Age: 600`, `Vary: Origin` — with no user-defined rule. [existing — verified at `app/interceptor/cors.py::preflight_headers`]
- **AC-34** Every non-preflight P1 response carries `Access-Control-Allow-Origin` (reflected Origin, or `*` when absent), `Access-Control-Expose-Headers: *`, `Vary: Origin`. `Access-Control-Allow-Credentials` is **never** emitted (no credentialed wildcard). [existing — verified at `app/interceptor/cors.py::response_headers`]
- **AC-35** The **management API (P2) emits no wildcard CORS** — these headers are produced only on P1. [existing — verified at `app/interceptor/cors.py` docstring + `engine._identified` call site]
- **AC-36** When `cors_enabled=false`, the per-response CORS set is empty and `OPTIONS` still returns a deterministic `204` (a preflight never falls through to a rule/404). [existing — verified at `app/interceptor/cors.py::preflight_response`]

### Area 9 — Simulated network conditions (latency · rate-limit · chaos) — `FEATURES.md §9`
- **AC-37** Conditions are applied around the served response in the **frozen order rate-limit → chaos → latency**. [existing — verified at `app/interceptor/engine.py` steps 5, `conditions.py`]
- **AC-38** **Latency** (`latency_ms`, clamped 0–10000) sleeps cooperatively (never blocks other endpoints); the applied ms is recorded so `overhead_ms = duration − applied_latency` is observable. [existing — verified at `app/interceptor/conditions.py::apply_latency`, `engine.py`]
- **AC-39** **Rate limit** (`rate_limit_per_min`, `0`=unlimited, clamped ≤ `RATE_LIMIT_MAX_PER_MIN`=100000) is a token bucket keyed per endpoint (or per rule on override), applied to **every** served path including MITM forwards and Auto-CRUD writes; over-limit → `429` with `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`; the limiter **fails open** on internal error. [existing — verified at `app/interceptor/conditions.py::check_rate_limit`, `engine.py::_rate_limit`]
- **AC-40** **Chaos** (`chaos_pct` 0–100) injects on a hit a random `502/503/504` by default; `chaos_pct=0` never fires, `100` always fires. An **opt-in** connection-drop (`chaos_mode="dropout"`) closes the connection, bounded by `CHAOS_DROP_TIMEOUT_S` so it cannot hang a worker. [existing — verified at `app/interceptor/conditions.py::roll_chaos`, note: `chaos_mode` is **not** in the frozen §5.3 schema today — see §9 OQ-2]

### Area 10 — Real-time split-screen dashboard — `FEATURES.md §10`
- **AC-41** Every served mock request is logged and published; subscribed clients receive a `new_request` event (the `RequestSummary` shape) in near-real-time over WebSocket, with an SSE fallback delivering the identical stream. [existing — verified at `app/websocket.py`, `engine._persist_and_publish`]
- **AC-42** The feed is **owner-gated**: the capability is presented as `?cap=<owner_secret>` and verified **before any frame** — an anonymous/wrong/cross-owner subscribe gets **zero** events (WS close `4401` / SSE `401`). Channel isolation: a client authed for `tokenA` receives only `tokenA` events. [existing — verified at `app/websocket.py::feed_ws/feed_sse`, `app/auth.py::verify_cap_owns_token`]
- **AC-43** Concurrent feed connections per endpoint are bounded by `WS_MAX_CONN_PER_ENDPOINT` (50); excess are refused (WS close `1013` / SSE `503`). Per-client send is bounded by `WS_SEND_TIMEOUT_S`; dead/slow clients are dropped, never stalling fan-out. [existing — verified at `app/websocket.py` (`at_capacity`, `broadcast`)]
- **AC-44** On connect the server sends a `hello {token, server_time}` frame; the deep inspector shows Headers · Query · Body · Response Served · State & Tracing for a selected trace via `GET /api/requests/{id}`. [existing — verified at `app/websocket.py`, `app/routes/api.py::get_request`, `app/models.py::RequestDetail`]
- **AC-45** Split-screen dashboard (live feed + deep inspector), rule builder, and endpoint settings are rebuilt as a fresh React SPA. [new — React `src/`; design-agent + copywriter own visual/voice]

### Area 11 — Data retention — `FEATURES.md §11`
- **AC-46** A hard **per-endpoint trace cap of `TRACE_CAP`=100** is enforced **at write time** (prune to newest 100 by id on every insert) so it never drifts between sweeps. [existing — verified at `app/database.py::TraceWriter._PRUNE_SQL/insert_trace`]
- **AC-47** A background sweep (interval `RETENTION_SWEEP_SECONDS`=300) enforces the 100-cap **and** a **24h TTL** (`TRACE_TTL_HOURS`=24) on traces; expired `endpoint_state` / `crud_collections` rows (24h TTL) are also reaped. [existing — verified at `config.py`; sweep re-homed to a `tokio` interval task — spec §8]
- **AC-48** `GET /api/endpoints/{token}/requests?limit&offset` lists traces (newest first, `limit` 1–200) and `DELETE …/requests` clears history; both owner-gated. [existing — verified at `app/routes/api.py::list_requests/clear_requests`]

### Area 12 — Local tunnel CLI — `FEATURES.md §12`
- **AC-49** A second Rust binary (`tunnel`) reverse-tunnels public traffic for an endpoint to the operator's localhost: it opens one authenticated WebSocket to `/ws/tunnel/{slug}` presenting the owner capability as `Authorization: Bearer <secret>` (or `?cap=`); an unauthenticated/wrong-owner bind is refused with WS close `4401` and never registered. UX parity: `--port --endpoint --secret`. [existing — verified at `app/routes/tunnel.py`; CLI was `python -m tunnel` → now `tunnel` bin]
- **AC-50** In the resolution order, **tunnel is consulted after Auto-CRUD and before MITM**; tunneled traffic is forwarded down the bound socket and the CLI's response replayed to the public caller, labeled `served_by="tunnel"` in the feed. A second correctly-authenticated owner bind **takes over** (last-bind-wins): the prior socket gets `{t:"err","rebound elsewhere"}` then close `4409`. [existing — verified at `app/routes/tunnel.py::TunnelRegistry.bind`, `engine._resolve_unmatched`]
- **AC-51** No tunnel connected / drop / per-request timeout (`TUNNEL_REQUEST_TIMEOUT_S`=30) resolves to a deterministic `504 {error:"no_tunnel"}` (never a hang); tunneled traffic is subject to the same ingest/rate caps as every other P1 path; control frames use the JSON `{t:"req|res|err|ping|pong", id, ...}` shape with base64 bodies; backoff reconnect on the CLI side. [existing — verified at `app/routes/tunnel.py::forward_to_tunnel`, framing in module docstring]

### Area 13 — Deployment — `FEATURES.md §13`
- **AC-52** `scripts/start.sh` runs `pnpm build` (Vite → `dist/`) → `cargo build --release` → applies SQLite migrations on startup → seeds demo data on first run → serves on `http://localhost:8080` from the **single binary** (SPA + API + mock plane + WS/SSE + in-process sweep/buckets). [new — mirrors `../shortener-link/scripts/start.sh`; spec §11]
- **AC-53** **Docker is optional**: if kept, a **single app container** (no Redis, no Postgres) with a persistent volume for `data/app.db`; the old two-service `docker-compose.yml` (app + Redis with healthcheck wait) is re-authored to the single-container shape. [new — supersedes verified `Dockerfile`/`docker-compose.yml`; spec §3]
- **AC-54** All config is env-driven with safe defaults; a missing/blank `MOCK_DOMAIN` degrades to path-fallback-only and never crashes startup (AC-10). [existing — verified at `config.py`; ported to a Rust `config` module — see `../shortener-link/backend/src/config.rs`]
- **AC-55** `cargo test` (backend integration, mirroring `../shortener-link/backend/tests/`) and Playwright e2e (frontend) gate the build; the QA lane validates every AC and the full §5 contract on both FE and BE sides. [new — spec §11]

### Cross-cutting parity invariants
- **AC-56** Every mock response carries `X-HookBox-Endpoint: <token>` and `X-HookBox-Served-By: <rule|crud|mitm|tunnel|default|cors|chaos|ratelimit>`, plus `X-HookBox-Rule-Id: <id>` when a rule matched. [existing — verified at `app/interceptor/engine.py::_identified`]
- **AC-57** An unknown endpoint token on P1 returns `404 {error:"unknown_endpoint"}`; a deleted/expired token returns `410 {error:"endpoint_gone"}`; neither is logged as a trace. [existing — verified at `app/interceptor/engine.py::_unknown_or_gone`, `app/routes/api.py::delete_endpoint` (tombstone)]
- **AC-58** P1 request bodies over `MAX_INGEST_BODY_BYTES` (1 MB) are rejected with `413` before buffering; captured request/response bodies in traces are truncated at `MAX_BODY_BYTES` (256 KB). [existing — verified at `app/interceptor/engine.py::_read_body_capped`, `_spawn_trace`]
- **AC-59** The trace write + feed publish run **off the response path** (fire-and-forget); a slow/failed trace never delays or fails the served mock response. [existing — verified at `app/interceptor/engine.py::_spawn_trace` (`create_task`, never awaited)]
- **AC-60** Owner-capability auth: every `/api/**` route except `POST /api/session` requires `Authorization: Bearer <owner_secret>`; missing/malformed/unknown → `401`; valid-but-non-owner of `{token}` → **`404`** (never `403`, so a non-owner can't distinguish "exists but not mine" from "doesn't exist"). Error bodies are uniformly `{error, detail}`. [existing — verified at `app/auth.py`, `app/routes/api.py`]
- **AC-61** The owner capability and `Cookie`/`Authorization`/`X-Owner-Id` headers are **redacted** before a trace is persisted (never stored). [existing — verified at `app/interceptor/engine.py::_redact`]

---

## 5. Frozen interface contract — DRAFT (system-architect to finalize)

> Lifted from the verified Python contract (`app/models.py` is annotated as "the FROZEN §5.3 interface contract" today). FE and BE implement against exactly these shapes. **The system-architect owns the final word in `architecture.md`; REVISE lifts §5 from there to match.**

### 5.1 Auth (P2)
- All `/api/**` except `POST /api/session` require `Authorization: Bearer <owner_secret>`.
- `401 {error:"unauthorized", detail}` with `WWW-Authenticate: Bearer` for missing/malformed/unknown secret.
- `404 {error:"not_found", detail}` for a valid secret that does not own the addressed `{token}`/resource.
- Feed (WS/SSE) and tunnel-bind auth use `?cap=<owner_secret>` (feed) / `Authorization: Bearer` (tunnel), verified before any frame.

### 5.2 Management API endpoints (`/api/**`, all JSON; `{error, detail}` on error)
| # | Method | Path | Request | Success | Notes |
|---|---|---|---|---|---|
| 1 | POST | `/api/session` | `{email}` | `200 SessionResponse` | no auth; rotates secret; rate-limited `429` |
| 2 | GET | `/api/endpoints` | — | `200 EndpointSummary[]` | |
| 3 | POST | `/api/endpoints` | `EndpointCreate` | `201 EndpointDetail` | |
| 4 | GET | `/api/endpoints/{token}` | — | `200 EndpointDetail` | |
| 5 | PATCH | `/api/endpoints/{token}` | `EndpointConfigPatch` | `200 EndpointDetail` | partial; clamps/validates |
| 6 | DELETE | `/api/endpoints/{token}` | — | `200 Message` | tombstones → `410` on P1 |
| 7 | GET | `/api/endpoints/{token}/rules` | — | `200 MockRule[]` | ORDER BY priority,id |
| 8 | POST | `/api/endpoints/{token}/rules` | `MockRuleCreate` | `201 MockRule` | |
| 9 | GET | `/api/endpoints/{token}/rules/{id}` | — | `200 MockRule` | `404` if absent |
| 10 | PATCH | `/api/endpoints/{token}/rules/{id}` | `MockRulePatch` | `200 MockRule` | |
| 11 | DELETE | `/api/endpoints/{token}/rules/{id}` | — | `204` | `404` if absent |
| 12 | GET | `/api/endpoints/{token}/requests?limit&offset` | — | `200 RequestSummary[]` | `limit` 1–200, `offset` ≥0 |
| 13 | GET | `/api/requests/{id}` | — | `200 RequestDetail` | owner via trace's endpoint |
| 14 | DELETE | `/api/endpoints/{token}/requests` | — | `200 Message` | clear history |
| 15 | GET | `/api/endpoints/{token}/state` | — | `200 {state:{}}` | |
| 16 | DELETE | `/api/endpoints/{token}/state` | — | `200 Message` | |
| 17 | GET | `/api/endpoints/{token}/collections/{name}` | — | `200 {items:[]}` | unsafe name → `422` |
| 18 | DELETE | `/api/endpoints/{token}/collections/{name}` | — | `200 Message` | |
| 19 | GET | `/healthz` | — | `200` | P3 liveness |

[existing — verified at `app/routes/api.py` (the 18 documented `/api` routes) + `/healthz`]

### 5.3 Shared data models (Rust structs / TS types serialize to these JSON shapes)
Reproduced from `app/models.py` (verified). Field names, types, defaults, and validation clamps are part of the contract:
- `SessionCreate{email}` · `SessionResponse{owner_id, owner_secret, endpoints[], primary}` · `EndpointSummary{token, name?, mock_url, path_url, created_at, last_hit?, request_count}`.
- `EndpointCreate{name?≤100}` · `EndpointConfigPatch{name?, auto_crud?, target_url?(http/https or null), default_mode?("mock_404"|"echo"), latency_ms?(0–10000), rate_limit_per_min?(0–100000), chaos_pct?(0–100), cors_enabled?}` · `EndpointDetail{…all config…, tunnel_active, created_at, last_hit?, request_count}`.
- `MatchCriteria{method="ANY", path="/*", headers{}, query{}, body_conditions[BodyCondition{path,op(eq|neq|contains|exists),value?}], state_requirements[StateRequirement{key,op(eq|neq|exists|absent),value?}]}`.
- `StateWrite{key,value}` · `ResponseSpec{status_code=200(100–599), headers{}, body_template(≤256000), content_type="application/json"}` · `WebhookAction{url, body_template}` (stored, no-op).
- `MockRuleCreate{name?≤120, priority=100(0–100000), enabled=true, match, response, state_writes[], latency_ms?(0–10000), rate_limit_per_min?(0–100000), webhook_action?}` · `MockRulePatch` (all optional) · `MockRule = MockRuleCreate + {id, token, created_at}`.
- `RequestSummary{id, token, method, path, status_code, served_by("rule"|"crud"|"mitm"|"tunnel"|"default"|"cors"|"chaos"|"ratelimit"), matched_rule_id?, duration_ms, overhead_ms, timestamp}` · `RequestDetail = RequestSummary + {request_headers{}, query_params{}, request_body?, response_headers{}, response_body?, trace[TraceEvent{step,detail}], state_snapshot{}}`.
- `Message{message, success=true}`.

[existing — verified verbatim at `app/models.py`]

### 5.4 WebSocket + SSE feed (P3)
- **Endpoints:** `GET /ws/{token}?cap=<secret>` (WebSocket); `GET /sse/{token}?cap=<secret>` (SSE fallback).
- **Auth:** verify cap owns token before any frame; WS close `4401` / SSE `401` on failure; connection-cap → WS close `1013` / SSE `503`.
- **Server→client events (JSON):** `hello{token, server_time}` (on connect), `new_request{…RequestSummary…}`, `state_changed{token, key, value}`, `endpoint_updated{token, fields[]}`.
- **Client→server:** `"ping"` text → server replies `"pong"` (WS); SSE sends `: ping` heartbeat comments every ~25s.

[existing — verified at `app/websocket.py`, payloads in `app/models.py` (`WsHello`, `WsStateChanged`, `WsEndpointUpdated`)]

### 5.5 Mock plane (P1) behavior — FROZEN resolution & wrapping
- **Resolution order:** `OPTIONS preflight → matching rule (priority,id) → Auto-CRUD → tunnel → MITM → default`.
- **Conditions wrap (around the served body):** `rate-limit (429) → chaos (5xx / connection-drop) → latency (sleep)`.
- **Default mode:** `mock_404` → `404 {error:"no_match"}`; `echo` → `200 {method, path, query, headers, body}`.
- **Identifying headers:** `X-HookBox-Endpoint`, `X-HookBox-Served-By`, `X-HookBox-Rule-Id` (when matched), `X-HookBox-Truncated` (MITM truncation).
- **Caps:** ingest body `MAX_INGEST_BODY_BYTES`→`413`; trace body truncation `MAX_BODY_BYTES`; template `TEMPLATE_MAX_SIZE`/`TEMPLATE_MAX_TAGS`; CRUD `CRUD_MAX_ITEMS`/`CRUD_MAX_ITEM_BYTES`; MITM `MITM_MAX_BODY_BYTES`/`MITM_TIMEOUT_S`/`MITM_MAX_REDIRECTS`.
- **Unknown/gone:** `404 unknown_endpoint` vs `410 endpoint_gone`; not traced.

[existing — verified at `app/interceptor/engine.py`, `cors.py`, `crud.py`, `proxy.py`, `conditions.py`, `planes.py`]

### 5.6 DB schema (SQLite — replaces SQLite **and** every Redis responsibility, spec §6)
- **Ported from `app/database.py` (verified):** `owners(owner_id PK, email UNIQUE, secret_hash, created_at, last_seen)`; `endpoints(token PK, owner_id FK, name, auto_crud, target_url, default_mode, latency_ms, rate_limit_per_min, chaos_pct, cors_enabled, created_at, last_hit, request_count)`; `mock_rules(id PK, token FK, name, priority, enabled, match_json, response_json, state_writes_json, latency_ms, rate_limit_per_min, webhook_json, created_at)`; `request_logs(id PK, token FK, method, path, status_code, served_by, matched_rule_id, duration_ms, overhead_ms, request_headers, query_params, request_body, response_headers, response_body, trace_json, state_snapshot, created_at)`.
- **New (replacing Redis, spec §6):** `endpoint_state(token, key, value, expires_at)` (PK `(token,key)`, 24h TTL); `crud_collections(token, name, items_json, updated_at)` (PK `(token,name)`, atomic CAS in one transaction, item/size caps).
- **Tombstone for `410`:** the Python path uses a Redis "gone" marker; the re-platform needs a SQLite-backed equivalent (e.g. a `gone_at` column or `endpoint_tombstones` table) — **flagged in §9 OQ-1** for the architect.
- WAL + `foreign_keys=ON`; applied via `migrations/` on startup (mirrors `../shortener-link`).

### 5.7 Sandboxed templating grammar — FROZEN
The closed tag set and "unknown → literal, never error" rule from AC-16/17/18 (`app/interceptor/templating.py`). No engine, no eval.

### 5.8 Tunnel control protocol — FROZEN
`/ws/tunnel/{slug}`; bind via `Authorization: Bearer` (or `?cap=`); JSON frames `→{t:"req",id,method,path,query,headers,body_b64}` / `←{t:"res",id,status,headers,body_b64}` / `←{t:"err",id?,message}` / `↔{t:"ping"|"pong"}`; close codes `4401` (unauthorized), `4409` (rebound); last-bind-wins takeover; `504 no_tunnel` on miss/timeout. [existing — verified at `app/routes/tunnel.py`]

---

## 6. Out of scope

See §2. Additionally for this PRD: choosing exact Rust crates (architect's call, with `../shortener-link/Cargo.toml` as the default palette), the visual design system (design-agent), and the full copy/voice surface incl. landing/marketing (copywriter-engineer).

## 7. Risks & assumptions

- **R1 — Subtle behavioral drift.** The value of this re-platform is *faithful* parity; a re-implementation that's "close" silently breaks consumers' mocks. **Mitigation:** §5 is lifted verbatim from verified code; the QA lane validates every AC against the documented status codes/headers/shapes; port the Python tests' intent into `cargo test`.
- **R2 — Concurrency model change.** Python's single-threaded asyncio guaranteed the tunnel registry / state writes were race-free "for free." Rust + multi-threaded tokio means the in-memory rate-limit buckets, tunnel registry, and broadcast fan-out need explicit `Send + Sync` synchronization (`DashMap`/`Mutex`). **Mitigation:** call it out for the architect; SQLite transactions cover CRUD atomicity (AC-26).
- **R3 — Loss of the Redis "fail-closed/open" semantics.** Several ACs today (state-gated rules fail closed, CRUD→503, limiter fails open) are *Redis-down* behaviors. With SQLite as the single store these mostly disappear; the limiter's "fails open on internal error" (AC-39) should be **preserved as a property of the in-memory bucket**, not silently dropped. **See §9 OQ-3.**
- **R4 — `chaos_mode` is read defensively today and is not in the frozen §5.3 schema.** The dropout variant (AC-40) works via `getattr(ep, "chaos_mode", "error")`. The re-platform should decide whether to promote `chaos_mode` to a first-class `endpoints`/`rules` field. **See §9 OQ-2.**
- **A1** The architect adopts `../shortener-link`'s crate/framework palette (Axum 0.7, sqlx-sqlite, reqwest+rustls, tokio, tracing) and React/Radix/Tailwind/CVA foundation unless it documents a better choice.
- **A2** "Faithful parity" means the externally observable contract (status, headers, shapes, order, caps) is preserved; internal structure is free to be idiomatic Rust.
- **A3** Default port is `8080` (spec/`../shortener-link`), vs the Python default `8000` (`config.py`) — the re-platform standardizes on `8080`.

## 8. Rollout

In-place re-platform of this repo (spec §4): the Rust crate + React SPA are built up alongside the retired Python tree, which is then removed from the working tree (kept in git history). Single binary; Docker optional and single-container. Delivery follows the spec §12 pipeline: this PRD → DESIGN agents (journey ∥ ux ∥ architect ∥ security) → design-agent → copywriter-engineer → REVISE (freeze §5) → approval → BREAKDOWN → FE ∥ BE lanes → QA loop → security review → sync.

## 9. Open Questions (must be empty before lock)

- **OQ-1 (Architect — `410 endpoint_gone` tombstone).** Today the "gone" marker lives in Redis (`redis_state.mark_gone/is_gone`, used by `delete_endpoint` and `_unknown_or_gone`). On the all-SQLite stack, how is a deleted token distinguished from a never-existed token to return `410` vs `404` (AC-57)? Options: a `gone_at` column / `endpoint_tombstones` table with its own TTL, or accept that a deleted token simply returns `404` (dropping the `410` distinction). **Architect to decide and freeze in §5.6.**
- **OQ-2 (Architect — promote `chaos_mode`?).** The opt-in connection-drop chaos (AC-40) relies on a `chaos_mode` field that is **not** in the current frozen §5.3 schema (read defensively in `conditions.py`). Should the re-platform make `chaos_mode("error"|"dropout")` a first-class column on `endpoints` (and/or per-rule), or keep the random-5xx-only behavior and treat dropout as out of parity? **Affects the §5.3 `EndpointConfigPatch`/`EndpointDetail` shape.**
- **OQ-3 (Architect/Security — degradation semantics after Redis removal).** Several current ACs are Redis-failure behaviors (state-gated rules fail **closed**, Auto-CRUD → `503`, rate limiter fails **open**). With SQLite as the single store, which of these still apply? Recommendation: keep "rate limiter fails open on internal error" as an in-memory-bucket property (AC-39); let state/CRUD become ordinary SQLite error paths. **Architect to confirm the new degradation contract; security to sign off.**
- **OQ-4 (Human/Product — default port).** Standardize on `8080` (spec, `../shortener-link`) vs preserve `8000` (current `config.py` default)? Assumed `8080` (A3) — confirm so deployment docs/compose are consistent.

---

## 10. Task graph (beads)

*Filled in BREAKDOWN mode after PRD approval. Will record the feature epic id and an `issue-id → AC-#` index here, with FE lane (`src/`, `public/`) and BE lane (`backend/`, `migrations/`, `Cargo.toml`) issues, a QA gate, a security-review gate, and a sync issue.*
