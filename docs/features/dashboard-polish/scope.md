# Dashboard design-elevation — SCOPE (slug: dashboard-polish)

> **Mode:** product-manager coordination of a **POLISH / ELEVATION pass on a working UI.**
> Grounded in the *actual* current templates + render (verified file-by-file below), not a
> rebuild. Every item is concrete, testable, and **functionality-preserving**. Authority: this
> doc builds on `docs/features/beeceptor-rewrite/ux.md` + `design.md` and obeys the LOCKED
> constraints in `docs/features/dashboard-polish/_brief.md`. The frozen FE↔BE contract is
> `docs/features/beeceptor-rewrite/prd.md` §5 and is **not** touched here.
>
> **Lane:** frontend only — edit `templates/` + `static/`. **Never** touch `app/` (esp.
> `app/routes/ui.py`, where the render fix lives). No external CDN / `src="http…"`; assets stay
> vendored at `/static/vendor/`. Keep a11y (ARIA, `aria-live`, focus, `prefers-reduced-motion`,
> ≥4.5:1 contrast) and the XSS-safe `x-text` rendering of captured data (never `x-html` /
> `innerHTML` / `|safe` on captured data).

---

## 0. Grounding — what is verified to render today (read before scoping)

The dashboard and landing **render and fully work**. Verified surfaces and their files:

- **Landing** — `templates/index.html` (`landing()` Alpine component): email → `POST /api/session`
  → localStorage `hookbox_owner` → `/d/<token>`; auto-resume, private-mode banner, 422/429/network
  states. Centered `.card` on `templates/base.html`.
- **Global shell / nav** — `templates/base.html`: vendored `tailwind.js` + `alpine.min.js` +
  `htmx.min.js`; inline `<style>` (`.card`/`.btn-*`/`.badge`/`.toast`/`pre`/`code`/`[x-cloak]`);
  `HookBox.*` identity helpers; `owner` nav block; `#toast` + `showToast()` + `copyToClipboard()`.
- **Dashboard shell** — `templates/dashboard.html` (`dashboard(token)`): endpoint bar (switcher,
  `+ New endpoint`, Auto-CRUD toggle, Rules, `+ New Rule`, Settings) + split-screen
  (`w-[40%] min-w-[360px]` feed / `flex-1` inspector), `notFound` state, owner-mismatch bounce.
- **Live feed** — `dashboard.html` `#feed` listbox + `templates/partials/feed_row.html`: method
  badge, path, status, served-by chip, latency, time; WS pill; Pause / "N new"; mock-URL chips;
  skeleton + empty states; keyboard nav.
- **Deep inspector** — `templates/partials/inspector.html` + `inspector_body_tree.html`: tabs
  Headers · Query · Body · Response Served · State & Tracing; lazy `GET /api/requests/{id}`;
  loading/pending/unauthorized/error gates; JSON tree; trace step list.
- **Rule builder** — `templates/partials/rule_modal.html` + `rule_row.html` +
  `static/js/rule-builder.js`: 5-tab modal (Matching·Response·Templating·Actions·Throttling),
  vertical rail with per-tab error dots, validation footer, disabled Webhook sub-section.
- **Settings overlay** — `templates/partials/endpoint_settings.html`: name, Auto-CRUD, Auto-CORS,
  MITM target, default mode, latency slider, rate-limit, chaos dial, danger zone.
- **Design layer** — `static/css/app.css` (tokens + `.feed-row`/`.m-*`/`.ws-*`/`.tab-btn`/
  `.kv-row`/`.tree-*`/`.trace-*`/`.mock-chip`/`.skeleton`/keyframes/reduced-motion).
- **Client logic** — `static/js/stores.js` (stores), `request-stream.js` (feed+stream),
  `util.js` (maps + XSS-safe helpers), `rule-builder.js`.

**Two grounded findings that shape this scope (not bugs to fix in `app/`, but FE consequences):**

1. **Server-side first paint is dormant.** `feed_row.html` and the `#hb-initial-feed` island are
   gated on a Jinja `initial_requests` var, but `app/routes/ui.py:59` renders `dashboard.html` with
   only `token` (+ domain ctx) — it passes **no** `initial_requests`. So today the feed is
   blank-until-WS-backfill. We are in the FE lane and **must not** edit `ui.py`, so AC-27a's
   server-render path is currently unreachable. Scope item (g) treats the **JS-driven loading
   skeleton** as the real first-paint experience to elevate, and flags the `initial_requests` wiring
   as an explicit **out-of-lane note** for the architect/BE (see §3). Do **not** delete the
   `initial_requests`/`feed_row.html` server path — keep it intact for when BE wires it.
2. **The split is desktop-mostly.** `dashboard.html:99` uses `flex-col md:flex-row` and the inspector
   mounts in a `<section>` that has no mobile drawer/back affordance; `ux.md §3.7` / `design.md §7`
   specify a `<1024px` full-screen inspector drawer that does **not** exist yet. This is a real polish
   gap, scoped under (d)/(h) as a CSS/Alpine-only enhancement (no store API change).

---

## 1. LOCKED — must NOT change (regression surface for QA + the design-agent)

These are the load-bearing seams every item below must preserve. Verified by grep across
`static/js/` + `templates/`.

**Alpine stores + their public APIs (do not rename, remove, or change call signatures):**
- `owner` — `isLoggedIn`, `email`, `token`, `secret`, `mockUrl`, `set()`, `setToken()`, `logout()`.
- `feed` — `rows`, `buffer`, `pending`, `paused`, `selectedId`, `capacity`, `shownCount`,
  `footerText`, `select(id)`, `togglePause()`, `flush()`.
- `stream` — `state`, `attempt`, `label`, `dotClass` (states: `connecting|reconnecting|live|offline|degraded|unauthorized`).
- `inspector` — `requestId`, `detail`, `tab`, `status`, the `isLoading/isPending/isReady/isError/isUnauthorized`
  getters, the body/header/trace getters, `setTab()`, `select()`, `clear()`, `retry()`, `load()`.
- `endpoint` — `token`, `detail`, `list`, `loading`, `saving`, `error`, `saveError`, `creating`,
  `BOUNDS`, `clampNum()`, `load()`, `loadList()`, `save()`, `setAutoCrud()`, `createEndpoint()`,
  `clearState()`, `clearHistory()`.
- `rules` — `token`, `list`, `loading`, `error`, `load()`, `create()`, `update()`, `toggle()`,
  `remove()`, `_sort()`.

**Cross-component events (the decoupling contract):** `hb:select-request`, `hb:endpoint-updated`,
`hb:state-changed`. **Entry point:** `HookBox.startRequestStream(token, { cap })`.

**Element IDs / hooks the JS reads:** `#hb-initial-feed` (JSON island parsed by `request-stream.js`),
`#feed` (listbox), `#toast`. Feed rows expose `data-request-id` (used by keyboard `scrollIntoView`).
`x-ref`s `rulesBtn` / `settingsBtn` / `dialog` (focus restore). The dashboard's `dashboard()` scope
exposes `token`, `notFound`, `settingsOpen`, `rulesOpen`, `ruleModalOpen`, `editingRule`,
`openRules/closeRules/openNewRule/editRule/closeRuleModal/confirmDelete/openSettings/closeSettings`
and the render helpers (`methodBadge/methodRail/statusClass/servedClass/servedLabel/relTime`) — the
partials depend on these names.

**§5 contract shapes (FROZEN — never add/rename a client-visible field here):** `SessionResponse`,
`EndpointSummary/Detail`, `EndpointConfigPatch`, `MockRule*`, `RequestSummary/Detail`, `TraceEvent`,
the 5 WS message types, the `served_by` enum
(`rule|crud|mitm|tunnel|default|cors|chaos|ratelimit`), the `?cap=` WS handshake, all status codes.
A pure-visual pass introduces **no new field**; if any item below seems to need one, it is out of
scope and goes to the architect.

**Local assets only.** No `<link>`/`<script>`/`@import`/`url()`/`<img src>` to any `http(s)://`
origin. Fonts stay the system stack + Monaco/Menlo (`--font-sans` / `--font-mono`). Any icon must be
inline SVG or a unicode glyph (current code already uses glyphs: `▸`, `●○◆✕◇▸`, `×`).

**Token discipline.** Reuse `static/css/app.css` `:root` tokens and the GitHub-dark palette. New
colors are out of scope unless they extend the existing semantic set; any new hue must pass the
`design.md §8.1` contrast bars and be added as a token, not a one-off hex.

---

## 2. Prioritized enhancement scope (P0 → P3)

> Priority key: **P0** = highest-leverage polish that lifts the whole product; **P1** = strong
> per-surface elevation; **P2** = refinement; **P3** = nice-to-have. Each item names the file(s),
> the exact change, and a **testable** acceptance check. All are FE-lane + functionality-preserving.

### (a) Landing — `templates/index.html`, `templates/base.html`

- **a1 (P1) Brand + first-impression polish.** The landing reuses the generic centered `.card`; the
  product reads as "a form", not "a precision instrument" (`design.md §1`). Add a compact branded
  header lockup: wordmark "HookBox" in `--text` with a one-line `--link` accent rule or inline-SVG
  glyph mark (local SVG only), and tighten the subtitle to the `ux.md §4` copy register. *Test:* `/`
  shows the elevated lockup; no `http` asset; existing form/`init()`/`submit()` untouched; submit
  still posts `/api/session` and routes to `/d/<token>`.
- **a2 (P2) Value-prop strip below the card.** Add 3 muted, icon-or-glyph feature bullets (Mock ·
  Intercept · Inspect) using `--text-muted`, purely presentational, inside the existing
  `max-w-md` column so layout/centering is preserved. *Test:* bullets render; tab order is
  email-input → submit (bullets are non-interactive); contrast ≥4.5:1.
- **a3 (P2) Field + button state finish.** The email input uses ad-hoc `border-[#30363d]`; elevate
  to a consistent focus ring (reuse `:focus-visible` from app.css), and give the submit button a
  clearer disabled/`Setting up…` treatment via `.is-disabled`. *Test:* keyboard focus shows the ring;
  `submitting` disables + relabels; 422 path still shows the constant inline error (AC-S5 copy
  unchanged).

### (b) Dashboard shell + endpoint bar — `templates/dashboard.html`, `static/css/app.css`

- **b1 (P0) Endpoint-bar hierarchy + grouping.** The bar packs switcher, `+ New endpoint`,
  Auto-CRUD, Rules, `+ New Rule`, Settings into one flat `flex` row (`dashboard.html:56-97`) with
  uneven visual weight. Elevate to clear clusters: **left** = endpoint identity (switcher + new),
  **right** = actions, with a hairline `--border` divider and consistent `text-sm`/`px` rhythm so
  `+ New Rule` reads as the single green primary (`design.md §4.3`). Keep every `@click`/`x-ref`/
  binding. *Test:* the only filled-green control in the bar is `+ New Rule`; switcher + Auto-CRUD
  still function; no JS hook renamed.
- **b2 (P1) Mock-URL chip prominence in the bar.** Today mock-URL chips live only inside the feed
  column (`dashboard.html:132-151`); the canonical thing a developer copies is buried. Surface the
  primary mock-URL chip (copy-only, neutral, never link-blue per `AC-48`/`VC-7`) in/under the
  endpoint bar on wide screens while keeping the feed-column chips for narrow. Reuse `.mock-chip` +
  existing `copyToClipboard`. *Test:* chip is a `code`+`Copy` button, `--text-2` (not `--link`),
  `aria-label` present, copies the URL; no new field; functionality of the feed chips preserved.
- **b3 (P2) Sticky-bar depth + scroll affordance.** The endpoint bar and feed/inspector headers use
  `panel-flush` with only a bottom border; under a streaming feed they can feel flat. Add a subtle
  `--shadow-pop`-on-scroll (or a stronger bottom hairline) so sticky chrome separates from scrolling
  content (`design.md §2.7` confines shadows to floating/sticky layers). *Test:* sticky headers stay
  legible over scrolled rows; reduced-motion unaffected (shadow is static).
- **b4 (P2) Switcher legibility.** The `<select id="ep-switcher">` is a raw native control. Keep it
  native (a11y + zero-dep), but elevate padding, `--border-strong` on focus, and a leading "Endpoint"
  micro-label so it reads as identity, not a stray dropdown. *Test:* `@change="switchEndpoint(...)"`
  intact; focus ring visible; label associated (`sr-only` retained or upgraded).

### (c) Live feed density / legibility — `templates/partials/feed_row.html`, `dashboard.html`, `static/css/app.css`

- **c1 (P0) Column rhythm + alignment.** Rows are a `gap:8px` flex where path width, status, chip,
  latency, and time float without a stable grid, so the eye can't scan a column (`design.md §4`
  wants method badges as a fixed gutter and right-aligned status/time). Elevate `.feed-row` to a
  consistent internal layout: fixed-width method-badge gutter, `flex-1` truncating path, then a
  right-aligned metadata cluster (status · served chip · latency · time) with tabular alignment.
  Keep the exact span structure + `data-cell` hooks that `request-stream.js` mirrors. *Test:* a
  feed of mixed methods/statuses aligns into scannable columns; server-rendered (`feed_row.html`)
  and JS-built rows are pixel-identical; `data-request-id`/`data-cell` preserved.
- **c2 (P1) Density tightening + hover/selected finish.** Bump density toward the `text-sm`/`text-xs`
  tool register (`design.md §2.6 VC-12`): tighten vertical padding a touch, ensure the selected
  method-accent rail (`.rail-*.is-selected`) and `--surface-2` fill read clearly, and verify the
  served-by outline chip never competes with the method badge. *Test:* selected row shows the
  method-colored left rail + brightened text; hover = `--surface-2`; one filled badge per row.
- **c3 (P1) Status + method color audit against tokens.** Confirm every `.m-*` and `.status-*`
  pairing matches `design.md §2.4/§2.5` and the `AC-VC1`/`AC-VC9`/`AC-VC2` contrast bars on the
  actual render (OPTIONS badge is the known borderline — `VC-2b` fallback `#c9d1d9`). This is an
  audit + token-tune, not new colors. *Test:* automated/manual contrast check ≥3:1 (badges/status)
  and 4xx amber visibly distinct from 5xx red; literal method/status text always present.
- **c4 (P2) Relative-time + footer clarity.** `relTime()` already renders "12s ago"; ensure the
  `--text-faint` timestamp is non-load-bearing (`VC-2a`) and the footer `footerText` ("Showing n of
  last 100") reads as a calm caption. *Test:* footer present; time uses faint token only as
  secondary; `feed.footerText` API unchanged.
- **c5 (P3) Optional client-side method filter chips.** `ux.md §3.2` mentions method filter chips;
  currently absent. If added, they must filter **rendered** `feed.rows` view-only via a local Alpine
  computed in the `dashboard()` scope **without** mutating `$store.feed.rows` (which `request-stream.js`
  owns) and without touching the cap/dedupe logic. *Test:* toggling a chip hides/shows rows visually;
  `feed.rows` array length and stream behavior unchanged; clears cleanly. *(Defer if it risks the
  store contract.)*

### (d) Deep inspector — `templates/partials/inspector.html`, `inspector_body_tree.html`, `static/css/app.css`

- **d1 (P1) Header strip + tab-strip finish.** The inspector header (`inspector.html:54-62`) and
  underline tab strip (`.tab-btn`) work but are visually thin. Elevate: stronger sticky header with
  the method badge + mono path + status echoing the selected feed row (the "one subject, one color"
  tie of `design.md §4.2`), and a clearer active-tab underline + hover. Keep `role=tablist`, arrow-key
  nav, `aria-selected`, `aria-controls`, and the `inspectorPanel()` tab list. *Test:* selecting a row
  shows a header that repeats the row's method color; tab keyboard nav intact; all 5 tabs switch.
- **d2 (P1) KV-row + JSON-tree readability.** `.kv-row` keys at `min-width:9rem` can crowd; tree
  indentation (`node.depth * 14px`) + syntax tints work but can be tightened for scan-ability
  (`design.md §3.4`). Elevate KV alignment, copy-button affordance, and tree line-height / disclosure
  triangle finish — purely CSS + existing markup. *Test:* headers/query/body render aligned;
  collapse/expand still driven by `bodyTree()` `visibleNodes`; values remain `x-text` (XSS-safe).
- **d3 (P1) Trace step list as the debugging payoff.** `.trace-step` glyphs (`●○◆✕◇▸`) + before→after
  diff are the feature's payoff (`design.md §3.4`); elevate visual hierarchy (glyph color weight,
  the `before → after` diff treatment, a distinct "final branch" emphasis) so a trace reads as a
  vertical timeline, not a list. Keep the `decorateTraceStep` output contract (`step/detail/cls/glyph/diff`).
  *Test:* matched=green ●, skipped=faint ○, state-write=info ◆, chaos=danger ✕; diff shows
  faint→success; reduced-motion safe.
- **d4 (P0) Mobile inspector drawer.** Per finding #2: on `<1024px` the inspector is a stacked
  section with no way back to the feed and no slide-in (`ux.md §3.7`, `design.md §7`). Add a
  CSS+Alpine-only full-screen drawer behavior keyed off `$store.feed.selectedId` with a back/close
  control that calls `$store.feed.select(null)` (existing API) to return to the feed. No store change,
  no new endpoint. *Test:* at narrow width, selecting a row slides the inspector over the feed with a
  back button; back returns to the feed; desktop split unchanged; `prefers-reduced-motion` disables
  the slide.
- **d5 (P2) Body tab controls.** `ux.md §3.4`/`design.md §3.4` call for Expand-all / Collapse-all /
  Copy / Raw-vs-Pretty on the Body tab; only the tree + a `<pre>` fallback exist. Add these as local
  controls in `bodyTree()`/the Body panel (toggle the existing `openState`, copy via
  `copyToClipboard`, raw via the already-present `reqBodyRaw`/`resBodyRaw`). *Test:* Expand/Collapse
  all toggles every node; Copy copies the pretty body; Raw shows `<pre>`; no captured data via
  `innerHTML`.

### (e) Rule-builder modal — `templates/partials/rule_modal.html`, `rule_row.html`, `static/css/app.css`

- **e1 (P1) Modal shell + rail finish.** The 5-tab modal works but the vertical rail, panels, and
  footer are visually utilitarian. Elevate: rail active-state weight + the per-tab **error dot**
  treatment (`.bg-[#f85149]` → token), panel spacing, and the footer validation summary so "N fields
  need attention" + disabled Save reads clearly (`design.md §3.9`, `AC-VC14`). Keep the whole
  `ruleModal()` form, `validation` computed, tab `role`s, focus trap, and `HookBox.RuleBuilder`
  contract. *Test:* invalid field marks its tab dot red + disables Save + jumps to first bad tab on
  submit; valid rule still POSTs/PATCHes via `$store.rules`.
- **e2 (P2) Field grouping + helper-text polish.** Matching/Response/Throttling panels use repeated
  `grid`/`flex` rows; tighten labels, helper text (`--text-muted`), and the key-value add/remove rows
  so dense forms feel organized. Reuse the shared input class verbatim. *Test:* all inputs keep their
  `x-model` bindings and ids; add/remove row buttons still mutate the form arrays.
- **e3 (P2) Templating tag palette as chips.** The Templating tab lists tags as `btn-secondary`
  buttons; elevate to a tidy mono chip palette (`design.md §3.10`) that still `appendTag()`s into the
  shared `form.bodyTemplate`. *Test:* clicking a chip appends the exact tag string; the shared body
  textarea updates in both Response + Templating tabs.
- **e4 (P2) Disabled Webhook sub-section clarity.** The deferred Webhook `fieldset` is `opacity-60`
  + disabled with a "coming per spec" note (`AC-33a`); elevate the dimmed/"deferred" visual so it
  clearly reads as intentionally-off, not broken. *Test:* fields stay `disabled`; the note is
  visible; serialization of `webhook_action` still happens when data is present (contract unchanged).
- **e5 (P1) Rule-row list finish.** `rule_row.html` rows (method badge + path + meta + toggle +
  Edit/Delete) are functional but cramped. Elevate spacing, the enable/disable toggle affordance, and
  the disabled (`opacity-60`) state. Keep optimistic `$store.rules.toggle`, `editRule`, `confirmDelete`.
  *Test:* toggle flips optimistically + reverts+toasts on failure; Edit opens prefilled modal; Delete
  confirms then `remove()`s.

### (f) Settings overlay — `templates/partials/endpoint_settings.html`, `static/css/app.css`

- **f1 (P1) Sectioning + scannability.** The overlay is a long single scroll
  (`endpoint_settings.html:41-171`). Elevate with clear section headers/grouping (Identity ·
  Behavior · Simulated network conditions · Danger zone), consistent label/helper rhythm, and visual
  separation of the danger zone (already `--danger`-bordered) so destructive actions are unmistakable.
  Keep `endpointSettings()` form, `snapshot()`, `saveSettings()`, all `x-model`s, and the focus trap.
  *Test:* every control keeps its binding; Save still PATCHes a clamped `EndpointConfigPatch`; Clear
  state / Clear history still confirm + call the store.
- **f2 (P2) Range/dial readability.** Latency + Chaos use native `range` inputs with a live mono
  value readout; elevate track styling (within CSS, no JS) and the min/max captions so the
  0–10000 / 0–100 bounds (`AC-27c`) are obvious. *Test:* slider value mirrors `form.latency_ms` /
  `form.chaos_pct` live; clamp behavior unchanged.
- **f3 (P2) Toggle + MITM-field finish.** Auto-CRUD / Auto-CORS toggles and the MITM `target_url`
  input (with its inline `targetError` http(s) hint) are functional; elevate spacing + the
  inline-error treatment to match the rule-modal field-error style. *Test:* invalid scheme shows the
  inline error + disables Save (client mirror of `AC-S6`); empty clears MITM; toggles persist.

### (g) Empty / loading / error states — `dashboard.html`, `inspector.html`, `endpoint_settings.html`, `rule_modal.html`, `static/css/app.css`

- **g1 (P0) Loading skeleton as the real first paint.** Per finding #1, the feed is JS-skeleton-then-
  backfill (server `initial_requests` is not wired and is out of our lane). Elevate the `.skeleton`
  rows to convincingly mirror real feed-row geometry (badge gutter + path bar + right cluster) so the
  loading state feels designed, not a grey block (`design.md §5`). *Test:* on load, skeleton rows
  match row layout; shimmer animates; static under `prefers-reduced-motion`; rows replaced by real
  data when `feed.rows` populates.
- **g2 (P1) Empty-feed as first-call funnel.** The empty state ("No requests yet" + copy chip) is the
  activation moment (`ux.md §3.6`). Elevate: friendlier hierarchy, the copyable mock-URL chip
  prominent, and a concrete "send a test request" hint (e.g. a sample `curl` line rendered as a
  copy-only `code` block — static text, not executed). *Test:* empty feed shows the chip + hint;
  copy works; no `http` asset; appears only when `!loading && rows.length === 0`.
- **g3 (P1) Inspector empty/pending/error finish.** "Select a request", "Loading detail…", "Detail
  still being written…" (+ Retry), unauthorized, and error states exist (`inspector.html:20-47`);
  elevate them into a consistent centered treatment with icon/glyph + calm copy so the lazy-load and
  fire-and-forget (`AC-31a`) states feel intentional. *Test:* each state renders with its existing
  gate (`isLoading/isPending/isUnauthorized/isError`); Retry still calls `$store.inspector.retry()`.
- **g4 (P2) Error/alert consistency.** Inline alerts use a mix of raw `bg-red-900/50…` and token
  styling across modal/settings/landing. Standardize on one elevated alert treatment (reuse the
  existing red alert classes consistently, or a token-based `.alert` helper) so errors look uniform.
  *Test:* save-error / load-error / network banners share one look; copy unchanged (esp. AC-S5
  email-exists-neutral copy).
- **g5 (P2) WS-health states are visible + labeled.** `Live / Reconnecting… / Offline / Realtime
  degraded / Unauthorized` (`AC-29`) are driven by `$store.stream.label`/`dotClass`; verify each is
  visually distinct, always text-labeled (never color-only), and the dot motion matches `AC-VC4`.
  *Test:* forcing each `stream.state` shows the right label + dot class; reduced-motion → static dots.

### (h) Motion — `static/css/app.css`, `templates/*` (transition utilities only)

- **h1 (P1) Honor the signature motions, verified.** `hb-pulse` (Live dot), `hb-throb`
  (reconnecting), `hb-flash` + `hb-slide` (arrival), `hb-shimmer` (skeleton) exist (`design.md §6`).
  Audit + tune timing/easing so liveness reads as calm, not frantic; ensure the arrival flash is the
  only per-row animation (no reflow) under burst. *Test:* `AC-VC4` (2s success pulse / amber throb /
  static red) + `AC-30b` (arrival flash ≤~900ms, batched) hold; the `feed-row--new` class is added
  then removed by `request-stream.js` (re-selection doesn't re-flash).
- **h2 (P1) Reduced-motion completeness.** The `@media (prefers-reduced-motion: reduce)` block
  disables `ws-dot--live`, `feed-row--new`, `skeleton`, `[class*="hb-"]` and clamps transitions.
  Extend it to cover any **new** motion added by this pass (drawer slide d4, sticky shadow b3, modal/
  panel transitions) so nothing animates under the preference. *Test:* with reduced-motion on, the
  drawer/feed/pill/modal show no animation and every state stays legible by color+text+glyph
  (`AC-VC17`).
- **h3 (P2) Micro-interaction finish.** Buttons/toggles/tabs/tree-triangle/copy-button transitions
  (`design.md §6.3`) — ensure consistent `--dur-fast` feel and a copy-success micro-confirmation that
  pairs with the existing "Copied!" toast. *Test:* hovers/toggles transition at the token duration;
  copy still toasts; no layout-thrash properties animated.

### (i) Cross-cutting a11y + focus (applies to every item above)

- **i1 (P0) Focus-visible everywhere (`AC-VC16`).** Every interactive element added/restyled keeps a
  visible focus ring (`:focus-visible` is global in app.css); modals keep their focus trap +
  `Esc`-restores-to-trigger (`endpoint_settings.html` / `rule_modal.html` `trapTab`). *Test:* keyboard
  tab through landing, bar, feed, inspector tabs, both modals shows rings; `Esc` restores focus.
- **i2 (P1) ARIA + live-region integrity.** Preserve `role=listbox/option` on the feed,
  `role=tablist/tab/tabpanel` in inspector + rule modal, `role=dialog aria-modal` on overlays,
  `role=status aria-live=polite` on the WS pill + toast, and `#feed`'s `:aria-live` flip on pause.
  Any restyle must not drop these. *Test:* the existing roles/labels remain; copy buttons keep
  `aria-label`s; no interactive `<div>` replaces a `<button>`.

---

## 3. Out-of-lane notes (flag to architect / BE — do NOT implement here)

- **N1 — Wire `initial_requests` for true server-side first paint (`AC-27a`).** `app/routes/ui.py`
  `dashboard()` should pass the recent `RequestSummary[]` (≤100) into the `dashboard.html` context so
  the existing `feed_row.html` server-render + `#hb-initial-feed` island activate (the FE already
  supports it). This is in the **backend lane** (`app/`) — out of scope for this polish pass. The FE
  keeps the dormant path intact. (Grounded: `ui.py:59` passes only `token`.)
- **N2 — No §5 changes implied.** Nothing in this scope needs a new request/response field, WS event,
  or status code. If implementation reveals a genuine need (e.g. a method-filter that needs a server
  facet, or a templating "Preview render" endpoint per `ux.md §2.3`/gap 8), it is **out of scope** and
  must go to the architect to amend §5 first — not invented in the FE lane.

---

## 4. Verification (matches `_brief.md`)

- `pytest` stays green, especially `tests/test_ui_render.py` (real templates render; **no** external
  CDN / `src="http…"` introduced).
- Headless-Chrome render of `/` and `/d/<token>` shows the elevated UI with every existing
  functionality intact (feed, inspector tabs, rules CRUD modal, settings overlay, switcher, Auto-CRUD).
- Spot-check the LOCKED list in §1: store APIs, events, element IDs, and §5 shapes unchanged.

---

## Summary (for the orchestrator)

The HookBox dashboard already renders with full functionality, so this is a **grounded design-elevation
pass, not a rebuild**: I audited every template, partial, the `app.css` token/component layer, and all
four JS files, and pinned the exact preservation contract (6 Alpine stores + their method signatures, the
`hb:*` events, the `#hb-initial-feed`/`#feed` hooks, `startRequestStream`, and the frozen §5 shapes/
`served_by` enum/`?cap=` handshake) that QA and the design-agent must not break. The prioritized scope
covers all eight requested areas — landing brand polish, endpoint-bar hierarchy + surfaced mock-URL chip,
feed column-rhythm/density/contrast, inspector header-tie + trace-as-timeline + a new mobile drawer, the
rule-modal/settings finish, designed empty/loading/error states, and audited motion + reduced-motion —
each as a concrete, testable, functionality-preserving item that only touches `templates/` + `static/`
and reuses existing tokens/local assets. Two grounded findings shape it: server-side first paint is
**dormant** because `app/routes/ui.py` passes no `initial_requests` (so the JS loading skeleton is the
real first-paint to elevate, and the wiring is flagged as a backend out-of-lane note, N1), and the split
lacks the `<1024px` inspector drawer that `ux.md`/`design.md` specify (scoped as CSS+Alpine-only via the
existing `feed.select(null)` API). No item requires a §5 change; anything that would (e.g. a templating
preview endpoint) is explicitly pushed to the architect rather than invented in the frontend lane.
