# Copy & content design: HookBox — Rust/Axum re-platform (SPA + landing)

- **Slug:** `hookbox-rust-replatform`
- **Status:** Draft (copywriter-engineer, DESIGN phase)
- **Author:** Copywriter Engineer (multi-agent pipeline)
- **Owns:** every word the user reads — landing/marketing, in-app microcopy, all state copy.
- **Upstream (read, in this order):** `ux.md` (the 6 surfaces + every interaction state), `design.md` (the visual voice — fresh, instrument-grade, quiet; teal + warm-slate; mono-as-texture), `journey.md` (every flow + error/empty/loading/pending state), `prd.md`, `architecture.md` (the now-frozen §5 — OQ-1/2/3/4 resolved), `FEATURES.md` (canonical terminology).
- **Downstream consumer:** `frontend-engineer` wires each key → string **1:1**. This file is the single source of truth for strings; no copy lives in components.

> **How to read this doc.** §1 is the voice contract. §2 is content design / IA per surface (hierarchy + what to omit). §3 is the landing/marketing copy. §4–§5 are the string tables — a flat **key → string** lookup the FE wires verbatim. Keys are stable and dot-namespaced by surface (`landing.hero.headline`, `feed.empty.title`, `rule.toast.saved`, `cli.secret.warning`). Where a string interpolates a value I use `{token}`, `{n}`, `{seconds}`, `{bytes}` etc. — the FE supplies the value. Terminology is locked to §5 / `FEATURES.md`: **endpoint · token · owner capability (the "secret") · rule · trace · state · Auto-CRUD · proxy / MITM · tunnel · served-by**. Anything I write that implies behavior the frozen contract doesn't guarantee is called out in **§6 Flags for the PM**.

---

## 1. Voice & tone

**Voice in one line:** *a sharp colleague handing you the right tool — precise, unhurried, dev-to-dev; says what happened and what to do, never sells, never apologizes twice.*

HookBox is an HTTP instrument. The copy reads like good tooling output: every word is load-bearing, the nouns are the real nouns (`endpoint`, `rule`, `trace`, `served-by`), and nothing is dressed up. We match design.md's **fresh, instrument-grade, quiet** — copy is part of the texture, not chrome on top of it.

### 1.1 Principles

1. **Precise over friendly.** Name the real thing. "Capability rotated" not "Something went wrong with your login." A developer trusts a tool that talks straight.
2. **Low-jargon, not dumbed-down.** Assume the reader knows HTTP, JSON, status codes, CORS, a proxy. Don't explain `429`. Do explain *HookBox-specific* concepts once (served-by, the resolution order, the capability-as-recovery-key).
3. **Short. Then stop.** Headlines ≤ 6 words. Helper text one line. Errors: what happened + the one next move. No throat-clearing ("Oops!", "Uh-oh", "It looks like").
4. **Active, present, second person.** "Copy the mock URL." "We rotated your secret." Not "The URL can be copied."
5. **No false promises.** Don't imply behavior the contract doesn't ship. The webhook action is "stored, not sent." The template palette inserts tags; it does not validate them. Flag, never fudge (§6).
6. **Mono is the data, sans is the talk.** Anything the user *copies or inspects* (URLs, tokens, headers, curl, tags) is mono and verbatim — copy never paraphrases a value. Sans prose surrounds it.
7. **Quiet by default, loud only for danger.** Success is a one-line confirmation, often just the optimistic UI settling. Destructive and security-relevant moments get full, unambiguous sentences.

### 1.2 Tone shifts by moment

| Moment | Tone | Shape |
|---|---|---|
| **Onboarding / empty** | inviting, instructive, zero pressure | one line of what-this-is + the exact next action (copy this, send that) |
| **Success** | quiet, terse, confirmatory | "Rule saved." "History cleared." Often paired with optimistic UI; never a celebration |
| **Error (user fixable)** | direct, blame-free, actionable | what happened → what to do; the field or the retry is right there |
| **Error (system / transient)** | calm, reassuring, non-alarming | "Reconnecting…" not "CONNECTION LOST"; we're handling it |
| **Destructive confirm** | plain, specific, names the target | "Delete endpoint {token}? This can't be undone." |
| **Security / capability** | serious, exact, no euphemism | "This is your secret. Anyone with it controls this endpoint." |

### 1.3 Do / Don't

- **Empty feed.** ✅ "No requests yet. Send one to your mock URL and it shows up here live." ❌ "Looks like it's quiet in here! 🦗 Get started by making your first request!"
- **401 bounce.** ✅ "Your session ended — your secret was rotated somewhere else. Enter your email to continue." ❌ "Authentication failed. Please log in again."
- **Rate limit.** ✅ "Too many attempts. Try again in {seconds}s." ❌ "Error 429: Rate limit exceeded."
- **Delete confirm.** ✅ "Delete endpoint {token}? Rules, state, and history go with it. This can't be undone." ❌ "Are you sure you want to do this?"
- **Webhook action.** ✅ "Stored, not yet sent. HookBox saves this but doesn't fire it." ❌ "Send a webhook when this rule matches."
- **Template palette.** ✅ "Click to insert. Unknown tags are left as-is — they never error." ❌ "Preview your rendered response." (we don't render-preview — §6)
- **Success toast.** ✅ "Rule saved." ❌ "Success! Your rule has been saved successfully. 🎉"

### 1.4 Terminology lock (use exactly these; never synonyms)

| Use | Never |
|---|---|
| endpoint | mock, project, app |
| token | id, slug (slug only in the CLI flag name `--endpoint`) |
| owner capability / "your secret" | password, key, API key, login |
| rule | mock, handler, route |
| trace | log, event, hit (row may be "request" in feed prose) |
| state | session, variables, store |
| Auto-CRUD | CRUD mode, database (chip label is `crud`) |
| proxy / MITM | passthrough, forward (chip label is `mitm`) |
| tunnel | reverse proxy, ngrok |
| served-by | source, handler, origin |
| mock URL | endpoint URL, base URL |

Capitalization: **Auto-CRUD**, **Auto-CORS**, **HookBox** are proper. `served-by` values are lowercase code (`rule`, `crud`, `mitm`, `tunnel`, `default`, `cors`, `chaos`, `ratelimit`). HTTP verbs and status codes are mono uppercase/digits.

---

## 2. Content design / IA — per surface

For each surface: the hierarchy (headline → subhead → body), and **what to omit** to keep it light.

### 2.1 Landing / email gate
- **Hierarchy:** wordmark → hero headline (display) → one subhead line → email field + button → one helper line ("your email is the recovery key") → 3-up feature strip (Mock · Intercept · Inspect) → quiet `/cli` link → footer.
- **Omit:** pricing, sign-up vs sign-in distinction (anti-enumeration — one door), feature laundry list above the fold, testimonials, "no credit card" badges. The hero does one job: take an email.

### 2.2 Dashboard (split-screen)
- **Sub-header hierarchy:** endpoint name (or token) → mock-URL chip(s) (mono, copy-only) → Auto-CRUD toggle → Rules · + New Rule (primary) · Settings.
- **Feed hierarchy:** endpoint name + ConnectionPill (the live-state truth) → Pause/Resume → the stream. Column order is fixed (method · path · status · served-by · latency · time) — no labels needed; the chips *are* the legend.
- **Inspector hierarchy:** subject strip (method · path · status · served-by · latency) → 5 tabs → per-tab content.
- **Omit:** column headers on the feed (chips self-describe; a streaming log doesn't get a header row), redundant "Request details" titles in the inspector (the subject strip is the title), any explanatory text on a populated screen. Prose appears only in empty/loading/error states.

### 2.3 Rule builder
- **Hierarchy:** dialog title ("New rule" / "Edit rule") → name · priority · enabled row → 5 tabs (Matching · Response · Templating · Actions · Throttling) → footer (Cancel · Save).
- **Per-tab:** each field gets a label + placeholder; helper text only where behavior is non-obvious (path `:param` capture, priority "lower wins", template "unknown tags stay literal", state-write "applied before the body", throttling "overrides the endpoint value").
- **Omit:** descriptions of what a rule is (the user opened the builder; they know), validation rules as prose (show them as inline errors when violated), a render-preview (§6).

### 2.4 Rules manager
- **Hierarchy:** title "Rules" → + New Rule → priority-ordered list (priority badge · name · method+path · enabled · row menu).
- **Body teaches once:** the empty state explains the resolution order; the populated list shows priority so "first match wins" is visible, not stated.
- **Omit:** per-row helper text, repeated explanations of priority on every row.

### 2.5 Endpoint settings
- **Hierarchy:** title "Settings" → grouped cards (Identity · Proxy target · Auto-CRUD · Default mode · Simulated conditions · CORS · Retention & state · Danger zone).
- **Each card:** section title → controls → one helper line per non-obvious control (SSRF→502, wrap order, 0=unlimited, CORS scope).
- **Omit:** restating field names in helper text, success prose (optimistic toggle + quiet toast). Danger zone is the one place that gets full sentences.

### 2.6 Tunnel / CLI
- **Hierarchy:** title → one-line what-it-does → the command (CodeBlock, copy, secret revealed-on-demand) → secret warning → behavior notes (takeover, `504 no_tunnel`).
- **Omit:** install instructions for a CLI that ships in the binary, a tutorial. Set expectations (`504` is not a bug; second bind takes over) so the terminal output isn't surprising.

---

## 3. Landing / marketing copy

First impression — fresh, beautiful, light. The hero earns the email; the strip earns the trust. (Keys are in §4.1 for wiring; the prose below is the editorial source.)

### 3.1 Hero
- **Headline:** **Mock, intercept, and watch HTTP — live.**
- **Subhead:** Self-hosted API mocking and request interception with a real-time inspector. One email in, one endpoint out. No password, no setup.
- **Field placeholder:** `you@company.com`
- **Primary CTA:** Get my endpoint
- **Helper under field:** Your email is the recovery key — no password. Re-enter it any time to pick up where you left off.

### 3.2 Value strip (3-up: Mock · Intercept · Inspect)
- **Mock** — *Endpoints in one step.* Spin up a mock URL instantly. Define rules by method, path, headers, and body — return exactly the response you want.
- **Intercept** — *Sit in the middle.* Point an endpoint at a real upstream and proxy what you don't mock. Add latency, rate limits, and chaos to see how your client copes.
- **Inspect** — *Every request, live.* A streaming feed of every hit with a deep inspector — headers, query, body, the response served, and the full resolution trace.

### 3.3 Feature blurbs (parity capabilities)
Short, mono-flavored, true to the contract. One sentence each.

- **Instant endpoints** — Enter your email; get a working endpoint and a wildcard mock URL on the spot.
- **Wildcard mock surface** — Every endpoint answers at `{token}.{domain}/…` (and `/e/{token}/…` for local dev) — every path is yours to mock.
- **Rule-driven responses** — Match on method, path (`:param` and `/*`), headers, query, and JSON body; first matching rule by priority wins.
- **Sandboxed templating** — Drop `{{now 'iso'}}`, `{{random 'uuid'}}`, `{{request.body.user.id}}`, `{{state.token}}` into a response. A closed scanner — no eval, no engine, unknown tags stay literal.
- **Stateful transactions** — Rules read, require, and write per-endpoint state, so `POST /login` can unlock a later `/dashboard` — multi-step flows without a backend.
- **Auto-CRUD** — Flip one switch and the endpoint becomes a REST store: `POST/GET/PUT/PATCH/DELETE /<collection>`, UUID ids, no rules required.
- **Proxy / MITM** — Set a target URL and unmatched requests forward upstream — with an SSRF guard and your secret stripped before it leaves.
- **Auto-CORS** — Preflight and CORS headers handled on the mock surface automatically. No `OPTIONS` rule to write.
- **Simulated conditions** — Per-endpoint or per-rule latency, rate limits, and a chaos percentage that injects `5xx` or drops the connection.
- **Real-time feed + inspector** — A live WebSocket feed (SSE fallback) and a five-tab inspector for every served request.
- **Retention** — The last 100 traces per endpoint, 24-hour TTL — enough to debug, light enough to stay fast.
- **Tunnel CLI** — Reverse-tunnel public traffic to `localhost` over one authenticated socket. Test webhooks against code running on your machine.

### 3.4 CTAs & footer
- **Primary CTA:** Get my endpoint
- **Secondary CTA:** Already have an endpoint? Tunnel from your machine →
- **Footer:** Self-hosted · Single binary · No tracking — built for developers who'd rather run it themselves.

---

## 4. String tables — landing & marketing (key → string)

> FE wires each key 1:1. `{…}` = interpolation slot. Strings are final copy.

### 4.1 Landing (`landing.*`)
| Key | String |
|---|---|
| `landing.brand.wordmark` | HookBox |
| `landing.brand.markAlt` | HookBox |
| `landing.hero.headline` | Mock, intercept, and watch HTTP — live. |
| `landing.hero.subhead` | Self-hosted API mocking and request interception with a real-time inspector. One email in, one endpoint out. No password, no setup. |
| `landing.email.label` | Email address |
| `landing.email.placeholder` | you@company.com |
| `landing.email.submit` | Get my endpoint |
| `landing.email.submitting` | Setting up… |
| `landing.email.helper` | Your email is the recovery key — no password. Re-enter it any time to pick up where you left off. |
| `landing.strip.mock.title` | Mock |
| `landing.strip.mock.body` | Spin up a mock URL instantly. Define rules by method, path, headers, and body — return exactly the response you want. |
| `landing.strip.intercept.title` | Intercept |
| `landing.strip.intercept.body` | Point an endpoint at a real upstream and proxy what you don't mock. Add latency, rate limits, and chaos to see how your client copes. |
| `landing.strip.inspect.title` | Inspect |
| `landing.strip.inspect.body` | A streaming feed of every hit with a deep inspector — headers, query, body, the response served, and the full resolution trace. |
| `landing.cli.link` | Already have an endpoint? Tunnel from your machine → |
| `landing.footer.tagline` | Self-hosted · Single binary · No tracking — built for developers who'd rather run it themselves. |

### 4.2 Landing states (`landing.state.*`)
| Key | String |
|---|---|
| `landing.error.email.invalid` | Enter a valid email address. |
| `landing.error.rateLimit` | Too many attempts. Try again in {seconds}s. |
| `landing.error.network` | Couldn't reach the server. Check your connection and try again. |
| `landing.error.noEndpoint` | Session created, but no endpoint came back. Try again — if it persists, restart the server. |
| `landing.warn.storage` | This browser won't remember your session (private mode or storage blocked). You can still continue — keep your email to return. |
| `landing.error.generic` | Something didn't go through. Try again. |

> Note (anti-enumeration, AC-1 / AC-D22): there is **no** "welcome back" or "account created" string. The form, button, and success path are identical for a brand-new and an existing email. Do not add a returning-user variant.

### 4.3 Marketing feature blurbs (`landing.feature.*` — optional long-page use)
| Key | String |
|---|---|
| `landing.feature.instant.title` | Instant endpoints |
| `landing.feature.instant.body` | Enter your email; get a working endpoint and a wildcard mock URL on the spot. |
| `landing.feature.wildcard.title` | Wildcard mock surface |
| `landing.feature.wildcard.body` | Every endpoint answers at {token}.{domain}/… (and /e/{token}/… for local dev) — every path is yours to mock. |
| `landing.feature.rules.title` | Rule-driven responses |
| `landing.feature.rules.body` | Match on method, path (:param and /*), headers, query, and JSON body; first matching rule by priority wins. |
| `landing.feature.templating.title` | Sandboxed templating |
| `landing.feature.templating.body` | Drop {{now 'iso'}}, {{random 'uuid'}}, {{request.body.user.id}}, {{state.token}} into a response. A closed scanner — no eval, no engine, unknown tags stay literal. |
| `landing.feature.state.title` | Stateful transactions |
| `landing.feature.state.body` | Rules read, require, and write per-endpoint state, so POST /login can unlock a later /dashboard — multi-step flows without a backend. |
| `landing.feature.crud.title` | Auto-CRUD |
| `landing.feature.crud.body` | Flip one switch and the endpoint becomes a REST store: POST/GET/PUT/PATCH/DELETE /<collection>, UUID ids, no rules required. |
| `landing.feature.proxy.title` | Proxy / MITM |
| `landing.feature.proxy.body` | Set a target URL and unmatched requests forward upstream — with an SSRF guard and your secret stripped before it leaves. |
| `landing.feature.cors.title` | Auto-CORS |
| `landing.feature.cors.body` | Preflight and CORS headers handled on the mock surface automatically. No OPTIONS rule to write. |
| `landing.feature.conditions.title` | Simulated conditions |
| `landing.feature.conditions.body` | Per-endpoint or per-rule latency, rate limits, and a chaos percentage that injects 5xx or drops the connection. |
| `landing.feature.feed.title` | Real-time feed + inspector |
| `landing.feature.feed.body` | A live WebSocket feed (SSE fallback) and a five-tab inspector for every served request. |
| `landing.feature.retention.title` | Retention |
| `landing.feature.retention.body` | The last 100 traces per endpoint, 24-hour TTL — enough to debug, light enough to stay fast. |
| `landing.feature.tunnel.title` | Tunnel CLI |
| `landing.feature.tunnel.body` | Reverse-tunnel public traffic to localhost over one authenticated socket. Test webhooks against code running on your machine. |

---

## 5. String tables — in-app microcopy & state copy (key → string)

### 5.1 App shell & nav (`shell.*`)
| Key | String |
|---|---|
| `shell.skipLink` | Skip to content |
| `shell.nav.endpointSwitcher.label` | Switch endpoint |
| `shell.nav.endpointSwitcher.placeholder` | Select an endpoint |
| `shell.nav.newEndpoint` | New endpoint |
| `shell.nav.newEndpoint.aria` | Create a new endpoint |
| `shell.nav.theme.aria` | Toggle theme |
| `shell.account.label` | Account |
| `shell.account.signedInAs` | Signed in as {email} |
| `shell.account.signOut` | Sign out |
| `shell.account.signOut.confirmTitle` | Sign out? |
| `shell.account.signOut.confirmBody` | This clears your secret from this browser. Re-enter your email to return. |
| `shell.account.signOut.confirm` | Sign out |
| `shell.mobileNav.open.aria` | Open menu |
| `shell.mobileNav.title` | Menu |

### 5.2 Dashboard sub-header (`dash.*`)
| Key | String |
|---|---|
| `dash.mockUrl.label` | Mock URL |
| `dash.mockUrl.copy.aria` | Copy mock URL |
| `dash.pathUrl.label` | Local path |
| `dash.pathUrl.copy.aria` | Copy local path |
| `dash.autoCrud.label` | Auto-CRUD |
| `dash.autoCrud.tooltip` | Serve a REST store for unmatched requests. |
| `dash.action.rules` | Rules |
| `dash.action.newRule` | New rule |
| `dash.action.settings` | Settings |
| `dash.tunnel.active` | Tunnel connected |
| `dash.tunnel.active.tooltip` | A tunnel is forwarding this endpoint to a local machine. |

### 5.3 Feed (`feed.*`)
| Key | String |
|---|---|
| `feed.header.title` | Live feed |
| `feed.pause` | Pause |
| `feed.resume` | Resume |
| `feed.pause.aria` | Pause the live feed |
| `feed.resume.aria` | Resume the live feed |
| `feed.newCount` | {n} new |
| `feed.newCount.aria` | {n} new requests — click to show |
| `feed.count` | Showing {n} of last 100 |
| `feed.loading.aria` | Loading recent requests |
| `feed.empty.title` | No requests yet |
| `feed.empty.body` | Send a request to your mock URL and it shows up here, live. |
| `feed.empty.sampleLabel` | Try it |
| `feed.empty.sampleHint` | Run this in a terminal — or just open the URL in a browser. |
| `feed.row.servedBy.aria` | Served by {servedBy} |
| `feed.row.latency` | {ms}ms |
| `feed.row.select.aria` | Inspect {method} {path}, {status} |

> `feed.empty.sample` is rendered as static mono text (never executed, not an asset): `curl {mock_url}/ping`. The FE substitutes the live `mock_url`.

### 5.4 Connection pill (`feed.conn.*`)
| Key | String |
|---|---|
| `feed.conn.connecting` | Connecting… |
| `feed.conn.live` | Live |
| `feed.conn.reconnecting` | Reconnecting… |
| `feed.conn.reconnecting.n` | Reconnecting… (try {n}) |
| `feed.conn.sse` | Live (SSE) |
| `feed.conn.sse.tooltip` | WebSocket unavailable — streaming over SSE instead. Same events. |
| `feed.conn.offline` | Offline |
| `feed.conn.offline.tooltip` | Lost connection to the feed. Showing the last requests received. |
| `feed.conn.unauthorized` | Not authorized for this feed |
| `feed.conn.unauthorized.tooltip` | Your secret no longer authorizes this feed. It may have been rotated elsewhere. |
| `feed.conn.busy` | Feed busy |
| `feed.conn.busy.tooltip` | Too many connections to this endpoint's feed ({max} max). Close another tab and it'll reconnect. |

### 5.5 Inspector (`insp.*`)
| Key | String |
|---|---|
| `insp.empty.title` | Select a request |
| `insp.empty.body` | Pick a request on the left to inspect it. |
| `insp.loading.aria` | Loading request detail |
| `insp.pending.title` | Detail on its way |
| `insp.pending.body` | This request just landed — its detail is still being written. |
| `insp.pending.retry` | Retry |
| `insp.error.title` | Couldn't load this request |
| `insp.error.body` | Something went wrong fetching the detail. |
| `insp.error.retry` | Retry |
| `insp.unauthorized.title` | Not authorized |
| `insp.unauthorized.body` | Your secret doesn't authorize this request. Re-enter your email to continue. |
| `insp.tab.headers` | Headers |
| `insp.tab.query` | Query |
| `insp.tab.body` | Body |
| `insp.tab.response` | Response Served |
| `insp.tab.trace` | State & Tracing |
| `insp.headers.empty` | No headers. |
| `insp.headers.redacted` | redacted |
| `insp.headers.redacted.tooltip` | Sensitive headers (your secret, Cookie, Authorization) are never stored. |
| `insp.query.empty` | No query parameters. |
| `insp.body.empty` | No request body. |
| `insp.body.binary` | Binary body — not shown. |
| `insp.body.truncated` | Truncated at {bytes} — captured bodies are capped. |
| `insp.body.pretty` | Pretty |
| `insp.body.raw` | Raw |
| `insp.body.expandAll` | Expand all |
| `insp.body.collapseAll` | Collapse all |
| `insp.body.copy` | Copy body |
| `insp.response.servedByLabel` | Served by |
| `insp.response.headers` | Response headers |
| `insp.response.body` | Response body |
| `insp.response.empty` | No response body. |
| `insp.trace.title` | Resolution trace |
| `insp.trace.empty` | No trace steps. |
| `insp.trace.stateTitle` | State at request time |
| `insp.trace.stateEmpty` | No state set on this endpoint. |
| `insp.trace.latency` | Applied latency: {applied}ms of {total}ms total |

### 5.6 Served-by chip labels & tooltips (`servedBy.*`)
> Chip label is the lowercase code; tooltip explains it. AC-56 union.

| Key | Label | Tooltip |
|---|---|---|
| `servedBy.rule.label` / `.tooltip` | rule | Matched a rule you wrote. |
| `servedBy.crud.label` / `.tooltip` | crud | Served by Auto-CRUD. |
| `servedBy.mitm.label` / `.tooltip` | mitm | Proxied to your upstream target. |
| `servedBy.tunnel.label` / `.tooltip` | tunnel | Forwarded down your tunnel to localhost. |
| `servedBy.default.label` / `.tooltip` | default | No rule matched — the endpoint's default response. |
| `servedBy.cors.label` / `.tooltip` | cors | An Auto-CORS preflight response. |
| `servedBy.chaos.label` / `.tooltip` | chaos | A failure injected by chaos. |
| `servedBy.ratelimit.label` / `.tooltip` | ratelimit | Rejected by the rate limit. |

### 5.7 Rule builder (`rule.*`)
| Key | String |
|---|---|
| `rule.new.title` | New rule |
| `rule.edit.title` | Edit rule |
| `rule.field.name.label` | Name |
| `rule.field.name.placeholder` | e.g. Login success |
| `rule.field.name.helper` | Optional. Just for you — shows in the rules list. |
| `rule.field.priority.label` | Priority |
| `rule.field.priority.helper` | Lower wins. Ties break by creation order. |
| `rule.field.enabled.label` | Enabled |
| `rule.tab.matching` | Matching |
| `rule.tab.response` | Response |
| `rule.tab.templating` | Templating |
| `rule.tab.actions` | Actions |
| `rule.tab.throttling` | Throttling |
| `rule.cancel` | Cancel |
| `rule.save` | Save rule |
| `rule.saving` | Saving… |
| **Matching tab** | |
| `rule.match.method.label` | Method |
| `rule.match.method.any` | ANY |
| `rule.match.path.label` | Path |
| `rule.match.path.placeholder` | /users/:id |
| `rule.match.path.helper` | Use :name to capture a segment and /* to match the rest. Exact otherwise. |
| `rule.match.path.captured` | Captures: {names} |
| `rule.match.headers.label` | Required headers |
| `rule.match.headers.helper` | Request must include these. Names are case-insensitive. |
| `rule.match.headers.keyPlaceholder` | Header name |
| `rule.match.headers.valPlaceholder` | Expected value |
| `rule.match.query.label` | Required query params |
| `rule.match.query.keyPlaceholder` | Param |
| `rule.match.query.valPlaceholder` | Expected value |
| `rule.match.body.label` | Body conditions |
| `rule.match.body.helper` | Test the JSON body by path. |
| `rule.match.body.pathPlaceholder` | $.user.role |
| `rule.match.body.op.eq` | equals |
| `rule.match.body.op.neq` | not equals |
| `rule.match.body.op.contains` | contains |
| `rule.match.body.op.exists` | exists |
| `rule.match.body.valPlaceholder` | Value |
| `rule.match.state.label` | State requirements |
| `rule.match.state.helper` | Match only when the endpoint's state holds. |
| `rule.match.state.keyPlaceholder` | State key |
| `rule.match.state.op.eq` | equals |
| `rule.match.state.op.neq` | not equals |
| `rule.match.state.op.exists` | exists |
| `rule.match.state.op.absent` | absent |
| `rule.match.state.valPlaceholder` | Value |
| `rule.match.addRow` | Add |
| `rule.match.removeRow.aria` | Remove row |
| **Response tab** | |
| `rule.resp.status.label` | Status code |
| `rule.resp.status.helper` | 100–599. |
| `rule.resp.contentType.label` | Content-Type |
| `rule.resp.headers.label` | Response headers |
| `rule.resp.body.label` | Response body |
| `rule.resp.body.placeholder` | { "id": "{{random 'uuid'}}" } |
| `rule.resp.body.counter` | {used} / 256 KB |
| `rule.resp.body.counter.over` | {used} / 256 KB — over the cap, this rule won't render |
| **Templating tab** | |
| `rule.tmpl.intro` | Click a tag to insert it at your cursor in the response body. |
| `rule.tmpl.honesty` | Unknown tags are left exactly as written — they never error. |
| `rule.tmpl.group.time` | Time |
| `rule.tmpl.group.random` | Random |
| `rule.tmpl.group.request` | Request |
| `rule.tmpl.group.state` | State |
| `rule.tmpl.tag.now.iso` | {{now 'iso'}} |
| `rule.tmpl.tag.now.unix` | {{now 'unix'}} |
| `rule.tmpl.tag.random.uuid` | {{random 'uuid'}} |
| `rule.tmpl.tag.random.int` | {{random 'int' 1 100}} |
| `rule.tmpl.tag.request.path` | {{request.path.id}} |
| `rule.tmpl.tag.request.query` | {{request.query.q}} |
| `rule.tmpl.tag.request.header` | {{request.header.Authorization}} |
| `rule.tmpl.tag.request.body` | {{request.body.user.id}} |
| `rule.tmpl.tag.state` | {{state.token}} |
| **Actions tab** | |
| `rule.act.stateWrites.label` | State writes |
| `rule.act.stateWrites.helper` | Set state when this rule matches. Applied before the body renders, so {{state.key}} sees the new value. |
| `rule.act.stateWrites.keyPlaceholder` | State key |
| `rule.act.stateWrites.valPlaceholder` | Value (tags allowed) |
| `rule.act.webhook.label` | Webhook |
| `rule.act.webhook.badge` | Stored, not yet sent |
| `rule.act.webhook.helper` | HookBox saves this with the rule but doesn't fire it yet. |
| `rule.act.webhook.urlPlaceholder` | https://example.com/hook |
| `rule.act.webhook.bodyPlaceholder` | Webhook body (templated) |
| **Throttling tab** | |
| `rule.thr.intro` | Overrides the endpoint's values for requests this rule serves. |
| `rule.thr.latency.label` | Latency |
| `rule.thr.latency.unit` | ms |
| `rule.thr.latency.helper` | 0–10000ms. Delays the response. |
| `rule.thr.rateLimit.label` | Rate limit |
| `rule.thr.rateLimit.unit` | req/min |
| `rule.thr.rateLimit.helper` | 0 = unlimited. |
| `rule.thr.chaosMode.label` | Chaos mode |
| `rule.thr.chaosMode.inherit` | Inherit from endpoint |
| `rule.thr.chaosMode.error` | Error (random 5xx) |
| `rule.thr.chaosMode.dropout` | Dropout (drop connection) |
| **Validation & toasts** | |
| `rule.error.path.invalid` | Enter a path starting with /. |
| `rule.error.status.range` | Status must be 100–599. |
| `rule.error.state.key.invalid` | Keys allow letters, numbers, _ and - (max 64). |
| `rule.error.body.tooLarge` | Response body is over the 256 KB cap. |
| `rule.footer.invalid` | {n} fields need attention |
| `rule.footer.ready` | Ready to save |
| `rule.toast.saved` | Rule saved. |
| `rule.toast.updated` | Rule updated. |
| `rule.error.save` | Couldn't save the rule. Check the highlighted fields and try again. |
| `rule.error.gone` | This rule was deleted elsewhere. Closing and refreshing. |

### 5.8 Rules manager (`rules.*`)
| Key | String |
|---|---|
| `rules.title` | Rules |
| `rules.newRule` | New rule |
| `rules.loading.aria` | Loading rules |
| `rules.col.priority` | Priority |
| `rules.col.name` | Rule |
| `rules.col.match` | Match |
| `rules.col.enabled` | On |
| `rules.empty.title` | No rules yet |
| `rules.empty.body` | Without a rule, unmatched requests fall through to Auto-CRUD, then your tunnel, then your proxy target, then the default response. |
| `rules.error.title` | Couldn't load rules |
| `rules.error.retry` | Retry |
| `rules.row.menu.aria` | Rule actions |
| `rules.row.edit` | Edit |
| `rules.row.duplicate` | Duplicate |
| `rules.row.delete` | Delete |
| `rules.row.unnamed` | Untitled rule |
| `rules.delete.title` | Delete this rule? |
| `rules.delete.body` | "{name}" will stop matching requests. This can't be undone. |
| `rules.delete.confirm` | Delete rule |
| `rules.delete.cancel` | Cancel |
| `rules.toast.deleted` | Rule deleted. |
| `rules.toast.duplicated` | Rule duplicated. |
| `rules.toast.enabled` | Rule enabled. |
| `rules.toast.disabled` | Rule disabled. |
| `rules.error.toggle` | Couldn't update the rule. Reverted. |

### 5.9 Endpoint settings (`set.*`)
| Key | String |
|---|---|
| `set.title` | Settings |
| `set.loading.aria` | Loading settings |
| `set.error.title` | Couldn't load settings |
| `set.error.retry` | Retry |
| `set.save` | Save |
| `set.saving` | Saving… |
| `set.toast.saved` | Settings saved. |
| `set.error.save` | Couldn't save. Your changes were reverted. |
| **Identity** | |
| `set.identity.title` | Identity |
| `set.identity.name.label` | Endpoint name |
| `set.identity.name.placeholder` | e.g. Checkout API |
| `set.identity.name.helper` | Optional. Just a label for you. |
| `set.identity.mockUrl.label` | Mock URL |
| `set.identity.pathUrl.label` | Local path |
| `set.identity.token.label` | Token |
| **Proxy target** | |
| `set.proxy.title` | Proxy target (MITM) |
| `set.proxy.url.label` | Target URL |
| `set.proxy.url.placeholder` | https://api.example.com |
| `set.proxy.url.helper` | Unmatched requests forward here. A matching rule always wins. Leave empty to turn off. |
| `set.proxy.url.note` | Targets on private or loopback addresses are blocked and return 502. |
| `set.proxy.url.invalid` | Enter an http or https URL with a host. |
| **Auto-CRUD** | |
| `set.crud.title` | Auto-CRUD |
| `set.crud.toggle.label` | Serve a REST store for unmatched requests |
| `set.crud.peek.label` | Collections |
| `set.crud.peek.placeholder` | Collection name |
| `set.crud.peek.view` | View |
| `set.crud.peek.empty` | This collection is empty. |
| `set.crud.peek.count` | {n} items |
| `set.crud.peek.invalid` | Names allow letters, numbers, _ and - (max 64). |
| `set.crud.peek.clear` | Clear collection |
| **Default mode** | |
| `set.default.title` | Default response |
| `set.default.helper` | What an endpoint returns when nothing else matches. |
| `set.default.mock404.label` | 404 Not Found |
| `set.default.mock404.helper` | Return 404 with a no-match body. |
| `set.default.echo.label` | Echo |
| `set.default.echo.helper` | Reflect the request back as JSON (method, path, query, headers, body). |
| **Simulated conditions** | |
| `set.cond.title` | Simulated conditions |
| `set.cond.helper` | Applied in order: rate limit → chaos → latency. |
| `set.cond.latency.label` | Latency |
| `set.cond.latency.unit` | ms |
| `set.cond.latency.helper` | 0–10000ms added before responding. |
| `set.cond.rateLimit.label` | Rate limit |
| `set.cond.rateLimit.unit` | req/min |
| `set.cond.rateLimit.helper` | 0 = unlimited. Over the limit returns 429. Counts proxy and Auto-CRUD too. |
| `set.cond.chaos.label` | Chaos |
| `set.cond.chaos.unit` | % |
| `set.cond.chaos.helper` | Chance a request fails on purpose. 0 never, 100 always. |
| `set.cond.chaosMode.label` | Chaos mode |
| `set.cond.chaosMode.error` | Error (random 5xx) |
| `set.cond.chaosMode.dropout` | Dropout (drop the connection) |
| **CORS** | |
| `set.cors.title` | Auto-CORS |
| `set.cors.toggle.label` | Handle CORS automatically |
| `set.cors.helper` | Answers OPTIONS preflight and reflects the Origin — on the mock surface only. |
| **Retention & state** | |
| `set.retention.title` | Retention & state |
| `set.retention.note` | The last 100 requests per endpoint are kept for 24 hours. |
| `set.retention.clearHistory` | Clear request history |
| `set.retention.clearState` | Clear state |
| `set.retention.stateNote` | State is per-endpoint and shared across all callers. |
| **Danger zone** | |
| `set.danger.title` | Danger zone |
| `set.danger.delete.label` | Delete endpoint |
| `set.danger.delete.helper` | Permanently delete this endpoint, its rules, state, and history. |
| **Confirms** | |
| `set.confirm.clearHistory.title` | Clear request history? |
| `set.confirm.clearHistory.body` | All {n} traces for this endpoint will be removed. The live feed starts fresh. |
| `set.confirm.clearHistory.confirm` | Clear history |
| `set.confirm.clearState.title` | Clear endpoint state? |
| `set.confirm.clearState.body` | Every state key on this endpoint will be removed. Rules that require state stop matching until it's set again. |
| `set.confirm.clearState.confirm` | Clear state |
| `set.confirm.clearCollection.title` | Clear collection "{name}"? |
| `set.confirm.clearCollection.body` | Every item in this Auto-CRUD collection will be removed. This can't be undone. |
| `set.confirm.clearCollection.confirm` | Clear collection |
| `set.confirm.delete.title` | Delete endpoint {token}? |
| `set.confirm.delete.body` | This deletes the endpoint and everything in it — rules, state, history, and collections. The mock URL stops working. This can't be undone. |
| `set.confirm.delete.prompt` | Type the token to confirm: |
| `set.confirm.delete.placeholder` | {token} |
| `set.confirm.delete.confirm` | Delete endpoint |
| `set.confirm.cancel` | Cancel |
| **Setting toasts** | |
| `set.toast.historyCleared` | Request history cleared. |
| `set.toast.stateCleared` | State cleared. |
| `set.toast.collectionCleared` | Collection cleared. |
| `set.toast.deleted` | Endpoint deleted. |
| `set.error.delete` | Couldn't delete the endpoint. Try again. |

### 5.10 Dashboard shell states (`dash.state.*`)
| Key | String |
|---|---|
| `dash.state.loading.aria` | Loading endpoint |
| `dash.state.notFound.title` | Endpoint not found |
| `dash.state.notFound.body` | This endpoint doesn't exist — it may never have, or it was deleted long ago. |
| `dash.state.gone.title` | Endpoint deleted |
| `dash.state.gone.body` | This endpoint was deleted. Its mock URL now returns 410. |
| `dash.state.backToStart` | Back to start |
| `dash.state.offline.title` | You're offline |
| `dash.state.offline.body` | Lost the connection. Showing the last data received — we'll catch up when you're back. |

> AC-57 / OQ-1: `dash.state.gone.*` (410) and `dash.state.notFound.*` (404) are distinct — the architect kept the tombstone, so the UI uses the matching message per the API status.

### 5.11 Tunnel / CLI page (`cli.*`)
| Key | String |
|---|---|
| `cli.title` | Tunnel from your machine |
| `cli.intro` | The tunnel forwards public traffic for an endpoint to a server on your localhost — so you can test webhooks and integrations against code you're running right now. Tunneled requests show up in the feed labeled tunnel. |
| `cli.command.label` | Run this |
| `cli.command.template` | tunnel --port {port} --endpoint {token} --secret {secret} |
| `cli.command.portDefault` | 3000 |
| `cli.command.copy.aria` | Copy tunnel command |
| `cli.secret.reveal` | Reveal secret |
| `cli.secret.hide` | Hide secret |
| `cli.secret.warning` | This is your owner capability — your secret. Anyone who has it controls this endpoint. Don't paste it into shared terminals, screenshots, or issues. |
| `cli.behavior.title` | What to expect |
| `cli.behavior.order` | In the resolution order, the tunnel is tried after Auto-CRUD and before your proxy target. |
| `cli.behavior.noTunnel` | When nothing is connected, tunneled paths return 504 no_tunnel — that's expected, not a bug. |
| `cli.behavior.takeover` | Only one tunnel binds at a time. Connect a second and it takes over; the first is dropped with "rebound elsewhere." |
| `cli.behavior.reconnect` | If the connection drops, the CLI reconnects with backoff. Bad credentials stop it — it won't retry into a wall. |
| `cli.toast.copied` | Copied. |

### 5.12 CLI stdout contract (terminal — `cli.tty.*`)
> Not SPA strings, but the operator-facing words the `tunnel` binary prints (journey gap #6). Listed so the CLI and the docs agree.

| Key | String |
|---|---|
| `cli.tty.connecting` | Connecting to {host} for endpoint {token}… |
| `cli.tty.bound` | Tunnel up. Forwarding {token} → http://localhost:{port} |
| `cli.tty.request` | {method} {path} → localhost:{port} ({status}) |
| `cli.tty.unauthorized` | Authentication failed — your secret was rejected. Re-check --secret (it rotates each time you sign in). Stopping. |
| `cli.tty.rebound` | Disconnected — another tunnel took over this endpoint. Stopping. |
| `cli.tty.disconnected` | Connection lost. Reconnecting in {seconds}s… |
| `cli.tty.reconnected` | Reconnected. |
| `cli.tty.shutdown` | Tunnel closed. |

### 5.13 Global toasts & shared (`common.*`)
| Key | String |
|---|---|
| `common.copy` | Copy |
| `common.copied` | Copied |
| `common.copy.announce` | Copied to clipboard |
| `common.retry` | Retry |
| `common.cancel` | Cancel |
| `common.save` | Save |
| `common.close` | Close |
| `common.close.aria` | Close |
| `common.dismiss` | Dismiss |
| `common.error.401` | Your session ended — your secret was rotated somewhere else. Enter your email to continue. |
| `common.error.network` | Network error. Check your connection and try again. |
| `common.error.generic` | Something went wrong. Try again. |
| `common.notFound.title` | Page not found |
| `common.notFound.body` | That page doesn't exist. |
| `common.notFound.home` | Back to start |

### 5.14 Mock-surface error bodies (P1 — `mock.*`)
> These are the JSON `detail` strings the *mock plane* returns to API callers (not dashboard UI). Kept here so the words callers see in their own tooling are consistent with the dashboard's vocabulary. Bodies are `{error, detail}` per the contract; only `detail` is editorial.

| Key | error code | detail string |
|---|---|---|
| `mock.unknownEndpoint` | unknown_endpoint | No endpoint for this token. |
| `mock.endpointGone` | endpoint_gone | This endpoint was deleted. |
| `mock.noMatch` | no_match | No rule matched this request. |
| `mock.rateLimited` | rate_limited | Rate limit exceeded. Retry after {seconds}s. |
| `mock.chaos` | chaos | Chaos injected this failure. |
| `mock.bodyTooLarge` | body_too_large | Request body exceeds the 1 MB ingest limit. |
| `mock.upstreamUnreachable` | upstream_unreachable | Couldn't reach the proxy target. |
| `mock.upstreamBlocked` | upstream_blocked | The proxy target resolves to a blocked address. |
| `mock.upstreamTimeout` | upstream_timeout | The proxy target timed out. |
| `mock.noTunnel` | no_tunnel | No tunnel is connected for this endpoint. |
| `mock.crudBadBody` | bad_request | Write body must be a JSON object. |
| `mock.crudCapItems` | bad_request | Collection is full (1000 items max). |
| `mock.crudCapBytes` | bad_request | Item exceeds the 64 KB size cap. |

---

## 6. Flags for the PM (copy that touches unguaranteed/edge behavior)

1. **Template render-preview — intentionally absent.** ux.md §2.3 gap #4 flagged that a live preview would need the *same* closed scanner or it lies about behavior. I wrote `rule.tmpl.honesty` ("Unknown tags are left exactly as written — they never error") and **no preview copy**. If the PM/architect later ships a preview, it must run the real scanner; the copy must not promise rendering we don't do.
2. **`webhook_action` — "Stored, not yet sent."** Per PRD non-goal / OQ-9 and ux.md gap #3, the control is visible but inert. Copy (`rule.act.webhook.badge` / `.helper`) says so plainly rather than hiding it. Confirm this disclosure wording is acceptable and that shipping a visible-but-inert control is the intended UX (vs. omitting it).
3. **`410 endpoint_gone` vs `404` — now writable (OQ-1 resolved).** The architect kept the tombstone (`gone_at`, 7-day window), so I wrote **distinct** copy: `dash.state.gone.*` (410, "deleted") vs `dash.state.notFound.*` (404, "doesn't exist"). After the 7-day `GONE_TTL_HOURS` window a deleted token degrades to 404 — a user who returns much later sees "doesn't exist," not "deleted." Flagging in case the PM wants a softer long-tail message; I judged the extra state not worth the weight.
4. **`chaos_mode` — now first-class (OQ-2 resolved).** The architect promoted `chaos_mode("error"|"dropout")` to the schema (endpoint + per-rule), so I wrote real controls and labels (`set.cond.chaosMode.*`, `rule.thr.chaosMode.*` incl. an "Inherit from endpoint" option for the nullable per-rule override). No longer a gap; confirm the "dropout = drop the connection" phrasing reads right to users.
5. **`echo` default mode feed representation (journey gap #10).** I gave `echo` a clear settings description. Its feed/inspector representation just uses the normal `default` served-by chip + the 200 it returns — no special copy. Confirm no distinct "echoed" label is wanted.
6. **Owner-secret on the CLI page (ux.md gap #6).** I wrote a firm secret warning (`cli.secret.warning`) and a reveal-on-demand label pair. The command pre-fills the live secret on reveal/copy. Confirm plaintext reveal (vs. copy-only, never shown) is acceptable.
7. **Connection-cap message (journey gap #4).** I added a distinct `feed.conn.busy` state ("Feed busy — too many connections") with a `{max}` slot, separate from "Reconnecting," so a user on their 51st tab gets the right message. Needs the FE to map WS `1013` / SSE `503` to this state, not to reconnect.
8. **`feed.count` literal "100".** The trace cap (`TRACE_CAP=100`) is config, but I hardcoded "last 100" in user-facing copy (`feed.count`, `set.retention.*`, `rules.empty`/`mock.crudCap*` numbers) to keep it concrete and light. If any cap becomes operator-tunable per deploy and could differ from 100/1000/64 KB/256 KB/1 MB, these strings would drift — recommend keeping the defaults fixed in copy and treating a changed cap as a copy edit, not interpolation.
9. **Sign-out confirm.** I added a confirm dialog for sign-out (`shell.account.signOut.*`) because clearing the secret from the browser is effectively losing local access until re-auth. Confirm a confirm step is wanted (vs. one-click sign-out).
10. **Typed-token delete confirm.** `set.confirm.delete.*` requires typing the token (ux.md §2.5 "typed/explicit confirm"). Confirm the friction is intended for endpoint deletion.

---

## 7. Coverage map (surfaces → key namespaces)

| Surface (ux.md) | Namespaces |
|---|---|
| Landing / email gate (§2.1) | `landing.*` |
| App shell / nav (§1.2) | `shell.*`, `common.*` |
| Dashboard split-screen (§2.2) | `dash.*`, `feed.*`, `feed.conn.*`, `insp.*`, `servedBy.*` |
| Rule builder (§2.3) | `rule.*` |
| Rules manager (§2.4) | `rules.*` |
| Endpoint settings (§2.5) | `set.*` |
| Tunnel / CLI (§2.6) | `cli.*` |
| Mock-surface error bodies (§5.5 P1) | `mock.*` |

Every state in journey.md "Required states (per screen)" has a string: landing (idle/submitting/422/429/network/no-endpoint/storage-warn/auto-resume — note auto-resume shows nothing, it redirects); dashboard (loading/404/410/offline/loaded); feed (loading/empty/streaming/paused + every ConnectionPill state incl. busy/unauthorized); inspector (empty/loading/pending/unauthorized/error + per-tab empties); rules (loading/empty/error/list/delete-confirm/toggle); rule builder (per-field validation/footer count/saving/server-error); settings (loading/error/save-error/saving/field error/danger confirms); tunnel (page copy + CLI stdout contract).
