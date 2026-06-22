# HookBox

**Self-hosted, Beeceptor-class API mocking & HTTP interception platform.**

Mock, intercept, inspect, and virtualize HTTP APIs without standing up a backend.
HookBox serves rule-driven mock responses with dynamic templating, persists
per-endpoint state, can act as an instant CRUD backend, can forward unmatched
traffic to a real upstream (MITM) and capture it, auto-handles CORS, simulates
adverse network conditions, and streams every transaction to a real-time
split-screen debugging dashboard — behind a **no-password, email-keyed** session,
with a local-tunnel CLI.

It ships as a **single Rust binary over one SQLite file** that also serves the
React dashboard. No Redis, no Postgres, no Node at runtime — one process, one
file. Single-instance by design (in-process cache/queues/pub-sub) — the self-host
sweet spot; scale vertically.

> Re-platformed from the original Python (FastAPI + Jinja + Redis + WebSockets)
> implementation to Rust/Axum + SQLite for a feather-weight, self-contained
> deployment. Design & specs: [`docs/superpowers/specs/`](docs/superpowers/specs/)
> and [`docs/features/hookbox-rust-replatform/`](docs/features/hookbox-rust-replatform/).

## Stack

| Layer | Technology |
| --- | --- |
| Web / proxy | **Rust + Axum** (tokio), one binary |
| Durable store | **SQLite (WAL)** — endpoints, rules, traces, per-endpoint state, Auto-CRUD collections |
| Real-time fan-out | in-process `tokio::sync::broadcast` (WebSocket + SSE feed) |
| Rate limiting | in-memory token bucket (`DashMap`), bounded, fails open |
| Retention | in-process tokio interval sweep |
| Outbound HTTP (MITM) | `reqwest`, with a resolved-IP SSRF guard |
| Frontend | **Vite + React + TypeScript** SPA (Tailwind + Radix), served from `dist/` |

```
Browser ─▶ [ Rust / Axum binary ]  ──▶  SQLite (data/app.db, WAL)
              ├─ <token>.<MOCK_DOMAIN>/… , /e/<token>/…   P1 mock interceptor
              ├─ /api/**                                  P2 management API (capability-gated)
              ├─ /ws/<token> , /sse/<token>               owner-gated live feed
              ├─ /ws/tunnel/<token>                        tunnel control channel
              └─ static + SPA fallback                     P3 dashboard (dist/)
```

## Quick start

### Prerequisites
- **Rust** (stable, ≥1.80) — install via [rustup](https://rustup.rs/)
- **Node 22+** and **pnpm 10** — only to *build* the SPA (not at runtime)

```bash
bash scripts/start.sh      # builds SPA (Vite → dist/) + backend (cargo --release),
                           # applies migrations, seeds demo data on first run,
                           # then serves → http://localhost:8080
```

Open **http://localhost:8080**, enter an email on the landing page, and you get an
endpoint session instantly — no password, no registration. Your owner identity +
capability live in browser `localStorage`; the same email always recovers access
(the capability rotates on each sign-in, so an old leaked secret stops working).

### Docker

```bash
docker compose up -d --build       # one app container (Rust binary + SQLite); no Redis
curl -s http://localhost:8080/healthz   # 200
open http://localhost:8080
```

`docker compose down` keeps the named volume (config, rules, history, state);
`docker compose down -v` wipes it.

## Drive it from the CLI

```bash
# 1. Get an owner session → {owner_id, owner_secret, primary:{token,…}}
curl -s -X POST localhost:8080/api/session \
  -H 'content-type: application/json' -d '{"email":"you@example.com"}'

# 2. Add a mock rule (owner_secret as the bearer; <token> from step 1)
curl -s -X POST localhost:8080/api/endpoints/<token>/rules \
  -H 'Authorization: Bearer <owner_secret>' -H 'content-type: application/json' \
  -d '{"match":{"method":"GET","path":"/hello"},
       "response":{"status_code":200,"content_type":"application/json",
                   "body_template":"{\"hi\":\"{{request.query.name}}\"}"}}'

# 3. Hit the PUBLIC mock surface (no auth on the mock plane)
curl -s 'localhost:8080/e/<token>/hello?name=ada'    # → {"hi":"ada"}
```

## Addressing a mock endpoint

Every endpoint has a token `<token>`, reachable two equivalent ways:

1. **Wildcard subdomain (production):** `http(s)://<token>.<MOCK_DOMAIN>/<path>`
2. **Path fallback (local dev):** `http://<APP_HOST>:8080/e/<token>/<path>`

Both reach the same interceptor; the `/e/<token>` prefix is stripped so a rule
written for `/users` matches identically either way.

### Local wildcard DNS recipe
- **`*.localhost`** resolves to `127.0.0.1` on most OSes → `http://<token>.localhost:8080/<path>`.
- **`nip.io`**: `http://<token>.127.0.0.1.nip.io:8080/<path>`.
- Otherwise just use the **path fallback** `http://localhost:8080/e/<token>/<path>`.

If `MOCK_DOMAIN` is unset or has no dot, the app logs a warning and serves
**path-fallback-only** mode (it does not crash); the dashboard then shows only the
`/e/<token>` URL.

## Real-time feed is owner-gated

The dashboard's live feed (WebSocket `/ws/<token>` and SSE fallback `/sse/<token>`)
**requires the owner capability**, presented as `?cap=<owner_secret>` and verified
server-side **before** the socket is accepted / before any event is sent. The
**mock surface itself stays fully public**; only the observability feed is gated.

## Local tunnel CLI

Reverse-tunnel public traffic hitting `<token>.<MOCK_DOMAIN>` (or the path
fallback) to a server on your localhost — a Rust binary built alongside the server:

```bash
./backend/target/release/tunnel \
  --endpoint <token> --secret <owner_secret> \
  --port 3000 [--host ws://localhost:8080]
```

It authenticates with the endpoint's owner capability over the WebSocket control
channel, reconnects with backoff, and stops on a rejected secret. Tunneled traffic
appears in the live feed labeled `tunnel`.

## Configuration (environment variables)

All config is env-driven with safe defaults (nothing is required to boot). Set via
the shell, a `.env` file next to `docker-compose.yml`, or the compose
`environment:` block. The full list lives in `backend/src/config.rs`; the common
knobs:

| Variable | Default | Purpose |
| --- | --- | --- |
| `MOCK_DOMAIN` | `mock.local` | Wildcard mock surface base (`*.<MOCK_DOMAIN>`). Blank/dotless → path-fallback-only. |
| `APP_HOST` | `localhost` | Canonical UI/API host (apex + this host never hit the interceptor). |
| `APP_PORT` / `APP_BIND_HOST` | `8080` / `0.0.0.0` | Published HTTP port / bind address. |
| `DATABASE_PATH` | `data/app.db` | SQLite file (WAL). |
| `STATIC_DIR` | `dist` | Built SPA directory served as P3. |
| `TRACE_CAP` / `TRACE_TTL_HOURS` | `100` / `24` | Per-endpoint trace cap + TTL (enforced at write **and** by the sweep). |
| `RETENTION_SWEEP_SECONDS` | `300` | Background sweep interval. |
| `GONE_TTL_HOURS` | `168` | A deleted endpoint serves `410` until this tombstone window elapses, then `404`. |
| `MITM_TIMEOUT_S` / `MITM_MAX_BODY_BYTES` | `10` / `5000000` | Upstream forward timeout (→`504`) / max captured body. |
| `MITM_ALLOW_PRIVATE` | `false` | When `false`, MITM **blocks** loopback/private/link-local/metadata targets (SSRF guard on the **resolved** IP). |
| `LATENCY_MAX_MS` | `10000` | Upper clamp for simulated latency. |
| `RATE_LIMIT_MAX_PER_MIN` | `100000` | Upper bound for the configurable rate limit (`0` = unlimited). |
| `MAX_INGEST_BODY_BYTES` | `1000000` | Max mock-request body before a `413`. |

## Degradation behavior

With SQLite as the single local store, the old Redis-down failure modes collapse to
ordinary error handling:

| Feature | Behavior under an internal store error |
| --- | --- |
| Static mock matching | **Survives** (served from the in-process rule cache). |
| State-gated rules | **Fail closed** (state condition does not hold → rule skipped, never silently matched). |
| Auto-CRUD | **5xx** (no fabricated/lost data; writes are one SQLite transaction). |
| Rate limiter | **Fails open**, but the bucket map is bounded so it can't be weaponized into memory growth. |
| Real-time feed | Mock serving + SQLite trace logging are unaffected. |

## Development & tests

```bash
# Backend (Rust)
cd backend && cargo test            # 81 unit + 12 integration tests
cargo fmt --all -- --check          # formatting gate
cargo clippy --all-targets -- -D warnings

# Frontend (Vite + React)
pnpm install
pnpm exec tsc --noEmit              # typecheck
pnpm build                          # Vite → dist/
pnpm e2e                            # Playwright (builds + serves dist/, backend mocked in-spec)
```

CI (`.github/workflows/ci.yml`) runs the Rust suite (fmt + clippy + build + test),
a `cargo audit` dependency check, the frontend (typecheck + build + Playwright), and
validates/builds the Docker image on every push and PR.

## Project layout

```
backend/                 Rust crate
  Cargo.toml             bins: hookbox (server), tunnel (CLI), seed
  migrations/            SQLite schema (applied on startup)
  src/
    main.rs, config.rs, state.rs, error.rs, db.rs, auth.rs, ids.rs
    planes.rs, router.rs           # 3-plane Host+path dispatch (mock / api / ui)
    routes/{api,health,feed,tunnel_ws,spa}.rs
    interceptor/{engine,matcher,templating,crud,proxy,cors,conditions}.rs
    feed.rs, state_store.rs, crud_store.rs, rule_cache.rs, limiter.rs, ssrf.rs
    tasks/sweep.rs                  # retention sweep (both caps)
    bin/tunnel.rs, seed.rs
  tests/api.rs                      # integration tests
src/                     Vite + React + TS SPA (landing, dashboard, rule builder, settings, cli)
public/, index.html, vite.config.ts, tailwind.config.ts
e2e/                     Playwright suite
scripts/start.sh         build + seed + serve
Dockerfile, docker-compose.yml
```

## License

MIT
