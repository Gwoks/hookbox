# Visual design: HookBox — Rust/Axum re-platform (Vite + React + TS SPA)

- **Slug:** `hookbox-rust-replatform`
- **Status:** Draft (design-agent, DESIGN phase)
- **Author:** Visual designer (multi-agent pipeline)
- **Upstream contract (read first):** `docs/features/hookbox-rust-replatform/ux.md` (structure, IA, components, states, a11y — my starting contract), `docs/features/hookbox-rust-replatform/prd.md`, locked spec `docs/superpowers/specs/2026-06-21-hookbox-rust-replatform-design.md` (§10 frontend FROZEN).
- **Design-system precedent (token *approach* mirrored, values NEW):** `../shortener-link/docs/features/link-shortener/DESIGN.md §2`, `../shortener-link/src/globals.css`, `../shortener-link/tailwind.config.ts`, `../shortener-link/src/components/ui/button.tsx`, `.../status-badge.tsx`.
- **Anti-pattern being fixed:** the current Jinja templates hardcode hex inline (`#0d1117`, `#30363d`, `#58a6ff`, `#238636`, `#161b22` — verified at `templates/base.html` lines 27–44, 124–127). The SPA uses **semantic Tailwind tokens only**; no hex in components. This doc is the single source of truth for those tokens.

> **How to read this doc.** ux.md owns *what* each surface is and how it behaves; this doc owns *how it looks*. Every token here wires as a CSS custom property → Tailwind theme color, exactly as the reference does (`var(--bg-surface)` → `bg-surface`). `frontend-engineer` drops §2 into `src/globals.css` + `tailwind.config.ts` and builds the components in §3 against those tokens. Every load-bearing visual choice ends as a numbered, testable AC in §11 for the PM.

---

## 1. Design direction

**HookBox is a precision instrument that feels weightless.** Where the old dashboard was a heavy GitHub-dark panel of boxed cards and saturated chrome, the new identity is **bright paper + ink in light, deep graphite in dark**, with a single confident **teal accent**, hairline borders instead of filled boxes, and *monospace as texture* — every URL, header, token, JSON value, and trace step renders in mono, which is what makes HookBox read as an HTTP instrument rather than a SaaS template. Three words: **fresh, instrument-grade, quiet.**

It mirrors the reference's *system mechanics* (semantic tokens, `.dark` class, CVA variants, Radix, never-color-alone, four-states-everywhere) but is a **distinct product**: the reference is indigo + sans-data-table; HookBox is **teal + mono-data-feed**, with a denser, log-viewer rhythm on the left and an airy inspector on the right. It is **dark-first by identity** (a debugger you live in) but the PRD mandate is "fresh, beautiful, light," so **light is the first-paint default for new visitors** unless `prefers-color-scheme: dark` (see §11 AC-D24 and the UX handoff on ux.md gap #10), and both themes are first-class AA.

Distinct-from-old signals (verifiable): teal accent (not GitHub blue `#58a6ff`), warm-paper canvas in light (not pure white, not `#0d1117`), hairline 1px borders carrying structure (not filled `#161b22` cards with `#30363d` borders), method/served-by/status as a coordinated chip *system* rather than one blue `.badge-method`.

---

## 2. Design tokens

Wired exactly like the reference: primitives → semantic CSS vars in `:root` (light) and `.dark` (dark) in `src/globals.css`, surfaced as Tailwind colors in `tailwind.config.ts`. **All values below are [new — proposed]**; the *mechanism* (var→Tailwind, `.dark` strategy, the token names `bg-surface`/`text-secondary`/`accent`/`success-fg`…) is **[existing — verified at `../shortener-link/src/globals.css` + `tailwind.config.ts`]** and reused verbatim so frontend-engineer's wiring is identical to the reference.

### 2.1 Primitive palette (raw scales) — [new]

Neutral is a **slightly warm slate** (gives "paper," not clinical white, and avoids the cold GitHub-dark feel). Accent is **teal** (developer-tool, distinct from the reference indigo and the old blue). Method colors are a 7-hue coordinated set; status follows class.

```
/* Neutral — warm slate */
--slate-0:#ffffff  --slate-25:#fbfcfc  --slate-50:#f7f9f9  --slate-100:#eef1f2
--slate-200:#e0e5e7  --slate-300:#cbd2d6  --slate-400:#97a2a8  --slate-500:#697077
--slate-600:#4b5258  --slate-700:#343a40  --slate-800:#23282d  --slate-850:#1a1e22
--slate-900:#13171a  --slate-950:#0d1012

/* Accent — Teal */
--teal-50:#e7f8f5  --teal-100:#c6efe8  --teal-200:#8fe0d3  --teal-300:#4fc9b6
--teal-400:#1fae99  --teal-500:#0d9488   /* primary accent (light) */
--teal-600:#0b7d73   /* primary hover (light) */
--teal-700:#0a655e
--teal-300-dark:#3fd6bf  /* accent on dark (lifted ≥4.5:1 fg use, AA) */
--teal-400-dark:#22c3ab  /* accent fill on dark */

/* Status / semantic hues (also used by StatusCode classes) */
--green-50:#e8f7ee --green-100:#cbecd7 --green-500:#16a34a --green-600:#15803d --green-400-dark:#4ade80   /* 2xx / success */
--sky-50:#e7f3fb   --sky-100:#c9e4f7  --sky-500:#0284c7  --sky-600:#0369a1  --sky-400-dark:#56c2f5      /* 3xx / info */
--amber-50:#fdf3e2 --amber-100:#fae3bd --amber-500:#d97706 --amber-600:#b45309 --amber-400-dark:#fbbf24  /* 4xx / warning */
--red-50:#fdecec   --red-100:#fbd5d5  --red-500:#dc2626  --red-600:#b91c1c  --red-400-dark:#f87171      /* 5xx / danger */
--violet-50:#f1ecfd --violet-500:#7c3aed --violet-600:#6d28d9 --violet-400-dark:#a78bfa                  /* method PATCH / chip accent */
--rose-500:#e11d48 --rose-600:#be123c --rose-400-dark:#fb7185                                            /* method DELETE */
--orange-500:#ea580c --orange-600:#c2410c --orange-400-dark:#fb923c                                      /* method POST */
```

### 2.2 Semantic tokens (light / dark) — [new values; token names existing]

| Semantic token | Light | Dark | Use |
|---|---|---|---|
| `--bg-canvas` | `--slate-50` | `--slate-950` | App background behind surfaces |
| `--bg-surface` | `--slate-0` | `--slate-900` | Cards, panes, inspector, dialogs, top bar |
| `--bg-surface-raised` | `--slate-0` | `--slate-850` | Menus, popovers, toasts (float above surface) |
| `--bg-subtle` | `--slate-50` | `--slate-850` | Feed header, input wells, hovered/zebra rows, tab strip |
| `--bg-hover` | `--slate-100` | `--slate-800` | Row/list/button hover |
| `--bg-active` | `--slate-200` | `--slate-700` | Pressed / selected fill |
| `--border-default` | `--slate-200` | `--slate-700` | Hairline borders, row separators, dividers |
| `--border-strong` | `--slate-300` | `--slate-600` | Input borders, split-pane handle, emphasized edges |
| `--text-primary` | `--slate-900` | `--slate-50` | Headings, primary body, method/status digits |
| `--text-secondary` | `--slate-600` | `--slate-300` | Body, labels, secondary feed columns |
| `--text-tertiary` | `--slate-500` | `--slate-400` | Meta, relative time, placeholders, captions |
| `--text-on-accent` | `--slate-0` | `--slate-950` | Text on filled accent button |
| `--accent` | `--teal-500` | `--teal-300-dark` | Primary actions, focus ring, selected-row rail, links-that-are-actions |
| `--accent-hover` | `--teal-600` | `--teal-200` | Hover for accent |
| `--accent-fill` | `--teal-500` | `--teal-400-dark` | Filled accent button bg (dark uses 400 for body-text contrast) |
| `--accent-subtle-bg` | `--teal-50` | `rgba(63,214,191,.14)` | Selected tab, "N new" pill, accent chips |
| `--focus-ring` | `--teal-500` | `--teal-300-dark` | 2px focus-visible ring (see §9) |
| `--success-fg` / `--success-bg` | `--green-600` / `--green-50` | `--green-400-dark` / `rgba(74,222,128,.14)` | 2xx, live pill, success toast |
| `--info-fg` / `--info-bg` | `--sky-600` / `--sky-50` | `--sky-400-dark` / `rgba(86,194,245,.14)` | 3xx, info, sse-fallback pill |
| `--warning-fg` / `--warning-bg` | `--amber-600` / `--amber-50` | `--amber-400-dark` / `rgba(251,191,36,.14)` | 4xx, reconnecting pill, byte-cap warning |
| `--danger-fg` / `--danger-bg` | `--red-600` / `--red-50` | `--red-400-dark` / `rgba(248,113,113,.14)` | 5xx, offline pill, destructive, field errors |
| `--neutral-chip-fg` / `--neutral-chip-bg` | `--slate-600` / `--slate-100` | `--slate-200` / `--slate-800` | `default`/`echo` served-by, redacted note, disabled rule |
| `--overlay-scrim` | `rgba(13,17,20,.45)` | `rgba(0,0,0,.62)` | Dialog backdrop |

**Method colors** (own semantic set so `MethodBadge` never hardcodes hex; fg/bg pair both AA — see §9):

| Token | Light fg / bg | Dark fg / bg | Method |
|---|---|---|---|
| `--m-get-fg/bg` | `--sky-600` / `--sky-50` | `--sky-400-dark` / `rgba(86,194,245,.14)` | GET |
| `--m-post-fg/bg` | `--green-600` / `--green-50` | `--green-400-dark` / `rgba(74,222,128,.14)` | POST |
| `--m-put-fg/bg` | `--orange-600` / `--amber-50` | `--orange-400-dark` / `rgba(251,146,60,.14)` | PUT |
| `--m-patch-fg/bg` | `--violet-600` / `--violet-50` | `--violet-400-dark` / `rgba(167,139,250,.14)` | PATCH |
| `--m-delete-fg/bg` | `--rose-600` / `--red-50` | `--rose-400-dark` / `rgba(251,113,133,.14)` | DELETE |
| `--m-head-fg/bg` | `--slate-600` / `--slate-100` | `--slate-200` / `--slate-800` | HEAD / OPTIONS / ANY |

### 2.3 Served-by chip palette (8 values, AC-56) — [new]

Color-blind-safe set; **every chip = icon + text** (§3.4), so hue is secondary. Maps the `served_by` union (`rule|crud|mitm|tunnel|default|cors|chaos|ratelimit`):

| served_by | Icon (lucide) | Token pair | Rationale |
|---|---|---|---|
| `rule` | `git-branch` | `--accent` / `--accent-subtle-bg` | the "you authored this" path → brand teal |
| `crud` | `database` | `--info-fg/bg` | data store |
| `mitm` | `arrow-left-right` | `--violet-600/50` (dark `--violet-400-dark`/.14) | proxy hop |
| `tunnel` | `radio-tower` | `--success-fg/bg` | live link to your machine |
| `default` | `circle-dashed` | `--neutral-chip-fg/bg` | fell through |
| `cors` | `shield-check` | `--info-fg/bg` (outline variant) | preflight |
| `chaos` | `zap` | `--danger-fg/bg` | injected failure |
| `ratelimit` | `gauge` | `--warning-fg/bg` | throttled |

### 2.4 Typography — [new scale; mono-forward]

System stacks (no CDN, offline-safe, parity with ux.md §0). HookBox leans monospace harder than the reference — mono is the product's texture.

```
--font-sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
--font-mono: ui-monospace, "SF Mono", "JetBrains Mono", "Cascadia Code", Menlo, Consolas, monospace;
```

| Token | Size / line-height | Weight | Use |
|---|---|---|---|
| `--text-display` | 44px / 1.05 | 700, -0.02em | Landing hero headline only |
| `--text-h1` | 30px / 1.2 | 700 | Rare page titles |
| `--text-h2` | 22px / 1.25 | 650 | Dialog titles, landing section heads |
| `--text-h3` | 18px / 1.3 | 600 | Card / settings-section titles, inspector subject |
| `--text-h4` | 15px / 1.4 | 600 | Sub-section, group labels |
| `--text-body` | 15px / 1.5 | 400 | Default body |
| `--text-body-sm` | 13px / 1.45 | 400 | Helper text, secondary feed columns, table cells |
| `--text-caption` | 12px / 1.4 | 500 | Meta, relative time, chip labels |
| `--text-overline` | 11px / 1.3 | 600, +0.06em, uppercase | Section labels, stat captions, tab-strip eyebrows |
| `--text-mono` | 13px / 1.5 | 450, tabular-nums | URLs, headers, paths, tokens, trace steps |
| `--text-mono-sm` | 12px / 1.45 | 450, tabular-nums | Dense feed path column, JsonTree nodes |
| `--text-mono-lg` | 16px / 1.4 | 500 | CodeBlock (mock URL, curl, CLI command), inspector path |

`StatusCode` digits and `latency Nms` use `font-variant-numeric: tabular-nums` so columns align vertically as the feed streams.

### 2.5 Spacing (4px base) — [new, matches reference rhythm]

```
--space-0:0 --space-1:4 --space-2:8 --space-3:12 --space-4:16 --space-5:20
--space-6:24 --space-8:32 --space-10:40 --space-12:48 --space-16:64 --space-20:80
```
Density: **feed rows are dense** (vertical padding `--space-2`, row height ~40px — log-viewer cadence); inspector + settings are **airy** (section padding `--space-6`, group rhythm `--space-8`). Inputs/buttons: `--space-3` v / `--space-4` h. Chips: `--space-1` v / `--space-2` h.

### 2.6 Radii — [new]

```
--radius-xs:4px   (method/status/served-by chips, KV rows)
--radius-sm:6px   (buttons, inputs, menu items, sliders track)
--radius-md:10px  (cards, dialogs, popovers, CodeBlock, JsonTree container)
--radius-lg:14px  (landing hero panel, feature cards)
--radius-pill:9999px (ConnectionPill, "N new" pill, toggles)
--radius-full:50% (icon buttons, account avatar dot)
```

### 2.7 Elevation / shadow — [new, hairline-first]

Structure comes from hairline borders; shadows only for genuinely floating layers. Dark reduces opacity.

```
--shadow-xs: 0 1px 2px rgba(13,17,20,.05)
--shadow-sm: 0 1px 3px rgba(13,17,20,.08), 0 1px 2px rgba(13,17,20,.04)
--shadow-md: 0 4px 14px rgba(13,17,20,.10), 0 2px 4px rgba(13,17,20,.06)   /* menus, popovers, toasts */
--shadow-lg: 0 16px 40px rgba(13,17,20,.18), 0 4px 10px rgba(13,17,20,.06) /* dialogs, mobile sheets */
--shadow-focus: 0 0 0 2px var(--bg-surface), 0 0 0 4px var(--focus-ring)   /* offset ring on dense surfaces */
/* dark: same geometry, rgba(0,0,0,.4–.62) */
```

### 2.8 Motion & z-index — [new timings; reference scale]

```
--ease-standard: cubic-bezier(.2,0,0,1)
--ease-emphasized: cubic-bezier(.2,.7,0,1)   /* feed-row arrival */
--dur-instant:90ms --dur-fast:120ms --dur-base:180ms --dur-slow:240ms
/* prefers-reduced-motion: ALL durations -> ~0ms; feed-row arrival becomes instant insert;
   skeleton shimmer -> static block; ConnectionPill never relies on spin (text carries state). */

--z-base:0 --z-sticky:100 --z-nav:200 --z-dropdown:1000 --z-popover:1100
--z-dialog:1200 --z-toast:1300 --z-tooltip:1400
```

### 2.9 Layout tokens — [new, endpoint-scoped shell]

```
--header-h:52px                 /* slim sticky top bar (ux.md §1.2), tighter than ref 56 */
--subheader-h:48px              /* dashboard sub-header (endpoint actions) */
--feed-min-w:360px  --feed-pct:40%      /* SplitPane LEFT */
--splitter-w:6px                /* drag handle hit area */
--landing-max-w:560px           /* email-gate column */
--landing-hero-max-w:1040px     /* hero + feature grid */
--bp-sm:640px --bp-md:768px --bp-lg:1024px --bp-xl:1280px
```

---

## 3. Component styling

Per component: the visual spec, whether it reuses a reference `ui/` primitive (re-themed by tokens — **no structural change**) or is a new HookBox primitive, and the template/source it maps to. CVA variant pattern is **[existing — verified at `../shortener-link/src/components/ui/button.tsx`]**.

### 3.0 Reference `ui/` primitives — themed for HookBox
These adopt directly (ux.md §3.1); HookBox only supplies token *values* (§2). No hex enters them.
- **`Button`** — `primary` = `bg-accent-fill text-on-accent` (teal); `secondary` = `border-border-strong bg-surface`; `ghost`; `danger` = `bg-danger-fg text-white`; `link` = `text-accent`. The single accent (primary) button per surface is **+ New Rule** on the dashboard sub-header and **Save** in dialogs. Loading preserves width + `aria-busy` (verbatim from the reference Button).
- **`Input`/`Textarea`** — `mono` flag used heavily (path, target_url, body_template); error = `border-danger-fg` + message + `aria-invalid`.
- **`Switch`** — on = `bg-accent-fill`; used for Auto-CRUD, CORS, rule `enabled`.
- **`Dialog`** — `bg-surface`, `--radius-md`, `--shadow-lg`, scrim; mobile → full-screen sheet (`animation: sheet-in`).
- **`DropdownMenu` / `Tooltip` / `Toast` / `Skeleton` / `Spinner` / `Segmented` / `CopyButton` / `AppShell` / `ThemeToggle`** — re-themed only.

### 3.1 `MethodBadge` (new) — feed rows + inspector header
Map `templates/base.html .badge-method` (the single blue badge being replaced). CVA variant keyed by method; `--radius-xs`, `--text-overline` (uppercase mono, +0.06em), `--space-1`/`--space-2` padding, fg/bg from §2.2 method tokens. **Text label is the source of truth** (grayscale-legible); color is reinforcement. Fixed min-width so the feed's leading column aligns as rows stream.

### 3.2 `StatusCode` (new)
Tabular-figure digits (`--text-mono`, `tabular-nums`), colored by class: 2xx→`success-fg`, 3xx→`info-fg`, 4xx→`warning-fg`, 5xx→`danger-fg`. A **2px leading underline/dot in the class color** is the non-hue signal; the digits themselves stay full-contrast `text-primary`-weight so they're legible even when desaturated (never rely on the class hue alone — AC-D14).

### 3.3 `ConnectionPill` (new) — WS/SSE health (AC-41/43)
`--radius-pill`, icon + colored dot + text label; `role="status" aria-live="polite"`. States: `connecting` (`text-tertiary`, `loader` — under reduced-motion a static dot + "Connecting"), `live` (`success-fg`, `wifi`, steady dot), `reconnecting` (`warning-fg`, `rotate-cw`), `sse` (`info-fg`, `radio`, "Live (SSE)"), `offline` (`danger-fg`, `wifi-off`). **Text label carries the state** so the spin is never the sole cue (AC-D17).

### 3.4 `ServedByChip` (new, AC-56)
`--radius-xs` chip, `lucide` icon + text, fg/bg from §2.3. Same chip used in feed `FeedRow` and inspector subject strip — "one subject, one color."

### 3.5 `FeedRow` (new) — the signature row
Maps `templates/dashboard.html` feed grid. A focusable `role="option"` grid: `[MethodBadge] [path mono-sm truncate] [StatusCode] [ServedByChip] [latency tnum] [relative time tertiary]`. Row height ~40px, hairline `border-default` separator, hover `bg-hover`. **Selected** = `bg-active` + a **3px leading accent rail** (`--accent`) + `aria-selected` — a non-color marker (the rail + fill), not hue alone (AC-D13). New-row arrival animation in §5.

### 3.6 `JsonTree` (new) — inspector Body / Response (AC-44)
`--radius-md` container, `--bg-subtle`, `--text-mono-sm`. Keys `text-secondary`, strings `success-fg`-tinted, numbers/booleans `info-fg`-tinted, null/punctuation `text-tertiary` (syntax tint is decorative — never the only signal; the JSON structure + indent carries meaning). Disclosure triangles are focusable. Pretty/Raw via `Segmented`; Expand/Collapse-all + Copy in a quiet toolbar. **Every value a text node** (XSS-inert, ux.md §6) — visual spec must not introduce `dangerouslySetInnerHTML`.

### 3.7 `CodeBlock` (new) — mock URL, curl sample, CLI command
`--radius-md`, `--bg-subtle`, `--text-mono-lg`, 1px `border-default`, integrated `CopyButton` top-right. **Mock-URL chips are copy-only, never link-blue** (ux.md §2.2) — they use `text-primary` mono, not `--accent`, so users don't expect navigation.

### 3.8 `KeyValueRows` (new) — headers/query/state
Two-column hairline-separated rows: key (`text-mono-sm`, `text-secondary`) · value (`text-mono-sm`, `text-primary`, wrap). Per-row `CopyButton` (icon-sm `ghost`, appears on row hover/focus). A **redacted** value renders as a `--neutral-chip` "redacted" pill, not raw text (AC-61 surfacing). Edit mode (rule builder) adds add/remove icon-buttons.

### 3.9 `SplitPane` (new)
LEFT `--feed-pct` / min `--feed-min-w`, RIGHT `flex-1`. Divider is a `--splitter-w` hit area with a centered 1px `border-strong` hairline; on hover/focus the hairline thickens to 2px `--accent`. Below `--bp-md` it stacks (§7).

### 3.10 Rule-builder & settings dialog / `Tabs` (new wrapper)
Dialog header (`--text-h2` title + `icon` close), body = Radix `Tabs`, footer right-aligned (Cancel `ghost` · Save `primary`). **Tab strip:** `--bg-subtle` underline strip; selected tab = `text-primary` + 2px `--accent` underline bar (the bar is the non-color selection signal); inactive = `text-tertiary`. The five-tab inventory (Matching/Response/Templating/Actions/Throttling and the inspector's five) shares this wrapper. Templating-tab insert chips use `--accent-subtle-bg`; the `webhook_action` "stored, not yet sent" note renders as a `--warning-bg` `InlineAlert` (ux.md gap #3).

### 3.11 `Slider` (new) — latency / chaos / rate
Track `--bg-active` `--radius-sm`, filled portion `--accent-fill`, thumb `--radius-full` `bg-surface` + `border-strong` (focus ring on thumb). **Always paired with a number input** (keyboard + exact entry, ux.md §3.2). A `--text-caption` value readout sits inline.

### 3.12 `InlineAlert` (new)
Left-rail-accented panel, `--radius-md`, icon + message (+ optional action). Variants info/`--info`, warn/`--warning`, danger/`--danger` — fg for icon + heading, soft bg fill. Persistent (rate-limit, `429` with Retry-After, endpoint-gone, storage-unavailable).

---

## 4. Visual hierarchy & layout

- **Dashboard (the product):** the eye lands LEFT on the streaming feed (dense, mono, high-frequency motion), then RIGHT into the airy inspector when a row is selected. The split is asymmetric on purpose — the feed is the "now," the inspector is the "deep dive." The single accent button (**+ New Rule**) is the only filled-teal element on the sub-header, so the primary action is unambiguous.
- **Emphasis ladder:** filled teal (one primary action) → `text-primary` mono data → colored chips (method/status/served-by, low-saturation) → hairline structure → `text-tertiary` meta. Color saturation is *rationed* so the streaming method/status hues read as signal, not noise.
- **Density:** feed dense (40px rows); inspector/settings airy (`--space-6`/`--space-8`). This contrast is itself the hierarchy.
- **Alignment:** feed columns are a fixed grid with tabular numerics so the column edges stay rock-steady as rows prepend — critical for scannability under live updates (maps ux.md §2.2 feed grid).

---

## 5. Visual states

One row per state ux.md defines (ux.md §2.x, §3.3, §4). Async-four-states are first-class.

| Surface / element | State | Visual treatment |
|---|---|---|
| Any interactive (Button/Input/chip/row) | default | tokenized rest state |
| ″ | hover | `bg-hover`; buttons → `accent-hover`/`bg-hover`; row → `bg-hover` |
| ″ | focus-visible | 2px `--focus-ring` ring, 2px offset (`--shadow-focus` on dense surfaces), never removed (AC-D15) |
| ″ | active/pressed | `bg-active`; filled buttons darken one step |
| ″ | disabled | `text-tertiary` on `bg-subtle`, no shadow, `cursor-not-allowed` |
| Button / Save | loading | leading `Spinner`, label retained, width preserved, `aria-busy` |
| Feed list | loading | 6–8 skeleton `FeedRow`s shaped like the real grid; static block under reduced-motion |
| Feed list | empty (first-call funnel, AC) | muted `inbox` glyph, "No requests yet," copy-only mock-URL `CodeBlock`, static `curl <mock_url>/ping` sample (never executed) |
| Feed row | selected | `bg-active` + 3px leading `--accent` rail + `aria-selected` (non-color marker) |
| Feed row | new arrival | see §6 (accent-rail flash + slide; instant under reduced-motion) |
| Inspector | empty | centered `--text-secondary` "Select a request on the left to inspect it." |
| Inspector | pending (AC-59) | `--info` `InlineAlert` "Detail still being written…" + Retry; never a 404 |
| Inspector | error / unauthorized | `--danger` `InlineAlert` `role="alert"` + Retry |
| Dashboard pane | endpoint not found / gone (AC-57) | full-pane centered state, `unplug` glyph, message + "Back to start" |
| `ConnectionPill` | connecting/live/reconnecting/sse/offline | per §3.3 (icon + dot + text, color reinforces) |
| "N new" pill | buffered while paused | `--accent-subtle-bg` pill, `--text-caption`, count; pulses once on increment (reduced-motion: no pulse) |
| Rules manager | empty | "No rules yet — unmatched requests use Auto-CRUD / proxy / default" + "+ New Rule" |
| Form field | error | `border-danger-fg`, `--danger-fg` message below, `aria-invalid` + `aria-describedby` (text, not color-only) |
| Form / toggle | success | optimistic toggle settles; success `Toast` (`success-fg` icon); never sole confirmation |
| Byte counter (body_template) | approaching cap (AC-18) | `--text-caption` turns `--warning-fg` near 256 KB; over → `--danger-fg` |

---

## 6. Motion & micro-interactions

- **Feed new-row arrival (the defining interaction):** prepend with a `--dur-base` `--ease-emphasized` entrance — a brief left-rail flash in `--accent` (~600ms fade to transparent) + an 8px slide-down-into-place. The flash is the "something just happened" cue without yanking the eye. **Under `prefers-reduced-motion`: instant insert, no flash, no slide** (ux.md §4/§6, AC-D16).
- **Selection:** inspector content cross-fades `--dur-fast`; the subject strip's served-by/method color swaps instantly (no color tween — avoids muddy mid-states).
- **ConnectionPill transitions:** dot color crossfades `--dur-base`; `reconnecting` icon spins (reduced-motion: static, text carries state).
- **`state_changed`/`endpoint_updated`:** a single subtle `--accent-subtle-bg` pulse on the affected region (`--dur-slow`), no toast (ux.md §4, gap #8). Reduced-motion: no pulse.
- **Copy:** button → check + "Copied" `--dur-fast`, reverts ~1.6s, plus `Toast` + `sr-only aria-live` (reference `CopyButton` verbatim).
- **Dialog/sheet:** `content-in` (desktop) / `sheet-in` (mobile) from the reference keyframes.
- All durations ≤ `--dur-slow`; transitions are `transform`/`opacity`/`background-color` only (GPU-friendly, no layout thrash on the streaming feed).

---

## 7. Responsive

- **≥`--bp-md`:** SplitPane side-by-side (feed `--feed-pct`, inspector flex). Top bar + sub-header sticky.
- **<`--bp-md`:** pane **stacks** → feed full-width; selecting a row slides the inspector over it with a sticky "← Feed" back bar (ux.md §2.2 mobile parity). Sub-header endpoint actions (Rules / +New Rule / Settings / Auto-CRUD) collapse into the drawer/overflow menu; endpoint switcher moves into the drawer.
- **Rule builder + settings** become full-screen sheets (`sheet-in`).
- **Long mono strings** truncate with ellipsis, reveal full on tap/focus; **never horizontal-scroll**; tap targets ≥44×44px.
- **Landing** hero + feature grid: 3-col grid → 1-col stack <`--bp-md`; email input + button stack <`--bp-sm` (button full-width).

---

## 8. The landing page — visual direction (first impression: fresh + light)

Public `/` (ux.md §2.1). Default theme is **light** here for max "fresh & beautiful" first impression (re-resolves to user pref after `prefers-color-scheme`). Centered, `--landing-hero-max-w`, on `--bg-canvas` (warm paper).

- **Brand lockup:** inline-SVG **hook-mark** (a minimal hook glyph in `--accent` teal) + "HookBox" wordmark (`--text-h2`, `text-primary`). No external asset (offline-safe; replaces the old plain blue text brand at `base.html:127`).
- **Hero:** `--text-display` headline (copywriter owns words), `--text-body` `text-secondary` subhead, then the **email gate**: one large `Input lg` (mono placeholder hints it's a developer tool) + `Button lg primary` (teal). A `--text-caption` `text-tertiary` helper line under it. The hero panel sits on a subtle `--bg-surface` card with `--radius-lg` and `--shadow-sm` so it floats lightly above the canvas.
- **Decorative texture (light, performant):** a faint hairline grid or a single soft teal radial glow behind the hero (`--accent` at ~6% opacity), `aria-hidden`, removed under reduced-motion. This is the "beautiful & light" signature — no heavy gradients, no images.
- **3-up feature grid (Mock · Intercept · Inspect):** three `--radius-md` cards, hairline border, a `lucide` glyph in `--accent`, `--text-h4` title + `--text-body-sm` line. `aria-hidden` presentational per ux.md (design may keep — recommend keeping; it's the lightness/credibility payload).
- **Quiet `/cli` link** (`Button link`, `--accent`).
- **States:** submitting (button loading, input disabled), validation (`--danger` inline error), `429` (`--warning` `InlineAlert` w/ Retry-After), storage-unavailable (`--info` `InlineAlert`). Anti-enumeration: identical visuals for new vs existing email (AC-D22).

---

## 9. Visual accessibility (WCAG 2.1 AA, both themes)

Contrast targets (frontend-engineer verifies final rendered values; pairs chosen to clear AA):
- `--text-primary` on `--bg-surface`: ≥ 13:1 (L: `#13171a` on `#ffffff` ≈ 16.8:1; D: `#f7f9f9` on `#13171a` ≈ 16:1).
- `--text-secondary` on `--bg-surface`: ≥ 5.5:1 both themes.
- `--text-tertiary` on `--bg-surface`: ≥ 4.5:1 (never used for essential text <16px below that).
- `--text-on-accent` on `--accent-fill`: ≥ 4.5:1 — **L** white on `--teal-500 #0d9488` ≈ 4.5:1; **D** `--slate-950` on `--teal-400-dark #22c3ab` ≈ 9:1 (dark uses the lighter 400 fill *with dark ink* to clear AA — verify the light-theme pair at build; if `#0d9488` lands <4.5:1 for the rendered weight, darken to `--teal-600` for the fill).
- **Every method/status/served-by chip `fg` on its `bg` ≥ 4.5:1** in both themes (the `*-fg` is the high-contrast ink, the soft `*-bg` is the fill — the pair is the deliverable, same rule as the reference).
- Borders / UI boundaries (`--border-default`, split handle, focus ring) ≥ 3:1 against adjacent fill.

Non-negotiables:
- **Never color alone:** `MethodBadge` (text label), `StatusCode` (digits + class underline/dot), `ServedByChip` (icon + text), `ConnectionPill` (icon + text), selected `FeedRow` (rail + fill), form errors (text), byte-cap (text + color). All verifiable in grayscale (AC-D14).
- **Focus-visible:** 2px `--focus-ring`, 2px offset, on every interactive element incl. feed rows, tab triangles, slider thumbs, copy buttons; `outline:none` only when replaced by the ring (AC-D15).
- **prefers-reduced-motion:** all durations → ~0ms; feed-row arrival instant; skeleton → static block; pills/spinners carry state in text; pulses suppressed (AC-D16).
- **Theme:** semantic-token only, no hardcoded inversion; first paint = light default, then `prefers-color-scheme`, then persisted explicit toggle (AC-D24).

---

## 10. Implementation notes (frontend-engineer)

- **Wiring:** drop §2.1–2.2 into `src/globals.css` (`:root` light + `.dark` dark), exactly the structure of `../shortener-link/src/globals.css`. Add the new color groups to `tailwind.config.ts` `theme.extend.colors` mirroring the reference config — e.g. `accent.fill`, `method.{get,post,…}.{fg,bg}`, `served.{rule,crud,…}`, `neutral-chip.{fg,bg}`. Reuse the reference's `fontSize`, `borderRadius`, `boxShadow`, `zIndex`, `keyframes`/`animation` blocks; add `dur-instant`, `ease-emphasized`, and a `feed-row-in` keyframe.
- **`feed-row-in` keyframe (new):**
  ```css
  @keyframes feed-row-in { from { opacity:0; transform:translateY(-8px); } to { opacity:1; transform:translateY(0); } }
  @keyframes rail-flash { 0%{box-shadow:inset 3px 0 0 var(--accent);} 100%{box-shadow:inset 3px 0 0 transparent;} }
  /* row: animation: feed-row-in var(--dur-base) var(--ease-emphasized), rail-flash 600ms var(--ease-standard); */
  @media (prefers-reduced-motion: reduce){ .feed-row{ animation:none !important; } }
  ```
- **Focus ring** + **`.skeleton` shimmer** + reduced-motion `@media` block: copy verbatim from the reference `globals.css @layer base/utilities` (lines 178–231), swapping only token values.
- **CVA variants:** `MethodBadge`/`ServedByChip`/`StatusCode` follow the `buttonVariants` CVA pattern (`../shortener-link/src/components/ui/button.tsx`) and the `StatusBadge` `CONFIG` map (`.../status-badge.tsx`) — icon + label + `cls` token pair per variant.
- **No hex in components** — hard rule; all color via Tailwind token classes. A lint/grep for `#[0-9a-fA-F]{3,6}` under `src/components/**` should return zero (AC-D23, mirrors fixing `base.html`).
- **Templates touched:** none — this is a fresh `src/` SPA; the Jinja `templates/*` are retired (PRD §2). The hardcoded-hex `<style>` block in `templates/base.html` is *not* carried over.

---

## 11. UX handoff notes

- **Light as first paint (ux.md gap #10):** I set **light** as the new-visitor first-paint default (then `prefers-color-scheme`, then persisted toggle), to deliver the "fresh, beautiful, light" mandate, while keeping HookBox dark-first in *identity* and both themes AA. This is a visual default, not a structural change — confirm with PM (folded as AC-D24).
- **Method/served-by/status are now a coordinated chip *system*** (§2.2–2.3) replacing ux.md's "design-agent finalizes palette" placeholders. No structural change to the components ux.md defined; I only supplied hues + the never-color-alone signals. The `ServedByChip` icon set (§2.3) is a concrete proposal for ux.md's "design-agent finalizes."
- **`chaos_mode` Select (ux.md gap #2 / PRD OQ-2):** styled as a standard `Select` if the architect promotes the field; no visual blocker.
- **Mock-URL chips intentionally not link-colored** (§3.7) — reinforces ux.md's "copy-only, never link-blue."
- No contradictions with ux.md structure, copy, or a11y decisions were introduced.

---

## 12. PRD gaps — for the PM to add or clarify

1. **First-paint theme default.** PRD §1/spec §10 say "fresh, light"; this doc ships **both** themes AA with **light as the first-paint default** for new visitors (then `prefers-color-scheme`, then persisted). PM: confirm light-default is intended (vs. dark-default-but-lightweight). Ties to ux.md gap #10.
2. **Brand mark + accent hue.** This doc proposes an inline-SVG hook-mark and a **teal** accent (distinct from the old `#58a6ff` and the reference indigo). PM/human: confirm teal (and any real brand hue/logo to honor) — isolated to `--teal-*`/`--accent*` tokens + the SVG.
3. **No visual AC in the PRD today.** The PRD has zero visual/aesthetic ACs. PM: fold the §11 AC-D set below into Area 10 so the QA lane can test look-and-feel, not just behavior.
4. **Light-theme accent-fill contrast.** `--text-on-accent` (white) on light `--teal-500 #0d9488` is ~4.5:1 — at the AA edge for normal text. PM/design: accept the named fallback (darken fill to `--teal-600` for the primary button if rendered ratio dips below 4.5:1), and add it as a measurable AC (AC-D20).
5. **`webhook_action` honesty styling (ties to ux.md gap #3 / OQ-9).** Confirm the "stored, not yet sent" `InlineAlert` (warning variant) is acceptable as the visual treatment for the inert control.
6. **Decorative landing texture.** The hero glow/hairline-grid is `aria-hidden` and reduced-motion-safe. PM: confirm a purely-CSS decorative layer (no image asset) is in scope for the landing's "beautiful" bar.

### Visual acceptance criteria (testable) — propose for PRD Area 10
- **AC-D11** No component file under `src/components/**` contains a raw hex color (grep `#[0-9a-fA-F]{3,6}` → 0 matches); all color resolves through semantic Tailwind tokens. (Fixes the `base.html` inline-hex anti-pattern.)
- **AC-D12** Toggling `.dark` on `<html>` re-themes the entire SPA via tokens with no component-level color change; both themes pass automated contrast checks for every token pair in §9.
- **AC-D13** A selected `FeedRow` is distinguishable from unselected **with color removed** (3px leading rail + `bg-active` fill present in grayscale) and carries `aria-selected`.
- **AC-D14** `MethodBadge`, `StatusCode`, `ServedByChip`, and `ConnectionPill` are each identifiable in a grayscale screenshot (text label and/or icon present); status class is conveyed by digits + underline/dot, not hue alone.
- **AC-D15** Every interactive element shows a visible 2px focus-visible ring (≥3:1 against adjacent fill) on keyboard focus, including feed rows, JsonTree disclosure triangles, slider thumbs, and copy buttons.
- **AC-D16** Under `prefers-reduced-motion: reduce`, feed-row arrival is an instant insert (no slide/flash), skeletons are static blocks, and no auto-pulse/spin is the sole state cue.
- **AC-D17** `ConnectionPill` state (connecting/live/reconnecting/sse/offline) is conveyed by its **text label** independent of icon animation and dot color.
- **AC-D18** The live feed maintains vertical column alignment (tabular numerics for status/latency) as rows prepend; columns do not reflow on new-row arrival.
- **AC-D19** Mock-URL chips render in `text-primary` mono (not `--accent`/link color) and expose only a copy affordance — no navigation.
- **AC-D20** `--text-on-accent` on the primary button's `--accent-fill` measures ≥ 4.5:1 in both themes (apply the §11.4 `--teal-600` fallback in light if needed).
- **AC-D21** Every status/method/served-by chip's `fg` on its `bg` measures ≥ 4.5:1 in both themes.
- **AC-D22** The landing email gate is visually identical for a brand-new vs. existing email (no "welcome back" divergence) through submit/success.
- **AC-D23** The landing page first-paints in the light theme for a new visitor with no stored preference and no `prefers-color-scheme: dark`; an explicit toggle persists and overrides.
- **AC-D24** The `body_template` byte counter turns `--warning-fg` as it approaches 256 KB and `--danger-fg` when exceeded (text + color, not color alone).
