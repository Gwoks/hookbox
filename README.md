# HookBox

**Self-hosted, Beeceptor-class API mocking & HTTP interception platform.**

Mock, intercept, inspect, and virtualize HTTP APIs without standing up a backend.
HookBox is a low-overhead interceptor (target: **< 10 ms** of our own overhead on
the mock fast path) that serves rule-driven mock responses with dynamic
templating, persists per-endpoint state, can act as an instant CRUD backend, can
forward unmatched traffic to a real upstream (MITM) and capture it, auto-handles
CORS, simulates adverse network conditions, and streams every transaction to a
real-time split-screen debugging dashboard — behind a **no-password, email-keyed**
session, shipped as `docker compose up` with a local-tunnel CLI.

## Stack

| Layer | Technology |
| --- | --- |
| Web / proxy | FastAPI on `uvicorn[standard]` (async, uvloop + httptools) |
| Durable store | SQLite via `aiosqlite` (WAL) — endpoints, rules, request logs, owners |
| Ephemeral store / cache / pub-sub | Redis — per-endpoint state, Auto-CRUD collections, rate-limit token buckets, real-time fan-out |
| Outbound HTTP (MITM) | `httpx` async client |
| Frontend | Server-rendered **Jinja2 + HTMX + Alpine.js + Tailwind** (no Node build, no React/SPA) |

## Quick start (Docker)

```bash
docker compose up
# Redis starts first; the app waits until Redis is healthy, then starts on :8000.
# Open the dashboard:
open http://localhost:8000
```

Enter an email on the landing page to instantly get an endpoint session — no
password, no registration. Your owner identity + capability live in browser
`localStorage`; the same email always recovers access.

Stop and keep data: `docker compose down` (named volumes persist config, rules,
history, and best-effort Redis state). Wipe everything: `docker compose down -v`.

## Quick start (local, no Docker)

```bash
python -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt
# Redis must be reachable (e.g. `docker run -p 6379:6379 redis:7-alpine`).
export REDIS_URL=redis://localhost:6379/0
uvicorn app.main:app --host 0.0.0.0 --port 8000
```

The app **degrades gracefully** if Redis is down (see "Redis-down behavior"); it
never crashes on startup for a missing Redis or an unset `MOCK_DOMAIN`.

## Addressing a mock endpoint

Every endpoint has a token `<token>` and is reachable two equivalent ways:

1. **Wildcard subdomain (production):** `http(s)://<token>.<MOCK_DOMAIN>/<path>`
2. **Path fallback (local dev):** `http://<APP_HOST>:8000/e/<token>/<path>`

Both reach the same interceptor; the `/e/<token>` prefix is stripped so a rule
written for `/users` matches identically either way.

### Local wildcard DNS recipe

Real wildcard DNS is awkward on localhost. Options:

- **`*.localhost`** resolves to `127.0.0.1` on most OSes, so
  `http://<token>.localhost:8000/<path>` often works out of the box.
- **`nip.io`** wildcard DNS: `http://<token>.127.0.0.1.nip.io:8000/<path>`.
- Otherwise just use the **path fallback** `http://localhost:8000/e/<token>/<path>`.

If `MOCK_DOMAIN` is unset or misconfigured, the app logs a warning and serves
**path-fallback-only** mode (it does not crash); the dashboard then shows only the
`/e/<token>` URL chip.

## Configuration (environment variables)

All config is env-driven with safe defaults (nothing is required to boot). Set via
the shell, a `.env` file next to `docker-compose.yml`, or the compose `environment:`
block.

| Variable | Default | Purpose |
| --- | --- | --- |
| `MOCK_DOMAIN` | `mock.local` | Wildcard mock surface base (`*.<MOCK_DOMAIN>`). Blank/invalid → path-fallback-only. |
| `APP_HOST` | `localhost` | Canonical UI/API host (the apex + this host never hit the interceptor). |
| `APP_PORT` | `8000` | Published HTTP port. |
| `REDIS_URL` | `redis://localhost:6379/0` (compose: `redis://redis:6379/0`) | Redis connection. |
| `DATABASE_PATH` | `./data/hookbox.db` (compose: `/app/data/hookbox.db`) | SQLite file (on the `hookbox_data` volume in compose). |
| `TRACE_CAP` | `100` | Hard per-endpoint trace cap (prune oldest beyond this). |
| `TRACE_TTL_HOURS` | `24` | Traces older than this are swept. |
| `RETENTION_SWEEP_SECONDS` | `300` | Interval of the background sweep that enforces **both** caps. |
| `STATE_TTL_SECONDS` | `86400` | TTL of the per-endpoint Redis state hash (refreshed on write). |
| `CRUD_TTL_SECONDS` | `86400` | TTL of Auto-CRUD collections (refreshed on write). |
| `MITM_TIMEOUT_S` | `10` | Upstream forward timeout → `504` on timeout. |
| `MITM_MAX_BODY_BYTES` | `5000000` | Max captured upstream response body. |
| `MITM_ALLOW_PRIVATE` | `false` | When `false`, MITM **blocks** loopback/private/link-local/metadata targets (SSRF guard, evaluated on the resolved IP). |
| `MITM_FOLLOW_REDIRECTS` | `false` | If `true`, follow up to `MITM_MAX_REDIRECTS`, re-validating each hop's IP. |
| `LATENCY_MAX_MS` | `10000` | Upper clamp for simulated latency. |
| `RATE_LIMIT_MAX_PER_MIN` | `100000` | Upper bound for the configurable rate limit (`0` = unlimited). |
| `MAX_INGEST_BODY_BYTES` | `1000000` | Max mock-request body before a `413` (rejected before full buffering). |
| `MAX_BODY_BYTES` | `256000` | Max request/response body persisted to a trace (truncated beyond). |
| `CRUD_MAX_ITEMS` / `CRUD_MAX_ITEM_BYTES` | `1000` / `64000` | Auto-CRUD per-collection bounds. |
| `WS_MAX_CONN_PER_ENDPOINT` | `50` | Cap on concurrent live-feed connections per endpoint. |
| `ENDPOINT_ID_LENGTH` | `10` | Endpoint token length (ambiguity-stripped alphabet). |
| `OWNER_SECRET_BYTES` | `32` | Owner capability entropy (256-bit `token_urlsafe`). |

## Real-time feed is owner-gated

The dashboard's live feed (WebSocket `/ws/<token>` and SSE fallback `/sse/<token>`)
**requires the owner capability**, presented as `?cap=<owner_secret>` and verified
server-side **before** the socket is accepted / before any event is sent. The
**mock surface itself stays fully public** (callers must be able to hit the mock
URL); only the observability feed is gated.

## Local tunnel CLI

Reverse-tunnel public traffic hitting `<slug>.<MOCK_DOMAIN>` to your localhost:

```bash
python -m tunnel --port 3000 --endpoint <slug> \
  --server ws://localhost:8000 --secret <owner_secret>
```

The CLI authenticates with the endpoint's owner capability over the WebSocket
control channel. See `tunnel/README.md` for protocol details and reconnect
behavior. A Go binary would be the production-grade choice; this Python reference
satisfies the blueprint.

## Redis-down behavior (degradation)

| Feature | When Redis is down |
| --- | --- |
| Static mock matching | **Survives** (served from the in-process rule cache). |
| State-gated rules | **Fail closed** (state condition does not match; rule skipped). |
| Auto-CRUD | **503** (no fabricated/lost data). |
| Rate limiter | **Fails open** but bounded by the in-process body/size caps. |
| Real-time feed | Mock serving + SQLite logging unaffected; dashboard shows a "degraded" pill. |

## Operations

```bash
docker compose logs -f app          # tail logs
docker compose ps                   # health status of app + redis
curl -s http://localhost:8000/healthz   # {"status":"ok","redis":true,"db":true}
./reset_db.sh                       # wipe the SQLite DB for a fresh schema (dev)
```

API docs (OpenAPI/Swagger) are served at `/docs`.

## Project layout

```
app/
  main.py                 # app factory, lifespan, plane dispatch, mock catch-all
  middleware.py, planes.py# 3-plane Host+path dispatch (mock / api / ui)
  auth.py                 # owner-capability auth (bearer owner_secret)
  database.py             # SQLite WAL, §5.8 DDL, fire-and-forget trace writer
  redis_state.py          # state KV, Auto-CRUD store, rate-limit bucket, pub/sub
  rule_cache.py           # in-process compiled-rule cache (the <10ms path)
  pubsub.py               # Redis relay: trace fan-out + cfg cache-invalidation
  websocket.py            # owner-gated WS + SSE live feed
  routes/api.py           # the management REST API
  routes/ui.py            # server-rendered dashboard routes
  routes/tunnel.py        # tunnel WS control-channel server
  interceptor/
    engine.py             # the ordered mock pipeline (resolution + conditions)
    matcher.py            # rule selection (method/path/header/query/body/state)
    templating.py         # sandboxed {{...}} engine (no eval/Jinja over user text)
    crud.py               # Auto-CRUD over Redis JSON arrays
    proxy.py              # MITM forward + SSRF guard
    conditions.py         # latency / rate-limit / chaos
    cors.py               # auto-CORS preflight + per-response headers
  utils/cleanup.py        # retention sweep (both caps)
templates/, static/       # frontend lane (Jinja2 + HTMX + Alpine + Tailwind)
tunnel/                   # mock-tunnel reference CLI
config.py                 # env-driven configuration
Dockerfile, docker-compose.yml
```

## License

MIT
