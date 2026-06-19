# Security review (design): HookBox → Beeceptor-class rewrite (slug: beeceptor-rewrite)

> **Mode:** DESIGN (Stage A). No rewrite code exists yet. This threat-models the
> draft `prd.md` (§4 ACs, §5 provisional contract) against the *prior-art* code
> in `app/` + `templates/` that the rewrite **replaces**, governed by LOCKED
> `_decisions.md`. Output is consumed by the PM (fold required ACs into PRD §4)
> and the system-architect (fold contract notes into the frozen §5). I file **no
> bugs** here — nothing is built. The required ACs below are the gate the
> REVIEW-stage pass will hold the implementation to.
>
> **Honoring LOCKED constraints:** no password/registration/OTP wall is proposed
> (`_decisions.md` §5). Email stays the access key. Everything below hardens that
> model rather than replacing it. Auto-CORS stays wide-open *by spec* (§1.4); the
> findings below make wide-open CORS *safe* by forbidding credentialed CORS and
> isolating the management plane from it — they do not narrow the mock-surface
> origin policy.

---

## 1. Threat model — assets, trust boundaries, who can reach what

### 1.1 Assets (what an attacker wants)
- **A1 — Cross-tenant endpoint config & data.** Other owners' endpoints, mock
  rules, MITM target URLs, and **request logs** (which routinely contain bearer
  tokens, API keys, cookies, and PII in captured headers/bodies — the whole
  point of an interceptor is that it records secrets in transit).
- **A2 — The owner capability** (whatever authorizes `/api/*` mutation). If it is
  derivable from an email (today's `hash_email`) it is not a secret at all.
- **A3 — Server-side execution / file/network reach.** Via SSTI in the new
  templating engine (RCE), or SSRF in the new MITM forwarder (cloud metadata,
  internal services), or the tunnel control channel.
- **A4 — Service availability.** The fast path, Redis, SQLite, WS fan-out, and the
  new abuse-amplifiers: chaos/latency knobs, MITM (server as a proxy/DDoS
  reflector), Auto-CRUD (unbounded in-store arrays), retained traces.
- **A5 — Operator secrets.** `MOCK_DOMAIN` is fine to leak; Redis URL/creds, any
  `.env`, SMTP creds (being removed), and internal stack traces are not.

### 1.2 Trust boundaries & the three planes (`_decisions.md` §2)
- **P1 — Wildcard mock surface** `*.<MOCK_DOMAIN>/<path>` + path-fallback
  `/e/<token>/<path>`. **Fully untrusted, unauthenticated, internet-facing by
  design.** Anyone may hit any token's mock surface (that is the product). The
  *boundary* here is: a P1 request must NEVER be able to reach P2 (management) or
  P3 (UI) handlers, must never read/mutate another endpoint's **config/rules**
  (only exercise the mock), and its templating must never escape its sandbox.
- **P2 — Management API** `/api/*` on the app host. **The crown jewels.** Every
  mutation of endpoints/rules/config and every read of logs lives here. This is
  the IDOR/ownership boundary.
- **P3 — UI + static** `/`, `/d/<token>`, `/static/*`. Server-rendered Jinja. It
  renders **attacker-controlled captured request data** (headers/body/path) into
  the inspector — the stored-XSS boundary.
- **Plane-confusion is itself a vulnerability.** The middleware that decides
  "is this Host+path P1, P2, or P3?" is security-critical: a mock catch-all that
  can shadow `/api` (or vice-versa) breaks every other control. (Mirror of the
  prior art's `/hook/{user_id}/{endpoint_id}` catch-all colliding with real
  routes.)

### 1.3 Actors
- **Anonymous internet client** — can hit any P1 mock surface; can POST `/api/session`
  with any email; can open the WS/SSE pipe.
- **A legitimate owner** — holds their own capability; must be confined to their
  own tenant on P2/P3 and their own channel on the real-time pipe.
- **A malicious owner** — has a valid capability for *their* endpoint and uses the
  *intended* features (rules, templating, MITM target, chaos, Auto-CRUD, tunnel)
  as weapons against the server and third parties. **This is the primary actor**
  for SSTI/SSRF/DoS — they are inside the front door by design.
- **A network attacker** — sees cookies/headers if `Secure`/`HttpOnly` are wrong;
  but note this app is largely token-in-localStorage, so cookie flags matter only
  if the architect introduces any cookie/session.

### 1.4 Confirmed prior-art vulnerabilities the rewrite MUST NOT inherit (evidence)
These are the patterns to design *away from*; the design-time risk is "the
rewrite copies them" (PRD §8 R2/R3). Cited so the architect sees the anti-pattern:
- **Header-trust IDOR.** `app/routes/api.py:24-26` and `app/routes/backup.py:19-20`:
  `get_current_user` returns `{"user_id": x_user_id}` directly from a
  **client-supplied** `X-User-Id`/`X-Email` header with **no server-side
  validation**. Any caller sets the header to any 16-hex id and reads/deletes/
  exports any owner's data. (PRD §5.2 already flags this; it blocks lock.)
- **Capability is theater.** `app/database.py:98-100` `create_user_token()` mints a
  `secrets.token_urlsafe(32)` that is **never stored and never checked** — every
  `/api` route authorizes on the *header user-id alone*. The rewrite needs a
  capability that is actually persisted and verified.
- **Owner id == email hash, low-entropy & enumerable.** `app/database.py:104`
  `hash_email` = `sha256(email)[:16]` (64-bit, deterministic). It is both the
  public id *and* (today) the sole auth token. Knowing a victim's email yields
  their owner id directly — pure enumeration into A1.
- **WebSocket has zero auth/ownership.** `app/main.py:29-39` accepts a WS for any
  `endpoint_id` and joins it to that channel (`app/websocket.py:12-16`,
  `accept()` before any check). Cross-tenant live request-stream subscription.
- **Stored XSS in the inspector.** `templates/dashboard.html:213-235` interpolates
  captured `${req.path}`, `${req.method}`, header/body JSON into `innerHTML`
  with no escaping (only `prettyJson` falls back to `escapeHtml` on parse
  failure; the happy path and `req.path` are raw). An attacker who sends a
  webhook with a crafted path/header runs JS in the owner's dashboard — and the
  owner's capability lives in that same browser's `localStorage`. The rewrite's
  inspector must escape **all** captured fields.
- **Insecure-by-design ops routes (being removed — confirm they stay gone).**
  `app/routes/webhook.py:22-49` runs `git pull` + `os.system(...)` /
  `pkill` on a webhook (RCE-class auto-deploy); `app/main.py:123-268` `/status`
  reads `/home/ubuntu/.openclaw/...` and f-string-injects file contents into
  HTML (path + reflected-content XSS + info disclosure); `app/routes/backup.py`
  has SMTP creds + an unauthenticated-by-header export of ALL user data. PRD §2
  / `_decisions.md` §10 remove these — a required AC pins their absence.

---

## 2. Findings & risks  (each: what · §5 endpoint/flow · severity · why)

| # | Finding | §5 endpoint / flow | Severity | Why it matters |
|---|---------|--------------------|----------|----------------|
| F1 | **Email enumeration via `/api/session`.** A deterministic `hash_email` owner id + a session response that differs for new-vs-existing email lets an attacker confirm which emails have endpoints, and (worse) *derive the owner id of any email*. | `POST /api/session` (§5.2) | **High** | Owner id is also the IDOR key (F2). Enumeration → targeted IDOR. Also a privacy leak (who uses the service). |
| F2 | **IDOR / broken object-level authz on `/api/*`.** If the rewrite keeps header-trust (or makes the *derivable* owner id the sole credential), any caller reads/mutates/deletes any tenant's endpoints, rules, config, and **logs (secrets)**. | All `/api/endpoints/{token}`, `/rules`, `/requests`, `/api/requests/{id}` (§5.2) | **Critical** | Direct cross-tenant read of captured bearer tokens/PII and full takeover of others' mock behavior. The single highest risk (PRD R2). |
| F3 | **Owner capability entropy/secrecy.** `hash_email[:16]` (64-bit, email-derived) is not a secret. Auth must use a **separate, high-entropy, server-stored capability**, not the public owner id. | `/api/session` issues it; all `/api/*` verify it (§5.2 "Auth model") | **Critical** | Without this, F1 + F2 are unfixable: anyone who knows an email is authenticated as that owner. |
| F4 | **SSRF via MITM "Target Real API URL".** A user-set upstream lets the server fetch arbitrary URLs: `http://169.254.169.254/...` (cloud metadata), `http://localhost:6379` (its own Redis), `http://10.x/internal`, `file://`, `gopher://`, redirects to those, and DNS-rebinding to them after a passed check. | `PATCH /api/endpoints/{token}` (set `target_url`) → P1 forward (AC-14/17, §5.3) | **Critical** | Reads cloud IAM creds / internal services; can pivot to RCE on the host's own infra. Classic interceptor SSRF (PRD R3). |
| F5 | **SSTI / template injection in the response templating engine.** `{{...}}` tags in rule bodies must be a **sandboxed mini-grammar**, not Jinja `render_template_string`/`eval`/`format()` on attacker text. A naive Jinja render of a user-controlled rule body = RCE (`{{config}}`, `{{''.__class__...}}`). | Rule `response_body` rendering on P1 (AC-20–23, §5.3) | **Critical** | A malicious owner gains code execution on the server via their own rule. Must be an allow-listed tag evaluator with **no** Python attribute/`eval` access. |
| F6 | **WS/SSE channel authz (cross-tenant feed read).** If the real-time pipe (`/ws/{token}` provisional, §5.4) accepts any token with no capability check, any anonymous client subscribes to a victim's **live request stream** — i.e. their secrets in real time. | Real-time pipe (AC-27/32, §5.4) | **High** | Live exfiltration of captured headers/bodies; bypasses even a perfect `/api` authz because it is a different transport. WS upgrades skip route auth by default. |
| F7 | **Stored XSS rendering captured request data in the inspector.** Headers/body/path/query of an intercepted request are attacker-controlled and are rendered into the P3 dashboard. Unescaped output = JS execution in the owner's session (which holds the capability). | `GET /d/<token>` inspector + `/api/requests/{id}` data (AC-31, §5.4/P3) | **High** | Confirmed pattern in prior art (`dashboard.html:213-235`). Steals the owner capability → escalates to F2. Jinja autoescape ON + escape all JSON-rendered fields. |
| F8 | **DoS via abuse-amplifier features.** (a) **MITM** turns the server into an open proxy / SSRF-amplified request engine (also a DDoS reflector toward third parties). (b) **Chaos/latency** knobs (0–10000 ms, dropouts) let a malicious owner pin worker coroutines / exhaust the connection pool. (c) **Auto-CRUD** in-store JSON arrays grow unbounded. (d) Unbounded request body / WS connections / retained traces. | P1 forward, latency/chaos/CRUD (AC-12/24/26), WS (§5.4) | **High** | A single owner degrades the whole single-instance deployment. Needs body caps, upstream timeouts + size caps + concurrency caps, CRUD item/size caps, WS-per-token caps, and the rate-limit applied to *itself* not just to mock callers. |
| F9 | **Rate-limit / token-bucket is a control AND a target.** The Redis token bucket (AC-25) must (a) be keyed so one tenant can't evict another's buckets, (b) fail **closed-enough** (a Redis outage must not make latency/MITM unbounded), and (c) the limit must also cap **MITM forwards** and **Auto-CRUD writes**, not just rule hits. | AC-25, Redis | **Medium** | A bypassable or cross-tenant-poisonable limiter re-opens F8. |
| F10 | **Wide-open CORS leaking into the management plane / credentialed CORS.** Spec mandates `Access-Control-Allow-Origin: *` on **P1** (intended). The risk is (a) that header also being emitted on **P2 `/api/*`** (then any site reads a victim's endpoints if cookies are ever used), and (b) `Allow-Origin: *` **with** `Allow-Credentials: true` (invalid + dangerous). | AC-18/19 (P1) vs P2 (§5.2), OQ-12 | **Medium** | Wide-open CORS is safe only on the public mock surface and only without credentials. P2 must NOT inherit it. |
| F11 | **Tunnel control-channel auth & abuse.** `mock-tunnel` reverse-tunnels public traffic to a dev's localhost (AC-40/41, §5.6). Who may bind `<slug>`? An unauthenticated bind lets an attacker hijack a victim's slug (intercept their public traffic) or use the tunnel as an inbound pivot. | Tunnel WS control channel (§5.6, AC-40/41) | **High** | Slug-hijack = MITM of someone else's endpoint; unauth tunnel = arbitrary inbound to a dev box. Bind must require the endpoint's owner capability. |
| F12 | **Auto-CRUD / state injection & traversal.** CRUD `:id` and `<collection>` and state-var keys come from the URL/body. If used to build Redis keys or (worse) SQL by string-building, an attacker reaches other collections/endpoints' state. CRUD bodies are stored then echoed → second-order XSS/templating. | AC-8–13, §5.5 | **Medium** | Cross-collection/cross-endpoint state read; key-injection. Must namespace every Redis key by `endpoint_token` and validate `collection`/`id`/state-key against a safe charset. |
| F13 | **`aiosqlite` SQL injection if the rewrite string-builds queries.** Prior art is mostly parameterized but `backup.py:43,53` builds `IN (...)` placeholder lists by hand and the new rich rule/match/CRUD layer is a fresh opportunity to f-string user input into SQL. | All BE DB access (§5.5) | **High (if introduced)** | Standard SQLi → full DB read/write. Cheap to prevent: 100% parameterized queries, never f-string values. A REVIEW-stage grep gate will check. |
| F14 | **Secret leakage in logs / errors / responses.** Captured traces store secrets (acceptable, it's the product) but must not echo the **owner capability**, Redis URL, or stack traces. MITM upstream errors must not reflect internal hostnames/IPs; unknown-tag/SSTI failures must fail safe (AC-23) without leaking internals; the `/status` f-string disclosure must stay deleted. | `/api/requests/{id}`, error paths, AC-17/23, AC-45 | **Medium** | Avoids handing the attacker the capability or internal topology. |
| F15 | **No request-size / header / connection caps on P1.** Prior art truncates body to 1 MB at *store* time (`main.py:69`) but reads the **full** body into memory first (`await request.body()`), and has no upstream/preflight/WS caps. Unbounded ingest = memory DoS independent of the <10 ms budget. | P1 ingest, AC-38/39 | **Medium** | A few large concurrent POSTs OOM the single instance. Enforce a max body size at ingest (reject 413), cap stored size separately. |
| F16 | **CSRF on `/api/*` state-change — scheme-dependent.** If the architect authorizes `/api` purely by a **custom header / bearer** capability (not a cookie), CSRF is structurally mitigated (browsers can't forge custom auth headers cross-site). If *any* cookie/ambient credential is introduced, every state-changing `/api` route (session, endpoint create/patch/delete, rule CRUD) needs CSRF defense. | All mutating `/api/*` (§5.2) | **Low→High (cookie-dependent)** | Calling it out so the architect's auth choice (header-bearer, recommended) is made *consciously* to keep CSRF a non-issue. |

---

## 3. Required security ACs  (PM: fold these verbatim into PRD §4; each is testable)

> Each is observable via a `curl`/probe at REVIEW time. `<cap>` = the owner
> capability the architect defines (F3). `<tokenA>/<tokenB>` = two endpoints owned
> by two different emails. These do NOT add a password wall (LOCKED §5).

**Access model — entropy, enumeration, capability (F1–F3)**

- **SEC-AC-1 (Capability is a real, separate secret).** `POST /api/session` returns
  a high-entropy owner **capability** (≥128 bits, CSPRNG) that is **distinct from**
  the public owner id and is **stored server-side** (hashed) and verified on every
  `/api/*` call. The public owner id (email-derived `hash_email`) is **never
  accepted as an authorization credential**. *(BE — verify: a request with a valid
  owner id but absent/wrong capability is rejected 401.)*
- **SEC-AC-2 (No IDOR — cross-tenant read).** With owner B's capability, every
  `GET /api/endpoints/<tokenA>`, `/rules`, `/requests`, and `GET /api/requests/<A's id>`
  returns **404** (preferred, to not confirm existence) or 403 — never owner A's
  data. *(BE)*
- **SEC-AC-3 (No IDOR — cross-tenant mutate/delete).** With owner B's capability,
  `PATCH`/`DELETE /api/endpoints/<tokenA>` and all `/rules` mutations on `<tokenA>`
  return 403/404 and leave A's data unchanged. *(BE)*
- **SEC-AC-4 (Unauthenticated `/api/*` rejected).** Any `/api/endpoints*`,
  `/rules*`, `/requests*` call with **no** capability returns 401 (not 200, not a
  silent default user). *(BE — directly closes the `X-User-Id` header-trust hole.)*
- **SEC-AC-5 (Enumeration-resistant session).** `POST /api/session` is **constant in
  shape/timing** for new vs existing emails (same status, same response schema,
  no "welcome back" vs "created" distinction observable to the client) and is
  **rate-limited** per source. A malformed email returns **422**. *(BE)*

**SSRF — MITM target (F4)**

- **SEC-AC-6 (Scheme allow-list).** Setting `target_url` to any non-`http(s)`
  scheme (`file:`, `gopher:`, `ftp:`, `data:`, etc.) is rejected at config time
  (422) and never fetched. *(BE)*
- **SEC-AC-7 (Private/link-local/metadata blocked).** A MITM forward whose target
  resolves to a loopback (`127.0.0.0/8`, `::1`), private (`10/8`,`172.16/12`,
  `192.168/16`, `fc00::/7`), link-local (`169.254.0.0/16` incl. **`169.254.169.254`**,
  `fe80::/10`), or other non-public range is **blocked** (architect-defined status,
  e.g. 502/421) and **logged**, **after DNS resolution** (block on the resolved IP,
  not just the hostname string). *(BE — verify with a target pointing at `169.254.169.254`.)*
- **SEC-AC-8 (Redirects re-validated / capped).** MITM follows at most N redirects
  (small, e.g. ≤3) and **re-applies SEC-AC-7 to every redirect hop's resolved IP**;
  a redirect to a blocked range is refused. *(BE)*
- **SEC-AC-9 (Upstream timeout + response-size cap).** MITM enforces a connect/read
  **timeout** and a **max response size**; exceeding either yields a deterministic
  error status (architect-defined) and is logged — the worker is never pinned
  indefinitely and memory is bounded. *(BE)*

**SSTI — templating engine (F5)**

- **SEC-AC-10 (Sandboxed grammar, no code exec).** The templating engine evaluates
  **only** the documented allow-listed tags (`now`, `random`, `request.*`,
  `state.*`, …). A rule body of `{{ 7*7 }}`, `{{ config }}`, `{{ ''.__class__ }}`,
  `{{ self }}`, or any Python/attribute expression renders as **inert literal or
  empty per AC-23** and executes **no** Python — verified by a probe rule whose body
  is `{{''.__class__.__mro__}}` returning that text/empty, not an object or 500.
  The engine must NOT be Jinja `render_template_string`/`Template().render` over
  user text, nor `eval`/`exec`/`str.format` on attacker input. *(BE)*
- **SEC-AC-11 (Templating fails safe & quiet).** An unknown/malformed tag never
  500s the mock path and never leaks server internals/stack traces into the
  response (ties AC-23). *(BE)*

**Real-time pipe authz (F6)**

- **SEC-AC-12 (Channel requires capability).** Opening the WS/SSE pipe for
  `<tokenA>` requires owner A's capability (passed in a way WS allows — e.g.
  subprotocol/first-message/query token validated **before** `accept()` and
  before joining the channel); an anonymous or wrong-capability subscribe is
  rejected. *(BE — closes `main.py:29` no-auth.)*
- **SEC-AC-13 (Channel isolation).** A pipe authenticated for `<tokenA>` receives
  **zero** `<tokenB>` events (strengthens AC-32 into a security assertion). *(BE)*

**Stored XSS (F7)**

- **SEC-AC-14 (Captured data is escaped everywhere it renders).** Sending a webhook
  whose path/header/query/body contains `<script>` / `"><img onerror=...>` and then
  opening the inspector does **not** execute script: every captured field is
  HTML-escaped (Jinja autoescape ON for server-render; JS inserts via `textContent`
  / a real escaper, never raw `innerHTML`; no `| safe` on captured data). *(FE — fixes
  `dashboard.html:213-235` class of bug.)*
- **SEC-AC-15 (Inspector JSON/headers not executable).** Body/headers rendered as
  formatted JSON are inserted as text nodes (or escaped) so a JSON string value of
  `</script><script>...` cannot break out. *(FE)*

**CORS (F10)**

- **SEC-AC-16 (Wide-open CORS is P1-only).** `Access-Control-Allow-Origin: *` and
  the wide-open preflight headers appear on **mock-surface (P1)** responses only;
  `/api/*` (P2) responses do **not** carry wildcard CORS. *(BE — verify both planes.)*
- **SEC-AC-17 (No credentialed wildcard).** No response ever sends
  `Access-Control-Allow-Origin: *` together with `Access-Control-Allow-Credentials:
  true`. (If reflecting Origin, credentials policy is explicit and not `*`+creds.)
  *(BE)*

**DoS / abuse amplifiers (F8/F9/F15)**

- **SEC-AC-18 (Ingest body cap).** A P1 request body over the configured max is
  rejected with **413** before being fully buffered/processed; the cap is
  configurable (`config.py`/env). *(BE)*
- **SEC-AC-19 (Rate limit also covers MITM & CRUD writes).** The Redis token-bucket
  limit (AC-25) is enforced on **MITM forwards** and **Auto-CRUD writes**, not only
  rule hits, so a malicious owner cannot use those paths to bypass throttling or
  reflect traffic at a third party. Buckets are namespaced per endpoint token (no
  cross-tenant eviction). *(BE)*
- **SEC-AC-20 (Limiter fails safe on Redis loss).** If Redis is unavailable, the
  fast path degrades to a safe default (architect-defined: bounded local fallback
  or refuse the amplifier paths) — it does **not** silently make latency/MITM/CRUD
  unbounded. *(BE)*
- **SEC-AC-21 (Auto-CRUD bounded).** Each Auto-CRUD collection has a configurable
  cap on item count and per-item/total size; exceeding it is rejected, not grown
  unboundedly. *(BE)*
- **SEC-AC-22 (WS connection cap).** Concurrent real-time connections per endpoint
  (and/or per source) are bounded; excess are refused — no unbounded fan-out
  memory growth. *(BE)*

**Injection / traversal / keys (F12/F13)**

- **SEC-AC-23 (Parameterized SQL only).** No SQL query interpolates request-derived
  values via f-string/`%`/`.format`; all values are bound parameters. *(BE —
  REVIEW gate greps for f-string/`%`-built SQL.)*
- **SEC-AC-24 (Namespaced, validated keys).** Every Redis key (state, CRUD,
  buckets, pub/sub) is namespaced by the endpoint token, and user-supplied
  `collection`/`:id`/state-key are validated against a safe charset so they cannot
  inject a separator to reach another endpoint's/collection's namespace. *(BE —
  verify endpoint A's state is invisible to B even with a crafted key; ties AC-10.)*

**Secret hygiene & cruft removal (F14 + LOCKED §10)**

- **SEC-AC-25 (No secret/internal leakage).** The owner capability, Redis URL/creds,
  any `.env` value, and Python stack traces never appear in any HTTP response,
  rendered template, error body, or the captured-trace API output. Unhandled errors
  return a generic message (no debug). *(BE)*
- **SEC-AC-26 (Insecure cruft absent).** The `/status` crypto route, all
  `openclaw`/`holdings`/`fear_greed`/`restart_service` references, the GitHub
  auto-deploy webhook (`os.system`/`git pull`/`pkill` — RCE-class), and the SMTP
  backup are **absent** from shipped code (grep returns nothing). *(BE — overlaps
  AC-45; restated as a security AC because these are RCE/secret-exposure, not just
  cleanliness.)*

**Tunnel (F11)**

- **SEC-AC-27 (Tunnel bind is authenticated).** Binding `mock-tunnel` to `<slug>`
  requires that endpoint's owner capability; an unauthenticated or wrong-owner bind
  is rejected. A second binder cannot hijack an already-bound slug without the
  owner capability. *(BE — closes the slug-hijack/inbound-pivot vector.)*

---

## 4. §5 contract notes  (system-architect: fold into the frozen contract)

These resolve the security-tagged open questions (OQ-1/2/3/5/12) and set the
shapes the frozen §5 must carry. They do not pick the *implementation*, but they
constrain it.

1. **Auth model (resolves OQ-1, OQ-2 — the lock-blocker).**
   - Public **owner id** = `hash_email` (sha256→hex) is fine **as a non-secret
     identifier only**. Do not widen its entropy expecting it to be secret — it is
     derivable from the email by design; instead make it never an auth credential.
   - **Capability token:** `POST /api/session` issues a CSPRNG token ≥128 bits
     (`secrets.token_urlsafe(32)` is fine) **stored server-side hashed** (sha256)
     and bound to the owner id. Returned once to the client (kept in localStorage
     per LOCKED §5). **Recommended transport: `Authorization: Bearer <cap>`** (a
     custom/non-cookie header) so CSRF (F16) is structurally a non-issue.
   - **Per-route ownership check:** a single dependency replaces `get_current_user`
     (`api.py:24`): resolve owner-from-capability, then for any `/api/endpoints/{token}/*`
     assert the endpoint's `owner_id == caller`. Default to **404** on mismatch
     (avoids existence oracle). Contract: define **401** (missing/invalid cap) vs
     **404** (valid cap, not your object) and apply it uniformly.
   - **Resume (AC-2)** must NOT leak new-vs-existing (SEC-AC-5): identical response
     shape + status either way.

2. **Status-code matrix the contract must pin (security-relevant):**
   - `/api/session`: 200 (issued, identical for new/resume) · 422 (malformed email)
     · 429 (rate-limited).
   - `/api/*` ownership: **401** no/invalid capability · **404** valid capability /
     not your object (preferred over 403 to avoid confirming existence; architect
     picks one and applies it everywhere).
   - MITM: blocked-target → architect-defined (suggest **502** "bad gateway, target
     refused") · timeout/oversize → **504**/**502** deterministically (AC-17).
   - Rate limit → **429** + `Retry-After` (AC-25). Ingest oversize → **413**
     (SEC-AC-18). Unknown mock token → deterministic **404** (AC-7), not logged as a
     served mock.

3. **SSRF policy (resolves OQ-3) — the contract must state:** http(s) only;
   block-list applied to the **resolved IP(s)** (loopback/private/link-local/
   ULA/metadata `169.254.169.254` + IPv6 equivalents) at request time **and on each
   redirect hop**; max redirects (small); connect+read timeout; max response bytes;
   strip hop-by-hop and sensitive inbound headers before forwarding (reuse
   `format_headers` hop-by-hop set in `app/utils/helpers.py:19`, and additionally do
   **not** forward the owner capability upstream). Document whether a configurable
   allow-list overrides the block-list (default: block-list on, no allow-list).

4. **Templating engine contract (resolves OQ-8 with security teeth):** the §5 tag
   grammar must be an **explicit allow-list evaluated by a custom resolver**, not a
   general template engine over user text. No tag may reach Python attributes,
   builtins, `eval`/`exec`/`format`, the filesystem, or env. Unknown/malformed tag
   behavior (AC-23) defined as inert-literal-or-empty, never 500/leak. State the
   max template size / max tag count (DoS bound).

5. **Real-time pipe (resolves OQ-11 + OQ-5 security half):** the §5.4 contract must
   specify **how the capability is presented on the WS/SSE handshake** (subprotocol,
   first-message, or short-lived signed channel ticket) and that it is verified
   **before `accept()`/before channel join**. Channel name namespaced by endpoint
   token. SSE fallback (if chosen) needs the same gate. Tunnel control channel
   (§5.6) likewise: bind requires the owner capability (SEC-AC-27); define behavior
   when no tunnel is connected (don't hang the public caller — timeout to a
   deterministic status).

6. **CORS contract (resolves OQ-12 security half):** wide-open
   (`Allow-Origin: *`, `Allow-Methods: *` or the standard verb set,
   `Allow-Headers` reflected/`*`, a sane `Max-Age`) is emitted **only on P1**.
   **Never** `*` + `Allow-Credentials: true`. **P2 `/api/*` emits no wildcard CORS.**
   The plane-dispatch middleware is the right place to attach P1 CORS so it cannot
   bleed into P2/P3.

7. **Validation rules the contract should carry (pydantic v2):** email →
   `EmailStr` (422 on fail, as today); `target_url` → validated URL with the
   scheme/host policy above (not just `str`); `latency` clamped 0–10000;
   `rate_limit` ≥0 bounded; `chaos` 0–100; `collection`/CRUD `:id`/state-keys →
   constrained pattern (e.g. `^[A-Za-z0-9_-]{1,64}$`); rule `response_body` and
   request body → max-size bound. Numeric clamps are both UX and a DoS control
   (SEC-AC-18/21).

8. **Plane-dispatch is security-critical (LOCKED §2):** the middleware decision
   (Host + path → P1/P2/P3) must be unambiguous and ordered so the P1 mock
   catch-all can never match `/api`, `/d`, `/static`, or `/ws` on the app host, and
   `/api` under a mock Host is treated as mock traffic (AC-6). Document the exact
   precedence in §5.1.

---

## 5. Open security questions  (must be resolved before §5 lock)

- **SOQ-1 (→ OQ-1/2).** Confirm the capability transport: **Bearer header**
  (recommended, CSRF-free) vs cookie (would require CSRF tokens on all mutating
  `/api/*`). If cookie, also pin `HttpOnly`/`Secure`/`SameSite=Lax|Strict`. Which?
- **SOQ-2 (→ OQ-1).** 401-vs-404 policy for "valid capability, not your object":
  pick one (recommend **404** as existence-oracle defense) and apply uniformly.
- **SOQ-3 (→ OQ-3).** Is a per-endpoint **allow-list** for MITM targets in scope, or
  is the IP block-list the sole control? (Default: block-list only.) And is MITM to
  *public* DNS that later rebinds to private handled by resolve-then-pin, or
  re-resolve-and-check per hop?
- **SOQ-4 (→ OQ-5).** Tunnel: besides authenticated bind (SEC-AC-27), must public
  traffic into a tunnel respect the same body/rate caps, and is there any limit on
  *what* the tunneled local service can be (it's the dev's own box, but the server
  is the relay — reflection concerns)?
- **SOQ-5 (→ OQ-12).** Chaos "dropout" semantics: a raw **connection drop** vs a
  502/503/504 status. A real socket drop is a stronger resource/abuse vector
  (clients + our workers hang) — bound it (still subject to timeouts) or restrict
  chaos to status-only. Architect to decide; security needs the answer to size F8.
- **SOQ-6.** Redis durability (OQ-6) intersects security: if state/CRUD/buckets are
  Redis-only and Redis restarts, do **rate-limit buckets reset to empty** (a free
  burst window)? Acceptable, or do we need a floor? Note for the limiter design.
- **SOQ-7.** Export/restore survival (OQ-7): if it survives, `restore` must enforce
  ownership on **every** imported row (prior art `backup.py:91` checks
  `ep["user_id"] == user_id` but `INSERT OR REPLACE`s attacker-chosen `id`s — a
  tenant could overwrite/auto-create rows; the rewrite must bind owner from the
  capability, ignore client-supplied owner ids, and not allow id-collision
  overwrite of another tenant). Security must review the final shape.

---

## 6. Summary for the orchestrator
- **Threat model:** 3 planes (untrusted P1 mock surface · crown-jewel P2 `/api`
  authz boundary · stored-XSS P3 dashboard); primary actor is a *malicious owner*
  weaponizing intended features (SSTI/SSRF/DoS) plus an anonymous attacker doing
  IDOR/enumeration/channel-eavesdrop.
- **Top risks (critical):** (1) **IDOR + theatrical capability** — prior art trusts
  a client `X-User-Id` header and mints an unverified token (`api.py:24`,
  `database.py:98-104`); the rewrite must issue a real, server-stored,
  high-entropy capability and check ownership on every `/api/*` route. (2) **SSRF**
  in the user-set MITM target URL (metadata/internal/loopback) — block on resolved
  IP, every hop, http(s) only, timeouts + size caps. (3) **SSTI** in the new
  response templating engine — must be a sandboxed allow-list grammar, never
  Jinja-over-user-text/`eval`.
- **Top risks (high):** cross-tenant **WS/SSE feed** subscription (auth before
  `accept()`), **stored XSS** rendering captured request data (confirmed
  `dashboard.html:213-235` pattern), **DoS** via MITM/chaos/latency/Auto-CRUD/
  unbounded ingest, and **tunnel slug-hijack** without an authenticated bind.
- **No password wall added** (LOCKED honored); CORS stays wide-open on P1 only and
  never credentialed.
- I defined **27 required, testable security ACs** (SEC-AC-1…27) for the PM to fold
  into PRD §4 and **8 contract notes** (auth/status-matrix/SSRF/SSTI/real-time/CORS/
  validation/plane-dispatch) + **7 open security questions** for the architect.

