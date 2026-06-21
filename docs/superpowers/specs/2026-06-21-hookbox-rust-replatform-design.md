# HookBox — Rust/Axum + SQLite Re-platform (feather-weight) — Design Spec

- **Date:** 2026-06-21
- **Status:** Approved (brainstorming) — seeds the multi-agent feature pipeline
- **Feature slug:** `hookbox-rust-replatform`
- **Precedent:** `../../../../shortener-link` re-platformed Next.js+Postgres+Redis → single Rust(Axum)+SQLite binary serving a Vite+React SPA. This spec applies the same move to HookBox.

---

## 1. Goal

Recreate HookBox — today a Python **FastAPI + Jinja + SQLite + Redis + WebSockets**
app — as a **feather-weight, self-contained** application on the exact platform
shortener-link uses: a **single Rust (Axum) binary over SQLite (WAL)** that also
serves a **Vite + React SPA**. **No Redis, no Postgres, Docker optional.**
Single-instance by design (in-process cache/queues/pubsub) — the self-host sweet
spot; scale vertically.

The recreation is **full feature parity** (all 13 HookBox feature areas) with a
**fresh visual identity and voice** — not a 1:1 visual port of the current
dashboard. The frontend HTML/UX/copy is rebuilt to be fresh, beautiful, and light.

## 2. Non-goals

- Multi-instance / horizontal scale (explicitly traded away with Redis).
- Preserving the Python codebase as runnable (it is retired; it remains in git history).
- Email delivery / verification, passwords, or registration (the email-keyed, capability-backed model is preserved as-is).

## 3. Platform decisions (locked during brainstorming)

| Decision | Choice |
|---|---|
| Backend | Rust + **Axum**, single binary, tokio runtime |
| Data | **SQLite** (WAL) — the only datastore; replaces SQLite **and** Redis |
| Frontend | **Vite + React SPA** (reusing shortener-link's Radix + Tailwind + CVA foundation), served from the binary with SPA fallback |
| Live feed | **in-process tokio `broadcast` channel** (replaces Redis pub/sub); owner-gated **WebSocket** subscribe + **SSE** fallback |
| State / Auto-CRUD | **SQLite** (replaces Redis hashes/arrays) |
| Rate limiting | **in-memory token bucket** (e.g. `DashMap`), fails open |
| Retention | **tokio interval sweep** + write-time cap |
| Tunnel CLI | Rust binary (second `cargo` bin), `tokio-tungstenite` |
| Deployment | Single binary; **Docker optional** (one app container, no Redis) |
| Repo layout | **In-place re-platform** of this repo, mirroring shortener-link |

## 4. Target repo layout (mirrors shortener-link)

```
backend/                 Rust crate
  Cargo.toml             bins: hookbox (server), tunnel (CLI), seed
  migrations/            SQLite schema migrations (applied on startup)
  src/                   axum app, planes, interceptor, templating, proxy, ws, ...
  tests/                 cargo integration tests
src/                     Vite + React SPA (TypeScript)
public/                  static assets
dist/                    built SPA bundle (served by the binary)
data/app.db             SQLite (WAL), gitignored
scripts/start.sh         pnpm build → cargo build --release → migrate → seed → serve :8080
docs/features/hookbox-rust-replatform/   pipeline artifacts (prd, journey, ux, design, copy, architecture, security)
```

The retired Python tree (`app/`, `templates/`, `config.py`, `requirements*.txt`,
`tunnel/`, `Dockerfile`, `docker-compose.yml`) is removed from the working tree
during build (it stays in git history); Docker artifacts are re-authored to the
single-container shape if kept.

## 5. Architecture — three isolated request planes

A single Axum top-level router dispatches by **Host header + path prefix** so the
mock catch-all can never shadow the API or UI (preserves HookBox's hard
plane-isolation guarantee):

- **Mock plane (P1)** — `<token>.<MOCK_DOMAIN>/<path>` (host-based) **and**
  `/e/<token>/<path>` local fallback. Catch-all. Auto-CORS lives here only.
- **Management API plane (P2)** — `/api/**`. Capability-gated. **No** wildcard CORS.
- **Dashboard plane (P3)** — static SPA assets + SPA fallback (non-API, non-mock paths).

## 6. Data model (SQLite — replaces every Redis responsibility)

- `endpoints` — `owner_id` (non-secret), `token`, **`capability_hash`** (256-bit secret, hashed at rest), `target_url`, settings (latency, rate_limit, chaos %, chaos mode, auto_crud flag, auto_cors flag), timestamps.
- `rules` — endpoint FK, priority, match criteria (method, path with `:param`/`/*`, headers, query, JSON-body conditions, state requirements), response (status, headers, templated body), per-rule throttling/actions.
- `traces` — endpoint FK, request/response capture, served-by, rule-id, source label (`rule`/`crud`/`tunnel`/`proxy`/`default`), ts. **100-trace per-endpoint cap + 24h TTL**, enforced at write-time *and* by the sweep.
- `endpoint_state` — endpoint FK, key, value, 24h TTL. (Replaces Redis hash `state:<token>`.)
- `crud_collections` — endpoint FK, collection name, JSON array of items (UUID ids), item-count + size caps; writes atomic via SQLite transaction.

## 7. Mock resolution pipeline (faithful port)

```
OPTIONS preflight → matching rule (priority→id) → Auto-CRUD → tunnel → MITM → default
```

Wrapped, around the served response, in the order:
`rate-limit (429) → chaos (5xx / connection-drop) → latency (sleep)`.

Every mock response carries `X-HookBox-Endpoint`, `X-HookBox-Served-By`, and
(when a rule matched) `X-HookBox-Rule-Id`.

## 8. Subsystems re-implemented in Rust

- **Templating (sandboxed)** — port the hand-written **single-pass scanner**: `{{now 'iso'}}`, `{{random 'uuid'}}`, `{{request.query.*}}`, `{{request.path.*}}`, `{{request.header.*}}`, `{{request.body.<jsonpath>}}`, `{{state.*}}`, etc. **No `eval`/`exec`, no general template engine over user text.** Unknown/malformed tags left literal; never error the mock path.
- **MITM proxy** — `reqwest`; SSRF guard evaluated on the **resolved IP** (block loopback/private/link-local/metadata `169.254.169.254`); strip owner capability + sensitive headers before forwarding; cap request/response body + timeout. A matching local rule always wins over forwarding.
- **Auto-CORS** — P1 only; auto-handle `OPTIONS` preflight; reflect Origin; never claim credentials alongside a wildcard origin.
- **Live feed** — persist trace → publish to the in-process `broadcast` channel → fan out to subscribers. WS subscribe is **owner-gated** (capability required); SSE fallback for the same stream.
- **Rate limit** — in-memory token bucket keyed by endpoint; `0` = unlimited; covers MITM forwards and CRUD writes; **fails open** under internal error.
- **Retention** — tokio interval sweep enforcing the 100-cap + 24h TTL, with the cap also held at write time.
- **Tunnel CLI** — `tunnel` bin: `python -m tunnel`-equivalent UX (`--port --endpoint --secret`); authenticated WebSocket control channel; backoff reconnect; tunneled traffic labeled `tunnel` in the feed.

## 9. Auth model (preserved)

Email → non-secret `owner_id` + a freshly minted **256-bit owner capability**
(stored hashed). The capability backs every `/api/**` call and the WS/SSE feed
subscription, and **rotates on each email submit** (old leaked secret stops
working). No registration, no password, no verification.

## 10. Frontend (fresh identity)

React SPA reusing shortener-link's Radix + Tailwind + CVA component foundation,
but with a **new, fresh, light visual design and a distinct product voice**.
Core screens: landing / email gate, the **split-screen dashboard** (live feed +
deep inspector: Headers · Query · Body · Response Served · State & Tracing), the
**rule builder** (Matching · Response · Templating · Actions · Throttling), and
endpoint settings (proxy target, Auto-CRUD, chaos/latency/rate-limit, CORS,
retention, clear-state). Real-time via a WebSocket hook with SSE fallback.

## 11. Build & verification

`scripts/start.sh`: `pnpm build` (Vite → `dist/`) → `cargo build --release` →
apply migrations on startup → seed demo data on first run → serve on
`http://localhost:8080`. Tests: `cargo test` (backend integration) + Playwright
e2e (frontend), mirroring shortener-link's gates.

## 12. Delivery — multi-agent pipeline (with new copywriter-engineer)

The existing HookBox agent pipeline drives delivery, **re-scoped for Rust+React**
and extended with a copy authority:

```
product-manager (PRD)
  → user-journey ∥ ui-ux ∥ system-architect ∥ security-engineer (DESIGN)
  → design-agent  (visual, after ui-ux)
  → copywriter-engineer  (voice + content design/IA + landing/marketing + microcopy → copy.md;
                          collaborates with ui-ux early, refines after design)
  → product-manager (REVISE — folds critiques, design, copy, architecture, security into the PRD; freezes §5)
  → [approval] → product-manager (BREAKDOWN — build sub-graph)
  → frontend-engineer (Vite/React, src/) ∥ backend-engineer (Rust/Axum, backend/)  [drain bd lanes]
  → qa-engineer (functionality + user POV, loop)
  → security-engineer (REVIEW — code-level, loop)
  → sync
```

**Pipeline prerequisites (this re-platform):**
- `backend-engineer` re-scoped: Rust/Axum, owns `backend/` (+ `Cargo.toml`, `migrations/`).
- `frontend-engineer` re-scoped: Vite/React/TS, owns `src/` + `public/` + frontend config; consumes `design.md` **and** `copy.md`.
- New `copywriter-engineer` agent + a `step:copywriter` discovery issue (depends on `design`; `prd-revise` depends on it).

## 13. Open questions

None blocking. (Brainstorming resolved: frontend = React SPA matching shortener;
scope = full parity incl. tunnel CLI; runtime = all in-process + SQLite;
copywriter = full copy surface incl. landing/marketing.)
