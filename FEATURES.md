# HookBox Features

HookBox is a self-hosted, Beeceptor-class API mocking & HTTP interception
platform. It covers the full mock/intercept/virtualize workflow behind a
no-password, email-keyed session.

---

## 1. Email-keyed access (no password)

Enter an email on the landing page to instantly generate or resume an endpoint
session. The email maps to a non-secret `owner_id`; the server also issues a
secret **owner capability** (256-bit) that backs every management call and the
real-time feed. No registration, no password, no email verification. The
capability rotates on each email submit (an old leaked secret stops working).

## 2. Wildcard mock surface + path fallback

Each endpoint is reachable at `<token>.<MOCK_DOMAIN>/<path>` and, for local dev,
at `/e/<token>/<path>`. Three hard-isolated request planes (mock / management API
/ dashboard UI) guarantee the mock catch-all never shadows the API or UI.

## 3. Rule-driven mock responses

A multi-tab rule builder (Matching · Response · Templating · Actions · Throttling)
defines match criteria (method, path with `:param` segments and `/*` wildcard,
headers, query, JSON body conditions, state requirements) and a templated
response. Rules are evaluated first-by-priority-then-id, deterministically.

## 4. Dynamic response templating (sandboxed)

Response bodies support `{{now 'iso'}}`, `{{random 'uuid'}}`,
`{{request.query.<k>}}`, `{{request.path.<name>}}`, `{{request.header.<name>}}`,
`{{request.body.<jsonpath>}}`, `{{state.<k>}}`, and more — evaluated by a
hand-written single-pass scanner with **no `eval`/`exec` and no Jinja over user
text**. Unknown/malformed tags are left literal and never error the mock path.

## 5. Stateful / multi-step transactions

Rules can read, require, and mutate per-endpoint state (SQLite `endpoint_state`,
24h TTL). Example: a `POST /login` rule sets `authenticated=true`; a later
`/dashboard` rule matches only when that state holds. State is per-endpoint
(shared across all callers of the mock) and clearable from the dashboard.

## 6. Instant Auto-CRUD

Toggle Auto-CRUD and the endpoint becomes a REST DB backend over a SQLite-backed
JSON array per collection: `POST/GET/PUT/PATCH/DELETE /<collection>[/<id>]`, with
generated UUID ids, no rules required. Bounded by configurable item-count and
size caps.

## 7. Proxy / partial mocking (MITM)

Set a `target_url` and unmatched requests forward to the real upstream via
`reqwest`; the real response is captured, returned to the caller, and logged as
"Proxied". A matching local rule always wins over the forward. An SSRF guard
blocks loopback/private/link-local/metadata targets (evaluated on the resolved
IP), strips the owner capability and sensitive headers, and caps body + timeout.

## 8. Auto-CORS engine

Every intercepted response auto-handles `OPTIONS` preflight and injects wide-open
dynamic CORS headers on the mock surface (P1 only — the management API carries no
wildcard CORS). Origin is reflected and credentials are never claimed alongside a
wildcard origin.

## 9. Simulated network conditions

Per-endpoint / per-rule **latency** (0–10000ms), **rate limit** (req/min via an
in-process token bucket; `0` = unlimited; covers MITM forwards and CRUD writes too),
and a **chaos** percentage that injects random `502/503/504` by default with an
opt-in connection-drop mode — all bounded by the global rate/size caps.

## 10. Real-time split-screen dashboard

A live feed streams every served request over a WebSocket (SSE fallback), fanned
out via an in-process broadcast channel. A deep inspector shows Headers · Query · Body · Response
Served · State & Tracing for each trace. The feed and inspector reconcile via the
management API; the feed is **owner-gated** (capability required to subscribe).

## 11. Data retention

Two configurable caps enforced by a background sweep: a hard **100-trace
per-endpoint** cap and a **24-hour TTL**. The cap is also held at write-time so it
never drifts between sweeps.

## 12. Local tunnel CLI

`tunnel --port 3000 --endpoint <slug> --secret <owner_secret>` (a Rust binary built
alongside the server) reverse-tunnels public traffic for `<slug>` to your localhost
over an authenticated WebSocket control channel, with backoff reconnect. Tunneled
traffic appears in the live feed labeled `tunnel`.

## 13. Deployment

`docker compose up` brings up a single healthchecked app container (the Rust binary
+ SQLite), with a named persistent volume for the SQLite data. No Redis, no
Postgres — all four former Redis duties run in-process.

---

## Resolution order (mock surface)

```
OPTIONS preflight → matching rule → Auto-CRUD → tunnel → MITM → default
```

Conditions are applied around the served response in the order:
`rate-limit (429) → chaos (5xx / drop) → latency (sleep)`.

Every mock response carries identifying headers `X-HookBox-Endpoint`,
`X-HookBox-Served-By`, and (when a rule matched) `X-HookBox-Rule-Id`.
