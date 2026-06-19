# LOCKED DECISIONS — beeceptor-rewrite (read this FIRST, then `prompt.txt`)

These are **frozen** by the human before planning. Every agent (PM, journey, ux,
design, architect, security, engineers, QA) MUST honor them. Do **not** re-open
them. If something here conflicts with `prompt.txt`, **this file wins** and the
conflict is noted below.

## 0. What we are building
A **ground-up rewrite** of HookBox into a **Beeceptor-class platform**: self-hosted
API mocking, HTTP request interception, real-time debugging, and behavior
virtualization. The full product spec is `prompt.txt` at the repo root — read it
in full. This is **NOT an incremental feature** on the current toy webhook-catcher;
it **replaces** it. Treat the existing `app/`, `templates/`, `config.py`,
`docker-compose.yml`, `Dockerfile` as the *prior art to supersede*, not a base to
preserve.

## 1. Locked tech stack (chosen over Go/Rust and React — do NOT substitute)
- **Language/runtime:** Python 3.12+, fully async.
- **Web/proxy framework:** FastAPI on `uvicorn[standard]` (uvloop + httptools).
- **Durable store:** `aiosqlite` (SQLite, WAL mode). Holds endpoints, mock rules,
  request logs, endpoint config. One file on a named Docker volume.
- **Ephemeral store / cache / pub-sub:** **Redis** — per-endpoint stateful KV
  (Section 1.1), rate-limit token buckets (Section 1.6), and the real-time
  fan-out pub/sub channel for the dashboard (Section 5). Redis is a first-class
  container in compose.
- **Outbound HTTP (MITM forward):** `httpx` async client, streaming.
- **Frontend:** server-rendered **Jinja2** templates + **HTMX** + **Alpine.js** +
  **Tailwind CSS**. **No build step, no Node, no React/SPA.** Tailwind via the
  Play CDN (or a tiny standalone Tailwind CLI build into `static/`) — the
  architect picks one and records it in §5/architecture.md.

### 1a. Deliberate deviation from prompt.txt §2 ("React + Tailwind")
We are **intentionally substituting HTMX + Alpine.js + Tailwind for React.** This
delivers the same real-time split-screen, high-density dashboard UX with **zero
JS build toolchain**. **Do not spec or implement React, JSX, Vite, or a Node
build.** The prompt's **`useRequestStream` React hook** becomes an **Alpine.js
store + a small vanilla-JS module** (`static/js/request-stream.js`) with the
*identical responsibilities*: open the WS/SSE pipe, exponential-backoff
reconnect, dedupe, and feed the live feed without locking the DOM. Keep the name
in spirit ("request stream") so it maps cleanly back to the spec.

## 2. Routing & isolation (prompt §3.2)
- **Wildcard subdomain interception:** `*.<MOCK_DOMAIN>` (e.g.
  `https://<token>.mock.local`) is the public mock surface. A Host-header
  dispatch middleware extracts the endpoint token from the subdomain and routes
  to the interceptor engine.
- **Hard isolation** of three planes: (1) wildcard mock interception, (2) the
  management/admin REST API (`/api/*`), (3) the dashboard UI + static assets
  (`/`, `/d/...`, `/static/...`). The architect specifies exactly how the
  middleware decides which plane a request belongs to (Host + path), so the mock
  catch-all can never shadow `/api` or the UI and vice-versa.
- **Local-dev fallback:** real wildcard DNS is awkward on localhost, so ALSO
  support a path-based addressing fallback (e.g. `/e/<token>/<path>`) and
  document `*.localhost` / `nip.io` usage. Both must reach the same engine.

## 3. Required features — cover ALL of prompt.txt §1 (none optional)
1. **Stateful / multi-step transactions** — per-endpoint state in Redis; rules can
   read/require/mutate state vars (login sets `authenticated=true`; a later rule
   matches only when it is true).
2. **Instant Auto-CRUD** — "Enable Auto-CRUD" toggle turns the endpoint into a REST
   DB backend (`POST/GET/PUT/DELETE /<collection>[/:id]`) over an in-store JSON
   array, no rules needed.
3. **Proxy-based partial mocking (MITM)** — per-endpoint "Target Real API URL";
   unmatched requests forward to the real API via `httpx`, capture+log the real
   response, return it to the client.
4. **Auto-CORS engine** — every intercepted request auto-handles `OPTIONS`
   preflight and injects wide-open dynamic CORS headers.
5. **Dynamic response templating** — `{{now 'iso'}}`, `{{random 'uuid'}}`,
   `{{request.query.<k>}}`, `{{request.path.<k>}}`, `{{request.body.<jsonpath>}}`,
   `{{state.<k>}}`, etc. The architect defines the exact tag grammar + engine.
6. **Simulated network conditions** — per-rule/endpoint latency slider
   (0–10000ms), rate limit (req/min, Redis token bucket), and a chaos %
   that injects random 502/503/504 / dropouts.

## 4. Data model (prompt §3.4) — at minimum
`endpoints`, `mock_rules`, `state_objects` (Redis-backed; the durable schema may
keep a snapshot/config row), `request_logs`. The architect owns the authoritative
schema in §5. Email-based access (§ below) maps an email → owner token.

## 5. Access model (prompt §UX-1) — no registration / no password
A user enters an **email** on the landing page to **instantly generate or resume**
an endpoint session (`https://<token>.mock.local`). Session persisted in browser
storage (localStorage). No password wall. The email is the recovery/access key
(hash → owner id, as today's `hash_email`). Security-engineer: threat-model this
(enumeration, ownership/IDOR on the mgmt API, token entropy) and define the
required security ACs — but **do not** add a password/registration wall.

## 6. Data retention (prompt §3.3)
Implement **BOTH** caps (configurable), not just one: a hard **100-trace-per-
endpoint** cap (prune oldest beyond 100) **and** a **24-hour TTL** sweep. A
background task enforces both; document the interval.

## 7. Performance budget (prompt §3.1): <10ms mock-path overhead
The mock **fast path** must add <10ms of *our* overhead: resolve the matching
rule from an in-memory/Redis cache, apply templating + optional simulated
latency, serialize, and return — while writing the `request_logs` trace
**asynchronously / fire-and-forget** (never block the response on the DB write)
and publishing to the real-time pub/sub. (MITM-forward latency is dominated by
the upstream and is exempt from the <10ms budget.)

## 8. Local tunnel client (prompt §6.1)
Deliver a **blueprint + a working Python reference** `mock-tunnel` CLI
(`mock-tunnel --port 3000 --endpoint <slug>`) that reverse-tunnels public traffic
hitting `<slug>.<domain>` down to the developer's localhost, over a multiplexed
WebSocket control channel to the server. Note in the architecture that a Go
binary would be the production-grade choice; the Python reference satisfies the
spec's "blueprint + simple CLI."

## 9. Deployment (prompt §6.2)
Functional `Dockerfile`(s) + a master `docker-compose.yml`: services for the app,
**Redis**, with healthchecks, named persistent volumes (sqlite data + redis),
an internal network, and deterministic readiness (`depends_on: condition:
service_healthy`).

## 10. Cleanup (mandatory)
Remove the **leaked `/status` crypto-trading-bot route** and all crypto/openclaw
references currently in `app/main.py` (lines ~123–268) — it is cruft from an
unrelated project and must not survive the rewrite.

## 11. Lanes (keep parallel autonomy safe)
- **backend-engineer** lane: `app/`, `config.py`, `requirements.txt`.
- **frontend-engineer** lane: `templates/` + `static/`.
- The tunnel CLI + Docker/compose: assign to backend lane (Python + infra).
- FE and BE code only against the **frozen §5 contract** — they never need each
  other to change shape.

## 12. Git / process
- Work lands **directly on `main`** (human's choice), committed in logical
  milestones, pushed at session end (per CLAUDE.md "land the plane").
- All task tracking in **beads** (`bd`); knowledge via `bd remember`. No
  TodoWrite / MEMORY.md.
