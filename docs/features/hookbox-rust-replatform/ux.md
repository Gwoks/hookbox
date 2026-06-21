# UI/UX: HookBox — Rust/Axum re-platform (Vite + React + TypeScript SPA)

- **Slug:** `hookbox-rust-replatform`
- **Status:** Draft (ui-ux, DESIGN phase)
- **Author:** UI/UX designer (multi-agent pipeline)
- **Upstream:** `docs/features/hookbox-rust-replatform/prd.md`, locked spec `docs/superpowers/specs/2026-06-21-hookbox-rust-replatform-design.md` (frontend §10), `FEATURES.md`.
- **Reference UI:** `../shortener-link/src/` — the React/Radix/Tailwind/CVA SPA we mirror (component conventions, tokens, a11y). **The reference's *structure and conventions* are reused; HookBox's visual identity and voice are NEW.**
- **Downstream consumers:** `design-agent` (visual layer / final tokens), `copywriter-engineer` (voice, strings, microcopy), `system-architect` (consumes the §5 shapes this design binds to), `frontend-engineer` (builds `src/`).

> **How to read this doc.** It defines the SPA's information architecture, screen-by-screen structure, the component inventory (reusing shortener-link's `ui/` primitives), interaction states, real-time feed behavior, and accessibility — *as structure and interaction, not final pixels or final words*. Where I name colors/labels they are **placeholders** for the design-agent / copywriter to finalize. This is a fresh, light React SPA — **not** a port of the Jinja+Alpine templates in `templates/`. The current `templates/dashboard.html`, `partials/inspector.html`, `index.html` are the *behavioral parity reference* (read and honored), not a code source. The PRD-frozen data shapes (§5.3), resolution semantics (§5.5), and WS/SSE events (§5.4) are the hard contract every screen binds to.

---

## 0. Stack & conventions (mirrors `../shortener-link`)

The SPA reuses the reference's foundation verbatim in *approach*:

- **React 18 + TypeScript + Vite**, served from the single Rust binary with SPA fallback (spec §4/§10). Routing via **react-router** (`createBrowserRouter`), mirroring `../shortener-link/src/router.tsx`: public routes + a protected route group wrapped in an auth gate + app shell.
- **Tailwind + semantic CSS-variable tokens** wired as Tailwind theme colors (`bg-surface`, `text-secondary`, `accent`, `success-fg`, …) exactly as in the reference. **No hardcoded hex in components** — a hard rule the current Jinja templates violate (they litter `#0d1117`/`#30363d`/`#58a6ff` inline; the SPA must not). Dark mode via `.dark` class on `<html>` driven by a theme provider + `prefers-color-scheme`. **HookBox ships dark-first** (its identity is a developer debugging tool) but BOTH themes are first-class and AA-compliant (§6).
- **CVA for variant components** (`buttonVariants` pattern, `../shortener-link/src/components/ui/button.tsx`).
- **Radix primitives** for the hard a11y: `Dialog` (rule builder, settings, confirms, mobile nav), `Tabs` (inspector + rule builder), `DropdownMenu` (row/endpoint actions), `Toast`, `Tooltip`, `Switch`, `Select`, `RadioGroup`, `Popover`. This is the reliable path to the keyboard/focus bar below — same rationale as the reference.
- **lucide-react** icons (consistent 1.5px stroke); **`cn()`** class merge helper; **zod** for client-side form validation of the §5.3 shapes.
- **System font stacks** (no CDN) for offline/self-host runnability — but HookBox leans **monospace-forward**: every token, URL, header name/value, JSON body, template tag, and trace step renders in `--font-mono`. This is the product's texture (it is an HTTP instrument) and matches the current dashboard's mono density.
- **Live data** via a `useRequestStream` hook (WebSocket with SSE fallback) — the React equivalent of the current `static/js/request-stream.js` + `stores.js` feed store. State held in a small store (Zustand or React context + reducer; architect/frontend's call) per endpoint: `{ rows, paused, pending, selectedId, connection }`.

**New primitives HookBox needs beyond the reference `ui/` set** (the reference has no live feed): `MethodBadge`, `StatusCode`, `ServedByChip`, `ConnectionPill`, `KeyValueRows`, `JsonTree`, `CodeBlock` (with copy), `FeedRow`, `Tabs` wrapper, `Slider` (latency/chaos), and a `SplitPane`. All built on the same token/CVA/Radix foundation.

---

## 1. Information architecture & navigation

HookBox is **single-resource-centric**: the unit of work is an *endpoint*, and almost everything (feed, inspector, rules, settings, state, collections) is scoped to one endpoint at a time. This differs from shortener-link's many-rows-in-a-table model, so the shell differs: **HookBox does not use a persistent left nav of sections.** The endpoint *is* the workspace.

### 1.1 Route tree (react-router)

```
PUBLIC
  /                         Landing / email gate            (AC-1..AC-5, AC-46 landing)
  /cli                      Tunnel & CLI guidance (public, linkable)   (AC-49..AC-51)
  *                         SPA fallback → in-app "not found"

PROTECTED  (auth gate: owner capability in storage, else → /)
  wrapped in <AppShell>
  /d                        → redirect to primary endpoint /d/:token
  /d/:token                 Dashboard — split-screen (feed + inspector)   (AC-41..AC-45)
      ?req=:id              deep-link a selected trace into the inspector
  /d/:token/rules           Rules manager (overlay route on the dashboard) (AC-11..AC-15)
  /d/:token/rules/new       Rule builder (create)                          (AC-14)
  /d/:token/rules/:ruleId   Rule builder (edit)                            (AC-14)
  /d/:token/settings        Endpoint settings (overlay route)              (AC-22, AC-27, AC-28..AC-40, AC-48)
```

**Auth gate** mirrors `router.tsx`'s `ProtectedRoute`: while resolving identity → quiet centered spinner; no owner capability in `localStorage` (`hookbox_owner` shape from `base.html`: `{owner_id, owner_secret, token, mock_url, email}`) → `Navigate to="/"`. A `401` from any `/api` call clears identity and bounces to `/` (AC-60). Note: HookBox auth is **email-keyed capability in localStorage**, not the reference's NextAuth session — the gate checks storage, not a session cookie.

**Overlay routes** (`/rules`, `/settings`, rule builder): rendered as Radix `Dialog`s layered over the dashboard so the live feed keeps streaming behind them (the current Jinja app does this with `x-show` overlays — we preserve it as nested routes that mount dialogs, so they are deep-linkable and Back-button-friendly). The rule builder is large enough that on desktop it is a centered dialog and on mobile a full-screen sheet.

### 1.2 App shell (adapted from `app-shell.tsx`)

Reuse the reference `AppShell` *pattern* (skip-to-content link first in tab order, slim sticky top bar, Radix-Dialog mobile drawer, `PageHeader`), but the shell content is endpoint-scoped, not a section nav:

- **Top bar (sticky, `h-header`):** wordmark/hook-mark (left) · **Endpoint switcher** (a Radix `Select` listing `GET /api/endpoints`; choosing one routes to `/d/:token`, AC-2/AC-3 list) · **+ New endpoint** button (`POST /api/endpoints` → route to new token) · spacer · **theme toggle** · **account menu** (`DropdownMenu`: shows the owner email, Sign out → clears storage + `/`).
- **No left section rail.** The dashboard fills the viewport below the top bar as a split pane. The current `templates/dashboard.html` "endpoint bar" (switcher · +New · mock chip · Auto-CRUD toggle · Rules · +New Rule · Settings) is **split**: identity + switching go to the top bar; the per-endpoint action cluster (Rules / +New Rule / Settings / Auto-CRUD quick toggle) becomes a **dashboard sub-header** directly above the split pane, so it scrolls with the workspace, not the global chrome.
- **Mobile:** the endpoint switcher + actions collapse behind the existing slide-in drawer pattern (focus-trapped Radix Dialog, Escape/scrim/scroll-lock for free).

### 1.3 Navigation model summary

| From | To | Mechanism |
|---|---|---|
| Landing (email submit) | `/d/:primaryToken` | `POST /api/session` → persist owner → route |
| Auto-resume on `/` | `/d/:token` | owner in storage → `Navigate` (AC-3a parity) |
| Endpoint switcher | `/d/:otherToken` | Select → route (AC-2 list) |
| + New endpoint | `/d/:newToken` | `POST /api/endpoints` (AC-3) |
| Feed row click / `↑↓`+Enter | inspector populates; URL `?req=:id` | client selection + `GET /api/requests/:id` (AC-44) |
| Rules / +New Rule / Settings | overlay routes | Radix Dialog over the live dashboard |
| CLI link (settings / empty states) | `/cli` | static guidance page (AC-49..51) |

---

## 2. Screens & components — screen by screen

Notation: regions top→bottom, primary hierarchy first. FRs cited as AC-#. Components in `CamelCase` are inventory items (§3).

### 2.1 Landing / email gate — AC-1..AC-5, AC-46(landing)

Reference: `index.html` (behavior) + `guest-hero.tsx`/`auth-screen.tsx` (structure). Public, centered single column on `--bg-canvas`, `--guest-hero-max-w`. **One job: take an email, mint/resume a session.**

Structure:
- Brand lockup (hook-mark SVG + wordmark) — design-agent owns the mark; keep it an inline SVG (no external asset, offline-safe), as the current `index.html` does.
- Short product line (copywriter owns wording; current: "Self-hosted API mocking, interception & real-time debugging. No password.").
- **Email form:** one `Input type="email"` (label present, may be visually-hidden) + a primary `Button size="lg"` submit. Below it: a quiet helper "Your email is the recovery key — no password." (copywriter).
- A 3-up value strip (Mock · Intercept · Inspect) — presentational, `aria-hidden`, optional (design-agent may cut).
- A quiet link to `/cli` ("Have an endpoint already? Tunnel from your machine →").

Interaction & states:
- **default → submitting:** button `loading` (spinner + label retained, width preserved, `aria-busy`); input `disabled`.
- **validation (client):** light email-shape check; on fail → inline field error under the input, `aria-invalid`, focus returns to input. The server `422` is authoritative.
- **`429`:** persistent `InlineAlert` "Too many attempts — try again shortly" with the `Retry-After` window (AC-5). NOT a raw 429.
- **`!ok` / network:** generic retryable `InlineAlert`.
- **anti-enumeration (AC-1):** copy and UI are **identical** for brand-new vs existing email — never reveal which. No "welcome back" vs "account created" divergence.
- **success:** persist owner to `localStorage`, route to `/d/:primary.token`.
- **storage unavailable (private mode):** warn (`InlineAlert`) that the session won't be remembered; still allow submit; never redirect-loop (parity with current `index.html`).
- **auto-resume:** present valid owner → immediate redirect before paint (no form flash).

### 2.2 Dashboard — split-screen (the product) — AC-41..AC-45, AC-56..AC-61

Reference behavior: `templates/dashboard.html` + `partials/inspector.html`. The signature screen. A **`SplitPane`**: LEFT = live feed (≈40%, min ~360px), RIGHT = deep inspector (flex-1). Full-bleed below the top bar + dashboard sub-header.

**Dashboard sub-header** (above the pane): endpoint name · **mock-URL chip(s)** (`CodeBlock`/copy-only chips for `mock_url` and `/e/:token` `path_url`; never link-blue — copy-only) · Auto-CRUD quick `Switch` (optimistic, AC-24 toggle) · **Rules** · **+ New Rule** (the single primary/accent button) · **Settings**. When `MOCK_DOMAIN` is unset the mock chip surfaces the `/e/:token` form only (AC-10).

#### LEFT — Live feed
- **Feed header:** endpoint name · **`ConnectionPill`** (WS health: connecting / live / reconnecting / SSE-fallback / offline — always icon + text, `role="status" aria-live="polite"`, AC-41/43) · **Pause/Resume** toggle (`aria-pressed`).
- **"N new" pill:** appears while paused with buffered rows; click flushes + autoscrolls (AC-43 backpressure-friendly).
- **Feed list** (`role="listbox"`, `aria-live="polite"` when not paused, `off` when paused): a stream of **`FeedRow`** items. Columns (1:1 with the current grid so it stays scannable): `MethodBadge` · path (mono, truncate, full on hover/focus) · `StatusCode` (tabular, colored by class + always legible numerically) · `ServedByChip` (rule/crud/mitm/tunnel/default/cors/chaos/ratelimit — AC-56) · latency `Nms` (tabular) · relative time.
- **Selection:** click or `↑/↓` then Enter selects a row → populates the inspector and sets `?req=:id`. Selected row gets `aria-selected` + a visible non-color marker (leading accent rail + bg), not color alone.

#### RIGHT — Deep inspector — AC-44, AC-58, AC-61
Lazy-loads `GET /api/requests/:id` (`RequestDetail`, §5.3) on selection. **Radix `Tabs`** with the five frozen tabs (label 1:1 with `partials/inspector.html`):

1. **Headers** — `KeyValueRows` of request headers (capability + `Cookie`/`Authorization`/`X-Owner-Id` already redacted server-side, AC-61; surface a small "redacted" note where applicable). Per-row copy.
2. **Query** — `KeyValueRows` of query params.
3. **Body** — request body via `JsonTree` (Pretty/Raw toggle, Expand/Collapse all, Copy); `empty` / `binary, truncated` / `xml|text` (raw `<pre>`) variants. **Every captured value is a text node — never rendered as markup** (XSS-inert, parity with the current inspector's `x-text`-only rule).
4. **Response Served** — Served-by chip + the `X-HookBox-Endpoint` / `X-HookBox-Served-By` / `X-HookBox-Rule-Id` / `X-HookBox-Truncated` identifying headers (AC-56), response headers (`KeyValueRows`), response body (`JsonTree`).
5. **State & Tracing** — the vertical trace-step list (`TraceEvent[]`: step + detail, with before→after state diffs where encoded) + the `state_snapshot` at request time (`KeyValueRows`). Surfaces `overhead_ms` vs `duration_ms` (applied latency observable, AC-38).

A header strip above the tabs repeats the selected row's method/path/status/served-by/latency ("one subject, one color").

#### Dashboard states (every async surface — four states, a hard rule from the reference)
- **loading:** skeleton feed rows (shaped like the real grid; static under reduced-motion) + inspector "Select a request" empty state.
- **empty feed:** the **first-call funnel** — muted glyph, "No requests yet," the copyable mock-URL chip, and a **copy-only sample request** (`curl <mock_url>/ping`) rendered as static text (never executed, not an http asset — offline-safe; parity with current empty state).
- **inspector empty:** "Select a request on the left to inspect it."
- **inspector pending (AC-59):** trace written fire-and-forget → if detail not yet present, show "Detail still being written…" + **Retry**, NOT a 404 (parity with current `isPending`).
- **inspector error / unauthorized:** `role="alert"` + Retry.
- **endpoint not found / gone:** full-pane state "Endpoint not found — it may have been deleted or expired" + "Back to start" (AC-57 surfaced; on `404` from `GET /api/endpoints/:token`).
- **connection lost:** `ConnectionPill` reflects reconnecting/offline; feed keeps last rows; auto-reconnect with backoff (parity with current stream).

Responsive: at `<768px` the split pane stacks — feed full-width; selecting a row slides the inspector over it with a "← Feed" back bar (parity with the current mobile back bar). Never horizontal-scroll.

### 2.3 Rule builder — Matching · Response · Templating · Actions · Throttling — AC-11..AC-19, AC-23, AC-37..AC-40

Reference behavior: `templates/partials/rule_modal.html` + `rule_builder.js`. **NEW [react]** per AC-14. A large Radix `Dialog` (desktop) / full-screen sheet (mobile) opened from `/d/:token/rules/new` or `/rules/:ruleId`. Header (title + close), a **Radix `Tabs`** body with the five frozen tabs, footer (Cancel · Save — primary, with `loading`). Above the tabs: rule `name` (optional ≤120), `priority` (number, default 100), `enabled` `Switch`. Tabs map **1:1 to the §5.3 `MockRuleCreate` shape**:

1. **Matching** → `MatchCriteria`:
   - `method` Select (`ANY` + verbs).
   - `path` mono `Input` with a helper documenting `:param` capture and trailing `/*` (live-validated; show captured param names as chips).
   - **Headers** / **Query** required-pairs editors (`KeyValueRows` in edit mode — add/remove rows; header names case-insensitive).
   - **Body conditions** repeater (`BodyCondition`: jsonpath `path` + `op` Select `eq|neq|contains|exists` + `value`; `exists` hides the value field).
   - **State requirements** repeater (`StateRequirement`: `key` + `op` `eq|neq|exists|absent` + `value`; key validated to `^[A-Za-z0-9_-]{1,64}$`, AC-23 — inline error on unsafe key).
2. **Response** → `ResponseSpec`: `status_code` number (100–599, clamp + inline validate), `content_type` Select/free, response `headers` editor, `body_template` mono `Textarea` (≤256 KB; show a byte counter approaching the cap, AC-18).
3. **Templating** → a **palette/cheatsheet** of the closed tag set (AC-16): clickable chips that insert `{{now 'iso'}}`, `{{random 'uuid'}}`, `{{random 'int' lo hi}}`, `{{request.path.<name>}}`, `{{request.query.<k>}}`, `{{request.header.<name>}}`, `{{request.body.<jsonpath>}}`, `{{state.<k>}}` into the body textarea at the cursor. **Important UX honesty:** unknown tags render verbatim and never error (AC-17/18) — the palette is a discoverability aid, not a validator that rejects free text. No live "preview render" that could imply a general engine; if a preview is offered it must run the *same* closed scanner semantics (out of scope to spec here — flagged as a gap).
4. **Actions** → `state_writes[]` (`StateWrite{key,value}` repeater; key charset-validated, values templated and applied *before* the body renders — AC-19; note this in helper copy) + `webhook_action` (`{url, body_template}` — **stored, no-op today**; the UI must clearly label it "stored, not yet sent" so users aren't misled, per non-goal/OQ-9).
5. **Throttling** → per-rule overrides: `latency_ms` (0–10000, `Slider`+number) and `rate_limit_per_min` (0–100000, `0`=unlimited). Helper: "Overrides the endpoint value for requests this rule serves" (AC-15).

States: per-field inline validation (zod against §5.3 clamps); Save `loading`; `201`/`200` → toast "Rule saved" + close + feed reflects on next match; `404` on edit (rule gone) → toast + close + refresh list; unsafe-key inline errors block save for that field.

### 2.4 Rules manager — AC-11

Reference: `partials/rule_row.html`. An overlay (`/d/:token/rules`) listing `GET …/rules` **ordered priority→id** (AC-11 — show the ordering explicitly with a priority column so users understand "first match wins"). Each `RuleRow`: drag-free priority badge · name · method+path summary · `enabled` `Switch` (optimistic PATCH) · row `DropdownMenu` (Edit · Duplicate · Delete). Header: "+ New Rule". States: loading (text/skeleton), error (`alert` + retry), **empty** ("No rules yet — unmatched requests use Auto-CRUD / proxy / default" + "+ New Rule" — teaches the resolution order). Delete → confirm `Dialog` (danger), `204` → remove + toast.

### 2.5 Endpoint settings — AC-22, AC-27, AC-28..AC-40, AC-48

Reference: `partials/endpoint_settings.html`. Overlay `Dialog`/sheet (`/d/:token/settings`), single scrollable column, grouped sections (each a labeled card). Binds to `EndpointConfigPatch` (PATCH `…/endpoints/:token`, AC-5 management) with **optimistic + reconcile**; clamps mirror §5.3.

1. **Identity** — `name` (≤100), the mock + path URLs (copy-only chips), token (read-only mono).
2. **Proxy target (MITM)** — `target_url` `Input` (http/https + host or empty to clear; inline `422` on invalid, AC-32). Helper notes: matching local rules always win (AC-28); SSRF-blocked targets return `502` (AC-29) — surface this as helper text, not a surprise.
3. **Auto-CRUD** — `Switch` (mirrors the dashboard quick toggle). When on, show a **Collections peek** affordance: list/clear a collection via `GET/DELETE …/collections/:name` (AC-27); unsafe name → `422` inline.
4. **Default mode** — `RadioGroup` `mock_404` | `echo` (AC default behavior).
5. **Simulated conditions** — `latency_ms` (`Slider` 0–10000), `rate_limit_per_min` (number, 0=unlimited, clamp ≤100000), `chaos_pct` (`Slider` 0–100). Helper documents the frozen wrap order **rate-limit → chaos → latency** (AC-37). **`chaos_mode`** (`error` random-5xx vs `dropout` connection-drop) is surfaced *only if* the architect promotes it to the schema (OQ-2) — flagged as a gap; design assumes a `Select` placeholder gated on that decision.
6. **CORS** — `cors_enabled` `Switch`; helper: auto-handles `OPTIONS` preflight and reflects Origin on the mock surface only (AC-33/34/35).
7. **Retention & state** — read-only note of the 100-trace cap + 24h TTL (AC-46/47); **Clear request history** (`DELETE …/requests`, AC-48), **Clear state** (`DELETE …/state`, AC-22), **Clear collection** (per-name) — each a danger action behind a confirm `Dialog`.
8. **Danger zone** — **Delete endpoint** (`DELETE …/endpoints/:token`; tombstoned → `410` on the mock plane, AC-57) behind a typed/explicit confirm; on success route to another endpoint or the empty dashboard.

States: each section saves independently (per-field PATCH or grouped Save); optimistic toggles reconcile on response; validation inline; destructive actions always confirm and name the target.

### 2.6 Tunnel / CLI guidance — AC-49..AC-51

Reference: `FEATURES.md §12`. A focused `/cli` page (also reachable from settings + the empty feed). Not interactive beyond copy. Content:
- What the tunnel does (reverse-tunnel public mock traffic to localhost), labeled `tunnel` in the feed (AC-50).
- The exact command in a `CodeBlock` with copy: `tunnel --port 3000 --endpoint <token> --secret <owner_secret>` (the Rust bin replaces `python -m tunnel`, AC-49). **The `<owner_secret>` is the live capability** — render it pre-filled from storage *with a reveal/copy* control and a clear warning that it is a secret (don't paste it publicly).
- A short note on behavior: one authenticated WS bind, last-bind-wins takeover, `504 no_tunnel` when nothing is connected (AC-50/51) — set expectations so a `504` isn't mistaken for a bug.
- A live-ish hint: the dashboard `EndpointDetail.tunnel_active` flag can drive a small "tunnel connected" indicator in the dashboard sub-header.

States: copy-to-clipboard with toast + copied state; secret hidden by default (reveal toggle); offline-safe (all static text).

---

## 3. Component inventory

Reuse the reference `ui/` primitives **as-is in approach** (CVA + Radix + tokens); add HookBox-specific ones. Each lists variants and the full state set. States are a contract for frontend + a checklist for QA.

### 3.1 Reused from `../shortener-link/src/components/ui/` (adopt directly)
- **`Button`** (`button.tsx`) — variants `primary|secondary|ghost|danger|link`, sizes `sm|md|lg|icon|icon-sm`; states default/hover/active/focus-visible(ring)/disabled/**loading**(spinner, width preserved, `aria-busy`). Icon buttons require `aria-label`.
- **`Input` / `Textarea`** (`input.tsx`) — label always present (may be `sr-only`); `mono` flag (HookBox uses it heavily); states default/focus/filled/**error**(`aria-invalid`+`aria-describedby`)/disabled/readonly.
- **`Segmented`** (`segmented.tsx`) — for theme toggle, body Pretty/Raw, range/mode pickers; `role="group"`, `aria-pressed`, keyboard-operable.
- **`Switch`** (`switch.tsx`, Radix) — Auto-CRUD, CORS, rule enabled; `role="switch"`, labeled.
- **`Dialog`** (`dialog.tsx`, Radix) — rule builder, settings, confirms, mobile drawer; focus-trap, Escape, scroll-lock, scrim, focus-restore; mobile → bottom/full sheet.
- **`DropdownMenu`** (`menu.tsx`, Radix) — endpoint account menu, rule row actions; roving focus, type-ahead, Esc.
- **`Toast`** (`toast.tsx`, Radix) — `success|error|info`; `aria-live` polite/assertive; top-right desktop / top-center mobile; never the sole confirmation.
- **`Tooltip`** (`tooltip.tsx`, Radix) — hover+focus; never sole carrier of essential info.
- **`Skeleton` / `Spinner`** (`skeleton.tsx`, `spinner.tsx`) — loading; static block under reduced-motion.
- **`StatusBadge` pattern** (`status-badge.tsx`) — the **icon+text, never-color-alone** convention is the model for HookBox's `MethodBadge`/`ServedByChip`/`ConnectionPill`.
- **`CopyButton`** (`copy-button.tsx`) — the canonical copy pattern: copies → toast + transient copied state + `sr-only` `aria-live` announce + textarea/`execCommand` fallback for insecure contexts. **Reuse verbatim** (the current Jinja `copyToClipboard` already matches this behavior).
- **`AppShell` / `PageHeader`** (`app-shell.tsx`) — skip-link, sticky top bar, Radix-Dialog mobile drawer; adapted to endpoint-scoped content (§1.2).
- **`ThemeToggle`** (`theme-toggle.tsx`) + theme provider — System/Light/Dark segmented, persisted.

### 3.2 New HookBox primitives (same foundation)
- **`MethodBadge`** — colored method pill (GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS), CVA-variant by method; **text label is the source of truth** (color is secondary, grayscale-safe). Used in feed rows + inspector header.
- **`StatusCode`** — tabular-figure status number, colored by class (2xx/3xx/4xx/5xx) but the *digits* are always legible without color.
- **`ServedByChip`** — chip for the 8 served-by values (rule/crud/mitm/tunnel/default/cors/chaos/ratelimit, AC-56); icon+text; distinct, color-blind-safe set (design-agent finalizes the palette).
- **`ConnectionPill`** — WS/SSE health (`connecting|live|reconnecting|sse|offline`); icon+text+dot; `role="status" aria-live="polite"`.
- **`FeedRow`** — composes MethodBadge + path + StatusCode + ServedByChip + latency + time as a focusable `role="option"`; new-row enter affordance (§4).
- **`KeyValueRows`** — read mode (headers/query/state display, per-row copy) and edit mode (add/remove pairs, for the rule builder). Keys mono.
- **`JsonTree`** — collapsible JSON tree (Pretty) + Raw `<pre>` toggle + Expand/Collapse all + Copy; capped node count; **every value a text node** (XSS-inert). Mirrors the current `bodyTree`.
- **`CodeBlock`** — mono block (mock URLs, curl sample, CLI command) with an integrated `CopyButton`; static-text only.
- **`Tabs`** — thin Radix Tabs wrapper for the inspector (5 tabs) and rule builder (5 tabs); arrow-key + Home/End nav, `role=tablist/tab/tabpanel`.
- **`Slider`** — labeled range for latency/chaos with a paired number input (never slider-only — keyboard + exact entry).
- **`SplitPane`** — the dashboard's resizable-ish 40/flex layout; stacks under `--bp-md`.
- **`InlineAlert`** — persistent in-context alert (info/warn/danger) for rate-limit/error explanations (from `guest-hero.tsx`'s pattern).

### 3.3 Global state set (applies to every interactive component)
default · hover · focus-visible (visible non-color ring, never removed) · active/pressed · disabled (muted, `cursor-not-allowed`) · loading (`aria-busy`) · empty · error · success. Every list/feed/inspector tab/form designs all four async states (loading/empty/error/success) first-class.

---

## 4. Real-time feed behavior (the interaction that defines HookBox)

Binds to §5.4 (WS `GET /ws/:token?cap=`; SSE `GET /sse/:token?cap=`; events `hello`, `new_request`, `state_changed`, `endpoint_updated`).

- **Connect:** `useRequestStream(token, { cap })` opens the WS with `?cap=<owner_secret>`; on `hello{token, server_time}` the pill goes **live**. Auth fails (close `4401` / SSE `401`) → pill **offline** + a one-line "Not authorized for this feed" (should not normally happen for the owner). At capacity (close `1013` / SSE `503`) → pill shows "Feed busy — too many connections" (AC-43).
- **New row (`new_request`, the `RequestSummary` shape):** prepend to the feed list. **Enter affordance:** a brief highlight/slide on the new row (`--dur-base`), **fully removed under `prefers-reduced-motion`** (instant insert). The list is `aria-live="polite"` so SR users hear new activity *only when not paused* (set `aria-live="off"` while paused to avoid a flood).
- **Autoscroll vs pause:** while **live** and the user is at the top, the list auto-follows new rows. The moment the user scrolls down or hits **Pause**, auto-follow stops and incoming rows buffer; a **"N new"** pill shows the buffered count; clicking it flushes + scrolls to top (parity with current `feed.paused/pending/flush`). This is the standard log-viewer affordance and prevents the feed yanking the viewport while inspecting.
- **Selection persistence:** selecting a row pins the inspector; new rows arriving do **not** change the selection. Selection survives pause/resume. `?req=:id` deep-links it.
- **`state_changed` / `endpoint_updated`:** update the relevant store quietly (state snapshot, endpoint config) — refresh open settings/inspector without a full reload; no toast spam (a subtle "updated" pulse at most).
- **Reconnect:** drop → pill **reconnecting** + backoff retry; on reconnect, the server resends `hello`; the client back-fills recent rows via `GET …/requests?limit&offset` (AC-48) so a brief disconnect doesn't lose context.
- **SSE fallback:** if WS fails to open (proxy strips upgrades), transparently fall back to SSE delivering the identical event stream; pill shows **sse** so the user knows. `: ping` heartbeats keep it alive (§5.4).
- **Empty-until-first-call:** the first-call funnel (§2.2) is the default state until the first `new_request`.

---

## 5. Keyboard accessibility & responsive

- **Skip-to-content** link is the first focusable element (from `app-shell.tsx`).
- **Feed:** the list is focusable; `↑/↓` move selection, `Enter` inspects, selection `scrollIntoView({block:'nearest'})` (parity with current `moveSelection`). `Home/End` jump to newest/oldest. **Space** toggles Pause when the feed header is focused.
- **Inspector & rule-builder tabs:** Radix Tabs — `←/→` cycle, `Home/End` to ends, `role=tablist/tab/tabpanel` with `aria-controls`/`aria-selected`/roving `tabindex` (parity with current `inspectorPanel`).
- **Dialogs (rule builder, settings, confirms, mobile drawer):** Radix focus-trap, Escape to close, scroll-lock, focus restored to the trigger.
- **Menus / selects / switches / sliders:** Radix-backed keyboard semantics; sliders also have a paired number input for exact entry.
- **Copy controls:** keyboard-reachable, `aria-label` includes what's copied, `sr-only aria-live` confirms beyond the toast.
- **Focus visibility:** a visible focus-visible ring on everything interactive, ≥3:1 against adjacent fill, never removed (`outline:none` only when replaced by the ring).
- **Responsive:** `<768px` the split pane stacks (feed → selected inspector with "← Feed" back); the endpoint sub-header actions collapse into the drawer/menu; long mono strings truncate with ellipsis and reveal on tap/focus; **never horizontal-scroll**; tap targets ≥44×44px. Rule builder + settings become full-screen sheets.

---

## 6. Accessibility — WCAG 2.1 AA, both themes

- **Two themes, both AA.** Semantic-token theming (no hardcoded hex in components — explicitly *unlike* the current Jinja templates). First visit follows `prefers-color-scheme`; explicit toggle persists. HookBox is dark-first by identity, but light must meet the same bar. The design-agent owns the final token values and must verify: body text ≥4.5:1, large/UI-boundary ≥3:1, status/served-by/method chip `fg` on its `bg` ≥4.5:1, in **both** themes.
- **Never color alone.** `MethodBadge`, `StatusCode`, `ServedByChip`, `ConnectionPill`, and every status carry an icon and/or text label; verifiable in grayscale. Status-code class is conveyed by the digits + an icon, not hue alone.
- **Screen-reader semantics:** the feed is a `listbox` of `option`s with `aria-selected`; `aria-live=polite` (gated off while paused); inspector/rule tabs use real `tablist/tab/tabpanel`; `KeyValueRows` use real definition or table semantics; `aria-busy` on loading regions; `aria-invalid`+`aria-describedby` on field errors; icon-only buttons always `aria-label`.
- **XSS-inert capture rendering:** every captured request/response value (headers, query, body, trace) renders as a **text node** (React escapes by default; `JsonTree`/`KeyValueRows` never `dangerouslySetInnerHTML`). This preserves the current inspector's hard rule and protects against payloads that probe the dashboard.
- **Motion:** `prefers-reduced-motion: reduce` → all transitions 0ms, new-feed-row highlight becomes an instant insert (no slide), skeleton shimmer → static block, no auto-advancing motion; the `ConnectionPill` never relies on a spin as the sole cue (text label carries it).
- **Forms:** every input programmatically labeled; errors are text (not color-only) and associated to the field; required/clamped state announced.

---

## 7. Consistency notes — patterns reused

- **Token-driven theming + Tailwind semantic colors + `.dark` strategy** — directly from `../shortener-link` `DESIGN.md §2` and its components. (Fixes the current templates' inline-hex anti-pattern.)
- **CVA variant components** — `Button` (`button.tsx`) is the template for `MethodBadge`/`ServedByChip`.
- **Radix for all overlays/menus/tabs/toasts** — `dialog.tsx`, `menu.tsx`, `toast.tsx`, `segmented.tsx`, `tooltip.tsx`, `app-shell.tsx`'s mobile drawer.
- **CopyButton behavior** (toast + copied state + fallback + `sr-only` announce) — reused verbatim from `copy-button.tsx`; matches the current Jinja `copyToClipboard`.
- **AppShell skeleton** (skip-link, sticky top bar, mobile drawer, `PageHeader`) — from `app-shell.tsx`, adapted to endpoint scope.
- **Auth-gated protected route group** — the `router.tsx` `ProtectedRoute` pattern, adapted to localStorage-capability auth instead of NextAuth.
- **Four-states-everywhere + never-color-alone + visible-focus** principles — from `DESIGN.md §1`.
- **Behavioral parity with the current dashboard** — feed pause/buffer/"N new", keyboard feed nav, 5-tab inspector, Pretty/Raw JSON tree, first-call empty funnel, mobile "← Feed" back bar, copy-only mock chips — all carried over from `templates/dashboard.html` + `partials/inspector.html` (read, not ported as code).

---

## 8. PRD gaps — numbered list for the PM to add or clarify

1. **App shell IA decision (no left-nav).** The PRD/spec inherit shortener-link's "persistent left nav" mental model, but HookBox is single-endpoint-scoped. This doc proposes **no section rail**: endpoint switcher + actions in the top bar / dashboard sub-header. The PM should confirm this IA (and whether multi-endpoint management warrants its own list screen, vs. just the switcher).
2. **`chaos_mode` in the settings UI (ties to OQ-2).** AC-40's opt-in connection-drop needs `chaos_mode` ("error"|"dropout") to be a first-class field. It is **not** in the frozen §5.3 `EndpointConfigPatch`/`EndpointDetail`. The settings screen can't render the control until the architect promotes it. PM: resolve OQ-2 and add the field (or explicitly mark dropout out-of-parity so the UI omits it).
3. **`webhook_action` honesty.** It is accepted-and-stored-but-no-op (non-goal / OQ-9). The Actions tab must label it "stored, not yet sent." PM: confirm the exact disclaimer wording is in scope for the copywriter, and that shipping a visible-but-inert control is acceptable (vs. hiding it).
4. **Template "preview" expectations.** AC-16/17/18 guarantee unknown tags render verbatim and never error — so the Templating tab is a discoverability palette, **not** a validating preview. The PRD doesn't say whether a live render preview is desired. If it is, it must run the *same* closed scanner (no general engine). PM: clarify whether a preview is in scope; if yes, the architect must expose a preview endpoint or the FE must reimplement the scanner (duplication risk).
5. **`410 endpoint_gone` surfacing (ties to OQ-1).** AC-57 distinguishes `410 gone` from `404 unknown`, but OQ-1 leaves the tombstone mechanism unfrozen. The dashboard "endpoint not found/gone" state can't differentiate the two messages until the architect decides. PM: resolve OQ-1 so the UI can (or explicitly can't) show distinct copy.
6. **Owner-secret exposure on the CLI page.** The tunnel command needs the live `owner_secret`. Pre-filling it is convenient but exposes a rotating capability in the UI. PM: confirm the UX (reveal-on-demand + copy + secret warning) is acceptable, and whether the secret should ever be shown in plaintext vs. only copied.
7. **Reconnect back-fill ordering.** §5.4 has no "replay since cursor" — after a WS drop we back-fill via `GET …/requests` (AC-48), which could duplicate or gap rows around the reconnect boundary. PM/architect: confirm the client-side de-dupe-by-id strategy is sufficient, or whether the feed needs a since-id parameter.
8. **`endpoint_updated` / `state_changed` UX scope.** §5.4 emits these but the PRD doesn't specify the dashboard's reaction (silent refresh? toast? highlight?). This doc proposes a quiet refresh with no toast. PM: confirm.
9. **Multi-endpoint empty state.** What does a brand-new owner with exactly one auto-provisioned endpoint (AC-3) but zero rules/traces see first? This doc routes them to the first-call funnel on the dashboard. PM: confirm there's no separate onboarding/welcome screen expected.
10. **Light-theme commitment.** Spec §10 says "fresh, light"; the current product is dark. This doc ships **both** themes AA, dark-first. PM/design-agent: confirm whether "light" means a light *default* or merely "feather-weight," and which theme is the first-paint default.
