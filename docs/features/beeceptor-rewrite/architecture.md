# Architecture: HookBox — Beeceptor-class API Mocking & Interception Platform (slug: beeceptor-rewrite)

> **Authority:** This document owns the **authoritative §5 frozen interface contract** (this file's §5). The PRD's §5 is provisional; REVISE lifts §5 from here verbatim. FE (`templates/` + `static/`) and BE (`app/`, `config.py`, `requirements.txt`, Docker, tunnel) build **only** against §5 below and never need each other to change shape.
>
> **Locked stack (do not substitute — `_decisions.md` §1):** Python 3.12+ async · FastAPI on `uvicorn[standard]` · `aiosqlite` (SQLite WAL) · **Redis** (state KV + pub/sub + rate-limit token buckets) · `httpx` (MITM forward) · server-rendered **Jinja2 + HTMX + Alpine.js + Tailwind**. **No React/JSX/Vite/Node build.** The spec's `useRequestStream` React hook is delivered as an Alpine store + `static/js/request-stream.js` with identical responsibilities (open pipe, exponential-backoff reconnect, dedupe, non-blocking feed).
>
> **Grounding tags:** `[existing — verified at <path>]` = read and confirmed in the current tree. `[new]` = does not exist yet. Nothing is tagged existing unless I verified it in this session.

---

## 1. Approach

HookBox becomes a **single async FastAPI app** that serves **three hard-isolated request planes** decided by a single ASGI-level dispatch step: (P1) the **wildcard mock surface** (`*.<MOCK_DOMAIN>` via Host header, plus the localhost path-fallback `/e/<token>/...`), (P2) the **management API** (`/api/*`), and (P3) the **dashboard UI + static** (`/`, `/d/<token>`, `/static/*`). Plane selection happens **first**, in a `PlaneDispatchMiddleware` that inspects `Host` + path and tags `request.state.plane`; the mock interceptor is a single catch-all route mounted **last** so it can never shadow `/api` or the UI (FastAPI matches explicit routes before the catch-all, and the middleware hard-routes mock-host traffic away from the API/UI routers regardless). This replaces the current `/hook/{user_id}/{endpoint_id}` shape `[existing — verified at app/main.py:41]` and the leaked crypto `/status` route `[existing — verified at app/main.py:123-268]`, which are deleted.

The **interceptor engine** (P1) is a deterministic pipeline tuned for a **<10ms fast path**: resolve the endpoint config + ordered rules from a **hot in-process cache** (warmed from SQLite, invalidated via a Redis pub/sub `cfg:<token>` signal on any management write) → CORS preflight short-circuit → match (method/path/header/query/body/state) → **sandboxed templating** (a hand-written tag scanner — *no `eval`, no Jinja in the response body path*) → state mutation in Redis → if unmatched and a target is set, **MITM-forward via `httpx`** → apply latency/chaos/rate-limit → build response → **fire-and-forget** trace write to SQLite **and** publish to Redis pub/sub (`await` neither on the response path). The durable store keeps `endpoints`, `mock_rules`, `request_logs`; Redis holds ephemeral **per-endpoint state**, **Auto-CRUD collections**, **rate-limit token buckets**, and the **pub/sub fan-out** that powers the real-time dashboard. Key decisions: (a) **owner-capability auth** replaces the insecure `X-User-Id` header trust `[existing — verified at app/routes/api.py:24]` — a non-secret `owner_id` (= today's `hash_email`) plus a **secret bearer `owner_secret`** validated server-side per request; (b) **Tailwind via Play CDN** (zero build step, honoring "no Node") with the option of a committed standalone CLI build documented but not required; (c) **WebSocket primary, SSE fallback** for the live feed, both fed from the same Redis channel; (d) the **rule cache is in-process** (a dict keyed by token) for the <10ms budget, with Redis pub/sub as the cross-process invalidation bus so a future second app instance stays correct.

---

## 2. Component & file design

Legend: **REPLACE** = file exists, rewrite contents; **NEW** = create; **DELETE** = remove.

### Backend lane (`app/`, `config.py`, `requirements.txt`)

| File | Action | Responsibility |
| --- | --- | --- |
| `config.py` | REPLACE `[existing — verified at config.py]` | All env-driven config: `MOCK_DOMAIN`, `APP_HOST/PORT`, `REDIS_URL`, `DATABASE_PATH`, retention (`TRACE_CAP=100`, `TRACE_TTL_HOURS=24`, `RETENTION_SWEEP_SECONDS=300`), bounds (`LATENCY_MAX_MS=10000`, `RULE_CACHE_TTL`), MITM policy (`MITM_TIMEOUT_S`, `MITM_MAX_BODY_BYTES`, `MITM_ALLOW_PRIVATE=false`), token entropy lengths. Reads `os.getenv` with safe defaults. |
| `requirements.txt` | REPLACE `[existing — verified at requirements.txt]` | Pin: `fastapi`, `uvicorn[standard]`, `aiosqlite`, `pydantic>=2`, `python-multipart`, **add** `redis>=5` (async `redis.asyncio`), `httpx`, `jinja2`, `itsdangerous` (signed secret optional), `websockets` (tunnel CLI client), `typer` or `argparse` (CLI). |
| `app/main.py` | REPLACE `[existing — verified at app/main.py]` | App factory, lifespan (`init_db`, open Redis pool, start pub/sub relay, start retention task, warm rule cache), mount `/static`, register `PlaneDispatchMiddleware`, include `api_router` + `ui_router`, register the WS routes (`/ws/{token}`, `/ws/tunnel/{token}`) and the **mock catch-all** (mounted last). **Delete** `/hook/...`, `/status` crypto, `/login`, `/register`, `/backup` routes. |
| `app/middleware.py` | NEW | `PlaneDispatchMiddleware`: compute plane from `Host`+path, set `request.state.plane`, `request.state.token`; for mock-host requests targeting `/api`/UI it stays in P1 (mock), for app-host `/api`,`/`,`/d`,`/static` it stays P2/P3. Cheap, no DB. |
| `app/planes.py` | NEW | Pure plane-resolution logic (host→token extraction, path-fallback parse `/e/<token>/<rest>`), unit-testable without ASGI. |
| `app/interceptor/engine.py` | NEW | The core ordered pipeline (`handle_mock(request) -> Response`). Orchestrates match → template → state → CRUD → MITM → conditions → trace/publish. Owns the resolution order. |
| `app/interceptor/matcher.py` | NEW | Rule selection: method/path (exact + `:param` + wildcard), header/query/body (jsonpath-lite) conditions, **state requirements**. Returns the first enabled matching rule (by `priority`, then `id`). |
| `app/interceptor/templating.py` | NEW | **Sandboxed** tag engine (§5.7). Hand-written `{{ ... }}` scanner; no code eval. Resolves `now/random/request.*/state.*/uuid/...`. Fail-safe on unknown tags. |
| `app/interceptor/crud.py` | NEW | Auto-CRUD over a Redis-backed JSON array per `<token>:<collection>`. Parses `/<collection>[/<id>]`, executes POST/GET/PUT/PATCH/DELETE. |
| `app/interceptor/proxy.py` | NEW | MITM forward via shared `httpx.AsyncClient`: SSRF guard, hop-by-hop stripping (reuse `format_headers` `[existing — verified at app/utils/helpers.py:18]`), size/timeout caps, capture response for trace. |
| `app/interceptor/conditions.py` | NEW | Latency (`asyncio.sleep`), chaos (seeded RNG → 502/503/504/dropout), rate-limit check (delegates to `app/redis_state.py` token bucket). |
| `app/interceptor/cors.py` | NEW | Auto-CORS: build preflight `OPTIONS` response + the header set injected on every response (§5.6). |
| `app/redis_state.py` | NEW | Async Redis facade: state KV (`state:<token>` hash), CRUD store (`crud:<token>:<coll>` list), token bucket (`rl:<token>` / `rl:<token>:<rule_id>`), pub/sub publish, key TTLs. Single shared connection pool. |
| `app/pubsub.py` | NEW | Subscribe to `trace:<token>` + `cfg:*`; relay trace events to locally-connected WS/SSE clients; on `cfg:<token>` invalidate the rule cache. Replaces in-process-only fan-out. |
| `app/rule_cache.py` | NEW | In-process `dict[token] -> CompiledEndpoint` (config + compiled rule list). `get(token)` loads from SQLite on miss; `invalidate(token)` on pub/sub `cfg`. Powers the <10ms path. |
| `app/websocket.py` | REPLACE `[existing — verified at app/websocket.py]` | `ConnectionManager` keeps the local registry (token→clients) but is now **fed by `app/pubsub.py`** (Redis relay), not directly by the request handler. Adds SSE generator helper. |
| `app/database.py` | REPLACE `[existing — verified at app/database.py]` | WAL pragma, new DDL (§5.8: `endpoints`, `mock_rules`, `request_logs`, `owners`), `hash_email` (keep `[existing — verified at app/database.py:102]`), `gen_owner_secret`, `gen_token`. Fire-and-forget trace insert + write-time prune helper. |
| `app/models.py` | REPLACE `[existing — verified at app/models.py]` | Pydantic v2 models for §5 (session, endpoint config, rich rule, trace summary/detail, WS payloads). Drop `UserRegister`/`UserLogin`. |
| `app/auth.py` | NEW | `require_owner(...)` dependency: read `Authorization: Bearer <owner_secret>` (+ `X-Owner-Id`), validate against `owners`, attach `owner_id`. `assert_owns_endpoint(token, owner_id)`. Replaces `get_current_user` header-trust `[existing — verified at app/routes/api.py:24]`. |
| `app/routes/api.py` | REPLACE `[existing — verified at app/routes/api.py]` | All `/api/*` management endpoints per §5.2, real ownership auth. |
| `app/routes/ui.py` | NEW | Server-rendered Jinja routes: `/`, `/d/{token}`. (Moves UI out of `main.py`.) |
| `app/routes/tunnel.py` | NEW | Server side of the tunnel WS control channel (§4.6): `/ws/tunnel/{token}` registry + request/response framing. |
| `app/utils/cleanup.py` | REPLACE `[existing — verified at app/utils/cleanup.py]` | Retention sweep enforcing **both** caps (100-trace + 24h TTL) on `RETENTION_SWEEP_SECONDS` (§5.8). |
| `app/utils/helpers.py` | REPLACE `[existing — verified at app/utils/helpers.py]` | Keep `generate_endpoint_id` (ambiguity-stripped alphabet) + `format_headers` (hop-by-hop strip). Add `gen_owner_secret`, jsonpath-lite getter, RNG seam for chaos tests. |
| `app/routes/backup.py` | DELETE `[existing — verified at app/routes/backup.py]` | SMTP backup removed; export/restore deferred (PRD §9 OQ-7, not in this contract). |
| `app/routes/webhook.py` | DELETE `[existing — verified at app/routes/webhook.py]` | GitHub auto-deploy `git pull`+restart removed (cruft/RCE, `_decisions.md` §10 spirit). |
| `tunnel/mock_tunnel.py` + `tunnel/__init__.py` + `tunnel/README.md` | NEW | `mock-tunnel --port 3000 --endpoint <slug>` reference CLI (§4.6). Entry point in `pyproject`/`setup` or run via `python -m tunnel`. |

### Frontend lane (`templates/`, `static/`)

| File | Action | Responsibility |
| --- | --- | --- |
| `templates/base.html` | REPLACE `[existing — verified at templates/base.html]` | Shell: Tailwind Play CDN + Alpine.js CDN + HTMX CDN + `defer`-loaded `static/js/*`. Remove `/backup` nav + login redirect JS. localStorage owner read into an Alpine store. |
| `templates/index.html` | REPLACE `[existing — verified at templates/index.html]` | Landing: email form → `POST /api/session` → store `{owner_id, owner_secret, token}` in localStorage → redirect `/d/<token>`. |
| `templates/dashboard.html` | REPLACE `[existing — verified at templates/dashboard.html]` | Split-screen: left live feed (Alpine store fed by `request-stream.js`), right inspector tabs (Headers/Query/Body/Response/State&Trace), connection-health pill, endpoint config panel, rule-builder modal mount. |
| `templates/partials/*.html` | NEW | HTMX partials: rule row, rule-builder modal body, endpoint settings form, inspector tab bodies (server-rendered when fetched by `/api` HTML responses are **not** used — inspector renders client-side from JSON; partials are for the rule list/settings HTMX swaps). |
| `templates/mock.html`, `login.html`, `register.html`, `backup.html` | DELETE `[existing — verified]` | Superseded; no password/registration/backup wall. |
| `static/css/app.css` | NEW | Tailwind layer overrides + design tokens (palette from design step). |
| `static/js/request-stream.js` | NEW | The `useRequestStream` substitute: open WS (`/ws/<token>`), exponential-backoff reconnect (250ms→8s, jitter), **dedupe** by `request_id`, push into Alpine `feed` store, cap buffer at retention cap, drive the health pill. SSE fallback if WS fails repeatedly. |
| `static/js/stores.js` | NEW | Alpine stores: `feed` (rows + selected), `inspector` (tab data, lazy-fetch full trace via `GET /api/requests/{id}`), `endpoint` (config), `rules` (list/edit). |
| `static/js/rule-builder.js` | NEW | Multi-tab Create-Rule modal logic; serializes the `MockRule` JSON to `POST/PATCH /api/endpoints/{token}/rules`. |

### Infra

| File | Action | Responsibility |
| --- | --- | --- |
| `Dockerfile` | REPLACE `[existing — verified at Dockerfile]` | `python:3.12-slim`, install reqs, `CMD ["uvicorn","app.main:app","--host","0.0.0.0","--port","8000"]`, non-root user, `HEALTHCHECK` curl `/healthz`. |
| `docker-compose.yml` | REPLACE `[existing — verified at docker-compose.yml]` | Services `app` + `redis`; `redis` healthcheck (`redis-cli ping`); `app` `depends_on: redis: condition: service_healthy` + own healthcheck; named volumes `hookbox_data` (SQLite) + `hookbox_redis`; internal network. |
| `README.md`, `FEATURES.md` | REPLACE `[existing — verified]` | New platform docs: env vars, `*.localhost`/`nip.io` DNS recipe, tunnel usage, compose. |

---

## 3. Routing & isolation (a) — 3-plane Host-header dispatch + path fallback (`_decisions.md` §2)

### 3.1 Plane decision (in `PlaneDispatchMiddleware`, runs before routing)

Inputs: `host = request.headers["host"]` (strip `:port`), `path = request.url.path`. `APP_HOSTS` = `{localhost, 127.0.0.1, <APP_HOST>, <bare MOCK_DOMAIN>}` (the bare apex serves the UI).

```
def resolve_plane(host, path) -> (plane, token|None):
    sub = subdomain_of(host, MOCK_DOMAIN)        # "<token>" if host == <token>.<MOCK_DOMAIN>, else None
    if sub:                                       # P1: wildcard mock host — EVERYTHING here is mock,
        return ("mock", sub)                      #     including /api and /static (they are the mock's own paths)
    if path == "/e" or path.startswith("/e/"):    # P1: localhost path-fallback
        token = path.split("/", 3)[2] if len-ok else None
        return ("mock", token)                    # rest-of-path handed to engine as the mock path
    if path == "/api" or path.startswith("/api/"):
        return ("api", None)                      # P2: management
    if path.startswith("/static/") or path in ("/","/healthz") or path.startswith("/d/") \
       or path.startswith("/ws/"):
        return ("ui", None)                       # P3: UI/static/ws
    return ("ui", None)                           # default app-host → UI (404 inside UI router)
```

**Why the mock catch-all can never shadow `/api` or the UI:** two independent guards.
1. **Host guard:** the mock engine only ever runs when `request.state.plane == "mock"`, which on the **app host** requires the `/e/<token>/` prefix. Plain `/api/...` and `/d/...` on the app host resolve to `"api"`/`"ui"` and are dispatched to those routers.
2. **Route-order guard:** the FastAPI catch-all (`@app.api_route("/{full_path:path}")`) is registered **last**. Explicit routers (`/api/*`, `/`, `/d/{token}`, `/static`, `/ws/*`) are matched first by Starlette. The catch-all only fires for unmatched paths, and it immediately checks `request.state.plane`; if not `"mock"` it returns the UI 404 — it never serves mock content for an API/UI path.
   Conversely, on a **mock host** (`<token>.<MOCK_DOMAIN>`), a request to `/api/users` is `plane="mock"`, so it is interpreted as **mock path `/api/users`** for that endpoint (matched against rules/CRUD/MITM) — management is unreachable there by construction. The management API is **only** on the app host.

### 3.2 Path fallback ↔ subdomain equivalence
For `/e/<token>/<rest>`, the engine receives **mock path = `/<rest>`** (the `/e/<token>` prefix is stripped) so a rule written for path `/users` matches identically whether reached via `https://<token>.<MOCK_DOMAIN>/users` or `http://localhost:8000/e/<token>/users`. Local DNS recipe documented in README: `*.localhost` resolves to `127.0.0.1` on most OSes, or use `http://<token>.127.0.0.1.nip.io:8000`.

---

## 4. Sequences (key flows)

### 4.1 Core interceptor fast path (b) — target **<10ms** of our overhead (`_decisions.md` §7)

```
Request hits catch-all (plane=="mock", token=T, mock_path=P, method=M)
 1. ep = rule_cache.get(T)                      # in-process dict hit; SQLite only on cold miss
        └─ miss → SELECT endpoint+rules, compile, store; subscribe handled by pubsub
    if ep is None: return 404 JSON {error:"unknown_endpoint"}   (NOT logged against any real endpoint)
 2. if M == OPTIONS: return cors.preflight(request)             # short-circuit, still trace async
 3. rule = matcher.select(ep.rules, M, P, headers, query, body, state_snapshot)
        - state_snapshot read from Redis ONLY if any rule declares a state requirement (lazy)
 4. if rule:
        body = templating.render(rule.response_body_template, ctx)   # sandboxed, no eval
        apply rule.state_writes -> redis_state.set(T, k, v)          # fire-and-forget if not gating later
        resp = Response(body, rule.status_code, headers+rule.response_headers)
        served_by = "rule:<id>"
    elif ep.auto_crud and crud.matches(P):
        resp = await crud.handle(T, M, P, body)                      # Redis list op
        served_by = "crud"
    elif ep.target_url:
        resp = await proxy.forward(ep, M, P, headers, body)          # httpx; EXEMPT from <10ms
        served_by = "mitm"
    else:
        resp = default_404 or default_echo                           # ep.default_mode
        served_by = "default"
 5. conditions: rate-limit check (token bucket) -> maybe 429
                chaos roll (seeded) -> maybe 502/503/504/dropout
                latency -> await asyncio.sleep(ms/1000)              # excluded from overhead metric
 6. resp.headers += cors.headers(request)  + X-HookBox-* identifying headers
 7. asyncio.create_task(persist_and_publish(T, trace))              # FIRE-AND-FORGET: never awaited
 8. return resp
```

**<10ms guarantees:** step 1 is an O(1) dict lookup; matching is linear over a small compiled rule list with precompiled path regexes; templating is a single-pass scanner over the body string; step 7 never blocks (the trace SQLite write + Redis publish happen in a background task). The only awaited I/O on a matched fast path is the **optional** state read (step 3, only when a rule gates on state) and the **optional** rate-limit check (step 5, one Redis `EVAL`); both are sub-millisecond against local Redis and are documented as the measured overhead. MITM (step 4 elif) and applied latency (step 5) are explicitly **exempt** from the budget.

### 4.2 Stateful multi-step (c) — login → gated dashboard
```
POST /login  → rule "set_auth": match method=POST path=/login → state_writes {authenticated:"true"} → 200
   redis HSET state:T authenticated true   (TTL = STATE_TTL, default 24h)
GET /dashboard → rule "dash": match path=/dashboard AND state.authenticated == "true"
   if state.authenticated == "true": return dashboard body (200)
   else: rule not selected → falls through to CRUD/MITM/default (e.g. 401 default)
```
State is per-endpoint (`state:<token>` Redis hash). Reset via `DELETE /api/endpoints/{token}/state` (clears the hash). Isolation: keyspace is prefixed by token, so endpoint A's hash is unreachable from B.

### 4.3 Auto-CRUD (c)
```
POST /books {title:"x"} → id=uuid4 → RPUSH crud:T:books {id,title:"x"} → 201 {id,title:"x"}
GET  /books            → LRANGE crud:T:books 0 -1 → 200 [ ... ]
GET  /books/<id>       → scan list → 200 {..} or 404
PUT/PATCH /books/<id>  → replace/merge element → 200 {..} or 404
DELETE /books/<id>     → remove element → 204 (empty)   ; subsequent GET/<id> → 404
```
Collections live in Redis (durable across restart iff Redis AOF/RDB persistence is on — see §7 R7; `MOCK_DOMAIN`-volume + `--appendonly yes` in compose). Precedence: **rules > CRUD > MITM > default** (§4.1) — a rule that matches `/books` wins over CRUD; documented and shown in the trace `served_by`.

### 4.4 MITM forward (c)
```
unmatched + ep.target_url set:
   url = ep.target_url + mock_path + querystring
   SSRF guard: resolve host; reject private/link-local/loopback/metadata unless MITM_ALLOW_PRIVATE
   httpx.request(M, url, headers=stripped, content=body, timeout=MITM_TIMEOUT_S, follow_redirects=False)
   on success: copy status+safe headers+body (<= MITM_MAX_BODY_BYTES) → client ; trace.served_by="mitm" ; trace.response captured
   on timeout/conn error: 504 (timeout) / 502 (conn) JSON {error:"upstream_unreachable"} ; logged
```

### 4.5 Real-time pipeline (d) — Redis pub/sub → WS/SSE
```
persist_and_publish(T, trace):                      # background task (fire-and-forget)
   await db insert into request_logs (+ write-time prune to 100)
   await redis PUBLISH trace:T  json(ws_event)      # ws_event = {type:"new_request", data:{summary}}

app/pubsub.py relay (one task per process, started at lifespan):
   psub = redis.pubsub(); await psub.psubscribe("trace:*","cfg:*")
   async for msg in psub.listen():
        if channel startswith "trace:": manager.broadcast(token, payload)   # to local WS/SSE clients
        if channel startswith "cfg:":   rule_cache.invalidate(token)

Client (static/js/request-stream.js):
   ws = new WebSocket(/ws/<token>); on message → dedupe(request_id) → feed.unshift(row) (cap 100)
   on close → backoff reconnect (250→500→1000→2000→4000→8000ms + jitter); health pill: live/reconnecting/down
   fallback: after N WS failures → EventSource(/sse/<token>)
```
Channel scoping: `trace:<token>` — a dashboard subscribes only to its token, so cross-endpoint leakage is impossible.

### 4.6 Tunnel control channel (f)
```
CLI: mock-tunnel --port 3000 --endpoint <slug> --server wss://app  --secret <owner_secret>
  → WS connect /ws/tunnel/<slug>  with Authorization: Bearer <owner_secret>  (must own <slug>)
  → server registers tunnel for <slug> in a process-local registry (single-instance scope)

Public request to <slug>.<MOCK_DOMAIN> when a tunnel is registered AND no local rule matches
  (tunnel sits in the resolution chain as a peer of MITM; ep.tunnel_active flips when registered):
   server frames request → {id, method, path, headers, body_b64} → WS send to CLI
   CLI replays to http://localhost:3000 → frames response {id, status, headers, body_b64} → WS send back
   server resolves the pending future for id → returns response to public caller (timeout → 504)

Framing (JSON text frames over one WS, multiplexed by request id):
   →client {t:"req", id, method, path, query, headers, body_b64}
   ←client {t:"res", id, status, headers, body_b64}
   ↔       {t:"ping"} / {t:"pong"}        (keepalive)
   ←client {t:"err", id, message}
Reconnect: CLI backoff like request-stream.js; while disconnected, public callers get 504 "no_tunnel".
```
Resolution precedence with tunnel: **rule > CRUD > tunnel(if active) > MITM(if target) > default**. (Tunnel and MITM are mutually-exclusive "remote" modes; if both set, tunnel wins because it's an explicit live dev session.)

---

## 5. FROZEN INTERFACE CONTRACT (authoritative)

> Everything below is **frozen**. FE and BE implement against exactly these shapes. All `/api/*` JSON. All timestamps ISO-8601 UTC strings. All management mutations require owner auth (§5.1).

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

### 5.4 WebSocket / SSE messages (real-time pipe)

- **Live feed WS:** `GET ws(s)://<app-or-mock-host>/ws/{token}`. Auth: opportunistic — token in path scopes the channel; no secret required to *watch* a feed (consistent with the existing open WS `[existing — verified at app/main.py:29]`); ownership is not leaked because only summaries flow and full detail requires bearer auth via `GET /api/requests/{id}`.
- **SSE fallback:** `GET /sse/{token}` (text/event-stream), same payloads as `data:` events.
- **Direction:** server → client only (feed). Client → server: none required; server ignores inbound text except `"ping"`.

| Direction | Event `type` | Payload (`data`) | When |
| --- | --- | --- | --- |
| s→c | `new_request` | `RequestSummary` (§5.3) | a request was served on `{token}`'s mock surface |
| s→c | `endpoint_updated` | `{ token, fields:[...] }` | endpoint config changed (so dashboard refreshes settings) |
| s→c | `state_changed` | `{ token, key, value }` | a rule mutated state |
| s→c | `hello` | `{ token, server_time }` | sent on connect (lets client sync + confirm channel) |
| c→s | `ping` | `"ping"` (raw text) | client keepalive; server replies WS pong frame |

**Total WebSocket/SSE message types defined: 5** (4 server→client events + 1 client→server keepalive).

### 5.5 Mock-surface behavior contract (P1 catch-all — behavioral, not a fixed path)
- **Reachable as:** any method, any path on `<token>.<MOCK_DOMAIN>/<path>` **or** `/<app-host>/e/<token>/<path>`.
- **Resolution order (frozen):** `OPTIONS preflight` → **matching rule** (by `priority`,`id`) → **Auto-CRUD** (if `auto_crud` and path looks like `/<collection>[/<id>]`) → **tunnel** (if `tunnel_active`) → **MITM** (if `target_url`) → **default** (`default_mode`: `mock_404` returns `404 {error:"no_match"}`; `echo` returns `200` echoing the request).
- **Every response** carries auto-CORS headers (§5.6 when `cors_enabled`) **plus** identifying headers: `X-HookBox-Endpoint: <token>`, `X-HookBox-Served-By: <served_by>`, `X-HookBox-Rule-Id: <id-or-absent>`.
- **Unknown token:** `404 {error:"unknown_endpoint", detail:"..."}` with header `X-HookBox-Endpoint: <token>`; **not** logged against any endpoint.
- **Conditions order:** rate-limit (→`429 {error:"rate_limited"}` + `Retry-After: <sec>` + `X-RateLimit-Limit/Remaining`) → chaos (→ random `502|503|504` JSON `{error:"chaos"}`, or, for `dropout`, the connection is closed without a response) → latency (`sleep` before returning).

### 5.6 Auto-CORS header set (frozen)
- **Preflight** (`OPTIONS`, when `cors_enabled`): status `204`, headers:
  `Access-Control-Allow-Origin: <reflected Origin or *>`,
  `Access-Control-Allow-Methods: GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD`,
  `Access-Control-Allow-Headers: <reflected Access-Control-Request-Headers or *>`,
  `Access-Control-Max-Age: 600`,
  `Vary: Origin`.
- **Every non-preflight response** (when `cors_enabled`): `Access-Control-Allow-Origin: <reflected Origin or *>`, `Access-Control-Expose-Headers: *`, `Vary: Origin`.
- **`Access-Control-Allow-Credentials`:** **omitted** (not sent). Sending `*` for origin and `true` for credentials is invalid per the Fetch spec; since the spec demands "wide-open", we reflect the origin and **do not** claim credentials support. (Recorded as the resolution of PRD OQ-12.)

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

---

## 6. Scope gaps for the PM to resolve (PRD review loop)

The contract above is implementable as written, but these PRD items are **technically underspecified** and I have made a **default choice** that the PM must confirm or override:

1. **"Webhook Actions" tab (PRD AC-33, OQ-9).** The spec lists a "Webhook Actions" tab in the rule builder but never defines behavior. **Default:** modeled as `WebhookAction` (fire-and-forget outbound `POST` on rule match) in the contract, **but marked optional** — FE renders the tab, BE may ship it as a no-op stub in v1. *PM: in scope for v1, or defer the field?* (If deferred, it stays in the schema as accepted-but-ignored so the contract doesn't change later.)
2. **`echo` default mode.** I added `default_mode: "mock_404" | "echo"` so an endpoint with no rules/CRUD/MITM can still be useful (echo the request like the old `/hook` did `[existing — verified at app/main.py:92]`). *PM: confirm `echo` is desired, or is `mock_404` the only default?*
3. **MITM SSRF policy is security-owned (PRD OQ-3).** I froze `MITM_ALLOW_PRIVATE=false` (block private/loopback/link-local/metadata) + `MITM_TIMEOUT_S` + `MITM_MAX_BODY_BYTES` as the *shape*; the **exact** allow/deny values and redirect policy must come from security-engineer. The contract field (`target_url`) and error codes (`502`/`504`) are stable regardless.
4. **Tunnel auth + "no tunnel connected" (PRD OQ-5).** Frozen: bearer `owner_secret` to bind `<slug>`; public callers get `504 {error:"no_tunnel"}` when none connected. *Security to confirm bearer-over-WS is acceptable; shape is stable.*
5. **State scope (PRD AC-10).** Frozen as **per-endpoint** (`state:<token>`), not per-caller-session, matching the spec's login→dashboard example. *PM: confirm per-endpoint (shared across all callers of the mock) is the intended semantics.*
6. **Export/restore (PRD OQ-7).** **Not** in this contract (SMTP removed regardless). If the PM wants JSON export/restore back, it's additive (`GET/POST /api/owner/export`) and does not disturb §5.

These do not block FE/BE from starting against §5 — fields 1, 6 are additive; 2, 5 are enum/semantic confirmations; 3, 4 are value-fills behind stable shapes.

---

## 7. Data model & storage notes

- **Durable vs ephemeral split:** SQLite is the source of truth for config + rules + trace history (survives restart on the named volume). Redis holds live state, CRUD collections, rate buckets, and pub/sub. **R7 (PRD):** to keep Auto-CRUD/state across restarts, compose runs Redis with `--appendonly yes` on a named `hookbox_redis` volume; the contract still treats Redis data as *best-effort durable* (a `redis` wipe loses live state/collections but never config/rules/history).
- **aiosqlite access pattern:** keep the per-request `get_db` dependency `[existing — verified at app/database.py:11]` for management routes (short-lived connection, `Row` factory). The **interceptor fast path does not open a per-request DB connection** — it reads from `rule_cache` (in-process) and writes traces from a **single long-lived background connection** owned by `persist_and_publish` (serialized writes, WAL allows concurrent reads). This is the key change that protects the <10ms budget.
- **Migration approach:** this is a **ground-up replacement**, not a live migration — `init_db` creates the new schema with `CREATE TABLE IF NOT EXISTS`. Old `users`/`requests` tables are abandoned (a fresh volume is expected per `_decisions.md` §0). `reset_db.sh` `[existing — verified at repo root]` documents the wipe for dev. No data carried from the toy schema.

---

## 8. FE / BE work split (independent lanes against frozen §5)

**BE owns** (`app/`, `config.py`, `requirements.txt`, Docker, `tunnel/`): every `/api/*` endpoint (§5.2) returning exactly the §5.3 models; the mock catch-all behavior (§5.5) incl. resolution order, templating engine (§5.7), Auto-CRUD, MITM, conditions, auto-CORS (§5.6); Redis facade + pub/sub relay; the WS/SSE **server** emitting §5.4 events; the retention sweep (§5.8); the tunnel WS server + `mock-tunnel` CLI; SQLite DDL + WAL; auth (§5.1). BE can build and test entirely with `curl`/pytest against §5 without any template existing.

**FE owns** (`templates/`, `static/`): landing email form → `POST /api/session` → localStorage; the split-screen dashboard consuming `GET /api/endpoints/{token}`, `/rules`, `/requests`, `/requests/{id}`; `request-stream.js` consuming the §5.4 WS/SSE events (dedupe, backoff, cap, health pill); the inspector tabs rendering `RequestDetail`; the multi-tab rule-builder serializing `MockRuleCreate`/`MockRulePatch`; endpoint settings form serializing `EndpointConfigPatch`; CORS-agnostic (it talks to same-origin `/api`). FE can build against a static JSON fixture matching §5.3 and a mock WS that emits §5.4 events — it never needs BE running to develop the UI shapes.

**Seam guarantee:** the only shared artifacts are the §5.3 JSON shapes and the §5.4 event names/payloads. Neither lane can force the other to change shape because both reference this frozen section, not each other's code.

---

## 9. Technical risks, concurrency, failure handling, tradeoffs

- **R1 — <10ms vs Python.** Mitigated by the in-process `rule_cache` (no per-request SQLite read on the matched path), single-pass templating (no Jinja/regex-per-tag), and **fire-and-forget** trace+publish (`asyncio.create_task`, never awaited). Residual awaited I/O on the fast path = optional state read + optional rate-limit `EVAL` against local Redis (sub-ms). **Benchmark harness (PRD OQ-4):** a `pytest`/`locust` profile hitting a no-latency matched rule, measuring `overhead_ms = duration_ms − applied_latency`, asserting median < 10ms (the trace column `overhead_ms` makes this directly observable).
- **R2 — Auth IDOR.** The old `X-User-Id` trust `[existing — verified at app/routes/api.py:24]` is replaced by hashed-secret bearer (§5.1); endpoint routes verify ownership → `404` for non-owners. Security-engineer owns the final entropy/enumeration ruling (PRD OQ-1/2).
- **R3 — MITM SSRF.** `proxy.forward` resolves+vets the target IP (block private/loopback/link-local/metadata unless `MITM_ALLOW_PRIVATE`), caps body+timeout, disables redirects. Value-fill is security-owned (PRD OQ-3); shape is frozen.
- **R4 — Background-task loss on crash.** Fire-and-forget traces can be lost if the process dies mid-task; acceptable (traces are debug telemetry, not transactional). The response is never at risk. Pub/sub publish failures are swallowed (logged) — a dead Redis degrades the live feed, not the mock path (engine reads cache, not Redis, for matching).
- **R5 — Rule cache staleness across instances.** In a single-instance deployment (locked scope) the cache is authoritative. The `cfg:<token>` pub/sub invalidation makes it *correct* for a future second instance; until then it's belt-and-suspenders. Tradeoff: a management write has eventual (≈ms) visibility on the mock path after invalidation.
- **R6 — Chaos/rate-limit determinism for tests.** `conditions.py` takes an injectable RNG (`random.Random(seed)`); `chaos_pct=100` → always fail, `0` → never (AC-26). Rate-limit uses a Redis Lua token bucket (atomic) so concurrency is correct under load.
- **R7 — Redis durability** (above): AOF on a named volume; contract treats Redis as best-effort durable.
- **R8 — WebSocket fan-out backpressure.** A slow WS client could stall the relay; `ConnectionManager.broadcast` sends per-client with a short timeout and drops dead/slow clients (extends the existing disconnect-on-error pattern `[existing — verified at app/websocket.py:31]`). Feed is capped client-side (100 rows) so the DOM never grows unbounded (AC-30).
- **R9 — Tunnel abuse.** Reverse-tunneling public traffic to a dev box is an abuse vector; mitigated by requiring `owner_secret` to bind a slug and scoping to single-instance. Production-grade Go binary is explicitly deferred (`_decisions.md` §8); Python reference satisfies the blueprint.
- **Edge cases handled by the contract:** unknown token (`404`, not logged); `OPTIONS` always answered even with zero rules; templating unknown tag → literal (no 500); CRUD on non-existent id → `404`; MITM target unset + no match → `default_mode`; rate-limit `0` = unlimited; chaos `dropout` = connection close (resolution of PRD OQ-12).
