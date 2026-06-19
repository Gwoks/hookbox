# UI/UX: HookBox — Beeceptor-class API Mocking & Interception Platform (slug: beeceptor-rewrite)

> Scope: the UI/interaction design for the ground-up rewrite, **within the LOCKED stack** —
> server-rendered **Jinja2 + HTMX + Alpine.js + Tailwind**, **no React/JSX/Vite/Node build**
> (`_decisions.md` §1, §1a). This doc covers `prompt.txt` §2: the email entry screen, the
> real-time split-screen dashboard (LEFT live feed / RIGHT deep inspector), and the multi-tab
> Create-Rule modal, plus the surfaces needed to drive every §1 feature (Auto-CRUD, MITM,
> CORS, templating, latency/rate/chaos) from the UI.
>
> **Authority boundaries.** Visual specifics — exact hex palette per method badge, motion/easing
> curves, density tokens, icon set — are the **design-agent's** to finalize (`design.md`,
> feeds PRD OQ-15). This doc fixes **structure, behaviour, states, accessibility, copy, and the
> exact HTMX/Alpine wiring** that replaces React. Every concrete API field, the WS payload shape,
> the `mock_rules` schema, resolution precedence, and identifying headers are the **architect's**
> (`architecture.md`, PRD §5 / OQ-2,8,9,10,11,12). Where this doc names a field/route/event, it is
> **provisional**, marked `[ARCH-GAP]`, and the FE binds only to the final frozen §5. I do **not**
> re-open any LOCKED decision; I align to them (notably: no password wall, no React, email-keyed
> localStorage session, the `request-stream.js` substitute for `useRequestStream`).

---

## 0. Design baseline — what already exists (reuse, do not reinvent)

Read of the seven current templates establishes a small, coherent **GitHub-dark** design system.
The rewrite **keeps the visual language** and rebuilds the *interaction layer* on HTMX + Alpine.

**Tokens & component classes already defined in `base.html` `<style>` (REUSE verbatim, do not redefine):**

| Token / class | Value / role | Where defined |
| --- | --- | --- |
| Page bg / text | `#0d1117` / `#e6edf3` | `base.html:9` |
| `.card` | `#161b22` bg, `1px #30363d` border, `8px` radius | `base.html:10` |
| `.btn-primary` | `#238636` green, hover `#2ea043` | `base.html:11` |
| `.btn-secondary` | `#21262d` bg, `#30363d` border | `base.html:13` |
| `.btn-danger` | `#da3633`, hover `#f85149` | `base.html:15` |
| `.badge` / `.badge-method` | pill, `12px` bold; method badge currently single blue `#388bfd` | `base.html:17-18` |
| `pre` / `code` | `#0d1117` bg, Monaco/Menlo mono | `base.html:19-20` |
| `.copy-btn` | hover `#58a6ff` | `base.html:21` |
| `.toast` | bottom-right, green, fade, `z-1000` | `base.html:23-24`, `showToast()` `base.html:56` |
| Accent / link blue | `#58a6ff`; muted text `#8b949e`; success `#3fb950` | used throughout |
| Input pattern | `bg-[#0d1117] border border-[#30363d] rounded px-3 py-2 focus:border-[#58a6ff] focus:outline-none` | `login.html:22`, `mock.html` |
| Toggle pattern | Tailwind peer switch (`sr-only peer` + `peer-checked:bg-[#238636]`) | `mock.html:108-111` |
| Inline alert | `bg-red-900/50 border border-red-500 text-red-200` (and green/blue variants) | `login.html:15-16`, `backup.html:148-154` |
| WS status colors | `text-green-500` / `text-yellow-500` / `text-red-500` | `dashboard.html:97-101` |
| Breadcrumb | `text-sm text-[#8b949e]` with `/` separators, hover `#58a6ff` | `dashboard.html:7`, `mock.html:7` |

**Patterns to PRESERVE:** `{% extends "base.html" %}` + `{% block content %}`; localStorage session
key **`hookbox_user`** (`base.html:43`); the `showToast()` / `copyToClipboard()` helpers; the
disabled-button-during-submit convention (`login.html:45`); `Escape` closes modal (`dashboard.html:258`);
WS protocol-pick (`wss:`/`ws:` from `window.location.protocol`, `dashboard.html:44`).

**Deliberate breaks from the old UI (justified):**
1. **Stop building the whole page with `innerHTML` string templates in `<script>`** (today `dashboard.html`,
   `index.html`, `mock.html` render their entire body via JS template literals). That approach is the
   project's *de-facto* "client framework" and is exactly what HTMX+Alpine replace. New templates render
   **real server-side Jinja** for first paint; Alpine owns reactive local state; HTMX owns server round-trips.
   This is more consistent with the **locked** stack than the status-quo JS-string rendering.
2. **Add Alpine.js + HTMX `<script>`s and the `static/` mount to `base.html`** (`static/` does not exist
   yet; PRD §7). Tailwind stays via the CDN already in `base.html:7` unless the architect swaps to a CLI build
   (PRD §5; either way this doc's classes are unaffected).
3. **Remove the password/login mental model.** `login.html` + `register.html` collapse into one email
   entry on `/` (LOCKED §5). The old nav "Backup" link (`base.html:35`) is removed (backup is OQ-7).

---

## 1. Screens & components affected — which templates, reuse vs new (cite files)

### Global shell — `base.html` **[REWRITE]**
- **Keep** all `<style>` tokens above. **Add** to `<head>`: Alpine.js (`defer`) + HTMX CDN scripts, plus
  `<script src="/static/js/request-stream.js" defer>` and the Alpine store registrations
  (`/static/js/stores.js` `[new]`). `static/` mount is new (PRD §7).
- **Rewrite the nav** (`base.html:29-50`): brand `HookBox` (drop the emoji per design-agent's call), and
  the localStorage-driven right side becomes an **Alpine component** reading `hookbox_user` instead of the
  two duplicated raw-`innerHTML` scripts (`base.html:41-50` and `:66-84` are redundant today — collapse to one).
  Remove the `/backup` link. Logout clears `hookbox_user` and returns to `/`.
- **Keep** `#toast` + `showToast()` (`base.html:54-61`) — reused for "Copied!", "Rule saved", "Auto-CRUD on".
- Provide a **`{% block fullbleed %}`** escape hatch: the dashboard split-screen needs the full viewport
  width/height, not the centered `max-w-6xl … py-8 <main>` (`base.html:51`). `base.html` renders either the
  centered `<main>` (entry, settings) **or** a full-bleed `<main>` when the page sets `fullbleed`.

### Entry / landing — `index.html` becomes the email screen **[REWRITE]**, `login.html` + `register.html` **[REMOVE]**
- Replaces the current "list of endpoints" `index.html` AND the `login`/`register` pair. Reuses the **centered
  card** layout straight from `login.html:6-31` (`.card p-8`, centered header, the email input pattern, the
  inline error/success alert divs). See §4 copy and §3 states.
- After session resolves, route to `/d/<token>` (replaces today's redirect to `/`).

### Dashboard split-screen — `dashboard.html` **[REWRITE]** (the centerpiece, `prompt §2.2`)
- Full rewrite. Replaces the single-column table + the request-detail modal (`dashboard.html:15-23, 204-240`).
  New structure: a two-column split (LEFT feed / RIGHT inspector) — see §2.
- **New HTMX partial templates** under `templates/partials/` `[new]`:
  - `partials/feed_row.html` — one live-feed row (server-rendered; also the unit `request-stream.js` clones).
  - `partials/inspector.html` — the right-pane tabbed inspector for a selected request (HTMX `GET`-loaded).
  - `partials/inspector_body_tree.html` — recursive JSON/XML tree node partial (collapsible).
  - `partials/rule_row.html` — a row in the rules list.
  - `partials/rule_modal.html` — the multi-tab Create/Edit-Rule modal body (HTMX-loaded into the modal shell).
  - `partials/endpoint_settings.html` — the endpoint config panel (MITM target, Auto-CRUD, CORS, latency/rate/chaos).

### Endpoint settings + Rule builder — supersedes `mock.html` **[REWRITE → folded into the dashboard]**
- The old per-method `mock.html` config screen is replaced. Rule authoring moves into the **Create-Rule modal**
  (§2.3); endpoint-level config (MITM/Auto-CRUD/CORS/throttle) moves into a **Settings panel** reachable from the
  dashboard header. We **reuse** `mock.html`'s proven field markup: the toggle switch (`mock.html:108-111`), the
  status-code/content-type/delay inputs, the headers/body `<textarea>`s, and the "Test Your Mock" sender
  (`mock.html:170-179`) — re-homed into modal tabs and the settings panel.

### Removed — `backup.html` **[REMOVE]**, `login.html`, `register.html` **[REMOVE]**
- Per PRD §2 non-goals / §6. If OQ-7 keeps export/restore, it returns as a small block in the Settings panel,
  not a separate nav page.

---

## 2. Layout & placement — where it lives in `base.html`'s structure

### 2.0 How the three planes map to UI (LOCKED §2)
Only **Plane 3** (UI) is rendered here. The dashboard never *serves* mock traffic; it *observes* it. The mock
URL(s) the user points clients at — `https://<token>.<MOCK_DOMAIN>/…` and the path fallback `/e/<token>/…`
(PRD §5.1) — are shown as **copyable, non-clickable** `code` chips (§4), so nobody mistakes the dashboard origin
for the mock origin.

### 2.1 Entry screen `/` (centered `<main>`, NOT full-bleed)
```
nav (base.html)
└─ <main max-w-6xl>            ← default centered main
   └─ .card p-8 (max-w-md mx-auto)   ← reuse login.html:6
      ├─ header: title + one-line subtitle
      ├─ #alert (hidden; error/success)   ← reuse login.html:15-16
      └─ <form> email input + "Get my mock endpoint" btn-primary w-full
```

### 2.2 Dashboard `/d/<token>` (FULL-BLEED `<main>`, `prompt §2.2`)
A fixed-height app shell: a thin **endpoint bar** on top, then a **horizontal split** filling the rest. On
desktop the split is side-by-side; it collapses to stacked/drawer on narrow screens (§3.7).

```
nav (base.html, slim)
└─ <main fullbleed h-[calc(100vh-navh)] flex flex-col>
   ├─ ENDPOINT BAR  (h-12, .card-flush, flex items-center justify-between px-4)
   │   ├─ left:  breadcrumb  Endpoints / <token>      ← reuse dashboard.html:7 pattern
   │   │         + WS health pill (pulsing dot)       ← §2.4, replaces dashboard.html:97
   │   ├─ mid:   copyable mock-URL chip(s) (subdomain + path-fallback)  ← reuse code+copy, index.html:93
   │   └─ right: [Settings] [+ New Rule] btn-primary  ·  Auto-CRUD toggle (mock.html:108 switch)
   │
   └─ SPLIT  (flex-1 flex min-h-0)
      ├─ LEFT  "Live Feed"     (w-[40%] min-w-[360px] border-r #30363d, flex flex-col)
      │   ├─ feed toolbar (h-10): method filter chips · search · ▮▮/► pause · 🗑 clear
      │   ├─ #feed (flex-1 overflow-y-auto)   ← the ultra-fast scroll list; rows = partials/feed_row.html
      │   └─ feed footer (h-8): "N shown · cap 100" + reconnect notice slot
      └─ RIGHT "Inspector"     (flex-1 min-w-0, flex flex-col)
          ├─ inspector header: METHOD badge · full path · status · latency · served-by tag
          ├─ tab strip (role=tablist): Headers · Query · Body · Response · State & Tracing
          └─ #inspector-panel (flex-1 overflow-auto)   ← partials/inspector.html
```
- **Endpoint bar height ~48px, feed toolbar ~40px** — deliberately dense (developer tool), consistent with the
  compact `text-sm`/`text-xs` scale the current templates already use.
- **Create-Rule modal & Settings panel** are overlays on top of this shell (§2.3, §2.5), reusing the dark
  backdrop + centered panel pattern from `dashboard.html:15-23` (`fixed inset-0 bg-black/50 … z-50`).

### 2.3 Create-Rule modal (overlay, `prompt §2.3`)
Reuse the existing modal shell geometry (`dashboard.html:16` — `max-w-4xl max-h-[80vh] overflow-hidden`,
header with title + `×`). Inside, a **left vertical tab rail + right tab body** (vertical tabs read better for a
5-tab config form than a top strip):
```
.modal (fixed inset-0 bg-black/50 flex z-50)         ← reuse dashboard.html:15
└─ .card w-full max-w-4xl max-h-[85vh] flex flex-col
   ├─ header: "Create rule" / "Edit rule" + × (Esc closes)
   ├─ body flex flex-1 min-h-0
   │   ├─ tab rail (w-44 border-r, role=tablist, vertical):
   │   │     1 Matching   2 Response   3 Templating   4 Actions   5 Throttling
   │   │     (each shows a ● dot if it has unsaved/!default values)
   │   └─ tab panel (flex-1 overflow-auto p-5)   ← one Alpine x-show per tab
   └─ footer (border-t, justify-between):
        left: validation summary ("2 fields need attention")
        right: [Cancel] btn-secondary   [Save rule] btn-primary
```
**Tab contents** (fields are provisional → `[ARCH-GAP]` `mock_rules` schema, OQ-9):
- **1 Matching** — Method (multiselect chips incl. ANY), Path pattern + match-type (exact / prefix / regex),
  optional **header conditions** (key/op/value rows, add/remove), **query conditions** (rows), optional **body
  match** (JSONPath + op + value rows), and **State requirement(s)** (`state.<k>` op value — drives AC-9). A
  live "this rule matches: `GET /books/:id` when `authenticated == true`" summary line.
- **2 Response** — Status code (reuse `mock.html:116`), Content-Type (reuse the select `mock.html:121-126`),
  custom response headers (JSON `<textarea>`, reuse `mock.html:135-137`), response body (mono `<textarea>`,
  reuse `mock.html:140-142`). A "Format JSON" helper button (Alpine, pretty-prints the textarea).
- **3 Templating** — same response-body editor with a **tag palette** (clickable chips insert at cursor):
  `{{now 'iso'}}`, `{{random 'uuid'}}`, `{{request.query.<k>}}`, `{{request.path.<k>}}`,
  `{{request.body.<jsonpath>}}`, `{{state.<k>}}` (AC-20–23). A **"Preview render"** button posts the body to a
  dry-run endpoint and shows the rendered result `[ARCH-GAP: preview endpoint; OQ-8 grammar]`. Inline note that
  unknown tags fail safe (AC-23).
- **4 Actions (State & Webhook)** — **State writes**: rows of `set state.<k> = <value/template>` (drives AC-8,
  e.g. login → `authenticated=true`) and an optional "clear state" action. **Webhook action** is fenced behind
  a clearly-labeled toggle and marked **provisional** in the UI until the architect/PM rule on scope
  (PRD AC-33 / OQ-9): URL, method, body template fields, shown disabled with a "Coming per spec — confirm scope"
  helper if deferred. The UI structure exists either way so no rework if it lands.
- **5 Throttling** — per-rule **Latency** slider 0–10000ms with a numeric box (AC-24), **Rate limit** req/min
  numeric (AC-25), **Chaos %** dial 0–100 + a small note "% of requests return a random 502/503/504"
  (AC-26). Reuse the delay input idea from `mock.html:131`. (Endpoint-level equivalents live in Settings, §2.5;
  rule-level overrides endpoint-level — surface that precedence inline `[ARCH-GAP: per-rule vs per-endpoint, OQ]`).

### 2.4 WS health pill (in the endpoint bar, `prompt §2.2`)
Replaces the bare text in `dashboard.html:97-101`. A pill: a **dot + label**, bound to the Alpine
`stream` store state:
- `connected` → green dot `#3fb950` with a **pulse** (CSS `@keyframes` ping/breathe; design-agent owns timing),
  label "Live".
- `connecting`/`reconnecting` → amber `#d29922` dot, label "Reconnecting…" (+ attempt count after N tries).
- `disconnected`/`closed` → red `#f85149` dot (no pulse), label "Offline".
Colors reuse the existing green/yellow/red intent. `aria-live="polite"` so state changes are announced (§5).

### 2.5 Settings panel (overlay or right-drawer)
Endpoint-level config, opened from `[Settings]`. Same `.card` overlay pattern. Sections:
- **Mock URLs** (copyable chips, both surfaces).
- **Auto-CRUD** toggle + helper "Turns this endpoint into a REST DB backend (POST/GET/PUT/DELETE /collection)"
  (AC-11). When ON, a small read-only hint lists detected collections `[ARCH-GAP]`.
- **Proxy / MITM**: "Target Real API URL" input + helper "Unmatched requests forward here and are captured"
  (AC-14). Shows last-forward status if available.
- **Auto-CORS**: read-only note "Wide-open CORS + OPTIONS preflight handled automatically" (AC-18/19) — it is
  always-on per spec; no toggle unless the architect adds one.
- **Network conditions (endpoint default)**: latency / rate / chaos controls (mirror §2.3 tab 5).
- **Danger zone**: Clear state (AC-10), Clear traces, Delete endpoint (reuse `btn-danger` + confirm,
  `dashboard.html:242-251`).

---

## 3. Interaction & states

### 3.1 Reactivity model — how HTMX/Alpine deliver what React would have
Three responsibilities, three tools (no React, LOCKED §1a):

| Concern | Owner | Mechanism |
| --- | --- | --- |
| **Live request stream** (push, the `useRequestStream` job) | `static/js/request-stream.js` + Alpine `$store.stream` | WebSocket; on message → push into `$store.feed.rows` (capped). Backoff/dedupe in JS (§3.3). |
| **Local UI state** (selected row, active tabs, modal open, filters, optimistic toggles) | **Alpine** (`x-data`/stores) | Pure client reactivity; no server round-trip for tab switches, collapse/expand, filtering. |
| **Server round-trips** (load inspector detail, submit rule, save settings, list rules) | **HTMX** | `hx-get`/`hx-post`/`hx-patch`/`hx-delete` returning Jinja partials swapped into targets. |

This split is the explicit React→HTMX/Alpine substitution the lock requires: **Alpine = component state/render**
(React's `useState`/render), **HTMX = data fetching/mutation + server-rendered partials** (React's data layer),
**`request-stream.js` = the streaming hook** (`useRequestStream`). The architect confirms WS-vs-SSE and payloads
(OQ-11); the FE binds to the frozen shape only.

### 3.2 Live feed — selecting & rendering rows (`prompt §2.2 left`)
- **First paint:** server renders the most recent traces (up to the 100 cap) as `feed_row.html` partials so the
  feed is populated **before** JS runs (progressive enhancement; the old all-JS `dashboard.html` showed nothing
  until fetch resolved — we fix that).
- **New row arrives (WS):** `request-stream.js` prepends a `feed_row` clone at the top with a brief **highlight
  flash** (background fade; design-agent owns the easing) so the eye catches it; older rows scroll down. Feed is
  a plain `overflow-y-auto` column; **newest on top** (matches `requests.unshift` in `dashboard.html:63`).
- **Click a row:** Alpine sets `selectedId`; the row gets a selected style (`bg-[#21262d]`, left accent border in
  the method color); HTMX fires `hx-get="/api/requests/{id}"` `[ARCH-GAP path/shape]` → swaps the right
  `#inspector-panel` with `inspector.html`. Keyboard: `↑/↓` move selection, `Enter`/click loads, focus ring visible.
- **Auto-follow vs pause:** a **▮▮ Pause** toggle (Alpine) stops auto-prepend/auto-scroll so a developer can read
  a row while traffic keeps streaming server-side; a "**N new**" pill appears and, when clicked (or unpause),
  flushes buffered rows. Prevents the "list jumps while I'm reading" problem inherent to live feeds.
- **Filter/search:** method chips + a path search box filter the *rendered* rows client-side (Alpine), no refetch.

### 3.3 Live stream lifecycle — `request-stream.js` (the `useRequestStream` substitute, `prompt §5.2`, AC-30)
Vanilla module + Alpine `$store.stream`, same responsibilities the spec assigns the React hook:
- **Open** the pipe for the current `<token>` (WS path `[ARCH-GAP /ws/<token>, OQ-11]`); pick `wss:`/`ws:` from
  `location.protocol` (reuse `dashboard.html:44`).
- **Exponential backoff reconnect:** on close/error, retry with backoff (e.g. 0.5s → 1 → 2 → 4 → … capped ~30s)
  + jitter; surface attempt state to the WS pill (§2.4). Reset backoff on a clean open. (Old code's flat 3s
  `setTimeout(connectWebSocket, 3000)` at `dashboard.html:88` is replaced.)
- **Dedupe:** keep a small `Set` of seen `request_id`s; ignore replays after reconnect (AC-30).
- **Non-blocking / DOM-safe:** **cap** the feed at the retention max (100) — drop/trim oldest beyond cap so the
  DOM never grows unbounded (AC-30, ties to retention §J). Coalesce bursts (rAF/microtask batch) so a flood
  doesn't thrash layout. Never block the main thread.
- **Lifecycle:** close the socket on page unload (reuse `beforeunload` cleanup, `dashboard.html:261-263`); pause
  reconnection while `document.hidden`, resume on focus (battery/CPU friendliness).
- **Channel scoping (AC-32):** the store is bound to exactly one token; a dashboard for `tokenA` shows only
  `tokenA` rows (BE enforces; FE asserts the message token matches before rendering).

### 3.4 Inspector tabs (`prompt §2.2 right`, AC-31)
- **Tabs:** Headers · Query Params · Body · Response Served · State & Tracing. **Alpine** owns the active tab
  (instant switch, no fetch). The **panel content** for the selected request is loaded **once** by HTMX when the
  row is clicked (full detail in one partial) and tabs `x-show` slices of it — avoids a fetch per tab.
  `[ARCH-GAP: whether full detail is pushed inline on the WS event or lazy-loaded — OQ-11; design accommodates both]`.
- **Headers / Query Params:** definition-list style key/value rows (mono keys), each value copyable. Replaces the
  raw `JSON.stringify(...)` `<pre>` dump in `dashboard.html:233-234` with a scannable table; a "raw" toggle keeps
  the `<pre>` for power users (reuse the `pre/code` style).
- **Body (collapsible JSON/XML tree):** `inspector_body_tree.html` recursive partial. Each object/array node has
  a disclosure triangle (Alpine `x-data="{open:true}"`), key in muted color, value syntax-tinted, arrays show
  length, long strings truncate with expand. Top controls: **Expand all / Collapse all**, **Copy**, **Raw/Pretty**
  toggle (reuse `prettyJson` idea, `dashboard.html:254`). XML → tagged tree; non-JSON/XML → mono `<pre>` fallback.
  Tree built **server-side** in Jinja (recursion) so it works without JS; Alpine only toggles open/closed.
- **Response Served:** the status (color-coded), response headers (incl. injected CORS, AC-19), response body
  tree, applied latency, and **served-by** label: `matched rule "<name>"` / `Auto-CRUD` / `proxied (MITM)` /
  `default 404` / `chaos 503` (AC-13/15/16/26) `[ARCH-GAP: served_by enum, OQ-10]`.
- **State & Tracing:** an ordered, readable trace of resolution for this request: which rules were evaluated and
  why each matched/skipped (incl. **state conditions**, AC-9), **state mutations** applied (before→after, AC-8),
  CORS handling, and the final branch taken. This is the feature's debugging payoff — render as a vertical
  step list, not raw JSON. `[ARCH-GAP: trace payload shape — OQ-9/10/11]`.

### 3.5 Create-Rule modal interactions (AC-33/34)
- **Open:** `[+ New Rule]` → HTMX `hx-get` loads `rule_modal.html` into the modal shell (fresh) or
  `…/rules/{id}` for edit; Alpine opens the overlay. Tab nav is Alpine; **all 5 tabs share one form** so a single
  Save submits everything.
- **Validation (client + server):** required = method + path; invalid JSON in body/headers shows an inline error
  on that field (reuse the inline-alert style) and marks the owning tab's dot red; status code constrained
  100–599 (reuse `mock.html:116`). The footer summarizes count of fields needing attention and **disables Save**
  until resolved. On submit, HTMX `hx-post`/`hx-patch` to `/api/.../rules`; **server-side** validation errors
  return the partial **with errors rendered** (HTMX swaps it back) — no full reload.
- **Success:** modal closes, `showToast('Rule saved')`, the rules list refreshes via HTMX (`hx-trigger` on a
  custom `rule-saved` event), and the interceptor honors it next request (AC-33/34, BE).
- **Edit / disable / delete (AC-34):** rules list (in Settings or a rules drawer) shows each rule with an
  enable/disable toggle (optimistic Alpine flip + HTMX `hx-patch`; revert + toast on failure), Edit (reopens
  modal prefilled), Delete (`btn-danger` + confirm, reuse `dashboard.html:242`).
- **Preview render (templating, AC-20-23):** "Preview" posts current body + sample request context to a dry-run
  endpoint `[ARCH-GAP]`; renders result in a panel; on engine error shows the fail-safe message, never a 500 dump.

### 3.6 Universal states (every async surface)
For each data region define all five (the old templates only partly did):
- **Loading:** skeleton rows in the feed; a spinner/`aria-busy` in the inspector and modal (HTMX `hx-indicator`).
  Disable submit + change label during in-flight (reuse `submitBtn.disabled` pattern, `login.html:45`).
- **Empty:**
  - Feed: friendly empty state with the **mock URL to copy** and a "Send a test request" hint
    (reuse "Test Your Mock" sender from `mock.html:170-179`) — turns a dead screen into a first-call funnel.
    (Better than today's bare "No requests captured yet", `dashboard.html:205`.)
  - Inspector (nothing selected): centered hint "Select a request to inspect."
  - Rules list: "No rules yet — unmatched requests use Auto-CRUD / proxy / default 404" + `[+ New Rule]`.
- **Error:** inline alert (reuse `bg-red-900/50…`); inspector/feed show a retry affordance; WS errors flow to the
  pill (§2.4) not a blocking modal. 404/unknown endpoint on `/d/<token>` → a clear "Endpoint not found" card
  (reuse `dashboard.html:128`) with a link back to `/`.
- **Success:** toast + optimistic UI where safe (toggles); inline green alert on the entry/settings forms
  (reuse `login.html:16`).
- **Disabled:** Save disabled until valid; toggles disabled while their PATCH is in flight; "Preview"/"Test"
  disabled while running; the Webhook-action block disabled if deferred (§2.3 tab 4).

### 3.7 Responsive
Developer tool → desktop-first, but degrade gracefully. ≥1024px: side-by-side split. <1024px: feed full-width;
clicking a row opens the inspector as a **full-screen drawer** with a back button (the split becomes a stack).
Modal/settings go near-full-screen on small viewports. The entry screen is already responsive (`max-w-md mx-auto`).

---

## 4. Copy — labels, button text, empty-state & error text

**Entry `/`:**
- Title: **"HookBox"**; subtitle: **"Mock, intercept, and inspect any HTTP API. Enter your email to get an
  instant mock endpoint — no password."** (states the no-wall model, LOCKED §5).
- Input label **"Email address"**, placeholder `you@example.com` (reuse `login.html:23`).
- Button **"Get my mock endpoint"** (in-flight: **"Setting up…"**).
- Helper under input: **"Your email is your access key — reuse it to resume your endpoints."**
- Error (422/malformed): **"That doesn't look like a valid email address."**
- Network error: **"Couldn't reach HookBox. Check your connection and try again."** (reuse `login.html:76`).

**Dashboard bar / feed:**
- Mock URL chip label: **"Your mock URL"** with subdomain + a **"Local fallback"** chip for `/e/<token>/…`
  (LOCKED §2 / AC-5). Copy → toast **"Copied!"** (reuse `base.html:63`).
- WS pill: **"Live"** / **"Reconnecting…"** / **"Offline"**.
- Feed toolbar: **"Pause"/"Resume"**, **"Clear"**, search placeholder **"Filter by path…"**, **"N new"** pill.
- Feed footer: **"Showing {n} of last 100"** (ties retention cap to UI, §J).
- Feed empty: **"No requests yet."** / **"Point your app at the URL above, or send a test request to see traffic
  stream in live."**

**Inspector:**
- Tabs: **Headers**, **Query Params**, **Body**, **Response Served**, **State & Tracing**.
- Empty: **"Select a request on the left to inspect it."**
- Body controls: **Expand all / Collapse all / Copy / Raw / Pretty**.
- Served-by chips: **"Matched rule"**, **"Auto-CRUD"**, **"Proxied"**, **"Default 404"**, **"Chaos"**
  `[ARCH-GAP labels, OQ-10]`.

**Create-Rule modal:**
- Title **"Create rule"** / **"Edit rule"**; tabs **Matching · Response · Templating · Actions · Throttling**.
- Buttons **"Cancel"**, **"Save rule"** (in-flight **"Saving…"**); **"Format JSON"**, **"Preview render"**,
  **"+ Add condition"**, **"+ Add state write"**.
- Matching helper: **"This rule matches when all conditions below are true."** Live summary e.g.
  **"GET /books/:id when state.authenticated == true"**.
- Templating helper: **"Insert a tag, then Preview to see it rendered. Unknown tags are left as-is."** (AC-23).
- Throttling helpers: **"Delay each response (0–10000 ms)."** · **"Max requests per minute (extra requests get
  429)."** · **"Percent of requests that fail with a random 502/503/504."**
- Validation summary: **"{n} field(s) need attention."** Field errors: **"Enter a path."**, **"Status code must
  be 100–599."**, **"Response headers must be valid JSON."**

**Settings panel:**
- Auto-CRUD: **"Auto-CRUD"** — **"Turn this endpoint into an instant REST backend. POST/GET/PUT/DELETE
  /<collection> works with no rules."** (AC-11).
- MITM: **"Target real API URL"** — **"Unmatched requests are forwarded here, captured, and returned."** (AC-14).
- CORS: **"Auto-CORS is on. Preflight OPTIONS and wide-open CORS headers are added to every response."** (AC-18/19).
- Danger zone: **"Clear state"**, **"Clear traces"**, **"Delete endpoint"** + confirm
  **"Delete this endpoint and all its traces? This can't be undone."**

**Tone:** terse, developer-direct, lowercase-y, no marketing fluff — matches the current copy register
("Self-hosted webhook testing", `index.html:12`).

---

## 5. Accessibility — semantics, focus, keyboard, contrast, ARIA

- **Semantics:** real `<nav>`, `<main>`, `<form>`, `<button>` (not clickable `<div>`s — the old feed rows used
  `onclick` on `<tr>`, `dashboard.html:211`; new rows are `<button>`/`<a>` or `<tr>` with `tabindex=0` +
  `role=button` + Enter/Space handlers). The split panes are `<section aria-label="Live request feed">` and
  `<section aria-label="Request inspector">`.
- **Tabs (inspector & modal):** `role="tablist"` / `role="tab"` (`aria-selected`, `aria-controls`) /
  `role="tabpanel"` (`aria-labelledby`). Arrow-key navigation between tabs; active tab in the roving tabindex.
- **Live feed:** `#feed` is `aria-live="polite"` (or a visually-hidden polite region announcing "New GET
  /books, 200") so screen-reader users learn of traffic without being spammed; **Pause** also pauses
  announcements. New-row highlight is **not** color-only — pair with the brief motion + the row already carrying
  text (method/path/status).
- **WS pill:** `role="status"` `aria-live="polite"`; never color-only — always a text label ("Live"/"Offline"),
  satisfying non-color signaling.
- **Method badges & status codes:** color-coded **and** always show the literal text (GET/POST…, 200/404…) so
  meaning never depends on hue (color-blind safe). The design-agent must verify each badge meets **WCAG AA
  contrast** on `#161b22`/`#0d1117` (PRD OQ-15) — the current single blue `#388bfd` on `#161b22` is the baseline
  to keep or beat.
- **Focus management:** opening the modal moves focus to its first field and **traps** focus within; `Esc` closes
  (reuse `dashboard.html:258`) and returns focus to the trigger. Drawers likewise. All interactive elements show
  a visible focus ring (reuse `focus:border-[#58a6ff]`; add `focus-visible` outline for non-input controls).
- **Keyboard:** entire dashboard usable without a mouse — `Tab` order feed→inspector→bar; `↑/↓` move feed
  selection, `Enter` inspects; `1–5` (when inspector focused) switch tabs; `/` focuses feed filter; `n` opens New
  Rule; `Esc` closes overlays. Document shortcuts in a small "?" affordance.
- **Forms:** every input has a programmatically-associated `<label>` (the current templates do this — keep it);
  errors referenced via `aria-describedby`; `aria-invalid` on failed fields; `aria-busy` during HTMX requests.
- **Reduced motion:** wrap the WS-pulse and row-flash in `@media (prefers-reduced-motion: reduce)` to disable
  animation (the live feed must stay usable for vestibular-sensitive users).
- **Copy buttons:** give `aria-label="Copy mock URL"` etc. (the current `📋` button has no accessible name,
  `index.html:95` — fix in the rewrite).
- **Contrast caveat:** muted `#8b949e` on `#0d1117` is ~AA for normal text — keep it for secondary text only, not
  for anything load-bearing; the design-agent confirms (OQ-15).

---

## 6. Consistency notes — patterns/classes reused from existing templates

- **Whole visual system** (palette, `.card`, `.btn-*`, `.badge`, `pre/code`, `.toast`, inputs, toggle switch,
  inline alerts, breadcrumb, WS status colors) is reused **verbatim** from `base.html` + `mock.html` + `login.html`
  — see the §0 table for exact citations. Net-new CSS is minimal: the split-screen flex shell, the WS-pulse
  keyframe, the row-flash, the JSON-tree disclosure, and badge color variants per method (design-agent).
- **Modal**: reuse `dashboard.html:15-23` geometry/backdrop (`fixed inset-0 bg-black/50`, `max-w-4xl
  max-h-[80vh]`, header + `×`, `Esc` close).
- **Session/nav**: reuse the `hookbox_user` localStorage contract (`base.html:43`) — the new entry screen writes
  it (as `login.html:69` does today) and the dashboard reads it; just routed to `/d/<token>` instead of `/`.
- **Toggle, inputs, selects, textareas, status-code field**: lifted from `mock.html` (cited in §0/§2.3) so the
  rule modal and settings feel identical to the config the user already knew.
- **Feed row shape**: keeps the method-badge + path + (now) status columns that `dashboard.html:211-216` and the
  WS `new_request` handler (`dashboard.html:60-77`) already use — the rewrite *extends* `broadcast_new_request`
  (`app/websocket.py:38`) rather than inventing a new mental model, so the feed maps cleanly to existing BE shape.
- **Helpers**: keep `showToast`, `copyToClipboard`, `formatDate` relative-time, `prettyJson`, `escapeHtml`
  (`base.html`/`dashboard.html`) — move into a shared `static/js/util.js` `[new]` instead of duplicating per page.
- **Deliberate divergence (and why):** we stop rendering pages via JS template-literal `innerHTML` (today's de-facto
  pattern) in favor of server-rendered Jinja + HTMX partials + Alpine. This is **more** consistent with the LOCKED
  stack than the status quo, removes the hand-rolled client framework, and is the only divergence from "match what
  exists." It is justified by `_decisions.md` §1/§1a and improves first-paint, escaping/XSS safety (Jinja
  auto-escape vs manual `escapeHtml`), and accessibility.

---

## 7. PRD gaps — UI requirements / ACs the PM must add or clarify

> Numbered; each is a UI-facing requirement or AC the PRD/architect should add or pin down. These complement the
> existing `[ARCH-GAP]`/OQ markers and should feed OQ-15 and REVISE.

1. **Entry-screen copy & multi-endpoint resume (AC-1/2/3).** PRD says email → token, but the UI must know: does a
   returning email land directly on `/d/<token>`, or on an endpoint **picker** if the owner has several? Add an AC
   for the >1-endpoint case (today there's no list screen in the new IA). Confirm the exact success/route copy.
2. **First-paint vs WS-only feed.** Add an AC that the feed's recent traces are **server-rendered on first load**
   (progressive enhancement), not fetched only after JS — current `dashboard.html` shows nothing pre-fetch. State
   whether the trace list endpoint returns HTML partials (HTMX) or JSON the FE renders.
3. **Inspector data delivery (ties OQ-11).** Decide and AC whether full trace detail (headers/body/response/trace)
   is **pushed inline** on the WS `new_request` event or **lazy-loaded** by HTMX on row click. This changes payload
   size, the live-feed cap math, and whether the inspector ever shows a loading state.
4. **`served_by` taxonomy + identifying header surfaced in UI (AC-13/15/16, OQ-10).** The "Response Served" and
   "State & Tracing" tabs need a **frozen enum** of how a request was handled (matched-rule / Auto-CRUD / proxied /
   default / chaos / rate-limited) and the `X-HookBox-*` header names to display. Add as ACs so AC-28/31 are testable.
5. **Trace/tracing payload shape (AC-31 "State & Tracing").** Define what the trace contains (rules evaluated,
   match/skip reasons, state before→after, CORS, final branch) so the tab is implementable. Currently unspecified
   beyond prose; promote to a concrete AC + schema field (architect).
6. **"Webhook Actions" tab scope (AC-33, OQ-9).** PM must decide **in or out** for this milestone. The modal needs
   the answer to either build live fields or render them disabled-with-explanation. Add an explicit scope AC.
7. **Rule-level vs endpoint-level latency/rate/chaos precedence (AC-24/25/26).** The UI shows both (modal tab 5 +
   Settings); the PRD/architect must state which wins and whether rule-level is an override or additive — needed
   for accurate helper copy and to avoid contradictory UI.
8. **Templating "Preview render" endpoint (AC-20–23).** The Templating tab's Preview and the rule modal's safety
   story need a **dry-run render endpoint** (input: body + sample request/state; output: rendered or fail-safe
   message). Not in PRD §5.2 — add it, or drop Preview from this milestone (state which).
9. **Auto-CRUD discoverability (AC-11/12).** When Auto-CRUD is ON and rules-vs-CRUD precedence applies (AC-13),
   what does the UI show — detected collections? a hint that some paths are served by CRUD? Add a small AC so the
   Settings/feed don't mislead (e.g. a `served_by: Auto-CRUD` chip suffices, but confirm).
10. **Pause/auto-follow behavior (AC-27/30).** A live feed needs a defined pause/resume + "N new" buffering
    behavior so reading a row isn't disrupted by incoming traffic. Promote to an AC (interaction detail not yet in
    PRD; affects AC-30's "without locking the DOM" intent).
11. **Feed cap = retention cap, surfaced (AC-30/35).** Confirm the client feed cap equals the 100-trace retention
    cap and that the UI states "last 100." Add an AC tying the visible cap to retention so the number isn't arbitrary.
12. **Empty-state "send a test request" affordance.** The empty feed should offer an in-dashboard test sender
    (reuse `mock.html`'s) to bootstrap the first request. Add as a (minor) AC, or explicitly cut it — it strongly
    affects first-run UX/activation.
13. **Connection-health precision (AC-29).** AC-29 says "connected vs reconnecting/disconnected." Confirm the
    **three** discrete states (Live / Reconnecting / Offline) and that reconnecting exposes attempt/backoff info,
    so design-agent and QA have a testable target.
14. **Mock-URL canonical example + clickability (AC-44, OQ-14).** Fix the exact `MOCK_DOMAIN` example token-URL
    shown in chips and confirm they're **copy-only, not anchors** (the dashboard origin ≠ mock origin) to prevent
    users clicking into the wrong plane. Tie to OQ-14.
15. **Settings vs `/d/<token>/…` routing.** PRD §3 lists "Endpoint settings + Rule builder (modal/panel … or
    `/d/<token>/…`)" ambiguously. Confirm Settings + rule modal are **overlays on `/d/<token>`** (this doc's
    assumption) vs separate routed pages, so HTMX targets/`hx-push-url` and back-button behavior are specified.
16. **Export/restore placement (OQ-7).** If export/restore survives, the PRD must say it lives in the Settings
    panel (this doc's assumption) rather than the removed `/backup` page, so the nav and IA are final.
17. **Logout/identity-switch (LOCKED §5).** Define the UI for clearing the localStorage session and entering a
    different email (the new nav must offer it). Add a small AC so the no-account model is still escapable.
18. **Accessibility ACs (feeds OQ-15).** Promote the AA-contrast requirement for every method/status badge,
    keyboard operability of the feed+tabs+modal, `aria-live` on feed and WS pill, focus-trap on modal, and
    reduced-motion handling into explicit ACs so QA can verify them and design-agent's palette is constrained.
