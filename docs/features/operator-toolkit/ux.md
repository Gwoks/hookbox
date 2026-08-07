# UI/UX: Operator Toolkit (F1–F7)

**Slug:** `operator-toolkit` · **Input:** `docs/features/operator-toolkit/prd.md` (DRAFT)
**Mode:** design proposal. Every placement/interaction below is grounded in a file read in this
repo; deviations from a PRD acceptance criterion are called out inline as **[deviates from AC-n]**
and repeated in §7 PRD gaps.

> **Stack note (correcting the brief).** The task brief described HookBox as server-rendered Jinja
> templates in `templates/`. That directory does not exist. HookBox's UI is a **Vite + React +
> TypeScript SPA**: routes in `src/router.tsx`, screens in `src/screens/*.tsx`, primitives in
> `src/components/ui/*` (Radix + CVA + Tailwind token classes), HookBox-specific components in
> `src/components/hookbox/*`, and **all** user-facing strings in the single copy table
> `src/lib/copy.ts` behind `t()`. This document designs against that reality, which is also what the
> PRD's §6/§7 file lists assume.

**Verification legend:** **[existing — `path:line`]** = read in this repo. **[new]** = to be created.

---

## 1. Screens & components affected

### 1.1 Files touched, and what is reuse vs. new

| Feature | File | Change | Reuse / new |
|---|---|---|---|
| F1, F5 | `src/screens/dashboard.tsx` **[existing — `:289-351` `FeedPane`]** | feed-header action group gains Clear all + Export CSV; new confirm dialog + export progress strip | reuse `Button`, `Menu*`, `Dialog*`, `InlineAlert`, `useToast`; **new** local `ExportProgress` strip |
| F1 | `src/feed/use-feed.ts` **[existing — `:71` `rows`, `:77` `buffer`, `:74` `newCount`]** | expose `clearRows()` that empties `rows`, `buffer.current` and `newCount` | reuse |
| F1 | `src/components/hookbox/confirm-dialog.tsx` **[new]** | extract the confirm pattern currently private to `settings.tsx` **[existing — `:659-705` `ConfirmDialog`]**, **plus an error path** (see §3.1) | **new** (extraction) |
| F2 | `src/components/hookbox/app-shell.tsx` **[existing — `:101`]** | delete the `dash.pathUrl.label` `UrlChip`; fix the module doc-comment **[existing — `:4-5`]** | deletion only |
| F3 | `src/screens/settings.tsx` **[existing — `Section` at `:707-720`]** | new "Configuration" `Section` | reuse `Section`, `Button`, `InlineAlert`, `Dialog*`; **new** `Progress`, hidden file input |
| F3, F5 | `src/components/ui/progress.tsx` **[new]** | one determinate progress bar primitive shared by import + export | **new** (~20 lines, tokens only) |
| F3, F5 | `src/lib/download.ts` **[new]** | one `downloadBlob(filename, mime, bytes)` helper: create object URL → click → **revoke** (AC-12 + AC-49 both require this) | **new** |
| F3 | `src/lib/config-bundle.ts` **[new, PRD §7]** | pure build/validate; UI only consumes its typed result + first-failure message | **new** |
| F5 | `src/lib/csv.ts` **[new, PRD §7]** | pure serializer | **new** |
| F4 owner | `src/components/hookbox/app-shell.tsx` | "Share" control in the sub-header action group + `ShareDialog` mount | reuse `Button`, lucide `Share2` |
| F4 owner | `src/components/hookbox/share-dialog.tsx` **[new, PRD §7]** | mint / list / revoke | reuse `Dialog*`, `CodeBlock`, `MockUrlChip`, `InlineAlert`, `Field`/`Input`, `Button`, `SkeletonLines`, `useToast` |
| F4 public | `src/screens/share-view.tsx` **[new, PRD §7]** | the `/s/:code` viewer | reuse `BrandMark`, `ThemeToggle`, `InlineAlert`, `Tabs*`, `KeyValueRows`, `JsonTree`, `MethodBadge`, `StatusCode`, `ServedByChip`, `SkeletonLines`, `Button`; **new** local `SharedRequestRow` (disclosure semantics — see §2.6) |
| F4 | `src/router.tsx` **[existing — `:35-49`]** | register `/s/:code` **above** the `*` catch-all | reuse |
| F4, F5 | `src/api/client.ts` **[existing — `noAuth` at `:78-81`, 401 bounce at `:99-105`]** | 3 owner + 2 `noAuth: true` share methods | reuse |
| F6 | `src/screens/rules-manager.tsx` **[existing — toolbar `:191-194`, empty state `:218-227`]** | "Add default rule" in both | reuse `Button`, `Tooltip`, `useToast` |
| F7 | `src/components/hookbox/json-tree.tsx` **[existing — `:68-103`]** | **defensive**: default to Raw above a size threshold (see §3.7) | small change |
| F7 | `src/components/hookbox/key-value-rows.tsx` **[existing — `:9` `const REDACTED = '__redacted__'`]** | **bug**: server writes `<redacted>` **[existing — `backend/src/helpers.rs:43`]**, so the redaction pill never renders | one-const fix |
| all | `src/lib/copy.ts` **[existing]** | new keys, §5 below | reuse `t()` |

### 1.2 Primitives reused verbatim (no new design system)

`Button` (variants `primary`/`secondary`/`ghost`/`danger`/`link`, sizes `sm`/`md`/`icon-sm`, `loading`)
**[existing — `src/components/ui/button.tsx`]** · `Dialog`/`DialogContent`/`DialogHeader`/`DialogBody`/
`DialogFooter` (Radix, focus trap + restore, mobile bottom-sheet) **[existing — `src/components/ui/dialog.tsx`]** ·
`Menu`/`MenuTrigger`/`MenuContent`/`MenuItem`/`MenuSeparator` **[existing — `src/components/ui/menu.tsx`]** ·
`Tooltip` **[existing]** · `Switch` **[existing]** · `SkeletonLines` **[existing]** ·
`useToast` (success + danger, 3.2 s) **[existing — `src/components/ui/toast.tsx`]** ·
`InlineAlert` (`info`/`warning`/`danger`, persistent, `role="alert"|"status"`, optional `action`)
**[existing — `src/components/hookbox/inline-alert.tsx`]** · `CodeBlock` + `MockUrlChip` (integrated
`CopyButton`, copy-only, never link-coloured — AC-D19) **[existing — `src/components/hookbox/code-block.tsx`]** ·
`KeyValueRows` · `JsonTree` (Pretty/Raw + copy, degrades to `<pre>` on parse failure) ·
`MethodBadge` · `StatusCode` · `ServedByChip` · `Field`/`Input`/`Label`.

**Exactly two new primitives**, both justified by two callers each: `Progress` (F3 + F5) and
`ConfirmDialog` (F1 + the two existing Settings confirms it is extracted from). Everything else is
composition.

---

## 2. Layout & placement

### 2.1 The structure we are placing into

`AppShell` **[existing — `src/components/hookbox/app-shell.tsx:72-151`]** is three bands, and the
sub-header has a deliberate split that this design preserves:

```
┌─ header  (border-b, bg-surface, px-4 py-2) ──────────────────────────────────┐
│ BrandMark · EndpointSwitcher                     ThemeToggle · AccountMenu   │
├─ sub-header (border-b, bg-surface, px-4 py-2.5) ─────────────────────────────┤
│ LEFT = the endpoint SUBJECT            │ RIGHT = ACTIONS                     │
│ Mock URL chip · [Local path chip ✂F2]  │ headerExtra · Rules · ⚙ · +New rule │
│ · Auto-CRUD chip · Tunnel badge        │ ▲ F4 "Share" goes HERE              │
├─ main#main (flex-1, overflow-hidden) ────────────────────────────────────────┤
│ /d/:token → SplitPane[ FeedPane │ Inspector ]                                │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 F2 — remove the "Local path" chip

Delete line `:101`. The left cluster becomes `Mock URL chip · Auto-CRUD · Tunnel`. Keep the `UrlChip`
helper **[existing — `:153-169`]** even though it now has one caller (minimal diff; its skeleton
branch `h-5 w-40 animate-pulse` is still the sub-header's loading state). Settings → Identity keeps
the `CodeBlock` unchanged (AC-8) **[existing — `src/screens/settings.tsx:255-261`]**, so nothing is
lost — only relocated. No migration copy, no "moved to Settings" hint: the chip's replacement is one
click away on a screen the operator already knows.

This deletion is what makes room for F4's Share control without the sub-header wrapping at tablet
widths — F2 and F4 should land in the same PR.

### 2.3 F1 + F5 — the Live Feed header action group

Today: `[ "N new" pill? ] [ Pause/Resume ]` in a `flex items-center gap-2` inside a header whose pane
can be as narrow as **360 px** (`min-w-feed`) **[existing — `src/components/hookbox/split-pane.tsx:53`,
`tailwind.config.ts:113-115`]**. The left side already holds `h2 "Live feed"` + `"Showing N of last
100"`. Adding two more labelled buttons overflows.

**Options considered**

| # | Approach | Trade-off |
|---|---|---|
| 1 | Three labelled ghost buttons inline (literal AC-1/AC-46) | Overflows below ~560 px; a destructive control sits permanently one click from the pause button an operator jabs at constantly |
| 2 | Icon-only buttons + tooltips | Fits, but an unlabelled trash icon next to a live data list is a mis-click magnet |
| 3 | Overflow `Menu` ("Feed actions") holding Clear all + Export CSV; Pause stays inline | Fits at 360 px; reuses the rules-manager row-menu pattern exactly; puts the irreversible action one deliberate step deeper (mitigates R7); costs one extra click for export (a rare action) |
| 4 | Responsive label collapse (`sr-only md:not-sr-only`) | Works, still 3 controls' worth of hit area at 360 px |
| 5 | Leave Clear all in Settings only | Contradicts the PRD's whole premise ("on the screens where the operator already is") |

**Recommended: 3, with 4 as the documented fallback.** `Pause/Resume` stays a first-class inline
control (high frequency, carries `aria-pressed`). Clear all + Export CSV move into a single
`Menu` whose trigger is `Button variant="ghost" size="icon-sm"` + `MoreHorizontal` +
`aria-label={t("feed.actions.menu.aria")}` — byte-for-byte the pattern at
`src/screens/rules-manager.tsx:274-298`. **[deviates from AC-1/AC-46, which specify "to the left of
the pause/resume control" — gap #1]**

```
┌─ feed header (border-b, px-3 py-2) ─────────────────────────────┐
│ Live feed  Showing 37 of last 100      [3 new] [⋯] [⏸ Pause]    │
└─────────────────────────────────────────────────────────────────┘
                                          │
                              ┌───────────▼──────────────┐
                              │ Export CSV               │
                              │ ──────────────────────── │
                              │ Clear all   (danger-fg)  │
                              └──────────────────────────┘
```

Menu contents, in this order (destructive last, separated — same shape as the rules row menu where
`Delete` is last with `className="text-danger-fg"`):

1. `MenuItem` **Export CSV** — `disabled={rows.length === 0 || exporting}`
2. `MenuSeparator`
3. `MenuItem className="text-danger-fg"` **Clear all** — `disabled={rows.length === 0 || exporting}`

The **trigger stays enabled** even on an empty feed so keyboard users can open it and hear the items
announced as disabled; a `div` of `text-caption text-text-tertiary` under the items carries
`t("feed.actions.emptyHint")` when `rows.length === 0`.

**Fallback layout (if the PM keeps AC-1's literal placement):** three inline `Button variant="ghost"
size="sm"`, order `[Clear all] [Export CSV] [Pause]`, each with `<Icon/>` + `<span className="sr-only
md:not-sr-only">` — the exact pattern the Settings link in the sub-header already uses **[existing —
`app-shell.tsx:128-135`]**. Clear all is **ghost + `text-danger-fg`**, *not* `variant="danger"`:
filled red is reserved in this codebase for confirm buttons inside a dialog and for the Settings
danger zone **[existing — `settings.tsx:331`, `:433-440`]**, and design.md's "one accent button per
surface" logic applies equally to filled destructive fills. **[gap #2]**

The F5 progress strip is a **new band between the feed header and the feed list**, matching the
offline banner's shape **[existing — `dashboard.tsx:264-274`]** but at the feed pane's `px-3`:

```
┌─ feed header ───────────────────────────────────────────────────┐
├─ export strip (border-b, px-3 py-2) ────────────────────────────┤
│ ▓▓▓▓▓▓▓▓░░░░░░░░░░  Exporting 41 of 100…          [Cancel]      │
├─ feed list (unchanged, still scrolls, still live) ──────────────┤
```

Non-modal on purpose: the export reads data the operator is watching; blocking the screen for
10–40 s of background fetches would be hostile, and a modal implies the feed has stopped (it has
not).

### 2.4 F3 — the Settings "Configuration" section

Settings' current vertical order is *form fields → **Save** → immediate-effect operations*: Identity,
Proxy, Auto-CRUD, Default response, Simulated conditions, Auto-CORS, **Save**, Retention & state,
Danger zone **[existing — `settings.tsx:233-442`]**.

Export/import take effect immediately and are **not** part of the unsaved form, so the new `Section`
goes **after Save, before Retention & state**:

```
… Auto-CORS ┈┈┈┈
            [ Save ]            ← everything above needs Save
┌─ Configuration ────────────────────────────────────────────────┐
│ Move this endpoint's settings and rules to another endpoint…   │
│ [ Export config ]  [ Import config… ]                          │
│ Reads a hookbox-config JSON file. Settings are replaced;       │
│ rules are added.                                               │
└────────────────────────────────────────────────────────────────┘
… Retention & state · Danger zone
```

Same `Section` component, same `Button variant="secondary" size="sm"` + `flex flex-wrap gap-2` row as
Retention & state **[existing — `settings.tsx:400-415`]**, so the band reads as one family of
"operations".

### 2.5 F4 owner-side — Share control placement

AC-23 puts Share "immediately after the Mock URL chip", i.e. inside the **subject** cluster.

| # | Placement | Trade-off |
|---|---|---|
| 1 | Left cluster, after the Mock URL chip (literal AC-23) | Maximum proximity to the thing being shared; but injects an *action* into the identity row, and at tablet width it shoves the chip's `CopyButton` onto a second line |
| 2 | Right action cluster, first (before Rules) | Honours the documented left=subject / right=actions split **[existing — `app-shell.tsx:4-7` doc-comment]**; slightly lower proximity |
| 3 | Settings screen only | Kills discoverability; contradicts the PRD |
| 4 | Inside the account or endpoint menu | Hides a feature whose whole point is reaching for it mid-conversation |
| 5 | **Right action cluster, first, with an active-link count badge** | Option 2 plus a persistent, glanceable "a public link exists" signal — which for a feature that publishes captured traffic is a safety affordance, not decoration |

**Recommended: 5.** `Button variant="ghost" size="sm"` + lucide `Share2` + label "Share", and when
`activeShareCount > 0` a count badge using the Auto-CRUD chip's exact treatment (`rounded-xs
bg-subtle px-1.5 py-0.5 text-caption text-text-secondary` **[existing — `app-shell.tsx:104-107`]**).
**[deviates from AC-23 — gap #12]**

```
│ …  Mock URL [https://…/e/ab12  ⧉]  ⛁ Auto-CRUD  📡 Tunnel connected │
│                        [Live] [⇱ Share 2] [Rules] [⚙] [+ New rule]  │
```

The count needs one `GET /api/endpoints/{token}/shares` per owner-screen mount. `AppShell` is
presentational today (it receives `endpoint`/`endpoints` as props), so either (a) `AppShell` takes a
new optional `shareCount?: number` prop and each screen fetches, or (b) `AppShell` does the one small
fetch itself. Recommend **(b)** with graceful degradation: on any failure render plain "Share" with no
badge. If the architect wants `AppShell` to stay fetch-free, drop the badge — the rest of the design
is unaffected. Share is **never disabled**, including on a zero-traffic endpoint: "here's where you
can watch it arrive" is a legitimate first move.

**Share dialog anatomy** (`share-dialog.tsx`, `DialogContent` = `w-[min(560px,92vw)]`, bottom sheet
below `sm` — both free from the primitive):

```
┌─ Share read-only ──────────────────────────────────── [×] ─┐
│ A share link lets anyone with the URL read this endpoint's │  DialogBody
│ recent requests in a browser. No account, no sign-in, no   │
│ way to change anything.                                    │
│                                                            │
│ ⚠ A share link exposes captured traffic                    │  InlineAlert warning
│   Whoever holds the URL sees each request's method, path,  │  role="status", PERSISTENT,
│   status, headers, query and body — and the response       │  ABOVE the create button (R2)
│   headers and body HookBox returned. Authorization,        │
│   Cookie and X-Owner-Id request headers are hidden;        │
│   nothing else is. Response headers are shown exactly as   │
│   sent, including any Set-Cookie from your upstream.       │
│                                                            │
│ Label (optional)                                           │  Field + Input, ≤80 chars
│ [ e.g. For Acme support ticket #421            ]           │
│                    [ Create share link ]                   │  Button primary
│                                                            │
│ ── Your share link ───────────────────────────── (on 201) ─│  role="status" aria-live=polite
│ ┌──────────────────────────────────────────────────────┐   │
│ │ https://hookbox.example/s/pK3n…8Qz              ⧉   │   │  CodeBlock (mono-lg + CopyButton)
│ └──────────────────────────────────────────────────────┘   │
│ Copy it now — you can always come back to this dialog.     │
│                                                            │
│ ── Active links (2 of 10) ─────────────────────────────────│
│ ┌──────────────────────────────────────────────────────┐   │
│ │ Acme ticket #421            Created 2h · Opened 5m   │   │
│ │ [https://…/s/pK3n…8Qz ⧉]        [↗ Preview] [Revoke] │   │  MockUrlChip + ghost buttons
│ ├──────────────────────────────────────────────────────┤   │
│ │ Untitled link               Created 3d · Never opened│   │
│ │ [https://…/s/w7Ba…1Lm ⧉]        [↗ Preview] [Revoke] │   │
│ └──────────────────────────────────────────────────────┘   │
├────────────────────────────────────────────────── [Done] ──┤  DialogFooter, ghost
└────────────────────────────────────────────────────────────┘
```

Details that matter:

- The minted URL gets a full **`CodeBlock`** (`mono-lg`, integrated `CopyButton`) — the same
  "here is your thing, copy it" treatment as the mock URL in the feed empty state and Settings →
  Identity **[existing — `dashboard.tsx:398`, `settings.tsx:250-253`]**. List rows get the compact
  **`MockUrlChip`**. Both are copy-only, so the URL never renders as accent/link text (AC-D19) —
  navigation is an explicit separate affordance.
- **Preview** is a real `<a target="_blank" rel="noopener noreferrer">` (via `Button variant="ghost"
  size="sm" asChild`, the `asChild`-link pattern from `app-shell.tsx:121-123`). For a feature that
  publishes data, "see exactly what they see" is the single most reassuring control in the dialog.
- Timestamps use `title={iso}` + a relative label, reusing `FeedRow`'s `relTime` shape **[existing —
  `src/components/hookbox/feed-row.tsx:23-34`]** (extract it to `src/lib/time.ts` rather than
  copy-pasting).
- **Revoke is irreversible** — §5.1 says a revoked code can never be re-minted — so a single click
  must not do it, and Undo is impossible. Use an **inline two-step** inside the row (Revoke →
  "Revoke this link? It stops working immediately and can't be brought back." + `[Cancel] [Revoke]`)
  rather than a nested `Dialog`: no second focus trap, no scrim-on-scrim. **[gap #13]**
- At the cap, Create is **pre-emptively disabled** with `t("share.limit.reached", {max})` shown as
  `text-caption text-text-tertiary` under it, *and* a server 422 `detail` is still surfaced in an
  `InlineAlert variant="danger"` if it happens anyway (AC-27). **[gap #14]**

### 2.6 F4 public viewer — `/s/:code`

The hard requirement: it must not read as "the dashboard with pieces missing". Three levers do the
work — **different chrome, different shape, an explicit standing banner**.

| # | Layout | Trade-off |
|---|---|---|
| a | Reuse `SplitPane` + a read-only `Inspector` | Highest reuse, but it *is* the dashboard minus chrome; and `min-w-feed: 360px` + drag-to-resize is hostile on the phone a link recipient is very likely holding |
| b | Single centred column, row expands in place | Mobile-first, obviously a different object, no second route, no selection state to reconcile against polling |
| c | List → sub-route `/s/:code/r/:id` | Cleanest deep links, but adds a route the PRD's §5 doesn't define, and full-page transitions on a poll-driven page |
| d | Two-pane ≥md, stacked sheet <md | Needs responsive machinery the dashboard itself has never implemented |
| e | **Single centred column styled as a document/transcript card** | (b) plus a visual frame that reads as an artifact someone sent you, not an app |

**Recommended: e.** A centred `max-w-[920px]` column on `bg-canvas`, holding one `rounded-md border
border-border bg-surface` card. Rows live inside the card under a column-header band that reuses the
rules-manager header treatment (`bg-subtle`, `text-overline uppercase tracking-wide
text-text-tertiary` **[existing — `rules-manager.tsx:232-238`]**). Clicking a row expands the detail
**in place** as a disclosure.

```
 [skip to content]
┌─ header (px-6 py-4, no border) ─────────────────────────────────────┐
│ 🪝 HookBox                                            [☀/🌙]        │   BrandMark (NOT a link),
└─────────────────────────────────────────────────────────────────────┘   ThemeToggle only
┌─ standing banner (border-y, px-4 py-2, full width) ─────────────────┐
│ ℹ Read-only shared view                                             │   InlineAlert info,
│   Someone shared this HookBox endpoint's recent requests with you.  │   role="status",
│   You're not signed in, and nothing on this page can be changed.    │   never dismissible
└─────────────────────────────────────────────────────────────────────┘
        ┌─ main#main · mx-auto max-w-[920px] px-4 py-6 ─────────────┐
        │ Shared requests                              ← <h1>, STATIC│
        │ Endpoint: checkout-api · Capturing since 12 Mar 2026 ·     │   text-body-sm secondary,
        │ 1,284 requests received in total                            │   name is truncated + quoted
        │                                                            │
        │ ┌──────────────────────────────────────────────────────┐   │
        │ │ Showing 37 of the last 100   Updated 3s  [⟳ Refresh] │   │   card header row
        │ ├──────────────────────────────────────────────────────┤   │
        │ │ METHOD  PATH          STATUS  SERVED   MS    WHEN    │   │   text-overline, bg-subtle
        │ ├──────────────────────────────────────────────────────┤   │
        │ │ ▸ POST  /webhooks/pay   200  ⎇ rule   12ms   4s      │   │   button aria-expanded=false
        │ │ ▾ GET   /health         404  ○ default  3ms   1m      │   │   aria-expanded=true
        │ │ ┌─ region id="req-42" ─────────────────────────────┐  │   │
        │ │ │ [Headers] [Query] [Body] [Response]              │  │   │   Tabs — NO trace tab
        │ │ │ KeyValueRows / JsonTree …                        │  │   │
        │ │ └──────────────────────────────────────────────────┘  │   │
        │ │ ▸ POST  /webhooks/pay   500  ⚡ chaos   8ms   2m      │   │
        │ └──────────────────────────────────────────────────────┘   │
        │ Updates every 5 seconds while this tab is open.            │   text-caption tertiary
        └────────────────────────────────────────────────────────────┘
┌─ footer (px-6 py-6, border-t) ──────────────────────────────────────┐
│ Served by HookBox — a self-hosted request inspector.                │
│ This page is read-only.                                             │
└─────────────────────────────────────────────────────────────────────┘
```

Why each choice:

- **Chrome comes from `/cli`, not `AppShell`.** `/cli` is the repo's existing public-ish page shape:
  `flex min-h-screen flex-col bg-canvas` + a slim `header` with `BrandMark` + `ThemeToggle` + a
  `main#main` **[existing — `src/screens/cli.tsx:67-84`]**. Copying that skeleton satisfies AC-43 by
  construction: there is no switcher, no account menu, no `headerExtra`, no Rules/Settings/New rule,
  no connection pill.
- **The `<h1>` is always the static string** `t("viewer.title")` — "Shared requests". The endpoint
  name is user-controlled and this page is unauthenticated, so a name like *"Session expired — enter
  your password to continue"* must never be able to occupy the page's most authoritative slot.
  The name renders only as `Endpoint: <truncated name>` in secondary body text, `title`-attributed
  for the full value. `document.title` is likewise static (`"Shared requests · HookBox"`). **[gap #16]**
- **The banner is a standing, non-dismissible `InlineAlert variant="info" role="status"`** spanning
  full width above the content — the one element that is impossible to miss on arrival and still
  present after scrolling back up. `info`, not `warning`: nothing is wrong.
- **`ThemeToggle` stays.** It is a viewer preference, reads no data, mutates nothing, and is already
  on `/cli`. It is not an "owner affordance" in AC-43's sense.
- **No `SplitPane`, no drag handle, no `role="listbox"` selection.** Disclosure rows mean the poll
  can prepend new rows without disturbing which row is open, and there is no "selected row scrolled
  away" state to reason about.
- **No "State & Tracing" tab** — `trace` / `state_snapshot` / `matched_rule_id` are not in the
  projection (AC-34). Four tabs only, reusing `insp.tab.headers|query|body|response`.
- **No arrival animation.** `animate-feed-row-in` + `rail-flash` is the *live* feed's signature
  **[existing — `tailwind.config.ts:154-173`]**; a 5 s poller must not cosplay as a live stream.
  New rows simply appear and the "Showing N" + "Updated {when}" line changes.
- **`SharedRequestRow` is local to `share-view.tsx`, not a `FeedRow` change.** `FeedRow` is
  `role="option"` + `aria-selected` for the dashboard's listbox **[existing — `feed-row.tsx:50-53`]**;
  a disclosure needs `<button type="button" aria-expanded aria-controls>`. Reuse the *class strings*
  and the three chips, not the component — that keeps the dashboard's primitive untouched.
- **Mobile:** the row grid degrades with a `sm:contents` wrapper — below `sm` the trailing meta
  (status · served-by · ms · when) wraps to a second line inside the same grid cell; at `sm+` the
  wrapper becomes `display: contents` and the six columns line up exactly like `FeedRow`. Cheap, no
  new breakpoint machinery. **[gap #29 — no AC covers the phone case]**

### 2.7 F6 — Rules Manager placements

Toolbar (right side, before "New rule"): `Button variant="secondary" size="sm"` "Add default rule".
Secondary is mandatory — `variant="primary"` is the single accent button per surface and "New rule"
already owns it **[existing — `button.tsx:1-6` doc-comment, `rules-manager.tsx:191-194`]**.

```
[←] Rules                       [ Add default rule ] [ + New rule ]
```

Empty state: the card at `rules-manager.tsx:218-227` currently has title + body and **no action at
all**. Add the same secondary button, centred under the body, plus a one-line helper explaining what
the rule does. Do **not** add a second CTA there — "New rule" is in the toolbar directly above.

```
┌─ rounded-md border bg-surface p-8, text-center ─────────────┐
│                    No rules yet                             │
│  Without a rule, unmatched requests fall through to         │
│  Auto-CRUD, then your tunnel, then your proxy target,       │
│  then the default response.                                 │
│                                                             │
│              [ Add default rule ]                           │
│  Answers anything no other rule matches with a 200 and a    │
│  placeholder JSON body.                                     │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Interaction & states

Every state below names the primitive that renders it. "toast" = `useToast` (3.2 s, auto-dismiss,
success or danger); "inline alert" = `InlineAlert` (persistent). The rule of thumb the codebase
already follows: **short confirmations toast, anything the operator must read or act on is an inline
alert.**

### 3.1 F1 — Clear all

| State | Rendering |
|---|---|
| unavailable | Menu items disabled when `rows.length === 0` (AC-1) or while an export is in flight; hint line `t("feed.actions.emptyHint")` under them when empty |
| confirming | `ConfirmDialog`: `DialogHeader` `t("feed.clearAll.confirm.title")`, `DialogBody` `t("feed.clearAll.confirm.body", {endpoint})`, `DialogFooter` `[Button ghost Cancel] [Button danger "Clear all"]` — the rule-delete shape **[existing — `rules-manager.tsx:317-336`]**, no typed token (AC-2) |
| cancelled | `Esc` / scrim / Cancel → Radix closes, focus returns to the menu trigger, **zero** requests (AC-3) |
| in flight | confirm button `loading` (spinner, width preserved, `aria-busy`); Cancel disabled |
| success | one `DELETE /api/endpoints/{token}/requests`; dialog closes; success toast; `clearRows()` empties `rows` + `buffer` + `newCount` **client-side immediately**, so the feed shows `FeedEmpty` and the "N new" pill is gone in the same frame (AC-4, AC-5) |
| failure | dialog **stays open**, danger toast, an `InlineAlert variant="danger"` inside `DialogBody` carrying the server `detail`, no rows removed (AC-6) |

**Extraction defect to fix while extracting.** The Settings `ConfirmDialog` does
`await onConfirm(); onClose()` inside `try { } finally { }` with **no `catch`** **[existing —
`settings.tsx:688-697`]**. On rejection the dialog does stay open (good), but the rejection escapes as
an unhandled promise rejection and **the user is told nothing**. The extracted
`src/components/hookbox/confirm-dialog.tsx` must catch, keep the dialog open, render the error in the
body, and let the caller toast. This also fixes the two existing Settings confirms. **[gap #28]**

**Confirm copy carries no count.** AC-2 only requires naming the endpoint, and a count would be
wrong twice over: `rows.length` drifts while the dialog is open (the feed keeps streaming), and
`endpoint.request_count` is a monotonic lifetime counter on `endpoints`
**[existing — `backend/migrations/0001_init.sql:31`]**, not the number of stored traces (capped at
`TRACE_CAP` = 100). The existing `set.confirm.clearHistory.body` — *"All {n} traces … will be
removed"* fed from `request_count` **[existing — `settings.tsx:449`]** — is misleading for any
endpoint that has ever taken more than 100 hits. **[gap #3]**

**Interaction with F5:** Clear all and Export CSV must be mutually exclusive. Deleting the rows an
export is mid-way through fetching turns every remaining row into a `pending`/`unavailable` sentinel
and produces a mostly-useless file. Both menu items disable while `exporting === true`. **[gap #4]**

### 3.2 F5 — Export CSV

Phases, all rendered in the one non-modal strip of §2.3:

| Phase | Rendering |
|---|---|
| idle | menu item enabled iff `rows.length > 0 && !exporting`; item tooltip/hint `t("feed.export.note", {n})` — *"Exports the {n} requests listed now — newest first"*, because the snapshot is taken at click time and the live feed keeps growing |
| fetching | `Progress` (`role="progressbar"`, `aria-valuemin=0`, `aria-valuemax=total`, `aria-valuenow=done`, `aria-valuetext` = the localized label) + `t("feed.export.progress", {done, total})` + `Button variant="ghost" size="sm"` Cancel. Bounded concurrency 4 (AC-47) |
| serialising | same strip, label swaps to `t("feed.export.preparing")` with the bar at 100%. **Required**: 100 rows × two ≤256 KB bodies can be ~50 MB of string building + `Blob` construction, a genuinely multi-second main-thread pause. Without this phase the UI looks hung right at the finish line. **[gap #10]** |
| done | strip disappears; `downloadBlob()` fires and revokes the object URL (AC-49); toast `t("feed.export.done", {n})`, or `t("feed.export.done.partial", {n, m})` when any row lacked detail (AC-52) |
| cancelled | `AbortController.abort()`, strip disappears, **no file**, quiet toast `t("feed.export.cancelled")` — silence after an explicit cancel reads as a bug (AC-48) |
| per-row failure | never aborts; the four detail cells carry `pending` (404) or `unavailable` (anything else); counted into `m` (AC-52) |
| 401 | the client clears the session and bounces **[existing — `client.ts:99-105`]**; the screen unmounts, the abort fires from cleanup, no file, no toast (AC-53) |
| unmount / route change | `AbortController.abort()` in the effect cleanup. No `beforeunload` prompt — too aggressive for a re-runnable read |

Progress announcements: the `progressbar` node carries the live numbers for AT users that track it,
but the visible label is **not** an `aria-live` region — 100 polite announcements would be abusive.
A separate `sr-only` `aria-live="polite"` node announces only at start, ~25/50/75%, and completion.

Only one export at a time; the menu item is disabled while in flight (no queue, no second strip).

### 3.3 F3 — Export / import config

**Export** (`Button secondary` "Export config"):

1. Fetches **server** state — `GET /api/endpoints/{token}` + `GET .../rules` — never the dirty form.
   Exporting unsaved form values would produce a file that disagrees with the endpoint it names.
   When the form is dirty, `t("set.config.export.dirty")` shows as `text-caption text-text-tertiary`
   under the buttons: *"Exports the saved configuration — save your changes first to include them."*
   **[gap #6]**
2. `downloadBlob("hookbox-config-<token>.json", "application/json", …)` (AC-12), object URL revoked.
3. Failure → `InlineAlert variant="danger"` in the section (not a toast: the operator may need to
   retry and read why).

**Import** — a five-step pipeline, and **nothing hits the network before step 4**:

| Step | UI |
|---|---|
| 1 pick | Visually-hidden native `<input type="file" accept="application/json,.json" className="sr-only">` + a `<label htmlFor>` styled with the exported `buttonVariants({variant:'secondary', size:'sm'})` **[existing — `button.tsx:68`]**. `sr-only` (not `hidden`) keeps the input focusable and announced; the label makes the whole control clickable. Do **not** hand-roll `ref.current.click()` behind a `<button>` — that loses the native accessible name |
| 2 validate | Parse + `configBundleSchema` entirely client-side. On failure: `InlineAlert variant="danger" role="alert"` naming the **first** failing field/index (AC-16), plus "Choose another file". **Zero** `PATCH`/`POST` issued — assertable |
| 3 confirm | `ConfirmDialog`-shaped preview **before any write**: `exported_at`, the bundle's endpoint name, `rules.length`, and the two things AC-18/AC-20 make surprising: *"Replaces the nine settings on this endpoint with the values in the file, then **adds** {n} rules to the {existing} already here. Nothing in the rules list is replaced or deleted."* Confirm button is `variant="primary"`, **not** `danger` — this codebase reserves filled danger for irreversible deletes, and import is recoverable **[gap #7]** |
| 3b dirty guard | If the form above has unsaved edits, the same dialog adds `t("set.config.confirm.dirty")`: *"You have unsaved changes on this screen. Importing discards them."* The PATCH + re-fetch of AC-20 would otherwise silently throw the operator's edits away **[gap #5]** |
| 4 apply | Both buttons disabled (AC-20). `Progress` + label: `t("set.config.import.progressConfig")` → `t("set.config.import.progressRules", {i, n})`. Determinate: `n` is known before the first request |
| 5 settle | Success → success toast + re-fetch `GET /api/endpoints/{token}`, and the form is **remounted** (`key={configVersion}`) so every field shows server truth rather than stale local state. Section shows `t("set.config.import.done", {n})` |

Partial failure (AC-19) is a **persistent `InlineAlert variant="danger"`**, never a toast — it carries
five facts and must survive scrolling and re-reading:

```
⚠ Settings were applied and 4 of 7 rules were created.
  Rule 5 ("Refund webhook") failed: body_template exceeds the 256 KB cap.
  No rule after it was attempted, and nothing was rolled back.
                                                    [ View rules ]
```

`View rules` is a `Button variant="secondary" size="sm" asChild` → `<Link to={`/d/${token}/rules`}>`,
so the operator can see the deterministic prefix immediately. Config-step failure uses the same
alert with `t("set.config.import.failedConfig")` and states that **zero** rules were created.

### 3.4 F4 owner — Share dialog

| State | Rendering |
|---|---|
| opening | `SkeletonLines lines={3}` in the list region, `aria-busy`, sr-only `t("share.list.loading.aria")` |
| empty | in-body centred `h3` + body: *"No share links yet"* / *"Create one when you need to show someone what their webhook actually sent."* |
| list | newest first (server order, AC-25); each row: label-or-`t("share.row.untitled")`, `Created {when}` + `Opened {when}`/`Never opened`, `MockUrlChip`, `[Preview]`, `[Revoke]` |
| creating | Create `loading`; label input disabled |
| created | `CodeBlock` block appears above the list inside a `role="status" aria-live="polite"` region announcing `t("share.toast.created")`; row also prepends to the list. **Focus is not moved** — the operator may still be typing a label |
| label invalid (>80) | `Field error` (red border + `role="alert"` message), Create disabled |
| at cap | Create disabled + `t("share.limit.reached", {max})`; a server 422 still surfaces its `detail` in an `InlineAlert variant="danger"` (AC-27) |
| revoke armed | row swaps to the inline confirm; `Cancel` restores; `Esc` in the row cancels the arm without closing the dialog |
| revoking | row's Revoke `loading`; the rest of the list stays interactive |
| revoked | row removed without a reload (AC-26) + success toast |
| revoke failed | row restored, row-level `text-body-sm text-danger-fg` message + danger toast: *"Couldn't revoke the link. It's still active."* — the "it's still active" clause matters; an ambiguous failure on a revocation is a security-relevant ambiguity |
| list load failed | `InlineAlert variant="danger"` + Retry (the `rules.error.*` pattern **[existing — `rules-manager.tsx:204-216`]**) |

### 3.5 F4 public viewer — `/s/:code`

**Route gate.** `/s/:code` must render with **no** session and must never redirect to `/`, unlike
`/d/:token` **[existing — `dashboard.tsx:136-140`]**, and must never create one (AC-41). Both fetches
go through `noAuth: true` so a share 401/404 can never call `session.clear()` or bounce a logged-in
owner out of their own tab (AC-42) **[existing — `client.ts:78-81`, `:99-105`]**.

| State | Rendering |
|---|---|
| loading | banner + `<h1>` render immediately (the frame is not data-dependent); card body = `SkeletonLines lines={8}`, `aria-busy="true"`, sr-only `t("viewer.loading.aria")` |
| empty | in-card centred `t("viewer.empty.title")` / `t("viewer.empty.body")`. **Must not reuse `FeedEmpty`** — it renders `absolutize(mockUrl)` in a `CodeBlock` plus a curl sample **[existing — `dashboard.tsx:384-413`]**, which would leak `mock_url` and violate AC-34. **[gap #17]** |
| list | rows collapsed; header line "Showing {n} of the last 100" + "Updated {when}" + Refresh |
| row expanded | disclosure region with 4 `Tabs`; detail fetched lazily on first expand and cached per id |
| detail loading | `SkeletonLines lines={6}` inside the open region |
| detail 404 | `InlineAlert variant="info"`: *"This request is no longer available"* / *"HookBox keeps the last 100 requests for 24 hours. This one has aged out."* There is **no** "pending" state here (unlike the owner Inspector at `inspector.tsx:63-66`): the list came from the DB, not a WS broadcast, so a 404 means gone, not not-yet-written |
| detail error | `InlineAlert variant="danger"` + Retry inside the region |
| unavailable (404 on the list) | **full-page** centred card mirroring the dashboard's not-found/gone shape **[existing — `dashboard.tsx:143-158`]**: `t("viewer.unavailable.title")` / `t("viewer.unavailable.body")`. **One message for unknown + revoked + deleted** (AC-36) — no Retry (retry cannot help), and nothing in the copy or the state machine may distinguish the three. A single neutral `Button variant="link"` → `/` labelled *"What is HookBox?"* |
| rate-limited (429) | `InlineAlert variant="warning"` with the `Retry-After` seconds counting down; **polling pauses** for that window then auto-retries once; the stale list stays visible underneath |
| error (other/network) | `InlineAlert variant="danger"` + Retry, last-known rows kept visible — the offline-tolerant pattern the dashboard already uses **[existing — `dashboard.tsx:264-274`]** |

**Refresh behaviour (AC-45), stated honestly in the UI.** Poll `GET /api/share/{code}/requests` every
5 s **only while `document.visibilityState === 'visible'`**; stop on `visibilitychange → hidden` and
on unmount. Zero WS, zero `EventSource`. Because the cadence is a real behaviour the viewer will
notice, the page says so: `t("viewer.updating")` under the card while visible, swapping to
`t("viewer.updating.paused")` when hidden/backgrounded, plus an explicit `[⟳ Refresh]` button for
people who want control. **[gap #18]**

Polling must never disturb reading: the expanded row id is preserved across polls; if the expanded
row ages out of the list, the region **stays open** and shows the detail-404 state rather than
collapsing and yanking content out from under the cursor.

**Absence audit (AC-43), written as assertable accessible names.** None of these may exist on
`/s/:code`: `Switch endpoint`, `Account`, `Sign out`, `Rules`, `New rule`, `Settings`,
`Pause the live feed` / `Resume the live feed`, `Feed actions`, `Clear all`, `Export CSV`, `Share`,
`Copy mock URL`, `Copy local path`, `Resize feed and inspector`. Nor may the DOM contain the endpoint
token, `mock_url`, `path_url`, `target_url`, or any rule text.

**Two exposure details the layout owes the viewer page:**

- `served_by` is in the public projection, so a viewer learns whether traffic was mocked, proxied via
  MITM, or tunnelled to someone's laptop. That is decided contract, but the owner-voiced tooltip
  strings must not travel with the chip: `servedBy.mitm.tooltip` says *"Proxied to **your** upstream
  target"* and `servedBy.tunnel.tooltip` says *"…down **your** tunnel to localhost"*
  **[existing — `copy.ts:157-159`]**. `ServedByChip` renders no tooltip today
  **[existing — `served-by-chip.tsx:40-55`]** — keep it that way on this page. **[gap #20]**
- Every link on the page must be same-origin, and the page must not load off-origin subresources:
  the share **code is in the URL path**, so any outbound navigation or asset request leaks a bearer
  credential in `Referer`. §5.9.3 covers nginx access logs but not this, nor crawlers. **[gap #19]**

### 3.6 F6 — Add default rule

| State | Rendering |
|---|---|
| available | `Button variant="secondary" size="sm"` in the toolbar and in the empty-state card |
| already has a catch-all | **disabled**, and the reason must be reachable by keyboard. A `disabled` `<button>` fires no pointer events, so a `Tooltip` on it is dead for mouse users too — wrap it: `<Tooltip content={t("rules.default.exists")}><span tabIndex={0} aria-describedby=…><Button disabled …/></span></Tooltip>`. Keeps the primitive's disabled styling *and* makes the explanation reachable (AC-61) **[gap #26]** |
| in flight | `loading` + disabled. **Required, not optional**: AC-61's duplicate guard is computed from the rules list, which only updates *after* the reload, so the double-click window it names is only actually closed by disabling during the POST **[gap #24]** |
| success | list reloads, success toast `t("rules.default.toast")`; the new rule sorts **last** (priority 1000 vs default 100, `ORDER BY priority, id`). In a long list it lands below the fold — a `scrollIntoView({block:'nearest'})` on the new row id is a two-line courtesy, not a requirement |
| failure | danger toast `t("rules.default.error")`, list unchanged (AC-60) |

### 3.7 F7 — no new UI, three rendering consequences

1. **The Inspector's Response-body panel becomes real.** No frontend change needed
   **[existing — `inspector.tsx:246-253`]** — but the value it now receives is arbitrary
   upstream/CLI bytes, up to `MAX_BODY_BYTES` (256 KB), and lossy-decoded (U+FFFD). `JsonTree`
   already degrades to `<pre>` when `JSON.parse` throws **[existing — `json-tree.tsx:72-78`]**, and
   caps its viewport at `max-h-[40vh] overflow-auto` — so HTML, plain text and mojibake render
   safely as text nodes (AC-67). **But** a 256 KB *valid* JSON body builds thousands of recursive
   `Node` components with two levels auto-expanded **[existing — `json-tree.tsx:13`]**, which can
   jank or freeze the tab. Mitigation: default `mode` to `'raw'` above a threshold (~64 KB) with a
   caption `t("insp.body.largeRaw")`. This affects the owner Inspector **and** the public viewer.
   **[gap #21]**
2. **Truncation is silent.** R4 accepts no marker and no flag, and the copy table already carries an
   **unwired** key `insp.body.truncated` — *"Truncated at {bytes} — captured bodies are capped."*
   **[existing — `copy.ts:135`]**. Either wire it off the `len === MAX_BODY_BYTES` heuristic R4 itself
   suggests, or record explicitly that truncation is invisible in the UI. Silence is worse on the
   public viewer, where a viewer has no other way to learn a body was cut. **[gap #22]**
3. **The redaction pill is broken, and F4 makes it matter.** `KeyValueRows` tests
   `v === '__redacted__'` **[existing — `key-value-rows.tsx:9`]** but the server writes
   `<redacted>` **[existing — `backend/src/helpers.rs:43`]**, so today the literal string
   `<redacted>` renders as an ordinary mono header value and the neutral-chip pill never appears. On
   an unauthenticated page, "HookBox hid this" must be unmistakable and must not look like a header
   value someone actually sent. One-const fix; it also repairs the owner Inspector. **[gap #23]**

---

## 4. Copy

New keys for `src/lib/copy.ts`, in the table's existing style (sentence case, second person, plain,
no exclamation marks, em dashes for asides), grouped as new `copy.md` sections **§5.15–§5.19**
(existing sections run §4.1–§5.14 **[existing — `docs/features/hookbox-rust-replatform/copy.md`]**).
Namespaces: `feed.*` / `set.config.*` / `rules.default.*` extend existing screens; `share.*` is the
**owner** dialog and `viewer.*` is the **public** page — deliberately different prefixes so an audit
of "what strings can an anonymous visitor see" is one grep.

**Reused, not duplicated:** `common.cancel`, `common.retry`, `common.close`, `set.confirm.cancel`,
`set.toast.historyCleared`, and on the public viewer `insp.tab.headers|query|body|response`,
`insp.headers.empty`, `insp.query.empty`, `insp.body.empty`, `insp.response.headers`,
`insp.response.body`, `insp.response.empty`, `insp.response.servedByLabel`, `shell.skipLink`.
(Confirm with the PM that owner-authored `insp.*` values are acceptable verbatim on a public page —
they read neutrally, but they were written for an operator. **[gap #27]**)

### §5.15 Feed actions (`feed.*`) — F1, F5

| Key | Value |
|---|---|
| `feed.actions.menu.aria` | Feed actions |
| `feed.actions.emptyHint` | Nothing to clear or export yet — the feed is empty. |
| `feed.clearAll` | Clear all |
| `feed.clearAll.aria` | Clear all captured requests |
| `feed.clearAll.confirm.title` | Clear all requests? |
| `feed.clearAll.confirm.body` | Every request captured for "{endpoint}" will be deleted and the feed starts fresh. This can't be undone. |
| `feed.clearAll.confirm.confirm` | Clear all |
| `feed.clearAll.error` | Couldn't clear the requests. Nothing was deleted. |
| `feed.export` | Export CSV |
| `feed.export.aria` | Export the listed requests as CSV |
| `feed.export.note` | Exports the {n} requests listed now — newest first. |
| `feed.export.progress` | Exporting {done} of {total}… |
| `feed.export.progress.aria` | Exporting requests |
| `feed.export.preparing` | Preparing file… |
| `feed.export.cancel` | Cancel export |
| `feed.export.cancelled` | Export cancelled. Nothing was downloaded. |
| `feed.export.done` | Exported {n} requests. |
| `feed.export.done.partial` | Exported {n} requests — {m} without detail. |
| `feed.export.error` | Couldn't build the export. Nothing was downloaded. |

### §5.16 Configuration export / import (`set.config.*`) — F3

| Key | Value |
|---|---|
| `set.config.title` | Configuration |
| `set.config.helper` | Move this endpoint's settings and rules to another endpoint, or keep a copy alongside your code. |
| `set.config.export` | Export config |
| `set.config.export.aria` | Download this endpoint's configuration as JSON |
| `set.config.export.dirty` | Exports the saved configuration — save your changes first to include them. |
| `set.config.export.error` | Couldn't build the export. Try again. |
| `set.config.import` | Import config… |
| `set.config.import.helper` | Reads a HookBox config JSON file. Settings are replaced; rules are added. |
| `set.config.import.fileHint` | One .json file, up to 5 MB and 200 rules. |
| `set.config.import.invalid` | That file isn't a valid HookBox config: {reason} |
| `set.config.import.wrongVersion` | Unsupported config version {version}. This build reads version 1. |
| `set.config.import.tooLarge` | That file is larger than 5 MB. |
| `set.config.import.tooManyRules` | That file has {n} rules — the limit is 200. |
| `set.config.import.chooseAnother` | Choose another file |
| `set.config.confirm.title` | Import this configuration? |
| `set.config.confirm.body` | Replaces the nine settings on this endpoint with the values in the file, then adds {n} rules to the {existing} already here. Nothing in the rules list is replaced or deleted. |
| `set.config.confirm.exported` | Exported {when} from an endpoint named "{name}". |
| `set.config.confirm.dirty` | You have unsaved changes on this screen. Importing discards them. |
| `set.config.confirm.confirm` | Import configuration |
| `set.config.import.progressConfig` | Applying settings… |
| `set.config.import.progressRules` | Creating rule {i} of {n}… |
| `set.config.import.done` | Configuration imported — {n} rules added. |
| `set.config.import.failedConfig` | Couldn't apply the settings, so no rules were created. The server said: {detail} |
| `set.config.import.failedRule` | Settings were applied and {done} of {total} rules were created. Rule {index} ("{name}") failed: {detail}. No rule after it was attempted, and nothing was rolled back. |
| `set.config.import.viewRules` | View rules |

### §5.17 Share links — owner (`share.*`) — F4

| Key | Value |
|---|---|
| `share.action` | Share |
| `share.action.aria` | Share this endpoint read-only |
| `share.action.count.aria` | Share this endpoint read-only — {n} active links |
| `share.title` | Share read-only |
| `share.intro` | A share link lets anyone holding the URL read this endpoint's recent requests in a browser. No account, no sign-in, and no way to change anything. |
| `share.warning.title` | A share link exposes captured traffic |
| `share.warning.body` | Whoever holds the URL sees each request's method, path, status, headers, query and body — and the response headers and response body HookBox returned. Authorization, Cookie and X-Owner-Id request headers are hidden; nothing else is. Response headers are shown exactly as they were sent, including any Set-Cookie from your upstream. Don't share an endpoint that carries production secrets. |
| `share.create` | Create share link |
| `share.creating` | Creating… |
| `share.label.label` | Label |
| `share.label.placeholder` | e.g. For Acme support ticket #421 |
| `share.label.helper` | Optional. Only you see it — it helps you remember who a link went to. |
| `share.label.tooLong` | Labels are 80 characters or fewer. |
| `share.created.title` | Your share link |
| `share.created.hint` | Copy it now — you can always come back to this dialog for it. |
| `share.list.title` | Active links |
| `share.list.count` | {n} of {max} |
| `share.list.loading.aria` | Loading share links |
| `share.list.empty.title` | No share links yet |
| `share.list.empty.body` | Create one when you need to show someone what their webhook actually sent. |
| `share.list.error.title` | Couldn't load share links |
| `share.row.untitled` | Untitled link |
| `share.row.created` | Created {when} |
| `share.row.lastUsed` | Opened {when} |
| `share.row.neverUsed` | Never opened |
| `share.row.open` | Preview |
| `share.row.open.aria` | Open this share link in a new tab |
| `share.row.revoke` | Revoke |
| `share.row.revoke.confirm` | Revoke this link? |
| `share.row.revoke.confirmHint` | It stops working immediately and can't be brought back. |
| `share.limit.reached` | You have the maximum of {max} active links. Revoke one to create another. |
| `share.toast.created` | Share link created. |
| `share.toast.revoked` | Share link revoked. |
| `share.error.create` | Couldn't create the link. Try again. |
| `share.error.revoke` | Couldn't revoke the link. It's still active. |
| `share.done` | Done |

### §5.18 Public shared view (`viewer.*`) — F4

| Key | Value |
|---|---|
| `viewer.title` | Shared requests |
| `viewer.docTitle` | Shared requests · HookBox |
| `viewer.banner.title` | Read-only shared view |
| `viewer.banner.body` | Someone shared this HookBox endpoint's recent requests with you. You're not signed in, and nothing on this page can be changed. |
| `viewer.subject.name` | Endpoint: {name} |
| `viewer.subject.unnamed` | Unnamed endpoint |
| `viewer.subject.since` | Capturing since {when} |
| `viewer.subject.total` | {n} requests received in total |
| `viewer.count` | Showing {n} of the last 100 |
| `viewer.updated` | Updated {when} |
| `viewer.updating` | Updates every 5 seconds while this tab is open. |
| `viewer.updating.paused` | Paused while this tab is in the background. |
| `viewer.refresh` | Refresh |
| `viewer.loading.aria` | Loading shared requests |
| `viewer.empty.title` | No requests yet |
| `viewer.empty.body` | Nothing has reached this endpoint yet. This page updates on its own — leave it open. |
| `viewer.row.expand.aria` | Show detail for {method} {path}, {status} |
| `viewer.row.collapse.aria` | Hide detail for {method} {path} |
| `viewer.detail.loading.aria` | Loading request detail |
| `viewer.detail.gone.title` | This request is no longer available |
| `viewer.detail.gone.body` | HookBox keeps the last 100 requests for 24 hours. This one has aged out. |
| `viewer.detail.error.title` | Couldn't load this request |
| `viewer.detail.error.body` | Something went wrong fetching the detail. |
| `viewer.unavailable.title` | This link isn't available |
| `viewer.unavailable.body` | It may have been revoked, or it may never have existed. Ask whoever sent it for a new one. |
| `viewer.unavailable.about` | What is HookBox? |
| `viewer.rateLimited.title` | Too many requests |
| `viewer.rateLimited.body` | This page has been loaded too often. It retries in {seconds}s. |
| `viewer.error.title` | Couldn't load the shared requests |
| `viewer.error.body` | Something went wrong reaching the server. |
| `viewer.footer` | Served by HookBox — a self-hosted request inspector. This page is read-only. |

**Copy constraint on `viewer.unavailable.*`:** one string set covers unknown, revoked and deleted
(AC-36). Do not add a "this link was revoked" variant later — that reintroduces the oracle the AC
exists to remove.

### §5.19 Default rule (`rules.default.*`) — F6

| Key | Value |
|---|---|
| `rules.default.add` | Add default rule |
| `rules.default.helper` | Answers anything no other rule matches with a 200 and a placeholder JSON body. |
| `rules.default.exists` | This endpoint already has an enabled catch-all rule. |
| `rules.default.adding` | Adding… |
| `rules.default.toast` | Default catch-all rule added. |
| `rules.default.error` | Couldn't add the default rule. Try again. |

### Additional key (F7 rendering)

| Key | Value |
|---|---|
| `insp.body.largeRaw` | Large body — showing raw text. |

---

## 5. Accessibility

Baseline: WCAG 2.1 AA in both themes, with the existing global focus ring (`--shadow-focus`, a 2px
surface halo + 2px `--focus-ring` **[existing — `globals.css:149`]**) and the global
`prefers-reduced-motion` block that zeroes every animation and transition
**[existing — `globals.css:283-301`]** — so no new motion needs bespoke handling, including the
`Progress` fill's width transition.

**Semantics**

- Feed-actions `Menu`: Radix `DropdownMenu` gives `role="menu"`/`menuitem`, roving focus, `Esc`,
  typeahead and focus restoration to the trigger. Icon-only trigger carries
  `aria-label={t("feed.actions.menu.aria")}`. Disabled items expose `data-disabled` +
  `aria-disabled`, already styled **[existing — `menu.tsx:38`]**.
- `ConfirmDialog` / `ShareDialog`: Radix `Dialog` supplies `role="dialog" aria-modal`, the focus
  trap, `Esc`, scrim click and focus restore (AC-65) **[existing — `dialog.tsx`]**. `DialogHeader`
  renders `DialogPrimitive.Title`, so every dialog has an accessible name for free. Add
  `DialogDescription` (already exported at `dialog.tsx:66`, currently unused) to the Share dialog so
  the intro paragraph becomes the dialog's description rather than anonymous body text.
- `Progress`: `role="progressbar"` + `aria-valuemin`/`aria-valuemax`/`aria-valuenow` +
  `aria-valuetext` (the localized "Exporting 41 of 100…") + `aria-label`. Visible label is **not** a
  live region; a separate `sr-only aria-live="polite"` node announces start / ~25 / 50 / 75 /
  complete only.
- File input: `sr-only` native `<input type="file">` (focusable and announced) + `<label htmlFor>`
  carrying the visible button styling, `aria-describedby` → `set.config.import.fileHint`. Validation
  failures are `InlineAlert role="alert"`.
- Tooltip-on-disabled (F6): `disabled` buttons dispatch no pointer events and are out of tab order,
  so the tooltip must hang off a focusable wrapper `<span tabIndex={0} aria-describedby=…>`. Mirror
  the reason in `title` as a no-JS fallback.
- Public viewer document structure: `<header>` → banner (`role="status"`) → `<main id="main">` with
  exactly one `<h1>` → `<footer>`, plus the reused skip link `t("shell.skipLink")` **[existing —
  `app-shell.tsx:74-79`]**. The dashboard has no `<h1>`; a standalone public page must, and the
  viewer's is static (see §2.6).
- Disclosure rows: `<button type="button" aria-expanded={open} aria-controls={"req-"+id}>` and a
  `<div id={"req-"+id} role="region" aria-label={…}>`. **Not** `role="option"`/`listbox` — there is no
  single-selection model here, and `FeedRow`'s listbox semantics are wrong for a page whose content
  updates underneath the reader.

**Keyboard**

- Every new control is a real `<button>`, `<a>`, `<input>` or Radix primitive — nothing is a
  `div onClick`. Tab order in the feed header: `[N new] → [⋯] → [Pause]`; in the export strip:
  `[Cancel]`; in the Share dialog: `label → Create → (created copy) → row 1 [copy, Preview, Revoke] →
  … → Done`.
- Inline revoke confirm: `Esc` cancels the arm without closing the dialog (Radix's `Esc` closes the
  dialog, so the row handler must `stopPropagation` while armed). Arming moves focus to the row's
  destructive `Revoke` so a keyboard user does not have to hunt for it.
- Viewer: `Enter`/`Space` toggle a row; focus stays on the trigger through expand and collapse; a
  poll that adds rows never moves focus and never collapses an open row.

**Live regions & announcements**

- Success/failure of every mutation: existing `Toast` (`ToastPrimitive` is announced by default) —
  short strings only.
- Created share link: `role="status" aria-live="polite"` around the `CodeBlock` region. Focus is not
  stolen. `CopyButton` already ships its own `sr-only role="status" aria-live="polite"` "Copied to
  clipboard" **[existing — `copy-button.tsx:47-49`]**.
- Loading: `aria-busy="true"` + `sr-only` label on each skeleton container, matching every existing
  screen **[existing — `dashboard.tsx:355-359`, `rules-manager.tsx:197-201`]**.
- Viewer poll: an `sr-only aria-live="polite"` announcement of the new count, throttled to at most
  once per 10 s, so a busy endpoint does not spam a screen reader.

**Contrast**

No new colour pairs are introduced. Every new surface reuses shipped, AA-verified pairings:
`text-danger-fg` on `bg-surface-raised` (the rules row Delete item), `warning-fg`/`warning-bg` and
`info-fg`/`info-bg` (`InlineAlert`), `neutral-chip-*` (the count badge), `accent-fill` on
`bg-subtle` (the `Progress` fill), `text-text-tertiary` on `bg-surface` for captions. State is never
carried by hue alone: the destructive menu item is worded "Clear all"; progress carries digits;
`ServedByChip`/`StatusCode`/`MethodBadge` are icon-plus-text by construction
**[existing — `served-by-chip.tsx:1-5`]**; the disclosure chevron rotates *and* `aria-expanded`
changes.

---

## 6. Consistency notes

Patterns and classes lifted, not invented:

1. **Destructive confirm** = `Dialog` + `DialogHeader`/`DialogBody`/`DialogFooter` + ghost Cancel +
   `variant="danger"` confirm — from `rules-manager.tsx:317-336` and `settings.tsx:659-705`. F1 and
   F3 both use it via one extracted component.
2. **Row overflow menu** = `Button variant="ghost" size="icon-sm"` + `MoreHorizontal` +
   `aria-label` + `MenuContent align="end"` with the destructive item last and
   `className="text-danger-fg"` — from `rules-manager.tsx:274-298`. Reused verbatim for the feed
   header.
3. **Filled `variant="danger"` only inside a confirm dialog or the bordered danger zone** —
   `settings.tsx:422-441`. In-page destructive triggers are ghost + `text-danger-fg`.
4. **One `variant="primary"` per surface** — `button.tsx:1-6`. "New rule" keeps it in Rules Manager
   (so F6 is secondary); "Save" keeps it in Settings (so Export/Import are secondary); "Create share
   link" is the primary *inside its dialog*, which is its own surface.
5. **Copy affordances**: `CodeBlock` (mono-lg + integrated `CopyButton`) for a full-width value to
   copy; `MockUrlChip` for an inline one. Both copy-only, never accent/link coloured (AC-D19) —
   `code-block.tsx`. Navigation is always a separate explicit control.
6. **Error surfaces**: transient success/failure → `useToast`; anything the operator must read, act
   on, or retry → `InlineAlert` with `role="alert"`/`"status"` and an optional `action` slot —
   `rules-manager.tsx:204-216`, `inspector.tsx:107-171`, `dashboard.tsx:264-274`.
7. **Loading** = `SkeletonLines` inside an `aria-busy` container with an `sr-only` label. Never a
   bare spinner for a region; `Spinner` only inside `Button loading`.
8. **Settings section** = `Section` (`space-y-3 border-b border-border pb-6 last:border-b-0` + `h4`)
   — `settings.tsx:707-720`.
9. **List/table header band** = `bg-subtle` + `text-overline uppercase tracking-wide
   text-text-tertiary` over a grid whose columns the rows repeat — `rules-manager.tsx:232-238`.
   Reused for the public viewer's card header.
10. **Public-page chrome** = `flex min-h-screen flex-col bg-canvas` + slim `header` with `BrandMark`
    + `ThemeToggle` + `main#main` — `cli.tsx:67-84`. Reused for `/s/:code`.
11. **Full-page terminal state** = centred `min-h-screen` card with `h2` + body + one secondary
    action — `dashboard.tsx:143-158`. Reused for the viewer's "link unavailable".
12. **Feed-row grammar** = `MethodBadge` · truncated mono path with `title` · `StatusCode` ·
    `ServedByChip` · `tnum` latency · relative time with `title={iso}` — `feed-row.tsx:70-81`.
    Reused (as classes, not as the component) by the viewer row.
13. **Relative time** — `relTime` currently lives inside `feed-row.tsx:23-34`. Three new surfaces
    need it (share rows, viewer rows, "Updated {when}"); extract to `src/lib/time.ts` rather than
    copy it four times.
14. **Values are always text nodes** through `KeyValueRows` / `JsonTree` / `CodeBlock`; no
    `dangerouslySetInnerHTML` anywhere (AC-67) — `inspector.tsx:16-19`.
15. **All strings via `t()`** from the single copy table (AC-64) — `copy.ts:1-9`. Known pre-existing
    exceptions the public viewer will inherit: `JsonTree` hardcodes "Pretty"/"Raw"/"JSON view
    mode"/"(empty)" **[existing — `json-tree.tsx:83-98`]** while `insp.body.pretty`/`insp.body.raw`
    sit unused in the table, and `KeyValueRows` hardcodes "redacted"/"None" while
    `insp.headers.redacted` sits unused. Pre-existing debt, but F4 puts it on an unauthenticated
    page — cheap to fix in this batch.
16. **Client boundary**: all fetches through `src/api/client.ts` with zod schemas; the two public
    calls use `noAuth: true` so they never touch `session` (AC-42) — `client.ts:75-136`.

---

## 7. PRD gaps

Numbered items the PM must add, clarify, or decide. Items marked **BLOCKING** should be settled
before UI implementation starts, because the answer changes the design rather than refining it.

1. **AC-1 / AC-46 — placement of Clear all + Export CSV.** Both ACs fix them "to the left of the
   pause/resume control" in the feed header, which cannot fit at the feed pane's 360 px minimum
   (`min-w-feed`) alongside the title, the count and the "N new" pill. Reword to *"in the feed
   header's action group, which may be an overflow menu"*, and add an AC that the feed header stays
   usable at 360 px.
2. **AC-1 — visual weight of the destructive control.** "Destructive" is ambiguous. Specify ghost +
   `text-danger-fg` (matching the rules row Delete item), not `variant="danger"`, which this codebase
   reserves for confirm buttons and the Settings danger zone.
3. **AC-2 — no count in the F1 confirm body**, and decide the fate of the existing
   `set.confirm.clearHistory.body`. `endpoint.request_count` is a monotonic lifetime counter on
   `endpoints` **[existing — `backend/migrations/0001_init.sql:31`]**, not the number of stored
   traces (capped at `TRACE_CAP` = 100), so the shipped Settings copy "All {n} traces … will be
   removed" is wrong for any endpoint past 100 hits.
4. **AC-4/AC-5 vs AC-47 — F1 × F5 interaction is undefined.** Add an AC that Clear all and Export
   CSV are mutually exclusive (both disabled while an export is in flight, or the export auto-cancels
   on a successful clear). Otherwise a clear mid-export silently produces a file of
   `pending`/`unavailable` sentinels.
5. **AC-20 — dirty-form hazard on import.** "The Settings form … reflect the server state" will
   silently discard the operator's unsaved edits. Add: the import confirm warns when the form is
   dirty, and the form is re-initialised from the server after a successful import.
6. **F3 export source is unspecified.** Add an AC that the bundle is built from freshly fetched
   server state (not the in-memory form), and that the UI says so when the form is dirty.
7. **AC-18 — additive rules must be stated pre-write.** Add an AC that the confirm step names both
   numbers ("adds {n} rules to the {existing} already here; nothing is replaced or deleted") before
   any request is sent. Also confirm the intended button variant is `primary`, not `danger`.
8. **§7 is missing `src/lib/download.ts`.** AC-12 and AC-49 both require object-URL create → click →
   **revoke** plus a filename convention. Add the shared helper so the two features cannot drift.
9. **§7 is missing a progress primitive.** AC-20 and AC-48 both require determinate progress and no
   such primitive exists. Either add `src/components/ui/progress.tsx` to §7, or state that a text-only
   counter satisfies both ACs.
10. **AC-48/AC-49 — the serialisation phase is unaccounted for.** 100 rows × two ≤256 KB bodies can
    mean tens of MB of string + `Blob` building on the main thread after the last fetch. Add a
    "Preparing file…" phase to the progress contract, and an AC that a second export cannot start
    while one is in flight.
11. **AC-52 — completion messaging.** Specify both strings (`m === 0` vs `m > 0`), and decide whether
    the `pending` / `unavailable` sentinels are explained anywhere in the UI or only in §5.6. Today a
    recipient of the CSV has no way to learn what they mean.
12. **AC-23 — Share control placement.** Requests moving it from the subject cluster to the
    right-hand action group (preserving `AppShell`'s documented left=subject / right=actions split),
    and asks the PM to decide whether an active-link count badge is in scope — it implies one
    `GET /api/endpoints/{token}/shares` per owner-screen mount.
13. **AC-25/AC-26 — revocation needs a confirmation step.** §5.1 makes revocation irreversible (a
    revoked code can never be re-minted), so add an AC for a confirm step and state that Undo is
    impossible.
14. **AC-27 — pre-emptive cap handling.** Add an AC that Create is disabled with an explanatory
    message once `SHARE_MAX_PER_ENDPOINT` active links exist, in addition to surfacing the 422.
15. **BLOCKING — §5.4/R3 share-code storage must be settled before the dialog is built.** The
    plaintext-code design supports "copy an existing link again" and a Preview action. If security.md
    overrides to `code_hash` + display prefix, `code` becomes write-once and the dialog changes
    materially (show-once panel, no re-copy, no Preview, different empty/list copy). Resolve, then
    lock AC-24/AC-25.
16. **AC-43 needs two additions.** (a) Turn it into an assertable list of accessible names that must
    be **absent** from `/s/:code` (see §3.5 for the list). (b) Add: no share-controlled string
    (endpoint name, request path, header name/value, body content) may be rendered as the page
    `<h1>`, as `document.title`, as a link `href`, or as an image/iframe source. The endpoint name is
    operator-controlled text on an unauthenticated page and is a ready-made phishing slot.
17. **AC-44 — forbid reusing the owner empty state.** `FeedEmpty` renders `mock_url` in a `CodeBlock`
    plus a curl sample **[existing — `dashboard.tsx:384-413`]**; reusing it on `/s/:code` would
    violate AC-34. Make the prohibition explicit so no one "reuses the component" in good faith.
18. **AC-45 — the cadence must be visible, and 429 behaviour must be defined.** Add: the page states
    its refresh interval and that polling pauses when the tab is hidden; a manual Refresh control
    exists; on 429 polling pauses for `Retry-After` and resumes once automatically.
19. **§5.9.3 covers logs but not crawlers or referrers.** The share code is a bearer credential in a
    URL path. Add ACs for (a) `noindex, nofollow` on `/s/:code` (meta tag and/or an nginx
    `X-Robots-Tag` on `/s/` and `/api/share/`), and (b) no off-origin links or subresources on the
    viewer page, so the code cannot leak via `Referer`.
20. **`served_by` is public — confirm and constrain.** A viewer learns whether traffic was mocked,
    proxied (MITM) or tunnelled to someone's laptop. It is in the frozen projection, so this is an
    acknowledgement request, plus an AC that the owner-voiced `servedBy.*.tooltip` strings ("**your**
    upstream target", "**your** tunnel to localhost") are never rendered on the public page.
21. **F7 rendering safety is unspecified.** `response_body` can now be 256 KB of arbitrary
    upstream/CLI content. Add an AC that `JsonTree` defaults to Raw above a size threshold (owner
    Inspector **and** public viewer) so a large valid-JSON body cannot jank or freeze the tab via the
    recursive tree with two levels auto-expanded.
22. **F7 truncation honesty.** `insp.body.truncated` exists in the copy table but is unwired
    **[existing — `copy.ts:135`]**. Decide: wire it off R4's own `len === MAX_BODY_BYTES` heuristic,
    or record that truncation is invisible in the UI. It matters most on the public viewer, where a
    viewer has no other way to know a body was cut.
23. **Pre-existing redaction-pill bug, promoted by F4.** `KeyValueRows` tests `'__redacted__'`
    **[existing — `key-value-rows.tsx:9`]** but the server writes `'<redacted>'` **[existing —
    `backend/src/helpers.rs:43`]**, so the neutral-chip pill never renders and the literal
    `<redacted>` shows as an ordinary header value. Add an AC to fix the sentinel: on an
    unauthenticated page "HookBox hid this" must not look like data the caller sent.
24. **AC-61's duplicate guard has a hole.** The "already has a catch-all" check is derived from the
    rules list, which only refreshes *after* the reload — so it does not close the double-click window
    it names. Add an AC that the control is disabled while the POST is in flight.
25. **AC-57 — empty-state composition.** Confirm that only "Add default rule" is added to the rules
    empty state (no second CTA), and that both instances are `variant="secondary"` so "New rule"
    keeps the single primary on that surface.
26. **AC-61 — the tooltip must be keyboard-reachable.** A `disabled` `<button>` fires no pointer
    events and is out of tab order, so a tooltip on it is unreachable by mouse *and* keyboard. Add an
    AC that the reason is exposed via a focusable wrapper and/or `aria-describedby`.
27. **AC-64 — copy ownership and reuse.** Confirm that §4 above is the source for the new `copy.md`
    §5.15–§5.19 sections (R10 assigns final wording to design/ux), and confirm that reusing
    owner-authored `insp.*` values verbatim on an unauthenticated page is acceptable.
28. **AC-6 vs. the component it will be built from.** The `ConfirmDialog` in `settings.tsx` has no
    `catch` **[existing — `settings.tsx:688-697`]**: a failing confirm leaves the dialog open (as
    AC-6 wants) but throws an unhandled rejection and tells the user nothing. Add an AC that the
    shared confirm renders the failure and that the two existing Settings confirms are fixed by the
    same change.
29. **No AC covers the viewer on a phone.** Share links are opened from chat, tickets and email,
    where mobile is the common case, and the dashboard's `SplitPane` (`min-w-feed: 360px`,
    drag-to-resize) has no mobile treatment. Add a responsive AC for `/s/:code` (single column, no
    horizontal scroll, rows legible at 360 px).
30. **Out of scope, recorded:** `shell.mobileNav.*` copy keys exist **[existing — `copy.ts:59-60`]**
    but nothing renders them, so the owner sub-header and feed header have no mobile plan today. F1,
    F4 and F5 add controls to both. Confirm this stays out of scope for this batch rather than being
    silently made worse.
