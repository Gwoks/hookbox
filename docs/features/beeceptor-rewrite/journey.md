# User journey: HookBox — Beeceptor-class API mocking & interception (slug: beeceptor-rewrite)

> Grounded in the **current** HookBox (`app/`, `templates/`) and the LOCKED rewrite
> (`_decisions.md`) + spec (`prompt.txt`). "EXISTS today" / "NEW (rewrite)" tags flag
> which screens/behaviors are real now vs. invented by this work, so the PM and engineers
> don't assume scaffolding that isn't there.
>
> **Reality check on what exists today (do not assume more):**
> - Landing/email screen EXISTS (`templates/login.html` → `POST /api/login`), but it stores
>   `localStorage['hookbox_user'] = {id, email, token}` and redirects to `/` (an endpoint
>   *list*), **not** straight to a dashboard. The rewrite collapses this to email → `/d/<token>`.
> - Auth EXISTS but is **client-trusted `X-User-ID` + `X-Email` headers** (`app/routes/api.py:24`)
>   — trivial IDOR. The mock URL is `/hook/{user_id}/{endpoint_id}` (`templates/dashboard.html:178`),
>   i.e. the **owner id is in the public URL**. Every flow below that touches `/api` or the public
>   URL inherits this hole until the architect/security define the capability-token model (OQ-1/2).
> - WS EXISTS at `/ws/{endpoint_id}` (`app/main.py:29`) with an in-process `ConnectionManager`
>   (`app/websocket.py`) and a **naive fixed 3s reconnect, no backoff, no dedupe, no resume**
>   (`templates/dashboard.html:88`). Health is 3 strings: Connecting / Live / Disconnected.
> - Dashboard EXISTS (`templates/dashboard.html`) but is a **single-column table**, not split-screen;
>   the request detail is a **single modal with no tabs**, showing only Headers / Query / Body —
>   **no Response-served, no rule-match/state trace** (those columns don't exist in the schema,
>   `app/database.py:47`).
> - Mock config EXISTS as a **separate page** `/m/<id>` (`templates/mock.html`), one rule **keyed by
>   HTTP method** (`UNIQUE(endpoint_id, method)`), with a status/body/delay/headers form. There is
>   **no path/header/query/body matching, no templating, no state, no rate-limit, no chaos, no MITM,
>   no Auto-CRUD** — all NEW.
> - There is **no Redis, no pub/sub, no httpx proxy, no subdomain routing, no tunnel** today.
> - Retention EXISTS as a single 168h TTL sweep (`app/utils/cleanup.py`) — **no 100-trace cap, wrong
>   TTL**; the rewrite needs both caps at 24h (LOCKED §6).
>
> Personas: **(D)** app/frontend developer, **(Q)** integration/QA rule author, **(F)** dev behind a
> firewall (tunnel), **(O)** operator/self-hoster.

---

## Primary (happy) flow — email → live mock → inspect

The end-to-end "I have an API to fake and watch" path. Entry = landing page; success =
developer sees a live, color-coded request stream and a fully-rendered inspector for a request
their own app made to a mocked endpoint.

1. **(D)** Operator has already run `docker compose up` (see Operator flow); `MOCK_DOMAIN` is set.
   Developer opens `/` (landing). **If `localStorage` owner identity already present → skip to step 4**
   (auto-resume to `/d/<token>`). (NEW behavior: today `/` is the endpoint list, not an auto-redirect.)
2. **(D)** Enters email, submits the single-field form (`POST /api/session`). Server hashes email →
   owner id, finds-or-creates the owner, ensures **at least one endpoint token**, returns
   `{owner, capability token, endpoints[], mock_url(s)}`.
3. **(D)** FE writes owner identity + capability token to `localStorage` and routes to `/d/<token>`
   (AC-3). Landing shows a brief "Welcome / redirecting" success state.
4. **(D)** Dashboard `/d/<token>` loads (NEW split-screen, AC-27/31): header shows the **canonical
   public mock URL(s)** — both the subdomain form `https://<token>.<MOCK_DOMAIN>/…` **and** the
   path fallback `/e/<token>/…` (AC-4/5, OQ-14) — with copy buttons; left column = empty live feed
   ("No requests yet" empty state); right column = inspector placeholder ("Select a request");
   a **connection-health pill** shows "Connecting → Live" (AC-29).
5. **(D)** Browser opens the real-time pipe (WS, `request-stream.js`, AC-30); on open, health → Live
   and the feed back-fills the most recent traces via `GET /api/endpoints/<token>/requests`
   (so a reload isn't blank).
6. **(D/Q)** Developer points their app/curl at the mock URL and fires a request, e.g.
   `curl https://<token>.<MOCK_DOMAIN>/api/users`.
7. **(server)** Host-dispatch middleware extracts `<token>` from the subdomain (or `/e/<token>/…`),
   confirms it's a real, non-expired endpoint, auto-handles CORS (AC-18/19), runs the resolution
   pipeline (state-gated rule → rule → Auto-CRUD → MITM → default), applies templating + simulated
   latency, serializes, returns — and **fire-and-forget** writes the trace to SQLite + publishes to
   Redis pub/sub (AC-38/39). Response carries `X-HookBox-Endpoint: <token>` (AC-4).
8. **(D)** Within ~1 frame a **new feed row** appears at the top without reload (AC-27): color-coded
   method badge, truncated path, served status code, "served-by" hint (rule/CRUD/MITM) (AC-28).
9. **(D)** Developer clicks the row → right-column **deep inspector** opens with tabs **Headers /
   Query Params / Body (collapsible JSON/XML tree) / Response Served / State & Tracing Logs**
   (AC-31), each rendering the captured data for that request, including **which rule matched and
   any state changes**.
10. **Success:** developer confirms request was intercepted, the served response matches expectation,
    and the trace is fully inspectable — without writing a backend, configuring CORS, or reloading.

### Sub-flow A — Author a mock rule (Q), happy path
1. **(Q)** On `/d/<token>` clicks "Create Rule" → NEW **multi-tab modal** (AC-33): **Matching
   Criteria** (method, path/pattern, header/query/body conditions, optional state requirement),
   **Response Payload** (status, headers, body), **Response Templating** (`{{now}}`, `{{random
   'uuid'}}`, `{{request.*}}`, `{{state.*}}`), **Actions** (state read/require/write; "Webhook
   Actions" — scope TBD, OQ-9), **Throttling** (latency / rate-limit / chaos).
2. **(Q)** Fills tabs, sees an inline templating/JSON preview (desired), submits →
   `POST /api/endpoints/<token>/rules`; modal closes; rule appears in a rule list.
3. **(Q)** Re-fires the matching request (step 6 above); feed row now shows "matched rule", the
   Response-Served tab shows the templated body, the trace tab shows the matched rule id. Done.

### Sub-flow B — Enable Auto-CRUD (Q), happy path
1. **(Q)** Opens endpoint settings, toggles **Enable Auto-CRUD** on (AC-11); confirms persistence.
2. **(Q)** `POST /e/<token>/books {…}` → `201` + object with generated `id`; `GET /books` → array;
   `GET /books/<id>` → object; `PUT` → updated; `DELETE` → `204`/`200`; later `GET /books/<id>` →
   `404` (AC-12). Each appears in the live feed labeled "served-by Auto-CRUD".

### Sub-flow C — MITM partial mocking (Q), happy path
1. **(Q)** Sets **Target Real API URL** in settings (AC-14).
2. **(Q)** Fires a path with **no** matching rule → server forwards via `httpx`, returns the **real**
   upstream status/headers/body, and logs the real response (AC-14/15); feed row labeled "proxied".
3. **(Q)** Adds a rule that matches that path → next call returns the **local mock**, upstream
   receives **zero** requests (AC-16); trace shows "matched rule".

---

## Alternate flows — secondary paths the user may take

- **Auto-resume (returning D).** Reopen any page with `localStorage` present → straight to
  `/d/<last token>`, no email entry (extends AC-3). (Today: stored user gates `/` and bounces
  `/login` to `/`; the rewrite must pick the *endpoint* to land on, not just "logged in".)
- **Resume on a new device / cleared storage.** Re-enter the **same email** → same owner, existing
  endpoints resolved, **no duplicate owner** (AC-2). This is the only "recovery" path (no password).
- **Multiple endpoints per owner.** An owner may have >1 endpoint (today's `/` list). The rewrite
  must decide: does `/d/<token>` have an endpoint **switcher / list**, or is the list a separate
  screen? (Today the list at `/` is the home; the rewrite redirects `/` away — see Gap 4.)
- **Create an additional endpoint.** Owner adds a new endpoint (`POST /api/endpoints`) and gets a
  fresh token + URLs. (Today via a `prompt()` on the list page, `templates/index.html:104`.)
- **Path-fallback addressing (local dev).** Developer who can't do wildcard DNS uses
  `/e/<token>/<path>` instead of the subdomain; identical engine + result (AC-5).
- **Edit / disable / delete a rule.** From the rule list (AC-34); interceptor honors change on next
  request. Disable ≠ delete (toggle should round-trip without losing the rule body).
- **Per-method vs. per-path rules.** Today rules are keyed *only* by method; the rewrite adds
  path/header/body matching, so two rules can match the same method — **ordering/precedence** of
  multiple matching rules is a new concern (see Gap 9).
- **Inspect a historical (pre-reload) request.** Click a back-filled row that arrived before this
  WS session; inspector must lazy-load full detail via `GET /api/requests/<id>` (today the modal
  always re-fetches detail — keep that, AC-31 / OQ-11).
- **Replay / re-send a captured request (likely-expected).** Beeceptor users expect to replay a
  trace against the mock or upstream. **Not in any AC** — confirm in/out of scope (Gap 16).
- **Tunnel mode (F).** Instead of letting HookBox serve the mock, the dev binds `<slug>` to their
  localhost: `mock-tunnel --port 3000 --endpoint <slug>` → public traffic to
  `<slug>.<MOCK_DOMAIN>` reverse-tunnels to `localhost:3000`, response returns to the caller
  (AC-40). Traffic still appears in the feed (confirm — Gap 14). On drop, CLI reconnects (AC-41).
- **Adverse-conditions demo (Q).** Dial latency/rate-limit/chaos to demo a flaky API to teammates,
  then dial back to 0 (AC-24/25/26).
- **Clear endpoint state (Q).** Reset per-endpoint state so state-gated rules return to pre-condition
  behavior (AC-10) — mechanism undefined (Gap 6 / OQ).
- **Export / restore data.** The JSON export *concept* exists (`app/routes/backup.py`); survival is
  OQ-7. If kept, it's an alternate "back up my endpoints/rules" path; the **SMTP email backup is
  removed** regardless (LOCKED, no nav link to `/backup`).
- **Operator (O).** `docker compose up` → healthchecks green → app waits on Redis
  (`depends_on: service_healthy`) → configure `MOCK_DOMAIN`/Redis/retention via env (AC-42/44);
  data survives `down && up` without `-v` (AC-43).

---

## Error & failure paths — be ruthless here (these are what implementations forget)

### Access / session
- **Malformed email** at landing → `422`; form shows an inline field error and stays usable (AC-1).
  (Today a non-`ok` response dumps `data.detail` into a red box, `templates/login.html:57` — keep
  the inline pattern, but **don't** reveal whether the email already existed → enumeration, OQ-1/R9.)
- **Empty / disposable / over-long email** → validated client+server; no owner created on reject.
- **Capability token missing/expired/forged** on an `/api/*` call → **401**, FE clears stale
  `localStorage` and routes back to landing (NEW; today an API 4xx silently bounces to `/login`,
  `templates/dashboard.html:127` only handles the endpoint-GET case).
- **IDOR: owner B hits `/api/endpoints/<A's token>/*`** → must be **403/404**, never served
  (R2/OQ-2). **This is the headline security failure path and has no positive happy-path AC pairing
  yet** (Gap 1).
- **Token in the public mock URL leaks the owner id** (today `/hook/{user_id}/…`): the rewrite's
  `*.<MOCK_DOMAIN>/<token>` must **not** embed the owner id; define what `<token>` reveals (Gap 1).

### Mock surface (interception)
- **Unknown / never-existed subdomain** `https://bogus.<MOCK_DOMAIN>/…` → deterministic `404` JSON
  identifying the unknown endpoint; **not** logged against any real endpoint (AC-7). **[ARCH-GAP]**
- **Expired / deleted endpoint** hit after TTL/100-cap pruning or owner delete → distinct, documented
  status (expired vs. never-existed may differ); feed/owner sees nothing. **No AC for the
  expired-but-known case** (Gap 2).
- **Bare apex / `www` / non-token host** (`https://<MOCK_DOMAIN>/`, no subdomain) → must resolve to
  the **UI/landing**, not the interceptor (plane isolation, AC-6). Define the reserved-subdomain
  list (`www`, `api`, `app`, `static`, …) so a token can never collide with the UI/API planes (Gap 3).
- **Mock path collides with reserved paths.** `<token>.<MOCK_DOMAIN>/api/…` and `/static/…` are
  **mock traffic**, but on the app host `/api`/`/static`/`/d` are management/UI — the middleware
  must not let a catch-all rule shadow them and vice-versa (AC-6). Path-fallback `/e/<token>/api/x`
  must treat `/api/x` as the **mock** sub-path, not management.
- **Templating failures:** unknown/malformed tag, `{{request.body.<jsonpath>}}` on a non-JSON or
  empty body, `{{state.<k>}}` for an unset key, recursive/huge expansion → **never 500**, never leak
  internals, fail-safe per contract (AC-23). **[ARCH-GAP: failure mode]**
- **Auto-CRUD malformed input:** `POST` with invalid/empty JSON, wrong content-type, `PUT`/`DELETE`
  on a missing id, id type mismatch, body exceeding size cap, deeply nested payload → defined 4xx,
  not 500 (AC-12 only covers the happy lifecycle — Gap 7).
- **CORS edge:** wildcard `Access-Control-Allow-Origin: *` **cannot** be combined with
  `Allow-Credentials: true` (browsers reject) — define the stance (AC-19, OQ-12). Preflight for a
  path that has no rule must still succeed (AC-18).

### MITM / upstream
- **Upstream unreachable / DNS fail / connection refused** → deterministic error (e.g. `502`) +
  logged failure trace (AC-17). **[ARCH-GAP: status]**
- **Upstream timeout** (slow loris / hung) → `504` after a bounded timeout; response path not blocked
  indefinitely (AC-17). **[ARCH-GAP: timeout value]**
- **Oversized / streaming / chunked upstream response** → enforce a max-capture/body size; decide
  truncate-and-log vs. stream-through-without-full-capture vs. reject (R3, OQ-3). **No AC** (Gap 8).
- **SSRF via Target Real API URL** → block private/link-local/loopback/cloud-metadata
  (`169.254.169.254`, `localhost`, RFC1918), validate scheme, control redirects (R3, OQ-3).
  **Security failure path with no positive/negative AC pair** (Gap 8).
- **Upstream returns its own CORS / `Set-Cookie` / hop-by-hop headers** → strip/normalize so our
  auto-CORS and the client aren't broken (`format_headers` strips hop-by-hop today; extend it).
- **Upstream redirect (3xx) to an internal host** → SSRF-adjacent; follow vs. return decision (OQ-3).

### Real-time pipe (WS)
- **WS drops mid-session** → health pill flips to "Reconnecting"; `request-stream.js` retries with
  **exponential backoff** (NEW — today it's a flat 3s, `templates/dashboard.html:88`) and resumes;
  events that arrived during the gap are not silently lost (back-fill on resume) (AC-30).
- **Reconnect storm / thundering herd** — many tabs reconnecting after a server restart must not
  hammer the server; backoff + jitter required (R-implied; **no AC for jitter/cap** — Gap 11).
- **Duplicate events** (publish-then-backfill overlap, or at-least-once pub/sub) → client **dedupes**
  by request id; the feed shows no doubles (AC-30).
- **Redis down / pub/sub unavailable** → mock fast path must **still serve responses** (degrade: log
  best-effort, skip live push), and the dashboard health pill must show a **degraded** state, not a
  silent dead feed (R7-adjacent; **no AC** — Gap 5, the single biggest infra gap).
- **Redis down also breaks state, Auto-CRUD, rate-limit** (all Redis-backed). Define per-feature
  degradation: does a state-gated rule fail open or closed? Does Auto-CRUD 503? (Gap 5/OQ-6.)
- **Wrong-channel leakage** → a `/ws/<A>` socket must never receive `<B>`'s events (AC-32);
  failure here is a cross-tenant data leak.
- **Browser tab backgrounded / laptop sleep** → socket silently half-open; client needs a
  heartbeat/ping or visibility re-check to detect and reconnect (no AC — Gap 11).
- **Unbounded feed growth** → a long-lived high-volume tab must cap/virtualize the DOM list to the
  retention cap so the browser doesn't OOM (AC-30 mentions cap/virtualize; needs an explicit number).

### Network conditions / limits
- **Rate-limit exceeded** → `429` + `Retry-After`/rate headers, via Redis token bucket; under limit
  passes (AC-25). Edge: limit change while a bucket is mid-window; rate-limit **vs. the <10ms budget**
  (the bucket check is on the fast path). **[ARCH-GAP: 429 body, window]**
- **Chaos at 100%** → every request a random `{502,503,504}`/dropout; at 0% none (AC-26). Edge:
  chaos **+ Auto-CRUD** (does a chaos 503 still mutate the collection? define ordering), chaos +
  MITM (fail before or after forwarding?). **[ARCH-GAP: dropout semantics, ordering]** (Gap 12).
- **Latency 10000ms + client timeout** → client may give up before the response; trace still records
  served status. Latency must not block the event loop for other endpoints (must be async sleep).
- **Negative / out-of-range latency/rate/chaos** in settings → validated, clamped to bounds (config
  bounds exist for delay today, `app/models.py:40`; extend to rate/chaos).

### Tunnel
- **No tunnel connected** but a request hits a tunnel-bound `<slug>` → defined response (502/503 "no
  agent"), not a hang (AC-40 implies, **not** an explicit AC — Gap 14). **[ARCH-GAP]**
- **Tunnel control channel drops** → CLI backoff-reconnects (AC-41); in-flight public request during
  the gap → error, not infinite wait.
- **Two CLIs bind the same `<slug>`** → who wins? Reject second, or round-robin? Tunnel **auth: who
  may bind a slug** (OQ-5) — an unauth tunnel = traffic hijack (Gap 14).
- **Local service down / wrong port** → tunnel returns upstream-style error to the public caller.
- **Slow/oversized local response over the multiplexed WS** → backpressure/size limits (OQ-5).

### Operator / deploy
- **Redis container unhealthy at boot** → app must wait (`depends_on: service_healthy`), not crash
  loop (AC-42). If Redis never comes up, surface a clear failure.
- **`MOCK_DOMAIN` unset/misconfigured** → wildcard dispatch can't work; app should fail fast or fall
  back to path-only mode with a clear log (Gap 13 / OQ-14).
- **Volume missing / permissions** → SQLite write fails; must surface, not silently lose traces.
- **`docker compose down -v`** wipes data (expected); `down && up` must **not** (AC-43).

---

## Edge cases — empty, first-run, concurrency, volume, limits

- **First-run, zero endpoints:** brand-new owner — does `POST /api/session` auto-create the first
  endpoint, or land on an empty list? (Today: register/login creates a *user* with **no** endpoint;
  the list shows an empty state with "+ Create Endpoint", `templates/index.html:64`.) The rewrite's
  AC-1 says "at least one endpoint token" — confirm auto-provision (Gap 4).
- **0-trace empty feed:** dashboard with no requests yet → explicit empty state in **both** columns
  (feed: "Waiting for requests, here's your URL"; inspector: "Select a request"). Today the table
  renders "No requests captured yet" (`templates/dashboard.html:205`) — keep, adapt to split-screen.
- **Endpoint exists but expired (within owner view):** the owner's dashboard for an expired/pruned
  endpoint → "expired / re-create" state, distinct from "not found".
- **Exactly 100 traces, then the 101st:** oldest pruned; the open dashboard feed must drop the pruned
  row to stay consistent with the cap (AC-35). Define whether prune is write-time or sweep (OQ-13).
- **24h TTL boundary:** a trace at 23h59m vs. 24h01m; sweep interval granularity means "24h" is
  approximate — document the interval (AC-37/OQ-13).
- **Burst / high volume:** hundreds of req/s into one endpoint → feed must not lock the DOM (AC-30),
  fire-and-forget logging must not back up (AC-39), and the SQLite writer must batch/queue or it
  becomes the bottleneck vs. the <10ms budget (R4/OQ-4).
- **Concurrency on shared state:** two simultaneous requests both mutating the same `state.<k>` (e.g.
  two `/login`) or two `POST`s to the same Auto-CRUD collection → atomicity / race on the Redis KV
  and the CRUD array; id generation must be collision-safe (Gap 6/7).
- **Concurrent rule edits:** Q edits a rule while D's traffic is hitting it → interceptor picks up the
  change on the *next* request (AC-34); no partial-rule served.
- **Multiple dashboards on the same token:** two tabs/devices both subscribed → both get every event
  (fan-out), neither starves the other (AC-32 covers isolation, not multi-subscriber fairness).
- **Huge / binary / non-UTF-8 request body:** today body is truncated to 1 MB and UTF-8-replaced
  (`app/main.py:69`); inspector Body tab must handle binary/oversized gracefully (show "binary,
  truncated"), and the JSON/XML tree must fall back to raw for non-JSON.
- **Deeply nested / malformed JSON in inspector tree:** collapsible tree must not stack-overflow or
  hang on pathological nesting; fall back to raw text.
- **Templating with huge `{{request.body.<jsonpath>}}` echo** → output-size cap so a mock can't be
  weaponized into a giant response.
- **OPTIONS preflight storm** from a browser → auto-CORS must answer cheaply (cacheable via
  `Access-Control-Max-Age`) and not pollute the feed (decide: are preflights shown/logged? Gap 12).
- **Clock skew / timestamps:** feed/inspector use UTC ISO (today `datetime.utcnow().isoformat()`);
  FE renders relative "Xs ago" (`formatDate`) — define behavior across day/timezone boundaries.
- **localStorage disabled / private mode:** session can't persist → graceful "enable storage or
  re-enter email each visit" rather than an infinite redirect loop (today storing failure isn't
  handled).
- **Endpoint deleted while a dashboard is open on it** → open WS/feed should detect 404 on next
  fetch and show "endpoint deleted", not keep a dead live pill.

---

## Required states per screen (loading / empty / error / success)

| Screen (NEW unless noted) | Loading | Empty | Error | Success |
| --- | --- | --- | --- | --- |
| **Landing `/`** (EXISTS as `/login`) | Submit button → "Processing…", disabled | n/a (single field) | Inline field error on `422`; network-error banner; **must not** reveal email-exists | Brief "Welcome / redirecting" → `/d/<token>` |
| **Dashboard shell `/d/<token>`** | Skeleton header + "Connecting…" pill | — | "Endpoint not found / expired"; "owner mismatch → re-enter email" | URL(s) shown w/ copy; split-screen mounted |
| **Live feed (left col)** | "Connecting to live stream…" | "No requests yet — send one to <URL>" (0-trace) | "Live stream lost — reconnecting (backoff)"; **"Redis/realtime degraded"** state | Rows stream in; health pill "Live"; auto-scroll/pinned |
| **Deep inspector (right col)** | Per-row "Loading detail…" (lazy fetch) | "Select a request" placeholder | "Failed to load detail / trace unavailable"; binary/oversized body fallback | 5 tabs populated incl. Response-Served + State/Trace |
| **Rule builder modal** (EXISTS as `/m/` page, no tabs) | Saving spinner; templating/JSON preview state | New rule = blank tabs w/ defaults | Per-field validation (bad JSON, bad regex/pattern, bad status); save-failed toast | Rule persisted, appears in list, interceptor uses it |
| **Endpoint settings** (NEW) | Loading config | Defaults (CRUD off, no target, 0/0/0) | Invalid MITM URL (SSRF-blocked), out-of-range slider, save fail | Toggles/sliders persisted; effect on next request |
| **Rule list** (partial today) | Loading rules | "No rules yet — create one" | Load error | List w/ enable/disable/edit/delete |
| **Endpoint list / switcher** (EXISTS at `/`) | "Loading…" | "No endpoints yet — create your first" | Load error → bounce to landing if auth-fail | List w/ URLs, view/settings/delete |
| **Tunnel CLI (terminal)** (NEW) | "Connecting to <server>…" | — | "Auth failed / slug taken / server unreachable / local :port refused"; reconnecting w/ backoff | "Tunnel live: <slug>.<domain> → :3000"; per-request log lines |
| **Operator / compose (terminal)** (NEW) | "Waiting for Redis healthy…" | — | "Redis unhealthy / volume error / MOCK_DOMAIN unset" | All healthchecks green; app serving |

---

## PRD gaps — concrete, numbered; what the PM MUST add or clarify

> Ordered by severity. Each names the missing **state / flow / AC**. Many tie to existing
> `[ARCH-GAP]`/OQ markers but are called out here because they are **user-facing flows with no AC**,
> not just schema unknowns.

1. **(IDOR / token-leak — headline.)** AC-1/2 only cover the *happy* session path. There is **no AC
   for the negative cross-owner case**: "owner B requesting `/api/endpoints/<A's token>/*` gets
   403/404 and is never served." Add it. Also clarify, in the access section, **what `<token>` in the
   public mock URL reveals** — today the URL embeds the owner id (`/hook/{user_id}/…`); the rewrite
   must guarantee the public mock token does **not** expose the owner identity. (Ties OQ-1/2, R2.)

2. **(Expired-but-known endpoint.)** AC-7 covers an *unknown* token only. Add ACs/states for: a mock
   request to a **known-but-expired/pruned/deleted** endpoint (distinct status + body), and the
   **owner viewing their own expired endpoint** in the dashboard ("expired / re-create" state, not
   "not found"). Today TTL deletes the endpoint row entirely — define expired vs. never-existed.

3. **(Reserved subdomains / apex routing.)** Plane isolation (AC-6) doesn't address the **bare apex,
   `www`, or reserved subdomains**. Specify the reserved-name list (`www`, `api`, `app`, `static`,
   the dashboard host) and that a generated `<token>` can never collide with them, plus what
   `https://<MOCK_DOMAIN>/` (no subdomain) serves. Without this, the mock catch-all can shadow the UI.

4. **(First-run provisioning + endpoint list/switcher.)** AC-1 says "at least one endpoint token" but
   never states **who creates the first endpoint** (auto on first session, or empty list + create?).
   And §3 redirects `/` to `/d/<token>`, **deleting today's endpoint-list home** — but owners can have
   **multiple** endpoints. The PRD must define: (a) auto-provision the first endpoint on session, and
   (b) where the **endpoint list / switcher** lives now that `/` is gone (a `/d` index? an in-dashboard
   dropdown?). This is a real flow with no screen assigned.

5. **(Redis-down degradation — biggest infra gap.)** Redis backs **pub/sub, state, Auto-CRUD,
   rate-limit, and possibly the rule cache**. There is **no AC and no state** for "Redis is down."
   Specify per-feature degradation and a user-visible state: does the **mock fast path still serve**
   (AC-38 implies yes — confirm it survives Redis loss)? Does the dashboard show a **"realtime
   degraded"** pill instead of a silently dead feed? Do state-gated rules **fail open or closed**?
   Does Auto-CRUD/rate-limit return 503? (Ties OQ-6, R7.)

6. **(State lifecycle: reset, scope, concurrency.)** AC-8/9/10 cover set/gate/isolate but leave open:
   the **state-reset UX/flow** (AC-10 calls it "architect-defined" — the PM must say it's a
   user-facing button/endpoint and where), the **scope** (per-endpoint vs. per-session/per-caller —
   matters for multi-user demos), **TTL/expiry of state**, and the **concurrent-mutation** semantics
   (two simultaneous `/login`). Add ACs for reset and for concurrent writes.

7. **(Auto-CRUD unhappy paths.)** AC-12 is happy-lifecycle only. Add ACs/states for: invalid/empty/
   non-JSON `POST` body, `PUT`/`DELETE` on a **missing id**, **id collision/strategy** under
   concurrency, **content-type** handling, **per-collection size cap / pagination** on large `GET`,
   and where collections persist (Redis vs. SQLite — durability across restart, OQ-6).

8. **(MITM safety: SSRF, timeout, size, redirects.)** AC-17 is a single "upstream fails → error"
   line marked `[ARCH-GAP]`. Promote the **SSRF policy to an explicit AC** (block private/link-local/
   loopback/metadata IPs, scheme allow-list, redirect handling — negative test that
   `http://169.254.169.254/...` is refused), plus ACs for **timeout value**, **oversized/streaming
   response** handling (truncate vs. stream vs. reject), and **header stripping** (upstream CORS/
   `Set-Cookie`/hop-by-hop). (Ties OQ-3, R3.)

9. **(Rule precedence & multi-match ordering.)** AC-13 covers rule-vs-CRUD-vs-MITM only. With NEW
   path/header/body matching, **multiple rules can match one request** — the PRD doesn't define
   ordering (priority field? first-match? specificity?). Add an AC: deterministic, documented order
   among multiple matching rules, surfaced in the trace tab. (Ties OQ-10.)

10. **(`request-stream.js` resilience details + backoff cap/jitter.)** AC-30 says "exponential
    backoff + dedupe + non-blocking," but omits: **max backoff cap**, **jitter** (to prevent the
    reconnect-storm in this doc's failure section), **heartbeat/ping** to detect half-open sockets
    after tab-sleep, **resume/back-fill** semantics so events during the gap aren't lost, and the
    **explicit feed DOM cap** number. Add these as sub-ACs — they are exactly what implementations
    forget and what separates this from today's naive 3s reconnect.

11. **(WS health states are under-specified.)** AC-29 names "connected vs reconnecting/disconnected"
    but the journey needs a **fourth state: "realtime degraded" (Redis down / publishing but pipe
    unhealthy)** and a **"endpoint gone"** state (404 on refresh while open). Enumerate the full health
    state machine so design/FE build all of them.

12. **(Chaos + CORS + ordering semantics.)** OQ-12 lumps these; the PM must decide the **user-visible**
    behavior: (a) does a **chaos failure still mutate** state/Auto-CRUD (ordering of chaos vs.
    side-effects)? (b) is **chaos applied before or after MITM** forwarding? (c) are **OPTIONS
    preflights logged/shown in the feed** or suppressed? (d) the `Allow-Credentials`-vs-wildcard stance
    (browsers reject `*` + credentials). Each changes what the developer sees and must be an AC.

13. **(`MOCK_DOMAIN` UX & local-dev recipe.)** OQ-14 is open. The PM must commit to the **canonical
    example URL shown in the UI**, the documented `*.localhost`/`nip.io` recipe, and the **fail-fast vs.
    path-only fallback** behavior when `MOCK_DOMAIN` is unset/misconfigured. Without this, the dashboard
    can't render a correct copyable URL (today it just uses `window.location.origin`).

14. **(Tunnel: no-agent, auth, slug contention.)** AC-40/41 cover the happy forward + reconnect only.
    Add ACs/states for: **no tunnel connected** → defined 502/503 (not a hang); **tunnel auth — who may
    bind a slug** (unauth bind = traffic hijack, OQ-5); **two CLIs binding the same slug**; **local
    service down**; and whether **tunneled traffic still appears in the live feed**. These are the
    tunnel's real failure surface.

15. **(Inspector data availability vs. fire-and-forget.)** Fire-and-forget logging (AC-39) means a
    just-served request may appear in the **live feed (from pub/sub) before its full trace is in
    SQLite**. The PM must define the inspector's behavior when a clicked row's detail **isn't written
    yet** (retry/spinner/"detail pending") so the happy path in step 9 doesn't 404 under load.

16. **(Replay / re-send — confirm scope.)** Beeceptor users expect to **replay a captured request**
    against the mock or upstream; today's app has no replay, and no AC mentions it. The PM must
    explicitly include or exclude replay (and any "edit-and-resend") so it isn't silently assumed.

17. **(Export/restore decision — OQ-7 is user-facing.)** OQ-7 ("does export/restore survive") is
    flagged as PM-owned but unresolved, and it removes a nav item users see today (`/backup`). The PM
    must decide in/out and, if in, the minimal flow + empty/error states; if out, confirm the nav link
    and pages are removed (the journey assumes removed).

18. **(Empty/loading/error states are not enumerated in the PRD.)** The PRD has rich ACs but **no
    explicit per-screen state matrix**. Lift the "Required states" table above into the PRD (or a UX
    addendum) so every NEW screen (split-screen feed, deep inspector, rule-builder modal, endpoint
    settings, tunnel CLI output, operator compose) has defined loading / empty / error / success
    states — these are the parts that ship broken when only the happy path is specced.

---

### Biggest gaps (summary)
The three most dangerous omissions are all **unhappy paths with no AC**: (1) the **cross-owner
IDOR/negative-auth** case (the public mock token still embeds the owner id today, and there's no AC
that owner B is refused owner A's endpoints); (2) **Redis-down degradation** — Redis silently backs
pub/sub, state, Auto-CRUD, and rate-limit, yet no AC or user-visible state says what happens when it
falls over (does the mock path survive, do gated rules fail open/closed, does the feed show
"degraded"?); and (3) **MITM SSRF + timeout/oversize** safety, which is a single hand-wavy `[ARCH-GAP]`
line for what is the platform's biggest attack surface. Close behind: the PRD never says **who creates
the first endpoint or where the multi-endpoint switcher lives** now that `/` is gone, and it omits a
**per-screen empty/loading/error state matrix** for every NEW screen — the exact scaffolding
implementations skip.
