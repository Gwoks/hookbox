# UI/UX (elevated): HookBox dashboard — design-elevation pass (slug: dashboard-polish)

> **What this is.** The elevated interaction + layout design for a **working, fully-functional**
> dashboard + landing. This is **polish/elevation, not a rebuild**. It builds on
> `docs/features/beeceptor-rewrite/ux.md` + `design.md` (the prior spec — *but those cite
> `login.html`/`mock.html`/`backup.html`, which no longer exist*; the live template set is the 9
> files below) and obeys `docs/features/dashboard-polish/scope.md` (P0→P3 items a–i) and
> `docs/features/dashboard-polish/_brief.md` (LOCKED constraints).
>
> **Lane (LOCKED).** Frontend only: `templates/` + `static/`. **Never** touch `app/`. Assets stay
> vendored at `/static/vendor/` — **no `src="http…"`/`@import`/`url()`/`<img src>` to any external
> origin**, icons are inline SVG or the existing unicode glyphs (`▸ ● ○ ◆ ✕ ◇ × →`). The frozen
> FE↔BE §5 contract (`docs/features/beeceptor-rewrite/prd.md` §5) and the 6 Alpine store APIs are
> **not** changed. Captured request data is **always** rendered via `x-text` (text node), never
> `x-html`/`innerHTML`/`|safe`. Keep ARIA roles, `aria-live`, focus traps, `:focus-visible`,
> `prefers-reduced-motion`, ≥4.5:1 (text) / ≥3:1 (badges/graphical) contrast.
>
> **Testability.** Every elevation carries a numbered, QA-checkable acceptance check (**UX-##**).
> They reference the *real* selectors/components in the current templates. The **PRD gaps** list is
> last.

---

## 0. Grounding — the live surface I am elevating (verified file-by-file)

The 9 templates that exist (no others): `templates/base.html`, `templates/index.html`,
`templates/dashboard.html`, and `templates/partials/{feed_row,inspector,inspector_body_tree,rule_row,rule_modal,endpoint_settings}.html`.
Design layer: `static/css/app.css`. Client logic: `static/js/{stores,request-stream,util,rule-builder}.js`.

**Two seams every elevation must respect (verified, beyond the scope's two findings):**

1. **Live rows are rendered by the Alpine `x-for` in `dashboard.html:198-219`, *not* by JS
   cloning `feed_row.html`.** `request-stream.js` only mutates `$store.feed.rows`
   (`unshift`/`splice`); Alpine paints them. `feed_row.html` is used *only* by the dormant
   server-first-paint loop (`dashboard.html:189-195`, gated on the unwired `initial_requests`).
   **Consequence (UX-C1):** the `feed_row.html` span order/`data-cell` markup and the
   `dashboard.html` `x-for` span order **must be elevated together, identically**, so the two
   first-paint paths stay pixel-identical. The `toRow()` shape in `request-stream.js:38-53`
   (`id, method, path, status_code, served_by, matched_rule_id, duration_ms, timestamp, _new`) is
   the only data a row has — **no new field may be referenced**.
2. **The feed row is a single flat flexbox with no grid.** `.feed-row` (`app.css:133-149`) is
   `display:flex; gap:8px` with `flex-1` only on the path span; status/served/latency/time float
   right at their natural widths, so columns don't align down the list. This is the core
   scannability gap (scope c1). Elevating it is a **CSS-only** change to `.feed-row` internals plus
   matching span classes in *both* row markups — **no JS, no store, no new element**.

---

## 1. Screens & components affected — templates, reuse vs new (cite files)

All changes are **restyle + light additive markup** inside existing files. **No new template
file** is required. Net-new CSS lives in `static/css/app.css` (extends existing tokens/classes).

| Surface | File(s) | Reuse vs new | Scope item |
| --- | --- | --- | --- |
| Global shell / nav / toast | `templates/base.html`, `static/css/app.css` | **Reuse** `.card`/`.btn-*`/`.badge`/`pre`/`code`/`.toast`/`#toast`/`copyToClipboard`. Add a small inline-SVG wordmark mark; add `.alert` + `.brand-mark` CSS. | a1, g4, h3, i1 |
| Landing | `templates/index.html` | **Reuse** `landing()` JS + form verbatim; restyle card header, add value-prop strip, elevate field/button states. | a1, a2, a3 |
| Dashboard shell + endpoint bar | `templates/dashboard.html`, `app.css` | **Reuse** all `@click`/`x-ref`/bindings. Restyle the bar into 2 clusters; surface a wide-screen mock-URL chip; sticky-depth CSS. | b1–b4, b2 |
| Live feed rows | `templates/partials/feed_row.html` **and** `dashboard.html:198-219`, `app.css` | **Restyle `.feed-row` to a grid; edit *both* row markups identically.** | c1–c5 |
| Feed chrome (header, footer, "N new", chips, skeleton, empty) | `dashboard.html`, `app.css` | **Reuse** all bindings. Restyle skeleton geometry + empty-state funnel. | b2, c4, g1, g2, g5, h1 |
| Deep inspector | `templates/partials/inspector.html`, `inspector_body_tree.html`, `app.css` | **Reuse** `inspectorPanel()`/`bodyTree()`/all gates. Restyle header tie, tabs, KV, tree, trace; add local Body controls (no captured-data `innerHTML`). Add mobile drawer (CSS+Alpine). | d1–d5, g3 |
| Rule builder modal + rule rows | `templates/partials/rule_modal.html`, `rule_row.html`, `app.css` | **Reuse** `ruleModal()`/`validation`/`HookBox.RuleBuilder`/focus trap. Restyle rail/panels/footer/tag-chips/webhook/rows. | e1–e5 |
| Settings overlay | `templates/partials/endpoint_settings.html`, `app.css` | **Reuse** `endpointSettings()`/`snapshot()`/`saveSettings()`/trap. Add section headers, range styling, field-error parity. | f1–f3 |
| Motion + a11y (cross-cutting) | `app.css`, all templates | Tune keyframes; extend reduced-motion to new motion; verify focus/ARIA preserved. | h1–h3, i1–i2 |

---

## 2. Layout & placement — where it lives in `base.html`'s structure

`base.html` provides: the slim/standard `{% block nav %}` (dashboard sets `nav_pad=py-2`), the
`{% block fullbleed %}` escape hatch (dashboard uses it; landing uses the centered
`{% block content %}` inside `<main class="max-w-6xl mx-auto px-6 py-8">`), and the global
`#toast`. **None of these blocks move.** The elevation re-flows *within* them.

### 2.1 Landing `/` (centered `{% block content %}`, unchanged column)
Stays the `max-w-md mx-auto mt-12` column with the `.card p-8`. Elevation reorders *inside* the
card and adds **one** presentational strip **below** it (still inside the centered column so
centering is preserved):

```
<main max-w-6xl>                         (base.html, unchanged)
└─ div.max-w-md mx-auto mt-12            (index.html:18, unchanged)
   ├─ .card p-8
   │   ├─ brand lockup  ── inline-SVG mark + "HookBox" wordmark (--text) + 1px --link accent rule
   │   ├─ subtitle (tightened, --text-muted text-sm)
   │   ├─ [storage-unavailable banner]   (a3 → .alert--warn, unchanged copy/logic)
   │   ├─ [error banner]                 (a3 → .alert--error, unchanged copy/logic)
   │   ├─ <form> email + submit          (unchanged JS; elevated focus ring + disabled state)
   │   └─ recovery-key helper (--text-muted text-xs)
   └─ value-prop strip  ── 3 glyph bullets: Mock · Intercept · Inspect (--text-muted, non-interactive)
```

### 2.2 Dashboard `/d/<token>` (FULL-BLEED, fixed-height app shell)
The shell geometry is correct and **kept**: outer `flex flex-col` at `height: calc(100vh - 49px)`
(`dashboard.html:34`), sticky endpoint bar (`z-10`), then the split. **Split proportions stay
exactly** `feed = w-full md:w-[40%] md:min-w-[360px]`, `inspector = flex-1`
(`dashboard.html:102, 231`) — the 40/60 ratio is the right developer-console balance and changing
it is out of scope. The elevation is **internal rhythm + the missing `<1024px` drawer**, not new
proportions.

```
nav (slim, base.html py-2)
└─ <div fullbleed flex flex-col  h:calc(100vh-49px)>      (dashboard.html:30-34)
   ├─ ENDPOINT BAR  .panel-flush sticky z-10               (dashboard.html:56)  → b1 reflow into 2 clusters
   │     left  cluster: [Endpoint ▾ switcher] [+ New endpoint]  · (wide) mock-URL chip   (b2/b4)
   │     ───── hairline --border divider ─────
   │     right cluster: [Auto-CRUD ⏻] [Rules] [+ New Rule = the one green] [Settings]     (b1)
   └─ SPLIT  flex flex-col md:flex-row flex-1 min-h-0       (dashboard.html:99)
      ├─ LEFT  feed  <section w-full md:w-[40%] md:min-w-[360px] border-r>   (dashboard.html:102)
      │   ├─ feed header .panel-flush sticky  (name · WS pill · Pause)        (dashboard.html:105)
      │   ├─ "N new" pill (paused only)                                       (dashboard.html:125)
      │   ├─ mock-URL chips block (narrow-screen home of the chip)            (dashboard.html:133)
      │   ├─ #feed listbox  flex-1 overflow-y-auto  (skeleton/empty/rows)     (dashboard.html:154)
      │   └─ footer .panel-flush  "Showing n of last 100"                     (dashboard.html:223)
      └─ RIGHT inspector  <section flex-1>  #inspector-root                   (dashboard.html:231)
          ├─ empty "Select a request…"  OR
          └─ #inspector-mount → partials/inspector.html  (header tie · tabs · panels)
   + overlays (siblings, unchanged mount points): endpoint_settings.html, rules manager div, rule_modal.html
```

**Mobile (`<1024px`) layout — the one structural addition (d4).** Below `md`, the split already
stacks (`flex-col`). Today the inspector `<section>` simply renders *under* the feed with no way
back and no slide. Elevation: the inspector `<section id="inspector-root">` becomes a
**full-screen drawer** driven purely by `$store.feed.selectedId`:
- `selectedId === null` → drawer hidden (translated off-canvas), feed full-width.
- `selectedId !== null` → drawer slides over the feed (`z-40`, `--shadow-overlay`,
  `transform: translateX` `--dur-slow`), pinning a **back bar** ("← Feed") at top.
- The back control calls the **existing** `$store.feed.select(null)` (no new API) and returns to
  the feed. At `md+` the drawer styles are inert and the side-by-side split is unchanged.
This is **CSS + Alpine `:class`/`x-show` only**, keyed off existing state. No store/endpoint change.

**UX-L1.** At `≥768px` (`md`) the split is side-by-side with feed at 40% (min 360px) and inspector
filling the rest; the endpoint bar and both panel headers are sticky and remain visible while the
`#feed` list scrolls under them.
**UX-L2.** At `<1024px`, selecting a feed row slides the inspector in as a full-screen drawer with
a "← Feed" control; activating it returns to the feed (`feed.select(null)`), and the desktop
side-by-side layout is unchanged at `md+`. Under `prefers-reduced-motion` the drawer appears with
no slide.

---

## 3. Interaction & states

### 3.1 Reactivity model (preserved — the regression surface)
Three owners, unchanged: **Alpine stores** (selection, tabs, modal/overlay open, form snapshots,
optimistic toggles), **`fetch` in the stores** (detail/rules/endpoint round-trips), and
**`request-stream.js`** (the WS feed pipe). The elevation touches **markup + CSS only**. Every
`@click`, `x-model`, `x-show`, `x-if`, `:class`, `x-ref`, `role`, `aria-*`, `data-*`, element
`id`, and the `dashboard()`/`inspectorPanel()`/`bodyTree()`/`ruleModal()`/`endpointSettings()`
function bodies stay as-is unless an item explicitly adds a *local* helper that touches no store.

### 3.2 Endpoint bar — hierarchy, grouping, the single primary (b1, b2, b4)
- Reflow the one flat `flex justify-between` row (`dashboard.html:56`) into the two clusters in §2.2
  with a hairline `--border` left-padding divider before the right cluster. **`+ New Rule` is the
  only `.btn-primary` (green) control in the bar**; `+ New endpoint`, `Rules`, `Settings` are
  `.btn-secondary`; Auto-CRUD is the peer toggle. All keep their exact handlers.
- **Switcher (b4):** keep the native `<select id="ep-switcher">` (a11y + zero-dep). Add a leading
  `Endpoint` micro-label (`text-[10px] uppercase tracking-wide --text-faint`, or upgrade the
  existing `sr-only` label to a visible one), elevate padding, and add `--border-strong` on focus.
  `@change="switchEndpoint(...)"` and the `<template>` option list are untouched.
- **Wide-screen mock-URL chip (b2):** surface **one** primary mock-URL `.mock-chip` (copy-only,
  `--text-2`, never `--link`) in the left cluster, shown only at `lg+` (`hidden lg:inline-flex`).
  It reuses `mockUrl` + `copyToClipboard(mockUrl)` and carries the same `aria-label`. The
  feed-column chips (`dashboard.html:133-151`) stay for narrow screens (the wide chip and the
  narrow chips never both show: narrow chips `lg:hidden` OR simply remain — they don't conflict
  functionally; spec keeps both so no data path changes).

**UX-B1.** In the endpoint bar the only filled-green button is `+ New Rule`; the switcher,
`+ New endpoint`, `Rules`, `Settings`, and the Auto-CRUD toggle all still invoke their original
handlers (verified by `@click`/`@change` unchanged).
**UX-B2.** At `lg+` a copy-only mock-URL chip appears in the bar rendered in `--text-2` (not
`--link`), has an `aria-label`, and copying it shows the "Copied!" toast; no §5 field added.
**UX-B4.** The `<select id="ep-switcher">` shows a visible/labelled "Endpoint" affordance and a
focus ring, and changing it still routes to `/d/<token>`.

### 3.3 Live feed — density, scannability, the column grid (c1–c5)
**c1 (P0) — the grid.** Re-spec `.feed-row` from free-flex to a **fixed-template grid** so every
column aligns down the list:

```
.feed-row { display:grid;
  grid-template-columns: 52px minmax(0,1fr) auto auto auto;
  /*               method | path(truncate) | status | served | latency·time   */
  align-items:center; column-gap:10px; }
```
- **method badge** = fixed 52px gutter (left), so paths start at one x-position.
- **path** = `minmax(0,1fr)` with `truncate` (`title=path` for the full value).
- **status** = right-aligned, **tabular-nums**, colored text only (`.status-*`).
- **served chip** = optional outline `.served-chip`, never a fill.
- **latency + time** = a right metadata cluster; latency `--text-muted`, time `--text-faint`,
  both `text-xs` + `tabular-nums`. On `<1024px` (drawer mode where the feed is full width) the
  served chip and latency may wrap/hide first (reflow rule), but method+path+status always stay
  (the AC-28 essentials).
- The span order in **both** `feed_row.html:50-57` and the `x-for` `dashboard.html:209-216` is kept
  1:1; each span keeps its `data-cell` (server path) / position so the two render identically.

**c2 (P1) — density + selected/hover finish.** Keep `padding:8px 12px`; tighten line-height to the
`text-sm`/`text-xs` register. The selected state stays `.is-selected` → `--surface-2` fill +
method-colored left rail (`.rail-*.is-selected`, `app.css:157-164`) + brightened `--text`. Hover =
`--surface-2`. Exactly **one filled badge per row** (the method); the served chip is outline so it
never competes.

**c3 (P1) — color/contrast audit (token-tune only, no new hues).** Verify on the real render that
every `.m-*`/`.status-*` pair meets the §8.1 bars: GET/POST/PUT/PATCH/DELETE filled badges ≥3:1;
`OPTIONS` is the known-borderline `.m-options` (`#30363d`/`#8b949e` ≈3.2:1) — if QA measures <3:1,
apply the **VC-2b** fallback by bumping `.m-options` text to `--text-2` `#c9d1d9` (already noted in
the class comment). 4xx `--client-err` amber must read visibly distinct from 5xx `--danger` red.
Literal method/status text is always present (color is never the only signal).

**c4 (P2) — time + footer.** `relTime()` output stays; the timestamp uses `--text-faint` as a
*secondary* cue only (the row already carries method/path/status). The footer keeps
`$store.feed.footerText` ("Showing n of last 100") as a calm `--text-muted` caption.

**c5 (P3, defer-if-risky) — optional method filter chips.** If added, they filter a **local
computed view** in the `dashboard()` scope (e.g. `visibleRows` derived from `$store.feed.rows`
filtered by a local `activeMethods` set) used by the `x-for` — **without** mutating
`$store.feed.rows` (owned by `request-stream.js`) and without touching cap/dedupe. Chips are
toggle `aria-pressed` buttons; "All" clears. *Defer if it risks the store contract.*

**UX-C1.** Server-rendered rows (`feed_row.html`) and live `x-for` rows are visually identical
(same column grid, same badge/status/chip/latency/time order); `data-request-id` and `data-cell`
attributes are preserved on the server path.
**UX-C2.** With a feed of mixed methods/statuses, method badges form a fixed left gutter and
status/latency/time align into right-hand columns (tabular). The selected row shows the
method-colored left rail + `--surface-2` fill; hover shows `--surface-2`; each row has exactly one
filled badge.
**UX-C3.** Every method badge and status color meets ≥3:1 on its surface (OPTIONS uses the
`#c9d1d9` fallback if it measures <3:1); 4xx amber and 5xx red are visibly different hues; the
literal method and status text always render.

### 3.4 Deep inspector — header tie, tabs, KV/tree, trace, Body controls, drawer (d1–d5, g3)
- **d1 header tie.** The sticky header (`inspector.html:54-62`) repeats the **selected row's
  method badge color + mono path + status + served chip** — the "one subject, one color" tie to
  the feed's selected rail. Keep `$store.inspector.methodBadge/statusClass/servedClass/servedLabel`
  bindings and `d().method/path/status_code/duration_ms`. Elevate the tab strip
  (`role=tablist`, `.tab-btn`): clearer active underline (`.tab-btn.is-active` = 2px `--link`
  underline + `--link` text) and hover (`--text-2`); **keep** arrow-key nav
  (`cycleTab`/`selectTabIndex`), `aria-selected`, `:aria-controls`, roving `:tabindex`.
- **d2 KV + tree readability.** Elevate `.kv-row` alignment (keep `kv-key min-width:9rem`, tune
  copy-button affordance) and the JSON tree (`inspector_body_tree.html`): tighten line-height,
  finish the disclosure triangle (`▸` rotate via `.tree-toggle` transform), keep syntax tints
  (`.v-string/.v-number/.v-bool/.v-null`). Collapse/expand stays driven by `bodyTree()`
  `visibleNodes`; every key/value stays `x-text` (`JSON.stringify` for strings) — XSS-safe.
- **d3 trace-as-timeline.** Elevate `.trace-step` into a vertical timeline: weight the glyph
  colors (matched `●` `--success`, skipped `○` `--text-faint`, state-write `◆` `--info`, chaos `✕`
  `--danger`, cors/other `◇`/`▸` muted), connect steps with a faint left rail, and give the
  before→after diff (`.state-before` faint → `.state-arrow` → `.state-after` success) a distinct
  treatment. **Keep the `decorateTraceStep` output contract** (`step/detail/cls/glyph/diff`,
  `stores.js:614-632`) — do not change the regex or the produced classes/glyphs.
- **d4 mobile drawer.** As §2.2 / UX-L2 — CSS+Alpine only off `$store.feed.selectedId`, back via
  `feed.select(null)`.
- **d5 Body tab controls.** Add **local** controls to the Body panels (request + response):
  **Expand all / Collapse all** (toggle every node by setting `bodyTree()`'s `openState` for all
  `nodesRaw()` ids — a local method on the `bodyTree()` component, no store change), **Copy**
  (`copyToClipboard($store.inspector.reqBodyRaw / resBodyRaw)` — already exist), **Raw / Pretty**
  (toggle between the existing tree and the `<pre><code x-text="…BodyRaw">` fallback via a local
  `raw` boolean). **No captured data via `innerHTML`** — Copy uses the already-stringified raw
  getter; Raw uses `x-text`.
- **g3 empty/pending/error finish.** Elevate the five gates
  (`isLoading/isPending/isUnauthorized/isError` + the `selectedId===null` empty in
  `dashboard.html:232`) into one **centered treatment**: a muted glyph + calm copy, consistent
  vertical centering. Keep each gate's existing `x-if` and the Retry buttons calling
  `$store.inspector.retry()`. Copy stays: "Select a request on the left to inspect it.",
  "Loading detail…", "Detail still being written…", "Not authorized to view this request.".

**UX-D1.** Selecting a row renders an inspector header whose method badge color matches the
selected feed row's rail; all 5 tabs switch on click and via Left/Right/Home/End arrow keys with
`aria-selected` tracking the active tab.
**UX-D2.** Headers/Query/Body render aligned; tree nodes collapse/expand via the disclosure
triangle; every captured key/value is a text node (no `x-html`/`innerHTML` on captured data).
**UX-D3.** A trace renders as a vertical timeline: matched=`●` green, skipped=`○` faint,
state-write=`◆` info, chaos=`✕` danger; before→after diffs show faint→success; the
`decorateTraceStep` glyph/class contract is unchanged.
**UX-D5.** On the Body tab, Expand-all expands every node, Collapse-all collapses every node, Copy
copies the pretty body (toast "Copied!"), and Raw/Pretty toggles the `<pre>` fallback vs the tree;
no captured data is inserted via `innerHTML`.

### 3.5 Rule builder modal + rule rows (e1–e5)
- **e1 shell + rail.** Elevate the vertical rail active state (keep `bg-[#21262d] text-[#58a6ff]
  border-[#58a6ff]`), tokenize the per-tab **error dot** (`bg-[#f85149]` → `--danger`), tighten
  panel spacing, and finish the footer validation summary ("N field(s) need attention" + disabled
  Save). **Keep** the whole `ruleModal()` form, the `validation` computed
  (`HookBox.RuleBuilder.validate`), tab `role`s/`aria-*`, `trapTab` focus trap, and `save()` jump-
  to-first-bad-tab behavior.
- **e2 field grouping.** Tighten labels/helper text (`--text-muted`) and the add/remove KV rows in
  Matching/Response/Actions/Throttling; reuse the shared input class verbatim. All `x-model`
  bindings and element ids unchanged; add/remove buttons still `push`/`splice` the form arrays.
- **e3 tag palette as chips.** Restyle the Templating tag buttons into a tidy mono chip palette
  (still `@click="appendTag(tag)"` into the shared `form.bodyTemplate`); the shared body textarea
  updates from both Response and Templating tabs (same `x-model`).
- **e4 webhook "deferred" clarity.** Elevate the disabled Webhook `fieldset` (`opacity-60` +
  `disabled`) so it reads as intentionally-off (e.g. a "Coming soon" pill + `--warn` note); fields
  stay `disabled`; `webhook_action` still serializes when data is present
  (`rule-builder.js:274`).
- **e5 rule-row finish.** Elevate `rule_row.html` spacing, the enable/disable toggle affordance,
  and the disabled `opacity-60` state. Keep optimistic `$store.rules.toggle(token, rule)`,
  `editRule(rule)`, `confirmDelete(rule)`.

**UX-E1.** An invalid field marks its tab's rail dot `--danger`, disables Save, and submitting
jumps to the first bad tab; a valid rule still POSTs (create) / PATCHes (edit) via `$store.rules`
and the modal closes on success.
**UX-E5.** Toggling a rule flips optimistically and reverts + toasts on failure; Edit opens the
modal pre-filled (`editingRule`); Delete confirms then `remove()`s; a disabled rule renders at
`opacity-60`.

### 3.6 Settings overlay (f1–f3)
- **f1 sectioning.** Group the single scroll (`endpoint_settings.html:41-171`) under clear headers:
  **Identity** (name), **Behavior** (Auto-CRUD, Auto-CORS, MITM target, default mode),
  **Simulated network conditions** (latency, rate limit, chaos — already under an `<h3>`), and the
  existing **Danger zone** (keep its `--danger`-bordered box). Consistent label/helper rhythm.
  Keep `endpointSettings()`, `snapshot()`, `saveSettings()`, all `x-model`s, and `trapTab`.
- **f2 range/dial readability.** Style the native `range` tracks/thumbs in CSS (no JS) and make the
  `0–10000` / `0–100` min/max captions obvious; the live mono readout still mirrors
  `form.latency_ms` / `form.chaos_pct`; clamp behavior (`clampNum`) unchanged.
- **f3 toggle + MITM finish.** Elevate Auto-CRUD/Auto-CORS toggle spacing and bring the MITM
  `target_url` inline error to the rule-modal field-error style (red border + `--danger` helper)
  via the existing `targetError` computed. Invalid scheme still disables Save; empty clears MITM.

**UX-F1.** Every settings control keeps its `x-model`; Save PATCHes a clamped `EndpointConfigPatch`;
Clear state / Clear history still confirm then call the store. Sections render under labelled
headers and the Danger zone is visually separated.
**UX-F3.** An invalid MITM scheme shows the inline `--danger` error and disables Save (mirrors
AC-S6); clearing the field clears the error; toggles persist on Save.

### 3.7 The full state matrix (empty / loading / error / degraded / success)
Every async region keeps its existing gate and is elevated visually. Matrix (state → where →
trigger/binding → elevated treatment → copy unchanged unless noted):

| Region | State | Gate / trigger (preserved) | Elevated treatment |
| --- | --- | --- | --- |
| **Feed** | Loading | `loading` true; `x-if` skeleton block `dashboard.html:164` | **g1:** 5 `.skeleton` rows shaped like the real grid (52px badge block + path bar + right cluster), shimmer; static under reduced-motion. |
| Feed | Empty | `!loading && feed.rows.length===0` (`dashboard.html:173`) | **g2:** friendly hierarchy + prominent copy-only mock-URL chip + a copy-only sample `curl` `code` block (static text, never executed; no `http` asset). |
| Feed | Has rows | `feed.rows.length>0` (`dashboard.html:198`) | The elevated grid rows (c1/c2). |
| Feed | New arrival | `_new` flag → `.feed-row--new` | **h1:** single arrival flash `hb-flash` ≤~900ms (batched); no reflow; class removed by `request-stream.js` so re-selection doesn't re-flash. |
| Feed | Paused + buffered | `feed.paused && feed.pending>0` (`dashboard.html:125`) | "N new" pill (`--success` fill, `--bg` text) → `feed.flush()`. `#feed` `:aria-live` flips to `off` when paused. |
| **WS pill** | live / connecting / reconnecting / degraded / offline / unauthorized | `$store.stream.label` + `dotClass` (`request-stream.js:377-398`) | **g5:** each state visually distinct, **always text-labelled** (never color-only): "Live" (pulse), "Connecting…", "Reconnecting…(n)" (throb), "Realtime degraded", "Offline" (static), "Unauthorized" (static). `role=status aria-live=polite`. |
| **Inspector** | nothing selected | `feed.selectedId===null` (`dashboard.html:232`) | **g3:** centered muted glyph + "Select a request on the left to inspect it." |
| Inspector | Loading | `inspector.isLoading` | centered spinner glyph + "Loading detail…", `aria-busy`. |
| Inspector | Pending (AC-31a) | `inspector.isPending` | centered + "Detail still being written…" + sub-note + **Retry now** (`retry()`). |
| Inspector | Unauthorized | `inspector.isUnauthorized` | centered `--danger` "Not authorized to view this request." `role=alert`. |
| Inspector | Error | `inspector.isError` | centered `--danger` message + **Retry** (`retry()`). `role=alert`. |
| Inspector | Ready | `inspector.isReady` | header tie + tabs + panels (d1–d3, d5). |
| **Rules overlay** | Loading / Error / Empty / List | `rules.loading` / `rules.error` / empty / `x-for` (`dashboard.html:267-285`) | Loading caption; **g4** `.alert--error`; empty funnel "No rules yet…" + `+ New Rule`; list = elevated `rule_row.html`. |
| **Settings** | Loading / Error / SaveError / Ready | `endpoint.loading` / `.error` / `.saveError` / `.detail` | Loading caption; **g4** unified `.alert--error` banners; Ready = sectioned form (f1). |
| **Rule modal** | ServerError / Validation / Saving | `serverError` / `validation` / `saving` | **g4** `.alert--error` banner; rail dots + footer count; Save `is-disabled` + "Saving…". |
| **Landing** | storage-unavailable / error / submitting / field error | `!storageAvailable` / `errorBanner` / `submitting` / `fieldError` | **a3/g4:** `.alert--warn` / `.alert--error`; submit `.is-disabled` + "Setting up…"; constant `fieldError` copy (AC-S5 unchanged). |

**Degraded specifically (UX-G5).** Forcing `$store.stream.state` to each of the six values shows
the matching label + dot class from the store getters; "degraded" reads "Realtime degraded" with
`ws-dot--degraded` (amber, static); reduced-motion turns live/reconnect dots static while the label
remains. No state is color-only.

**g4 alert unification.** Standardize the mix of raw `bg-red-900/50 border border-red-500
text-red-200` (used in `index.html`, `rule_modal.html`, `endpoint_settings.html`, the rules
overlay) onto one `.alert` helper set in `app.css` (`--danger`/`--warn`/`--info`/`--success`
variants using the `*-bg` tokens + border). Apply consistently; **copy is unchanged** (especially
the AC-S5 email-exists-neutral landing copy).

**UX-G1.** On load the feed shows skeleton rows whose geometry matches the real row grid; they
shimmer, are static under `prefers-reduced-motion`, and are replaced when `feed.rows` populates.
**UX-G2.** When `!loading && rows.length===0` the empty feed shows the copyable mock-URL chip and a
copy-only sample request hint; copy works; no external asset is loaded.
**UX-G3.** Each inspector state (loading/pending/unauthorized/error/empty) renders under its
existing gate with the centered elevated treatment, and Retry still calls
`$store.inspector.retry()`.
**UX-G4.** Save-error, load-error, modal server-error, and landing banners share one `.alert`
look; the AC-S5 landing field copy is unchanged.

### 3.8 Motion (h1–h3)
- **h1.** Keep the signature motions (`hb-pulse` live dot, `hb-throb` reconnect, `hb-flash`+
  `hb-slide` arrival, `hb-shimmer` skeleton). Audit timing so liveness reads calm; the arrival
  flash is the **only** per-row animation under burst (no reflow). The `_new`→`.feed-row--new`
  add/remove is owned by `request-stream.js` (re-selection doesn't re-flash).
- **h2.** Extend `@media (prefers-reduced-motion: reduce)` (`app.css:338`) to cover **all new
  motion**: the mobile drawer slide (d4), any sticky-shadow transition (b3), and modal/panel/tag-
  palette transitions — so nothing animates under the preference and every state stays legible by
  color + text + glyph.
- **h3.** Buttons/toggles/tabs/tree-triangle/copy-button transitions feel consistent at
  `--dur-fast`; the copy-success micro-confirmation pairs with the existing "Copied!" toast; no
  layout-thrashing properties are animated.

**UX-H1.** The live dot pulses on a ~2s `--success` cycle; reconnecting throbs amber; offline is a
static red dot; a new row flashes `--success-bg` decaying ≤~900ms and re-selecting it does not
re-flash.
**UX-H2.** With `prefers-reduced-motion: reduce`, the drawer, feed rows, WS pill, skeleton, and
modals show no animation, and every state remains legible by color + text + glyph.

### 3.9 Keyboard + focus (i1)
Preserved end-to-end and verified after restyle: feed `#feed` listbox Up/Down move selection +
`scrollIntoView`/focus (`moveSelection`); inspector tab strip Left/Right/Home/End; both modals trap
Tab (`trapTab`) and `Esc` closes + restores focus to the trigger (`rulesBtn`/`settingsBtn`/
`x-ref="dialog"`); the rule modal `Esc` doesn't close while it's the topmost (the rules-overlay
`Esc` guard `if (rulesOpen && !ruleModalOpen)` stays). **Every** added/restyled interactive element
keeps a visible `:focus-visible` ring (global in `app.css:303`). Any new control (Body-tab buttons,
filter chips, drawer back button, wide mock-URL chip) is a real `<button>` — never an interactive
`<div>`.

**UX-I1.** Keyboard-tabbing through landing → endpoint bar → feed → inspector tabs → rules modal →
settings modal shows a visible focus ring on every control; `Esc` closes each overlay and restores
focus to its trigger.

---

## 4. Copy

**Landing (`index.html`) — unchanged logic strings; only the lockup/strip are new:**
- Wordmark: **"HookBox"**. Subtitle (tighten to one line): keep
  "Self-hosted API mocking, interception & real-time debugging." (drop the second sentence into the
  existing recovery-key helper if it crowds).
- Value-prop strip (a2, new, presentational): **"Mock"**, **"Intercept"**, **"Inspect"** with a
  one-clause gloss each (e.g. "Mock — instant REST responses", "Intercept — proxy & capture real
  APIs", "Inspect — live request feed + deep trace"). `--text-muted`, non-interactive.
- **Unchanged (do not edit):** field error "Enter a valid email address." / "That email looks
  invalid. Please check and try again." (AC-S5 constant), 429 "Too many attempts…", network
  "Network error — check your connection and try again.", submit "Get my endpoint" / "Setting up…".

**Endpoint bar:** switcher micro-label **"Endpoint"**; buttons keep **"+ New endpoint"** /
**"Rules"** / **"+ New Rule"** / **"Settings"**; Auto-CRUD label **"Auto-CRUD"**; mock-URL chip
**Copy** with `aria-label="Copy mock URL <url>"`.

**Feed:** WS pill **Live / Connecting… / Reconnecting…(n) / Realtime degraded / Offline /
Unauthorized** (from the store — do not hardcode). Pause toggle **Pause / Paused**; buffered pill
**"N new"**; footer **"Showing n of last 100"**. Empty (g2, elevate hierarchy, keep meaning):
**"No requests yet."** / **"Send a request to your mock URL and it will appear here live."** + a
sample like `curl https://<your-mock-url>/ping` as copy-only text.

**Inspector:** tabs **Headers · Query Params · Body · Response Served · State & Tracing**
(unchanged); Body controls (d5) **Expand all · Collapse all · Copy · Raw / Pretty**; states keep
their existing copy (§3.7). Served chips keep the `util.js` labels (**Matched rule / Auto-CRUD /
Proxied / Tunneled / Default / CORS / Chaos / Rate-limited**).

**Rule modal:** title **Create rule / Edit rule**; tabs **Matching · Response · Templating ·
Actions · Throttling**; footer **"N field(s) need attention" / "Ready to save"**; buttons
**Cancel / Create rule / Save rule / Saving…**; webhook (e4) **"Coming soon"** pill + keep the
"…disabled; no outbound request is made in v1." note.

**Settings:** section headers **Identity · Behavior · Simulated network conditions · Danger zone**;
all field labels/helpers and confirm dialogs unchanged.

---

## 5. Accessibility

- **Semantics preserved:** `<nav>`/`<main>`/`<form>`/`<button>`; `#feed` `role=listbox` with
  `role=option` rows (`:aria-selected`), inspector `role=tablist`/`tab`/`tabpanel`, modals
  `role=dialog aria-modal=true` with `aria-labelledby`, WS pill + `#toast` `role=status
  aria-live=polite`. No restyle may replace a `<button>` with a clickable `<div>`.
- **Live region:** `#feed` keeps `:aria-live="paused ? 'off' : 'polite'"` so pausing also pauses
  announcements; the arrival cue is never color-only (motion + the row's own method/path/status
  text + the "N new" counter).
- **Non-color signaling:** method/status carry literal text; the WS pill carries a text label; the
  trace uses distinct glyphs (`●○◆✕◇▸`) in addition to color.
- **Focus:** global `:focus-visible` ring on all controls; modals trap focus and `Esc` restores to
  the trigger; the new drawer back button and Body-tab controls are keyboard-reachable.
- **Contrast:** body/secondary text ≥4.5:1 (`--text`/`--text-2`/`--text-muted`); `--text-faint`
  only for non-load-bearing decoration/timestamps (VC-2a); badges/status/dots ≥3:1 (VC-2, with the
  OPTIONS VC-2b fallback).
- **Copy buttons** keep `aria-label`s (mock URL, per-header, per-query). Range inputs keep
  `aria-describedby`; toggles keep `aria-label`/associated `<label>`.

**UX-I2.** After the restyle, every preserved role/label is still present
(`listbox/option`, `tablist/tab/tabpanel`, `dialog/aria-modal`, `status/aria-live`), and `#feed`'s
`aria-live` still flips to `off` when paused.

---

## 6. Consistency notes — patterns/classes reused from existing templates

- **Visual system reused verbatim:** `.card`, `.btn-primary/.btn-secondary/.btn-danger`, `.badge`
  + the `.m-*` method map, `.status-*`, `.served-chip`, `.panel-flush`, `.feed-row` (+ `.rail-*`),
  `.ws-pill`/`.ws-dot*`, `.tab-btn`, `.kv-row`, the `.tree-*`/`.v-*` syntax tints, `.trace-*`,
  `.mock-chip`, `.skeleton`, the `:disabled/.is-disabled` + `:focus-visible` rules, and all
  keyframes — all in `static/css/app.css` / `base.html`. `#toast` + `showToast()` +
  `copyToClipboard()` reused for every copy/save confirmation.
- **Tokens only:** any new color must be an existing `:root` token (`--danger`, `--warn`,
  `--success-bg`, etc.) — no one-off hex. The `.alert` helper (g4), the feed grid (c1), the trace
  timeline rail (d3), the range styling (f2), the drawer (d4), and the brand mark (a1) are the only
  net-new CSS and all consume existing tokens.
- **Modal geometry reused:** the rules overlay, rule modal, and settings overlay all keep the
  `fixed inset-0 bg-black/50 z-50` backdrop + `.card max-w-* max-h-[85vh]` +
  `box-shadow: var(--shadow-overlay)` pattern.
- **Deliberate divergence from the prior spec docs:** `beeceptor-rewrite/ux.md` + `design.md` cite
  `login.html` / `mock.html` / `backup.html` and the `hookbox_user` key — **those are stale** (the
  live key is `hookbox_owner` and those templates were removed). This doc cites only the 9 live
  templates and the real `hookbox_owner` flow.

---

## 7. PRD gaps — UI requirements / ACs the PM must add or clarify

> Numbered, UI-facing. None require touching `app/` from this lane; the FE consequence is noted.

1. **Wire `initial_requests` for true server-side first paint (AC-27a).** `app/routes/ui.py`
   `dashboard()` passes only `token`, so the `feed_row.html` server loop + `#hb-initial-feed`
   island (both already present and kept intact) never activate — first paint is the JS skeleton.
   The PM/architect should decide whether to wire the recent `RequestSummary[]` (≤100) into the
   template context (backend lane). Until then, the elevated **skeleton** (g1/UX-G1) is the real
   first-paint and the dormant server path stays untouched.
2. **Confirm the wide-screen vs narrow mock-URL chip behavior (b2).** Should the bar's `lg+`
   mock-URL chip and the feed-column chips be mutually exclusive (hide the feed chips at `lg+`), or
   both visible? This is purely presentational but affects redundancy/clutter — please pin the
   intended behavior so QA has a target.
3. **Sample-request hint in the empty feed (g2).** The elevated empty state proposes a copy-only
   `curl` line built from the mock URL. Confirm (a) the exact canonical command/text and (b) that
   it is copy-only and never executed — so the activation hint is consistent and safe.
4. **Method filter chips — in or out for this pass (c5)?** The prior `ux.md §3.2` mentions them;
   they're currently absent. If in, confirm they are a **view-only local filter** (no
   `$store.feed.rows` mutation, no cap/dedupe change). If a server-side facet is ever wanted, that
   needs a §5 change and goes to the architect. Default recommendation: **defer** (P3) to protect
   the store contract.
5. **Body-tab "Raw vs Pretty" default + large-body behavior (d5).** Confirm the default view
   (Pretty/tree) and whether Expand-all should be bounded for very large trees (the tree already
   caps at 5000 nodes in `util.js`). Needed so Expand-all has predictable, testable behavior.
6. **Densit y / minimum supported width (carried from design.md gap 7).** The feed grid
   (`52px | 1fr | status | served | latency·time`) and the `<1024px` drawer assume a target
   desktop width and a narrow breakpoint. Please state the supported minimum width and target so QA
   verifies the grid + drawer at the intended size rather than guessing.
7. **Webhook "deferred" visual is still gated on AC-33a scope.** e4 elevates the disabled webhook
   section to read intentionally-off; the PM must still confirm webhook is out for this milestone
   (so the section stays disabled-but-serialized) — otherwise the fields need to become live, which
   is a contract/behavior change beyond this polish lane.
8. **"Realtime degraded" state — when does it fire?** The store exposes a `degraded` state
   (`stream.state`) and this doc gives it a label/dot (g5/UX-G5), but nothing in `request-stream.js`
   currently sets `degraded`. Confirm whether the BE/stream should ever enter it (e.g. SSE
   fallback) so the visual isn't dead UI; if never, the PM may drop it from the labelled set.
