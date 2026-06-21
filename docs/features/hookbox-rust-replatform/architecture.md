# Architecture: HookBox — Rust/Axum + SQLite Re-platform

- **Slug:** `hookbox-rust-replatform`
- **Status:** AUTHORITATIVE — this file OWNS the frozen §5 interface contract. REVISE lifts §5 from here.
- **Author:** system-architect
- **Grounded in:** the full verified Python tree under `app/` + `config.py`, and the reference Rust project `../shortener-link/backend/` (Axum 0.7 / sqlx-sqlite / reqwest-rustls / tokio / tracing; `scripts/start.sh` shape).

Tagging convention: **[existing — verified at `path`]** = ported faithfully, byte-level external contract preserved; **[new]** = Rust/React artifact to create. Reference-mirror items are **[new — mirrors `../shortener-link/...`]**.

---

## Approach

HookBox becomes a single Rust binary (`hookbox`) on Axum 0.7 + a multi-threaded tokio runtime over one WAL-mode SQLite file (sqlx 0.8), serving a Vite/React SPA from `dist/` with SPA fallback. **All four Redis responsibilities collapse into in-process structures + SQLite** (spec §3, §6): the live-feed pub/sub becomes a `tokio::sync::broadcast` channel per process; per-endpoint state and Auto-CRUD become SQLite tables; the rate limiter becomes an in-memory `DashMap` token bucket; retention becomes a `tokio::time::interval` sweep plus a write-time prune trigger. A second bin (`tunnel`) is the reverse-tunnel CLI; a third bin (`seed`) plants demo data on first run.

The central design constraint is **faithful external parity**: the resolution order, status codes, `X-HookBox-*` headers, caps, error envelope `{error, detail}`, and the management-API JSON shapes are reproduced exactly from the verified Python (`app/models.py` is annotated there as "the FROZEN §5.3 interface contract"). Internal structure is idiomatic Rust. The single hard architectural guarantee carried over verbatim is the **three-plane host+path dispatch** (`app/planes.py::resolve_plane`): a tower middleware classifies every request into P1 (mock catch-all) / P2 (`/api/**`) / P3 (UI + WS/SSE + health) purely from `Host` + path, so the mock catch-all can never shadow the management API or dashboard. The interceptor fast path reads a compiled-endpoint cache (no per-request DB read on a match) and fires the trace write + feed publish off the response path, preserving the Python `<10ms` budget property. Note: unlike the shortener (single flat router, host-agnostic), HookBox **must** keep host-aware dispatch — that is the load-bearing difference from the reference.

Key decisions resolved below (§ Open-question resolutions): **OQ-1** = a `gone_at` tombstone column on a retained-row mechanism backs `410` vs `404`; **OQ-2** = `chaos_mode` is promoted to a first-class `endpoints` column (and per-rule override) and joins the frozen §5.3 schema; **OQ-4** = serve port standardizes on **8080**; **OQ-3** = the in-memory limiter keeps its fail-open property, state/CRUD become ordinary SQLite error paths (architectural view; security signs off).

---

## §5 FROZEN interface contract (AUTHORITATIVE)

> **FROZEN.** FE (`src/`) and BE (`backend/`) implement against exactly these shapes, status codes, headers, and orders. Neither lane may change a shape without a §5 amendment here. All `/api/**` bodies are JSON; every error body is the **flat** envelope `{"error": "<code>", "detail": "<human>"}` (NOT the shortener's nested `{error:{code,message}}` — HookBox keeps its own flat shape per AC-60). All timestamps are RFC3339 UTC strings (e.g. `2026-06-21T12:00:00Z` / `...+00:00`). [existing — verified at `app/routes/api.py`, `app/models.py`]

### 5.1 Auth (P2 management plane)

- Every `/api/**` route **except** `POST /api/session` requires `Authorization: Bearer <owner_secret>`.
- Missing / malformed / unknown secret → `401 {"error":"unauthorized","detail":...}` with header `WWW-Authenticate: Bearer`. [existing — `app/auth.py::require_owner`]
- Valid secret that does **not** own the addressed `{token}`/resource → `404 {"error":"not_found","detail":...}` (never `403` — a non-owner cannot distinguish "exists but not mine" from "does not exist"). [existing — `app/auth.py::assert_owns_endpoint`]
- The public `owner_id` is **never** a credential — only `sha256(secret)` is looked up. [existing — `app/auth.py`]
- Feed (WS/SSE) subscribe auth: `?cap=<owner_secret>` query param. Tunnel bind auth: `Authorization: Bearer <owner_secret>` header (with `?cap=` fallback). Both verified **before any frame**. [existing — `app/websocket.py`, `app/routes/tunnel.py`]

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
                       chaos_mode?: "error"|"dropout",                   // [new] OQ-2 — see resolution
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

[existing — verified verbatim at `app/models.py`; `chaos_mode` is the only **[new]** addition, see OQ-2 resolution]

### 5.4 WebSocket + SSE live feed (P3)

- **WS:** `GET /ws/{token}?cap=<owner_secret>` (Upgrade). **SSE:** `GET /sse/{token}?cap=<owner_secret>` (`text/event-stream`).
- **Auth before any frame:** verify cap owns token. On failure → WS **accept-then-close `4401`** (so the close code reaches the client, not a 1006); SSE → `401 {"error":"unauthorized",...}`. [existing — verified at `app/websocket.py` (the accept-then-close rationale is load-bearing)]
- **Connection cap** per endpoint = `WS_MAX_CONN_PER_ENDPOINT` (50). Over cap → WS accept-then-close `1013`; SSE → `503 {"error":"too_many_connections",...}`. [existing]
- **Channel isolation:** a client authed for `tokenA` receives only `tokenA` events. [existing]
- **Server → client events** (JSON; WS as a JSON text frame, SSE as `event: <type>\ndata: <json>\n\n`):
  - `hello` `{ "token": string, "server_time": datetime }` — first frame on connect.
  - `new_request` `{ ...RequestSummary... }` — one per served mock request.
  - `state_changed` `{ "token": string, "key": string, "value": string }`.
  - `endpoint_updated` `{ "token": string, "fields": string[] }`.
  - **Wire envelope:** WS frames are `{"type": "<event>", "data": {<payload>}}`. SSE frames put `<event>` in the `event:` line and `<payload>` (the inner `data` object) in the `data:` line. [existing — verified at `app/websocket.py::broadcast` / `_sse_format`, `app/interceptor/engine.py::_persist_and_publish`]
- **Client → server:** WS text `"ping"` → server replies text `"pong"`. SSE: server emits a `: ping` comment heartbeat every ~25s (`_SSE_HEARTBEAT_S`). [existing]
- A dead/slow per-client send is bounded by `WS_SEND_TIMEOUT_S` (5s) and the client is dropped, never stalling fan-out. [existing]

### 5.5 Mock plane (P1) behavior — FROZEN resolution & wrapping

- **Resolution order** (first that produces a response wins):
  `OPTIONS preflight → matching rule (priority,id) → Auto-CRUD → tunnel → MITM → default`. [existing — `app/interceptor/engine.py::handle_mock` / `_resolve_unmatched`]
- **Conditions wrap** (applied around the resolved body, in this exact order):
  `rate-limit (429) → chaos (5xx / connection-drop) → latency (sleep)`. Rate-limit and chaos short-circuit (return before latency); applied latency is recorded so `overhead_ms = duration − applied_latency`. [existing]
- **Default mode:** `mock_404` → `404 {"error":"no_match","detail":...}`; `echo` → `200 {"method","path","query","headers","body"}` (headers are the lower-cased request headers). [existing — `app/interceptor/engine.py::_echo_response`]
- **Identifying headers** on every P1 response: `X-HookBox-Endpoint: <token>`, `X-HookBox-Served-By: <rule|crud|mitm|tunnel|default|cors|chaos|ratelimit>`, `X-HookBox-Rule-Id: <id>` (only when a rule matched), `X-HookBox-Truncated: true` (MITM body truncation only). [existing — `app/interceptor/engine.py::_identified`, `app/interceptor/proxy.py`]
- **Auto-CORS** (P1 only, when `cors_enabled`): every non-preflight response also carries `Access-Control-Allow-Origin` (reflected `Origin`, else `*`), `Access-Control-Expose-Headers: *`, `Vary: Origin`. `OPTIONS` preflight → `204` with reflected Origin, reflected `Access-Control-Request-Headers` (else `*`), `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`, `Access-Control-Max-Age: 600`, `Vary: Origin`. `Access-Control-Allow-Credentials` is **never** emitted. When `cors_enabled=false` the per-response CORS set is empty but `OPTIONS` is still answered with a deterministic `204` (never falls through). The management plane (P2) emits **no** wildcard CORS. [existing — `app/interceptor/cors.py`]
- **Auto-CRUD lifecycle** (when `auto_crud` and no rule matched and path is a valid 1- or 2-segment CRUD path with safe segments): `POST /<coll>`→`201` (server uuid `id`), `GET /<coll>`→`200` (array), `GET /<coll>/<id>`→`200`|`404`, `PUT /<coll>/<id>`→`200`|`404`, `PATCH /<coll>/<id>`→`200`|`404`, `DELETE /<coll>/<id>`→`204`|`404`, `HEAD` mirrors `GET`. Write body must be a JSON **object** (else `400 {"error":"bad_request",...}`); `id` is server-assigned/immutable; caps `CRUD_MAX_ITEMS` (1000) and `CRUD_MAX_ITEM_BYTES` (64 KB) → `400`. A 3+-segment or unsafe-segment path is **not CRUD** → falls through to tunnel/MITM/default. [existing — `app/interceptor/crud.py`]
- **MITM** (target_url set, nothing else handled): forward, return upstream status/safe-headers/capped-body labeled `mitm`; SSRF-blocked / connection / DNS error → `502 {"error":"upstream_unreachable",...}`; timeout → `504 {"error":"upstream_timeout",...}`; body capped at `MITM_MAX_BODY_BYTES` (5 MB) with `X-HookBox-Truncated: true`. [existing — `app/interceptor/proxy.py`]
- **Tunnel** (CLI bound): forward down the socket, replay response labeled `tunnel`; no tunnel / drop / timeout → `504 {"error":"no_tunnel",...}`. [existing — `app/routes/tunnel.py::forward_to_tunnel`]
- **Caps:** ingest body > `MAX_INGEST_BODY_BYTES` (1 MB) → `413 {"error":"payload_too_large",...}` before buffering; trace request/response bodies truncated at `MAX_BODY_BYTES` (256 KB); template `TEMPLATE_MAX_SIZE` (256 KB) / `TEMPLATE_MAX_TAGS` (500). [existing — `app/interceptor/engine.py::_read_body_capped`, `app/database.py`]
- **Unknown / gone token:** unknown → `404 {"error":"unknown_endpoint",...}`; deleted/expired → `410 {"error":"endpoint_gone",...}`; both carry `X-HookBox-Endpoint` and **neither is logged as a trace**. (See OQ-1 resolution for the SQLite tombstone backing the `410`.) [existing — `app/interceptor/engine.py::_unknown_or_gone`]
- **Rate-limit 429** body: `{"error":"rate_limited","detail":...}` + headers `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`. **Chaos 5xx** body: `{"error":"chaos","detail":...}`. Chaos `dropout` closes the connection (no body), bounded by `CHAOS_DROP_TIMEOUT_S`. [existing — `app/interceptor/engine.py`, `conditions.py`]

### 5.6 SQLite schema — FROZEN (DDL replaces both the old SQLite tables **and** all four Redis responsibilities)

Applied as numbered migrations under `backend/migrations/` on startup (`sqlx::migrate!`), WAL + `foreign_keys=ON` + `busy_timeout` (mirrors `../shortener-link/backend/src/db.rs`). Timestamps are TEXT RFC3339 (`datetime('now')` default). [existing tables verified at `app/database.py::_DDL`; the three Redis-replacement tables are **[new]**]

```sql
-- migrations/0001_init.sql  --------------------------------------------------

CREATE TABLE owners (                                      -- [existing]
    owner_id    TEXT PRIMARY KEY,                          -- sha256(lower(trim(email)))[:16], non-secret
    email       TEXT UNIQUE NOT NULL,
    secret_hash TEXT NOT NULL,                             -- sha256(owner_secret); rotates each /api/session
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT
);

CREATE TABLE endpoints (                                   -- [existing] + chaos_mode [new]
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
- **OQ-1 tombstone:** `endpoints.gone_at` (see resolution). A deleted endpoint is **not** hard-deleted; it is tombstoned (`gone_at = datetime('now')`, rows reset to a soft-gone state) so the mock plane returns `410`. Tombstones are reaped by the sweep after a TTL (`GONE_TTL_HOURS`, default 168h / 7d) — after which the token resolves to `404`.

### 5.7 Sandboxed templating grammar — FROZEN

Hand-written single-pass scanner; closed tag set; unknown/malformed → left **literal**; never errors the mock path; no eval / no general template engine / no format-string over user text. Tags (FROZEN): `{{now 'iso'|'unix'|'epoch_ms'}}`, `{{random 'uuid'}}`, `{{random 'int' <lo> <hi>}}`, `{{random 'hex' <len>}}`, `{{request.method|path|body}}`, `{{request.query.<k>}}`, `{{request.path.<name>}}`, `{{request.header.<name>}}`, `{{request.body.<jsonpath>}}`, `{{state.<k>}}`. DoS bounds: template > `TEMPLATE_MAX_SIZE` returned unrendered; at most `TEMPLATE_MAX_TAGS` substitutions. [existing — `app/interceptor/templating.py`]

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

## Data model & storage

- **One WAL SQLite file** at `DATABASE_PATH` (default `data/app.db`), opened via a sqlx `SqlitePool` (`max_connections` ~8) with `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=true`, `busy_timeout=5s` — mirrors `../shortener-link/backend/src/db.rs::pool`. [new — mirrors reference]
- **Two access patterns** (ported from `app/database.py`):
  1. **Management routes (P2):** use the shared pool directly (`sqlx::query_as` against `owners/endpoints/mock_rules/request_logs/endpoint_state/crud_collections`).
  2. **Interceptor fast path (P1):** reads **no row on a match** — the endpoint config + compiled rules come from the in-process `RuleCache` (cold-loads once from SQLite, invalidated on any management write). The trace write runs off the response path on a spawned task via the shared pool.
- **`endpoint_state` (replaces `state:<token>`):** read = `SELECT key,value FROM endpoint_state WHERE token=? AND expires_at > now`; lazy — only read when `any_rule_gates_on_state`. Write = upsert with `expires_at = now + 24h`. Key charset `^[A-Za-z0-9_-]{1,64}$` enforced before write; unsafe key skipped. [existing semantics — `redis_state.get/set/clear_state`]
- **`crud_collections` (replaces `crud:<token>:<coll>`):** the JSON-array-list with **atomic read-modify-write inside one SQLite transaction** — `BEGIN IMMEDIATE; SELECT items_json …; <mutate in Rust>; UPDATE … SET items_json=?,updated_at=now,expires_at=now+24h; COMMIT`. `busy_timeout` + `BEGIN IMMEDIATE` serialize concurrent writers to the same collection, replacing the Redis `WATCH/MULTI` CAS and guaranteeing no lost update (AC-26). Server-assigned uuid `id` so two POSTs never collide. Item/byte caps enforced in-transaction. [existing semantics — `redis_state.crud_cas`]
- **Rate-limit buckets:** in-memory `DashMap<String, Bucket>` keyed `rl:<token>` / `rl:<token>:<rule_id>`; the token-bucket math is ported from the reference `ratelimit.rs::consume` (proportional refill; `retry_after = ceil((deficit/refill)*(window/1000))`). `limit<=0` ⇒ unlimited. **Fails open** on any internal error (returns allowed). [existing semantics — `redis_state.rate_limit_check` + reference `ratelimit.rs`]
- **Retention sweep:** a `tokio::time::interval(RETENTION_SWEEP_SECONDS=300)` task (mirrors `../shortener-link/backend/src/tasks/sweep.rs`) that (a) prunes `request_logs` beyond the newest `TRACE_CAP`(100) per token and past the 24h `TRACE_TTL_HOURS`, (b) deletes expired `endpoint_state` / `crud_collections` rows, (c) reaps endpoint tombstones older than `GONE_TTL_HOURS`. The 100-cap is **also** held at write time (a prune query after each trace insert, ported from `app/database.py::_PRUNE_SQL`) so it never drifts between sweeps. [existing semantics — `app/database.py::TraceWriter`, `config.py`]

## Component & file design

```
backend/
  Cargo.toml                # [[bin]] hookbox / tunnel / seed; deps mirror ../shortener-link
  migrations/
    0001_init.sql           # [new] the full §5.6 schema (incl. gone_at, chaos_mode, 2 Redis-replacement tables)
  src/
    main.rs                 # hookbox bin: load Config, open pool, migrate, build broadcast hub +
                            #   limiter + tunnel registry + rule cache, spawn sweep task, axum::serve :8080
    lib.rs                  # module tree + build_app(state) for tests (mirrors reference lib.rs)
    config.rs               # [new] env-driven Config (mirrors config.py + reference config.rs); PATH_FALLBACK_ONLY derived
    state.rs                # [new] AppState { pool, cfg, rule_cache, limiter, feed_hub, tunnels } (Clone, Arc)
    error.rs                # [new] ApiError -> FLAT {error,detail} envelope (NOT the reference's nested shape)
    db.rs                   # [new] pool() + migrate() (mirrors reference db.rs)
    ids.rs                  # [new] gen_token (ambiguity-stripped), gen_owner_secret (256-bit), hash_email, hash_secret
    auth.rs                 # [new] require_owner extractor (Bearer→owner_id), assert_owns_endpoint (→404),
                            #   verify_cap_owns_token (feed + tunnel gates)
    planes.rs               # [new] resolve_plane(host,path)->PlaneResult (PORT of app/planes.py, 1:1 semantics)
    router.rs               # [new] top-level dispatch: a from_fn middleware classifies plane and routes
                            #   P1→interceptor, P2→/api router, P3→SPA+feed+health; mock catch-all can't shadow P2/P3
    routes/
      mod.rs                # api_router() merge + build_app
      api.rs                # [new] the 18 /api routes (PORT of app/routes/api.py)
      health.rs             # [new] /healthz
      feed.rs               # [new] /ws/{token} + /sse/{token} owner-gated subscribe (PORT of app/websocket.py)
      tunnel_ws.rs          # [new] /ws/tunnel/{slug} server side (PORT of app/routes/tunnel.py)
      spa.rs                # [new] ServeDir(dist) + index.html SPA fallback (mirrors reference spa.rs)
    interceptor/
      mod.rs
      engine.rs             # [new] handle_mock: the §5.5 resolution + conditions wrap (PORT of engine.py)
      matcher.rs            # [new] compile_path + select() priority,id (PORT of matcher.py)
      templating.rs         # [new] single-pass sandboxed scanner (PORT of templating.py)
      crud.rs               # [new] Auto-CRUD over crud_collections w/ in-txn CAS (PORT of crud.py + redis_state.crud_cas)
      proxy.rs              # [new] MITM via reqwest+rustls; SSRF on resolved IP + IP pinning (PORT of proxy.py)
      cors.rs               # [new] auto-CORS header sets (PORT of cors.py)
      conditions.rs         # [new] clamp + apply_latency + chaos roll + rate-limit glue (PORT of conditions.py)
    state_store.rs          # [new] endpoint_state read/write/clear (replaces redis_state state ops)
    crud_store.rs           # [new] crud_collections txn CAS (replaces redis_state crud ops)
    rule_cache.rs           # [new] CompiledEndpoint cache + invalidate (PORT of rule_cache.py)
    feed.rs                 # [new] FeedHub: broadcast::Sender per process; subscribe/publish; conn-cap accounting
    limiter.rs              # [new] DashMap token bucket, fail-open (PORT of redis_state.rate_limit + reference math)
    ssrf.rs                 # [new] is_blocked_ip + resolve_and_check (PORT of proxy.py guard; reference ssrf.rs classifiers)
    helpers.rs              # [new] is_safe_key, jsonpath_lite, strip_forward_headers, redact (PORT of utils/helpers.py)
    tasks/
      mod.rs
      sweep.rs              # [new] retention interval task (mirrors reference tasks/sweep.rs)
    seed.rs                 # [new] demo-data seeding
    bin/
      tunnel.rs             # [new] the tunnel CLI bin (--port --endpoint --secret), tokio-tungstenite, backoff reconnect
      seed.rs               # [new] seed bin entry
  tests/
    api.rs                  # [new] integration tests (mirrors reference tests/api.rs) covering §5 surfaces

src/                        # [new] Vite + React + TS SPA (frontend lane owns)
public/  dist/  data/app.db
scripts/start.sh            # [new] pnpm build → cargo build --release → migrate (on boot) → seed → serve :8080
```

**FeedHub design (the broadcast ⇄ management-API reconciliation):** the hub holds, per token, a `tokio::sync::broadcast::Sender<FeedEvent>` (created lazily on first subscribe) and an atomic subscriber count for the connection cap. The engine, after persisting a trace, calls `hub.publish(token, FeedEvent::NewRequest(summary))` — non-blocking; if there are zero receivers it is a no-op (a dead feed never affects the mock path). Each WS/SSE subscriber holds a `broadcast::Receiver`. On a lagged receiver (`RecvError::Lagged`), the subscriber drops the missed frames and continues — it does **not** error the socket; the client **reconciles** by re-fetching authoritative state from the management API (`GET …/requests`, `GET …/state`, `GET …/{token}`). This is the explicit contract: **the broadcast channel is best-effort/at-most-once for liveness; the management API is the source of truth.** The `hello` frame on connect tells the client to do an initial reconcile fetch.

## Sequences

**A. Served mock request (P1, rule match)**
1. tower middleware: `resolve_plane(host, path)` → `mock { token, mock_path }`.
2. `engine::handle_mock`: `rule_cache.get(token)` (in-mem; cold-load from SQLite once) → `CompiledEndpoint` or `unknown_or_gone` (404/410, see OQ-1).
3. Ingest-cap the body (`413` if over `MAX_INGEST_BODY_BYTES`).
4. If `OPTIONS` → CORS preflight `204`, spawn trace, return.
5. If `any_rule_gates_on_state` → read `endpoint_state` (SQLite, expiry-filtered).
6. `matcher::select` (priority,id). On match: render `state_writes` (write to `endpoint_state` first), render `body_template`, build response.
7. Conditions wrap: `limiter.check(eff_rate)` → 429? ; else chaos roll → 5xx/DROP? ; else `apply_latency`.
8. Attach `X-HookBox-*` + CORS headers.
9. `tokio::spawn` trace write (insert + prune to `TRACE_CAP`) then `feed_hub.publish(NewRequest)`. Response returns without awaiting the spawn (preserves `<10ms`).

**B. Auto-CRUD write (no rule match)** — engine `_resolve_unmatched` → `crud_store` opens `BEGIN IMMEDIATE`, reads `items_json`, mutates in Rust (append uuid / replace / merge / delete), enforces caps, `UPDATE … COMMIT`. `busy_timeout` serializes concurrent writers → no lost update.

**C. Feed subscribe (WS)** — `/ws/{token}?cap=` → `auth::verify_cap_owns_token` (deny → accept+close `4401`); cap check → conn-cap (`1013`); `accept()`; send `hello`; `feed_hub.subscribe(token)`; loop `select!` over the broadcast receiver (forward frames) and inbound `"ping"` (reply `"pong"`); on lag, drop+continue (client reconciles via API).

**D. Tunnel forward** — public P1 request, no rule/CRUD, `tunnels.is_active(token)` → `forward_to_tunnel`: alloc id, send `{"t":"req",...}` down the bound socket, await the oneshot for the matching `{"t":"res",...}` with `TUNNEL_REQUEST_TIMEOUT_S`; timeout/drop → `504 no_tunnel`.

**E. Session create** — `POST /api/session`: per-IP limiter; upsert owner rotating `secret_hash`; if owner has no endpoints, provision one; return `SessionResponse` (same shape new vs existing).

## FE / BE work split

The §5 contract is the only interface between lanes; each builds independently against it.

- **BE lane (`backend/`, `migrations/`, `Cargo.toml`)** owns: the migration; all 18 `/api` routes + `/healthz`; the plane router + interceptor pipeline (engine/matcher/templating/crud/proxy/cors/conditions); state/crud SQLite stores; rule cache; limiter; FeedHub + WS/SSE endpoints; tunnel WS server + the `tunnel` CLI bin; SSRF guard; retention sweep; the `seed` bin; `scripts/start.sh`; `cargo test` integration suite. BE produces the exact JSON shapes and headers in §5.2–§5.8.
- **FE lane (`src/`, `public/`, Vite config)** owns: the React SPA — landing/email gate, split-screen dashboard (live feed + deep inspector: Headers · Query · Body · Response Served · State & Tracing), the 5-tab rule builder (Matching · Response · Templating · Actions · Throttling), endpoint settings. FE codes against §5.3 types (zod schemas mirroring the structs), the §5.2 routes, and the §5.4 feed events. The WS hook subscribes to `/ws/{token}?cap=`, renders `new_request` live, and **reconciles** authoritative lists/state via the §5.2 GET routes (per the FeedHub at-most-once contract), with SSE fallback to `/sse/{token}`. FE consumes `design.md` + `copy.md` for visual/voice.
- **Independence guarantee:** because §5 fixes every shape, status, header, event payload, and close code, FE can build against a mock server (or `cargo test` fixtures) and BE against the documented event/route shapes without either changing the other's surface. The two **[new]** fields (`chaos_mode` on `EndpointConfigPatch`/`EndpointDetail`/`MockRule*`) are additive and defaulted, so an FE that ignores them still works.

## Technical risks

- **R1 — Behavioral drift.** The whole value is faithful parity; "close" silently breaks consumers' mocks. Mitigation: §5 lifted verbatim from verified code; `cargo test` asserts status/headers/shapes/order/caps; port the Python tests' intent.
- **R2 — Concurrency model change.** Python's single-threaded asyncio made the tunnel registry / state writes race-free for free. Multi-threaded tokio needs explicit `Send + Sync`: `DashMap` for buckets, `Mutex`/`DashMap` for the tunnel registry, `BEGIN IMMEDIATE` + `busy_timeout` for CRUD atomicity. The per-request-id oneshot map in `TunnelConnection` must be `Mutex`-guarded (was implicitly serial in Python).
- **R3 — CRUD CAS under SQLite.** `BEGIN IMMEDIATE` takes a write lock; high write concurrency to one collection serializes (acceptable — bounded by `busy_timeout`, retried). This is stricter (and simpler) than Redis WATCH/MULTI and cannot lose updates.
- **R4 — Broadcast lag / slow clients.** A slow WS client must never stall fan-out. Mitigation: bounded broadcast channel; on `Lagged` the subscriber drops frames (client reconciles via API), and per-send is time-bounded (`WS_SEND_TIMEOUT_S`); conn-cap (`1013`) bounds memory.
- **R5 — DNS-rebinding TOCTOU on MITM.** Must resolve to IPs, block any private/loopback/link-local/metadata IP, then **pin the connection to the validated IP** while preserving Host header + TLS SNI. `reqwest` needs explicit IP-pinning (custom resolver / `resolve()` override) — do not rely on a second resolution at connect. Re-validate every redirect hop (default: redirects off).
- **R6 — SPA fallback vs plane isolation.** The P3 `ServeDir` fallback must be reached **only** for app-host non-`/api` non-mock paths; the plane middleware must run first so a mock-host request never hits the SPA fallback (and `/api` on a mock host stays P1, per `planes.py`).
- **R7 — Restart loses in-memory state.** Buckets + tunnel registry + broadcast subscribers are per-process and reset on restart (accepted, single-instance). Durable state (endpoints/rules/state/crud/traces) survives in SQLite.

---

## Open-question resolutions (recorded, frozen into §5)

- **OQ-1 — `410 endpoint_gone` vs `404` (RESOLVED: keep the distinction via a SQLite tombstone column).** The Redis "gone" marker is replaced by a **`gone_at TEXT` column on `endpoints`**. `DELETE /api/endpoints/{token}` does **not** hard-delete; it sets `gone_at = datetime('now')` and clears the endpoint's live config (rules/state/crud cascade or are emptied), then invalidates the rule cache. On P1, `_unknown_or_gone` resolves: row absent → `404 unknown_endpoint`; row present with non-null `gone_at` → `410 endpoint_gone`. The retention sweep hard-deletes tombstoned rows older than `GONE_TTL_HOURS` (default 168h / 7d), after which the token degrades to `404`. Rationale: a column on the already-loaded endpoint row costs nothing on the hot path (the rule cache can cache the gone state), needs no second store, and preserves AC-57 exactly. (Note: this changes #6 `DELETE` to a tombstone-update rather than a row delete; the API response `200 Message` is unchanged.)
- **OQ-2 — promote `chaos_mode` (RESOLVED: yes, first-class).** `chaos_mode: "error"|"dropout"` becomes a real column on `endpoints` (`NOT NULL DEFAULT 'error'`) and a nullable per-rule override (`mock_rules.chaos_mode`, null ⇒ inherit). It joins the frozen §5.3 schema in `EndpointConfigPatch`, `EndpointDetail`, `MockRuleCreate/Patch/MockRule`. Rationale: the Python read it defensively via `getattr(ep,"chaos_mode","error")` precisely because it lacked a home; the re-platform gives it one so the dropout variant (AC-40) is configurable and the contract is explicit rather than implicit. Default `"error"` keeps the random-5xx behavior identical for anyone who never sets it (parity-safe, additive).
- **OQ-3 — post-Redis degradation contract (architectural view; security signs off).** State and CRUD are now the single SQLite store, so the old Redis-down branches collapse to **ordinary SQLite error handling** (a DB error → `500`/`503` is an exceptional fault, not a routine degradation mode). The one property that must be **explicitly preserved** is the rate limiter's **fail-open on internal error** (AC-39): the in-memory `DashMap` bucket returns `allowed` if its own logic errors, so a limiter bug never wedges the mock path (it stays bounded by the ingest body cap). State-gated rules still **fail closed** in the sense that an unreadable/empty state means a gated rule simply does not match (same observable outcome as Python's fail-closed, now without a Redis-down trigger). Auto-CRUD no longer has a "store down → 503" mode under normal operation; a genuine SQLite failure surfaces as `503 {"error":"store_unavailable",...}` to keep the error envelope stable. Security-engineer to confirm.
- **OQ-4 — default port (RESOLVED: 8080).** Standardize on `8080` per the spec and `../shortener-link` (`scripts/start.sh` serves `:8080`), superseding the Python default `8000` (`config.py::PORT`). `APP_PORT`/`PUBLIC_PORT` env override retained.
```
