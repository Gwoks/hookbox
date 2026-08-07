# Visual design: Operator Toolkit (F1–F7)

**Slug:** `operator-toolkit` · **Builds on:** `docs/features/operator-toolkit/ux.md` (functional layer —
IA, components, interaction, copy, a11y) · **Context:** `prd.md`, `architecture.md`, `security.md`
**Scope:** the aesthetic layer only — colour, type, spacing, hierarchy, state treatment, motion,
polish. Nothing here changes ux.md's structure, copy or accessibility decisions; where a visual
choice touches them it is listed in **§9 UX handoff notes**.

> **Stack note.** The task brief described Jinja templates in `templates/`; that directory does not
> exist. HookBox's UI is a Vite + React + TS SPA with Tailwind token classes over CSS custom
> properties. ux.md already corrected this. Tokens below are grounded in
> `src/globals.css` + `tailwind.config.ts`, not in a hypothetical `base.html`.

**Verification legend.** **[existing — verified at `path:line`]** = read in this repo.
**[new — proposed]** = does not exist; I am proposing it. No class or variable is presented as
existing unless it was read.

---

## 1. Design direction

Operator surfaces (F1, F3, F5, F6) get **zero new visual language** — every control is composed from
tokens and component classes already shipped, so the additions read as things that were always
there; the only *new* visual object is a 6 px determinate progress bar. The public `/s/:code` viewer
goes the other way: it is deliberately a **different kind of object** — a quiet, centred document
sheet on canvas with a standing info band, a footer, and *not one accent-filled control anywhere on
the page* — so no screenshot of it can be mistaken for the operator's own authenticated dashboard.

Three rules carry the whole batch:

1. **Destructive weight is graded, not binary.** In-page destructive triggers are ghost +
   `text-danger-fg`; filled `variant="danger"` appears only inside a confirm dialog or the Settings
   danger zone **[existing — verified at `src/screens/settings.tsx:433-440`, `rules-manager.tsx:331`]**.
2. **One accent-filled control per surface** (a shipped rule **[existing — verified at
   `src/components/ui/button.tsx:1-6`]**), extended here to the viewer as *zero*.
3. **Hue is never the only signal.** Every state below pairs colour with text, an icon, a fill, a
   position or a rotation — because HookBox's hairline borders measure ≈1.3:1 against their fill
   (§8) and cannot carry state.

---

## 2. Design tokens

### 2.1 Colour — semantic (no new colours are introduced by this feature)

| Token / class | Light value | Dark value | Where I use it |
|---|---|---|---|
| `bg-canvas` **[existing — `globals.css:77,162`]** | `--slate-50` `#f7f9f9` | `--slate-950` `#0d1012` | viewer page background; recessed detail well inside the transcript card |
| `bg-surface` **[existing — `globals.css:78,163`]** | `#ffffff` | `--slate-900` `#13171a` | transcript card, viewer banner band, dialogs |
| `bg-surface-raised` **[existing — `globals.css:79,164`]** | `#ffffff` | `--slate-850` `#1a1e22` | menu surface (feed-actions menu), toasts |
| `bg-surface-subtle` **[existing — `globals.css:80,165`]** | `#f7f9f9` | `#1a1e22` | viewer column-header band |
| `bg-surface-hover` / `bg-surface-active` **[existing — `globals.css:81-82,166-167`]** | `#eef1f2` / `#e0e5e7` | `#23282d` / `#343a40` | row hover; `Progress` track |
| `border-border` / `border-border-strong` **[existing — `globals.css:83-84,168-169`]** | `#e0e5e7` / `#cbd2d6` | `--slate-700` / `--slate-600` | all hairlines; secondary-button border |
| `text-text-primary / -secondary / -tertiary` **[existing — `globals.css:85-87,170-172`]** | `#13171a` / `#4b5258` / `#697077` | `#f7f9f9` / `#cbd2d6` / `#97a2a8` | body / supporting / caption ink |
| `accent`, `accent-fill`, `accent-subtle-bg` **[existing — `globals.css:90-93,174-177`]** | `--teal-500`, `--teal-600`, `--teal-50` | `--teal-300-dark`, `--teal-400-dark`, `rgba(63,214,191,.14)` | BrandMark; `Progress` fill; export-strip wash |
| `success-fg/bg` **[existing — `globals.css:95-96,179-180`]** | `--green-600` / `--green-50` | `--green-400-dark` / 14 % | success toast icon, copy-confirm check |
| `info-fg/bg` **[existing — `globals.css:97-98,181-182`]** | `--sky-600` / `--sky-50` | `--sky-400-dark` / 14 % | viewer standing banner; detail-gone alert |
| `warning-fg/bg` **[existing — `globals.css:99-100,183-184`]** | `--amber-600` / `--amber-50` | `--amber-400-dark` / 14 % | share exposure warning; 429 alert; one-time-link note; `target_url` diff row |
| `danger-fg/bg` **[existing — `globals.css:101-102,185-186`]** | `--red-600` / `--red-50` | `--red-400-dark` / 14 % | Clear all trigger + confirm; import failure; revoke failure |
| `neutral-chip-fg/bg` **[existing — `globals.css:103-104,187-188`]** | `--slate-600` / `--slate-100` | `--slate-200` / `--slate-800` | share-count badge; "Read-only" chip; `<redacted>` pill |
| `focus` (`--focus-ring`) **[existing — `globals.css:94,178`]** | `--teal-500` | `--teal-300-dark` | global focus ring |
| `served-*`, `method-*` chip pairs **[existing — `globals.css:107-137,191-221`]** | — | — | reused verbatim on the viewer rows |
| `--overlay-scrim` **[existing — `globals.css:105,189`]** | `rgba(13,17,20,.45)` | `rgba(0,0,0,.62)` | dialog scrim (unchanged) |

**Verified dead classes — do not copy them into new code.** `bg-subtle`, `bg-hover`, `bg-active` are
used at 12 call sites **[existing — verified at `rules-manager.tsx:232`, `code-block.tsx:22,42`,
`json-tree.tsx:81`, `tabs.tsx:19`, `segmented.tsx:25`, `connection-pill.tsx:59`, `app-shell.tsx:104,163`,
`feed-row.tsx:64-65`, `inspector.tsx:266`, `rule-builder.tsx:518`, `slider.tsx:30`]** but
`tailwind.config.ts:22-28` defines these under `colors.surface.*`, so the generated utilities are
`bg-surface-subtle` / `bg-surface-hover` / `bg-surface-active`. `bg-subtle` etc. generate **nothing**,
which is why the rules-manager column band, the CodeBlock/JsonTree/Tabs fills and the feed-row hover
currently render with no fill at all. All new code in this feature uses the `bg-surface-*` names; see
§10 for the one-line fix of the existing sites (recommended in the same PR so the new viewer band and
the rules band match).

### 2.2 Typography **[all existing — verified at `tailwind.config.ts:80-93`]**

| Class | Size / line-height / weight | Use in this feature |
|---|---|---|
| `text-display` | 44 / 1.05 / 700 | **not used** (reserved for landing + `/cli`) |
| `text-h1` | 30 / 1.2 / 700 | the viewer's single `<h1>` "Shared requests" |
| `text-h2` | 22 / 1.25 / 650 | `DialogHeader` title (free from the primitive **[`dialog.tsx:49`]**); viewer "link unavailable" heading |
| `text-h3` | 18 / 1.3 / 600 | — |
| `text-h4` | 15 / 1.4 / 600 | Settings "Configuration" section heading; empty-state headings |
| `text-body` | 15 / 1.5 | share-dialog intro paragraph |
| `text-body-sm` | 13 / 1.45 | dialog bodies, list rows, progress label, helper text |
| `text-caption` | 12 / 1.4 / 500 | hints under buttons, "Updated {when}", footer, badges |
| `text-overline` | 11 / 1.3 / 600 / +0.06em | column-header band, "Your share link" label |
| `text-mono` / `-sm` / `-lg` | 13 / 12 / 16, weight 450–500 | paths, header values, the minted share URL (`mono-lg` in `CodeBlock`) |
| `font-sans` / `font-mono` **[`globals.css:140-142`]** | system stacks | as shipped |
| `.tnum` **[existing — `globals.css:257-259`]** | `tabular-nums` | progress counters, latency, status, countdown seconds, share count |

Font stacks and the type scale are untouched. **`.tnum` is mandatory on every counting label added by
this feature** (`Exporting 41 of 100…`, `Creating rule 3 of 7…`, `retries in 12s`, the share count) so
digits don't reflow while they change.

### 2.3 Spacing, radius, elevation, z-index, motion

- **Spacing** — Tailwind's default 4 px scale plus `spacing.header: 52px` / `subheader: 48px`
  **[existing — `tailwind.config.ts:116-119`]**. The rhythms I match exactly: feed pane `px-3 py-2`
  **[`dashboard.tsx:314`]**, sub-header `px-4 py-2.5` **[`app-shell.tsx:98`]**, dialog `px-6 py-4/py-5`
  **[`dialog.tsx:48,55,60`]**, Settings section `space-y-3 … pb-6` **[`settings.tsx:715`]**, empty-state
  card `p-8` **[`rules-manager.tsx:219`]**, `/cli` chrome `px-6 py-4` **[`cli.tsx:69`]**.
- **Radius** **[existing — `tailwind.config.ts:94-100`]** — `xs 4` (chips, badges), `sm 6` (buttons,
  inputs, `Progress`), `md 10` (cards, dialogs, `CodeBlock`, `InlineAlert`), `lg 14` (unused here),
  `pill` (count badge, progress bar).
- **Elevation** **[existing — `tailwind.config.ts:101-107`, values at `globals.css:145-149,223-226`]** —
  `shadow-xs` on the viewer transcript card (**the one elevation delta in this design**; the dashboard
  puts shadows only on overlays, so a hairline shadow on canvas reads "artifact, not app"),
  `shadow-md` menus/toasts, `shadow-lg` dialogs, `shadow-focus` unchanged.
- **z-index** **[existing — `tailwind.config.ts:130-138`]** — `dropdown 1000` (feed-actions menu),
  `dialog 1200`, `toast 1300`, `tooltip 1400`. The export strip is **in flow**, no z-index. Nothing new.
- **Motion** **[existing — `tailwind.config.ts:120-129`, `globals.css:151-157`]** — durations
  `instant 90ms / fast 120ms / base 180ms / slow 240ms`; easings `standard cubic-bezier(.2,0,0,1)`,
  `emphasized cubic-bezier(.2,.7,0,1)`. Animations reused: `animate-fade-in`, `animate-content-in`,
  `animate-sheet-in`, `animate-overlay-in`, `animate-toast-in`, `animate-spin`. **Deliberately not
  reused:** `animate-feed-row-in` + `rail-flash` **[`tailwind.config.ts:154-173`]** — that pair is the
  *live* feed's signature and must never appear on the 5 s-polled viewer.

### 2.4 New tokens (exactly one) and new primitives (exactly two)

| Item | Status | Detail |
|---|---|---|
| `maxWidth.viewer: '920px'` | **[new — proposed]** in `tailwind.config.ts:108-112` next to `landing/landing-hero/settings` | the viewer content column. Alternative if the PM wants zero config churn: the arbitrary class `max-w-[920px]`. A named token is preferable — three places need it (banner, main, footer). |
| `src/components/ui/progress.tsx` | **[new — proposed]**, ux.md §1.1 | 6 px determinate bar, tokens only, ~20 lines (§10.1). No `Progress`-like primitive exists **[verified: zero matches for `progress` under `src/`]**. |
| `src/components/hookbox/confirm-dialog.tsx` | **[new — proposed]**, ux.md §1.1 | extraction of `settings.tsx:659-705`; purely structural, adds one `InlineAlert` slot. No new visual language. |

No new colour, no new font size, no new radius, no new shadow, no new keyframe.

---

## 3. Component styling

### 3.1 F1 — the destructive "Clear all" trigger

**Primary (ux.md's recommended overflow-menu placement).** Trigger =
`Button variant="ghost" size="icon-sm"` + `MoreHorizontal className="h-4 w-4"` +
`aria-label={t("feed.actions.menu.aria")}` — byte-for-byte the shipped row-menu trigger
**[existing — `rules-manager.tsx:274-283`]**. It sits left of Pause inside the header's
`flex items-center gap-2` **[existing — `dashboard.tsx:325`]**.

Menu content (`MenuContent align="end"` **[existing — `menu.tsx:10-27`]**, `bg-surface-raised`,
`rounded-md`, `shadow-md`, `animate-content-in`):

```
Export CSV                       ← MenuItem, default ink (text-text-secondary)
──────────────────────────────   ← MenuSeparator (my-1 h-px bg-border)
Clear all                        ← MenuItem, DESTRUCTIVE
Nothing to clear or export yet…  ← hint div, only when rows.length === 0
```

Destructive item — **exact classes**:

```tsx
<MenuItem className="text-danger-fg focus:bg-danger-bg focus:text-danger-fg data-[disabled]:text-text-tertiary">
```

The two `focus:` overrides are load-bearing and are a **fix**, not decoration: `MenuItem`'s base
string ends with `focus:bg-surface-hover focus:text-text-primary` **[existing — `menu.tsx:38`]**, and
`.focus\:text-text-primary:focus` (specificity 0,2,0) beats a plain `.text-danger-fg` (0,1,0) —
so on the shipped rules-manager Delete item **the red disappears at the exact moment the pointer or
keyboard lands on it** **[existing — `rules-manager.tsx:291-296`]**. `focus:text-danger-fg` restores
it; `focus:bg-danger-bg` makes the highlight itself destructive-coloured (fg 5.66:1 on that fill,
§8). Recommend applying the same two classes to the existing rules Delete item in the same PR.

Hint line (empty feed): `<div className="px-2 py-1.5 text-caption text-text-tertiary">` — the
AccountMenu's non-interactive-row treatment **[existing — `app-shell.tsx:239`]**.

**Fallback (if the PM keeps AC-1's literal inline placement — ux.md gap #1/#2).** Inline
`Button variant="ghost" size="sm"` with:

```tsx
className="text-danger-fg hover:bg-danger-bg hover:text-danger-fg"
```

plus `<Trash2 className="h-3.5 w-3.5" aria-hidden="true"/>` and
`<span className="sr-only md:not-sr-only">` — the responsive-label pattern already in the sub-header
**[existing — `app-shell.tsx:128-135`]**. The `hover:text-danger-fg` is required for the same
specificity reason (`ghost` carries `hover:text-text-primary` **[existing — `button.tsx:20`]**).
Disabled needs nothing: `disabled:bg-surface-subtle disabled:text-text-tertiary` (0,2,0) already wins
**[existing — `button.tsx:14`]**. Icon at `h-3.5 w-3.5` matches the Pause icon it sits beside
**[existing — `dashboard.tsx:344-346`]**.

### 3.2 F1 — the confirm dialog

`DialogContent` (`w-[min(560px,92vw)]`, `rounded-md`, `border-border`, `bg-surface`, `shadow-lg`,
bottom-sheet below `sm` — all free from the primitive **[existing — `dialog.tsx:22-29`]**).
`DialogHeader` → `text-h2`. `DialogBody` → `<p className="text-body-sm text-text-secondary">`.
`DialogFooter` → `[Button variant="ghost"] [Button variant="danger"]` — the shipped delete shape
**[existing — `rules-manager.tsx:317-336`]**. `variant="danger"` = `bg-danger-fg text-white`
**[existing — `button.tsx:21`]**; white on `--red-600` = 6.47:1 light, and in dark
`--red-400-dark` with **white** ink is only 1.7:1 → see §8 and PRD gap 4.

Error path (ux.md §3.1, gap #28): an `InlineAlert variant="danger" role="alert" className="mt-3"`
appended inside `DialogBody`, above the footer, carrying the server `detail`. In flight: confirm
button `loading` (spinner keeps the width, `aria-busy` **[existing — `button.tsx:52,60`]**), Cancel
`disabled`.

### 3.3 F5 — the export progress strip and the `Progress` bar

A new band between the feed header and the feed list, matching the offline-banner shape
**[existing — `dashboard.tsx:264-274`]** at the feed pane's `px-3` rhythm:

```
┌ feed header ──────────────────────────────────────────────┐
├ export strip  border-b border-border bg-accent-subtle-bg ─┤
│ Exporting 41 of 100…                            [Cancel]  │  row 1
│ ▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  row 2 (h-1.5)
├ feed list (still live, still scrolling) ──────────────────┤
```

```tsx
<div className="animate-fade-in space-y-1.5 border-b border-border bg-accent-subtle-bg px-3 py-2">
  <div className="flex items-center justify-between gap-2">
    <span className="tnum text-body-sm text-text-secondary">{label}</span>
    <Button variant="ghost" size="sm" onClick={cancel} disabled={phase === 'serialising'}>
      {t('feed.export.cancel')}
    </Button>
  </div>
  <Progress value={done} max={total} label={label} />
</div>
```

Why `bg-accent-subtle-bg` and not `bg-surface-subtle`: `--bg-subtle` and `--bg-canvas` are **the same
value in light** (`--slate-50` **[verified — `globals.css:77,80`]**) and the feed pane has no
background of its own **[existing — `dashboard.tsx:312`]**, so a "subtle" strip would be invisible in
light theme. The teal wash is faint, matches the bar fill, and reads as *activity* rather than as a
warning or an error. Label uses `text-text-secondary` (7.23:1 on that wash) not `text-tertiary`
(4.57:1 — too close to the floor, §8).

Two-row layout, not one: at the feed pane's 360 px minimum **[existing — `min-w-feed`,
`tailwind.config.ts:113-115`, `split-pane.tsx:53`]** a label + bar + Cancel cannot share a line.

**Cancel is `disabled`, never removed, during "Preparing file…"** — it cannot interrupt synchronous
serialisation, and removing it would reflow the strip at the worst possible moment.

`Progress` spec (§10.1 for the code): track `h-1.5 w-full rounded-pill bg-surface-active`
(6 px, same height as the shipped slider track **[existing — `slider.tsx:30`]**); fill
`h-full rounded-pill bg-accent-fill` with `transition-[width] duration-base ease-standard`;
fill-vs-track contrast 3.94:1 light / 5.19:1 dark (§8). `aria-valuetext` carries the localized label
so the digits and the bar can never disagree.

### 3.4 F3 — the Settings "Configuration" section

Reuses `Section` verbatim (`space-y-3 border-b border-border pb-6 last:border-b-0` + `h4`
**[existing — `settings.tsx:707-720`]**), placed after Save and before Retention & state, and copies
Retention's button row exactly so the two bands read as one family of "operations"
**[existing — `settings.tsx:396-419`]**:

```tsx
<Section title={t('set.config.title')}>
  <p className="text-body-sm text-text-tertiary">{t('set.config.helper')}</p>
  <div className="flex flex-wrap gap-2">
    <Button variant="secondary" size="sm" …>{t('set.config.export')}</Button>
    {/* file input + label — see below */}
  </div>
  <p className="text-caption text-text-tertiary">{t('set.config.import.helper')}</p>
</Section>
```

**The import control is a `<label>`, and it needs a focus ring the shipped primitives don't give it.**
A `sr-only` input is invisible, so the global `:focus-visible` outline **[existing —
`globals.css:247-251`]** lands on something with no painted box. Fix with a peer:

```tsx
<input id="cfg-import" type="file" accept="application/json,.json"
       className="peer sr-only" aria-describedby="cfg-import-hint" … />
<label htmlFor="cfg-import"
       className={cn(buttonVariants({ variant: 'secondary', size: 'sm' }), 'cursor-pointer',
         'peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-focus',
         'peer-disabled:cursor-not-allowed peer-disabled:bg-surface-subtle peer-disabled:text-text-tertiary')}>
  {t('set.config.import')}
</label>
```

`buttonVariants` is already exported **[existing — `button.tsx:68`]**; it has no `cursor-pointer`
(real `<button>`s don't need one) and no disabled styling for the label case — both added above.
The input must precede the label in the DOM for `peer-*` to apply.

**Import confirm dialog — the config diff.** `security.md` AC-S21 requires a pre-apply field-by-field
diff that ux.md's confirm body predates. Visual spec:

```tsx
<div className="divide-y divide-border overflow-hidden rounded-sm border border-border">
  {changes.map(c => (
    <div key={c.field}
         className={cn('grid grid-cols-[8rem_minmax(0,1fr)] gap-2 px-2.5 py-1.5',
                       c.field === 'target_url' && 'bg-warning-bg')}>
      <span className="font-mono text-mono-sm text-text-secondary">{c.field}</span>
      <span className="min-w-0 break-all font-mono text-mono-sm">
        <span className="text-text-tertiary line-through">{c.from}</span>
        <span className="px-1 text-text-tertiary" aria-hidden="true">→</span>
        <span className="text-text-primary">{c.to}</span>
      </span>
    </div>
  ))}
</div>
```

`line-through` + tertiary ink for the old value, primary ink for the new (the arrow is decorative and
hidden from AT — the DOM order already reads old-then-new). The `target_url` row is amber-washed and
gets a `text-caption text-warning-fg` line beneath the block, because that one field silently
re-points live traffic (`security.md` S-11). Confirm button is `variant="primary"` — import is
recoverable, so filled danger would be miscalibrated (ux.md §3.3, gap #7). **Needs copy keys** (PRD
gap 9).

**Import progress:** the same `Progress` + `text-body-sm text-text-secondary` label inside the
Section, `space-y-1.5`, both buttons `disabled`.

**Partial failure:** persistent `InlineAlert variant="danger" role="alert"` with
`action={<Button variant="secondary" size="sm" asChild><Link …>View rules</Link></Button>}` — the
shipped alert-with-action shape **[existing — `rules-manager.tsx:204-216`]**.

### 3.5 F4 owner — the Share control in the sub-header

`Button variant="ghost" size="sm"` + `Share2 h-4 w-4` + `<span className="sr-only sm:not-sr-only">`,
first in the right action cluster **[existing — `app-shell.tsx:119-143`]**, before Rules:

```tsx
<Button variant="ghost" size="sm" onClick={open}
        aria-label={count > 0 ? t('share.action.count.aria', { n: count }) : t('share.action.aria')}>
  <Share2 className="h-4 w-4" aria-hidden="true" />
  <span className="sr-only sm:not-sr-only">{t('share.action')}</span>
  {count > 0 && (
    <span className="tnum ml-0.5 inline-flex min-w-5 justify-center rounded-pill bg-neutral-chip-bg px-1.5 text-caption font-medium text-neutral-chip-fg">
      {count}
    </span>
  )}
</Button>
```

The badge uses `neutral-chip-*` (6.99:1 light / 11.70:1 dark) and `rounded-pill`, **not** ux.md's
suggested `rounded-xs bg-subtle` — `bg-subtle` is a dead class (§2.1) and a count reads as a pill
everywhere else in the product's idiom. `min-w-5` + `.tnum` stop the button reflowing between 1 and 2
digits. Non-structural deviation, logged in §9.

### 3.6 F4 owner — the Share dialog

> **Contract correction that changes this dialog's visuals.** `architecture.md` D9/D10/D11 make the
> code **hashed** (`code_hash = sha256(code)`), revoke by non-secret integer `id`, and the plaintext
> code exists **only in the 201 body** — the list carries `{ id, label, created_at, last_used_at }`
> and no URL **[architecture.md:37, :330-341]**. `security.md` §4 note says to treat that as
> authoritative. So ux.md §2.5's per-row `MockUrlChip` + per-row `Preview` **cannot exist**. Spec
> below is for the hashed design; the plaintext variant is in §9 in case the PM reverts.

`DialogContent` default width (560 px / bottom-sheet <`sm`), `DialogHeader` `text-h2`.

```
┌ Share read-only ─────────────────────────────────────── [×] ┐
│ text-body text-text-secondary  ← DialogDescription intro    │
│                                                             │
│ ⚠ A share link exposes captured traffic       InlineAlert    │
│   …  (warning, role="status", persistent, above Create)      │
│                                                             │
│ Label (optional)                              Field + Input │
│ [ e.g. For Acme support ticket #421                   ]     │
│                              [ Create share link ] primary  │
│                                                             │
│ ┌ ONE-TIME PANEL (only after 201) ───────────────────────┐  │
│ │ YOUR SHARE LINK                        text-overline    │  │
│ │ ┌───────────────────────────────────────────────────┐  │  │
│ │ │ https://hookbox.example/s/pK3n…8Qz            ⧉  │  │  │  CodeBlock mono-lg
│ │ └───────────────────────────────────────────────────┘  │  │
│ │ ⚠ Shown once — copy it now. text-caption warning-fg     │  │
│ │                                   [↗ Open in new tab]  │  │
│ └─────────────────────────────────────────────────────────┘  │
│                                                             │
│ Active links  2 of 10          text-h4 + text-caption tert. │
│ ┌─ divide-y border rounded-md ──────────────────────────┐   │
│ │ Acme ticket #421           Created 2h · Opened 5m  [Revoke]│
│ │ Untitled link              Created 3d · Never opened [Revoke]│
│ └───────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────── [Done] ─────┤
└─────────────────────────────────────────────────────────────┘
```

- **Intro** — `DialogDescription` (already exported, currently unused **[existing —
  `dialog.tsx:66`]**) styled `text-body text-text-secondary`.
- **Exposure warning** — `InlineAlert variant="warning" role="status"` with `title`; the alert's own
  `border-l-2 border-l-warning-fg bg-warning-bg rounded-md p-3` **[existing —
  `inline-alert.tsx:13,37`]**. Persistent, never dismissible, always above Create.
- **Label field** — `Field` + `Input` **[existing — `input.tsx:15-33,82-117`]**; invalid state is
  `border-danger-fg` + a `text-body-sm text-danger-fg role="alert"` message (both from the primitive).
- **Create** — `variant="primary"` (`bg-accent-fill text-text-on-accent`, 5.01:1 light / 8.60:1 dark);
  the dialog is its own surface so this does not violate the one-accent rule.
- **One-time panel** — `role="status" aria-live="polite"` +
  `space-y-2 rounded-md border border-border bg-accent-subtle-bg p-3`. The teal wash marks it as the
  freshly created thing without shouting; the `CodeBlock` inside gets `className="bg-surface"` so the
  nested block reads as the *value* rather than a second panel. The shown-once line is
  `flex items-start gap-1.5 text-caption text-warning-fg` with
  `<AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0"/>` — amber + icon + words, never colour
  alone. "Open in new tab" is `Button variant="ghost" size="sm" asChild` → `<a target="_blank"
  rel="noopener noreferrer">` **[pattern existing — `app-shell.tsx:121-123`]**, and it lives **inside
  this panel only** (it is the only place the URL exists).
- **List** — `overflow-hidden rounded-md border border-border divide-y divide-border`; each row
  `flex flex-wrap items-center justify-between gap-2 px-3 py-2.5`; label
  `text-body-sm text-text-primary truncate` (untitled → `text-text-tertiary italic`… **no italic** —
  use `text-text-tertiary` only, the product uses no italics anywhere); meta
  `text-caption text-text-tertiary` with `title={iso}`; Revoke =
  `Button variant="ghost" size="sm" className="text-danger-fg hover:bg-danger-bg hover:text-danger-fg"`.
- **Armed revoke (inline two-step)** — the row's content is replaced in place by
  `bg-danger-bg -mx-3 px-3 py-2.5` (full-bleed inside the row) carrying
  `text-body-sm text-danger-fg` question + `text-caption text-text-secondary` consequence +
  `[Button ghost Cancel] [Button danger size="sm" Revoke]`. No animation: a destructive confirm
  should not slide.
- **At cap** — Create `disabled` + `text-caption text-text-tertiary` under it. A server 422 still
  renders an `InlineAlert variant="danger"` above the list.
- **Footer** — `DialogFooter` with `Button variant="ghost"` Done.

### 3.7 F4 public viewer — `/s/:code`

The chrome is `/cli`'s, not `AppShell`'s **[existing — `cli.tsx:67-84`]**, and the aesthetic job is to
make "different object" legible in one glance:

```
┌ header  px-6 py-4, NO border, bg transparent (canvas) ────────────────┐
│ 🪝 HookBox (BrandMark, NOT a link)                        [☀/🌙]      │
├ banner  border-y border-border bg-surface px-4 py-3 ──────────────────┤
│   ┌ InlineAlert info · role=status · mx-auto max-w-viewer ─────────┐  │
│   │ ℹ Read-only shared view                                       │  │
│   │   Someone shared this HookBox endpoint's recent requests…      │  │
│   └────────────────────────────────────────────────────────────────┘  │
├ main#main  mx-auto w-full max-w-viewer flex-1 px-4 py-6 space-y-4 ────┤
│ Shared requests                                    text-h1            │
│ Endpoint: checkout-api · Capturing since … · 1,284 …  text-body-sm sec │
│ ┌ CARD  overflow-hidden rounded-md border bg-surface shadow-xs ─────┐ │
│ │ Showing 37 of the last 100  [Read-only]   Updated 3s  [⟳ Refresh] │ │
│ │ METHOD  PATH        STATUS  SERVED  MS  WHEN   (hidden sm:grid)   │ │
│ │ ▸ POST  /webhooks/pay  200  ⎇ rule  12ms  4s                      │ │
│ │ ▾ GET   /health        404  ○ default 3ms  1m                     │ │
│ │ ┌ well  bg-canvas border-t px-3 py-3 ─────────────────────────┐   │ │
│ │ │ [Headers][Query][Body][Response]  ← 4 tabs, no trace tab    │   │ │
│ │ └──────────────────────────────────────────────────────────────┘   │ │
│ └───────────────────────────────────────────────────────────────────┘ │
│ Updates every 5 seconds while this tab is open.   text-caption tert.  │
├ footer  border-t border-border px-6 py-6 ─────────────────────────────┤
│ Served by HookBox — … This page is read-only.  text-caption tertiary  │
└───────────────────────────────────────────────────────────────────────┘
```

Element-by-element:

- **Root** — `flex min-h-screen flex-col bg-canvas` **[existing — `cli.tsx:68`]**. Note this is
  `min-h-screen` (a scrolling document), not the dashboard's `h-screen overflow-hidden`
  **[existing — `app-shell.tsx:73`, `dashboard.tsx:146`]** — the scroll model itself is a
  differentiator.
- **Banner** — full-bleed band on `bg-surface` holding an `InlineAlert variant="info" role="status"`
  constrained to `mx-auto max-w-viewer`, so its left edge lines up with the `<h1>` and the card. The
  band wrapper is the shipped offline-banner shape **[existing — `dashboard.tsx:265-273`]**; `info`
  (sky), never `warning` — nothing is wrong. Never dismissible: no `[×]`, no close affordance.
- **"Read-only" chip in the card header** **[new — proposed]**:
  `inline-flex rounded-xs bg-neutral-chip-bg px-1.5 py-0.5 text-caption font-medium text-neutral-chip-fg`.
  The banner is in flow and scrolls away; this chip keeps a read-only cue visible next to the data at
  any scroll position. Needs one copy key (§9, PRD gap 8).
- **Card header row** — `flex flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-2`;
  left `text-body-sm text-text-secondary` + chip; right `text-caption text-text-tertiary` ("Updated
  {when}", `title={iso}`) + `Button variant="secondary" size="sm"` Refresh with
  `<RotateCw className="h-3.5 w-3.5"/>` — `secondary` (a visible boundary) rather than `ghost`,
  because the audience is a non-expert who needs the one control on the page to look like a control.
- **Column-header band** — `hidden grid-cols-[…] gap-2 border-b border-border bg-surface-subtle px-3 py-2 text-overline uppercase tracking-wide text-text-tertiary sm:grid`
  — the shipped list-header treatment **[existing — `rules-manager.tsx:232-238`]**, hidden below `sm`
  where rows stack.
- **`SharedRequestRow` (collapsed)** — a real `<button type="button">`, full width, `text-left`,
  `min-h-11` (44 px touch target — link recipients are on phones),
  `grid items-center gap-2 px-3 py-2.5 text-body-sm`,
  `grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto] sm:grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto_auto_auto_auto]`,
  `border-b border-border last:border-b-0`, `hover:bg-surface-hover focus-visible:bg-surface-hover`.
  Cells reuse the feed-row *grammar* (`MethodBadge` · truncated `font-mono text-mono-sm` path with
  `title` · `StatusCode` · `ServedByChip` · `.tnum` latency · `text-caption` relative time)
  **[existing — `feed-row.tsx:70-81`]** but **not** the component (it is `role="option"`).
  Chevron: `ChevronRight className="h-3.5 w-3.5 text-text-tertiary transition-transform duration-fast ease-standard"`
  + `rotate-90` when open — the JsonTree disclosure idiom **[existing — `json-tree.tsx:44-47`]**.
- **Open row + detail well** — the open trigger takes `bg-canvas`, and the well below is
  `border-t border-border bg-canvas px-3 py-3`; together they read as one recessed block.
  `bg-canvas` is darker than `bg-surface` in **both** themes (light `#f7f9f9` < `#ffffff`; dark
  `#0d1012` < `#13171a` **[verified — `globals.css:77-78,162-163`]**), which is why it is the correct
  "recess" token and `bg-surface-subtle` is not (that one *inverts* to lighter in dark).
  **No accent rail** — `shadow-[inset_3px_0_0_var(--accent)]` is the dashboard's selection signature
  **[existing — `feed-row.tsx:65`]** and is forbidden here.
- **Tabs** — the shipped `Tabs`/`TabsList`/`TabsTrigger` **[existing — `tabs.tsx`]**: 4 triggers,
  active = `text-text-primary` + 2 px accent underline. This underline is the **only** accent-coloured
  pixel on the page besides the BrandMark glyph and the focus ring — acceptable because it is a
  selection marker, not a call to action.
- **Footer** — `border-t border-border px-6 py-6 text-caption text-text-tertiary`, content
  `mx-auto max-w-viewer`. The dashboard has no footer at all; this is a document cue.
- **Hard visual prohibitions (each assertable):** no `variant="primary"` / `bg-accent-fill` control
  anywhere on the page; no `animate-feed-row-in`; no `rail-flash`; no `SplitPane` or drag handle
  **[existing — `split-pane.tsx`]**; no `ConnectionPill` **[existing — `connection-pill.tsx`]**; no
  `MockUrlChip`/`CodeBlock` carrying an endpoint URL; no `FeedEmpty` (it renders `mock_url` + a curl
  sample **[existing — `dashboard.tsx:384-413`]**); no `ServedByChip` tooltip (the shipped chip
  renders none — keep it that way, the strings are owner-voiced **[existing —
  `served-by-chip.tsx:40-55`, `copy.ts:157-159`]**).

### 3.8 F6 — "Add default rule"

Toolbar: `Button variant="secondary" size="sm"` immediately left of the primary "New rule"
**[existing — `rules-manager.tsx:191-194`]**; `gap-2` between them. Empty-state card
(`rounded-md border border-border bg-surface p-8 text-center` **[existing —
`rules-manager.tsx:219`]**): the same secondary button centred under the body with `mt-4`, then
`<p className="mx-auto mt-2 max-w-md text-caption text-text-tertiary">` for the helper — mirroring the
card's existing `mx-auto mt-2 max-w-md text-body-sm text-text-tertiary` body one step quieter.

Disabled-with-reason wrapper (ux.md §3.6, gap #26):

```tsx
<Tooltip content={t('rules.default.exists')}>
  <span tabIndex={0} title={t('rules.default.exists')} className="inline-flex rounded-sm">
    <Button variant="secondary" size="sm" disabled>…</Button>
  </span>
</Tooltip>
```

The wrapper needs no focus classes — the global `:focus-visible` rule paints every focusable element
**[existing — `globals.css:247-251`]**; `rounded-sm` on the wrapper just keeps the outline's corners
matching the button inside it.

### 3.9 F7 — two rendering consequences with a visual surface

1. **Large-body raw default** (ux.md §3.7 item 1). When `JsonTree` opens in `raw` because the body
   exceeds ~64 KB, render the reason as
   `<p className="px-3 pb-2 text-caption text-text-tertiary">{t('insp.body.largeRaw')}</p>` inside the
   container, directly under the Pretty/Raw strip. Quiet caption, not an `InlineAlert` — nothing is
   wrong, the tool just chose a mode.
2. **The `<redacted>` pill** (ux.md §3.7 item 3, gap #23). Once the sentinel matches, the shipped
   pill renders: `inline-flex rounded-xs bg-neutral-chip-bg px-1.5 py-0.5 text-caption font-medium text-neutral-chip-fg`
   **[existing — `key-value-rows.tsx:32-35`]**. Visually this is exactly right for an unauthenticated
   page — a *neutral chip*, not red (nothing failed) and not mono text (so it cannot be mistaken for a
   value the caller actually sent). No change needed beyond the sentinel constant.

---

## 4. Visual hierarchy & layout

**Feed pane (F1 + F5).** Emphasis order left→right: `h2 "Live feed"` (`text-h4`) → count
(`text-caption` tertiary) → … → `[N new]` (secondary — the only bordered control, because it is the
one thing that appears *because something changed*) → `[⋯]` (icon-only ghost) → `[Pause]` (ghost with
label). The destructive action is deliberately the *lowest-emphasis* pixel in the header until it is
opened. The export strip inserts a tinted band between chrome and data — the eye is drawn to it once,
then returns to the still-streaming list beneath.

**Settings (F3).** The screen's existing vertical logic is *form → Save → immediate operations*.
Configuration joins the operations group, so all three "operations" bands (Configuration, Retention,
Danger zone) share one silhouette: `h4` heading, `text-body-sm` tertiary rationale, a
`flex flex-wrap gap-2` row of `secondary size="sm"` buttons, a `text-caption` footnote. Escalating
emphasis top-to-bottom ends at the only bordered-and-washed block on the screen (danger zone,
`border-danger-fg/40 bg-danger-bg` **[existing — `settings.tsx:422`]**) — Configuration must not
compete with it, which is why it carries no fill and no border of its own.

**Share dialog (F4 owner).** Reading order is a funnel: what this is (`text-body`) → what it exposes
(amber alert — the heaviest object in the dialog, and it is above the button on purpose) → the one
input → the single accent action → the created artifact (teal panel) → the inventory (`h4` + list) →
Done. Density: airy above the fold (`space-y-4`), compact in the list (`py-2.5` rows).

**Viewer (F4 public).** Eye path: brand → info band → `text-h1` → one line of secondary context →
card. The card is the only elevated, only bordered, only `bg-surface` object on a canvas page —
everything funnels into it. Inside the card, density steps up (12 px overline band, 44 px rows,
`text-mono-sm` values) which is where the page finally *feels* technical; the frame around it stays
generous (`py-6`, 920 px column, real footer). That contrast — airy document frame, dense data card —
is the whole differentiator from the dashboard's edge-to-edge, zero-margin, two-pane density.

**Accent ledger (testable).** Feed pane: 0 accent buttons (progress fill only). Settings: 1 (Save).
Rules Manager: 1 (New rule). Share dialog: 1 (Create share link). Viewer: **0**.

---

## 5. Visual states

One row per state ux.md defines. `focus-visible` everywhere = the global 2 px `--focus-ring` outline
at `outline-offset: 2px` **[existing — `globals.css:247-251`]**; not repeated per row.

### 5.1 Generic control states (apply to every new control)

| State | Treatment |
|---|---|
| rest — ghost | `text-text-secondary`, no fill **[existing — `button.tsx:20`]** |
| rest — secondary | `border border-border-strong bg-surface text-text-primary` **[`button.tsx:19`]** |
| rest — primary | `bg-accent-fill text-text-on-accent` **[`button.tsx:18`]** |
| hover | ghost/secondary → `bg-surface-hover`; primary/danger → `opacity-90`; destructive ghost → `bg-danger-bg` + `text-danger-fg` retained |
| active/pressed | no separate style (shipped buttons define none); `aria-pressed` + icon swap carries Pause **[existing — `dashboard.tsx:341-347`]** |
| focus-visible | 2 px `--focus-ring` outline, 2 px offset (3.74:1 light / 9.94:1 dark) |
| disabled | `bg-surface-subtle text-text-tertiary cursor-not-allowed shadow-none` **[`button.tsx:14`]**; menu items `data-[disabled]:text-text-tertiary` **[`menu.tsx:38`]** |
| loading | `Spinner h-4 w-4` prepended, width preserved, `aria-busy` **[`button.tsx:52,60`]** |

### 5.2 F1 — Clear all

| State (ux.md §3.1) | Visual |
|---|---|
| unavailable | both menu items `data-[disabled]` → tertiary ink; trigger stays enabled; hint `px-2 py-1.5 text-caption text-text-tertiary` |
| confirming | dialog: scrim `--overlay-scrim` + `animate-overlay-in`; panel `animate-content-in` (`animate-sheet-in` <`sm`); `text-h2` title; `text-body-sm text-text-secondary` body; footer `[ghost] [danger]` |
| cancelled | dialog unmounts, no visual residue; focus ring reappears on the `[⋯]` trigger |
| in flight | confirm `loading` (spinner + `aria-busy`), Cancel `disabled` → `bg-surface-subtle` |
| success | dialog closes; feed body swaps to `FeedEmpty`; `[N new]` secondary pill disappears; success toast (`CheckCircle2 text-success-fg`, `bg-surface-raised`, `shadow-md`, `animate-toast-in`, 3200 ms **[existing — `toast.tsx:40-48`]**) |
| failure | dialog stays; `InlineAlert variant="danger" role="alert" className="mt-3"` in the body (rail `border-l-danger-fg`, fill `bg-danger-bg`, `ShieldAlert` icon); danger toast (`XCircle text-danger-fg`); rows untouched |

### 5.3 F5 — Export CSV

| Phase (ux.md §3.2) | Visual |
|---|---|
| idle | menu item default ink; hint line under items when relevant |
| fetching | strip visible (`animate-fade-in`), label `Exporting {done} of {total}…` in `.tnum text-body-sm text-text-secondary`, bar filling with `transition-[width] duration-base`, Cancel ghost enabled |
| serialising | same strip, bar pinned 100 %, label `Preparing file…`, **Cancel `disabled` (not removed)** so the strip's geometry never reflows |
| done | strip unmounts instantly (no exit animation — the download is the feedback); success toast with the count |
| cancelled | strip unmounts; danger-free quiet success-variant toast per ux.md copy; no file |
| per-row failure | no visual change during the run (the file carries the sentinels); completion toast uses the partial-count string |
| 401 | screen unmounts (client bounce **[existing — `client.ts:99-105`]**); no strip, no toast |
| unmount / route change | strip disappears with the screen; nothing persists |

### 5.4 F3 — Export / import config

| State (ux.md §3.3) | Visual |
|---|---|
| export idle | `secondary size="sm"` |
| export, form dirty | extra `text-caption text-text-tertiary` line under the button row |
| export failure | `InlineAlert variant="danger"` inside the Section (not a toast) |
| 1 pick | `sr-only peer` input + `buttonVariants(secondary, sm)` label; `peer-focus-visible:outline …` ring |
| 2 validate fail | `InlineAlert variant="danger" role="alert"` naming the first failure + a "Choose another file" label-button below it |
| 3 confirm | dialog with the diff block (§3.4): `divide-y` rows, old value `line-through text-text-tertiary`, new `text-text-primary`, `target_url` row `bg-warning-bg` + amber caption; confirm `variant="primary"` |
| 3b dirty guard | an extra `text-body-sm text-warning-fg` line inside the dialog body above the footer |
| 4 apply | both controls `disabled` → `bg-surface-subtle text-text-tertiary`; `Progress` + `.tnum` label |
| 5 settle | success toast; `text-caption text-text-tertiary` done line in the Section; form remounts (no flash — same tokens) |
| partial failure | persistent `InlineAlert variant="danger"` + `action` = `secondary size="sm"` "View rules" |

### 5.5 F4 owner — Share dialog

| State (ux.md §3.4) | Visual |
|---|---|
| opening | `SkeletonLines lines={3}` in the list region (`.skeleton` shimmer on `--bg-subtle`, static under reduced motion **[existing — `globals.css:264-277,292-297`]**), container `aria-busy` |
| empty | centred `h3 text-h4 text-text-primary` + `text-body-sm text-text-tertiary`, `py-6 text-center` inside the list border |
| list | `divide-y` rows, label `text-body-sm text-text-primary truncate`, meta `text-caption text-text-tertiary` |
| creating | Create `loading`; label `Input` `disabled` → `bg-surface-subtle` |
| created | one-time panel appears: `rounded-md border border-border bg-accent-subtle-bg p-3`, overline label, `CodeBlock className="bg-surface"`, amber shown-once line, ghost "Open in new tab"; focus is **not** moved |
| label invalid | `Input` `border-danger-fg` + `text-body-sm text-danger-fg role="alert"` message; Create `disabled` |
| at cap | Create `disabled` + `text-caption text-text-tertiary` reason; a 422 adds `InlineAlert variant="danger"` |
| revoke armed | row swaps to `bg-danger-bg -mx-3 px-3 py-2.5` with `text-danger-fg` question, `text-caption text-text-secondary` consequence, `[ghost Cancel] [danger Revoke]`; no animation |
| revoking | that row's Revoke `loading`; siblings unchanged |
| revoked | row removed (instant, no collapse animation — an animated exit on a security action invites a double-take); success toast |
| revoke failed | row restored + `text-body-sm text-danger-fg` line inside the row (it must be visible next to the still-active link, not only in a toast) + danger toast |
| list load failed | `InlineAlert variant="danger"` + `action` Retry (`secondary size="sm"` + `RotateCw h-3.5`) |

### 5.6 F4 public viewer

| State (ux.md §3.5) | Visual |
|---|---|
| loading | header + banner + `h1` paint immediately (frame is data-independent); card body `SkeletonLines lines={8}` in `p-3`, `aria-busy` |
| empty | in-card centred block `px-6 py-12 text-center`: `h2 text-h4 text-text-primary` + `max-w-sm text-body-sm text-text-tertiary`. Shape mirrors `FeedEmpty`'s frame **[existing — `dashboard.tsx:387-393`]** with the URL/curl half deleted |
| list | rows collapsed; header line + "Read-only" chip + "Updated {when}" + Refresh |
| row expanded | chevron `rotate-90`; trigger + well on `bg-canvas`; well `border-t border-border px-3 py-3`; 4 tabs with the 2 px accent underline on the active one |
| detail loading | `SkeletonLines lines={6}` inside the well, `aria-busy` |
| detail 404 | `InlineAlert variant="info"` (sky, `Info` icon) inside the well — aged out is not an error |
| detail error | `InlineAlert variant="danger"` + Retry inside the well |
| unavailable (list 404) | full-page: header + footer retained, **banner omitted** (there is no data to be read-only about), body `flex min-h-screen flex-col items-center justify-center gap-3 bg-canvas p-6 text-center` with `h1 text-h2` + `max-w-md text-body-sm text-text-tertiary` + one `Button variant="link"` → `/` — the shipped terminal-state shape **[existing — `dashboard.tsx:143-158`]** |
| rate-limited (429) | `InlineAlert variant="warning"` above the card, countdown seconds in `.tnum`; the stale card stays at **full opacity** (the rows are still true) and "Updated {when}" switches to `text-warning-fg` — dimming would read as "disabled" |
| error (other) | `InlineAlert variant="danger"` + Retry above the card; last-known rows kept |
| poll arrival | **no animation at all.** New rows appear; "Showing N" and "Updated {when}" change. The open row keeps its `bg-canvas` well and its scroll position |
| tab hidden → visible | the under-card line swaps between the "updates every 5 s" and "paused" strings, both `text-caption text-text-tertiary` |

### 5.7 F6 — Add default rule

| State (ux.md §3.6) | Visual |
|---|---|
| available | `secondary size="sm"` in toolbar and empty-state card |
| already has a catch-all | `disabled` → `bg-surface-subtle text-text-tertiary`; reason in a `Tooltip` on a focusable `span` (tooltip surface `bg-surface-raised shadow-md` from the primitive) + `title` fallback |
| in flight | `loading` + `disabled` |
| success | list re-renders; new row appears **without** `animate-feed-row-in` (the rules list has no arrival animation today); success toast |
| failure | danger toast only; list visually unchanged |

---

## 6. Motion & micro-interactions

Everything uses shipped duration/easing tokens; nothing new is added to `keyframes`.

| Interaction | Motion | Duration / easing |
|---|---|---|
| Feed-actions menu open | `animate-content-in` (fade + 4 px rise + .98→1) **[existing — `tailwind.config.ts:142-145,169`]** | `--dur-base` 180 ms `--ease-standard` |
| Dialog open (confirm, import, share) | `animate-overlay-in` scrim + `animate-content-in` panel; `animate-sheet-in` below `sm` | 180 / 240 ms standard |
| Export strip appears | `animate-fade-in` (opacity only — no height animation, so the feed list below never janks) | 180 ms standard |
| Export strip disappears | none (instant) — the file download is the terminal feedback | — |
| `Progress` fill | `transition-[width]` only (no transform, no layout) | `--dur-base` standard |
| Button hover/ink | `transition-colors duration-fast ease-standard` **[existing — `button.tsx:14`]** | 120 ms |
| Disclosure chevron (viewer row) | `transition-transform duration-fast ease-standard` + `rotate-90` | 120 ms |
| Copy confirmation | icon swap to `Check text-success-fg` for 1600 ms + sr-only announcement **[existing — `copy-button.tsx:30,41-49`]** | — |
| Toast | `animate-toast-in` (8 px slide-in from the right), 3200 ms auto-dismiss | 180 ms standard |
| Skeletons | `.skeleton::after` shimmer sweep 1.4 s | — |
| **WS / live-feed arrival (dashboard, unchanged)** | `animate-feed-row-in` + 600 ms `rail-flash` **[existing — `tailwind.config.ts:154-173`]** | 180 ms emphasized |
| **Poll arrival on `/s/:code`** | **none, by design** — no row animation, no flash, no count pulse | — |
| Armed revoke / revoked row removal | none | — |

Reduced motion needs **zero bespoke handling**: the global block zeroes every animation and
transition, kills the skeleton shimmer, and neutralises `.feed-row`
**[existing — `globals.css:283-301`]**. Consequences to verify rather than assume: the `Progress` fill
jumps instead of easing (still correct — the digits carry the value), the strip appears instantly, and
the chevron snaps (still correct — `aria-expanded` and the well carry the state).

Performance guard rails: the only animated properties in this feature are `opacity`, `transform` and
`width` on a 6 px bar; nothing animates `height`, `box-shadow` or `top`. During an export the strip
must not re-mount per tick — only its label text and the fill's inline `width` change.

---

## 7. Responsive

Breakpoints are Tailwind defaults (`sm 640`, `md 768`, `lg 1024`, `xl 1280`); the codebase uses
`sm:` and `md:` only, plus `max-sm:` in the dialog primitive.

| Surface | Behaviour |
|---|---|
| Feed header (F1/F5) | The overflow menu is what makes 360 px work (`min-w-feed` **[existing — `tailwind.config.ts:113-115`]**): title + count + `[N new]?` + `[⋯]` + `[Pause]` fit. In the inline fallback, labels collapse via `sr-only md:not-sr-only`. |
| Export strip | Two rows always (label+Cancel / bar) — never a single row that could clip Cancel at 360 px. |
| Sub-header (F2/F4) | Already `flex-wrap` with `gap-x-4 gap-y-2` **[existing — `app-shell.tsx:98`]**. Deleting the Local path chip (F2) buys the width Share needs; below `sm` the Share label collapses to icon-only (`sr-only sm:not-sr-only`) while the count badge stays visible. |
| Settings | Column is `max-w-settings` (640 px) **[existing — `tailwind.config.ts:111`]**; the button row is `flex-wrap gap-2`, so Export/Import stack at narrow widths. The diff block's `grid-cols-[8rem_minmax(0,1fr)]` holds at 320 px with `break-all` values. |
| Dialogs | `w-[min(560px,92vw)]`, and below `sm` a bottom sheet: `max-sm:bottom-0 max-sm:w-screen max-sm:rounded-b-none max-sm:animate-sheet-in` **[existing — `dialog.tsx:25-27`]**. The share list's rows are `flex-wrap`, so label / meta / Revoke stack on a phone. |
| Viewer `/s/:code` | Single column, `max-w-viewer` (920 px), `px-4`. Column-header band `hidden sm:grid`. Row grid drops to 4 columns below `sm` with the trailing meta wrapped by a `sm:contents` span (ux.md §2.6), so no horizontal scroll at 360 px. Row min-height 44 px. `ThemeToggle` stays (`size="icon"`, 36 px, adequate). Card keeps `rounded-md` on mobile (it is a document, not a full-bleed app pane). |
| Rules Manager (F6) | Toolbar is already a two-item flex row; at 360 px "Add default rule" + "New rule" wrap — acceptable, both stay full-width-legible. |

Nothing hides content responsively except the column-header band (its labels are redundant with the
row content) — no data, no control and no state message is ever hidden by a breakpoint.

---

## 8. Visual accessibility

**Contrast (WCAG 2.1 AA), computed from the hex values in `globals.css` for every pair this feature
introduces or leans on.** Text target 4.5:1 (<18.66 px regular), non-text/UI target 3:1.

| Pair | Light | Dark | Used for |
|---|---|---|---|
| `text-primary` on `bg-surface` | **18.0:1** | **17.1:1** | all body ink |
| `text-secondary` on `bg-surface` | **7.9:1** | **11.8:1** | dialog bodies, row labels |
| `text-tertiary` on `bg-surface` | **5.0:1** | **6.9:1** | captions, meta |
| `text-tertiary` on `bg-surface-subtle` | **4.8:1** | **6.4:1** | column-header band |
| `text-secondary` on `accent-subtle-bg` | **7.2:1** | **9.6:1** | export-strip label |
| `text-tertiary` on `accent-subtle-bg` | **4.6:1** | — | ✗ **avoid** — too close to the floor; strip label uses `text-secondary` |
| `danger-fg` on `bg-surface` | **6.5:1** | **6.5:1** | inline destructive trigger |
| `danger-fg` on `bg-surface-raised` | **6.5:1** | **6.1:1** | destructive menu item |
| `danger-fg` on `danger-bg` | **5.7:1** | **5.4:1** | armed revoke, focused destructive item, danger alerts |
| `warning-fg` on `warning-bg` | **4.6:1** | **8.1:1** | exposure warning, 429, shown-once note, `target_url` diff row |
| `info-fg` on `info-bg` | **5.3:1** | **6.9:1** | viewer standing banner, detail-gone |
| `neutral-chip-fg` on `neutral-chip-bg` | **7.0:1** | **11.7:1** | share count badge, Read-only chip, `<redacted>` pill |
| `text-on-accent` on `accent-fill` | **5.0:1** | **8.6:1** | Create share link, import confirm |
| `accent-fill` fill vs `surface-active` track | **3.9:1** | **5.2:1** | `Progress` (non-text ≥3:1) |
| `focus-ring` vs `bg-surface` | **3.7:1** | **9.9:1** | global focus outline (non-text ≥3:1) |
| `border-border` vs `bg-surface` | **1.3:1** | **1.4:1** | ⚠ **decorative only** |
| `white` on `danger-fg` (`variant="danger"` label) | **6.5:1** | **1.7:1** | ⚠ **dark-theme failure — see below** |

Two findings the PM must turn into ACs:

1. **`variant="danger"` fails AA in dark theme.** `button.tsx:21` hardcodes `text-white`, and white on
   `--red-400-dark` (`#f87171`) is ~1.7:1. Every confirm dialog in the product is affected today
   (rule delete, clear history, clear state, delete endpoint), and F1 + F4's revoke add two more. Fix
   is one token: `text-white` → `text-text-on-accent` (`--slate-0` light / `--slate-950` dark), which
   gives 6.5:1 light and **8.3:1** dark. PRD gap 4.
2. **Hairline borders cannot carry state** (1.3:1). Every state in §5 therefore carries text + icon +
   fill; the only border used as a signal is `Input`'s `border-danger-fg` (6.5:1, ≥3:1) and it is
   always accompanied by a `role="alert"` message **[existing — `input.tsx:26`, `input.tsx:106-109`]**.
   This contradicts the replatform design doc's "borders ≥3:1" claim
   **[existing — `docs/features/hookbox-rust-replatform/design.md:327`]**; the *focus ring* clears
   3:1, the hairlines do not.

**Focus-visible.** Global: `outline: 2px solid var(--focus-ring); outline-offset: 2px`
**[existing — `globals.css:247-251`]**. Two places in this feature need explicit care:
the F3 file input (`sr-only` → the ring must be projected onto the `<label>` via `peer-focus-visible:*`,
§3.4) and the F6 disabled-button tooltip wrapper (a focusable `<span>` gets the ring for free; give it
`rounded-sm` so the corners match).

**Never colour-only.** Destructive menu item = word "Clear all" + red ink + red focus fill. Progress =
digits + bar. Import diff = `line-through` + order + field name (not just ink colour). Rate limit =
words + countdown + amber. Viewer read-only = banner words + chip words + the absence of every
control. Disclosure = chevron rotation + `aria-expanded` + the recessed well. `StatusCode` /
`MethodBadge` / `ServedByChip` are icon-plus-text by construction
**[existing — `served-by-chip.tsx:1-5`, `status-code.tsx:1-4`]**.

**Reduced motion.** Fully covered by the global block (§6). One assertion worth writing down: under
`prefers-reduced-motion: reduce` the export strip and the `Progress` fill must still be *visible and
correct* — the guard zeroes durations, it does not hide anything.

**Other.** No text is rendered below 11 px. No `text-tertiary` is used for essential text on a fill
where it measures <4.5:1. Zoom to 200 % keeps the viewer single-column (it already is) and the strip
two-row. Dark mode is a pure token switch; no component in this feature branches on theme.

---

## 9. Implementation notes

### 9.1 The two new files, in full

`src/components/ui/progress.tsx` **[new — proposed]**:

```tsx
/** Progress (design.md §3.3). Determinate 6px bar; tokens only. Shared by the F5
 *  CSV export strip and the F3 config import. Width transition is zeroed by the
 *  global prefers-reduced-motion block. */
import { cn } from '@/lib/cn'

export function Progress({
  value, max, label, className,
}: { value: number; max: number; label: string; className?: string }) {
  const pct = max > 0 ? Math.min(100, Math.max(0, Math.round((value / max) * 100))) : 0
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={label}
      aria-label={label}
      className={cn('h-1.5 w-full overflow-hidden rounded-pill bg-surface-active', className)}
    >
      <div
        className="h-full rounded-pill bg-accent-fill transition-[width] duration-base ease-standard"
        style={{ width: `${pct}%` }}
      />
    </div>
  )
}
```

`src/components/hookbox/confirm-dialog.tsx` **[new — proposed]** — structurally identical to
`settings.tsx:659-705`, with two visual additions: an optional `InlineAlert variant="danger"
role="alert" className="mt-3"` rendered inside `DialogBody` when the confirm rejects, and
`disabled={busy}` on Cancel. No other style change; the extraction must keep `text-h2` title,
`text-body-sm text-text-secondary` body and `[ghost][danger]` footer so the two existing Settings
confirms look identical after the swap.

### 9.2 Files `frontend-engineer` touches, and what is visual there

| File | Visual work |
|---|---|
| `src/screens/dashboard.tsx` **[existing]** | feed-header menu trigger + destructive item classes (§3.1); export strip markup (§3.3) between header and list |
| `src/components/hookbox/app-shell.tsx` **[existing]** | delete the second `UrlChip` (F2); add the Share ghost button + neutral-chip count badge (§3.5); fix the module doc-comment |
| `src/screens/settings.tsx` **[existing]** | new `Section` (§3.4), `peer`-based file-input label, `Progress`, diff block, partial-failure alert; swap the local `ConfirmDialog` for the extracted one |
| `src/components/hookbox/share-dialog.tsx` **[new]** | §3.6 in full |
| `src/screens/share-view.tsx` **[new]** | §3.7 in full; import **no** owner components (`AppShell`, `SplitPane`, `ConnectionPill`, `FeedEmpty`, `FeedRow`) |
| `src/screens/rules-manager.tsx` **[existing]** | §3.8; plus the two `focus:` classes on the existing Delete item |
| `src/components/ui/menu.tsx` **[existing]** | *optional*: if the destructive-item fix is preferred centrally, add a `destructive` prop rather than repeating the three classes at two call sites |
| `src/components/hookbox/json-tree.tsx`, `key-value-rows.tsx` **[existing]** | §3.9 (one caption, one sentinel) |
| `tailwind.config.ts` **[existing]** | add `maxWidth.viewer: '920px'` (§2.4) |

### 9.3 Gotchas verified in this repo

1. **`bg-subtle` / `bg-hover` / `bg-active` generate no CSS** (§2.1). Use `bg-surface-subtle` /
   `-hover` / `-active`. Recommended in the same PR: fix the 12 existing sites — without it, the new
   viewer column band will have a fill while the rules-manager band (same intended treatment) does
   not. It is a pure find-and-replace with no token change.
2. **`focus:`/`hover:` variants out-specify plain colour classes** (0,2,0 vs 0,1,0). Any destructive
   ghost button or menu item needs its danger ink restated in the interactive variant (§3.1).
3. **Alpha modifiers on `var()` tokens are silently dropped** in Tailwind 3.4: `resolveConfig`
   calls `withAlphaValue(color, alpha, toColorValue(color))` and `parseColor('var(--x)')` returns
   `null`, so the un-alpha'd colour is returned **[verified at
   `node_modules/.pnpm/tailwindcss@3.4.19/…/lib/util/resolveConfig.js:167` and
   `…/lib/util/withAlphaVariable.js:26-31`]**. `settings.tsx:422`'s `border-danger-fg/40` therefore
   paints a full-strength border. **New code must not use `/alpha`** — use a token that already
   encodes the tint (`accent-subtle-bg`, `*-bg`) or a solid border token.
4. **`--bg-subtle` equals `--bg-canvas` in light theme** (`--slate-50`), so a "subtle" fill is
   invisible on a canvas-backed pane. That is why the export strip uses `accent-subtle-bg` (§3.3) and
   the viewer's recess uses `bg-canvas` (§3.7).
5. **`text-overline` already sets `letter-spacing: .06em`**, and the existing bands add
   `tracking-wide` (.025em), which wins and *reduces* the tracking. Keep writing
   `text-overline uppercase tracking-wide` for consistency with `rules-manager.tsx:232` /
   `inspector.tsx:228`; dropping `tracking-wide` product-wide is a separate cleanup, not this PR.
6. **`:focus-visible` also sets `border-radius: 4px`** in the base layer **[existing —
   `globals.css:247-251`]**; utilities out-rank it, so any element that needs a non-4 px radius while
   focused must carry an explicit `rounded-*` class (all specs above do).
7. **`Button asChild` does not forward `disabled`** **[existing — `button.tsx:53`]** — the viewer's
   Refresh and the "Open in new tab" link must be genuinely non-disableable or use a real `<button>`.

---

## 10. UX handoff notes

Items where this visual layer touches ux.md and needs `ui-ux`/PM reconciliation. None of them change
ux.md's information architecture; several are consequences of docs written after it.

1. **BLOCKING — the Share dialog must be re-specified for hashed codes.** `architecture.md` D9/D10/D11
   (endorsed by `security.md` §4's note) make the plaintext code exist only in the 201 body and revoke
   happen by integer `id`. ux.md §2.5/§3.4's per-row `MockUrlChip`, per-row copy and per-row **Preview**
   are therefore unimplementable, and `share.row.open*` copy keys become dead. §3.6 above designs the
   one-time-panel variant; ux.md needs matching structure + copy (a "shown once" line, and Preview
   moved into the one-time panel). This is ux.md's own gap #15, now decided.
2. **New element: a "Read-only" `neutral-chip` in the viewer card header** (§3.7). Scroll-persistent
   cue for a banner that is in flow. Needs one copy key (`viewer.readOnlyChip` = "Read-only"). Please
   bless or reject.
3. **New copy needed for the F3 import diff** (`security.md` AC-S21): a section label, the
   `target_url` warning line, and an "unchanged fields omitted" note if the list is filtered. ux.md's
   §5.16 table predates that AC.
4. **New copy needed for the one-time link note** (something like `share.created.onceHint` — "This is
   the only time the link is shown. If you lose it, revoke it and create a new one.") and for the
   in-panel open action.
5. **Share count badge treatment differs from ux.md §2.5**: `rounded-pill bg-neutral-chip-bg`
   instead of `rounded-xs bg-subtle`, because `bg-subtle` is a dead class and a count reads as a pill.
   Non-structural.
6. **Cancel is disabled, not hidden, during "Preparing file…"** (§3.3/§5.3). ux.md's phase table lists
   Cancel only in the fetching phase; keeping the button mounted-but-disabled avoids a reflow at the
   most anxious moment. Please confirm the copy stays `feed.export.cancel`.
7. **Viewer rows get `min-h-11` (44 px)** rather than the feed's 40 px, for the phone case ux.md
   flagged as uncovered (gap #29). Slight density difference from `FeedRow` is intentional.
8. **The viewer's "link unavailable" page keeps the header and footer but drops the standing banner**
   (§5.6) — there is no shared data on that page to be read-only about. ux.md §3.5 says "full-page
   centred card" without specifying chrome.
9. **Refresh is `variant="secondary"`, not ghost** (§3.7) — the only control on a page shown to
   non-experts should look like a control. ux.md left the variant open.
10. **The destructive-menu-item colour fix also changes the existing rules-manager Delete item**
    (§3.1). It is a visual bug fix inside a file this feature already touches; ux.md's §6 item 2 says
    the pattern is reused "verbatim", so the pattern itself needs updating in both places.
11. **`variant="danger"`'s dark-theme label contrast fix** (§8 finding 1) changes every existing
    confirm dialog's button ink from `#ffffff` to `--slate-950` in dark. Cosmetically visible on four
    shipped dialogs; strictly an a11y improvement.

---

## 11. PRD gaps

Numbered visual requirements the PM must add or clarify. Items marked **BLOCKING** change the design
rather than refining it.

1. **BLOCKING — lock the share-code storage decision in §5 before the dialog is built.**
   `architecture.md` D9/D10/D11 (hashed + integer `id`) contradicts PRD §5.4/§5.5.2/AC-25 (plaintext
   `code`, list shows "the URL, a copy action"). Rewrite AC-24/AC-25 for the one-time-disclosure UI
   and drop "the URL" from AC-25, or the engineer will build a dialog the API cannot feed. Extends
   ux.md gap #15.
2. **Add an AC for the destructive control's visual weight, including its interactive states.**
   ux.md gap #2 asks for ghost + `text-danger-fg`; add that the danger ink must be *retained* on
   hover and keyboard focus (assertable: computed colour of the focused "Clear all" item equals
   `--danger-fg`), and that the disabled state renders `--text-tertiary`.
3. **Add an AC that `/s/:code` contains zero accent-filled controls** — no element with
   `background-color: var(--accent-fill)` and no `variant="primary"` — plus: the page has a `<footer>`,
   has no `h-screen overflow-hidden` shell, and never applies `animate-feed-row-in` / `rail-flash`.
   This is the testable form of "must not read as the dashboard with pieces missing" (AC-43 currently
   only enumerates absent accessible names).
4. **Add a contrast AC for `variant="danger"` in dark theme.** `text-white` on `--red-400-dark` is
   ~1.7:1 **[existing — `button.tsx:21`]**, failing AA on every confirm button in the product and on
   F1's and F4's new ones. Fix: `text-text-on-accent` (6.5:1 light / 8.3:1 dark). Needs an AC because
   it edits a shared primitive four existing dialogs depend on.
5. **Add an AC for the dead background tokens.** `bg-subtle` / `bg-hover` / `bg-active` generate no CSS
   (§2.1), so the rules-manager column band, `CodeBlock`, `JsonTree`, `Tabs`, `Segmented`,
   `ConnectionPill`, the Auto-CRUD chip and feed-row hover/selected render without their intended
   fills. Decide: fix the 12 sites in this batch (recommended — the new viewer band must match the
   rules band) or accept the drift and state it.
6. **Add an AC that the F3 file input is keyboard-focusable *with a visible ring*.** AC-65 requires a
   visible focus ring on every new control; an `sr-only` input's ring is invisible unless projected
   onto its `<label>` (§3.4). Assertable: `Tab` to the import control produces a visible 2 px outline.
7. **Specify the progress contract's visual determinacy** (extends ux.md gap #9/#10): a determinate
   bar with `aria-valuenow`/`aria-valuetext`, a `.tnum` numeric label, a "Preparing file…" phase at
   100 %, a Cancel affordance that stays mounted-but-disabled during that phase, and no layout shift
   between phases.
8. **Approve the "Read-only" chip in the viewer card header and add its copy key** (§3.7). Without it
   the only read-only signal scrolls away on a long transcript.
9. **Add the F3 import-diff copy keys and an AC for the `target_url` emphasis.** `security.md` AC-S21
   mandates a pre-apply diff; nothing in ux.md §5.16 covers a diff's labels, and the
   traffic-affecting field needs a specified visual emphasis (amber row + caption), not just presence.
10. **Add a reduced-motion AC covering the new surfaces**: with `prefers-reduced-motion: reduce`, the
    export strip, `Progress`, the viewer disclosure and every dialog still render and still convey
    state (durations are zeroed, nothing is hidden). The global block exists
    **[existing — `globals.css:283-301`]** but no AC asserts the new components respect it.
11. **Add a touch-target AC for `/s/:code`** (extends ux.md gap #29): row triggers ≥44 px tall, no
    horizontal scroll at 360 px, and the column-header band hidden below `sm`.
12. **Confirm the one accent-per-surface ledger** (§4): Settings keeps Save as its only primary
    (Export/Import secondary), Rules Manager keeps New rule (Add default rule secondary), the Share
    dialog's Create is primary *within the dialog*, the feed pane has none, the viewer has none.
13. **Add `maxWidth.viewer: '920px'` to the design-token list** (or explicitly permit the arbitrary
    class). Three places on the viewer must share the value or the banner, content and footer will
    not align.
14. **Decide whether `insp.body.largeRaw` renders as a caption or an alert** (§3.9). ux.md gap #21
    asks for the Raw default; the *visual* question — quiet caption vs `InlineAlert` — is unanswered,
    and it appears on the unauthenticated page too.
