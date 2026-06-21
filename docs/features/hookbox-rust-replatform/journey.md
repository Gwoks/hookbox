# User journey: HookBox — Rust/Axum + SQLite Re-platform

Scope note: this maps the end-to-end flows the re-platform implies, grounded in
the *current* Python behavior (verified in `app/routes/*.py`, `app/websocket.py`,
`app/routes/tunnel.py`) and the *current* screens (`templates/index.html`,
`templates/dashboard.html`, `templates/partials/*`). The SPA must preserve these
flows even though it is a fresh React build. Screens that exist today: **landing /
email gate** (`/`), **split-screen dashboard** (`/d/<token>`) with an **endpoint
bar** (switcher · +New endpoint · Auto-CRUD toggle · Rules · +New Rule · Settings),
the **5-tab rule modal**, the **rules-manager overlay**, the **endpoint-settings
overlay** (incl. Danger zone), and the **deep inspector** (5 tabs). The **tunnel
CLI** has no GUI today — it is terminal-only. No other screens exist; anything
beyond these is genuinely new and must be flagged (see PRD gaps).

---

## Primary (happy) flow

1. **Land.** User opens the app host `/`. SPA boots; before paint it reads
   `localStorage.hookbox_owner`. No stored owner → show the email gate (brand
   lockup, single email field, "Get my endpoint").
2. **Submit email.** `POST /api/session {email}` → `200 SessionResponse
   {owner_id, owner_secret, endpoints[], primary}`. The shape is identical for
   new vs returning emails (anti-enumeration, AC-1). A brand-new owner is
   auto-provisioned one endpoint returned as `primary` (AC-3).
3. **Persist + route.** Store `{owner_id, owner_secret, token, mock_url, email}`
   to localStorage; navigate to `/d/<primary.token>`.
4. **Dashboard boots.** Read owner from storage; resolve token from URL. Fire
   `GET /api/endpoints/{token}` (name/mock_url/path_url), `GET /api/endpoints`
   (switcher list), and open the live feed `GET /ws/{token}?cap=<secret>`. Server
   sends `hello{token, server_time}`; pill goes **Live**. Back-fill the historical
   window via `GET /api/endpoints/{token}/requests?limit=100` and reconcile.
5. **Copy the mock URL.** User copies the wildcard URL (`https://<token>.<MOCK_DOMAIN>`)
   or the local fallback (`/e/<token>`) from a copy-only chip.
6. **Build a mock rule.** Open the 5-tab rule modal (+New Rule):
   - **Matching** — method (ANY/verb), path (exact / `:param` / trailing `/*`),
     match headers, match query, body conditions (`eq/neq/contains/exists`),
     state requirements (`eq/neq/exists/absent`).
   - **Response** — status (100–599), Content-Type, response headers (JSON
     object), body template.
   - **Templating** — tag palette appends into the shared body field.
   - **Actions** — state writes (key/value, may contain tags); webhook section
     present but **disabled** ("coming soon", serialized but no-op).
   - **Throttling** — name, priority (lower wins), latency override, rate-limit
     override, enabled toggle.
   Save → `POST /api/endpoints/{token}/rules` (`201`). Rule list refreshes.
7. **Hit the mock surface.** User sends a request to `<token>.<MOCK_DOMAIN>/<path>`
   (or `/e/<token>/<path>`). Engine resolves: OPTIONS preflight → matching rule →
   Auto-CRUD → tunnel → MITM → default; wraps in rate-limit → chaos → latency.
   Response carries `X-HookBox-Endpoint` / `X-HookBox-Served-By` (+ `Rule-Id`).
8. **Watch it live.** The served request is logged + published off the response
   path; a `new_request` frame lands on the WS, a new row flashes into the
   left-hand feed (newest-first, capped at 100, "Showing N of last 100").
9. **Inspect.** Click the row → `GET /api/requests/{id}` → right-hand inspector
   shows Headers · Query · Body · Response Served (with `X-HookBox-*`) · State &
   Tracing (step list + state snapshot).
10. **Tune behavior.** Open Settings → set proxy `target_url`, toggle
    Auto-CRUD / Auto-CORS, set latency / rate-limit / chaos, choose default mode
    (`mock_404` / `echo`). Save → `PATCH /api/endpoints/{token}` (`200`),
    `endpoint_updated` frame refreshes the open dashboard.
11. **Success state.** Feed streams continuously; inspector reflects each trace;
    settings/rules persist; the owner can resume any time by re-entering the same
    email (which rotates the secret).

## Alternate flows

- **Returning owner auto-resume.** Stored owner with a token → landing redirects
  straight to `/d/<token>` without showing the form (current `index.html init()`).
- **Re-enter email to rotate / recover.** Re-submitting the same email mints a
  **new** secret and invalidates the old one; the SPA must overwrite the stored
  secret. Any *other* tab/device still holding the old secret is now `401`.
- **Multi-endpoint.** Switcher (`GET /api/endpoints`) → select another token →
  navigate `/d/<other>`. "+New endpoint" → `POST /api/endpoints` (`201`) → route
  to the new token.
- **Auto-CRUD path.** With Auto-CRUD on and no rule match: `POST /<coll>` (201,
  server uuid), `GET /<coll>` (array), `GET/PUT/PATCH/DELETE /<coll>/<id>`,
  `HEAD` mirrors GET. Owner can peek (`GET …/collections/{name}`) and clear it.
- **Stateful multi-step.** `POST /login` rule writes `authenticated=true`; a
  `/dashboard` rule with `state_requirement authenticated eq true` then matches.
  State is per-endpoint, shared across callers; clearable from the Danger zone.
- **MITM proxy.** `target_url` set, request unmatched → forwarded upstream,
  response labeled `served_by="mitm"`; a matching local rule still wins.
- **Auto-CORS.** Browser preflight `OPTIONS` → `204` with reflected origin +
  headers; every P1 response carries `Access-Control-Allow-Origin`.
- **Tunnel CLI.** Operator runs `tunnel --port 3000 --endpoint <slug> --secret
  <owner_secret>`; CLI opens `/ws/tunnel/{slug}` with `Authorization: Bearer`,
  receives `{t:"bound"}`, forwards public traffic to localhost, response replayed
  to the caller, labeled `served_by="tunnel"` in the feed.
- **Pause the feed.** Toggle Pause → incoming rows buffer; an "N new" pill
  flushes them on resume (read position preserved).
- **Inspector body modes.** Pretty (collapsible JSON tree, expand/collapse all) ↔
  Raw `<pre>`; copy header / body / query values.
- **Path-fallback-only mode.** When `MOCK_DOMAIN` is unset, `mock_url` surfaces
  the `/e/<token>` form; no wildcard surface; startup warns, never crashes.

## Error & failure paths

- **Bad email.** `422` → inline field error, constant copy that never reveals
  new-vs-existing.
- **Session rate-limit.** `POST /api/session` over `SESSION_RATE_LIMIT_PER_MIN`
  → `429` + `Retry-After` → "Too many attempts" banner.
- **Network error on submit.** fetch throws → generic retryable banner, button
  re-enabled.
- **Session created but no endpoint.** Defensive: "no endpoint returned" banner
  (handled in current `index.html`).
- **Storage disabled (private mode).** Warn that the session can't be remembered;
  still allow submit; **no redirect loop**.
- **Not signed in on `/d/<token>`.** No stored secret → bounce to `/`.
- **Endpoint 404.** `GET /api/endpoints/{token}` → `404` → "Endpoint not found"
  card with "Back to start" (deleted/expired/foreign token).
- **Capability rejected mid-session (401).** `GET /api/endpoints/{token}` →
  `401` → bounce to `/` (secret rotated elsewhere / revoked).
- **Feed auth refused.** WS close `4401` / SSE `401` → pill **Unauthorized**;
  client does **not** hammer the gate; only retries if the stored secret changed.
- **Feed connection cap.** WS close `1013` / SSE `503` when
  `WS_MAX_CONN_PER_ENDPOINT` exceeded → must surface (currently maps to a
  reconnect attempt, not a distinct "too many connections" message — gap).
- **WS drop / half-open.** onclose → exponential backoff (250→8000ms + jitter),
  pill **Reconnecting (n)**; heartbeat ping + pong-grace forces reconnect on a
  half-open socket; after `MAX_WS_FAILS_BEFORE_SSE` (6) failures → **SSE
  fallback** (same owner gate). Tab hidden → reconnection paused; resume +
  back-fill on focus.
- **Inspector detail not yet written.** Trace is fire-and-forget; `GET
  /api/requests/{id}` may `404` briefly → **pending** "detail on its way" state +
  Retry, NOT a hard 404.
- **Inspector unauthorized / error.** `401` → "Not authorized"; other → error +
  Retry.
- **Rule save 422 / 404.** Inline server-error banner; jump to the first tab with
  a client-validation error; Save disabled until valid.
- **Settings `target_url` invalid.** Client scheme check + server `422` on PATCH
  → inline field error; Save disabled.
- **Mock surface — unknown token.** `404 {error:"unknown_endpoint"}`; **deleted**
  token → `410 {error:"endpoint_gone"}`; neither logged as a trace.
- **Mock surface — body too large.** `> MAX_INGEST_BODY_BYTES` → `413` before
  buffering.
- **Rate-limit (429).** Over-limit served path → `429` + `Retry-After`,
  `X-RateLimit-Limit/Remaining`; limiter **fails open** on internal error.
- **Chaos 5xx.** `chaos_pct` hit → random `502/503/504` (served_by `chaos`); opt-in
  `dropout` closes the connection (bounded by `CHAOS_DROP_TIMEOUT_S`).
- **MITM failures.** SSRF-blocked target → `502`; timeout → `504`; conn/DNS error
  → `502 upstream_unreachable`; body truncated → `X-HookBox-Truncated: true`.
- **CRUD caps.** Non-object write body → `400`; item-count cap / per-item byte cap
  exceeded → `400`; unsafe collection/id (or 3+ segments) falls through (not CRUD).
- **Tunnel — no tunnel / drop / timeout.** `504 {error:"no_tunnel"}`
  (`TUNNEL_REQUEST_TIMEOUT_S`), never a hang.
- **Tunnel — bad auth.** Unauthenticated / wrong-owner bind → WS close `4401`,
  never registered. CLI must surface this and **stop**, not backoff-loop.
- **Tunnel — rebound.** Second authenticated bind takes over; the prior socket
  gets `{t:"err","rebound elsewhere"}` then close `4409`. Displaced CLI must
  print a clear message and exit (not reconnect into a takeover war).
- **Tunnel — drop / reconnect.** CLI backoff-reconnects; while disconnected,
  public traffic gets `504`; the dashboard `tunnel_active` flips off via
  `endpoint_updated`.

## Edge cases

- **First run / empty endpoint.** Feed empty state: muted glyph, copy, the
  copyable mock URL, and a copy-only sample `curl` (static text, never executed).
- **No rules yet.** Rules overlay empty state: "unmatched requests use Auto-CRUD /
  proxy / default" + New Rule CTA.
- **No traces yet → first trace.** Empty feed must transition to the live row on
  the first `new_request` without a reload.
- **No selection.** Inspector "Select a request" empty state.
- **No state / no collections.** State snapshot + collection peek empty states.
- **Loading skeletons.** Feed skeleton (6 rows), settings "Loading…", rules
  "Loading…", inspector "Loading detail…".
- **Concurrency — CRUD.** Concurrent writes to one collection must not lose
  updates (atomic read-modify-write in one SQLite transaction); two POSTs get
  distinct server uuids.
- **Concurrency — state.** Per-endpoint state shared across all callers; a
  `state_write` rendered **before** the body so `{{state.k}}` sees the just-written
  value in the same response.
- **Concurrency — feed fan-out.** Many subscribers per endpoint; slow/dead client
  dropped (per-client `WS_SEND_TIMEOUT_S`) without stalling the broadcast.
- **Concurrency — secret rotation.** Two open tabs; one re-submits the email and
  rotates the secret; the other tab's next `/api/*` call `401`s and its feed
  `4401`s. The active tab must update its stored secret and survive.
- **Large volume.** Feed capped at 100 (write-time prune + sweep); burst arrivals
  coalesced via rAF; DOM never grows unbounded; 24h TTL reaps old traces/state/CRUD.
- **Large / binary bodies.** Inspector shows "[binary, truncated]"; trace bodies
  truncated at `MAX_BODY_BYTES`; large JSON tree capped (≤5000 nodes).
- **SSTI / XSS probes.** `{{ 7*7 }}` etc. returned verbatim (unknown tag);
  captured values rendered as text nodes only (no `x-html`/`innerHTML`).
- **Template limits.** Template > `TEMPLATE_MAX_SIZE` returned unrendered; at most
  `TEMPLATE_MAX_TAGS` substitutions.
- **Token case.** Subdomain label case preserved when resolving the token.
- **`MOCK_DOMAIN` unset.** Path-fallback-only; `mock_url` shows `/e/<token>`.

## Required states (per screen)

- **Landing / email gate** — idle · submitting ("Setting up…") · field-error
  (422) · banner-error (429 / network / no-endpoint) · storage-unavailable warn ·
  auto-resume redirect.
- **Dashboard shell** — loading · not-found (404 card) · not-signed-in (bounce) ·
  loaded.
- **Live feed** — loading skeleton · empty ("No requests yet" + mock URL +
  sample) · streaming · paused (+ "N new") · WS pill {connecting · live ·
  reconnecting(n) · degraded · unauthorized · offline} · connection-cap refused
  (needs distinct state — gap).
- **Inspector** — empty (no selection) · loading · **pending** (detail not yet
  written, retry) · unauthorized · error (retry) · ready (5 tabs, each with its
  own empty sub-state).
- **Rules overlay** — loading · empty · error · list · per-row enable/disable/
  delete (delete confirm).
- **Rule modal (5 tabs)** — per-field validation + red tab dots · footer
  "N fields need attention" / "Ready to save" · saving · server-error (422/404).
- **Settings overlay** — loading · error · loaded form · save-error · saving ·
  target-url field error · Danger-zone confirms (clear state / clear history).
- **Tunnel CLI (terminal)** — connecting · bound · forwarding · unauthorized
  (4401, stop) · rebound (4409, exit) · disconnected/reconnecting (backoff). **No
  GUI surface for tunnel state today** beyond the `tunnel_active` flag.

---

## PRD gaps (the deliverable — fold these into the PRD)

1. **No per-screen state matrix in any AC.** The PRD enumerates behavior but never
   mandates the loading / empty / error / pending / success states the SPA must
   render. Add explicit ACs for: landing (submitting / 422 / 429 / network /
   storage-disabled / auto-resume), dashboard shell (loading / 404 not-found /
   not-signed-in bounce), feed (skeleton / empty / streaming / paused), inspector
   (empty / loading / **pending** / unauthorized / error / ready), and the rules /
   settings / rule-modal overlays. These exist in the Python UI today and will
   silently regress in a fresh React build if not specified.

2. **Inspector "detail not yet written" (pending) state is unspecified.** AC-59
   makes the trace write fire-and-forget, and AC-44 specifies `GET
   /api/requests/{id}` — but the PRD never says what the UI does when that GET
   `404`s *because the trace isn't persisted yet*. The current app shows a
   **pending + retry** state, NOT a hard 404. Add an AC: a freshly-streamed
   `new_request` whose detail GET 404s must show "detail on its way" + retry, and
   the API/architect must decide whether a short-lived `404` here is acceptable or
   the row id should be withheld until persisted.

3. **WS reconnect / SSE-fallback / heartbeat behavior is not an AC.** AC-41/42/43
   cover auth, isolation, and caps but never the **client** resilience contract
   that exists today: exponential backoff with jitter, the WS-pill states
   (connecting / live / reconnecting(n) / degraded / unauthorized / offline),
   half-open detection via heartbeat, pause-reconnect-while-hidden + back-fill on
   focus, and the threshold (6 fails) to switch to SSE. Without an AC this is the
   single most likely thing to be dropped or reimplemented incorrectly.

4. **Connection-cap (`1013`/`503`) has no distinct user-facing state.** AC-43
   defines the refusal but the PRD/UI never distinguishes "too many connections"
   from an ordinary reconnect. Specify the pill/message for cap-refused so a user
   on their 51st tab isn't told "reconnecting" forever.

5. **Secret-rotation mid-session is unspecified end-to-end.** AC-2 rotates the
   secret server-side, but the PRD never states the *journey* consequence: open
   tabs/devices holding the old secret hit `401` on `/api/*` and `4401` on the
   feed. Add ACs for (a) the active tab overwriting its stored secret on re-submit,
   (b) a stale tab's graceful bounce-to-landing on `401`, and (c) the feed's
   "unauthorized, do not hammer, retry only if stored secret changed" rule (which
   exists in `request-stream.js` today).

6. **Tunnel CLI UX states (4401 / 4409 / no_tunnel / backoff) are server-only.**
   AC-49/50/51 describe the wire protocol but not the **operator experience**:
   what the CLI prints/does on auth failure (`4401` → stop, don't loop), on being
   rebound (`4409` → exit cleanly), on disconnect (backoff reconnect), and how the
   dashboard reflects `tunnel_active` flipping. There is no GUI for the tunnel
   today — confirm whether the SPA gets a tunnel-status indicator or it stays
   terminal-only, and specify the CLI's stdout contract as ACs.

7. **`410 endpoint_gone` vs `404 unknown_endpoint` depends on the unresolved
   OQ-1.** The whole "deleted endpoint" journey (user deletes, then traffic still
   arrives at the dead token → `410`) hinges on the tombstone mechanism the
   architect hasn't chosen. If OQ-1 resolves to "deleted = 404", the
   "Endpoint not found" card copy and the AC-57 distinction both change. Flag that
   the journey for delete-then-hit is **blocked** on OQ-1; the dashboard has no
   delete-endpoint UI today (only delete via `DELETE /api/endpoints/{token}` API)
   — confirm whether the SPA exposes endpoint deletion at all, and if so add the
   confirm flow + post-delete routing.

8. **`chaos_mode="dropout"` is not in the frozen schema (OQ-2) yet AC-40 relies on
   it.** The settings overlay today exposes only chaos **percent** (random 5xx),
   not the dropout mode. Either promote `chaos_mode` to `EndpointConfigPatch`/
   `EndpointDetail` (and add the settings control + an AC) or drop dropout from
   parity. As written, AC-40 and the §5.3 schema contradict each other.

9. **Auto-CRUD and state/collections have API peek/clear but thin journey/UI.**
   AC-22/27 expose `GET/DELETE …/state` and `…/collections/{name}`, but the PRD
   never describes how a user *sees* CRUD data or state in the SPA beyond the
   Danger-zone "Clear state / Clear history". There is no collection browser or
   state viewer screen today. Clarify: is peeking state/collections a journey the
   SPA must support (new screen) or API-only? If new, it must be flagged as a new
   screen, not "[existing]".

10. **`echo` default mode and `webhook_action` no-op need explicit journey copy.**
    `default_mode="echo"` returns the request reflected (200) — a real user-visible
    behavior with no AC describing the feed/inspector representation. `webhook_action`
    is "stored but no-op" (Non-goal) yet the rule modal *shows a disabled "coming
    soon" webhook section*; the PRD should state the SPA renders it disabled (not
    omitted) so the data shape round-trips, matching today's behavior.

11. **First-run / seeded-demo journey is unspecified.** AC-52 seeds demo data on
    first run, but the PRD never says what a *first-time operator* sees: is the
    seeded endpoint pre-populated with rules/traces so the empty states are
    skipped, or do they hit the genuine empty states? This materially changes the
    onboarding journey and the empty-state ACs.

12. **Offline / total-network-loss at the SPA level is unspecified.** Beyond the
    per-call banners, there is no AC for the app going fully offline (feed offline
    pill + management calls failing). Specify the global offline state and recovery
    (the feed pill already has an `offline` state to honor).

13. **Concurrency ACs are implicit.** AC-26 covers CRUD atomicity, but the PRD
    has no AC for (a) multiple feed subscribers + slow-client drop without
    stalling fan-out (exists in `websocket.py`), (b) shared per-endpoint state
    races across concurrent mock callers, or (c) the last-bind-wins tunnel
    takeover race. R2 flags the concurrency *risk* but no AC pins the observable
    contract — add them so QA can test them.
