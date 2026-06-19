# Visual design: HookBox — Beeceptor-class API Mocking & Interception Platform (slug: beeceptor-rewrite)

> **Lane.** This is the **aesthetic layer on top of `ux.md`** (the functional contract) — color,
> typography, spacing, hierarchy, motion, polish. I do **not** re-open `ux.md`'s structure, copy,
> states, or accessibility decisions; I give each a visual treatment. Visual choices that touch UX
> are flagged under **§11 UX handoff notes**.
>
> **Grounding (non-negotiable).** Every token is tagged **[existing — verified at `templates/<file>:line`]**
> or **[new — proposed]**. The base is the **GitHub-dark** system already in `base.html` `<style>`
> (`base.html:9-24`). I extend it; I do not fork a parallel system. Everything is plain CSS custom
> properties + Tailwind arbitrary values + light JS — `frontend-engineer` builds it from this spec alone.
>
> **Stack alignment.** Server-rendered Jinja2 + HTMX + Alpine.js + Tailwind, no React/JSX/Vite/Node
> build (LOCKED `_decisions.md` §1, §1a). Tailwind is the Play CDN today (`base.html:7`) unless the
> architect swaps to a CLI build (PRD §5/§7) — **this doc's classes are identical either way**; the
> only dependency is that the few `[new]` keyframes/utilities below live in `base.html`'s `<style>`
> (CDN) or a `static/css/app.css` (CLI build).
>
> **Testability.** Every visual decision that QA can verify is written as **VC-## (Visual Criterion)**
> so the PM can lift them into PRD §4 and resolve **OQ-15** (`prd.md:232`). VC-IDs continue the PRD's
> AC space conceptually but are namespaced `VC-` so they don't collide with AC-1..AC-45.

---

## 1. Design direction

HookBox should read as a **precision developer instrument**, not a SaaS marketing surface: a quiet,
near-black GitHub-dark canvas where the only saturated color is **signal** — the HTTP method on a feed
row, a status code, the pulsing "Live" dot, the green "matched rule" branch in a trace. The aesthetic
is **dense, monospace-forward, and calm under load**: hundreds of requests can stream into the left feed
without the screen feeling noisy, because chrome is desaturated (`#161b22`/`#21262d` greys, `#30363d`
hairlines) and color is reserved for the few things a developer scans for. This is a direct extension of
the existing HookBox look (`base.html:9-24`) — same `#0d1117` page, same `#161b22` cards, same `#238636`
green primary, same Monaco/Menlo mono — pushed from "a webhook catcher with one blue badge" to a
**high-density real-time console** with a full semantic palette, a method-badge color map, and motion
that confirms liveness without distracting.

---

## 2. Design tokens

All values below are formalized as **CSS custom properties** so FE has one source of truth. The "Source"
column proves provenance; **[existing]** values are copied verbatim from the cited line, **[new]** values
are proposed by this doc (and must clear the contrast checks in §9).

### 2.1 Color — surfaces & lines

| Token | Value | Role | Source |
| --- | --- | --- | --- |
| `--bg` | `#0d1117` | page background; `pre/code`/input wells | [existing — `base.html:9,19`] |
| `--surface` | `#161b22` | card / panel background | [existing — `base.html:10`] |
| `--surface-2` | `#21262d` | raised/selected row, secondary button, hover fill | [existing — `base.html:13`; hover row `dashboard.html:211`] |
| `--surface-3` | `#30363d` | hairline borders, toggle track, divider | [existing — `base.html:10,13`] |
| `--border` | `#30363d` | default 1px border | [existing — `base.html:10`] |
| `--border-strong` | `#3d444d` | border on hover/focus of dense controls | [new — proposed] |
| `--row-divider` | `#21262d` | feed/table row separators (subtler than `--border`) | [existing — `dashboard.html:211`] |

### 2.2 Color — text

| Token | Value | Role | Source |
| --- | --- | --- | --- |
| `--text` | `#e6edf3` | primary body text | [existing — `base.html:9`] |
| `--text-2` | `#c9d1d9` | secondary text, secondary-button label | [existing — `base.html:13`] |
| `--text-muted` | `#8b949e` | labels, metadata, breadcrumbs, timestamps | [existing — `base.html` usage; `dashboard.html:7`] |
| `--text-faint` | `#6e7681` | disabled text, placeholder, tree punctuation | [new — proposed] |
| `--link` | `#58a6ff` | links, focus ring, copy-hover, mono identifiers | [existing — `base.html:22`; `dashboard.html:166`] |

### 2.3 Color — semantic / intent (the signal layer)

| Token | Value | Meaning | Source |
| --- | --- | --- | --- |
| `--success` | `#3fb950` | 2xx, "Live", success alert, matched-rule branch | [existing — `dashboard.html:189`; success alert `login.html:16`] |
| `--success-strong` | `#2ea043` | primary-button hover, success emphasis | [existing — `base.html:12`] |
| `--success-bg` | `rgba(63,185,80,0.15)` | success alert fill / 2xx row tint | [new — proposed] (derived from `--success`) |
| `--info` | `#58a6ff` | info alerts, GET method, links | [existing — `base.html:22`] |
| `--info-bg` | `rgba(88,166,255,0.15)` | info alert fill | [new — proposed] |
| `--warn` | `#d29922` | 3xx, reconnecting, chaos/throttle warnings | [new — proposed] (GitHub "attention" amber; ux.md §2.4 names `#d29922`) |
| `--warn-bg` | `rgba(210,153,34,0.15)` | warn alert / 3xx row tint | [new — proposed] |
| `--danger` | `#f85149` | 5xx, "Offline", danger-button hover, errors | [existing — `base.html:16`; `dashboard.html:128`] |
| `--danger-base` | `#da3633` | danger-button rest fill | [existing — `base.html:15`] |
| `--danger-bg` | `rgba(248,81,73,0.15)` | error alert fill / 5xx & 4xx row tint | [new — proposed] (matches `bg-red-900/50` intent, `login.html:15`) |
| `--client-err` | `#e3a008` | 4xx status text (distinct from 5xx red, see VC-9) | [new — proposed] |

> The existing inline alerts use Tailwind `bg-red-900/50 … border-red-500` etc. (`login.html:15-16`,
> `backup.html:148`). I keep those classes verbatim where they appear; the `--*-bg` tokens above are for
> the **new** surfaces (status-code row tints, trace branches) so they share the same hue family.

### 2.4 Color — HTTP method badge map (VC-1 — the method-badge palette)

The current single blue badge (`.badge-method` `#388bfd`, `base.html:18`) does **not** scale to a feed
where method is the primary scan target (ux.md §3.2). A **filled, color-coded** map by HTTP semantics
(read=blue, create=green, replace=amber, mutate=purple, destroy=red) — the convention developers already
know from Postman/Swagger — with **white or near-black text chosen per swatch for contrast** (§9):

| Method | Badge fill | Text color | Hue rationale | Source |
| --- | --- | --- | --- | --- |
| **GET** | `#388bfd` | `#ffffff` | read = the existing blue, kept | [existing — `base.html:18`] |
| **POST** | `#3fb950` | `#0d1117` | create = success green | [new] (reuses `--success`) |
| **PUT** | `#d29922` | `#0d1117` | full replace = amber | [new] (reuses `--warn`) |
| **PATCH** | `#a371f7` | `#ffffff` | partial mutate = purple | [new — proposed] (GitHub "done" purple) |
| **DELETE** | `#f85149` | `#ffffff` | destroy = danger red | [new] (reuses `--danger`) |
| **OPTIONS** | `#30363d` | `#8b949e` | preflight/CORS = neutral chrome | [new] (reuses `--surface-3`/`--text-muted`) |
| **HEAD** | `#21262d` | `#8b949e` | metadata = quietest neutral | [new] (reuses `--surface-2`) |
| **ANY / other** | `#6e7681` | `#ffffff` | catch-all wildcard | [new] (reuses `--text-faint`) |

- `OPTIONS`/`HEAD` are deliberately **desaturated** so the high-frequency CORS-preflight chatter the
  Auto-CORS engine generates (AC-18) does not flood the feed with color.
- The badge **always renders the literal method text** (GET/POST/…), so meaning never depends on hue
  (color-blind safety, ux.md §5). VC-1 is satisfied by the swatch table **and** the per-pair contrast in §9.

### 2.5 Color — status-code map (VC-9)

Status is shown as **colored text/number** (not a filled pill — keeps rows scannable, one filled badge
per row max = the method):

| Class | Color token | Example | Source |
| --- | --- | --- | --- |
| 2xx | `--success` `#3fb950` | `200` `201` | [existing — `dashboard.html:189`] |
| 3xx | `--warn` `#d29922` | `301` `304` | [new] |
| 4xx | `--client-err` `#e3a008` | `404` `429` | [new — proposed] |
| 5xx | `--danger` `#f85149` | `502` `503` | [existing — `dashboard.html:128`] |

VC-9: 4xx and 5xx use **visibly distinct hues** (amber vs red) so a developer can tell "I sent a bad
request" (4xx) from "the mock/chaos failed" (5xx) at a glance — never both red.

### 2.6 Typography

| Token | Value | Source |
| --- | --- | --- |
| `--font-sans` | system UI stack: `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif` | [new — proposed] (Tailwind/browser default today; formalized) |
| `--font-mono` | `'Monaco', 'Menlo', 'Ubuntu Mono', monospace` | [existing — `base.html:20`] |

**Type scale** (Tailwind classes already in use; ratios formalized):

| Step | Tailwind | px / line-height | Use | Source |
| --- | --- | --- | --- | --- |
| Display | `text-2xl font-bold` | 24 / 32 | entry title, "Endpoint not found" | [existing — `login.html:10`] |
| Heading | `text-lg font-bold` | 18 / 28 | modal/section headers | [existing — `dashboard.html:18`] |
| Body | `text-base` | 16 / 24 | form values, prose | [existing — default] |
| Dense / UI | `text-sm` | 14 / 20 | dashboard chrome, feed rows, inputs | [existing — `dashboard.html:7`] |
| Micro | `text-xs` | 12 / 16 | metadata, badges, footer, helper | [existing — `base.html:17`; `dashboard.html:11`] |

- **Mono is load-bearing**, not decorative: paths, tokens, headers, query/body keys, JSON tree, code
  chips, status numbers all use `--font-mono` (extends `code` `base.html:20` and the `font-mono` usage at
  `dashboard.html:166,213`). Chrome/labels/buttons use `--font-sans`.
- **VC-12 (feed density):** feed rows render at `text-sm` (14px) with `--font-mono` for the path; the
  method badge is `text-xs` (12px, matches `.badge` `base.html:17`). This is the "high-density" register
  the spec demands (prompt §2.2) and matches the existing compact scale.

### 2.7 Spacing, radius, elevation, z-index

| Token | Value | Source |
| --- | --- | --- |
| Spacing scale | Tailwind 4px base: `1=4 2=8 3=12 4=16 5=20 6=24` px | [existing — `base.html:11` `8px 16px`; `:19` `12px`] |
| **Dense row padding** | `px-3 py-2` (12/8px) for feed rows & KV rows | [new] (between table `p-3` `dashboard.html:212` and tighter feed need) |
| `--radius-sm` | `4px` — badges, chips, status pills | [existing — `base.html:17`] |
| `--radius` | `6px` — buttons, inputs, `pre`, small cards | [existing — `base.html:11,19`] |
| `--radius-lg` | `8px` — `.card`, modal, panels | [existing — `base.html:10`] |
| `--shadow-overlay` | `0 16px 48px rgba(1,4,9,0.85)` | [new — proposed] (modal/drawer/popover lift off dark bg) |
| `--shadow-pop` | `0 8px 24px rgba(1,4,9,0.7)` | [new — proposed] (tag palette, dropdowns) |
| `--ring` | `0 0 0 2px #0d1117, 0 0 0 4px #58a6ff` | [new — proposed] focus-visible ring (offset on `--bg`) |
| z: base | `0` | content |
| z: sticky | `10` | endpoint bar, feed/inspector headers (sticky) | [new] |
| z: drawer | `40` | mobile inspector drawer | [new] |
| z: overlay | `50` | modal/settings backdrop | [existing — `dashboard.html:15`] |
| z: popover | `60` | tag palette, menus above modal | [new] |
| z: toast | `1000` | toast | [existing — `base.html:23`] |

> The existing UI ships **no shadows** (flat borders only). The `--shadow-*` tokens are **net-new**, used
> *only* on floating layers (modal, drawer, popover, toast) to separate them from the busy feed behind —
> not on flat cards (cards stay border-only, consistent with `.card` `base.html:10`).

### 2.8 Motion tokens (durations & easing)

| Token | Value | Use | Source |
| --- | --- | --- | --- |
| `--dur-fast` | `120ms` | hover, button, tab, toggle | [existing — `base.html:11` `0.2s` rounded down for snappier dense UI] |
| `--dur-base` | `200ms` | row select accent, panel swap | [existing — `base.html:11,13` `0.2s`] |
| `--dur-slow` | `300ms` | toast fade, drawer slide | [existing — `base.html:23` `0.3s`] |
| `--dur-flash` | `900ms` | new-row arrival highlight decay (§6) | [new — proposed] |
| `--dur-pulse` | `2000ms` | WS "Live" breathe cycle (§6) | [new — proposed] |
| `--ease-out` | `cubic-bezier(0.16,1,0.3,1)` | entrances, row flash | [new — proposed] |
| `--ease-standard` | `ease` | hovers/colors (matches existing `transition`) | [existing — `base.html:11`] |

---

## 3. Component styling

Per affected component: exact visual spec + reuse-vs-new + the template/partial it lives in
(file list from ux.md §1). "Reuse" = lift the existing class/markup unchanged.

### 3.1 Buttons — **reuse verbatim** (`base.html:11-16`)
`.btn-primary` (green `#238636`→hover `#2ea043`), `.btn-secondary` (`#21262d` + `#30363d` border),
`.btn-danger` (`#da3633`→`#f85149`). No visual change. **Add** only a `focus-visible` ring (§9) and a
`:disabled` style (VC-15): `opacity:0.5; cursor:not-allowed;` plus, during HTMX in-flight, the label
swaps to the "-ing" copy from ux.md §4 (e.g. "Saving…") — reusing the `submitBtn.disabled` convention
(`login.html:45`). **Dense variant** (feed toolbar, KV copy buttons): same colors at `text-xs px-2 py-1`.

### 3.2 Cards & panels — **reuse `.card`** (`base.html:10`)
`#161b22` / `1px #30363d` / `8px`. Used for: entry card, modal shell, settings panel, empty-state cards,
"Endpoint not found" card. **New variant `--card-flush`** (ux.md §2.2): a `.card`-colored bar with
**no radius and only a bottom border** for the sticky endpoint bar and feed/inspector headers, so the
split-screen panes meet flush with no rounded gaps:
```css
.panel-flush { background: var(--surface); border-bottom: 1px solid var(--border); }
```

### 3.3 Feed row — **[new]** `partials/feed_row.html` (the centerpiece, ux.md §3.2)
A horizontal, single-line-dense row tuned for fast vertical scanning:
```
[method badge]  [path (mono, truncate, flex-1)]   [status]  [served-by chip?]  [latency]  [time]
```
- Container: `<button>`/`<tr role=button>` (ux.md §5), `display:flex; align-items:center; gap:8px;`
  padding `px-3 py-2`, bottom border `--row-divider`, `font-mono text-sm`.
- **Left accent rail (VC-3):** a `3px` left border, **transparent at rest**, that becomes the row's
  **method color** when the row is `selected` — so the inspector's subject is unmistakable and color-tied
  to its method. (Reuses the method map §2.4.)
- States (one row per ux.md state, §5): **rest** transparent; **hover** `background:var(--surface-2)`
  (reuses `hover:bg-[#21262d]` `dashboard.html:211`); **selected** `background:var(--surface-2)` + method
  accent rail + `--text` brightened; **new-arrival** flash (§6); **focus-visible** ring (§9).
- Method badge = `.badge` geometry (`base.html:17`) recolored per §2.4; status = colored number per §2.5;
  served-by chip = §3.7; latency `text-xs --text-muted`; time = relative ("12s ago", reuse `formatDate`
  `dashboard.html:256`) `text-xs --text-faint`.

### 3.4 Inspector — **[new]** `partials/inspector.html`, `inspector_body_tree.html` (ux.md §3.4)
- **Header strip:** `panel-flush`, holds method badge · full path (mono, wrap) · status · latency ·
  served-by chip. Sticky (`z:sticky`).
- **Tab strip:** underline tabs (reuse the existing active-tab pattern `index.html:21`:
  `text-[#58a6ff] border-b-2 border-[#58a6ff]`); inactive `--text-muted` → hover `--text-2`. `role=tablist`
  (ux.md §5). VC-13: active tab = 2px `--link` underline + `--link` text; this is the **only** underline-tab
  pattern, reused from `index.html:21`.
- **Headers / Query KV rows:** definition-list rows — mono `--text-muted` key (min-w, right-pad), mono
  `--text` value (selectable, with a per-row dense copy button §3.1). Zebra-free; `--row-divider` between
  rows. Replaces the `JSON.stringify` `<pre>` dump (`dashboard.html:233`).
- **JSON/XML tree (`inspector_body_tree.html`):** recursive Jinja partial; each node:
  - disclosure triangle `▸/▾` in `--text-faint`, rotates on open (`transition: transform var(--dur-fast)`).
  - key in `--text-muted`, `:` punctuation `--text-faint`, **value syntax-tinted (VC-10):** string=`--success`
    light variant, number=`--link`, boolean/null=`--warn`, array/object brackets=`--text-faint` with a
    muted child-count (`[3]`). Tinting matches a calm dark-syntax theme; tints reuse semantic tokens so the
    palette stays small.
  - long strings truncate with an inline "expand". Built server-side (works without JS); Alpine only toggles
    `open`. Fallback for non-JSON/XML: mono `<pre>` (reuse `pre` `base.html:19`).
- **State & Tracing (VC-11):** a **vertical step list**, not raw JSON (ux.md §3.4). Each step = a left
  status glyph + text: matched=`--success` ●, skipped=`--text-faint` ○, state-write=`--info` ◆,
  CORS=`--text-muted`, chaos/error=`--danger` ✕. State mutations render `key: before → after` with
  before in `--text-faint`, arrow `--text-muted`, after in `--success`. The "final branch" is a bold
  full-width row tinted by branch type. This is the debugging payoff — high legibility over density.

### 3.5 Form fields — **reuse verbatim** (`login.html:22`, `mock.html`)
Input/select/textarea: `bg-[#0d1117] border border-[#30363d] rounded px-3 py-2 focus:border-[#58a6ff]
focus:outline-none` (reuse exactly). Mono textareas for headers/body (reuse `mock.html:136,141`). Labels
`text-sm` / helpers `text-xs --text-muted` (reuse `mock.html:79,88`). **Error state (VC-14):** `aria-invalid`
→ `border-color:var(--danger)`; inline message in the existing inline-alert style (`bg-red-900/50 border
border-red-500 text-red-200`, `login.html:15`).

### 3.6 Toggle switch — **reuse verbatim** (`mock.html:108-111`)
The Tailwind peer switch, `peer-checked:bg-[#238636]` (green = on, reuses primary). Used for Auto-CRUD
(endpoint bar + settings) and per-rule enable/disable. **In-flight (optimistic PATCH, ux.md §3.5):** track
→ `--text-faint` + `aria-busy` while the request is in flight; revert + toast on failure.

### 3.7 Badges & chips
- **Method badge:** §2.4. **Status:** §2.5 colored text.
- **Served-by chip (VC-8) — [new]:** small `--radius-sm` outline chip (no fill, to not compete with the
  method badge) labeling how a request was handled — text + a 1px border in the branch color:
  `Matched rule` (`--success`), `Auto-CRUD` (`--info`), `Proxied` (`--warn`), `Default 404` (`--text-muted`),
  `Chaos` (`--danger`), `Rate-limited` (`--client-err`). Labels & enum are `[ARCH-GAP: served_by enum,
  OQ-10]` (ux.md §3.4) — colors map by intent regardless of final labels. Drives AC-13/15/16/26 visibility.
- **Mock-URL chip (VC-7):** copyable, **non-anchor** `code` chip — `bg-[#0d1117]` well, mono `text-sm`,
  truncate, a `Copy` button (reuse `code`+copy `index.html:93-95`, but as `<button>` not the bare `📋`).
  A subtle "Local fallback" label distinguishes the `/e/<token>/…` chip from the subdomain chip. Rendered
  **muted/neutral, never `--link` blue**, to signal "this is the mock origin, not a dashboard link" (ux.md §2.0).

### 3.8 WS health pill — **[new]** (ux.md §2.4; replaces `dashboard.html:97`)
A pill = dot + text label, bound to the Alpine `stream` store:
| State | Dot | Label | Motion |
| --- | --- | --- | --- |
| connected | `--success` `#3fb950` | "Live" | pulse/breathe (§6) |
| connecting/reconnecting | `--warn` `#d29922` | "Reconnecting…" (+`(n)` after N tries) | slow opacity throb |
| disconnected/closed | `--danger` `#f85149` | "Offline" | none (static) |
Pill: `text-xs`, dot `8px` round, `gap:6px`, `--text-2` label. `role=status aria-live=polite` (ux.md §5).
**Never color-only** — the text label always carries the state (VC-6).

### 3.9 Modal & settings overlay — **reuse geometry** (`dashboard.html:15-23`)
Backdrop `fixed inset-0 bg-black/50 z-50`; panel `.card w-full max-w-4xl max-h-[85vh]` + header(title+`×`)
+ `--shadow-overlay` (new, to lift off the busy feed). **Rule modal** uses a **left vertical tab rail**
(ux.md §2.3): rail `w-44 border-r --border`; active rail item `bg-[#21262d]` + `--link` text + 2px left
accent; a rail item shows a **state dot** — `--warn` ● for non-default/unsaved, `--danger` ● if that tab
has a validation error (VC-14). Footer `border-t --border`, validation summary left, `[Cancel][Save]` right.

### 3.10 Tag palette (templating tab) — **[new]**
Clickable token chips that insert at cursor: `.btn-secondary` geometry at `text-xs`, mono label
(`{{now 'iso'}}`…), `--shadow-pop` if floating. Hover → `--surface-3`. Purely visual layer over ux.md §2.3.

### 3.11 Nav shell — **reuse** (`base.html:29-50`), de-emoji'd
Keep the bordered nav (`border-b --border`). Brand "HookBox" in `--link` bold, **drop the 📦 emoji**
(ux.md §0/§1 calls it; aligns the brand with the precision-tool tone). Right side = Alpine-rendered email
+ Logout (`.btn-secondary text-sm`, reuse `base.html:48`). **Slim variant on `/d/<token>`** (reduced
`py-2`) to give the split-screen vertical room (ux.md §2.2).

---

## 4. Visual hierarchy & layout

Maps to ux.md §2.2's full-bleed split shell. The hierarchy by emphasis (loudest → quietest):

1. **The live feed's signal columns** — method badge (only saturated fill on a row) + status color. The
   eye lands here first; everything else on the row is mono-grey.
2. **The selected row + its inspector** — the method-colored accent rail (§3.3) visually ties the
   highlighted feed row to the inspector header, which repeats the same badge. One subject, one color.
3. **Primary actions** — `[+ New Rule]` (green primary) in the endpoint bar; `[Save rule]` (green) in the
   modal footer. Green = the one "go" color, reused from `--btn-primary`.
4. **Chrome** — endpoint bar, toolbars, tab strips, labels: all `--surface`/`--surface-2` greys + `--text-muted`,
   deliberately recessive so 100 streaming rows never fight the controls.

**Density & rhythm:** dashboard chrome is `text-sm`/`text-xs` on `~40-48px` bars (ux.md §2.2) — a tool, not
a brochure. Vertical hairlines (`--border`) split feed|inspector; horizontal `--row-divider` separates feed
rows. **Alignment:** method badges form a fixed left gutter so paths align into a scannable column; status
codes right-align before the timestamp. **Eye-flow:** top-left (newest row, where arrivals flash) → down the
path column → right into the inspector → down through its tabs.

---

## 5. Visual states — one row per state ux.md defines (ux.md §3.6)

| State | Visual treatment | Tokens / source |
| --- | --- | --- |
| **Loading** | Feed: 3–5 **skeleton rows** — `--surface-2` blocks at row height with a faint left-to-right shimmer (1.2s linear, reduced-motion → static). Inspector/modal: centered spinner + `aria-busy`; HTMX `hx-indicator`. Buttons: disabled + "-ing" label. | `--surface-2`; spinner `--text-muted`; reuse `login.html:45` |
| **Empty (feed)** | Centered `.card`-less hint: muted icon, "No requests yet.", the **copyable mock-URL chip** (§3.7) + "send a test request" hint. Turns a dead screen into the first-call funnel (ux.md §3.6). | `--text-muted`; chip §3.7 |
| **Empty (inspector)** | Vertically-centered `--text-muted` "Select a request on the left to inspect it." | `--text-muted` |
| **Empty (rules)** | "No rules yet — unmatched requests use Auto-CRUD / proxy / default 404" + `[+ New Rule]`. | `--text-muted` + primary btn |
| **Error (inline)** | Reuse inline alert `bg-red-900/50 border border-red-500 text-red-200` + retry affordance. | [existing — `login.html:15`] |
| **Error (endpoint not found)** | Centered `.card`, `--danger` heading, link back to `/`. | reuse `dashboard.html:128` |
| **Error (WS)** | Flows to the WS pill (§3.8) only — **never** a blocking modal (ux.md §3.6). | pill `--warn`/`--danger` |
| **Success** | Toast (reuse `.toast` green, `base.html:23`) + inline green alert on forms (`login.html:16`) + optimistic UI on toggles. | [existing] |
| **Disabled** | `opacity:0.5; cursor:not-allowed;` on buttons/toggles; Save disabled until valid (VC-15); deferred Webhook block visibly dimmed with helper. | VC-15 |
| **Live WS update (new row)** | New row prepends with the **arrival flash** (§6) + (if paused) increments the "N new" pill; older rows shift down. | flash §6; `--success`-tint |
| **Selected** | `--surface-2` fill + method-color accent rail (VC-3) + inspector loads. | §3.3 |

---

## 6. Motion & micro-interactions

Subtle, GPU-friendly (`opacity`/`transform`/`background` only — never layout-thrashing properties),
all wrapped in `prefers-reduced-motion` (§9). Two signature motions:

### 6.1 WS "Live" pulse (VC-4) — the liveness heartbeat
The connected dot **breathes** on a calm 2s loop (a soft scale+glow "ping", not a frantic blink):
```css
@keyframes hb-pulse {
  0%,100% { box-shadow: 0 0 0 0 rgba(63,185,80,0.45); }
  50%     { box-shadow: 0 0 0 5px rgba(63,185,80,0); }
}
.ws-dot--live { animation: hb-pulse var(--dur-pulse) var(--ease-standard) infinite; }
```
- **VC-4 testable:** the live dot is `--success` and animates `hb-pulse` on a ~2s cycle; "Reconnecting"
  is `--warn` with a slower opacity throb; "Offline" is `--danger` and **static** (no animation). Under
  `prefers-reduced-motion: reduce`, all three are static colored dots (state still legible by color+label).

### 6.2 Incoming-request arrival flash (VC-5) — "something just landed"
A new feed row fades in and its background flashes the success tint, decaying over `--dur-flash`:
```css
@keyframes hb-flash {
  from { background: var(--success-bg); }
  to   { background: transparent; }
}
.feed-row--new { animation: hb-flash var(--dur-flash) var(--ease-out); }
```
- The row also slides in subtly (`translateY(-4px)→0`, `--dur-base`). `request-stream.js` adds
  `.feed-row--new`, then removes it after the animation so re-selection doesn't re-flash.
- **Burst behavior (ties AC-30, ux.md §3.3):** when many rows arrive at once, `request-stream.js` batches
  via `requestAnimationFrame` so flashes don't thrash; the flash is the *only* per-row animation (no
  reflow), keeping the DOM responsive under flood.
- **VC-5 testable:** a newly-arrived row shows the `--success-bg` highlight that decays to transparent
  within ~900ms; with `prefers-reduced-motion: reduce` the row appears with **no** flash/slide (it just
  inserts) and the "N new" counter still updates — liveness never depends on motion alone.

### 6.3 Micro-interactions (all `--dur-fast`, reuse existing `transition` feel `base.html:11`)
- Buttons/toggles/tabs: color/background transition `--dur-fast`.
- Tree disclosure triangle: `transform: rotate(90deg)` on open, `--dur-fast`.
- Row select: accent-rail color + background ease in `--dur-base`.
- Modal/drawer: backdrop `opacity` fade `--dur-base`; panel `translateY(8px)→0` (modal) / `translateX`
  slide (drawer) `--dur-slow` `--ease-out`.
- Toast: reuse existing `opacity` fade `--dur-slow` (`base.html:23`).
- Copy button: brief check-glyph swap + `--link` flash on success (pairs with the existing "Copied!" toast).
- "Format JSON" / "Preview render": button shows in-flight spinner, result panel fades in.

---

## 7. Responsive

Desktop-first developer tool (ux.md §3.7), graceful degradation:
- **≥1024px:** side-by-side split — feed `w-[40%] min-w-[360px]`, inspector `flex-1` (ux.md §2.2). Endpoint
  bar shows all chips inline.
- **<1024px:** feed goes **full-width**; selecting a row slides the inspector in as a **full-screen drawer**
  (`z:drawer`, `--shadow-overlay`, slide `--dur-slow`) with a back button; the split becomes a stack.
- **<1024px endpoint bar:** mock-URL chips collapse behind a "URLs" disclosure; `[+ New Rule]`/`[Settings]`
  may collapse to icon buttons; the WS pill **always stays visible** (liveness is essential).
- **Modal/settings:** near-full-screen (`max-w-full max-h-full` insets) on small viewports; vertical tab
  rail collapses to a horizontal scroll strip.
- **Entry screen:** already responsive (`max-w-md mx-auto`, reuse `login.html:6`).
- **Reflow rules:** feed columns — on narrow widths, drop the `served-by` chip and latency first (keep
  method + path + status, the AC-28 essentials), then truncate path harder. Nothing load-bearing hides.

---

## 8. Visual accessibility

Extends ux.md §5; the design-agent's job is to make the palette and motion **verifiably** accessible.

### 8.1 Contrast (WCAG AA target; ratios vs the stated background) — VC-2
Computed against the surface each element sits on. **AA thresholds:** normal text ≥4.5:1, large/bold ≥3:1,
UI/graphical ≥3:1. Badge text is bold ≥12px → uses the **≥3:1** (large/UI) bar; status numbers are bold.

| Pair | Ratio | Bar | Pass |
| --- | --- | --- | --- |
| `--text` `#e6edf3` on `--bg` `#0d1117` | ~15.8:1 | 4.5 | ✅ |
| `--text` on `--surface` `#161b22` | ~14.0:1 | 4.5 | ✅ |
| `--text-muted` `#8b949e` on `--bg` | ~6.4:1 | 4.5 | ✅ (secondary) |
| `--text-muted` on `--surface` | ~5.7:1 | 4.5 | ✅ |
| `--text-faint` `#6e7681` on `--bg` | ~4.0:1 | 4.5 (text) / 3 (non-text) | ⚠️ **use for non-text/punctuation/disabled only, never body copy** (VC-2a) |
| `--link` `#58a6ff` on `--bg` | ~6.6:1 | 4.5 | ✅ |
| `--success` `#3fb950` text on `--surface` | ~6.1:1 | 4.5 | ✅ |
| `--warn` `#d29922` text on `--surface` | ~6.6:1 | 4.5 | ✅ |
| `--client-err` `#e3a008` text on `--surface` | ~7.4:1 | 4.5 | ✅ |
| `--danger` `#f85149` text on `--surface` | ~5.0:1 | 4.5 | ✅ |
| **Badge GET** white `#fff` on `#388bfd` | ~3.4:1 | 3 (bold/UI) | ✅ |
| **Badge POST** `#0d1117` on `#3fb950` | ~8.9:1 | 3 | ✅ |
| **Badge PUT** `#0d1117` on `#d29922` | ~9.1:1 | 3 | ✅ |
| **Badge PATCH** white on `#a371f7` | ~3.6:1 | 3 | ✅ |
| **Badge DELETE** white on `#f85149` | ~3.8:1 | 3 | ✅ |
| **Badge OPTIONS** `#8b949e` on `#30363d` | ~3.2:1 | 3 | ✅ (borderline — VC-2b: if QA measures <3:1 on final render, bump text to `#c9d1d9`) |
| **Badge HEAD** `#8b949e` on `#21262d` | ~4.2:1 | 3 | ✅ |
| **Badge ANY** white on `#6e7681` | ~3.9:1 | 3 | ✅ |
| WS dot colors on `--surface` (graphical) | success 6.1 / warn 6.6 / danger 5.0 | 3 | ✅ |

- **VC-2:** every method-badge text/fill pair and every status-code-text/surface pair meets **≥3:1**;
  body and secondary text meet **≥4.5:1**. (Ratios above are computed; QA verifies on the shipped render.)
- **VC-2a:** `--text-faint` is restricted to non-text decoration (tree punctuation, disabled glyphs,
  timestamps where also conveyed elsewhere) — never load-bearing copy. (Resolves ux.md §5's `#8b949e`
  caveat by promoting load-bearing secondary text to `--text-muted` ≥4.5:1.)
- **VC-2b:** `OPTIONS` badge contrast is borderline (~3.2:1); the fallback text `#c9d1d9` (which measures
  comfortably ≥4.5:1) is specified if the final rendered swatch dips below 3:1.

### 8.2 Focus-visible (VC-16)
Every interactive element (feed row, tab, button, toggle, chip, copy button, tree node) shows a visible
focus ring on keyboard focus: `outline: 2px solid var(--link); outline-offset: 2px;` (or the `--ring`
box-shadow on `--bg`). Inputs reuse the existing `focus:border-[#58a6ff]` (`login.html:22`) **plus** the
ring. Focus is never removed without a visible replacement. Modal traps focus; `Esc` restores it to the
trigger (ux.md §5; reuse `dashboard.html:258`).

### 8.3 Non-color signaling (VC-6)
Already structural in the palette: method badge + status carry **literal text**; the WS pill carries a
**text label**; the trace step list uses **distinct glyphs** (●/○/◆/✕) in addition to color; the new-row
flash is paired with insertion + the "N new" counter. No state is conveyed by hue alone.

### 8.4 Reduced motion (VC-17)
```css
@media (prefers-reduced-motion: reduce) {
  .ws-dot--live, .feed-row--new, [class*="hb-"] { animation: none !important; }
  * { transition-duration: 0.01ms !important; }
}
```
Disables the WS pulse, row flash/slide, skeleton shimmer, and tree/panel transitions; all states remain
fully legible by color + text + glyph (VC-4/VC-5 explicitly require this). Liveness for vestibular-sensitive
users is conveyed by the dot color + "Live" label + the still-updating row count.

---

## 9. Implementation notes

**Where the tokens live.** Add a `:root { … }` custom-property block to `base.html`'s `<style>`
(`base.html:8-25`) — or to `static/css/app.css` if the architect chooses the Tailwind CLI build (PRD §7).
The existing literal hex values (`.card #161b22`, buttons, etc.) can be **left as-is** (no churn) and the
new tokens added alongside; or refactored to `var(--…)` — FE's call, both are fine. The **only hard
requirement** is that the `[new]` keyframes (`hb-pulse`, `hb-flash`, skeleton shimmer), the
`prefers-reduced-motion` block, and the `--shadow-*`/`--ring` utilities exist in whichever CSS file ships.

```css
:root{
  --bg:#0d1117; --surface:#161b22; --surface-2:#21262d; --surface-3:#30363d;
  --border:#30363d; --border-strong:#3d444d; --row-divider:#21262d;
  --text:#e6edf3; --text-2:#c9d1d9; --text-muted:#8b949e; --text-faint:#6e7681; --link:#58a6ff;
  --success:#3fb950; --success-strong:#2ea043; --success-bg:rgba(63,185,80,.15);
  --info:#58a6ff; --info-bg:rgba(88,166,255,.15);
  --warn:#d29922; --warn-bg:rgba(210,153,34,.15); --client-err:#e3a008;
  --danger:#f85149; --danger-base:#da3633; --danger-bg:rgba(248,81,73,.15);
  --radius-sm:4px; --radius:6px; --radius-lg:8px;
  --shadow-overlay:0 16px 48px rgba(1,4,9,.85); --shadow-pop:0 8px 24px rgba(1,4,9,.7);
  --dur-fast:120ms; --dur-base:200ms; --dur-slow:300ms; --dur-flash:900ms; --dur-pulse:2000ms;
  --ease-out:cubic-bezier(.16,1,.3,1);
}
/* method badges — extends .badge geometry (base.html:17) */
.m-get{background:#388bfd;color:#fff}.m-post{background:#3fb950;color:#0d1117}
.m-put{background:#d29922;color:#0d1117}.m-patch{background:#a371f7;color:#fff}
.m-delete{background:#f85149;color:#fff}.m-options{background:#30363d;color:#8b949e}
.m-head{background:#21262d;color:#8b949e}.m-any{background:#6e7681;color:#fff}
/* keyframes from §6, reduced-motion block from §8.4 */
```

**Tailwind usage.** Arbitrary-value classes already used in the repo (`bg-[#0d1117]`, `border-[#30363d]`,
`text-[#58a6ff]`) continue to work on the CDN build; method/keyframe/shadow specifics go in `<style>` (or
`app.css`) since the Play CDN can't see a `tailwind.config`. The split shell is plain Tailwind flex
(`flex`, `flex-col`, `flex-1`, `w-[40%]`, `min-w-[360px]`, `overflow-y-auto`, `min-h-0`) — no new utilities.

**Template/class touch-points for `frontend-engineer`** (files from ux.md §1):
- `base.html` — token `:root` + keyframes + reduced-motion + `--shadow/ring`; de-emoji brand; slim-nav
  variant; `{% block fullbleed %}`.
- `index.html` (entry) — reuse `.card` + input + inline-alert classes verbatim.
- `dashboard.html` — split shell (`panel-flush` bars), WS pill (`.ws-dot--live`), feed container.
- `partials/feed_row.html` — `.feed-row`, method badge `.m-*`, accent rail, status color, served-by chip,
  `.feed-row--new`.
- `partials/inspector.html` + `inspector_body_tree.html` — underline tabs (reuse `index.html:21`), KV rows,
  syntax-tinted tree, trace step list.
- `partials/rule_modal.html` — modal shell (reuse `dashboard.html:15`), vertical tab rail + state dots,
  tag palette, throttling sliders (reuse `mock.html:131` delay input).
- `partials/endpoint_settings.html` — reuse toggle (`mock.html:108`), inputs, mock-URL chips, danger zone.

**Reuse vs new summary.** ~80% reuse: buttons, `.card`, inputs, select, textarea, toggle, inline alerts,
`pre/code`, toast, breadcrumb, underline tab, WS color intents — all verbatim from `base.html`/`login.html`/
`mock.html`/`dashboard.html`. **Net-new CSS is small:** the method-badge `.m-*` map, the two keyframes
(`hb-pulse`, `hb-flash`) + skeleton shimmer, the JSON-tree disclosure/tint, the feed accent rail, the
served-by chip, `panel-flush`, and the `--shadow-*`/`--ring` tokens. This is exactly the "minimal net-new
CSS" ux.md §6 anticipated.

---

## 10. UX handoff notes

1. **Status color extends to a 4-class map (2xx/3xx/4xx/5xx).** ux.md shows method+status text; I add
   distinct hues for 3xx (amber) and 4xx (`#e3a008`) vs 5xx (red). This is additive (text still literal),
   but if the architect's `served_by`/status surfacing differs, the hue mapping should be confirmed. No
   structural change to ux.md.
2. **`served_by` chip is outline (not filled).** To keep one filled badge per row (the method), I render
   served-by as an outline chip. This is a visual refinement of ux.md §3.4's served-by label; labels/enum
   remain `[ARCH-GAP: OQ-10]` and unchanged.
3. **`--text-faint` is non-text only (VC-2a).** ux.md §5 flagged that `#8b949e` on `#0d1117` is borderline;
   I resolve it by keeping all **load-bearing** secondary text at `--text-muted` (≥4.5:1) and introducing a
   dimmer `--text-faint` strictly for decoration/disabled. No copy moves; just which token paints it.
4. **OPTIONS/HEAD badges are intentionally desaturated.** Because Auto-CORS generates frequent `OPTIONS`
   preflights (AC-18), coloring them loudly would swamp the feed. ux.md doesn't specify per-method weight;
   I'm setting visual priority. If product wants preflights visually suppressed/grouped, that's a UX call to
   add (see PRD gap 5).
5. **Mock-URL chips are rendered neutral, never link-blue.** Reinforces ux.md §2.0's "dashboard origin ≠
   mock origin." No behavior change (still copy-only, non-anchor) — purely a color decision.
6. **No new shadows on flat cards.** I confine the new `--shadow-*` to floating layers only, preserving the
   existing flat-bordered card aesthetic (`base.html:10`). Consistent with ux.md §6's "reuse the whole
   visual system."

---

## 11. PRD gaps (numbered — visual ACs/decisions the PM must add or clarify; feeds OQ-15)

1. **Adopt the method-badge palette as ACs (resolves the AC-28 design hook).** AC-28 (`prd.md:91`) says
   "color-coded method badge" and explicitly defers the palette to design REVISE. PM should lift **VC-1**
   (the §2.4 swatch map: GET blue / POST green / PUT amber / PATCH purple / DELETE red / OPTIONS·HEAD·ANY
   neutral, literal text always shown) and **VC-2** (per-pair ≥3:1 contrast) into PRD §4 / OQ-15.
2. **Adopt the status-code color map (VC-9).** Add an AC that 2xx/3xx/4xx/5xx render in distinct hues
   (green/amber/`#e3a008`/red) with the literal status number — so AC-28's "served status code" is testable
   for color-blind safety, not just presence.
3. **Adopt motion specs as ACs (VC-4, VC-5, VC-17).** AC-29/AC-30 (`prd.md:92-93`) need testable motion:
   (a) the connected WS dot pulses on a ~2s `--success` cycle, reconnecting=amber throb, offline=static red;
   (b) a new feed row flashes `--success-bg` decaying ≤~900ms; (c) under `prefers-reduced-motion` all of
   (a)+(b) are static while remaining legible. Promote to §4 ACs so QA can verify the "pulsing indicator"
   (prompt §2.2) and "without locking the DOM" (AC-30) intent.
4. **Adopt contrast + focus + reduced-motion as ACs (VC-2, VC-16, VC-17).** ux.md §7 item 18 asks to promote
   AA contrast, focus-visible, and reduced-motion to ACs; this doc supplies the concrete, measurable targets
   (§8). PM should turn each into a QA-checkable AC and close OQ-15 with them.
5. **Confirm whether `OPTIONS`/preflight rows are visually de-emphasized or filterable.** I desaturate
   `OPTIONS`/`HEAD` badges (design call, §2.4) so CORS chatter doesn't swamp the feed. PM/UX should confirm
   this is desired, or specify a stronger treatment (e.g. a default "hide preflights" filter chip) — it
   affects how dense the feed reads in practice (ties AC-18/AC-28).
6. **Freeze the `served_by` enum + chip labels (already OQ-10).** The served-by chip's **colors** map by
   intent (matched/CRUD/proxied/default/chaos/rate-limited) regardless of labels, but the **label strings**
   and enum are still `[ARCH-GAP: OQ-10]` (`prd.md:227`). PM should ensure OQ-10 resolution includes the
   exact chip labels so VC-8 is testable.
7. **Confirm a min/target dashboard viewport (density budget).** The feed's high-density `text-sm`/`text-xs`
   register (VC-12) assumes a desktop tool; ux.md §3.7 sets the 1024px split breakpoint. PM should state a
   supported minimum width / target so QA can verify the dense feed at the intended size rather than guessing.
