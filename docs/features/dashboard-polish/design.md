# Visual design: HookBox dashboard — design-elevation pass (slug: dashboard-polish)

> **What this is.** The *aesthetic* layer on top of `docs/features/dashboard-polish/ux.md` (the
> functional contract) for a **working, fully-functional** dashboard + landing. **Polish/elevation,
> not a rebuild.** `ux.md` owns IA, components, copy, states, a11y; this doc gives every one of those
> a refined visual treatment — token system, the method/status/served-by color system, component
> finish, hierarchy, and motion. I do **not** re-open `ux.md`'s structure, copy, or a11y decisions;
> anything a visual choice touches is flagged in **§ UX handoff notes**.
>
> **Lane (LOCKED — `_brief.md`).** Frontend only: `templates/` + `static/`. **Never** touch `app/`.
> Assets stay vendored at `/static/vendor/` — **no `src="http…"` / `@import url(http…)` / external
> `<img>` / web-font fetch of any kind**. Icons are inline SVG or the existing unicode glyphs
> (`▸ ● ○ ◆ ✕ ◇ × → ⏻`). The frozen §5 FE↔BE contract (`docs/features/beeceptor-rewrite/prd.md §5`)
> and the 6 Alpine store APIs are **unchanged**; captured request data is **always** `x-text`, never
> `x-html`/`innerHTML`/`|safe`. Keep WCAG-AA: ≥4.5:1 text, ≥3:1 badges/graphical; `:focus-visible`;
> `prefers-reduced-motion`; ARIA + `aria-live` preserved.
>
> **Grounding.** Every token/class is tagged **[existing — verified at `path:line`]** or
> **[new — proposed]**. The base is the GitHub-dark system in `base.html` `<style>` (`base.html:25-44`)
> + the formalized `:root` token block + component layer in `static/css/app.css`. I extend it; I do
> **not** fork a parallel system. All net-new CSS lands in `static/css/app.css` and consumes existing
> `:root` tokens — **no one-off hex**.
>
> **Testability.** Every elevation carries a numbered, QA-checkable **Visual Criterion (VC-##)**.
> VC-IDs here continue the prior `beeceptor-rewrite/design.md` VC space (which ended at VC-17) and
> start at **VC-18** so they never collide. The **PRD gaps** list is last.

---

## Design direction

HookBox reads as a **precision developer instrument**, not a SaaS marketing surface: a quiet,
near-black GitHub-dark console where the only saturated color is **signal** — the HTTP method on a
feed row, a status code, the pulsing "Live" dot, the green "matched-rule" branch in a trace. This
pass keeps that thesis and **tightens it**: a single 4px spacing grid and one type register make the
40/60 split feel engineered rather than assembled; chrome desaturates further so hundreds of streaming
rows never feel noisy; and the method-color system becomes the one organizing spine that ties a feed
row's left rail → its badge → the inspector header → the rule row. The elevation is **rhythm, density,
hierarchy, and motion discipline** layered onto the existing `#0d1117`/`#161b22`/`#30363d` palette and
Monaco/Menlo mono — never new structure.

---

## Design tokens

All values are **CSS custom properties**. The existing `:root` block (`app.css:15-65`) is the source
of truth and is **kept verbatim**; this pass adds a small, additive set. No existing token value
changes (changing one would ripple through `feed_row.html`'s inlined Jinja maps and `util.js` — out
of lane).

### Color — surfaces & lines (all existing)
| Token | Value | Role | Source |
| --- | --- | --- | --- |
| `--bg` | `#0d1117` | page; input wells; `pre`/`code`; chip bg | [existing — `app.css:17`] |
| `--surface` | `#161b22` | card / panel / sticky-bar bg | [existing — `app.css:18`] |
| `--surface-2` | `#21262d` | hover + selected row fill, secondary button | [existing — `app.css:19`] |
| `--surface-3` | `#30363d` | toggle track, skeleton highlight | [existing — `app.css:20`] |
| `--border` | `#30363d` | default 1px hairline | [existing — `app.css:21`] |
| `--border-strong` | `#3d444d` | border on hover/focus of dense controls (switcher) | [existing — `app.css:22`] |
| `--row-divider` | `#21262d` | feed/KV/rule row separators (subtler than `--border`) | [existing — `app.css:23`] |

### Color — text (all existing)
| Token | Value | Role |
| --- | --- | --- |
| `--text` `#e6edf3` | primary copy, selected-row text, headings | [existing — `app.css:26`] |
| `--text-2` `#c9d1d9` | secondary copy, secondary-button label, mock-chip code | [existing — `app.css:27`] |
| `--text-muted` `#8b949e` | labels, helper text, latency, footer | [existing — `app.css:28`] |
| `--text-faint` `#6e7681` | **non-load-bearing only**: timestamps, tree punctuation, disabled glyphs (VC-2a) | [existing — `app.css:29`] |
| `--link` `#58a6ff` | focus ring, copy-hover, active tab, mono identifiers | [existing — `app.css:30`] |

### Color — semantic / signal (all existing)
`--success #3fb950` / `--success-strong #2ea043` / `--success-bg rgba(63,185,80,.15)` /
`--info #58a6ff` / `--info-bg rgba(88,166,255,.15)` / `--warn #d29922` / `--warn-bg rgba(210,153,34,.15)` /
`--client-err #e3a008` / `--danger #f85149` / `--danger-base #da3633` / `--danger-bg rgba(248,81,73,.15)`
— all [existing — `app.css:33-43`]. These are the **only** colors any new rule may reference.

### Typography (existing stacks; one tightened register)
| Token | Value | Source |
| --- | --- | --- |
| `--font-sans` | `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif` | [existing — `app.css:46`] |
| `--font-mono` | `"Monaco", "Menlo", "Ubuntu Mono", monospace` | [existing — `app.css:47`] |

**Scale (Tailwind classes already in markup, no new sizes):** Display `text-2xl font-bold` (24/32, landing + "not found"); Heading `text-lg font-bold` (18/28, modal titles); Body `text-sm` (14, feed path, labels, inputs); Micro `text-xs` (12, badges, metadata, helper); Nano `text-[10px]` (section eyebrows, range min/max). **Mono** (`--font-mono`) for all captured data (path, KV, tree, trace step), the mock-chip, and metric readouts; **sans** for chrome/labels/buttons. **[new — proposed] one additive utility:** `.tabular` → `font-variant-numeric: tabular-nums` so status / latency / time columns align vertically down the list (consumed by the feed grid and inspector header). This is a rendering hint only; no markup contract changes.

### Spacing, radius, elevation, z-index, motion (all existing)
- **Spacing:** Tailwind 4px base (`1=4 2=8 3=12 4=16 5=20 6=24`). Feed row stays `8px 12px`; panel bars `px-4 py-2`; modal gutters `px-5 py-3/4`. [existing — `app.css` comments + markup]
- **Radius:** `--radius-sm 4px` (badges/chips/status), `--radius 6px` (buttons/inputs/`pre`), `--radius-lg 8px` (cards/modals). [existing — `app.css:50-52`]
- **Elevation:** flat by default; only overlays lift. `--shadow-overlay 0 16px 48px rgba(1,4,9,.85)` (modals/drawer), `--shadow-pop 0 8px 24px rgba(1,4,9,.7)` (reserved). Cards stay **border-only**. [existing — `app.css:53-54`]
- **z-index:** sticky bars/headers `z-10`; mobile inspector drawer **`z-40` [new — proposed, additive class only]**; overlays `z-50`; toast `1000`. [existing — markup + `base.html:40`]
- **Motion:** `--dur-fast 120ms` (hover/tab/toggle/triangle/copy), `--dur-base 200ms` (row select, panel swap, drawer-at-md inert), `--dur-slow 300ms` (drawer slide), `--dur-flash 900ms` (arrival), `--dur-pulse 2000ms` (live/throb). Eases `--ease-out cubic-bezier(.16,1,.3,1)`, `--ease-standard ease`. [existing — `app.css:57-64`]

**Net-new tokens/utilities introduced by this pass (all additive, all consume existing values):**
`.tabular` utility; `.feed-row` grid internals (re-spec, §Component); `.alert` family (§Visual states);
`.served-chip` size tune; `.trace-rail` connector; `.range-*` track/thumb; `.insp-drawer` (mobile);
`.brand-mark` (landing). **No `:root` value is added or changed.**

---

## Component styling

Per affected component: exact visual spec, reuse-vs-new, cited file. Every class below already styled
by `app.css` is **reused**; "elevate" = tune CSS internals only.

### Buttons — reuse verbatim, one hierarchy rule (`base.html:28-33`)
`.btn-primary` (green `#238636`→hover `#2ea043`), `.btn-secondary` (`#21262d` + `#30363d` border →hover `#30363d`), `.btn-danger` (`#da3633`→`#f85149`) are **reused unchanged**. **Elevation:** in the endpoint bar exactly **one** filled-green `.btn-primary` exists — `+ New Rule` (`dashboard.html:94`); `+ New endpoint`/`Rules`/`Settings` are `.btn-secondary`. **[new — proposed]** add `transition: background-color var(--dur-fast), border-color var(--dur-fast), box-shadow var(--dur-fast)` and an `:active { transform: translateY(0.5px); }` press cue to all three (currently `0.2s` ad-hoc) so button feedback is uniform and snappy. `:focus-visible` ring already global (`app.css:303`).

### Cards & panels — reuse (`base.html:27`, `app.css:125`)
`.card` (`#161b22` + `#30363d` border + 8px radius) and `.panel-flush` (surface + bottom hairline, used for every sticky bar/header) reused. **Elevation (b3 sticky depth):** **[new — proposed]** `.panel-flush.is-scrolled { box-shadow: 0 1px 0 var(--border), 0 6px 12px -8px rgba(1,4,9,.6); }` — a *hairline + faint drop* applied only when content scrolls under a sticky bar, so the endpoint bar and both panel headers read as elevated above the moving feed without a hard line. Toggle is CSS-only via a sentinel: a 1px `position:sticky; top:-1px` probe is not needed — instead bind `:class="{ 'is-scrolled': feedScrolled }"` where `feedScrolled` is a **local** `dashboard()` boolean set on the `#feed` `@scroll` (a local UI flag, touches no store). If FE prefers zero JS, ship the static hairline only and defer the shadow (still passes VC).

### Method badge color system — reuse map verbatim (`app.css:80-87`)
The `.badge` geometry (`base.html:34`: `4px 8px`, radius 4, 12px bold) + the `.m-*` map are the spine of the product and are **reused unchanged**:

| Method | Fill | Text | Contrast vs fill |
| --- | --- | --- | --- |
| GET | `#388bfd` | `#ffffff` | ~3.4:1 ✅ |
| POST | `#3fb950` | `#0d1117` | ~8.9:1 ✅ |
| PUT | `#d29922` | `#0d1117` | ~9.1:1 ✅ |
| PATCH | `#a371f7` | `#ffffff` | ~3.6:1 ✅ |
| DELETE | `#f85149` | `#ffffff` | ~3.8:1 ✅ |
| OPTIONS | `#30363d` | `#c9d1d9` | ~4.3:1 ✅ (already the VC-2b fallback text) |
| HEAD | `#21262d` | `#8b949e` | ~4.2:1 ✅ |
| ANY | `#6e7681` | `#ffffff` | ~3.9:1 ✅ |

**Elevation:** none to values. **VC-18:** every `.m-*` fill/text pair measures ≥3:1 on the shipped render; `.m-options` already ships `#c9d1d9` text (the prior VC-2b fallback baked in) so it is no longer borderline. The literal method text always renders → meaning never depends on hue.

### Status & served-by color system — reuse (`app.css:93-120`)
- **Status** = colored *text only*, never a fill (one filled badge per row max): `.status-2xx`→`--success`, `.status-3xx`→`--warn`, `.status-4xx`→`--client-err #e3a008`, `.status-5xx`→`--danger #f85149`. 4xx amber and 5xx red are **distinct hues** (VC-19). Status numbers get `.tabular` so they right-align as a column.
- **Served-by** = outline chip (`border: 1px solid currentColor`, no fill) so it never competes with the method badge: `.served-rule`→success, `.served-crud`/`.served-tunnel`→info, `.served-mitm`→warn, `.served-default`/`.served-cors`→`--text-muted`, `.served-chaos`→danger, `.served-ratelimit`→`--client-err`. **Elevation:** **[new — proposed]** tighten to `padding:1px 6px; font-size:11px; line-height:1.4; border-radius:var(--radius-sm)` and add `font-weight:500` so the outline chip reads crisp at feed density; on a selected/hover row the chip keeps its hue (it's already low-fill). The served-by *color* is a redundant cue — the chip always carries its literal label (Matched rule / Auto-CRUD / …).

### Feed row — the centerpiece: free-flex → aligned grid (`app.css:133-149`; markup `feed_row.html:40-58` + `dashboard.html:200-217`)
**c1 (P0).** Re-spec `.feed-row` from `display:flex` to a fixed-template **grid** so every column aligns down the list:

```css
.feed-row {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr) auto auto auto;
  /*               method | path (truncate) | status | served | latency·time   */
  align-items: center;
  column-gap: 10px;
  padding: 8px 12px;                    /* unchanged density */
  border-bottom: 1px solid var(--row-divider);
  border-left: 3px solid transparent;   /* method-colored rail when selected (VC-3) */
  font-family: var(--font-mono);
  font-size: 14px;
  line-height: 1.35;
  color: var(--text-2);
  text-align: left;
  background: transparent;
  cursor: pointer;
  transition: background-color var(--dur-base), border-color var(--dur-base);
}
```
- **method badge** → fixed **52px** gutter; paths start at one x-position.
- **path** → `minmax(0,1fr)` + `truncate` (keep `:title`/`title=` for the full value).
- **status** → right-aligned, `.tabular`, colored text.
- **served chip** → optional outline; `auto`-width column collapses to 0 when absent.
- **latency + time** → right metadata cluster, both `text-xs` + `.tabular`; latency `--text-muted`, time `--text-faint`. **[new — proposed]** wrap the two trailing spans so they occupy one grid cell: at narrow width (`<1024px` drawer / full-width feed) the served chip and latency may hide first via `@container`-free media rule; **method + path + status never hide** (the AC-28 essentials).
- **Selected/hover finish (c2):** hover `background:var(--surface-2)`; selected `.is-selected` → `--surface-2` fill + brightened `--text` + method-colored left rail via the existing `.rail-*.is-selected` map (`app.css:157-164`). Exactly **one** filled badge per row.
- **VC-C1 parity (LOCKED seam):** the span order in **both** `feed_row.html:50-57` and the `x-for` `dashboard.html:209-216` must be edited **identically** to match the 5-column grid; `data-request-id`, `data-cell`, `data-method`, `aria-label`, `role=option` are preserved on the server path. No new field is referenced (the `toRow()` shape in `request-stream.js:38-53` is the only data a row has).

### Inspector — header tie, tabs, KV, tree, trace (`inspector.html`, `inspector_body_tree.html`, `app.css:201-264`)
- **Header tie (d1):** the sticky header (`inspector.html:54-62`) repeats the selected row's **method badge color + mono path + status + served chip** — "one subject, one color." Add `.tabular` to its status + latency. Bindings (`$store.inspector.methodBadge/statusClass/servedClass/servedLabel`, `d()`) unchanged.
- **Tabs:** `.tab-btn` reused; **elevate** the active state — `.tab-btn.is-active` keeps `--link` text + 2px `--link` underline; add `font-weight:500` and a `hover { color: var(--text-2) }` (already present). Arrow-key nav / `aria-selected` / roving `tabindex` unchanged.
- **KV rows:** `.kv-row` reused (`kv-key min-width:9rem` kept). **Elevate** the copy button: the per-row `.copy-btn` shows at `--text-faint` and brightens to `--link` on row hover/focus (reuse `base.html:38-39` `.copy-btn`), so the affordance is present but quiet.
- **JSON tree:** `.tree-*`/`.v-*` syntax tints reused (string→success, number→link, bool/null→warn). **Elevate:** tighten `.tree-node line-height` to `1.55`; finish the disclosure triangle — `.tree-toggle` already rotates 90° via inline transform; add `transition: transform var(--dur-fast)`. Every key/value stays `x-text` (XSS-safe).
- **Trace timeline (d3):** elevate `.trace-step` into a vertical timeline. **[new — proposed]** add a faint connector rail and weight the glyph colors (already mapped: matched `●`→success, skipped `○`→`--text-faint`, state-write `◆`→info, chaos `✕`→danger, cors/other `◇`/`▸`→muted):
```css
.trace-step { position: relative; padding-left: 4px; }
.trace-glyph { width: 1.25rem; text-align: center; font-weight: 700; z-index: 1; }
/* connector: a hairline behind the glyph column, drawn between steps */
.trace-step:not(:last-child) .trace-glyph::after {
  content: ""; position: absolute; left: calc(0.625rem + 4px); top: 1.4rem; bottom: -0.4rem;
  width: 1px; background: var(--border);
}
```
The before→after diff (`.state-before` faint → `.state-arrow` muted → `.state-after` success) is kept. **The `decorateTraceStep` output contract (`stores.js:614-632`: `step/detail/cls/glyph/diff`) is unchanged** — no regex/class/glyph edits.
- **Body-tab controls (d5):** the local Expand-all / Collapse-all / Copy / Raw-Pretty controls live in a `.flex items-center gap-2` strip above each tree. Style them as `.btn-secondary text-xs px-2 py-1` (reuse) with the active Raw/Pretty segment marked by `--link` text + `--border-strong` border. **Copy** uses the already-stringified `reqBodyRaw`/`resBodyRaw` getter → toast; **Raw** uses the existing `<pre><code x-text>` fallback — **no captured data via `innerHTML`.**

### Rule modal + rule rows (`rule_modal.html`, `rule_row.html`)
- **Rail (e1):** the vertical tab rail keeps its active state (`bg-[#21262d] text-[#58a6ff] border-[#58a6ff]`). **Elevate:** tokenize the per-tab error dot from raw `bg-[#f85149]` to a class `.rule-tab-err { background: var(--danger); }` (8px, `aria-label="has errors"` kept); add `box-shadow: 0 0 0 2px var(--danger-bg)` so the dot reads as an alert, not decoration. Footer summary "N field(s) need attention" sits in `--danger` when count>0, `--text-muted` when "Ready to save".
- **Tag palette (e3):** the Templating tag buttons become a **mono chip palette** — reuse `.btn-secondary text-xs px-2 py-1 font-mono`, tighten `gap-2`, and give chips `border-color:var(--border)` resting → `--border-strong` on hover so the palette reads as a tidy token tray. Still `@click="appendTag(tag)"` into the shared `form.bodyTemplate`.
- **Webhook (e4):** the disabled `fieldset` (`opacity-60` + `disabled`) gets a **"Coming soon"** pill (`.alert--warn` inline, small) next to the legend so it reads intentionally-off; fields stay `disabled`; serialization unchanged (`rule-builder.js:274`).
- **Rule row (e5):** reuse `rule_row.html` layout. **Elevate** spacing to `px-3 py-2` (kept), add the method badge as the row's single fill, and render the disabled state at `opacity-60` (kept). The enable/disable toggle is the shared switch (below). Optimistic `$store.rules.toggle/editRule/confirmDelete` unchanged.

### Toggle switch — reuse the inline `sr-only peer` pattern (`endpoint_settings.html:73-76`, `rule_row.html:33-39`, `dashboard.html:83-88`)
The Tailwind peer switch (`w-9/11 h-5/6` track `#30363d`→checked `#238636`, white thumb, `peer-focus-visible:ring-2`) is **reused verbatim** across Auto-CRUD (bar + settings), Auto-CORS, and per-rule enable. **Elevate:** ensure all instances carry `after:transition-all` (already present) at `--dur-fast` feel; the checked green `#238636` matches `.btn-primary` so "on" reads consistently.

### Forms / inputs — reuse verbatim (`endpoint_settings.html`, `rule_modal.html`, `index.html`)
The shared input utility `bg-[#0d1117] border border-[#30363d] rounded px-3 py-2 focus:border-[#58a6ff] focus:outline-none` is reused everywhere; invalid fields flip border to `[#f85149]` (kept). **Elevate (f2 ranges):** **[new — proposed]** style native `range` tracks/thumbs in CSS (no JS), see Implementation notes — the only currently-unstyled control.

### Endpoint switcher — reuse native `<select>` (`dashboard.html:60-70`)
Keep the native `<select id="ep-switcher">` (a11y + zero-dep). **Elevate:** upgrade the `sr-only` label to a visible micro-eyebrow "Endpoint" (`text-[10px] uppercase tracking-wide text-[#6e7681]`); raise the select to `--border-strong` on focus; `@change`/option `<template>` untouched.

### Mock-URL chip — reuse (`app.css:269-282`)
`.mock-chip` (bg `--bg`, `--border`, mono, `--text-2` — **never `--link`**) reused. **Elevate:** the wide-screen bar chip is `hidden lg:inline-flex`; `Copy` button keeps `aria-label`; copying fires the existing toast.

### Brand mark — landing only (`index.html:23`)
**[new — proposed]** `.brand-mark` — a small **inline-SVG** wordmark mark (a stylized hook/bracket in `--link`, drawn with `<svg><path>` — no external asset) set left of "HookBox" wordmark (`--text`) with a 1px `--link` accent rule under the lockup. Presentational only.

---

## Visual hierarchy & layout

Maps to `ux.md §2`. Emphasis order, top to bottom:

1. **The live feed is the protagonist.** The 40/60 split (`feed = md:w-[40%] md:min-w-[360px]`, `inspector = flex-1`) is **kept** (the right developer-console balance). The feed earns weight through the saturated method gutter + the only animated element on screen (the live dot + arrival flash); everything else is desaturated chrome.
2. **One primary action.** `+ New Rule` is the single green button anywhere on the dashboard; the eye lands on it for the main create flow. The endpoint bar reflows into two clusters with a hairline `--border` divider before the right cluster (b1): left = identity (Endpoint switcher · + New endpoint · `lg+` mock chip), right = actions (Auto-CRUD · Rules · **+ New Rule** · Settings).
3. **Density without noise.** Mono 14px feed rows on an 8/12 padding rhythm, hairline `--row-divider` separators, `.tabular` numeric columns → a scannable ledger. Chrome (bars, headers, footer) uses `--text-muted`/`--text-faint` so saturated signal pops.
4. **Inspector reads as a detail surface, not a second app.** Sticky header tie (method color carried from the selected row) + underline tabs + scrollable panels; muted `text-xs uppercase tracking-wide` section eyebrows (`Request headers`, `Trace`, …) carry structure quietly.
5. **Overlays float above the console.** Rules/rule-modal/settings reuse `fixed inset-0 bg-black/50 z-50` + `.card max-w-* max-h-[85vh]` + `--shadow-overlay` — the only true elevation in the product.

The eye path: WS pill (is it live?) → top of feed (newest, flashing) → scan method gutter + status column → click → inspector header confirms subject by color → tabs.

---

## Visual states

One row per state `ux.md §3.7` defines. Treatment is visual-only; every gate/binding is preserved.

| Region · State | Gate (preserved) | Visual treatment |
| --- | --- | --- |
| **Feed · Loading** | `loading` → skeleton (`dashboard.html:164`) | 5 `.skeleton` rows shaped like the **real grid**: a 52px badge block + a path bar (`minmax`) + a short right cluster; `hb-shimmer`; **static** under reduced-motion (VC-24). |
| **Feed · Empty** | `!loading && rows.length===0` (`dashboard.html:173`) | Centered funnel: muted glyph, "No requests yet." (`--text`), sub-copy (`--text-muted`), the copy-only `.mock-chip`, and a copy-only sample `curl` in a `pre`/`code` (static text, never executed, no `http` asset) (VC-25). |
| **Feed · Has rows** | `rows.length>0` (`dashboard.html:198`) | The elevated grid rows (c1/c2). |
| **Feed · New arrival** | `_new` → `.feed-row--new` | Single `hb-flash` (`--success-bg`→transparent ≤900ms) + `hb-slide`; batched; no reflow; class removed by `request-stream.js:302` so re-selection doesn't re-flash (VC-22). |
| **Feed · Paused + buffered** | `paused && pending>0` (`dashboard.html:125`) | "N new" pill — `--success` fill, `--bg` text — `@click=flush()`; `#feed` `aria-live` flips to `off`. |
| **WS pill · 6 states** | `$store.stream.label` + `dotClass` (`request-stream.js:377-398`) | Each distinct AND text-labelled (never color-only, VC-26): **Live** `--success` `hb-pulse`; **Connecting…** `--warn` `hb-throb`; **Reconnecting…(n)** `--warn` `hb-throb`; **Realtime degraded** `--warn` static; **Offline** `--danger` static; **Unauthorized** `--danger` static. `role=status aria-live=polite`. |
| **Inspector · Empty** | `selectedId===null` (`dashboard.html:232`) | Centered muted glyph + "Select a request on the left to inspect it." |
| **Inspector · Loading** | `inspector.isLoading` | Centered spinner-dot glyph + "Loading detail…", `aria-busy`. |
| **Inspector · Pending** | `inspector.isPending` | Centered + "Detail still being written…" + faint sub-note + **Retry now** (`--btn-secondary`). |
| **Inspector · Unauthorized** | `inspector.isUnauthorized` | Centered `--danger` text "Not authorized to view this request." `role=alert`. |
| **Inspector · Error** | `inspector.isError` | Centered `--danger` text + **Retry**. `role=alert`. |
| **Inspector · Ready** | `inspector.isReady` | Header tie + tabs + panels. |
| **Rules / Settings / Modal / Landing · Error** | `*.error / saveError / serverError / errorBanner` | **Unified `.alert` family** (VC-23): `.alert--danger` (`--danger-bg` fill + `--danger` border + `--text`), `.alert--warn`, `.alert--info`, `.alert--success`. Replaces the raw `bg-red-900/50 border-red-500 text-red-200` mix in `index.html:40`, `rule_modal.html:41`, `endpoint_settings.html:48,56`, `dashboard.html:272`. **Copy unchanged** (esp. the AC-S5 neutral landing field copy). |
| **All buttons · Disabled** | `:disabled` / `.is-disabled` | `opacity:0.5; cursor:not-allowed` (`app.css:297-302`, reused). |
| **All controls · Focus** | keyboard focus | `outline:2px solid var(--link); outline-offset:2px` (`app.css:303`, reused) — **every** new control (Body-tab buttons, drawer back, wide chip) is a real `<button>` (VC-27). |
| **Hover** | pointer over interactive | Feed row / rule row → `--surface-2`; buttons → their hover fill; tabs → `--text-2`; copy → `--link`. |

---

## Motion & micro-interactions

All keyframes exist (`app.css:312-331`); this pass tunes timing and adds two motions, each with a
reduced-motion fallback. GPU-friendly properties only (opacity / transform / background) — **no
layout-thrashing** (no width/height/top animation in burst paths).

| Motion | Where | Spec | Reduced-motion |
| --- | --- | --- | --- |
| `hb-pulse` | live WS dot | `--success` box-shadow ring, ~2s loop | static dot |
| `hb-throb` | connecting/reconnecting dot | `--warn` opacity 1→.4, ~2s | static dot |
| `hb-flash` + `hb-slide` | new feed row | `--success-bg`→transparent ≤900ms + `translateY(-4px)`→0 ~200ms; **only per-row animation under burst** | no flash/slide |
| `hb-shimmer` | skeleton | 1.2s linear sweep | static block |
| **Drawer slide [new]** | mobile inspector (`<1024px`) | `transform: translateX(100%)`→`0`, `--dur-slow` `--ease-out`; `z-40` + `--shadow-overlay` | appears with no slide (VC-29) |
| **Sticky depth [new]** | panel bars (b3) | `box-shadow` fades in `--dur-fast` when content scrolls under | shadow may appear instantly |
| Button/toggle/tab/triangle/copy | everywhere | `--dur-fast` color/transform; `:active` 0.5px press | duration `0.01ms` (global rule) |
| Copy success | any copy button | pairs with the existing "Copied!" toast (no extra motion) | unchanged (toast is opacity only) |

**VC-28 (motion discipline):** under a 50-row burst, the only animated thing per row is the arrival
flash; the live dot keeps its calm 2s cadence; nothing reflows. The reduced-motion block
(`app.css:338-349`) is **extended** to cover the drawer slide, sticky-depth transition, and modal/tag-palette transitions.

---

## Responsive

Breakpoints are Tailwind's (`md=768px`, `lg=1024px`); the shell geometry is kept.

- **`≥768px` (md+):** side-by-side split — feed 40% (min 360px) + inspector `flex-1`. Endpoint bar + both panel headers sticky (`z-10`). Endpoint bar is one row; `flex-wrap` lets the right cluster wrap on tight widths. **VC-L1 (carried from ux.md):** sticky bars stay visible while `#feed` scrolls under them.
- **`<1024px`:** the feed grid's **served chip + latency reflow/hide first** (media rule); method + path + status always remain. The `lg+`-only bar mock-chip is hidden.
- **`<768px` (mobile, stacked):** the split stacks (`flex-col`). The inspector `<section id="inspector-root">` becomes a **full-screen drawer** driven purely by `$store.feed.selectedId` (d4): `selectedId===null` → translated off-canvas, feed full-width; `selectedId!==null` → slides over (`z-40`, `--shadow-overlay`) with a pinned **"← Feed"** back bar that calls the existing `feed.select(null)`. CSS + Alpine `:class`/`x-show` only; no store/contract change. At `md+` the drawer styles are inert. **VC-L2:** selecting a row on mobile slides the inspector in with a back control; activating it returns to the feed; desktop split unchanged.
- **Modals:** `max-w-*` + `p-4` backdrop padding keep overlays inside the viewport; internal `max-h-[85vh]` + `overflow-y-auto` handle short screens. The rule modal's vertical rail (`w-44`) stays; on very narrow widths it may wrap above panels (defer if risky — note in handoff).

---

## Visual accessibility

WCAG-AA target. Ratios computed against the surface each element sits on; QA re-verifies on the shipped render.

**Text / secondary (≥4.5:1):**
| Pair | Ratio | Bar |
| --- | --- | --- |
| `--text #e6edf3` on `--bg #0d1117` | ~15.8:1 | 4.5 ✅ |
| `--text` on `--surface #161b22` | ~14.0:1 | 4.5 ✅ |
| `--text-2 #c9d1d9` on `--surface` | ~11.6:1 | 4.5 ✅ |
| `--text-muted #8b949e` on `--bg` | ~6.4:1 | 4.5 ✅ |
| `--text-muted` on `--surface` | ~5.7:1 | 4.5 ✅ |
| `--link #58a6ff` on `--bg` | ~6.6:1 | 4.5 ✅ |
| `--success` text on `--surface` | ~6.1:1 | 4.5 ✅ |
| `--warn #d29922` text on `--surface` | ~6.6:1 | 4.5 ✅ |
| `--client-err #e3a008` text on `--surface` | ~7.4:1 | 4.5 ✅ |
| `--danger #f85149` text on `--surface` | ~5.0:1 | 4.5 ✅ |
| `--text-faint #6e7681` on `--bg` | ~4.0:1 | **<4.5 — decoration/timestamps only (VC-20)** |

**Badges / status / dots (graphical ≥3:1):** the §Method-badge table (all ≥3.4:1; OPTIONS now ~4.3:1). WS dots on `--surface`: success ~6.1 / warn ~6.6 / danger ~5.0 — all ≥3 ✅.

- **VC-20:** `--text-faint` is restricted to non-load-bearing decoration (tree punctuation, disabled glyphs, the feed timestamp which is *also* conveyed by row order) — never sole-source copy.
- **VC-21 focus-visible:** every interactive element (feed row, tab, button, toggle, chip, copy, tree node, drawer back) shows `outline:2px solid var(--link); outline-offset:2px` on keyboard focus; inputs keep `focus:border-[#58a6ff]` **plus** the ring. No focus removed without a visible replacement.
- **VC-26 non-color signaling:** method + status carry literal text; WS pill carries a text label; trace uses distinct glyphs (`●○◆✕◇▸`) in addition to color; the new-row cue pairs motion with the "N new" counter and the row's own text. No state is hue-only.
- **VC-29 prefers-reduced-motion:** the extended `@media (prefers-reduced-motion: reduce)` block stops the WS pulse/throb, row flash/slide, skeleton shimmer, drawer slide, sticky-depth and panel/tag transitions; every state stays legible by color + text + glyph.

---

## Implementation notes

All net-new CSS appends to `static/css/app.css` and consumes existing `:root` tokens. Files
`frontend-engineer` touches and the reuse-vs-new call:

**`static/css/app.css` (extend; the bulk of the work):**
```css
/* Tabular numerics for aligned columns (feed status/latency/time, inspector header) */
.tabular { font-variant-numeric: tabular-nums; }

/* c1 — feed row becomes an aligned 5-column grid (replaces the flex block) */
.feed-row { display: grid; grid-template-columns: 52px minmax(0,1fr) auto auto auto;
  align-items: center; column-gap: 10px; /* …keep all other props from app.css:138-149 */ }
.feed-meta { display: inline-flex; align-items: center; gap: 8px; white-space: nowrap; }
@media (max-width: 1023px) {
  .feed-row { grid-template-columns: 52px minmax(0,1fr) auto; }  /* drop served+latency */
  .feed-row .served-chip, .feed-row .feed-latency { display: none; }
}

/* g4 — unified alert family (replaces raw bg-red-900/50 utility clusters) */
.alert { border: 1px solid; border-radius: var(--radius); padding: 8px 12px; font-size: 14px;
  color: var(--text); }
.alert--danger  { background: var(--danger-bg);  border-color: var(--danger); }
.alert--warn    { background: var(--warn-bg);    border-color: var(--warn); }
.alert--info    { background: var(--info-bg);    border-color: var(--info); }
.alert--success { background: var(--success-bg); border-color: var(--success); }

/* d3 — trace timeline connector (see §Component for the rule) */
/* b3 — sticky depth */
.panel-flush.is-scrolled { box-shadow: 0 1px 0 var(--border), 0 6px 12px -8px rgba(1,4,9,.6); }

/* f2 — native range track/thumb (the one unstyled control) */
input[type="range"] { -webkit-appearance: none; appearance: none; height: 4px; border-radius: 9999px;
  background: var(--surface-3); }
input[type="range"]::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 14px;
  border-radius: 9999px; background: var(--link); cursor: pointer; }
input[type="range"]::-moz-range-thumb { width: 14px; height: 14px; border: 0; border-radius: 9999px;
  background: var(--link); cursor: pointer; }

/* d4 — mobile inspector drawer (inert at md+) */
@media (max-width: 767px) {
  #inspector-root.insp-drawer { position: fixed; inset: 49px 0 0 0; z-index: 40;
    background: var(--surface); box-shadow: var(--shadow-overlay);
    transform: translateX(100%); transition: transform var(--dur-slow) var(--ease-out); }
  #inspector-root.insp-drawer.is-open { transform: translateX(0); }
}

/* served-chip tune; tab/button weight; brand-mark accent rule — additive */
```
Extend the reduced-motion block (`app.css:338`) to add `.insp-drawer`, `.panel-flush`, and tag-palette transitions to the `animation:none` / `transition-duration:0.01ms` set.

**Templates (restyle + light additive markup only; every binding/id/ARIA preserved):**
- `templates/dashboard.html` — endpoint bar two-cluster reflow + divider; switcher eyebrow; `lg+` mock chip; the `x-for` feed-row spans wrapped into the 5-col grid (`.feed-meta`, `.tabular`); skeleton geometry; empty-state funnel + sample `curl`; `.alert--danger` in the rules overlay; drawer `:class="{ 'insp-drawer': true, 'is-open': $store.feed.selectedId !== null }"` + "← Feed" back `<button>` calling `$store.feed.select(null)`; optional local `feedScrolled` flag for b3.
- `templates/partials/feed_row.html` — **edit the spans identically** to the `x-for` (5-col grid, `.feed-meta`, `.tabular`); keep `data-cell`/`data-method`/`data-request-id`/`aria-label`.
- `templates/partials/inspector.html` — header `.tabular`; trace timeline markup unchanged (CSS does the rail); Body-tab control strip (local buttons); centered state gates.
- `templates/partials/inspector_body_tree.html` — no markup change (triangle transition is CSS).
- `templates/partials/rule_modal.html` — `.rule-tab-err` dot class; `.alert--danger` server-error banner; mono chip palette tune; webhook "Coming soon" pill.
- `templates/partials/rule_row.html` — spacing/badge finish (no contract change).
- `templates/partials/endpoint_settings.html` — section headers (Identity · Behavior · Simulated network conditions · Danger zone); `.alert--danger` banners; range styling consumed; MITM `.alert`-style inline error.
- `templates/index.html` — `.brand-mark` lockup; value-prop strip (Mock · Intercept · Inspect); `.alert--warn`/`.alert--danger` banners.
- `templates/base.html` `<style>` — keep the inline parity block; the canonical tokens stay in `app.css`. Add `.brand-mark` here only if it must paint pre-`app.css`.

**Reuse vs new summary:** reuse `.card`, `.btn-*`, `.badge`+`.m-*`, `.status-*`, `.served-chip`, `.panel-flush`, `.feed-row`+`.rail-*`, `.ws-pill`/`.ws-dot*`, `.tab-btn`, `.kv-row`, `.tree-*`/`.v-*`, `.trace-*`, `.mock-chip`, `.skeleton`, `:disabled`/`:focus-visible`, all keyframes, `#toast`/`showToast`/`copyToClipboard`. **New (all additive, all token-fed):** `.tabular`, `.feed-row` grid internals, `.feed-meta`, `.alert*`, `.trace-glyph::after` rail, `.panel-flush.is-scrolled`, `input[type=range]` styling, `.insp-drawer`, `.rule-tab-err`, `.brand-mark`.

---

## UX handoff notes

1. **Sticky-depth shadow (b3) wants a tiny local JS flag.** The cleanest visual (shadow only when content has scrolled) needs a `feedScrolled` boolean toggled on `#feed @scroll` — a **local `dashboard()` UI flag** touching no store. If ui-ux/PM prefers strictly zero new JS, ship the static hairline only (still passes the sticky VC). Flagging so it's a conscious call.
2. **`<1024px` feed reflow hides served-chip + latency.** `ux.md §3.3` says these "may wrap/hide first"; this doc commits to **hide** (via media rule) to keep method+path+status. Confirm hide-vs-wrap is acceptable.
3. **Rule-modal vertical rail on very narrow widths.** The `w-44` rail + panels can crowd below ~520px. This pass keeps the rail (a11y/structure from ux.md) and lets it sit above panels if it must wrap; if ux.md wants a different small-screen rule layout, reconcile in the revise loop.
4. **Drawer covers from `top:49px`** (below the slim nav). This assumes the nav height stays 49px (matches `dashboard.html:34` `calc(100vh - 49px)`). If the nav height changes, the drawer `inset` must track it — purely a CSS constant.

---

## PRD gaps

> Numbered, UI-facing visual requirements/ACs the PM should add or clarify. None require touching
> `app/`; FE consequence noted where relevant.

1. **Sample-request hint copy + safety (empty feed, g2/VC-25).** Confirm (a) the exact canonical `curl` text (e.g. `curl https://<your-mock-url>/ping`) and (b) that it is copy-only, rendered as static text, never executed and never a live `http` asset — so the activation hint is consistent and verifiable.
2. **Wide vs narrow mock-URL chip (b2).** Should the `lg+` bar chip and the feed-column chips be mutually exclusive (hide feed chips at `lg+`) or both visible? Purely presentational but affects redundancy; pin the target so QA has one.
3. **Minimum supported width + target desktop width.** The feed grid (`52px | 1fr | status | served | latency·time`) and the `<768px` drawer assume a target. State the supported minimum width so QA verifies the grid breakpoints + drawer at the intended size.
4. **"Realtime degraded" — does it ever fire?** The store exposes `degraded` (`request-stream.js:375`) and this doc gives it a labelled amber-static treatment (VC-26), but nothing currently sets `degraded`. Confirm whether the stream should ever enter it (e.g. SSE fallback) so the visual isn't dead UI; if never, drop it from the labelled set.
5. **Body-tab Raw/Pretty default + large-tree Expand-all bound (d5).** Confirm the default view (Pretty/tree) and whether Expand-all is bounded for very large trees (the flattener caps at 5000 nodes, `util.js:137`) so Expand-all has predictable, testable behavior.
6. **Webhook "Coming soon" visual is gated on scope (e4).** This pass styles the disabled webhook section as intentionally-off; the PM must confirm webhook stays out for this milestone (so it remains disabled-but-serialized) — otherwise fields become live, a behavior change beyond this lane.
7. **Sticky-depth shadow acceptance (b3).** If gap #1 in handoff notes lands on "static hairline only," the PM should write the sticky-bar AC against the hairline (not the drop shadow) so QA tests the shipped behavior.
8. **Contrast re-measure as an AC.** Add an AC requiring the rendered (not computed) method/status/dot pairs to be measured ≥3:1 and text ≥4.5:1 on the shipped page, since Tailwind/`app.css` cascade order could shift a swatch.
