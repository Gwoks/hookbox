# PRD: Operator Toolkit — six operator affordances + one enabling backend fix  (slug: operator-toolkit)

**Mode:** REVISE — folds `journey.md`, `ux.md`, `design.md`, `copy.md`, `architecture.md` and
`security.md` into one lockable document. **§5 is lifted from `architecture.md` §2 (authoritative)**,
with the one contract delta `security.md` requires (the public response-header filter, §5.5.5/§5.11).

**Stack correction, carried from three downstream docs.** The originating brief described HookBox as
FastAPI + `aiosqlite` + pydantic + Jinja templates in `templates/`. **None of that exists in this
repo.** HookBox is **Rust + axum 0.7.9 + sqlx 0.8/SQLite (WAL) + tokio** with a **React 18 + Vite +
zod + react-router SPA**, behind nginx in the deploy target
**[existing — verified at `backend/Cargo.toml`, `package.json`, `deploy/nginx.conf`; there is no
`templates/` directory]**. Every contract below is `serde` structs ⇄ `zod` schemas.

Verification legend:
- **[existing — verified at `path:line`]** — read in this repo during this pass.
- **[new — to be created]** — does not exist yet.

---

## 0. Revision log — the eleven conflicts between the six docs, and how each was decided

Each item is a real contradiction or defect found by a downstream agent. Every one is resolved with a
decision, not a hedge. Nothing here remains open.

| # | Conflict | Decision |
|---|---|---|
| **1** | **`security.md` S-1/S-2/S-4: F4 as drafted is not read-only.** `identified()` stamps `x-hookbox-endpoint: <token>` and `x-hookbox-rule-id` on **every** mock response **[existing — verified at `backend/src/interceptor/engine.rs:584-588`]**, and `spawn_trace` persists that map verbatim **[existing — verified at `:636-644`]**. AC-44a would publish it to anonymous viewers, handing them the endpoint token — which lets them `POST /e/<token>` to write into the endpoint, evict the shared evidence past `TRACE_CAP`, mutate CRUD collections, and make HookBox issue attacker-chosen requests to `target_url`. AC-34 (forbids `token`) and AC-44a (publishes `response_headers`) directly contradicted each other. | **MUST-fix, adopted in full.** `security.md`'s AC-S1..AC-S27 are folded in as numbered, **required** ACs (§4.9), keeping the `AC-S` prefix (see §0.1). The fix is **one deny-list applied only at the public share projection** (§5.5.5, §5.11): drop every `x-hookbox-*` header, mask the cookie/auth header family. Storage stays verbatim, the owner Inspector stays verbatim, F5's CSV stays verbatim — exactly `security.md`'s S-4 ruling. **F4 cannot ship without AC-S1, AC-S2 and AC-S4.** |
| **2** | **Share-code storage: `architecture.md` D9/D10 chose HASHED (`code_hash` + non-secret integer `id`); `security.md` §4(10) independently approved PLAINTEXT.** Cascade: `ux.md` §2.5 designed per-row copy + per-row Preview, which is unimplementable when the raw code is never stored. | **HASHED + integer `id` — `architecture.md` wins**, per `security.md`'s own §4 note ("treat architecture's hashed+id design as authoritative … Do not re-open R3 in favor of plaintext"). The decisive fact is D10, not at-rest paranoia: `DELETE /api/endpoints/{token}/shares/{code}` would put a live bearer credential in the owner-side nginx access log under `location /api/` **[existing — verified at `deploy/nginx.conf:18-25`]**, and no nginx prefix can exclude it without unlogging all endpoint routes. **Revoke is by `id`** (§5.1). **`design.md` §3.6's shown-once panel is the frozen UX** and `copy.md` §4.5 is the frozen copy. AC-24/AC-25/AC-26 are rewritten (§4.4); `ux.md`'s per-row URL, per-row copy and per-row Preview are **deleted**; `share.row.open*` keys are dropped in favour of `share.created.open*`. |
| **3** | **`copy.md` §7.1: a direct self-contradiction.** `ux.md` §2.5's disclosure claims response headers are "shown exactly as sent, including any Set-Cookie from your upstream" — which AC-S1 makes **false**. | **Resolved in favour of `security.md`.** The redaction is a MUST, so the disclosure must not claim total transparency. Frozen copy is `copy.md`'s `share.warning.body` + `share.warning.redaction` (§4.5 there): bodies and both header maps are shown **minus a stated list** — `Authorization`/`Cookie`/`X-Owner-Id` request headers, plus `Set-Cookie`/`Authorization`-family response headers and every internal `x-hookbox-*` header. `ux.md`'s wording is superseded (AC-S11, AC-93). |
| **4** | **`journey.md` BLOCKER 36 / E21-E22: F6's catch-all silently disables Auto-CRUD, tunnel, MITM and `default_mode`.** A matched rule returns before `resolve_unmatched()` is ever reached **[existing — verified at `backend/src/interceptor/engine.rs:141-145` (matched branch) vs `:228-245` (the `else` that calls `resolve_unmatched`)]**. `priority = 1000` orders rules against *each other* only and provides **no** protection. R8 only ever mentioned "404 becomes 200". | **F6 may not be a silent one-click action on such an endpoint.** Adopted: a **conditional confirm** (AC-122) using `copy.md`'s `rules.default.shadow.*`, rendering one bullet per fallback that is *actually* active (`auto_crud`, `tunnel_active`, `target_url`, `default_mode === "echo"`) plus the recovery line. **Zero applicable bullets ⇒ no confirm at all**, straight to the POST — so the common "make a new endpoint answer something" path stays one click. Not a refusal: the operator may knowingly want this. Plus a backend regression test that `auto_crud = true` + a catch-all is served by `rule`, not `crud` — so the behaviour is chosen, not discovered in production. |
| **5** | **`journey.md` BLOCKER 23: F3's import cannot update the form, and the next Save reverts it.** `SettingsForm` is mounted with **no `key`** **[existing — verified at `src/screens/settings.tsx:138-143`]** and seeds all nine fields with `useState(endpoint…)` **[existing — verified at `:161-170`]**, so AC-20's "re-fetch" cannot refresh the visible fields. | **Adopted as AC-89.** Mechanism is the implementer's choice (remount via `key`, or reset field state from an effect keyed on the fetched endpoint), but the AC is stated as an observable: after a successful import the on-screen form shows the imported values **before any further Save is possible**, and a Save issued immediately after an import `PATCH`es the **imported** values, never the pre-import ones. Both halves are asserted. |
| **6** | **`journey.md` BLOCKERs 1 & 2: F4's poller has no failure lifecycle, and a detail 404 is indistinguishable from a dead link.** AC-45 specified a bare fixed 5 s interval. AC-36 makes unknown/revoked/tombstoned byte-identical, and AC-35 makes a stale request id 404 too — so one stale row click would render the terminal "unavailable" page and stop polling. | **Frozen as AC-105 + AC-106 (+ AC-S8).** A **list**-level 404 is terminal (stop polling permanently). A **detail**-level 404 is **never** terminal — it renders `viewer.detail.gone.*` ("…The rest of the list still works.") because a row can 404 merely from `TRACE_CAP`/`TRACE_TTL_HOURS` eviction or an owner Clear all. 429 pauses for at least `Retry-After`; 5xx/network backs off exponentially to a ceiling; polls never overlap; resume on `online` and `visibilitychange → visible`; abort in flight on unmount. |
| **7** | **`journey.md` gap 8 (HIGH), `copy.md` §7.9: the copy button lies.** `setCopied(true)` runs **outside** the `try/catch` around `navigator.clipboard.writeText` **[existing — verified at `src/components/ui/copy-button.tsx:22-31`]**, and the shipped nginx listens on plain HTTP:80 **[existing — verified at `deploy/nginx.conf:1-2`]** — a non-secure context where the Clipboard API is unavailable. | **In scope as AC-132.** Promoted from "pre-existing debt" because F4's share URL is shown **exactly once**: a silent copy failure loses the link with no recovery. Success state only after the write resolves; on rejection, a distinct failure state plus a manually-selectable fallback (`common.copy.failed`). |
| **8** | **`ux.md` gap 23: the redaction pill has never rendered.** `key-value-rows.tsx` compares against `'__redacted__'` **[existing — verified at `src/components/hookbox/key-value-rows.tsx:9`]** but the backend writes `'<redacted>'` **[existing — verified at `backend/src/helpers.rs:43`]**. | **In scope as AC-133.** One-constant fix. Required here because AC-S1 makes redaction load-bearing on an **unauthenticated** page: "HookBox hid this" must be a neutral chip, not a literal string that looks like a value the caller sent. |
| **9** | **`design.md` §2.1/§8/§9.3: three pre-existing CSS defects that new code would inherit.** (a) `bg-subtle`/`bg-hover`/`bg-active` generate **no CSS** — the real utilities are `bg-surface-subtle`/`-hover`/`-active` **[existing — verified at `tailwind.config.ts:20-27`: these live under `colors.surface.*`]**, and 12 call sites are silently unstyled. (b) `MenuItem`'s `focus:text-text-primary` **[existing — verified at `src/components/ui/menu.tsx:38`]** out-specifies a consumer's `text-danger-fg`, so destructive red vanishes on hover/focus. (c) `variant="danger"` hardcodes `text-white` **[existing — verified at `src/components/ui/button.tsx:21`]**, ≈1.7:1 on `--red-400-dark` — an AA failure in dark theme. | **Fix in this batch, precisely scoped; no repo-wide sweep** (AC-84, AC-129, AC-130). In scope: (i) all **new** code uses `bg-surface-*` and never `bg-subtle`/`-hover`/`-active`; (ii) the dead classes are fixed **only at sites inside files this batch already edits** — `rules-manager.tsx:232`, `app-shell.tsx:104` and `:163`, `feed-row.tsx:64-65`, `inspector.tsx:266` — because the new viewer column band must match the rules band it copies; (iii) the `MenuItem` specificity fix, applied to the new feed-actions destructive item **and** the existing rules Delete item it is copied from; (iv) the one-token `variant="danger"` contrast fix (`text-white` → `text-text-on-accent`), because it is a shared primitive that F1's confirm and F4's revoke both consume. **Out of scope, follow-up issue filed at BREAKDOWN:** the remaining dead-class sites `code-block.tsx:22,42`, `json-tree.tsx:81`, `tabs.tsx:19`, `segmented.tsx:25`, `connection-pill.tsx:59`, `rule-builder.tsx:518`, `slider.tsx:30`, plus the product-wide `text-overline`+`tracking-wide` and `border-danger-fg/40` alpha-modifier findings (`design.md` §9.3 items 3 and 5). |
| **10** | **`security.md` OQ-S3 + `architecture.md` D15: is `SHARE_RATE_LIMIT_PER_MIN = 60` defensible?** One viewer polling at 5 s burns 12 req/min on the list route alone; the shipped compose topology has **no nginx** **[existing — verified at `docker-compose.yml`: a single app service with a `ports:` mapping and no proxy]**, so viewers can collapse into one bucket (S-6b). | **Adopted: `SHARE_RATE_LIMIT_PER_MIN = 120` per IP** (architecture's D15 number, ≈10 concurrent polling viewers behind one NAT egress IP) **plus a new global ceiling `SHARE_RATE_LIMIT_GLOBAL_PER_MIN = 1200`** (security.md's AC-S15 recommendation, ≈100 concurrent viewers instance-wide, bounding worst-case egress at 1200 × 512 KB/min). Both frozen in §5.8 and asserted by AC-113. Stated honestly in §8-R14: the limiter fails open on anomaly and evicts idle buckets past `MAX_BUCKETS = 100_000` **[existing — verified at `backend/src/limiter.rs:43`, `:88-89`, `:130-142`]**, so this is a **courtesy limit, not a guarantee**. |
| **11** | **Numbering drift.** All four downstream docs cite draft AC numbers (AC-25, AC-34, AC-37, AC-44a, AC-56a, AC-61, AC-63, AC-68..75, AC-S1, AC-S11, AC-S21, AC-S23 …). ~60 new ACs land in this revision. | **AC-1..AC-75 (plus AC-44a, AC-56a) keep their numbers and are amended in place; new ACs are a contiguous block AC-76..AC-136; `security.md`'s ACs keep the `AC-S1..AC-S27` prefix** (`design.md`, `copy.md` and `security.md`'s own machine-readable summary all reference them by that name). Nothing is renumbered, so **every cross-reference in all six documents still resolves.** §0.1 records the amended ACs; §11 records the final consistency pass. |

### 0.1 ACs amended in place (numbers unchanged, text changed)

| AC | Amendment | Source |
|---|---|---|
| AC-1 / AC-46 | Placement is "in the feed header's action group, which is an overflow `Menu`" — **not** literally "to the left of pause/resume", which cannot fit at the feed pane's 360 px `min-w-feed` **[existing — verified at `tailwind.config.ts:113-115`, `src/components/hookbox/split-pane.tsx:53`]**. Enable predicate moves to AC-76; visual weight to AC-84. | `ux.md` gaps 1–2, `design.md` §3.1 |
| AC-2 | The confirm body carries **no count** and states the blast radius is wider than the visible list. | `ux.md` gap 3, `copy.md` §2.1, `journey.md` 18 |
| AC-4 | "Immediately renders its empty state" is qualified by the arrival race (AC-80). | `journey.md` 21, `copy.md` §7.17 |
| AC-13 | Drop "built by composing the existing `endpointConfigPatchSchema`" — that schema makes all nine fields `.optional()` and is not `.strict()` **[existing — verified at `src/api/schemas.ts:48-58`]**, so composing it would pass a bundle missing every field. The bundle's `endpoint` is a **separate `.strict()` schema with all nine fields required** (§5.5.6). | `architecture.md` D13 |
| AC-20 | "The Settings form reflects server state" is not achievable by re-fetching alone — see AC-89. | `journey.md` BLOCKER 23 |
| AC-23 | Share sits **first in the sub-header's right action cluster**, not inside the left subject cluster, preserving `AppShell`'s documented left=subject / right=actions split **[existing — verified at `src/components/hookbox/app-shell.tsx:4-7`]**. Count badge in AC-98. | `ux.md` gap 12, `design.md` §3.5 |
| AC-24 | The 201 response is the **only** place `code` and `url` ever appear; the dialog shows them in a shown-once panel. | Item 2 |
| AC-25 | List rows show `id`-keyed content — **label, created-at, last-used, Revoke only. No URL, no copy action, no Preview.** | Item 2 |
| AC-26 | Revoke is `DELETE /api/endpoints/{token}/shares/{id}` — **by non-secret integer `id`, never by code.** | `architecture.md` D10 |
| AC-34 | Unchanged in intent, and now **true**, because AC-S1's filter removes `x-hookbox-endpoint`/`-rule-id` from `response_headers`. The omission list governs *projection keys*; AC-S1's filter governs *keys inside `response_headers`*. | `security.md` §4(1)-(2) |
| AC-37 | Narrowed to **handler-produced** responses (200/404/422/429/503). axum's auto-405 is framework-generated with an empty body and no custom header; it carries no user data. | `architecture.md` D14 |
| AC-38 | `SHARE_RATE_LIMIT_PER_MIN` default **120**, namespaced key `share:<ip>`, checked **before any DB read**, `HEAD` counted, plus the global ceiling. | Item 10, `security.md` AC-S7/AC-S14 |
| AC-44 | The state list gains **detail-gone** (AC-106) and is subject to AC-107..AC-112. | `journey.md` 1, 13, 14 |
| AC-44a | Unchanged in intent: all five body/header fields stay **present keys**. `response_headers` is now explicitly a **filtered** map (§5.5.5) — that is what makes AC-34 true. | `security.md` §4(1)-(2) |
| AC-45 | Superseded on lifecycle by AC-105; the 5 s cadence and the visibility gate survive. | `journey.md` BLOCKER 2 |
| AC-55 | Drop "(quoted per AC-54)". `=cmd\|' /c calc'!A1` contains no `,`, `"`, CR or LF, so the expected cell is the **unquoted** `'=cmd\|' /c calc'!A1`. Ordering is frozen as **guard, then quote**. | `architecture.md` D12 |
| AC-56 | Still "no client-side redaction". The values it describes are unchanged for the CSV — `security.md`'s S-4 ruling **accepts** verbatim response headers for the owner Inspector and the CSV. Only the **public** projection filters. AC-S25 adds the echo-body assertion. | `security.md` S-4, AC-S25 |
| AC-58 | The frozen payload is `§5.5.7` **as amended** by `copy.md` §4.7 (`rules.default.ruleName`, `rules.default.bodyTemplate`) — see AC-125. Any fixture asserting the old bytes must be updated. | `copy.md` §7.4 |
| AC-61 | Predicate extended: a **disabled** catch-all must also block a second one, the control disables while the POST is in flight, and a stale list is handled by refresh-and-explain rather than silent duplication — see AC-123. Keyboard reachability of the reason is AC-124. | `journey.md` 37, `ux.md` gaps 24/26 |
| AC-73 | Sub-condition **(d) is replaced** by `architecture.md` §2.10.7's (d1)/(d2)/(d3). As written it was unfalsifiable: `overhead_ms` is quantised to whole milliseconds **[existing — verified at `backend/src/interceptor/engine.rs:632`: `t0.elapsed().as_millis()`]** and the baseline is 0–1 ms, so "10 %" is degenerate and "p95 ≤ 5 ms" passes even at 400 % slower. (a)/(b)/(c) stand. | `architecture.md` D8 |
| AC-74 | Still "no schema change **for F7**". F4 adds `0002_share_links.sql`; the security lane adds one Cargo **feature flag** (AC-S18) — neither is F7. | `security.md` AC-S18 |

---

## 1. Problem & goal

A HookBox operator today can create an endpoint, watch hits stream into the Live Feed, inspect one
request at a time, and hand-write mock rules — but every step *after* "it works on my screen" is
manual and lossy. There is no way to clear a noisy feed without deleting the whole endpoint, no way
to get the captured traffic out of the browser, no way to move an endpoint's configuration to another
endpoint or machine, no way to show a teammate or a vendor what their webhook actually sent without
handing over the owner secret (which grants full mutate/delete), and no one-click way to make an
endpoint answer *something* instead of 404 while wiring up a client. Underneath all of that sits a
data gap: HookBox never persists the **response body** it sent, so "what did my mock reply?" is
unanswerable in the Inspector, in an export, or in a shared link **[existing — verified at
`backend/src/interceptor/engine.rs:683`: `response_body: None`]**. The goal of this batch is to close
those six operator-facing gaps on the screens where the operator already is — the dashboard
sub-header, the Live Feed panel header, the Settings screen and the Rules Manager — plus one
genuinely new public surface (a read-only share link) so an operator can prove "here is what you sent
me" without granting any write capability, plus one enabling backend fix (**F7**) so that can be
accompanied by "and here is what I answered".

**F4 is the high-risk item.** It introduces HookBox's **first unauthenticated read surface over user
data** — a new trust boundary reachable by anyone holding a URL, with no session, no owner secret and
no login. `security.md` proved the draft projection was **not actually read-only** (§0 item 1); the
fix is a hard requirement, not a nicety. **F7 is the second-most sensitive**: it is the only change
touching the mock plane's hot request path, and it enlarges what F4 publishes.

---

## 2. Non-goals

Global:
- **No new mock-plane (P1) request-handling behavior.** Matching, templating, CRUD, MITM, tunnel,
  chaos/latency/rate-limit decisions and **every byte the client receives** are unchanged
  **[existing — verified at `backend/src/interceptor/`]**. Inside `backend/src/interceptor/engine.rs`
  there is exactly one additive change (F7: persist the response body already held in memory) plus
  the AC-S3 echo-payload redaction on the **persist** path only. `matcher.rs`, `proxy.rs`,
  `templating.rs`, `cors.rs` and every other file under `backend/src/interceptor/` are untouched.
- **No change to the 18 existing management routes' shapes.** One existing *handler behavior* is
  extended (§5.3); one existing *field value* stops being permanently `null` (§5.10).
- **No multi-tenant/team accounts, roles, or invitations.** The owner-capability model is unchanged.
- **No i18n work.** New strings are English literals in the single copy table
  **[existing — verified at `src/lib/copy.ts`]**.
- **No CSP.** `security.md` AC-S26 asks for a starter Content-Security-Policy; deferred to a
  follow-up issue because `index.html` carries an inline pre-paint theme script
  **[existing — verified at `index.html:13-30`]** that needs a nonce or hash strategy, and a wrong CSP
  breaks the whole SPA. The cheap, non-breaking subset of AC-S26 (`Referrer-Policy`,
  `X-Content-Type-Options`, `X-Frame-Options`) **is** in scope (AC-S26 as scoped in §4.9).
- **No landing-page work.** `copy.md` §4.9 drafts three landing feature blocks; they are recorded
  there for a future pass and are **not** in this batch (no AC covers them).
- **No AC-S12 (mint-time window scoping).** Decided against — see §4.9's AC-S12 entry.

Per feature:
- **F1 Clear all:** not a per-row delete, not an undo, not a soft delete. It calls the existing
  hard-delete route.
- **F2 Local path chip:** `path_url` is **not** removed from the API, the DB, or Settings → Identity.
  Path-based mock routing (`/e/<token>`) is untouched **[existing — verified at
  `backend/src/planes.rs:151`]**.
- **F3 Export/import:** no bulk-replace of rules (no such primitive exists), no cross-endpoint clone,
  no import into a *new* endpoint, no server-side import endpoint, no export/import of traces,
  endpoint state, or CRUD collections. **No undo** (AC-91 mitigates with a pre-apply diff).
- **F4 Share links:** **no live streaming for the viewer in v1** — no public WS, no public SSE; the
  viewer polls. No public access to rules, endpoint config, mock URL, token, owner secret, state,
  CRUD collections, or resolution traces. No password-protected or expiring links (revoke is the only
  control). No embed/iframe/OG-preview support (AC-S13, AC-S26 make that enforced, not merely
  unimplemented). No public write of any kind. **No re-viewing a minted URL** (§0 item 2) and **no
  per-row deep-linking** (AC-112). No `SHARE_LINKS_ENABLED` env flag — the deploy-level opt-out
  (`location /api/share/ { return 404; }` + `location /s/ { return 404; }`) is the accepted mechanism,
  confirming `architecture.md` §2.9, because a server flag would need a shape change to
  `GET /api/endpoints/{token}` for the SPA to hide the control.
- **F5 CSV export:** not a server-generated export, no date-range or filter UI, no history beyond
  what the feed holds (cap 100 = `TRACE_CAP` **[existing — verified at `backend/src/config.rs:156`,
  `src/feed/use-feed.ts:42`]**), no XLSX/JSON variants, **no comment line inside the file** (RFC 4180
  has none — resolving `security.md` OQ-S4; the redaction/sentinel note is UI-side, AC-121).
- **F6 Default catch-all rule:** **not** auto-created on endpoint creation, not a global default, no
  change to `default_mode` semantics, and **no matcher/engine changes** (already supported — AC-63).
- **F7 Response-body capture:** the narrowest possible fix. Out of scope: any general response-path
  refactor; streaming/chunked support; changing `MAX_BODY_BYTES`; a truncation flag/marker column or
  any new column, table or env var; capturing response bodies for the management plane (`/api/*`) or
  the feed/WS payloads; back-filling `response_body` for pre-F7 traces (old rows stay `NULL` forever).
  **The chaos-dropout trace's pre-existing low fidelity is not fixed** — confirming
  `architecture.md` R-DROPOUT: `spawn_trace` there receives a throwaway empty `Response` **[existing —
  verified at `backend/src/interceptor/engine.rs:299-321`]**, so the row keeps `status_code = 0` and
  `response_headers = {}` while the client receives 499 + `Connection: close`. F7 passes `&[]`, which
  yields the `NULL` AC-69 requires. Fixing the other two fields would silently rewrite
  already-persisted values outside F7's frozen scope and still could not make `status_code` truthful.
  A follow-up issue is filed at BREAKDOWN.

---

## 3. Users & context

**Persona 1: the endpoint owner ("the operator")** — an authenticated user holding an `owner_secret`
in browser storage **[existing — verified at `src/api/session.ts`, attached as `Authorization:
Bearer` at `src/api/client.ts:78-81`]**. Every action in F1, F2, F3, F5, F6 and F4's owner half runs
on a screen that already requires that secret:

| # | Feature | Screen / component (all owner-authenticated) |
|---|---------|----------------------------------------------|
| F1 | Clear all logs | Live Feed panel header — `FeedPane` **[existing — verified at `src/screens/dashboard.tsx:289-351`]**, in the action group at `:325-350` |
| F2 | Remove "Local path" chip | Dashboard sub-header — `AppShell` **[existing — verified at `src/components/hookbox/app-shell.tsx:100-101`]** |
| F3 | Export / import config JSON | Settings screen — `SettingsForm` **[existing — verified at `src/screens/settings.tsx:149-482`]** |
| F4 owner | Share (mint/list/revoke) | Dashboard sub-header, **right** action cluster **[existing — verified at `src/components/hookbox/app-shell.tsx:119-143`]** |
| F5 | Export request log as CSV | Live Feed panel header — same action group as F1 |
| F6 | Add default rule | Rules Manager toolbar + empty state **[existing — verified at `src/screens/rules-manager.tsx:191-194`, `:218-227`]** |
| F7 | Response-body capture | **No new UI.** Visible in three existing places: the Inspector's Response-body panel stops always showing its empty state **[existing — verified at `src/screens/dashboard/inspector.tsx:246-253`]**, F5's CSV cell carries content, and F4's public detail does too |

**Persona 2, new with F4: the "viewer"** — *anyone* holding a share URL. The viewer:
- has **no** HookBox account, **no** session, **no** owner secret, and never obtains one;
- lands on a new public SPA route `/s/:code` **[new — to be created]** with no login prompt;
- can read the endpoint's recent request history (list + per-request detail) and nothing else;
- is untrusted and potentially adversarial: the link may be forwarded, indexed, pasted into a ticket,
  or brute-force scanned. `security.md` further establishes that the viewer page is **same-origin
  with the dashboard**, where the owner secret lives in `localStorage` — so XSS on a page made
  entirely of attacker-supplied text is owner takeover (AC-S13, AC-S15);
- is very likely on a **phone** (links arrive by chat, ticket and email), which is why AC-110 exists.

**Voice split, frozen by `copy.md` §1.1:** everything the viewer can read lives under the `viewer.*`
copy namespace plus an explicitly enumerated reuse list, so "what can a stranger see?" is one grep.
No owner vocabulary (`token`, `mock URL`, `your secret`) and no owner-voiced string ever renders
there (AC-111).

---

## 4. Acceptance criteria

165 ACs. Every one is testable and observable. Test surfaces in this repo: Rust unit tests in
`#[cfg(test)]` modules plus integration tests **[existing — verified at `backend/tests/api.rs`]**;
Playwright specs against a routed mock backend **[existing — verified at `e2e/mock-backend.ts:238`]**.

Count by feature — F1 15 · F2 6 · F3 21 · F4 61 · F5 20 · F6 11 · F7 15 · cross-cutting 16.

### 4.1 — Clear all logs (F1 — frontend-only; existing backend route)

- **AC-1** The Live Feed panel header renders a destructive "Clear all" control **in the header's
  action group**, which is an overflow `Menu` whose trigger is `Button variant="ghost"
  size="icon-sm"` + `MoreHorizontal` + `aria-label={t("feed.actions.menu.aria")}` — the shipped
  row-menu pattern **[existing — verified at `src/screens/rules-manager.tsx:274-298`]**. Menu order
  is `Export CSV` → `MenuSeparator` → `Clear all` (destructive last). The **trigger stays enabled**
  even when both items are disabled, so keyboard users can open it and hear them announced, and a
  `text-caption text-text-tertiary` hint line carries exactly one reason
  (`feed.actions.emptyHint` / `.busyHint` / `.offlineHint` — never two stacked). Rationale for
  deviating from the draft's literal "to the left of pause/resume": three labelled buttons plus the
  title, count and "N new" pill overflow the feed pane's 360 px `min-w-feed` minimum **[existing —
  verified at `tailwind.config.ts:113-115`, `src/components/hookbox/split-pane.tsx:53`]**.
  Pause/Resume stays a first-class inline control.
- **AC-2** Activating "Clear all" opens a confirm dialog built from the existing `Dialog` primitives
  following the rule-delete pattern **[existing — verified at
  `src/screens/rules-manager.tsx:317-336`]**: `DialogHeader` title, `DialogBody` explanatory body, a
  ghost Cancel and a `variant="danger"` confirm. No typed-token entry (matching the existing "Clear
  request history" confirm **[existing — verified at `src/screens/settings.tsx:445-455`]**).
- **AC-3** Cancelling (button, `Esc`, or overlay click) performs **no** network request and leaves the
  feed rows unchanged.
- **AC-4** Confirming issues exactly one `DELETE /api/endpoints/{token}/requests` **[existing —
  verified at `backend/src/routes/api.rs:878-889`, routed at `:999-1002`, client at
  `src/api/client.ts:221-226`]**. On 2xx: the dialog closes, a success toast is shown (reusing
  `set.toast.historyCleared`), and the Live Feed renders its empty state (`feed.empty.*`) — the
  feed's local row state is emptied client-side rather than waiting for a poll/WS message. The
  confirm button is `loading` and disabled while in flight, so a double-click cannot issue two
  DELETEs. Subject to AC-80's arrival-race qualification.
- **AC-5** Clearing also empties the paused-arrival buffer: after success the buffered "N new" pill is
  gone and `newCount === 0` — including when the feed was paused **[existing — verified at
  `src/feed/use-feed.ts:77` (`buffer`), `:74` (`newCount`), `src/screens/dashboard.tsx:326-335`]**.
- **AC-6** On a non-2xx / network failure the dialog **stays open**, a danger toast is shown, the
  server's `detail` is rendered inside the dialog body, and **no** rows are removed.
- **AC-76** **The enable predicate accounts for the paused buffer.** Both menu items are enabled iff
  `rows.length > 0 || newCount > 0`, and disabled while `exporting === true` or
  `navigator.onLine === false`. Test: with the feed paused, `rows` empty and `newCount === 3`, Clear
  all is **enabled**. Recorded limitation: server-side rows the client never loaded cannot enable the
  control; Settings → "Clear request history" remains the un-gated path for that case.
- **AC-77** **The confirm body names the endpoint, states the blast radius is wider than the visible
  list, and carries no count.** Frozen copy is `copy.md`'s `feed.clearAll.confirm.body`
  ("Deletes every request captured for "{endpoint}" — not only the ones listed here…"), with
  `{endpoint}` = `endpoint.name || endpoint.token` and never the mock URL. **The shipped string
  `set.confirm.clearHistory.body` loses its `{n}` slot** and its call-site interpolation **[existing —
  verified at `src/screens/settings.tsx:449`]**, because `{n}` is fed from `endpoint.request_count`, a
  monotonic lifetime counter **[existing — verified at `backend/migrations/0001_init.sql:31`]**, while
  at most `TRACE_CAP = 100` traces are stored — the sentence is wrong for any endpoint past 100 hits.
  Any e2e/visual assertion on the old string is updated.
- **AC-78** **Clear all resets the Inspector selection and the live-id set.** After a successful
  clear, no row is selected and the previously-selected id is removed from `liveIds` **[existing —
  verified at `src/screens/dashboard.tsx:203`, `:219-227`]**, so the Inspector cannot sit in
  "pending… Retry" forever **[existing — verified at `src/screens/dashboard/inspector.tsx:63-66`]**.
- **AC-79** **`request_count` is refreshed.** After a successful clear the screen re-fetches
  `GET /api/endpoints/{token}` (or decrements locally) so Settings' confirm copy and any counter do
  not read a stale lifetime value.
- **AC-80** **The arrival race has a defined end state.** A `new_request` may land between the confirm
  click and the 200. The asserted post-clear state is "empty **except** arrivals whose trace id is
  greater than any row present at confirm time"; the optional `feed.clearAll.confirm.note`
  ("Requests that arrive after this show up as normal.") sets that expectation. The e2e assertion for
  AC-4 tolerates such an arrival rather than requiring a strictly empty list.
- **AC-81** **Endpoint-gone mid-session is handled on every new control.** If the endpoint was deleted
  in another tab, Clear all, Export CSV, Share (mint/list/revoke) and Import each surface
  `common.error.endpointGone` on a 404/410 rather than a generic failure or a blank screen — the shell
  only handles 404/410 on initial load today **[existing — verified at
  `src/screens/dashboard.tsx:73-84`]**.
- **AC-82** **Clear all and Export CSV are mutually exclusive.** Both menu items are disabled while an
  export is in flight (`feed.actions.busyHint`), and the export's Cancel is the only way out. Test:
  starting an export disables Clear all; clearing is impossible until the export settles. Without
  this, clearing mid-export turns every outstanding row into a `pending`/`unavailable` sentinel and
  produces a file that looks corrupt.
- **AC-83** **The extracted `ConfirmDialog` renders confirm failures.** `src/components/hookbox/
  confirm-dialog.tsx` **[new]** is extracted from the private helper in Settings **[existing —
  verified at `src/screens/settings.tsx:659-705`]**, whose `try { await onConfirm(); onClose() }
  finally {}` has **no `catch`** **[existing — verified at `:688-697`]** — a rejection escapes as an
  unhandled promise rejection and the user is told nothing. The extraction catches, keeps the dialog
  open, renders an `InlineAlert variant="danger" role="alert"` in the body, and disables Cancel while
  busy. **The two existing Settings confirms are migrated to it** and are asserted to show an error
  on a failing confirm.
- **AC-84** **Destructive weight is graded, and the danger ink survives interaction.** The
  destructive menu item carries `text-danger-fg focus:bg-danger-bg focus:text-danger-fg
  data-[disabled]:text-text-tertiary`. The two `focus:` classes are a **fix**, not decoration:
  `MenuItem`'s base string ends with `focus:bg-surface-hover focus:text-text-primary` **[existing —
  verified at `src/components/ui/menu.tsx:38`]**, and `.focus\:text-text-primary:focus` (specificity
  0,2,0) beats a plain `.text-danger-fg` (0,1,0) — so today the shipped rules Delete item **loses its
  red at the exact moment focus lands on it**. **The same fix is applied to that existing item**
  **[existing — verified at `src/screens/rules-manager.tsx:291-296`]**. Assertable: the computed
  colour of the focused "Clear all" item equals `--danger-fg`; disabled renders `--text-tertiary`.
  Filled `variant="danger"` appears **only** inside a confirm dialog or the Settings danger zone
  **[existing — verified at `src/screens/settings.tsx:433-440`]**; in-page destructive triggers are
  ghost + `text-danger-fg` (with `hover:text-danger-fg` restated for the same specificity reason
  against `ghost`'s `hover:text-text-primary` **[existing — verified at
  `src/components/ui/button.tsx:20`]**).

### 4.2 — Remove the "Local path" URL chip (F2 — header only)

- **AC-7** The dashboard sub-header renders exactly **one** `UrlChip` — the Mock URL chip. The
  `t("dash.pathUrl.label")` chip is removed **[existing — verified at
  `src/components/hookbox/app-shell.tsx:101`]**; the `t("dash.mockUrl.label")` chip at `:100` is
  unchanged, still copy-only with no anchor. The `UrlChip` helper is kept even with one caller
  **[existing — verified at `:153-169`]**. This deletion is what buys the width F4's Share control
  needs at tablet widths, so **F2 should land before F4** in the same PR sequence.
- **AC-8** Settings → Identity still renders the "Local path" `CodeBlock` with
  `absolutize(endpoint.path_url)` **[existing — verified at `src/screens/settings.tsx:255-261`]**,
  byte-identical to today.
- **AC-9** `path_url` remains in the API contract and is still returned by
  `GET /api/endpoints/{token}` **[existing — verified at `backend/src/routes/api.rs:83`,
  `src/api/schemas.ts:63`]**; `endpointDetailSchema` is **not** changed. A contract test asserts
  `path_url` is still a required string on the detail response.
- **AC-10** The copy keys `dash.pathUrl.label` / `dash.pathUrl.copy.aria` remain in the copy table
  **[existing — verified at `src/lib/copy.ts:65-66`]**; they simply become unreferenced. The
  `AppShell` module doc-comment, which says the sub-header carries "mock URL + local path"
  **[existing — verified at `src/components/hookbox/app-shell.tsx:4-5`]**, is corrected.
  `pnpm typecheck` passes with no unused import/symbol errors. **The copy-parity check asserts one
  direction only** — every key in `copy.md` exists in `copy.ts`, not the converse — because this batch
  intentionally leaves up to eleven keys unwired (the optional `copy.md` items: `feed.clearAll.
  confirm.note`, `feed.export.detailNote`, `share.row.lastUsed.tooltip`, `dash.mockUrl.tooltip`,
  `rules.default.existsDisabled`, `rules.default.error.duplicate`, the `landing.feature.*` trio, and
  `dash.pathUrl.*`), resolving `copy.md` §7.19.
- **AC-85** **F2's test fallout is fixed, not discovered.** The sub-header's loading skeleton renders
  one placeholder chip per `UrlChip` **[existing — verified at
  `src/components/hookbox/app-shell.tsx:161-166`]**, so the skeleton drops from two chips to one.
  `e2e/visual.spec.ts` and `e2e/states.spec.ts` chip-count assertions and visual snapshots are updated
  in the same change; the suite passes with no `--update-snapshots` left uncommitted.
- **AC-86** **The removed chip keeps a discovery path.** The Mock URL chip gains
  `dash.mockUrl.tooltip` = "The endpoint's public mock URL. The local /e/ path is on the Settings
  screen." `UrlChip` renders no tooltip today, so this is a small component addition. This is the
  accepted answer to `journey.md` gap 45 / `copy.md` §7.16 — the alternative (accepting the
  discoverability loss silently) was rejected because R9 approved *removing* the chip, not losing the
  information.

### 4.3 — Export / import full endpoint config as JSON (F3)

- **AC-11** Settings renders a new "Configuration" `Section` **[existing pattern — verified at
  `src/screens/settings.tsx:707-720`]** containing "Export config" and "Import config…", placed
  **after Save and before Retention & state** (export/import take effect immediately and are not part
  of the unsaved form). Both are `Button variant="secondary" size="sm"` in a `flex flex-wrap gap-2`
  row copying Retention's shape **[existing — verified at `src/screens/settings.tsx:396-419`]** — so
  Save keeps the screen's single `variant="primary"`.
- **AC-12** "Export config" triggers a download of one `application/json` file named
  `hookbox-config-<token>.json`, via a shared `src/lib/download.ts` **[new]** helper that creates the
  object URL, clicks, and **revokes** it in a `finally`. The token is used (never the display name)
  because the token alphabet is filename-safe by construction **[existing — verified at
  `backend/src/ids.rs:17-24`]**.
- **AC-13** The exported bundle validates against the frozen `configBundleSchema` (§5.5.6) and
  contains `hookbox_config_version: 1`, `exported_at` (RFC3339 UTC), an `endpoint` object with
  exactly the nine `EndpointConfigPatch` fields **all required**, and a `rules` array. Non-portable
  fields are **absent**: `token`, `mock_url`, `path_url`, `created_at`, `last_hit`, `request_count`,
  `tunnel_active`. The bundle's `endpoint` object is a **separate `.strict()` schema**, not a
  composition of `endpointConfigPatchSchema` (which makes all nine `.optional()` and is not strict
  **[existing — verified at `src/api/schemas.ts:48-58`]**, so composing it would silently accept a
  bundle missing every field). A compile-time `_assignable` guard keeps the bundle type and
  `EndpointConfigPatch` in lockstep.
- **AC-14** Each exported rule contains exactly the `MockRuleCreate` fields **[existing — verified at
  `src/api/schemas.ts:115-126`]** and **omits** `id`, `token`, `created_at`. Rules are exported in the
  server's list order (`ORDER BY priority, id` **[existing — verified at
  `backend/src/routes/api.rs:542`]**).
- **AC-15** Export→import round-trip fidelity: exporting endpoint A, then importing that file into a
  *fresh* endpoint B, leaves B with A's nine config values and a rules list whose
  `MockRuleCreate`-shaped projection is deep-equal to A's, in the same order.
- **AC-16** "Import config" accepts a single `.json` file via a file input. The whole file is parsed
  and validated with `configBundleSchema` **before any network write**. On any validation failure —
  malformed JSON, wrong `hookbox_config_version`, unknown top-level keys, an invalid rule, a file
  larger than 5 MB, or more than 200 rules — the import is rejected with a specific human message
  naming the first failing field/index, and **zero** requests are sent (assertable: the route
  interceptor records no `PATCH`/`POST`).
- **AC-17** On a valid bundle **and after AC-S21's confirm**, import applies the config first via one
  `PATCH /api/endpoints/{token}` **[existing — verified at `backend/src/routes/api.rs:380-481`]**,
  then creates each rule in array order via `POST /api/endpoints/{token}/rules` **[existing — verified
  at `:584-628`]**, one request per rule.
- **AC-18** **Imported rules are added alongside existing rules; nothing is replaced or deleted.**
  Importing a 3-rule bundle into an endpoint with 2 rules yields 5. (There is no bulk-replace
  primitive, and a client-side delete-all-then-create would risk destroying rules on a mid-sequence
  failure.) Both numbers are stated **pre-write** in AC-S21's confirm.
- **AC-19** **Partial failure is stop-at-first-failure with no rollback.** If the config `PATCH`
  fails, **zero** rules are created and the error names the config step. If rule *k* of *n* fails,
  rules `1..k-1` remain, `k+1..n` are **not attempted**, nothing is rolled back, and the operator sees
  a **persistent** `InlineAlert variant="danger"` (never a toast) stating five facts in this order:
  the config was applied · `k-1` of `n` rules created · the 1-based index and `name` of the failing
  rule · the server's `detail` verbatim · that no rule after it was attempted and nothing was rolled
  back. Frozen copy: `set.config.import.failedRule` / `.failedConfig`.
- **AC-20** While an import is in flight the section shows determinate progress
  ("Applying settings…" → "Creating rule {i} of {n}…", AC-134) and both controls are disabled; on
  completion the screen re-fetches `GET /api/endpoints/{token}` **and** the form reflects the new
  values (AC-89).
- **AC-21** A rule whose `response.body_template` exceeds the server cap is rejected with 422
  **[existing — verified at `backend/src/routes/api.rs:571-573`]** and handled by AC-19 — it does not
  crash the importer or silently skip.
- **AC-22** Neither export nor import ever writes the `owner_secret`, `owner_id`, endpoint token, or
  any share-link material into the bundle. An automated assertion greps the produced file for the
  session secret and the strings `owner`, `code`, `share` and finds no such key (AC-S16, AC-S22).
- **AC-87** **Export builds from freshly fetched server state, never the in-memory form.** It fetches
  `GET /api/endpoints/{token}` and `GET /api/endpoints/{token}/rules` (a call this screen does not
  make today) and serialises those. When the form is dirty, `set.config.export.dirty` renders as
  `text-caption text-text-tertiary` under the button row. Test: edit a field without saving, export,
  and assert the file carries the **saved** value.
- **AC-88** **Export has real states.** Idle · busy (`set.config.export.busy`, button `loading`) ·
  rules-fetch failure (`set.config.export.error.rules`, an `InlineAlert variant="danger"` **inside the
  section**, not a toast, so the operator can read and retry) · generic failure
  (`set.config.export.error`) · 401 (the client clears the session and bounces **[existing — verified
  at `src/api/client.ts:99-105`]**) · zero-rules (`rules: []`, still a valid download) · success
  (`set.config.toast.exported`).
- **AC-89** **After a successful import the on-screen form shows the imported values, and the next
  Save cannot revert them.** (`journey.md` BLOCKER 23.) `SettingsForm` is mounted without a `key`
  **[existing — verified at `src/screens/settings.tsx:138-143`]** and seeds all nine fields once with
  `useState(endpoint…)` **[existing — verified at `:161-170`]**, so a re-fetch alone leaves stale
  fields on screen and the next `PATCH` sends the pre-import values. Mechanism is the implementer's
  choice (remount via a `key` that changes after import, or reset state from an effect keyed on the
  fetched endpoint identity/`updated_at`). **Two assertions, both required:** (a) immediately after
  import the rendered value of every changed field equals the imported value; (b) a Save clicked
  immediately after import issues a `PATCH` whose body carries the **imported** values.
- **AC-90** **File-input mechanics are specified, not improvised.** Re-selecting the *same* file after
  a rejection must still fire (the input's `value` is reset after every attempt). A cancelled file
  dialog is a silent no-op. A 0-byte file (`set.config.import.invalid.empty`), a non-JSON file with a
  `.json` name (`.invalid.json`, worded for a hand-editing human, not "Unexpected token"), an oversize
  file (`.tooLarge`), and a >200-rule file (`.tooManyRules`) each produce their own message. **A
  leading UTF-8 BOM (U+FEFF) is stripped before parsing**, not surfaced as a parser error — files
  round-tripped through editors commonly carry one.
- **AC-91** **A long import cannot be lost or accidentally doubled.** Only one import may be in flight
  (the control is disabled, so a double-click cannot start a second run or duplicate rules);
  `set.config.import.dontClose` ("Keep this tab open until it finishes.") is shown while applying; the
  completion report — success **or** AC-19's partial failure — is **persistent until dismissed**
  (`common.dismiss`), not a 3.2 s toast, and carries a `View rules` action
  (`Button variant="secondary" size="sm" asChild` → `<Link to={`/d/${token}/rules`}>`) so the operator
  can see the deterministic prefix on the screen where rules live. Test: a partial-failure report
  survives a scroll and a re-render.
- **AC-92** **The import control is keyboard-focusable with a *visible* ring.** It is a visually-hidden
  native `<input type="file" accept="application/json,.json" className="peer sr-only">` (focusable and
  announced, unlike `hidden`) plus a `<label htmlFor>` carrying `buttonVariants({variant:'secondary',
  size:'sm'})` **[existing — verified at `src/components/ui/button.tsx:68`]**. Because an `sr-only`
  input's global `:focus-visible` outline **[existing — verified at `src/globals.css:247-251`]** lands
  on an unpainted box, the ring is projected onto the label with
  `peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2
  peer-focus-visible:outline-focus`, and the label adds `cursor-pointer` plus `peer-disabled:*`
  styling that `buttonVariants` does not supply for a non-`<button>`. The input must precede the label
  in the DOM. Assertable: `Tab` to the import control produces a visible 2 px outline.

### 4.4 — Public read-only share links (F4)  ⚠️ HIGHEST RISK IN THIS BATCH

> **F4 crosses a new trust boundary.** `security.md` proved the draft projection was not actually
> read-only (§0 item 1); AC-S1, AC-S2 and AC-S4 (§4.9) are **blocking prerequisites** for shipping any
> part of F4. `architecture.md` D9/D10 changed the storage and revoke design (§0 item 2), which is why
> AC-24/AC-25/AC-26 read differently from the draft.

**Owner-side (authenticated):**

- **AC-23** The dashboard sub-header renders a "Share" control **first in the right-hand action
  cluster**, before "Rules" **[existing — verified at `src/components/hookbox/app-shell.tsx:119-143`]**
  — preserving `AppShell`'s documented left = subject / right = actions split **[existing — verified at
  `src/components/hookbox/app-shell.tsx:4-7`]**. It is `Button variant="ghost" size="sm"` + lucide
  `Share2` + `<span className="sr-only sm:not-sr-only">`, and it is **never disabled**, including on a
  zero-traffic endpoint ("here is where you can watch it arrive" is a legitimate first move). It opens
  a Share dialog that lists the endpoint's active share links and offers "Create share link".
- **AC-24** "Create share link" issues `POST /api/endpoints/{token}/shares` **[new]** and, on 201,
  reveals the full share URL **once**, in a dedicated one-time panel: a `CodeBlock` (`mono-lg` +
  integrated `CopyButton`), the shown-once note (`share.created.onceHint` — "Shown once — copy it now.
  HookBox keeps only a fingerprint of the link, so it can't show it again."), `share.created.lostHint`,
  and an "Open in new tab" action. The panel is `role="status" aria-live="polite"` and **focus is not
  moved** (the operator may still be typing a label). The newly created link also prepends to the
  active list — as a row **without** a URL (AC-25). **The 201 body is the only place `code` and `url`
  ever appear** (AC-104).
- **AC-25** The dialog lists active links via `GET /api/endpoints/{token}/shares` **[new]**, newest
  first, each row showing **the label (or `share.row.untitled`), created-at, last-used, and Revoke —
  and nothing else. No URL, no copy action, no Preview.** Those are impossible: the raw code is never
  stored (§5.4), so it cannot be re-derived server-side. `share.list.hint` states this in the dialog
  ("A link's URL is never shown again after it's created — the label and date are how you tell them
  apart."). Revoked links do not appear. Assertable: the dialog's DOM contains no `/s/` substring
  outside the one-time panel.
- **AC-26** Revoke issues `DELETE /api/endpoints/{token}/shares/{id}` **[new]** — **addressed by the
  non-secret integer `id`, never by the code** — returns 204, and removes the row without a full page
  reload. Rationale (`architecture.md` D10): the owner routes sit under nginx's `location /api/`
  **[existing — verified at `deploy/nginx.conf:18-25`]**, which logs the full request line, and no
  nginx prefix can exclude `/api/endpoints/*/shares/*` without unlogging every endpoint route — so a
  code in that path would be written to the access log in cleartext, the exact leak class commit
  `47a267c` fixed for `?cap=`. A test asserts no share code appears in any owner-route URL.
- **AC-27** An owner may hold at most `SHARE_MAX_PER_ENDPOINT` (default 10) active links per endpoint;
  the 11th `POST` returns 422 `validation_error` and the dialog surfaces the message rather than
  failing silently. The Create control is **also** pre-emptively disabled at the cap (AC-96).
- **AC-28** All three owner routes enforce ownership through the existing helper and return
  **404 (never 403)** for a valid secret that does not own the endpoint **[existing — verified at
  `backend/src/auth.rs:55-69`]**. Specifically: `POST /api/endpoints/{other_owners_token}/shares` →
  404; `GET` → 404; `DELETE …/shares/{id}` for an id belonging to another owner's endpoint → 404.
- **AC-29** All three owner routes return 401 with `WWW-Authenticate: Bearer` when the `Authorization`
  header is missing, malformed, or presents an unknown secret **[existing — verified at
  `backend/src/auth.rs:23-49`]**.
- **AC-30** Deleting (tombstoning) an endpoint revokes all of its share links in the same handler that
  clears rules/state/CRUD **[existing — verified at `backend/src/routes/api.rs:506-532`]**. After
  `DELETE /api/endpoints/{token}`, every previously working share URL resolves to 404. Ordering and
  atomicity are governed by AC-S9 (revoke **before** writing `gone_at`, or one transaction — and the
  resolver checks `gone_at` itself, because `ON DELETE CASCADE` never fires for a tombstone).

**Code entropy & storage:**

- **AC-31** A share code is generated from a CSPRNG with **at least 128 bits of entropy**; the shipped
  default is `SHARE_CODE_BYTES=24` → 192 bits → a 32-character base64url (no-pad) string. Unit tests
  assert: length ≥ 32 for the default, charset ⊆ `[A-Za-z0-9_-]`, and 10 000 generated codes are all
  distinct. The generator reuses the existing CSPRNG path **[existing — verified at
  `backend/src/ids.rs:41-45`]**.
- **AC-32** A share code is **not derived from and does not contain** the endpoint token, the
  `owner_id`, the `owner_secret`, the endpoint name, or a timestamp. A test asserts a generated code
  shares no substring of length ≥ 4 with a `gen_token(10)` value.

**Public resolver (unauthenticated) — the new trust boundary:**

- **AC-33** `GET /api/share/{code}/requests` **[new]** requires **no** credential and returns 200 with
  a `PublicShareFeed` (§5.5.4) for an active code.
- **AC-34** The public responses **never** contain: `token`, `owner_id`, `owner_secret`, `mock_url`,
  `path_url`, `target_url`, `matched_rule_id`, `trace`, `state_snapshot`, any rule object, any endpoint
  config field (`default_mode`, `latency_ms`, `rate_limit_per_min`, `chaos_pct`, `chaos_mode`,
  `cors_enabled`, `auto_crud`), or `tunnel_active`. **This AC is only true because AC-S1's filter
  removes `x-hookbox-endpoint` and `x-hookbox-rule-id` from `response_headers`** — the omission list
  governs *projection keys*, AC-S1's filter governs *keys inside `response_headers`* (§5.11). An
  automated test asserts the serialized public response contains none of those JSON keys; the
  token-absence half is asserted by AC-S2 (scoped to server-generated fields, **after** the filter).
- **AC-35** `GET /api/share/{code}/requests/{id}` **[new]** returns a `PublicRequestDetail` (§5.5.5)
  **only if** the request row's `token` equals the share link's endpoint token; otherwise **404**. This
  is enforced inside one statement (`WHERE id = ? AND token = ?`, §5.2), so cross-endpoint trace
  enumeration is structurally impossible. A test creates two endpoints, shares A, and asserts a request
  id belonging to B returns 404 through A's code.
- **AC-36** **Unknown code, revoked code, and tombstoned endpoint all return the same 404 `not_found`
  body** — a scanner cannot distinguish "never existed" from "revoked" from "endpoint deleted". A test
  asserts the three responses are byte-identical across status line, body **and headers** (AC-S14),
  including for `HEAD`. Guaranteed structurally by funnelling every negative outcome through one
  `share_not_found()` constructor (§5.2).
- **AC-37** **Revocation is immediate and uncacheable.** After a successful revoke, the *very next*
  public request with that code returns 404 with no intervening delay. Every **handler-produced**
  public response (200/404/422/429/503) carries `Cache-Control: no-store`. *Narrowed per
  `architecture.md` D14:* axum's auto-405 (required by AC-39) is framework-generated with an empty body
  and no custom headers and therefore carries neither `no-store` nor the flat error envelope; this is
  accepted, because a 405 discloses nothing and this matches every other route in the app. The resolver
  reads `share_links` per request and is never served from `rule_cache` or any other in-process cache
  **[existing — verified at `backend/src/rule_cache.rs:1-8`, keyed by endpoint token for the mock plane
  only]**.
- **AC-38** The public resolver is rate limited per client IP using the existing token-bucket limiter
  **[existing — verified at `backend/src/limiter.rs:80-128`]** at `SHARE_RATE_LIMIT_PER_MIN`
  (**default 120** — see AC-113), under a **namespaced key `share:<ip>`** that cannot collide with or
  evict the mock plane's `rl:<token>` buckets **[existing — verified at `backend/src/limiter.rs:71-76`]**
  (AC-S7). The check runs **before any DB read** and `HEAD` is counted (AC-S14). Over the limit returns
  429 `rate_limited` with `Retry-After` **[existing — verified at `backend/src/error.rs:66-69`]**. The
  client IP is resolved with the existing proxy-aware helper so every viewer behind nginx gets their own
  bucket **[existing — verified at `backend/src/routes/api.rs:273-299`]**.
- **AC-39** The public resolver exposes **no mutation surface**: `POST`, `PATCH`, `PUT` and `DELETE` to
  `/api/share/{code}/requests[/{id}]` return 405 and are asserted to leave `request_logs`,
  `mock_rules`, `endpoints`, `endpoint_state` and `crud_collections` row counts unchanged. (The
  coalesced `last_used_at` write touches `share_links` only, which is not in that list — AC-97/AC-S10.)
- **AC-40** Pagination is bounded: `limit` defaults to 50 and must be 1..200, `offset` must be ≥ 0;
  out-of-range values return 422 — matching the existing owner list route **[existing — verified at
  `backend/src/routes/api.rs:829-836`]**. Ordering relative to code resolution is frozen by AC-101.

**Public viewer page (frontend):**

- **AC-41** A new SPA route `/s/:code` **[new]** renders a read-only viewer. It renders for a visitor
  with **no** session, **never** redirects to `/` for a missing secret (unlike `/d/:token` **[existing —
  verified at `src/screens/dashboard.tsx:136-140`]**), and never creates a session. It is registered
  before the `*` fallback **[existing — verified at `src/router.tsx:35-49`]**.
- **AC-42** The viewer page issues **zero** requests carrying an `Authorization` header. A Playwright
  network assertion verifies every request from `/s/:code` has no `Authorization` header; the share
  fetches go through the client's existing `noAuth` path **[existing — verified at
  `src/api/client.ts:78-81`, `:99-105`]**, so a share 401/404 can never clear a real session or bounce a
  logged-in owner out of their own tab. AC-S13 additionally forbids the viewer's module graph from
  importing `src/api/session.ts` at all.
- **AC-43** The viewer renders **no owner affordances**. Assertable as a list of accessible names that
  must be **absent** from `/s/:code`: `Switch endpoint`, `Account`, `Sign out`, `Rules`, `New rule`,
  `Settings`, `Pause the live feed`, `Resume the live feed`, `Feed actions`, `Clear all`, `Export CSV`,
  `Share`, `Copy mock URL`, `Copy local path`, `Resize feed and inspector`. The DOM must additionally
  not contain the endpoint token, `mock_url`, `path_url`, `target_url`, or any rule text. The visual
  half of "must not read as the dashboard with pieces missing" is AC-109.
- **AC-44** The viewer has documented states, each visually distinct: loading · empty (valid link,
  nothing to show — AC-108) · list · detail loading · detail ready · **detail gone (AC-106)** ·
  unavailable (404 — one message covering unknown/revoked/deleted per AC-36, **terminal**) ·
  rate-limited (429, showing the `Retry-After` seconds counting down, stale rows retained at full
  opacity) · error (other, with Retry, last-known rows retained) · offline/hidden-tab (polling
  suspended). Copy is frozen by `copy.md` §4.6 (`viewer.*`). Governed by AC-105..AC-112.
- **AC-44a** **Bodies and headers ARE included in the public detail view — decided, not open.**
  `PublicRequestDetail` (§5.5.5) carries `request_headers`, `query_params`, `request_body`,
  `response_headers` **and** `response_body`. The only omissions are `token`, `matched_rule_id`,
  `overhead_ms`, `trace` and `state_snapshot` (AC-34). A test asserts all five of those fields are
  **present keys** on a 200 detail response (present-with-`null` when the underlying column is `NULL`),
  so a future reduction of the projection is a deliberate contract change rather than a silent
  regression. `response_headers` is a **filtered** map (AC-S1, §5.5.5) — filtered, not omitted, which is
  precisely what makes AC-34 true. `response_body` carries real captured content because F7 ships in
  this batch (AC-114).
- **AC-45** The viewer refreshes by **polling** `GET /api/share/{code}/requests` at a 5 s interval
  **only while the document is visible**, stopping on `visibilitychange → hidden` and on unmount. No
  WebSocket and no EventSource is opened from `/s/:code` (assertable: zero `ws://`/`wss://` and zero
  `text/event-stream` requests). The cadence is stated in the UI (`viewer.updating` /
  `viewer.updating.paused`) and a manual `Refresh` control exists. **Failure lifecycle is AC-105.**
- **AC-93** **The mint-time disclosure's wording matches the filter that actually ships.** Frozen copy
  is `copy.md`'s `share.warning.body` + `share.warning.redaction`, two paragraphs in one persistent
  `InlineAlert variant="warning" role="status"` titled `share.warning.title`, always rendered **above**
  the Create button. It must state: anyone with the URL, no sign-in · the last 100 requests · including
  requests that arrived **before** the link existed and requests **other people** sent · each request's
  method, path, status, headers, query and body · the response headers **and** body · that the
  endpoint's **name** is visible. Then, separately, what is hidden: `Authorization`, `Cookie` and
  `X-Owner-Id` request headers, plus `Set-Cookie`/`Authorization`-family response headers and every
  internal `x-hookbox-*` header — **and nothing else**. **`ux.md` §2.5's claim that response headers are
  "shown exactly as sent, including any Set-Cookie" is superseded and must not ship** (§0 item 3): it
  contradicts AC-S1, and claiming total transparency we do not provide is worse than the redaction
  itself. Assertable: the rendered disclosure contains no sentence asserting response headers are
  unmodified, and the named hidden-header list is exactly the one §5.11 freezes.
- **AC-94** **The label has an input, a limit and an inline error.** A `Field` + `Input` labelled
  `share.label.label`, placeholder `share.label.placeholder`, helper `share.label.helper` ("Optional,
  but worth it — the label and the date are all you'll have to tell links apart later. Only you see
  it.") — worded as a nudge rather than a shrug because under hashed storage the label is the link's
  only human identity (`architecture.md` §8 item 11). Over 80 characters: `Input` gets
  `border-danger-fg`, a `role="alert"` message renders `share.label.tooLong`, and Create is disabled —
  client-side, before the server's 422. A label that trims to empty is sent as `null` and the row
  renders `share.row.untitled`.
- **AC-95** **Revoke is a two-step, and its failure modes are unambiguous.** Because revocation is
  irreversible (a revoked code can never be re-minted, §5.1) and instantly breaks a URL already pasted
  into a ticket, a single click must not do it. The row swaps **in place** to an inline confirm
  (`share.row.revoke.confirm` + `share.row.revoke.confirmHint` — "It stops working immediately for
  everyone who has the URL, and it can't be brought back." — plus `[Cancel] [Revoke]`), not a nested
  `Dialog`: no second focus trap, no scrim-on-scrim. `Esc` while armed cancels the arm **without**
  closing the dialog. On 204: the row is removed and `share.toast.revoked` fires. **On 404 the
  operator's intent is already satisfied** — treat it as success, refresh the list, and show
  `share.toast.revokedAlready` (never an error). On 5xx: the row is **restored**, a row-level
  `text-body-sm text-danger-fg` message renders `share.error.revoke` ("Couldn't revoke the link. It's
  still active — try again.") next to the still-live link, and a danger toast fires. The "it's still
  active" clause is required: an ambiguous failure on a revocation is a security-relevant ambiguity.
- **AC-96** **The owner dialog has every state, not just the happy one.** loading (`SkeletonLines
  lines={3}`, `aria-busy`, `share.list.loading.aria`) · empty (`share.list.empty.title` / `.body`) ·
  list · creating (Create `loading`, label input disabled) · created (AC-24's panel) · label invalid
  (AC-94) · **at cap** (Create pre-emptively `disabled` with `share.limit.reached` as
  `text-caption text-text-tertiary` beneath it, **and** a server 422's `detail` still surfaced in an
  `InlineAlert variant="danger"` if it happens anyway — AC-27) · revoke armed / revoking / revoked /
  revoke failed (AC-95) · **list load failed** (`InlineAlert variant="danger"` + Retry, the shipped
  `rules.error.*` shape **[existing — verified at `src/screens/rules-manager.tsx:204-216`]**). The
  dialog's intro uses `DialogDescription` **[existing — verified at `src/components/ui/dialog.tsx:66`,
  currently unused]** so the paragraph becomes the dialog's accessible description.
- **AC-97** **`last_used_at` is displayed, and its coarseness is disclosed.** Each row renders
  `share.row.lastUsed` ("Opened {when}") or `share.row.neverUsed` ("Never opened") with `title={iso}`.
  Because the write is coalesced to at most once per link per 60 s (AC-S10, §5.2),
  `share.row.lastUsed.tooltip` explains that a very recent open may not show yet — without it, an
  operator who previews their own link and sees "Never opened" concludes the feature is broken. It is
  the operator's only "is anyone actually reading this?" signal, which is why the column is not dropped.
- **AC-98** **The Share control carries an active-link count badge, and degrades gracefully.** When
  `activeShareCount > 0` the button renders a `.tnum` pill
  (`rounded-pill bg-neutral-chip-bg px-1.5 text-caption font-medium text-neutral-chip-fg`, `min-w-5` so
  it does not reflow between 1 and 2 digits) and its `aria-label` becomes
  `share.action.count.aria`. **`rounded-xs bg-subtle` from `ux.md` §2.5 must not be used** — `bg-subtle`
  generates no CSS (§0 item 9, AC-130). The count costs one `GET /api/endpoints/{token}/shares` per
  owner-screen mount; **on any failure the control renders as plain "Share" with no badge and no error**
  — a badge is a safety affordance ("a public link exists"), never a blocker. Rationale for having it at
  all: for a feature that publishes captured traffic, a persistent glanceable signal is a safety
  affordance rather than decoration.
- **AC-99** **The minted URL is always on the app origin, and an unreachable one is called out at mint
  time.** `url` is built from `PUBLIC_BASE_URL` or origin-relative (§5.5.3) and **never** from `Host`,
  `X-Forwarded-Host`, or `mock_url`'s wildcard form. Two tests: (a) a request with a hostile
  `X-Forwarded-Host` still mints a URL on the configured base (AC-S14's host-header-injection case —
  otherwise the owner would hand the code to an attacker's domain); (b) the minted URL never has the
  form `https://{token}.{MOCK_DOMAIN}/s/{code}`, because `resolve_plane` sends **everything** on a mock
  host to the mock plane **[existing — verified at `backend/src/planes.rs:137-148`]**, where a share URL
  would be ingested as a webhook and the code persisted into `request_logs.path`. When
  `PUBLIC_BASE_URL` is unset the SPA absolutizes with `window.location.origin` **[existing — verified at
  `src/lib/url.ts:8-11`]**; if the resulting host is `localhost`, `127.0.0.1`, `::1`, a `*.local` name,
  or an RFC1918 address, the one-time panel additionally renders `share.created.localWarning`, which
  names `PUBLIC_BASE_URL`. Otherwise the operator pastes a link nobody else can open, with no warning.
- **AC-100** **Minting a share on a tombstoned endpoint returns 404.** `POST` checks
  `endpoints.gone_at` and returns the standard `not_found` before generating any code, because a link
  minted for a deleted endpoint would be dead on arrival (`architecture.md` §2.1 #19 step 3).
- **AC-101** **Parameter validation precedes code resolution, so `limit` is not an existence oracle.**
  `?limit=999` returns **422 for both a valid and an invalid code**. Were the order reversed, a live
  code would 422 and a dead one 404 — a boolean oracle that defeats AC-36. The existing owner route
  already orders it this way **[existing — verified at `backend/src/routes/api.rs:829-837`]**; the
  public route must too, and a test asserts both halves.
- **AC-102** **The public projections are standalone structs, so a future owner-shape field cannot leak
  by default.** `PublicRequestSummary`, `PublicRequestDetail`, `PublicShareFeed` and `PublicEndpointInfo`
  are separate `#[derive(Serialize)]` structs in `backend/src/models.rs`, built field-by-field from the
  row in `share.rs`. They must **not** be produced by `#[serde(skip)]`-ing fields off `RequestDetail`
  and must not `#[serde(flatten)]` an owner struct. A test asserts the serialized JSON key set is
  **exactly** the allow-list — so adding a column to the owner shape can never widen the public one.
- **AC-103** **`SHARE_CODE_BYTES` is clamped to ≥ 16 at load.** `int_env` will happily accept
  `SHARE_CODE_BYTES=1` today **[existing — verified at `backend/src/config.rs:17-22`]**, which would
  mint a guessable code. A unit test asserts `gen_share_code(1)` still yields ≥ 22 characters (a 128-bit
  floor holds).
- **AC-104** **The plaintext code is stored nowhere and appears in exactly one response.** Only
  `sha256(code)` is persisted (`share_links.code_hash`, §5.4), reusing the existing
  `ids::hash_secret` **[existing — verified at `backend/src/ids.rs:57-60`]** and mirroring
  `owners.secret_hash` **[existing — verified at `backend/migrations/0001_init.sql:11`]**. Tests: (a) the
  `share_links` row contains no substring of the minted code; (b) the code appears in the `201` body of
  `POST …/shares` and in **no other** response — not in `GET …/shares`, not in any error `detail`, not
  in any log line, not in the F3 bundle (AC-S16). Consequence: a leaked backup, replica or `.db` file
  copied off the box yields **no working URL**.
- **AC-105** **The viewer's polling lifecycle is fully specified.** (`journey.md` BLOCKER 2, AC-S8.)
  All seven rules are independently assertable:
  (a) **A list-level 404 is terminal** — polling stops permanently, the terminal unavailable page
  renders, and no further request is made (assert: exactly one request after the 404 across 30 s).
  (b) **429 pauses for at least `Retry-After`**, then auto-retries **once**; a fixed 5 s poll against a
  per-minute bucket never recovers, so this is required, not cosmetic.
  (c) **5xx / network / `contract_mismatch`** back off exponentially from 5 s with a ceiling (≤ 60 s),
  keeping the last-known rows visible.
  (d) **No two polls are ever in flight at once** (assert: no overlapping requests under an artificially
  slow response).
  (e) Polling **resumes** on `visibilitychange → visible` and on the `online` event.
  (f) Every in-flight fetch is **aborted on unmount**.
  (g) **A detail-level 404 is never terminal** and never stops list polling — see AC-106.
- **AC-106** **A detail 404 is "this one request is gone", never "this link is dead".** A row can 404
  merely because it aged out of the 100-row cap or 24 h TTL **[existing — verified at
  `backend/src/config.rs:156-157`]**, or because the owner ran Clear all — and AC-35/AC-36 make that
  404 byte-identical to a dead-code 404, so the **client** must carry the distinction. The open
  disclosure region renders `InlineAlert variant="info"` with `viewer.detail.gone.title` /
  `.gone.body` ("HookBox keeps the last 100 requests for 24 hours. This one has rolled off. The rest of
  the list still works."), the region **stays open** rather than collapsing and yanking content from
  under the cursor, and list polling continues. There is deliberately **no** "pending" state here
  (unlike the owner Inspector **[existing — verified at `src/screens/dashboard/inspector.tsx:63-66`]**):
  the list came from the DB, not a WS broadcast, so a 404 means gone, not not-yet-written. `info`, not
  `danger` — aged-out data is not an error.
- **AC-107** **The viewer orients a cold stranger.** A standing, **non-dismissible**
  `InlineAlert variant="info" role="status"` spans full width above the content with
  `viewer.banner.title` / `.banner.body`; a `Read-only` `neutral-chip` sits in the card header (the
  banner is in flow and scrolls away, the chip does not — `design.md` §3.7, copy key
  `viewer.readOnlyChip`); a `<footer>` carries `viewer.footer`. **The `<h1>` is always the static
  string `viewer.title` ("Shared requests") and `document.title` is always the static
  `viewer.docTitle`** — the endpoint name is operator-authored text on an unauthenticated page and a
  ready-made phishing slot ("Session expired — enter your password to continue"), so it may never
  occupy the page's most authoritative line. The name renders only as `viewer.subject.name`
  ("Endpoint: {name}"), truncated, `title`-attributed for the full value, with
  `viewer.subject.unnamed` substituted **into** the `{name}` slot so the prefix never disappears. One
  caption under the card states the retention window **and** the cadence in one line
  (`viewer.updating`). AC-S13 forbids any user value reaching `href`/`src`/`srcdoc`/`style`/`on*`.
- **AC-108** **The empty state covers both of its causes.** "Valid link, nothing to show" fires both
  when nothing has arrived yet **and** when the history rolled off (100-row cap / 24 h TTL / owner
  Clear all). Frozen copy `viewer.empty.body` says both ("Either nothing has arrived yet, or older
  requests have already rolled off"), because "No requests yet" tells a day-late recipient the link is
  broken. It must be visibly distinct from the terminal unavailable page. **`FeedEmpty` must not be
  reused** — it renders `absolutize(mockUrl)` in a `CodeBlock` plus a curl sample **[existing —
  verified at `src/screens/dashboard.tsx:384-413`]**, which would violate AC-34. The prohibition is
  explicit so nobody "reuses the component" in good faith.
- **AC-109** **The viewer is visually a different object, assertably.** All eight conditions:
  (a) **zero** accent-filled controls on the page — no `variant="primary"`, no element with
  `background-color: var(--accent-fill)`; the only accent pixels are the `BrandMark` glyph, the active
  tab's 2 px underline (a selection marker, not a call to action) and the focus ring;
  (b) the page has a `<footer>` (the dashboard has none);
  (c) the root is `flex min-h-screen flex-col bg-canvas` — a scrolling document, **not** the
  dashboard's `h-screen overflow-hidden` shell **[existing — verified at
  `src/components/hookbox/app-shell.tsx:73`]**;
  (d) `animate-feed-row-in` and `rail-flash` **never** apply **[existing — verified at
  `tailwind.config.ts:154-173`]** — that pair is the *live* feed's signature and a 5 s poller must not
  cosplay as a live stream; a poll arrival has **no** animation at all;
  (e) no `SplitPane`, drag handle, or `ConnectionPill`;
  (f) no `MockUrlChip`/`CodeBlock` carrying an endpoint URL, and no `FeedEmpty` (AC-108);
  (g) the content column, banner and footer all share `maxWidth.viewer: '920px'`, added to
  `tailwind.config.ts` **[existing — verified at `tailwind.config.ts:108-112`]** so the three cannot
  drift out of alignment;
  (h) chrome is copied from `/cli` **[existing — verified at `src/screens/cli.tsx:67-84`]** —
  `BrandMark` (not a link) + `ThemeToggle` only. `ThemeToggle` stays: it is a viewer preference, reads
  no data and mutates nothing, so it is not an owner affordance in AC-43's sense.
  This is the testable form of "must not read as the dashboard with pieces missing"; the accent ledger
  it belongs to is AC-134's sibling: Settings 1 (Save) · Rules Manager 1 (New rule) · Share dialog 1
  (Create, within its own surface) · feed pane 0 · viewer **0**.
- **AC-110** **The viewer works on a phone.** Share links arrive by chat, ticket and email, so mobile
  is the common case, and the dashboard's `SplitPane` (`min-w-feed: 360px`, drag-to-resize) has no
  mobile treatment. Required: single column at every width; **no horizontal scroll at 360 px**; row
  triggers **≥ 44 px** tall (`min-h-11`); the column-header band `hidden sm:grid` (its labels are
  redundant with the row content — the only thing hidden responsively, and it is neither data, control,
  nor state); the row grid drops to four columns below `sm` with the trailing meta wrapped by a
  `sm:contents` span. Zoom to 200 % keeps it single-column.
- **AC-111** **No owner-voiced string ever renders on the viewer.** `ServedByChip` renders no tooltip
  today **[existing — verified at `src/components/hookbox/served-by-chip.tsx:40-55`]** and must not
  start: `servedBy.mitm.tooltip` says "Proxied to **your** upstream target" and
  `servedBy.tunnel.tooltip` says "…down **your** tunnel to localhost" **[existing — verified at
  `src/lib/copy.ts:157-159`]**. `insp.headers.redacted.tooltip` is likewise owner-voiced ("your
  secret") **and** incomplete post-AC-S1, so the viewer uses `viewer.headers.redacted.tooltip`
  instead. `feed.empty.*` is never rendered here (AC-108). Reuse of the remaining `insp.*` values is
  **approved** (`copy.md` §6.1's ruling on `ux.md` gap 27): tab labels, per-tab empty states and
  response labels are neutral, factual and second-person-free, so they read the same to a stranger.
  Assertable by grepping the viewer's rendered DOM for "your". Acknowledged and accepted:
  `served_by` **is** in the frozen projection, so a viewer does learn whether traffic was mocked,
  proxied or tunnelled — that is decided contract (§5.5.5), and only the owner-voiced *tooltips* are
  withheld.
- **AC-112** **Malformed and missing codes render a state, never a crash; there is no row deep-linking.**
  `/s/` with no code renders the existing NotFound screen (the `/s/:code` route does not match it) and
  does not crash or blank. A code failing `^[A-Za-z0-9_-]{32,64}$` short-circuits to 404 server-side
  with **no DB read** (§5.2) and renders the same terminal unavailable page as AC-36. A hand-edited
  out-of-range `limit`/`offset` renders the error state, not a crash. **Selecting a row does not change
  the URL** — rows are in-place disclosures, so a poll can prepend rows without disturbing which row is
  open and there is no selected-row-scrolled-away state to reconcile. Per-row deep links are a
  deliberate non-goal for v1 (§2); adding them later is additive.
- **AC-113** **The public resolver's limits are frozen numbers with recorded arithmetic.**
  `SHARE_RATE_LIMIT_PER_MIN = 120` per IP and `SHARE_RATE_LIMIT_GLOBAL_PER_MIN = 1200` instance-wide
  (§5.8). Arithmetic, recorded so the numbers can be argued with rather than guessed at: one viewer
  polling at 5 s burns **12 req/min** on the list route alone before any detail clicks, so 60/min (the
  draft's default) is exceeded by **two** viewers behind one corporate NAT egress IP doing nothing
  unusual — the exact common case for "show the vendor". 120/min ≈ 10 concurrent polling viewers per
  IP. The global ceiling bounds worst-case egress: a detail response can carry 2 × `MAX_BODY_BYTES`
  ≈ 512 KB, so 1200 req/min caps the resolver at ≈ 600 MB/min (AC-S15). **Stated honestly:** the
  limiter fails open on anomaly and evicts the most-idle bucket past `MAX_BUCKETS = 100_000`
  **[existing — verified at `backend/src/limiter.rs:43`, `:88-89`, `:130-142`]**, and the shipped
  compose topology has **no nginx** **[existing — verified at `docker-compose.yml`: one app service
  with a `ports:` mapping, no proxy]** so viewers there may collapse into a single bucket. This is a
  **courtesy limit, not a guarantee** (§8-R14). Tests: 121 requests from one IP in a minute yields a
  429 with `Retry-After`; 100 000 distinct IPs do not evict any `rl:<token>` bucket and endpoint rate
  limits remain enforced (AC-S7).
- **AC-114** **F4 and F7 ship in the same release, so no share link is ever minted under a narrower
  projection than the one it publishes.** (`journey.md` gap 16 / E27 — a PM decision, not an
  architecture one.) F7 turns `response_body` from permanently-`null` into real content; a link minted
  before F7 would have been consented to under a materially narrower disclosure, and there is no
  mechanism to re-consent. The three alternatives — accept the silent widening, mass-revoke on
  upgrade, or notify every operator — are all worse than simply not creating the window. **Assertion
  owned by the QA gate:** the release that first exposes `POST /api/endpoints/{token}/shares` also
  contains F7's capture, verified by an end-to-end test that mints a link and reads a non-null
  `response_body` through `GET /api/share/{code}/requests/{id}`. Consequence for BREAKDOWN: the QA gate
  blocks on BE-1 **and** BE-2 together; neither may be released alone.

### 4.5 — Export request log as CSV (F5 — full fidelity)

- **AC-46** The Live Feed panel header renders an "Export CSV" control **in the same overflow `Menu`
  as Clear all** (AC-1), first, above the separator. Disabled when the enable predicate of AC-76 is
  false. *Placement deviates from the draft's literal "in the same action group inline" for the reason
  recorded in AC-1.*
- **AC-47** Activating it fetches `GET /api/requests/{id}` **[existing — verified at
  `backend/src/routes/api.rs:850-874`, client at `src/api/client.ts:217-219`]** for every row in the
  snapshot (AC-115), up to the feed cap of 100 **[existing — verified at `src/feed/use-feed.ts:42`]**,
  with **exactly 4** in-flight requests from a fixed worker pool over a shared cursor, results written
  into a pre-sized array **by index** so completion order never affects row order.
- **AC-48** While fetching, a determinate progress indicator shows "Exporting {done} of {total}…"
  (AC-134) with a Cancel action. Cancelling aborts the in-flight fetches via one shared
  `AbortController` and produces **no** file download and no object URL.
- **AC-49** On completion the browser downloads one `text/csv;charset=utf-8` file named
  `hookbox-requests-<token>-<YYYYMMDDTHHMMSSZ>.csv` (the stamp is `new Date().toISOString()` with `-`,
  `:` and the `.mmm` fraction removed), through the shared `src/lib/download.ts` **[new]** helper, and
  the object URL is revoked in a `finally` (AC-12 uses the same helper, so the two cannot drift).
- **AC-50** The file's first line is the frozen header row, and every data row has exactly these 10
  columns in this order (§5.6):
  `timestamp,method,path,status_code,served_by,duration_ms,request_headers,request_body,response_headers,response_body`
- **AC-51** Rows appear in the same order as the snapshot (newest first).
- **AC-52** **Per-row detail failure never aborts the export.** If a row's detail fetch returns 404
  (the documented "pending" case for a just-streamed trace **[existing — verified at
  `src/screens/dashboard/inspector.tsx:63-66`]**), the row is still emitted from the summary fields the
  feed already holds and the four detail cells contain the literal `pending`. For any other per-row
  failure (5xx, network, `contract_mismatch` **[existing — verified at `src/api/client.ts:128-134`]**,
  or the AC-119 timeout) those four cells contain the literal `unavailable`. **The six summary columns
  always come from the feed row, never from the detail response, even when the detail succeeds** — that
  makes a row's summary cells independent of the detail fetch and keeps this AC trivially true (§5.6).
  A completion toast reports the counts: `feed.export.done` or `feed.export.done.partial`.
- **AC-53** A 401 during export is the one aborting case: the client already clears the session and
  bounces **[existing — verified at `src/api/client.ts:99-105`]**, so the export stops, the screen
  unmounts, the abort fires from effect cleanup, and no file is downloaded.
- **AC-54** CSV correctness (RFC 4180): fields containing a comma, a double quote, CR or LF are wrapped
  in double quotes with embedded quotes doubled; the record separator is CRLF **including a trailing
  CRLF after the final record** (frozen so fixtures are byte-stable); the payload is UTF-8 **without** a
  BOM. Unit tests cover: a header value containing `,`; a JSON body containing `"`; a body containing a
  literal newline; a non-ASCII body (e.g. `héllo…🎉`) that round-trips byte-exact.
- **AC-55** CSV formula-injection guard: any cell whose first character is `=`, `+`, `-`, `@`, TAB
  (U+0009) or CR (U+000D) is emitted with a single leading apostrophe `'`. **Ordering is frozen as
  guard, then quote.** Unit test: a request body of `=cmd|' /c calc'!A1` exports as the **unquoted**
  `'=cmd|' /c calc'!A1` — it contains no `,`, `"`, CR or LF, so AC-54's own rule does not quote it. *The
  draft's "(quoted per AC-54)" parenthetical was wrong and is deleted (`architecture.md` D12).* The
  guard can never fire on the two bare-integer columns.
- **AC-56** `request_headers` and `response_headers` cells are compact JSON objects
  (`{"content-type":"application/json"}`), not flattened strings. **Redaction is asymmetric and the CSV
  reflects the server verbatim, adding none of its own:** `request_headers` arrive already redacted
  (`authorization`, `cookie`, `x-owner-id` → `<redacted>`) **[existing — verified at
  `backend/src/helpers.rs:34-45`, applied to *request* headers only at
  `backend/src/interceptor/engine.rs:674`]**, while `response_headers` are persisted **verbatim** with
  no redaction **[existing — verified at `backend/src/interceptor/engine.rs:636-644` — no `redact()`
  call on that map]**. **`security.md`'s S-4 ruling explicitly ACCEPTS verbatim response headers for the
  owner Inspector and this CSV** — the operator is proxying to a backend they own, "why didn't my cookie
  stick?" is the whole point of an inspector, and the CSV is a file they asked for; only the **public**
  projection filters (§5.11). A test asserts a request `Authorization` header exports as `<redacted>`
  and that the exporter does not mutate response-header values. AC-S25 adds the one value change:
  echo-mode `response_body` now carries `<redacted>` for those request headers (AC-S3).
- **AC-56a** The `response_body` cell carries the **actual captured response body** for every row whose
  detail fetch succeeded, delivered by F7 (§4.8) — it is not a documented-empty column. An end-to-end
  test adds a rule with a known `body_template`, sends one mock request, exports, and asserts the
  `response_body` cell equals the rendered body byte-for-byte after CSV unquoting (subject to AC-70
  truncation). A row whose stored `response_body` is `NULL` (an empty-bodied response — 204 preflight,
  empty CRUD, chaos dropout — or a trace captured before F7 shipped) exports as an **empty cell**, which
  is semantically distinct from the `pending` / `unavailable` sentinels of AC-52.
- **AC-115** **The exported row set is a snapshot taken at activation.** Rows change during an export
  (arrivals prepend, the 100-cap evicts, another tab clears), so the row array **and its order** are
  captured at click time, `total` is fixed for the run, and later arrivals are excluded. The menu
  item's note says so before the click: `feed.export.note` ("Exports the {n} requests listed now,
  newest first."). Test: inject an arrival mid-export and assert the file has exactly `total` data rows,
  none of them the new arrival.
- **AC-116** **Buffered ("N new") rows are excluded, and that is stated.** While paused, arrivals sit in
  `buffer` and are not "visible" **[existing — verified at `src/feed/use-feed.ts:77`]**, so they are not
  exported. `feed.export.note`'s `{n}` is the visible count, which makes the exclusion legible rather
  than surprising. (The alternative — flushing the buffer first — was rejected: it would mutate what the
  operator is looking at as a side effect of an export.)
- **AC-117** **There is a "Preparing file…" phase, and Cancel stays mounted.** 100 rows × two ≤ 256 KB
  bodies can be tens of MB of string building plus `Blob` construction — a genuinely multi-second
  main-thread pause after the last fetch resolves. Without a phase change the UI looks hung at the
  finish line. At 100 % the label swaps to `feed.export.preparing` and **Cancel becomes `disabled`, not
  removed** — it cannot interrupt synchronous serialisation, and removing it would reflow the strip at
  the most anxious moment. The strip's geometry is unchanged between phases (AC-134).
- **AC-118** **"No file was produced" has a state beyond 401.** A `Blob`/`createObjectURL` failure, a
  browser-blocked download, or an out-of-memory serialise renders `feed.export.error.file` ("Couldn't
  create the file. Your browser may have blocked the download, or the export was too large to build.");
  any other terminal failure renders `feed.export.error`. Neither leaves the progress strip stuck.
- **AC-119** **Every per-row fetch has a timeout, and 429 is handled.** A hung detail fetch maps to the
  `unavailable` sentinel after a bounded per-row timeout rather than leaving the progress strip stuck
  with only Cancel. `GET /api/requests/{id}` has no rate limit today **[existing — verified: only
  `POST /api/session` is limited, at `backend/src/routes/api.rs:211-214`]**, but the exporter honours a
  429's `Retry-After` if one is ever added, in `src/lib/request-export.ts` — the single place that would
  need to change (§8-R6).
- **AC-120** **One export at a time.** The menu item is disabled while `exporting === true` (no queue,
  no second strip), and Clear all is disabled for the duration (AC-82). Navigating away or unmounting
  aborts; there is deliberately **no** `beforeunload` prompt — too aggressive for a re-runnable read.
- **AC-121** **The sentinels' ambiguity is documented, and the recipient can learn what they mean.**
  §5.6 records explicitly that a request or response body whose literal text is `pending` or
  `unavailable` is indistinguishable from the sentinel; this is **accepted** rather than papered over
  (the alternatives — a sentinel shape that cannot occur naturally, or an out-of-band flag column —
  would either break the frozen 10-column format or make the file harder to parse). **No comment line is
  added to the file**: RFC 4180 has no comment syntax and §5.6 is frozen, which also resolves
  `security.md` OQ-S4. Instead, after an export that produced any sentinel, `feed.export.detailNote` is
  rendered as a **dismissible** `InlineAlert variant="info"` in the feed pane (resolving `copy.md`
  §7.10 in favour of its recommended option (a)) — the toast is too short-lived to carry it.

### 4.6 — Default catch-all rule (F6 — opt-in, pure rules-CRUD)

- **AC-57** The Rules Manager renders an "Add default rule" control in the toolbar, immediately left of
  the primary "New rule" **[existing — verified at `src/screens/rules-manager.tsx:191-194`]**, **and**
  centred in the empty state **[existing — verified at `:218-227`]** with a one-line helper
  (`rules.default.helper`) beneath it. Both instances are `Button variant="secondary" size="sm"` —
  `variant="primary"` is the single accent button per surface and "New rule" already owns it **[existing
  — verified at `src/components/ui/button.tsx:1-6`]**. **No second CTA is added to the empty state**
  ("New rule" is in the toolbar directly above).
- **AC-58** Activating it — **after AC-122's conditional confirm, when one applies** — issues exactly
  one `POST /api/endpoints/{token}/rules` with the frozen payload in §5.5.7 **as amended by AC-125**
  (`match.method = "ANY"`, `match.path = "/*"`, `response.status_code = 200`,
  `response.content_type = "application/json"`, `priority = 1000`, `enabled = true`, and the
  obviously-placeholder JSON `body_template` of AC-125). No other route is called.
- **AC-59** `priority = 1000` is asserted, so the catch-all loses to every default-priority rule (100)
  — "lower wins" **[existing — verified at `src/lib/copy.ts:176` ("Lower wins. Ties break by creation
  order.") and `backend/src/routes/api.rs:542` (`ORDER BY priority, id`)]**. 1000 is inside the accepted
  `0..=100000` band **[existing — verified at `backend/src/routes/api.rs:561-565`]**. **This orders rules
  against each other only and is explicitly *not* protection against AC-122's shadowing problem.**
- **AC-60** On 201 the rules list reloads and shows the new rule with `ANY` and `/*` in the Match
  column, and `rules.default.toast` fires. On failure `rules.default.error` fires as a danger toast and
  the list is unchanged. The new rule sorts **last** (priority 1000), so in a long list it lands below
  the fold; a `scrollIntoView({block:'nearest'})` on the new row is a courtesy, not a requirement.
- **AC-61** The control is **disabled with an explanatory, keyboard-reachable reason** when the endpoint
  already has a catch-all — see AC-123 for the full predicate and AC-124 for reachability.
- **AC-62** **Not auto-created:** `POST /api/endpoints` yields an endpoint whose
  `GET /api/endpoints/{token}/rules` is `[]`. A regression test asserts a freshly created endpoint has
  zero rules and that an unmatched request still returns the configured default (404 for
  `default_mode = "mock_404"`) **[existing — verified at `backend/src/router.rs:553-566`]**.
- **AC-63** **No backend change is required** — asserted, not assumed. The matcher already treats
  `"ANY"` as matching every method **[existing — verified at
  `backend/src/interceptor/matcher.rs:182`]** and already compiles `/*`, `*` and `/**` to a pure
  catch-all **[existing — verified at `backend/src/interceptor/matcher.rs:53`]**, with those values as
  the serde defaults **[existing — verified at `backend/src/models.rs:135-138`]**. An end-to-end check
  sends `PATCH /anything/at/all` at the mock surface after adding the default rule and observes 200 with
  `X-HookBox-Served-By: rule`. If any matcher/engine edit turns out to be needed, that is a scope change
  and must be raised, not silently implemented.
- **AC-122** **A catch-all silently disables Auto-CRUD, the tunnel, MITM and `default_mode` — so it
  cannot be a silent one-click action on such an endpoint.** (`journey.md` BLOCKER 36 / E21–E22.)
  Verified mechanism: a matched rule short-circuits the engine **[existing — verified at
  `backend/src/interceptor/engine.rs:141-145`]**, and Auto-CRUD → tunnel → MITM → `default_mode` are all
  reached **only** in the `else` branch via `resolve_unmatched` **[existing — verified at
  `backend/src/interceptor/engine.rs:228-245`]**. `priority = 1000` gives **no** protection (AC-59), and
  R8 only ever mentioned "404 becomes 200". Required behaviour:
  (a) Before the POST, the client evaluates the endpoint's four fallbacks — `auto_crud === true`,
  `tunnel_active === true`, `target_url` non-empty, `default_mode === "echo"`.
  (b) **If one or more applies**, a confirm dialog opens carrying `rules.default.shadow.title` +
  `rules.default.shadow.body`, then **one bullet per fallback that is actually active** —
  `.shadow.crud` / `.shadow.tunnel` / `.shadow.proxy` / `.shadow.echo` — then
  `rules.default.shadow.recover` ("Switch the rule off or delete it to get them back."), with
  `rules.default.shadow.confirm` ("Add rule anyway") as the confirm and `common.cancel` as the ghost.
  Confirm is `variant="primary"`, **not** `danger`: this is recoverable and may be exactly what the
  operator wants. **Conditional bullets only** — never a paragraph listing fallbacks that are switched
  off.
  (c) **If zero apply, there is no confirm at all** — straight to the POST. The common "make a brand-new
  endpoint answer something" path stays one click, which is F6's whole point.
  (d) Cancelling issues **zero** requests.
  (e) A **backend regression test** asserts that an endpoint with `auto_crud = true` plus a catch-all
  rule is served by `rule`, not `crud` — so the shadowing is a chosen, tested behaviour rather than
  something discovered in production.
  *Rejected alternative:* refuse-with-explanation. It would block a legitimate intent (an operator who
  deliberately wants to park a proxying endpoint behind a static 200) and there is no other one-click
  path to that state.
- **AC-123** **The duplicate guard is complete.** Three holes in the draft's predicate are closed:
  (a) a **disabled** catch-all also blocks a new one — the predicate is
  `rules.some(r => r.match.method === 'ANY' && r.match.path === '/*')` with **no `enabled` filter** —
  and the reason becomes `rules.default.existsDisabled` ("…but it's switched off. Turn it back on
  instead of adding another."), because the draft's `enabled`-only check let a second catch-all be
  created;
  (b) the control is **disabled while the POST is in flight** (`loading`), which is the only thing that
  actually closes the double-click window AC-61 named — the guard is computed from the rules list, which
  refreshes only *after* the reload;
  (c) a **stale list** (another tab created one seconds ago; there is no server-side guard) is handled
  by refreshing the list and showing `rules.default.error.duplicate` ("This endpoint already has a
  catch-all rule. The list has been refreshed."), never by silently creating a duplicate.
- **AC-124** **The disabled reason is reachable by mouse and keyboard.** A `disabled` `<button>` fires
  no pointer events and is out of tab order, so a `Tooltip` on it is dead for **both** input methods.
  The button is wrapped: `<Tooltip content={reason}><span tabIndex={0} title={reason}
  className="inline-flex rounded-sm"><Button disabled …/></span></Tooltip>`, with the reason also
  mirrored in `title` as a no-JS fallback and exposed via `aria-describedby`. The wrapper needs no focus
  classes — the global `:focus-visible` rule paints every focusable element **[existing — verified at
  `src/globals.css:247-251`]** — but keeps `rounded-sm` so the outline's corners match the button
  inside it.
- **AC-125** **§5.5.7's frozen `name` and `body_template` are amended to `copy.md`'s values.** The rule's
  own content is user-visible in three places (the rules list, the endpoint's HTTP response, and any log
  or test failure a caller reads), so it is **copy**, and `copy.md` owns it (§7.4 there). Frozen:
  `name` = `Catch-all (default)` (unchanged) and `body_template` =
  `{\n  "ok": true,\n  "hookbox": "default catch-all",\n  "message": "Edit this rule in HookBox to return your own response."\n}`.
  Rationale: `"ok": true` gives someone wiring up a client an immediately truthy response (F6's whole
  point); `"hookbox": "default catch-all"` makes it unmistakably a placeholder in a log or a test
  failure; `message` names the exact next action; and the shape is deliberately **not** that of a real
  API response, so nobody ships against it by accident. Any AC-58 fixture asserting the draft's older
  bytes is updated in the same change.

### 4.7 — Cross-cutting

- **AC-64** All new user-facing strings are added as keys to the single copy table **[existing —
  verified at `src/lib/copy.ts`]** and referenced only via `t()`; no literal user-facing English appears
  in any new/modified component. The values are `copy.md` §4's tables, wired 1:1 (a change to a value
  there is a copy edit, not a code edit). **The three CSV artifact literals are the deliberate
  exception:** `pending`, `unavailable` and the header row live as constants in `src/lib/csv.ts`
  **[new]**, not in `copy.ts`, because they go **into a machine-readable file** frozen by AC-50/AC-52
  and must never be reworded, capitalised or translated (resolving `copy.md` §7.13).
- **AC-65** Every new interactive control has an accessible name, is keyboard reachable, shows the
  existing visible focus ring, and every new dialog traps focus and restores it on close (matching the
  existing `Dialog` primitive **[existing — verified at `src/components/ui/dialog.tsx`]**). Specific
  cases covered elsewhere: the `sr-only` file input's projected ring (AC-92), the disabled button's
  tooltip wrapper (AC-124), the inline revoke confirm's `Esc` handling (AC-95), and the viewer's
  disclosure rows, which are `<button type="button" aria-expanded aria-controls>` inside a
  `role="region"` — **not** `role="option"`/`listbox`, because there is no single-selection model and
  `FeedRow`'s listbox semantics **[existing — verified at
  `src/components/hookbox/feed-row.tsx:50-53`]** are wrong for a page whose content updates underneath
  the reader. The viewer's document structure is `<header>` → banner (`role="status"`) → `<main
  id="main">` with exactly one `<h1>` → `<footer>`, plus the reused skip link `t("shell.skipLink")`
  **[existing — verified at `src/components/hookbox/app-shell.tsx:74-79`]**. On the viewer,
  `Enter`/`Space` toggle a row, focus stays on the trigger through expand and collapse, and a poll that
  adds rows never moves focus and never collapses an open row.
- **AC-66** `pnpm typecheck` passes; `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`
  pass; the existing Playwright suite passes unchanged except for specs deliberately updated for AC-7
  (AC-85) and AC-77's shipped-string change.
- **AC-67** No user-supplied value from any new surface is rendered with `dangerouslySetInnerHTML`; all
  values render as text nodes through the existing `KeyValueRows` / `JsonTree` / `CodeBlock` primitives,
  as the owner-side inspector already does **[existing — verified at
  `src/screens/dashboard/inspector.tsx:16-19`]**. **AC-S13 extends this** beyond `innerHTML` to
  `href`/`src`/`srcdoc`/`style`/`on*`, which AC-67 alone did not cover.
- **AC-126** **Every new control has a defined offline behaviour.** The dashboard already renders an
  offline banner **[existing — verified at `src/screens/dashboard.tsx:264-274`]**. While
  `navigator.onLine === false`: Clear all and Export CSV are **disabled** with
  `feed.actions.offlineHint` (AC-76); Share (mint/list/revoke) and Import are **attempted and fail**
  with the existing network message **[existing — verified at `src/api/client.ts:93-95`]** rather than
  being hidden, because `navigator.onLine` is unreliable and a false negative must not lock the operator
  out of their own endpoint. The viewer's offline state suspends polling and resumes on `online`
  (AC-105(e)), showing `viewer.offline.title` / `.body`.
- **AC-127** **Both newly crowded headers stay usable at 360 px.** The feed header (title + count +
  "N new" pill + the overflow trigger + Pause) fits at the feed pane's `min-w-feed` minimum **[existing
  — verified at `tailwind.config.ts:113-115`]** — that is what AC-1's overflow menu buys. The sub-header
  is already `flex-wrap` with `gap-x-4 gap-y-2` **[existing — verified at
  `src/components/hookbox/app-shell.tsx:98`]**; F2's deletion buys the width Share needs, and below `sm`
  Share's label collapses to icon-only (`sr-only sm:not-sr-only`) while the count badge stays visible.
  Assertable: no horizontal scroll and no clipped control at 360 px on either header. **Recorded as
  explicitly out of scope:** `shell.mobileNav.*` copy keys exist **[existing — verified at
  `src/lib/copy.ts:59-60`]** but nothing renders them, so the owner shell has no mobile navigation plan
  today; this batch must not make that worse, and does not attempt to fix it (`ux.md` gap 30).
- **AC-128** **Reduced motion is respected, and nothing is hidden by it.** The global block zeroes every
  animation and transition and kills the skeleton shimmer **[existing — verified at
  `src/globals.css:283-301`]**, so no new motion needs bespoke handling. What must be **verified rather
  than assumed**: under `prefers-reduced-motion: reduce` the export strip, the `Progress` fill, the
  viewer's disclosure and every new dialog still render and still convey their state — the fill jumps
  instead of easing (correct: the digits carry the value), the strip appears instantly, and the chevron
  snaps (correct: `aria-expanded` and the recessed well carry the state). The guard zeroes durations; it
  must not hide anything.
- **AC-129** **`variant="danger"` passes AA in dark theme.** `src/components/ui/button.tsx:21`
  hardcodes `bg-danger-fg text-white` **[existing — verified]**, and white on `--red-400-dark` is
  ≈**1.7:1** — an AA failure on **every** confirm button in the product today (rule delete, clear
  history, clear state, delete endpoint), to which F1's confirm and F4's revoke add two more. Fix is one
  token: `text-white` → `text-text-on-accent`, giving ≈6.5:1 light and ≈8.3:1 dark. In scope precisely
  because it is a shared primitive this batch newly consumes. Assertable by a computed-contrast check in
  both themes. Cosmetically visible on four shipped dialogs; strictly an a11y improvement.
- **AC-130** **New code never uses the dead background classes, and the sites this batch edits are
  fixed.** `bg-subtle`, `bg-hover` and `bg-active` generate **no CSS**: `tailwind.config.ts` defines
  them under `colors.surface.*` **[existing — verified at `tailwind.config.ts:20-27`]**, so the
  generated utilities are `bg-surface-subtle` / `bg-surface-hover` / `bg-surface-active`. Twelve
  existing call sites are therefore silently unstyled. Scope, decided precisely (§0 item 9):
  (a) **all new code uses the `bg-surface-*` names** — assertable by a grep of new/changed files for the
  three dead class names returning nothing;
  (b) the dead classes are **fixed only at sites inside files this batch already edits** —
  `rules-manager.tsx:232`, `app-shell.tsx:104` and `:163`, `feed-row.tsx:64-65`, `inspector.tsx:266`
  **[all existing — verified]** — because the new viewer column band copies the rules-manager band's
  treatment and the two must match;
  (c) **out of scope, follow-up issue filed at BREAKDOWN:** `code-block.tsx:22,42`, `json-tree.tsx:81`,
  `tabs.tsx:19`, `segmented.tsx:25`, `connection-pill.tsx:59`, `rule-builder.tsx:518`,
  `slider.tsx:30`, plus two related product-wide findings — `text-overline` already sets
  `letter-spacing: .06em` and the existing bands' added `tracking-wide` (.025em) wins and *reduces* it,
  and `border-danger-fg/40` **[existing — verified at `src/screens/settings.tsx:422`]** paints
  full-strength because Tailwind 3.4 silently drops alpha modifiers on `var()` tokens (`design.md`
  §9.3 items 3 and 5). **New code must not use an alpha modifier on a token colour** — use a token that
  already encodes the tint (`accent-subtle-bg`, `*-bg`) or a solid border token.
  A repo-wide sweep is deliberately rejected: it would touch nine unrelated components and change their
  rendered appearance in a batch whose visual review budget belongs to F4.
- **AC-131** **Hardcoded strings on the newly public components move to `t()`.** `JsonTree` hardcodes
  "Pretty", "Raw", "JSON view mode" and "(empty)" while `insp.body.pretty` / `insp.body.raw` sit unused
  in the table, and `KeyValueRows` hardcodes "redacted" and "None" while `insp.headers.redacted` sits
  unused **[both existing — verified at `src/components/hookbox/json-tree.tsx:83-98`,
  `src/components/hookbox/key-value-rows.tsx:22`, `:34`]**. Pre-existing debt, but F4 puts all of them
  on an **unauthenticated** page, where AC-64 would otherwise be unsatisfiable. One new key is added:
  `insp.headers.none` = "None".
- **AC-132** **The copy button tells the truth.** `setCopied(true)` currently runs **outside** the
  `try/catch` around `navigator.clipboard.writeText` **[existing — verified at
  `src/components/ui/copy-button.tsx:22-31`]**, so the UI announces "Copied to clipboard" — including
  through its `sr-only role="status"` live region at `:47-49` — when nothing was copied. The shipped
  nginx listens on plain HTTP:80 **[existing — verified at `deploy/nginx.conf:1-2`]**, a **non-secure
  context** where the Clipboard API is unavailable, i.e. precisely the deployment where an operator
  copies a share URL. Required: the success state is entered **only after the write resolves**; on
  rejection a **distinct failure state** renders `common.copy.failed` ("Couldn't copy. Select the text
  and copy it manually.") and the value is presented as manually-selectable text. Promoted from
  pre-existing debt to in-scope because **F4's share URL is shown exactly once** (AC-24) — a silent copy
  failure there loses the link with no recovery, which would make `share.created.onceHint` a promise the
  copy button can break. Two tests: a resolving clipboard shows success; a rejecting one shows the
  failure state and never announces success. This also repairs the Mock URL chip and F3's flows.
- **AC-133** **The redaction pill renders.** `KeyValueRows` compares against the literal
  `__redacted__` **[existing — verified at `src/components/hookbox/key-value-rows.tsx:9`]** but the
  backend writes `<redacted>` **[existing — verified at `backend/src/helpers.rs:43`]**, so the pill has
  **never** rendered anywhere it is used and the literal string `<redacted>` displays as an ordinary
  mono header value. One-constant fix. Required in this batch because AC-S1 makes redaction load-bearing
  on an unauthenticated page: "HookBox hid this" must be unmistakable and must not look like a value the
  caller actually sent. The shipped pill treatment is already correct for the job — a **neutral** chip
  **[existing — verified at `src/components/hookbox/key-value-rows.tsx:32-35`]**, not red (nothing
  failed) and not mono (so it cannot be mistaken for data). Test: a header value of `<redacted>` renders
  the chip, in both the owner Inspector and the public viewer.
- **AC-134** **One determinate progress contract, shared by F3 and F5.** A new
  `src/components/ui/progress.tsx` **[new]** (no such primitive exists) renders a 6 px bar from tokens
  only: track `h-1.5 w-full rounded-pill bg-surface-active`, fill `bg-accent-fill` with
  `transition-[width]` only (no transform, no layout, nothing animating `height` or `box-shadow`).
  Required properties: `role="progressbar"` + `aria-valuemin`/`aria-valuemax`/`aria-valuenow` +
  `aria-valuetext` carrying the **localized label**, so the digits and the bar can never disagree; the
  visible numeric label carries `.tnum` **[existing — verified at `src/globals.css:257-259`]** so digits
  do not reflow while counting; `total` is known before the first request in both callers, so the bar is
  never indeterminate; no layout shift between phases (AC-117). **The visible label is not an
  `aria-live` region** — 100 polite announcements would be abusive; a separate `sr-only
  aria-live="polite"` node announces at start, ~25/50/75 % and completion only
  (`feed.export.announce` / `set.config.import.announce`). During an export the strip must not re-mount
  per tick: only the label text and the fill's inline width change.

### 4.8 — Response body capture (F7 — enabling backend fix; F4 and F5 depend on it)

> **Scope note.** This is a *capture and persist* fix on the trace-write path only — **not** a
> response-path refactor (§2). Everything downstream of the DB column already exists and is verified:
> the column `request_logs.response_body TEXT` **[existing — verified at
> `backend/migrations/0001_init.sql:65`]**, the insert binding **[existing — verified at
> `backend/src/db.rs:53`, `:85`]**, the read path **[existing — verified at
> `backend/src/routes/api.rs:869`]**, the Rust model **[existing — verified at
> `backend/src/models.rs:266`]**, the TS schema **[existing — verified at `src/api/schemas.ts:169`]**
> and the Inspector panel **[existing — verified at `src/screens/dashboard/inspector.tsx:246-253`]**.
> **The single missing piece is one hardcoded `None`** **[existing — verified at
> `backend/src/interceptor/engine.rs:683`]**.

**Feasibility grounding (verified, so no `served_by` path needs to be scoped out).** Every mock-plane
response body is fully materialised in memory at construction; no path streams from a socket and no
path needs re-reading, a second upstream request, or any disk I/O:

| `served_by` | Where the bytes are built | Expected persisted `response_body` |
|---|---|---|
| `rule` | `Body::from(body_out.clone())` **[verified at `backend/src/interceptor/engine.rs:216-219`]** | the rendered body (AC-70 truncation) |
| `crud` | `json_response(...)` **[verified at `:409`]**, or `Body::empty()` **[`:410-413`]**; store error → `json_response` 503 **[`:419-426`]** | the serialised JSON; `NULL` for the empty/204 case |
| `tunnel` | `Body::from(body_bytes)` **[verified at `backend/src/routes/tunnel_ws.rs:70-76`]** — the bound CLI's reply, already fully buffered off the WS; failure → `json_response` 504 **[`engine.rs:448-458`]** | the CLI's body (AC-71 lossy UTF-8, AC-70 truncation); or the error JSON |
| `mitm` | `Body::from(pr.body)` **[verified at `engine.rs:478-481`]** — `pr.body` is a `Vec<u8>` already fully buffered and already capped at `MITM_MAX_BODY_BYTES` **[verified at `backend/src/interceptor/proxy.rs:211-213`]**; timeout/unreachable → `json_response` **[`:490-496`, `:501-507`]** | the upstream body (AC-71, AC-70); or the error JSON |
| `default` | `json_response` for `echo` **[verified at `:514-523`]** and `mock_404` **[`:525-533`]** | the serialised JSON — **with the echo payload's `headers` sub-object redacted, AC-S3** |
| `cors` | 204 preflight, `Body::empty()` **[verified at `:80`]** | `NULL` (AC-69) |
| `chaos` | `Chaos::Status` → `json_response` **[verified at `:325-329`]**; `Chaos::Drop` → `Body::empty()`, `status_code` 0 **[`:299-321`]** | the chaos error JSON; `NULL` for dropout |
| `ratelimit` | `json_response` 429 **[verified at `:263-274`]** | the rate-limit error JSON |

One pre-existing gap is **not** addressed and is not a regression: the 413 ingest-cap rejection returns
**without** calling `spawn_trace` **[existing — verified at
`backend/src/interceptor/engine.rs:65-72`]**, so it writes no trace row at all and has no
`response_body` to capture.

**Mechanism — frozen by `architecture.md` §2.10 as (i), "buffer once".** `spawn_trace` receives
`resp: &Response` **[existing — verified at `backend/src/interceptor/engine.rs:628`]** and today can
only read `resp.headers()`; an `axum::Body` cannot be read through a shared reference, which is exactly
why the current code gives up. The chosen shape is one `capture_response_body(resp) -> (Response,
Bytes)` helper called at four sites (with the chaos-dropout site passing a literal empty slice),
covering all eight `served_by` values through one code path, touching **neither** `tunnel_ws.rs`
**nor** `proxy.rs`. It is allocation-free for the payload: every body is a **single-frame** body built
from an owned buffer, so `to_bytes` returns the same `Bytes` via `Bytes::split_to` and rebuilding is a
refcount bump **[verified by `architecture.md` D1/D4 against `axum-0.7.9` and
`http-body-util-0.1.3`]**. Capturing **after** `identified()` is safe because that function only calls
`insert_header` and returns the same `Response` **[existing — verified at
`backend/src/interceptor/engine.rs:576-593`]** — which also keeps the trace's `response_headers`
complete. A `size_hint().exact()` guard is a forward-compatibility fuse: if a streaming mock body is
ever introduced it captures nothing rather than buffering an unbounded stream.

- **AC-68** **Every `served_by` path persists the response body it actually sent.** For each of the
  eight rows above, one integration test **[existing — verified at `backend/tests/api.rs`]** drives the
  mock plane, then asserts via `GET /api/requests/{id}` **[existing — verified at
  `backend/src/routes/api.rs:850-874`]** that `response_body` equals the bytes the HTTP client
  received, lossy-UTF-8 decoded (AC-71) with AC-70 truncation applied — the right-hand column, cell by
  cell. Every non-empty-bodied path must be **non-`null`**; the three genuinely empty-bodied cases must
  be `null` per AC-69. **No path may be left unimplemented or silently `null`: the matrix is eight cases
  and all eight assert an exact value.** The `rule` path is the primary case. The `default(echo)` case
  asserts AC-S3's redacted value, not the raw header map.
- **AC-69** **Empty body ⇒ `NULL`, mirroring `request_body` exactly.** The rule is the same convention
  already used on the request side **[existing — verified at
  `backend/src/interceptor/engine.rs:677-681`]**, and it is **structural**, not incidental: the capture
  helper's zero-length fast path returns empty bytes for the 204 CORS preflight, the empty 204 CRUD
  response and the chaos dropout. Tests assert `response_body IS NULL` (serialised as `null`) for all
  three. **No empty string is ever stored**, even under a pathological `MAX_BODY_BYTES=0`.
- **AC-70** **Truncation is identical to `request_body`'s existing convention, and made panic-safe.**
  Bodies are cut to `MAX_BODY_BYTES` **[existing — verified at `backend/src/config.rs:170`, default
  `256_000`]** with **no truncation marker, no suffix, no flag column** — exactly what `truncate` does
  today **[existing — verified at `backend/src/interceptor/engine.rs:646-653`]**; **both** body columns
  go through the **same** helper so they are indistinguishable in behaviour. That shared helper must
  floor the cut to a UTF-8 character boundary: today's byte slice **panics** when byte `cap` lands
  mid-character, and response bodies are attacker/upstream-controlled (§8-R12). Tests: (a) a
  300 000-byte ASCII body stores exactly `MAX_BODY_BYTES` bytes with no marker; (b) a body whose
  256 000th byte is mid-multibyte stores `MAX_BODY_BYTES - k` bytes (`k` ≤ 3) and **does not panic**;
  (c) a body of exactly `MAX_BODY_BYTES` is stored whole; (d) the same three cases pass for
  `request_body` **through the same helper**. AC-S17 adds the two conditions this AC alone did not
  cover: the mid-multibyte request must still return a normal 2xx **and** still write a trace row.
- **AC-71** **Non-UTF-8 response bodies never panic and never corrupt the row.** `mitm` and `tunnel`
  bodies are arbitrary `Vec<u8>`; they are decoded with the same lossy conversion the request path uses
  **[existing — verified at `backend/src/interceptor/engine.rs:73`]**. Test: an upstream returning
  `0x80 0xFF 0xFE` yields a stored `response_body` containing U+FFFD replacement characters, a 200 from
  `GET /api/requests/{id}`, and — critically — **the mock client still receives the original raw bytes
  byte-identically** (AC-72).
- **AC-72** **Zero client-visible change.** For every `served_by` path, the status line, every response
  header, and every response byte the client receives are byte-identical to the pre-change build. A
  golden test captures a full response (status + sorted headers + body bytes) for the `rule`, `mitm`,
  `default(echo)`, `default(mock_404)`, `crud`, `cors` and `ratelimit` paths and compares against
  fixtures recorded before the change. `X-HookBox-Served-By` values are unchanged **[existing —
  verified at `backend/src/interceptor/engine.rs:585`]**. **AC-S3's echo redaction applies to the
  persisted copy only** — the client's echo body still contains the caller's real header values, which
  this AC asserts.
- **AC-73** **No measurable latency regression — defined so QA can actually check it.**
  (a) **No new blocking or awaited I/O on the request path.** Buffering bytes the process already holds
  is allowed; a new DB statement, file write, upstream call, or additional `.await` on anything that can
  block is not. Verified by inspection at the security/QA gate plus (b) and (c). See AC-S20.
  (b) **Statement/round-trip counts unchanged:** a mock request issues the same number of SQLite
  statements as before, and a MITM request still makes exactly **one** upstream request (asserted
  against a counting test upstream). **Note for QA (AC-S20):** `insert_trace` is **two** statements
  (insert + prune) **[existing — verified at `backend/src/db.rs:61-106`]**, so the baseline is
  "unchanged", **not** "one".
  (c) **Persistence stays fire-and-forget:** the insert remains inside the existing `tokio::spawn`
  **[existing — verified at `backend/src/interceptor/engine.rs:699-704`]** and nothing about it is
  awaited before the response is returned. Asserted by a test that makes the DB slow (or the pool
  saturated) and shows the mock response still returns promptly.
  **(d) is replaced** by three measurable sub-conditions (`architecture.md` D8/§2.10.7 — the draft's
  `max(2 ms, 10 %)` / `p95 ≤ 5 ms` on `overhead_ms` was **unfalsifiable**, because
  `duration_ms = t0.elapsed().as_millis()` **[existing — verified at
  `backend/src/interceptor/engine.rs:632`]** quantises to whole milliseconds and the baseline is
  0–1 ms, so "10 %" is degenerate and "p95 ≤ 5 ms" passes even at 400 % slower):
  **(d1) Harness-timed, µs resolution.** In `backend/tests/api.rs`, 200 sequential mock requests
  against a rule with a 64 KB `body_template`, `latency_ms = 0`, `chaos_pct = 0`, each timed with
  `std::time::Instant` around the call. Median and p95 **wall time per request in µs** are recorded in
  the PR body alongside the same numbers from the parent commit. **Pass: median ≤ baseline +
  max(0.5 ms, 20 %).**
  **(d2) Bucket stability on the reported metric.** For the same 200 requests, at least **95 %** must
  land in the same integer-millisecond `overhead_ms` bucket as the baseline — the coarse "did someone
  add a blocking call" guard, stated in terms 1 ms quantisation can express.
  **(d3) Direct helper bound.** A unit test asserts the capture helper on a 5 MB body (the
  `MITM_MAX_BODY_BYTES` worst case **[existing — verified at `backend/src/config.rs:177`]**) completes
  in **< 1 ms** and that the rebuilt response's bytes are byte-equal to the input — achievable only if
  no full copy happens.
- **AC-74** **No schema change, no migration, no new env var, no contract shape change *for F7*.**
  `sqlx::migrate!` runs `0001_init.sql` plus F4's `0002_share_links.sql` **[existing — verified at
  `backend/src/db.rs:31-34`]**; `MAX_BODY_BYTES` is reused as-is. A test asserts the `RequestDetail`
  JSON key set is **unchanged** (`response_body` was already a present nullable key **[existing —
  verified at `backend/src/models.rs:266`, `src/api/schemas.ts:169`]**), so no frontend schema edit is
  required and the owner Inspector renders captured bodies with **zero** frontend changes **[existing —
  verified at `src/screens/dashboard/inspector.tsx:246-253`]**. F7 touches neither `Cargo.toml` nor
  `package.json` nor `src/api/schemas.ts`. (The one Cargo change in this batch is AC-S18's feature flag,
  which belongs to the security lane, not F7.)
- **AC-75** **The feed/WS payload stays body-free.** The `new_request` summary published to the feed hub
  still contains no request or response body **[existing — verified at
  `backend/src/interceptor/engine.rs:692-697`]**. A test asserts the `new_request` event payload has no
  `request_body` / `response_body` keys, so capture does not widen the WS surface — which keeps §5.7's
  "no changes" true and matters for F4's threat model.
- **AC-135** **Large and non-JSON bodies render safely, on both the Inspector and the viewer.** F7
  makes arbitrary upstream/CLI bytes reachable in a body panel for the first time. `JsonTree` already
  degrades to a raw `<pre>` when `JSON.parse` throws and caps its viewport at
  `max-h-[40vh] overflow-auto` **[existing — verified at
  `src/components/hookbox/json-tree.tsx:71-99`]**, so HTML, plain text and mojibake render safely as
  text nodes (AC-67). **But** a 256 KB *valid* JSON body builds thousands of recursive nodes with two
  levels auto-expanded **[existing — verified at `src/components/hookbox/json-tree.tsx:13`]**, which can
  jank or freeze the tab. Required: default the view mode to raw above a ~64 KB threshold, with
  `insp.body.largeRaw` ("Large body — showing raw text for speed.") rendered as a **quiet caption**
  (`px-3 pb-2 text-caption text-text-tertiary`) directly under the Pretty/Raw strip — **not** an
  `InlineAlert`: nothing is wrong, the tool made a performance choice (resolving `design.md` gap 14). A
  parse failure renders `insp.body.notJson` ("Not valid JSON — showing raw text.") the same way. Both
  apply to the owner Inspector **and** the public viewer's detail well, so both strings are
  voice-neutral.
- **AC-136** **Truncation is disclosed as the hedge it is.** AC-70 stores no marker, so the only
  available signal is a stored length equal to `MAX_BODY_BYTES`, which cannot distinguish "exactly the
  cap" from "cut at the cap". The already-shipped-but-unwired key `insp.body.truncated` **[existing —
  verified at `src/lib/copy.ts:135`]** is **wired off that heuristic**, and its **value changes** from
  "Truncated at {bytes} — captured bodies are capped." to **"Capped at {bytes} — this body may be cut
  short."** — because the old wording asserts a fact we do not have. Rendered as the same quiet caption
  as AC-135. Silence was rejected specifically because it is worst on the public viewer, where a viewer
  has no other way to learn a body was cut. (This is the user-facing counterpart to §8-R4's
  developer-only length heuristic, resolving `journey.md` gaps 39/40 and `ux.md` gap 22.)

### 4.9 — Required security ACs (F4, F7, F3, cross-cutting)

Lifted from `security.md` §3. **Every AC here is required**, not advisory; `security.md`'s own
MUST/SHOULD grading is recorded, but a SHOULD is only downgraded where this PRD says so explicitly and
gives a reason. **AC-S1, AC-S2 and AC-S4 are blocking prerequisites for F4** (§0 item 1) — without them
"read-only share link" is false. The `AC-S` prefix is retained so every cross-reference in
`security.md`, `design.md` and `copy.md` still resolves (§0 item 11).

**F4 — the public projection and its boundary**

- **AC-S1** **(MUST)** The public response-header **filter** lives in `backend/src/routes/share.rs`
  **[new]** and is applied **only** to the public projection: **drop** every header whose lowercased
  name starts with `x-hookbox-` (verified real members: `x-hookbox-endpoint`, `x-hookbox-served-by`,
  `x-hookbox-rule-id` **[existing — verified at `backend/src/interceptor/engine.rs:584-588`]**,
  `x-hookbox-plane` **[existing — verified at `backend/src/router.rs:170`]**, `X-HookBox-Truncated`
  **[existing — verified at `backend/src/interceptor/proxy.rs:214`]**; the **prefix rule** is frozen
  rather than an enumeration, so a future `x-hookbox-*` header cannot leak by omission), and **mask the
  value to `<redacted>`** for `set-cookie`, `set-cookie2`, `authorization`, `proxy-authenticate` and
  `www-authenticate`. Both lists are frozen as named constants (§5.11). **Test: a MITM/tunnel
  `Set-Cookie: sid=abc` shows `<redacted>` publicly while `GET /api/requests/{id}` still shows
  `sid=abc`** — that asymmetry *is* the S-4 ruling. Nothing changes at storage, at the owner Inspector,
  or in F5's CSV. Losing `X-HookBox-Truncated` from the public view is an accepted small fidelity cost.
- **AC-S2** **(MUST)** Token absence is asserted **after** the filter, for all eight `served_by` paths,
  and **scoped to server-generated fields** — `response_headers` and the summary/identity fields.
  Caller-supplied `path`, `query_params`, `request_body` and `request_headers` are in the projection by
  design, so an unscoped "token is not a substring anywhere" test is flaky (a caller can put the token
  in their own body) and would get weakened by the first engineer who hits it. The scoped version is the
  one that can stay green forever.
- **AC-S4** **(MUST)** Row-level invariant: **nothing masked in `request_headers` appears verbatim
  anywhere else in the same public row.** This is the assertion that catches AC-S3's class of bug
  generically rather than one path at a time.
- **AC-S5** **(MUST)** `/s/{code}` carries `Referrer-Policy: no-referrer` **and**
  `X-Robots-Tag: noindex, nofollow`. Playwright asserts **no request from the page carries a `Referer`
  containing the code**. Necessary because `access_log off` alone is **not sufficient**: landing on
  `/s/<CODE>` returns `index.html` (log suppressed), then the browser fetches `/assets/index-*.js`, CSS
  and fonts under `location / { try_files … }` **[existing — verified at `deploy/nginx.conf:9-11`]**,
  which has no `access_log off`; those carry `Referer: https://host/s/<CODE>` and nginx's default
  `combined` format logs the referer — the same leak class as commit `47a267c`, a different location.
  **Resolving `security.md` OQ-S2: both mechanisms, `<meta>` first** — a `<meta name="referrer"
  content="no-referrer">` in `index.html` covers every deployment including `cargo run` and the
  nginx-less compose topology, and the nginx `add_header` is the right layer for the header form. Also
  required: every link on the viewer is same-origin and the page loads **no** off-origin subresource.
- **AC-S6** **(MUST)** `access_log off;` on `location /api/share/` and `location /s/` (both longer
  prefixes than the existing locations, so nginx's longest-prefix rule selects them), **plus** an
  assertion that the application's own `tracing` never emits the code or the full path. `share.rs` logs
  the resolved `share_links.id` when it logs at all. There is no `TraceLayer` on the app router today
  **[existing — verified at `backend/src/router.rs:178-193`]** and this batch must not add one.
- **AC-S7** **(MUST)** Limiter keys are **namespaced and separately bounded** so the public route can
  never evict a `rl:<token>` bucket. Until now keys were `rl:<token>`, bounded by owner-created
  endpoints; a per-IP key on an unauthenticated route makes the key space **attacker-controlled**, and
  eviction today is namespace-blind **[existing — verified at `backend/src/limiter.rs:88-90`,
  `:130-142`]** — so filling the map would flush mock-plane rate-limit buckets and let anyone exceed an
  endpoint's configured `rate_limit_per_min`. The check runs **before any DB read**. Test: 100 000
  distinct IPs, then assert endpoint limits are still enforced, latency is stable, and the bucket count
  stays bounded. **The session limiter's unbounded static IP map must not be copied** **[existing —
  verified at `backend/src/routes/api.rs:304-326`]**.
- **AC-S8** **(MUST)** The viewer stops polling on a **list** 404 and backs off on 429 per
  `Retry-After`. Fully specified, including the detail-404 carve-out, by AC-105 and AC-106.
- **AC-S9** **(MUST)** The resolver's liveness predicate is `share_links.revoked_at IS NULL` **AND**
  `endpoints.gone_at IS NULL`, read per request in the resolving statement (§5.2). The tombstone handler
  revokes shares **before** writing `gone_at`, or both happen in one transaction — **and the resolver
  checks `gone_at` itself regardless**, because `ON DELETE CASCADE` never fires for a tombstone
  **[existing — verified at `backend/src/routes/api.rs:506-532`, `backend/src/tasks/sweep.rs`]**. Test:
  force the revoke statement to fail and assert every share URL for that endpoint still 404s.
- **AC-S10** **(MUST)** `last_used_at` is written **off the response path** and **at most once per link
  per 60 s**, or dropped. Kept, with both conditions (§5.2): a `tokio::spawn` after the 200 value is
  built, with the coalescing predicate in the `WHERE` clause. Without this, an unauthenticated GET is a
  write primitive on a single-writer WAL database **[existing — verified at `backend/src/db.rs:16-27`]**
  contending with the mock plane's hot-path writer — and it would contradict the no-new-blocking-I/O
  posture AC-73 imposes on F7. Displayed per AC-97.
- **AC-S11** **(MUST)** The mint step's disclosure names bodies **and** response bodies **and** both
  header maps, up to the last 100 requests, **including other people's requests and pre-mint ones**, and
  the endpoint's name. Wording frozen and reconciled with AC-S1 by AC-93. **Because AC-S12 is declined,
  the pre-mint clause is mandatory**, not optional.
- **AC-S12** **(SHOULD) — DECLINED, with reasons.** `security.md` recommends scoping the resolver window
  to `created_at >= share.created_at`. **Not adopted for v1.** It is the one finding where security costs
  product behaviour, and here it costs the *primary* journey: the canonical use is "show a vendor what
  their webhook already sent me", where the evidence arrived **before** the operator decided to share it
  (`journey.md` F4-A / E3). Scoping to mint time would make the feature answer "nothing to show" exactly
  when it is most needed, and `viewer.empty.body` would become the modal outcome. Consequences accepted
  and mitigated rather than hidden: AC-S11's pre-mint disclosure becomes **mandatory** (the operator is
  told, before the link exists, that earlier and other people's requests are included); the exposure is
  bounded by `TRACE_CAP = 100` and `TRACE_TTL_HOURS = 24` **[existing — verified at
  `backend/src/config.rs:156-157`]**; and revoke is immediate (AC-37). An opt-in "only requests from now
  on" toggle is recorded as a follow-up, and `copy.md` §7.2 already holds the alternate disclosure
  wording for that day. **Ship exactly one of the two wordings — the un-scoped one in `copy.md` §4.5.**
- **AC-S13** **(MUST)** No user-supplied value reaches `href`, `src`, `srcdoc`, `style` or any `on*`
  attribute on the viewer — this **extends** AC-67, which only covered `innerHTML`. Additionally, the
  viewer's module graph **must not import `src/api/session.ts`**, asserted statically. Rationale: the
  viewer is **same-origin** with the dashboard, where the owner secret lives in `localStorage`
  **[existing — verified at `src/api/session.ts:17-19`]**, so XSS on a page built entirely from
  attacker-supplied text is owner takeover — and there is no CSP anywhere today to contain it (§2).
- **AC-S14** **(MUST)** 404 identity holds across the status line, the body **and the headers**,
  including for `HEAD` (axum auto-implements it; it is rate-limited too). Also in this AC: the share
  `url` is **never** derived from `Host` or `X-Forwarded-Host` — host-header injection would make the
  owner hand the code to an attacker's domain (asserted by AC-99).
- **AC-S15** **(SHOULD → adopted as MUST)** A **global** resolver ceiling
  (`SHARE_RATE_LIMIT_GLOBAL_PER_MIN = 1200`, §5.8) plus the documented worst-case detail egress of
  2 × `MAX_BODY_BYTES` ≈ 512 KB per response. Promoted from SHOULD because per-IP limiting alone bounds
  nothing on the nginx-less compose topology once an attacker has more than a handful of source
  addresses, and egress is the resource an operator actually pays for. Numbers and arithmetic frozen in
  AC-113.
- **AC-S16** **(MUST)** Share codes never appear in an error `detail`, a log line, a public response
  body, or the F3 bundle. Asserted together with AC-104 and AC-22.

**F7 — the persist path**

- **AC-S3** **(MUST)** **`default_mode = "echo"` must not persist the caller's un-redacted headers.**
  Verified defect: the echo payload serialises the **raw** `headers_lower` map **[existing — verified at
  `backend/src/interceptor/engine.rs:514-523`]** while `redact()` is applied only to the
  `request_headers` column **[existing — verified at `:674`]** — so from F7 onward the same row would
  read `request_headers` masked **and** `response_body` carrying the caller's real `Authorization`
  inside its `headers` sub-object, then be CSV-exported by F5 and shown to unauthenticated viewers by
  F4. Fix on the **persist path only**: the trace's `response_body` for the echo path is built from
  `redact(&headers_lower)`. The client's echo body still contains the real values, so AC-72 and the §2
  non-goal both hold. Tests: the client body contains the secret; the stored value, the CSV cell and the
  public detail all show `<redacted>`.
- **AC-S17** **(MUST)** Truncation happens at row build, **before** all three surfaces (assert the
  **stored** length), and the mid-multibyte case must still return a normal **2xx** *and* still **write
  a trace row**. This is stricter than AC-70(b) for a verified reason: today's panic is on the
  **request** task — the record is built before the `tokio::spawn` **[existing — verified at
  `backend/src/interceptor/engine.rs:646-653` built before `:699`]** — and there is no panic layer
  **[existing — verified at `backend/src/router.rs:178-193`]**, so the current failure mode is a
  **dropped connection**, not merely a lost body.
- **AC-S18** **(SHOULD → adopted as MUST, narrowly scoped)** Add `CatchPanicLayer` outermost on the app
  router, so any future panic becomes a 500 rather than a dropped connection. This requires adding the
  `catch-panic` feature to the **existing** `tower-http` dependency **[existing — verified at
  `backend/Cargo.toml:23`: `tower-http = { version = "0.6", features = ["fs", "trace"] }`]** — **a
  feature flag on a crate already in the tree, not a new dependency**, and it belongs to the security
  lane, not F7, so `architecture.md`'s "F7 touches no `Cargo.toml`" (AC-74) is unaffected. Adopted
  rather than deferred because this batch newly routes attacker/upstream-controlled bytes through a
  truncation path; AC-S17 removes the one *known* reachable panic, and this removes the failure *mode*.
  Test: a handler that panics, mounted through the same layer stack, returns 500 and the server serves
  the next request successfully. It must not change any non-panicking response's bytes (AC-72 holds).
- **AC-S19** **(MUST)** MITM/tunnel bytes are truncated to `MAX_BODY_BYTES` **before** being moved into
  the spawned task, so the 5 MB `MITM_MAX_BODY_BYTES` worst case **[existing — verified at
  `backend/src/config.rs:177`]** is not retained past the response. What crosses into the task is the
  already-truncated `Option<String>` (≤ 256 KB) — this is the memory mitigation §8-R4 asked for.
- **AC-S20** **(MUST)** No new awaited I/O on the mock path (AC-73(a)), **and** QA is told that
  `insert_trace` is **two** statements **[existing — verified at `backend/src/db.rs:61-106`]**, so
  AC-73(b)'s baseline is "unchanged", not "one" — otherwise the AC would fail against a correct
  implementation.

**F3 — import**

- **AC-S21** **(MUST)** A **pre-apply diff and an explicit confirm**, with **zero requests until
  confirm**. "Validate fully before any write" is sufficient for *integrity* but says nothing about
  *intent*: a mailed bundle can silently re-point `target_url` — validated only for scheme and host
  **[existing — verified at `backend/src/helpers.rs:130-148`]** — so every future webhook is copied
  off-box; flip `default_mode`/`cors_enabled`/`auto_crud`; and add a catch-all rule that AC-18 will
  never delete, so the operator sees no loss. Required content, in this order (`copy.md` §2.3):
  provenance (`set.config.confirm.exported`) → **the field-by-field old→new diff of changing fields
  only**, with `set.config.diff.unchangedNote` so a nine-row wall cannot hide the one row that matters →
  the `target_url` consequence when present (`set.config.diff.targetUrl.warning`, on an amber-washed row
  with a `text-caption text-warning-fg` line beneath — it is the one field that silently re-points live
  traffic, and it also changes what an existing share link publishes) → how many rules will be **added**
  and to how many existing (`set.config.confirm.rules`) → the dirty-form warning when applicable
  (`set.config.confirm.dirty` — AC-20's PATCH + re-fetch would otherwise discard unsaved edits
  silently) → `set.config.confirm.confirm` ("Apply configuration"), as `variant="primary"` **not**
  `danger`, because import is recoverable and this codebase reserves filled danger for irreversible
  deletes. Field names stay in mono, verbatim, so the operator can match them against the file they are
  holding. The old value renders `line-through text-text-tertiary` and the new `text-text-primary`; the
  arrow is `aria-hidden` because DOM order already reads old-then-new. Diff labels need the
  `set.config.diff.*` copy keys (`design.md` gap 9). `set.config.diff.none` covers a bundle that changes
  nothing.
- **AC-S22** **(MUST)** `.strict()` at **every** level — assert that a **nested** unknown key is
  rejected, not just a top-level one. Import calls only `PATCH /api/endpoints/{token}` and
  `POST /api/endpoints/{token}/rules` and no other route. The bundle can carry no token, owner, share or
  trace data (AC-22). *Rule objects* deliberately keep the non-strict `mockRuleCreateSchema` **[existing
  — verified at `src/api/schemas.ts:115-126`]** so a stray `id`/`token`/`created_at` inside a rule is
  stripped rather than fatal, matching AC-16's "unknown **top-level** keys" wording — the `.strict()`
  requirement applies to the bundle envelope and its `endpoint` object.
- **AC-S23** **(SHOULD → adopted)** Warn when an imported rule uses a request-header template tag
  **[existing — verified at `backend/src/interceptor/templating.rs:138-139`]**, via
  `set.config.confirm.headerTagWarning` ("{n} rules in this file copy request headers into their
  responses. Anything a caller sends in a header — including credentials — can come back in the body.").
  Adopted because F7 + F4 make that template a way to move a caller's secret into a now-persisted and
  publicly-shared body (S-12). Verified good news that keeps this a warning rather than a block:
  templating is a **genuine sandbox** — a closed-grammar scanner where unknown tags are returned
  verbatim **[existing — verified at `backend/src/interceptor/templating.rs:1-12`, `:158-185`]** — and
  `webhook_action` is **inert**: parsed, stored and compiled **[existing — verified at
  `backend/src/models.rs:192-196`, `backend/src/rule_cache.rs:100-116`,
  `backend/src/interceptor/matcher.rs:134`]** but with **no dispatch site anywhere in `backend/src`**,
  so an imported `webhook_action.url` cannot SSRF today. **No SSTI and no import-driven SSRF exist**;
  this is an operator-awareness warning only.

**Cross-cutting**

- **AC-S24** **(MUST)** A CSRF regression assertion, plus a positive statement in this PRD that **no
  CSRF token is needed**. HookBox uses **no cookies anywhere**: a header-only capability with
  `credentials: 'omit'` **[existing — verified at `src/api/client.ts:82-86`,
  `src/api/session.ts:8-12`]** and no CORS layer on the management plane. That is why F1's destructive
  `DELETE` needs no token, and why a share 401/404 cannot log an owner out. Test: a request without
  `Authorization` returns 401 and **nothing is deleted**; no request from any new surface sends a
  cookie. AC-2's confirm is about *accidents*, not CSRF, and is proportionate given 100-row/24 h
  ephemerality plus the Settings precedent **[existing — verified at
  `src/screens/settings.tsx:445-455`]**.
- **AC-S25** **(MUST)** F5's CSV assertions are updated to expect the AC-S3 outcome: the
  `default_mode = "echo"` row's `response_body` cell carries `<redacted>` for `authorization`, `cookie`
  and `x-owner-id` inside its `headers` sub-object, matching that row's `request_headers` cell — so the
  two cells can no longer disagree (AC-S4's invariant, expressed in the artifact). **`response_headers`
  cells stay verbatim** per the S-4 ruling (AC-56, §5.11): the CSV is a file the operator asked for, and
  only the *public* projection filters. Test: export a row served by echo mode and assert both the
  `request_headers` and `response_body` cells show `<redacted>`, while `response_headers` shows the
  header values the server actually sent, unmodified.
- **AC-S26** **(SHOULD → adopted in a scoped subset)** Baseline response headers on the viewer
  document: `Referrer-Policy: no-referrer` (AC-S5), `X-Content-Type-Options: nosniff`, and
  `X-Frame-Options: DENY` — the last making §2's "no embed support" **enforced** rather than merely
  unimplemented (`journey.md` gap 7). **The starter CSP is explicitly deferred** to a follow-up issue
  filed at BREAKDOWN: `index.html` carries an inline pre-paint theme script **[existing — verified at
  `index.html:13-30`]** that needs a nonce or hash strategy, a wrong CSP silently breaks the entire SPA,
  and getting it right is its own piece of work rather than a line in this batch. Recorded honestly:
  until that lands, AC-S13's XSS-prevention ACs are the *only* mitigation for S-15, with no
  defence-in-depth behind them.
- **AC-S27** **(SHOULD → adopted)** A real `robots.txt` with `Disallow: /s/`. There is none anywhere
  today **[existing — verified: no `robots.txt` in the repo and none in `dist/`, which holds only
  `assets/` and `index.html`]**, so nginx's `try_files` currently answers `/robots.txt` with
  `index.html`. It is created as `public/robots.txt` **[new]** so Vite copies it into `dist/` and both
  nginx and the backend's SPA handler serve it. Belt-and-braces with AC-S5's `X-Robots-Tag`, because a
  crawler that ignores one may honour the other.

---

## 5. Frozen interface contract

**Source of truth.** §5 is **lifted from `architecture.md` §2, which is authoritative**, with exactly
one delta: `security.md` AC-S1 makes `PublicRequestDetail.response_headers` a **filtered** map, frozen
in §5.5.5 and §5.11. No other security requirement changes a shape — the rest are ACs on behaviour,
ordering, headers and status codes, folded into §5.2 and §5.9 where they are contractual.

**FROZEN once locked.** Both lanes build against this section alone; neither may change a shape the
other depends on.

Conventions inherited and unchanged: flat error envelope `{"error":"<code>","detail":"<human>"}`
**[existing — verified at `backend/src/error.rs:72-74`]**; owner routes require
`Authorization: Bearer <owner_secret>` via the `OwnerId` extractor and return 401 +
`WWW-Authenticate: Bearer`, or **404-not-403** **[existing — verified at `backend/src/auth.rs:37-49`,
`:55-69`, `:114-131`]**; any `sqlx::Error` maps to **503 `store_unavailable`** **[existing — verified at
`backend/src/error.rs:97-106`]**; timestamps are RFC3339 UTC via `to_rfc3339` **[existing — verified at
`backend/src/routes/api.rs:49-60`]**; every `Option<T>` / `.nullable()` field serialises as
**present-with-`null`**, never omitted **[existing — verified at `backend/src/models.rs:1-8` and the
test at `:292-319`]**.

### 5.1 New HTTP endpoints — owner-authenticated (F4)

| # | Method | Path | Auth | Request | Success | Errors |
|---|---|---|---|---|---|---|
| 19 | POST | `/api/endpoints/{token}/shares` | Bearer owner | `ShareLinkCreate` (§5.5.1) | **201** `ShareLinkCreated` (§5.5.3) + `Cache-Control: no-store` | 401 `unauthorized` · 404 `not_found` · 422 `validation_error` · 503 `store_unavailable` |
| 20 | GET | `/api/endpoints/{token}/shares` | Bearer owner | — | **200** `ShareLink[]` (§5.5.2) | 401 · 404 · 503 |
| 21 | DELETE | `/api/endpoints/{token}/shares/{id}` | Bearer owner | — | **204**, no body | 401 · 404 `not_found` · 503 |

**`{id}` is a positive integer** (`share_links.id`), **not** the share code (AC-26, `architecture.md`
D10). A non-integer `{id}` is rejected by axum's `Path<(String, i64)>` extraction before the handler
runs, exactly like the existing `/rules/:id` routes **[existing — verified at
`backend/src/routes/api.rs:632-645`]**.

Exact semantics, in handler order:

**#19 POST**
1. `OwnerId` extractor → **401** on missing/malformed/unknown Bearer (AC-29).
2. `assert_owns_endpoint(pool, token, owner_id)` → **404** for an unknown token *and* for another
   owner's token (AC-28) **[existing — verified at `backend/src/auth.rs:55-69`]**.
3. Reject a tombstoned endpoint: `SELECT gone_at FROM endpoints WHERE token = ?`; non-null → **404**
   `not_found` ("Endpoint not found.") — AC-100.
4. Validate `label`: absent/`None` is allowed; a present label is trimmed, and
   `label.chars().count() > 80` → **422** `validation_error` ("label must be at most 80 characters.").
   A label that trims to empty is stored as `NULL`.
5. Enforce the cap: `SELECT COUNT(*) FROM share_links WHERE token = ? AND revoked_at IS NULL`;
   `>= cfg.share_max_per_endpoint` → **422** `validation_error` (AC-27).
6. `let code = ids::gen_share_code(cfg.share_code_bytes);` →
   `INSERT INTO share_links (code_hash, token, label) VALUES (?, ?, ?)` with
   `code_hash = ids::hash_secret(&code)` **[existing — verified at `backend/src/ids.rs:57-60`]**;
   `RETURNING id, created_at`.
7. **201** with `ShareLinkCreated { id, code, url, label, created_at, last_used_at: null }`. **`code`
   and `url` appear only here, in only this response, and are never re-derivable** (AC-104).

**#20 GET** — `OwnerId` → `assert_owns_endpoint` → `SELECT id, label, created_at, last_used_at FROM
share_links WHERE token = ? AND revoked_at IS NULL ORDER BY created_at DESC, id DESC` → **200**
`ShareLink[]`. Revoked links never appear (AC-25). **No `code`, no `url`, no code prefix.**

**#21 DELETE** — `OwnerId` → `assert_owns_endpoint` → `UPDATE share_links SET revoked_at =
datetime('now') WHERE id = ? AND token = ? AND revoked_at IS NULL`. `rows_affected() == 0` → **404**
`not_found` ("Share link not found.") — covering an unknown id, an id on another endpoint, and an
already-revoked id, indistinguishably, and idempotent from the caller's point of view (the client treats
that 404 as success per AC-95). Otherwise **204** with no body, matching `delete_rule` **[existing —
verified at `backend/src/routes/api.rs:812`]**. **Soft revoke only — never a row delete**, so a revoked
`code_hash` can never be re-minted and the `UNIQUE` constraint keeps enforcing global uniqueness against
revoked codes too.

### 5.2 New HTTP endpoints — PUBLIC, no authentication (F4)

Mounted under `/api/` so nginx's existing `location /api/` proxy reaches them (§5.9) and so
`resolve_plane` classifies them as `Plane::Api` **[existing — verified at
`backend/src/planes.rs:162-164`]**. Together with `POST /api/session` **[existing — verified at
`backend/src/routes/api.rs:983`]** these are the **only** unauthenticated routes in HookBox.

| # | Method | Path | Auth | Request | Success | Errors |
|---|---|---|---|---|---|---|
| 22 | GET | `/api/share/{code}/requests?limit&offset` | **none** | `limit` 1..200 (default 50), `offset` ≥ 0 | **200** `PublicShareFeed` (§5.5.4) | **404** `not_found` (unknown / revoked / tombstoned — byte-identical) · 422 `validation_error` · 429 `rate_limited` + `Retry-After` · 503 `store_unavailable` |
| 23 | GET | `/api/share/{code}/requests/{id}` | **none** | — | **200** `PublicRequestDetail` (§5.5.5) | **404** (as above, plus an unknown request id, plus a request id not belonging to this share's endpoint) · 429 · 503 |

Every **handler-produced** response carries `Cache-Control: no-store` (AC-37, narrowed per
`architecture.md` D14). No other verb is routed for these two paths, so axum returns **405** (AC-39)
with an empty body, no `no-store` and not the flat error envelope — accepted, consistent with every
other route in the app, and it discloses nothing. `HEAD` is auto-implemented and is rate-limited
(AC-S14).

**The single 404 constructor.** AC-36 requires byte-identical bodies, status lines **and headers**. That
is guaranteed *structurally* by funnelling every negative outcome through one function:

```rust
/// The ONLY 404 the public resolver may emit. Unknown code, revoked code,
/// tombstoned endpoint, unknown request id and cross-endpoint request id all
/// return this exact value, so a scanner learns nothing from the difference.
fn share_not_found() -> ApiError {
    ApiError::not_found("This share link is not available.")
        .with_header("cache-control", "no-store")
}
```

**Frozen check order for #22 — the order is load-bearing:**

1. **Rate limit** — `state.limiter.check(&format!("share:{ip}"), cfg.share_rate_limit_per_min, 60)`
   using the existing token bucket **[existing — verified at `backend/src/limiter.rs:80-128`]** and the
   existing proxy-aware IP resolver **[existing — verified at `backend/src/routes/api.rs:273-299`]**,
   plus the global ceiling check. Over either limit → **429** `rate_limited` + `Retry-After`. The
   `share:` prefix is **required** so the key can never collide with or evict the mock plane's
   `rl:<token>` keys **[existing — verified at `backend/src/limiter.rs:71-76`]** (AC-S7); `limiter.rs`
   itself needs no change. **This step runs before any DB read.**
2. **Parameter validation** — `limit` in `1..=200`, `offset >= 0`, else **422** `validation_error` with
   the same wording as the owner route **[existing — verified at
   `backend/src/routes/api.rs:829-836`]**.
3. **Code shape** — `is_share_code_shape(&code)`; false → `share_not_found()` **with no DB read**.
4. **Code resolution** — one statement (below); any miss → `share_not_found()`.
5. **200** + `no-store`.
6. **Fire-and-forget** `last_used_at` touch, after the response value is built.

> **Why step 2 precedes step 4 (AC-101).** If `limit` were validated *after* resolving the code, then
> `?limit=999` would return 422 for a live code and 404 for a dead one — a boolean existence oracle that
> defeats AC-36. Validating parameters first makes the response depend only on the parameters. The
> existing owner route already orders it this way **[verified at
> `backend/src/routes/api.rs:829-837`]**; a test asserts `?limit=999` returns **422 for both** a valid
> and an invalid code.

**The resolution statement** (one round trip; joins liveness so a tombstone is indistinguishable —
AC-S9):

```sql
SELECT s.id, s.token, e.name, e.created_at AS endpoint_created_at, e.request_count
  FROM share_links s
  JOIN endpoints  e ON e.token = s.token
 WHERE s.code_hash = ?          -- sha256(code) — never the code itself
   AND s.revoked_at IS NULL
   AND e.gone_at   IS NULL
```

**Trace list** (after resolution), reusing the owner route's shape and ordering **[existing — verified at
`backend/src/routes/api.rs:838-845`]**:

```sql
SELECT * FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ? OFFSET ?
```

**#23 detail** — steps 1, 3, 4 as above (no `limit`/`offset`, so no step 2), then:

```sql
SELECT * FROM request_logs WHERE id = ? AND token = ?   -- token from the resolved share row
```

Miss → `share_not_found()`. **The `AND token = ?` is the whole of AC-35** — cross-endpoint trace
enumeration is impossible because the id is scoped by the share's endpoint inside the same statement.

**`last_used_at`, made safe (AC-S10).** Best-effort, coalesced, and off the response path, so an
unauthenticated GET can never contend on the single-writer WAL lock in front of a viewer's response:

```rust
// after the 200 value is built, mirroring engine::spawn_trace's fire-and-forget shape
let pool = state.pool.clone();
tokio::spawn(async move {
    let _ = sqlx::query(
        "UPDATE share_links SET last_used_at = datetime('now')
          WHERE id = ? AND (last_used_at IS NULL
                            OR last_used_at < datetime('now','-60 seconds'))",
    ).bind(share_id).execute(&pool).await;
});
```

At most one write per link per minute regardless of poll rate. This touches `share_links` **only**;
AC-39's asserted-unchanged tables (`request_logs`, `mock_rules`, `endpoints`, `endpoint_state`,
`crud_collections`) are untouched, so those row-count assertions hold verbatim.

**Code shape check** — hand-rolled, because there is **no `regex` dependency** **[existing — verified at
`backend/Cargo.toml`]**; the codebase precedent is `is_safe_key` **[existing — verified at
`backend/src/helpers.rs:118-125`]**:

```rust
/// base64url-no-pad shape gate. `SHARE_CODE_BYTES` is env-tunable, so accept the
/// whole plausible band rather than one exact length: 32 chars = the 24-byte
/// default, 64 chars covers up to 48 bytes.
fn is_share_code_shape(code: &str) -> bool {
    let n = code.len();                 // ASCII-only charset ⇒ len() == char count
    (32..=64).contains(&n)
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
```

### 5.3 Changed behavior on an existing endpoint (F4 only)

`DELETE /api/endpoints/{token}` **[existing — verified at `backend/src/routes/api.rs:506-532`]** —
**request and response shapes are unchanged** (200 `Message`). One statement is added, in the same
handler, immediately after the `crud_collections` delete at `:522-525` and **before** the `gone_at`
update at `:526-529` (ordering is AC-S9's requirement):

```sql
UPDATE share_links SET revoked_at = datetime('now') WHERE token = ? AND revoked_at IS NULL
```

That satisfies AC-30 for the *tombstone* window; the resolver's own `gone_at` check (§5.2) is the
belt-and-braces half, because `ON DELETE CASCADE` never fires for a tombstone. The eventual hard delete
by the retention sweep **[existing — verified at `backend/src/tasks/sweep.rs`]** removes the rows via
`ON DELETE CASCADE`, which works only because `foreign_keys` is ON **[existing — verified at
`backend/src/db.rs:21`]**.

`effective_client_ip` **[existing — verified at `backend/src/routes/api.rs:273-299`]** changes visibility
from private to `pub(crate)` — **no behaviour change**, and its five existing unit tests **[verified at
`:1018-1061`]** stay where they are. **No other existing route changes for any feature.**

### 5.4 DB schema — one new migration (F4 only)

`backend/migrations/0002_share_links.sql` **[new]**. Additive only; `0001_init.sql` is byte-untouched so
its recorded `sqlx` checksum stays valid **[existing — verified at `backend/src/db.rs:31-34`:
`sqlx::migrate!("./migrations")`]**.

```sql
-- migrations/0002_share_links.sql -------------------------------------------
-- F4 public read-only share links. The code is a URL-borne bearer credential:
-- stored ONLY as sha256 (mirroring owners.secret_hash), surfaced in plaintext
-- exactly once in the 201 response, and addressed thereafter by the non-secret
-- integer id so no code ever lands in an owner-route URL (and therefore never
-- in nginx's access log for `location /api/`).

CREATE TABLE share_links (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,  -- non-secret handle: list + revoke
    code_hash    TEXT NOT NULL UNIQUE,               -- sha256(code) hex, 64 chars; the lookup key
    token        TEXT NOT NULL,                      -- owning endpoint
    label        TEXT,                               -- optional operator note, <= 80 chars, NULL when blank
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at   TEXT,                               -- non-null => dead; never un-set, never deleted
    last_used_at TEXT,                               -- best-effort, coalesced to >= 60s granularity
    FOREIGN KEY (token) REFERENCES endpoints(token) ON DELETE CASCADE
);

-- Public resolver: exact-match on the UNIQUE code_hash index (one B-tree probe).
-- Owner list + the active-count cap check: covered by this composite index.
CREATE INDEX idx_share_token_active ON share_links(token, revoked_at, created_at DESC);
```

**Storage decision — HASHED, settled (§0 item 2).** `code_hash = sha256(code)` plus a non-secret integer
`id`. `architecture.md` §3.2 weighed five options; the decisive fact is D10 (a plaintext code in the
owner revoke URL would be written to nginx's access log in cleartext, and no nginx prefix can exclude
that path without unlogging every endpoint route), not at-rest paranoia. `security.md` §4's note
explicitly designates this design authoritative over its own standalone plaintext analysis and says
**do not re-open R3 in favour of plaintext**. Secondary benefits: `sha256` of a 192-bit CSPRNG value is
not brute-forcible, so the lookup is still one exact-match probe on a `UNIQUE` index; it matches the
codebase's existing precedent for bearer credentials (`owners.secret_hash` **[existing — verified at
`backend/migrations/0001_init.sql:11`, `backend/src/auth.rs:42-48`]**) and reuses `ids::hash_secret`
verbatim; and a leaked backup, replica or `.db` file yields **no working URL**. The cost — "you cannot
re-copy an existing link" — is covered by revoke-and-re-mint within a `SHARE_MAX_PER_ENDPOINT` of 10,
and is what makes AC-24's shown-once panel necessary.

**Retention.** Revoked rows are never deleted and never re-minted. A tombstoned endpoint's rows vanish
via `ON DELETE CASCADE` when the sweep hard-deletes at `GONE_TTL_HOURS` **[existing — verified at
`backend/src/tasks/sweep.rs`]**. `share_links` adds ≤ 10 rows × ~200 bytes per endpoint — negligible.

**No other table, column or index changes. F1, F2, F3, F5, F6 and F7 require zero schema change** —
`request_logs.response_body TEXT` already exists **[existing — verified at
`backend/migrations/0001_init.sql:65`]** and is already bound on insert **[existing — verified at
`backend/src/db.rs:85`]**; F7 changes only the *value* written (§5.10).

### 5.5 Shared data models

Rust: `backend/src/models.rs` **[existing — verified]**. TS: `src/api/schemas.ts` **[existing —
verified]**. Every optional field is **present-with-`null`**, never omitted.

**5.5.1 `ShareLinkCreate`** (request body for #19):

```
Rust:  #[derive(Deserialize)] pub struct ShareLinkCreate { #[serde(default)] pub label: Option<String> }
TS:    shareLinkCreateSchema = z.object({ label: z.string().max(80).nullable().optional() })
Wire:  { "label": "Acme vendor" }  |  { "label": null }  |  {}
```

**5.5.2 `ShareLink`** (owner list item — **carries no secret material**):

```
{ id: number, label: string | null, created_at: string, last_used_at: string | null }
```

**5.5.3 `ShareLinkCreated`** (201 body for #19 — the **only** place `code`/`url` ever appear):

```
{ id: number, code: string, url: string,
  label: string | null, created_at: string, last_used_at: null }
```

`url` is built by a new `share_url(state, code)` helper mirroring how `path_url` is built **[existing —
verified at `backend/src/routes/api.rs:42-44`]**:

```rust
fn share_url(state: &AppState, code: &str) -> String {
    format!("{}/s/{code}", state.cfg.public_base_url)   // blank base ⇒ "/s/{code}", SPA absolutizes
}
```

> **`share_url` must never use `mock_url`'s wildcard form, and never `Host`/`X-Forwarded-Host`**
> (AC-99, AC-S14). `mock_url` returns `https://{token}.{mock_domain}` in wildcard mode **[existing —
> verified at `backend/src/routes/api.rs:32-40`]**, and `resolve_plane` sends **everything** on a mock
> host to the mock plane **[existing — verified at `backend/src/planes.rs:137-148`]** — a share URL on
> the mock host would be swallowed by the interceptor, 404 as an unknown mock path, and persist the code
> into `request_logs.path`. The share URL is always on the **app** origin. Deriving it from a request
> header would let host-header injection make the owner hand the code to an attacker's domain. The SPA
> absolutizes a relative value with the existing helper **[existing — verified at
> `src/lib/url.ts:8-11`]**.

**5.5.4 `PublicShareFeed`** (200 body for #22):

```
{ endpoint: { name: string | null, created_at: string, request_count: number },
  requests: PublicRequestSummary[] }
```

`endpoint.name` is operator-authored text rendered to strangers — **intentionally disclosed**
(`security.md` §4(12)), as a text node only, never as the `<h1>` or `document.title` (AC-107).

**5.5.5 Public trace projections** — reduced from the owner shapes **[existing — verified at
`src/api/schemas.ts:149-172`, `backend/src/models.rs:244-269`]**:

```
PublicRequestSummary = { id: number, method: string, path: string, status_code: number,
                         served_by: ServedBy, duration_ms: number, timestamp: string }

PublicRequestDetail  = PublicRequestSummary & {
                         request_headers:  Record<string,string>,   // as stored (already redacted)
                         query_params:     Record<string,string>,
                         request_body:     string | null,
                         response_headers: Record<string,string>,   // FILTERED — see §5.11 / AC-S1
                         response_body:    string | null }
```

**Complete omission list vs. the owner shapes (AC-34/AC-44a): `token`, `matched_rule_id`,
`overhead_ms`, `trace`, `state_snapshot`.** The five body/header fields above are **present keys**
(present-with-`null` when the column is `NULL`), so a future narrowing is a deliberate contract change,
not a silent regression.

**`response_headers` is a FILTERED map — this is the one security-driven contract delta (AC-S1).** The
omission list above governs *projection keys*; §5.11's two constants govern *keys inside
`response_headers`*. Without the filter, AC-34 would be **false**, because `identified()` stamps
`x-hookbox-endpoint: <token>` and `x-hookbox-rule-id` onto every mock response **[existing — verified at
`backend/src/interceptor/engine.rs:584-588`]** and `spawn_trace` persists the map verbatim **[existing —
verified at `:636-644`]**. `response_headers` **stays in the projection *because* the filter makes AC-34
true** (`security.md` §4(2)).

**Implementation requirement — how AC-34 becomes structural rather than aspirational (AC-102).** The
public structs are **standalone `#[derive(Serialize)]` structs** in `backend/src/models.rs`, built
field-by-field from the row in `share.rs`. They must **not** be produced by `#[serde(skip)]`-ing fields
off `RequestDetail`, and must not `#[serde(flatten)]` an owner struct — either would make a future field
added to the owner shape leak into the public projection by default.

**5.5.6 `ConfigBundle`** — the F3 **file** format. A client/file contract; never a request body; no Rust
counterpart.

```
{ hookbox_config_version: 1,                 // z.literal(1)
  exported_at: string,                       // RFC3339 UTC, from new Date().toISOString()
  endpoint: {                                // ALL NINE REQUIRED (AC-13), .strict()
    name:               string | null,
    auto_crud:          boolean,
    target_url:         string | null,
    default_mode:       "mock_404" | "echo",
    latency_ms:         number,              // int
    rate_limit_per_min: number,              // int
    chaos_pct:          number,              // int
    chaos_mode:         "error" | "dropout",
    cors_enabled:       boolean },
  rules: MockRuleCreate[] }                  // mockRuleCreateSchema, non-strict (unknown keys stripped)
```

`configBundleSchema` is `.strict()` at the **top level** (rejecting any key other than the three above)
**and** on `endpoint` (rejecting `token`, `mock_url`, `path_url`, `created_at`, `last_hit`,
`request_count`, `tunnel_active` — AC-13), and a **nested** unknown key must be rejected too (AC-S22).
Import limits enforced **before any network write** (AC-16): file size ≤ **5 MB** (checked on
`File.size` before `text()`), `rules.length` ≤ **200**, `hookbox_config_version === 1`. Rule objects
reuse `mockRuleCreateSchema` **[existing — verified at `src/api/schemas.ts:115-126`]** so a stray
`id`/`token`/`created_at` **inside a rule** is stripped rather than fatal, matching AC-16's "unknown
**top-level** keys" wording.

A compile-time guard keeps the bundle and the API in lockstep:

```ts
// src/lib/config-bundle.ts
const _assignable: EndpointConfigPatch = {} as ConfigBundle['endpoint']  // fails typecheck on drift
```

**5.5.7 Default catch-all rule payload (F6)** — the exact frozen `MockRuleCreate` body, **as amended by
AC-125** (`copy.md` owns the two user-visible strings):

```json
{
  "name": "Catch-all (default)",
  "priority": 1000,
  "enabled": true,
  "match": { "method": "ANY", "path": "/*", "headers": {}, "query": {},
             "body_conditions": [], "state_requirements": [] },
  "response": { "status_code": 200, "headers": {}, "content_type": "application/json",
                "body_template": "{\n  \"ok\": true,\n  \"hookbox\": \"default catch-all\",\n  \"message\": \"Edit this rule in HookBox to return your own response.\"\n}" },
  "state_writes": [], "latency_ms": null, "rate_limit_per_min": null,
  "chaos_mode": null, "webhook_action": null
}
```

Backend confirmation for AC-63 — **no matcher or engine change is needed**: `"ANY"` already matches
every method **[existing — verified at `backend/src/interceptor/matcher.rs:182`]**, `/*` already compiles
to a pure catch-all **[existing — verified at `backend/src/interceptor/matcher.rs:53`]**, those are
already the serde defaults **[existing — verified at `backend/src/models.rs:135-138`]**,
`priority = 1000` is inside the accepted `0..=100000` band **[existing — verified at
`backend/src/routes/api.rs:561-565`]**, and the list order is `ORDER BY priority, id` **[existing —
verified at `backend/src/routes/api.rs:542`]** so the catch-all sorts last.

### 5.6 CSV artifact format (F5) — frozen

- Media type `text/csv;charset=utf-8`, UTF-8, **no BOM**, **CRLF** record separator, RFC 4180 quoting,
  **including a trailing CRLF after the final record** (frozen so fixtures are byte-stable).
- Filename `hookbox-requests-<token>-<YYYYMMDDTHHMMSSZ>.csv`, where the stamp is
  `new Date().toISOString()` with `-`, `:` and the `.mmm` fraction removed
  (`2026-08-07T11:22:33.444Z` → `20260807T112233Z`).
- Header row, then one row per **snapshot row, newest first** (AC-51, AC-115), exactly 10 columns:

```
timestamp,method,path,status_code,served_by,duration_ms,request_headers,request_body,response_headers,response_body
```

**Cell derivation — frozen, so a failed detail fetch can never shift a column:**

| Column | Source | On detail failure |
|---|---|---|
| `timestamp` | `row.timestamp` (the feed `RequestSummary`) | unchanged |
| `method` | `row.method` | unchanged |
| `path` | `row.path` | unchanged |
| `status_code` | `row.status_code` — bare integer, unquoted | unchanged |
| `served_by` | `row.served_by` | unchanged |
| `duration_ms` | `row.duration_ms` — bare integer, unquoted | unchanged |
| `request_headers` | `JSON.stringify(detail.request_headers)` — compact object | `pending` / `unavailable` |
| `request_body` | `detail.request_body ?? ''` | `pending` / `unavailable` |
| `response_headers` | `JSON.stringify(detail.response_headers)` — compact object | `pending` / `unavailable` |
| `response_body` | `detail.response_body ?? ''` | `pending` / `unavailable` |

The six summary columns always come from the feed row — **never** from the detail response, even when the
detail succeeds. That makes a row's summary cells independent of the detail fetch and keeps AC-52
trivially true.

- **Sentinels (AC-52):** `pending` when the detail fetch returned **404**; `unavailable` for any other
  per-row failure (5xx, network, `contract_mismatch`, or AC-119's timeout). An **empty cell** is
  semantically distinct: the detail fetch **succeeded** and the stored column is `NULL` (AC-56a/AC-69).
- **Accepted ambiguity (AC-121):** a request or response body whose literal text is `pending` or
  `unavailable` is indistinguishable from the sentinel. Documented here rather than worked around,
  because the alternatives would break the frozen 10-column format. **No comment line is added to the
  file** — RFC 4180 has none (resolving `security.md` OQ-S4); the explanation is UI-side.
- **Escaping, frozen order: formula guard, then quote.** (1) if the cell's first character is `=`, `+`,
  `-`, `@`, TAB (U+0009) or CR (U+000D), prefix a single `'`, applied uniformly to all ten columns (it
  can never fire on the two integer columns); (2) if the guarded value contains `,`, `"`, CR or LF, wrap
  in `"` and double every embedded `"`. Per AC-55, `=cmd|' /c calc'!A1` becomes `'=cmd|' /c calc'!A1`
  **unquoted**.
- **Header cells are emitted exactly as the server persisted them:** `request_headers` pre-redacted,
  `response_headers` **verbatim** (AC-56 — `security.md`'s S-4 ruling accepts verbatim here; only the
  public projection filters). The exporter performs **no** client-side redaction, ever.
- `request_body` / `response_body` carry the **persisted** body content — both real (F7), both truncated
  at `MAX_BODY_BYTES` with **no** marker (AC-70), both an **empty cell** when the stored column is
  `NULL`. The echo path's `response_body` carries `<redacted>` for the redacted request headers
  (AC-S3/AC-S25).

**Fetch mechanism (AC-47/AC-48).** Up to `FEED_CAP` = 100 rows **[existing — verified at
`src/feed/use-feed.ts:42`]**, `GET /api/requests/{id}` per row **[existing — verified at
`src/api/client.ts:217-219`]**, **exactly 4 in flight**, one shared `AbortController`:

```ts
// src/lib/request-export.ts  [new]
export const EXPORT_CONCURRENCY = 4
export async function fetchDetails(
  rows: readonly RequestSummary[],
  signal: AbortSignal,
  onProgress: (done: number, total: number) => void,
): Promise<ReadonlyArray<DetailCell>>   // index-aligned with `rows`
```

A fixed pool of `EXPORT_CONCURRENCY` workers pulls from a shared cursor; results are written into a
pre-sized array **by index**, so completion order never affects row order. `onProgress` fires once per
settled row. On abort: no file, no object URL. On success: `new Blob([csv], { type:
'text/csv;charset=utf-8' })` → `URL.createObjectURL` → programmatic anchor click → `URL.revokeObjectURL`
in a `finally` (AC-49).

### 5.7 WebSocket / SSE messages

**No changes.** No new event type; the existing `hello` / `new_request` / `state_changed` /
`endpoint_updated` envelope is untouched **[existing — verified at `src/feed/events.ts`,
`backend/src/routes/feed.rs`]**. **F7 does not widen it:** the `new_request` summary is built from
`summary_base`, which contains no body field **[existing — verified at
`backend/src/interceptor/engine.rs:692-697`]**, and stays that way (AC-75). F4's viewer opens **no**
WebSocket and **no** EventSource (AC-45), so `/ws/{token}` and `/sse/{token}` remain owner-gated by
`?cap=` **[existing — verified at `backend/src/routes/feed.rs:46-59`]**.

### 5.8 New configuration (env, safe defaults)

Added to `Config` following the established never-crash-on-a-missing-var rule **[existing — verified at
`backend/src/config.rs:10-41`, `:110-189`]**:

| Field | Env var | Default | Meaning |
|---|---|---|---|
| `share_code_bytes: usize` | `SHARE_CODE_BYTES` | `24` | CSPRNG bytes per code → 32-char base64url → **192 bits** (AC-31). **Clamped to ≥ 16 at load** (AC-103), because `int_env` accepts `1` today **[existing — verified at `backend/src/config.rs:17-22`]** |
| `share_max_per_endpoint: i64` | `SHARE_MAX_PER_ENDPOINT` | `10` | Max **active** links per endpoint (AC-27) |
| `share_rate_limit_per_min: i64` | `SHARE_RATE_LIMIT_PER_MIN` | **`120`** | Per-IP limit on the public resolver (AC-38, AC-113) — **raised from the draft's 60**, arithmetic in AC-113 |
| `share_rate_limit_global_per_min: i64` | `SHARE_RATE_LIMIT_GLOBAL_PER_MIN` | **`1200`** | Instance-wide ceiling on the public resolver (AC-S15, AC-113) — **new** |

**F7 adds no configuration.** It reuses `max_body_bytes` **[existing — verified at
`backend/src/config.rs:170`]** as the persisted cap for **both** body columns, and `mitm_max_body_bytes`
**[existing — verified at `:177`]** continues to bound what a MITM response can contain before
truncation. **No feature flag for capture** — a flag would make `response_body = null` ambiguous
("empty" vs "capture off") and fork the CSV/public-detail contract.

**No `SHARE_LINKS_ENABLED` flag** (§2, confirming `architecture.md` §2.9). An operator who wants no
public surface blocks it in nginx: `location /api/share/ { return 404; }` and
`location /s/ { return 404; }`. A server-side flag would have to be discoverable by the SPA to hide the
Share control, which would require a shape change to `GET /api/endpoints/{token}` — explicitly out of
scope.

### 5.9 Deploy surface (F4) — grounded constraint, not a preference

`deploy/nginx.conf` **[existing — verified at `deploy/nginx.conf:9-71`]** proxies **only** `/api/`,
`= /healthz`, `/ws/`, `/sse/` and `/e/`; everything else is
`location / { try_files $uri $uri/ /index.html; }`. Four consequences, all contractual:

1. The public resolver **must** live under `/api/` (§5.2) — any other prefix is swallowed by the SPA
   fallback and never reaches the backend.
2. `/s/:code` needs **no** proxy rule: it falls through `try_files` to `index.html`, and the plane
   resolver already routes unknown app-host paths to the UI plane **[existing — verified at
   `backend/src/planes.rs:178-179`]**, where `serve_spa` falls back to `index.html` **[existing —
   verified at `backend/src/routes/spa.rs:37-60`]**. So it also works without nginx (dev / `cargo run`
   / the compose topology).
3. **Share codes must never reach an access log** (AC-S6). Two new locations, both *longer* prefixes
   than the existing ones so nginx's longest-prefix rule selects them:

```nginx
# The share code is a bearer credential in the URL path — same class of leak as
# ?cap= on /ws/ and /sse/ (commit 47a267c). Longer prefix than `location /api/`,
# so nginx picks this one. X-Real-IP MUST stay: the public resolver's per-IP
# limiter depends on it (backend/src/routes/api.rs:273-299 only trusts it from a
# loopback peer, which nginx always is).
location /api/share/ {
    proxy_pass http://127.0.0.1:{{APP_PORT}}/api/share/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    access_log off;
}

# The viewer page itself: SPA fallback, but the code is in the path.
location /s/ {
    try_files $uri $uri/ /index.html;
    add_header Referrer-Policy "no-referrer" always;
    add_header X-Robots-Tag "noindex, nofollow" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    access_log off;
}
```

   The app's own logging must never log the code either: `share.rs` must not `tracing::*` the code or
   the full path; it logs the resolved `share_links.id`. **This is exactly why revoke is by `id`**
   (AC-26) — the owner route stays under the logging `location /api/`, and now carries no secret.
4. **`access_log off` is necessary but not sufficient** (AC-S5). Static assets fetched *after* the
   viewer document load under `location /` carry `Referer: https://host/s/<CODE>`, and nginx's default
   `combined` format logs the referer. Hence `Referrer-Policy: no-referrer` **both** as the
   `add_header` above **and** as a `<meta name="referrer" content="no-referrer">` in `index.html`, so
   deployments without nginx are covered too. Plus `public/robots.txt` **[new]** with `Disallow: /s/`
   (AC-S27).

### 5.10 Response-body capture (F7) — a *value* change, not a shape change

Nothing in the FE↔BE boundary changes shape. What changes is one field's realisable values, which is
still a contract statement and is therefore frozen here:

| Surface | Field | Before | After F7 |
|---|---|---|---|
| `GET /api/requests/{id}` **[existing — verified at `backend/src/routes/api.rs:850-874`]** | `response_body: string \| null` **[existing — verified at `backend/src/models.rs:266`, `src/api/schemas.ts:169`]** | always `null` | the captured body, or `null` for an empty-bodied response |
| `GET /api/share/{code}/requests/{id}` (§5.2 #23) | `response_body: string \| null` (§5.5.5) | — | same values |
| F5 CSV `response_body` (§5.6) | cell | always empty | the captured body; empty **only** when the stored value is `NULL` |

Frozen semantics of the persisted value (both body columns behave identically):

1. `NULL` ⟺ the body was zero-length (AC-69). **Never an empty string.**
2. Cut to at most `MAX_BODY_BYTES` **bytes of the decoded string**, floored to a UTF-8 character
   boundary, with **no marker and no flag** (AC-70) — a consumer cannot distinguish "exactly cap" from
   "truncated at cap" (accepted, §8-R4; surfaced as a hedged UI caption by AC-136).
3. Lossy UTF-8 decoding of the wire bytes; the TEXT column always holds valid UTF-8 (AC-71). The client
   still receives the original raw bytes (AC-72).
4. **No redaction of either body — with exactly one exception: the `default_mode = "echo"` payload's
   `headers` sub-object, which is built from `redact(&headers_lower)` on the persist path only**
   (AC-S3). Everything else is unchanged from today. Only *request headers* are otherwise redacted.
5. Rows written before F7 keep `response_body = NULL` forever; no back-fill (§2).

**No new endpoint, model, table, column, migration, env var, WS event or TS schema edit for F7.**
`Cargo.toml`, `package.json` and `src/api/schemas.ts` are untouched by F7.

### 5.11 The redaction table — frozen across all three surfaces

`security.md` §4(1)–(3) requires this be frozen rather than inferred. Two named constants live in
`backend/src/routes/share.rs` **[new]** (the public surface owns its own filter, so the whole
unauthenticated projection is auditable in one file):

```rust
/// Dropped entirely from the PUBLIC projection: internal markers that identify
/// the endpoint or the matched rule. Prefix rule, not an enumeration, so a
/// future x-hookbox-* header cannot leak by omission. (AC-S1)
const PUBLIC_RESPONSE_HEADER_DROP_PREFIX: &str = "x-hookbox-";

/// Value replaced with "<redacted>" in the PUBLIC projection: credential-bearing
/// response headers. Key is KEPT so the viewer can see one was sent. (AC-S1)
const PUBLIC_RESPONSE_HEADER_REDACT: [&str; 5] = [
    "set-cookie", "set-cookie2", "authorization",
    "proxy-authenticate", "www-authenticate",
];
```

| Surface | Request headers | Response headers | Request body | Response body |
|---|---|---|---|---|
| **Stored** (`request_logs`) | **redacted** — `REDACT_HEADERS = ["authorization","cookie","x-owner-id"]` → `<redacted>` **[existing — verified at `backend/src/helpers.rs:32-34`, `:43`, applied at `backend/src/interceptor/engine.rs:674`]** | **verbatim** **[existing — verified at `backend/src/interceptor/engine.rs:636-644`]** | verbatim | verbatim, **except** the echo payload's `headers` sub-object (AC-S3) |
| **Owner** `GET /api/requests/{id}` | as stored | **as stored — verbatim (ACCEPTED)** | as stored | as stored |
| **F5 CSV** | as stored | **as stored — verbatim (ACCEPTED)** | as stored | as stored |
| **Public** `GET /api/share/{code}/requests/{id}` | as stored (already redacted) | **FILTERED** — drop `x-hookbox-*`, mask the five above (AC-S1) | as stored | as stored |

**Why asymmetric, stated once so nobody re-litigates it** (`security.md` S-4): `redact()` exists to keep
HookBox's *own* capability out of traces — it is a capability scrubber, not a secret scrubber. For the
owner Inspector and the CSV, verbatim response headers are the **point** (the operator is proxying to a
backend they own, and "why didn't my cookie stick?" is what an inspector is for); redacting on the
persist path would also silently change the meaning of already-stored rows. For F4 the audience becomes
"anyone with a URL", and an upstream `Set-Cookie` handed to a stranger is a session-hijack primitive on
a **third-party** system with HookBox as the delivery mechanism. One deny-list at the public projection
fixes S-1, S-2 and S-4 together, changes no shape, and is narrower than redacting at rest.

**Residual, accepted and recorded:** the database holds upstream cookies for ≤ 24 h
(`TRACE_TTL_HOURS` **[existing — verified at `backend/src/config.rs:157`]**). Partial comfort verified:
MITM already strips upstream `Set-Cookie` **before** the response is built —
`STRIP_RESPONSE_HEADERS` includes `set-cookie` and `proxy-authenticate` **[existing — verified at
`backend/src/interceptor/proxy.rs:25-44`]** — so the un-redacted-at-rest exposure is narrower than it
first appears: it is **tunnel replies, rule-authored response headers, and HookBox's own
`x-hookbox-*` set**.

---

## 6. Affected files  *(all [existing — verified])*

**Frontend**

| File | Change | Feature |
|---|---|---|
| `src/screens/dashboard.tsx` **[`:289-351`]** | `FeedPane` header action group **[`:325-350`]** gains the overflow `Menu` (Clear all + Export CSV) and the non-modal export strip between header and list; `DashboardLoaded` wires `clearRows` and passes `token` down; selection + `liveIds` reset on clear **[`:203`, `:219-227`]** | F1, F5 (AC-1, AC-46, AC-78, AC-117) |
| `src/feed/use-feed.ts` **[`:55-66`, `:71`, `:74`, `:77`]** | add `clearRows(): void` — `setRows([])`, `buffer.current = []`, `setNewCount(0)`; one function covering AC-4 and AC-5 including the paused case | F1 |
| `src/components/hookbox/app-shell.tsx` **[`:98-118`]** | delete the `dash.pathUrl.label` `UrlChip` **[`:101`]**; add the Share ghost button + count badge + `<ShareDialog>` mount, first in the right action cluster **[`:119-143`]**; fix the module doc-comment **[`:4-5`]**; dead-class fixes at `:104` and `:163` (AC-130(b)) | F2, F4 |
| `src/screens/settings.tsx` **[`:149-482`]** | new Configuration `Section` **[pattern at `:707-720`]** after Save, before Retention **[`:396-419`]**; `peer`-based file-input label; `Progress`; the AC-S21 diff block; persistent partial-failure alert; swap the local `ConfirmDialog` **[`:659-705`]** for the extracted one; **AC-89's form-truth fix at `:138-143`/`:161-170`**; drop `{n}` from the clear-history confirm **[`:449`]**. Identity's Local path `CodeBlock` **[`:255-261`]** is byte-unchanged | F1, F3 (AC-8, AC-11, AC-77, AC-83, AC-89, AC-92, AC-S21) |
| `src/screens/rules-manager.tsx` **[`:191-194`, `:218-227`]** | "Add default rule" in toolbar + empty state; AC-122's conditional shadow confirm; AC-123's predicate + in-flight disable; AC-124's tooltip wrapper; the two `focus:` classes on the existing Delete item **[`:291-296`]** (AC-84); dead-class fix at `:232` (AC-130(b)) | F6, cross-cutting |
| `src/api/client.ts` **[`:78-81`, `:99-105`, `:217-226`]** | five new methods: `createShare`, `listShares`, `revokeShare(token, id)` (Bearer) and `getSharedRequests`, `getSharedRequest` with **`noAuth: true`** (AC-42); public paths use `encodeURIComponent(code)` | F4 |
| `src/api/schemas.ts` **[`:48-58`, `:115-126`, `:149-172`]** | `shareLinkCreateSchema`, `shareLinkSchema`, `shareLinkCreatedSchema`, `publicRequestSummarySchema`, `publicRequestDetailSchema`, `publicShareFeedSchema` + inferred types. **No change for F7** — `response_body: z.string().nullable()` already exists **[`:169`]** | F4 (§5.5) |
| `src/router.tsx` **[`:35-49`]** | register `{ path: "/s/:code", element: <ShareView /> }` before the `*` fallback | F4 |
| `src/lib/copy.ts` | all new keys from `copy.md` §4 wired 1:1; two **changed** values (`set.confirm.clearHistory.body`, `insp.body.truncated`); `dash.pathUrl.*` **[`:65-66`]** stay unreferenced (AC-10) | all FE |
| `src/components/ui/copy-button.tsx` **[`:22-31`, `:47-49`]** | AC-132 — success only after the write resolves, plus a failure/fallback state | cross-cutting |
| `src/components/hookbox/key-value-rows.tsx` **[`:9`, `:22`, `:32-35`]** | AC-133 — sentinel `__redacted__` → `<redacted>`; AC-131 — "redacted"/"None" → `t()` | cross-cutting |
| `src/components/hookbox/json-tree.tsx` **[`:13`, `:71-99`, `:81`, `:83-98`]** | AC-135 — raw default above ~64 KB + the two captions; AC-131 — Pretty/Raw/"(empty)" → `t()`; dead-class fix (AC-130(c) defers `:81`; the caption work touches the same file, so `:81` **is** in scope here) | F7, cross-cutting |
| `src/components/ui/button.tsx` **[`:21`]** | AC-129 — `text-white` → `text-text-on-accent` on `variant="danger"` | cross-cutting |
| `src/components/ui/menu.tsx` **[`:38`]** | *optional* per `design.md` §9.2 — add a `destructive` prop rather than repeating the three classes at two call sites (AC-84) | cross-cutting |
| `src/components/hookbox/feed-row.tsx` **[`:23-34`, `:64-65`]** | `relTime` extracted to `src/lib/time.ts` (four surfaces need it); dead-class fix at `:64-65` (AC-130(b)) | F4, cross-cutting |
| `src/screens/dashboard/inspector.tsx` **[`:246-253`, `:266`]** | **no change for F7's rendering** (already handles both `null` and populated); dead-class fix at `:266` (AC-130(b)) | F7, cross-cutting |
| `tailwind.config.ts` **[`:108-112`]** | add `maxWidth.viewer: '920px'` (AC-109(g)) | F4 |
| `index.html` **[`:3-6`]** | add `<meta name="referrer" content="no-referrer">` (AC-S5) | F4 |

**Backend**

| File | Change | Feature |
|---|---|---|
| `backend/src/routes/api.rs` **[`:273-299`, `:506-532`]** | **exactly two edits** — `fn effective_client_ip` → `pub(crate) fn`, and `delete_endpoint` gains the revoke-shares statement before `gone_at` (§5.3). `api_router()` is **not** changed | F4 |
| `backend/src/models.rs` **[`:135-138`, `:244-269`]** | add `ShareLinkCreate`, `ShareLink`, `ShareLinkCreated`, `PublicShareFeed`, `PublicEndpointInfo`, `PublicRequestSummary`, `PublicRequestDetail` as **standalone** structs (§5.5, AC-102). Nothing for F7 | F4 |
| `backend/src/ids.rs` **[`:41-45`, `:57-60`]** | add `gen_share_code` with the ≥ 16-byte clamp (AC-31, AC-32, AC-103) | F4 |
| `backend/src/config.rs` **[`:10-41`, `:110-189`]** | the four §5.8 fields + defaults + the `share_code_bytes` clamp | F4 |
| `backend/src/routes/mod.rs` | `pub mod share;` + `pub use share::share_router;` | F4 |
| `backend/src/router.rs` **[`:178-193`]** | `.merge(share_router())` next to `.merge(api_router())`; add `CatchPanicLayer` outermost (AC-S18). **No `TraceLayer`** (AC-S6) | F4, security |
| `backend/src/interceptor/engine.rs` **[`:636-644`, `:646-653`, `:677-683`, `:699-704`]** | F7 — add `capture_response_body`; delete the local `truncate` closure; `spawn_trace` gains `resp_body: &[u8]`; four call sites capture, the chaos-dropout site passes an empty slice; `response_body: None` at `:683` becomes the shared helper; `resp_headers` becomes `helpers::response_headers_for_trace(...)`; the echo payload's persisted copy uses `redact(&headers_lower)` (AC-S3). **The only file in `backend/src/interceptor/` that changes** | F7, security |
| `backend/src/helpers.rs` **[`:32-49`, `:118-125`]** | add `truncate_utf8`, `body_for_trace` (AC-70) and `response_headers_for_trace` (the R11 seam) | F7 |
| `backend/Cargo.toml` **[`:23`]** | add the `catch-panic` feature to the existing `tower-http` dep — **a feature flag, not a new crate** (AC-S18) | security |
| `backend/tests/api.rs` | new integration coverage for §5.1/§5.2 and the F7 matrix (see §6.1) | F4, F7 |

**Untouched, explicitly:** `backend/src/db.rs`, `backend/src/limiter.rs`, `backend/src/auth.rs`,
`backend/src/error.rs`, `backend/src/planes.rs`, `backend/src/rule_cache.rs`,
`backend/src/tasks/sweep.rs`, `backend/src/routes/tunnel_ws.rs`, `backend/src/interceptor/proxy.rs`,
`backend/src/interceptor/matcher.rs`, `backend/src/interceptor/templating.rs`, and every other file
under `backend/src/interceptor/`. `package.json` gains no dependency.

**Deploy / test**

| File | Change |
|---|---|
| `deploy/nginx.conf` **[`:9-11`, `:18-25`]** | the two new locations from §5.9.3 (with the four security headers and `access_log off`) |
| `e2e/mock-backend.ts` **[`:139`, `:238`]** | route stubs for `POST/GET /api/endpoints/:token/shares`, `DELETE …/shares/:id`, `GET /api/share/:code/requests[/:id]`, plus fault-injection switches for F3's partial failure, F5's per-row 404, and the viewer's 404/429/5xx lifecycle (AC-105) |
| `e2e/visual.spec.ts`, `e2e/states.spec.ts`, `e2e/journeys.spec.ts` | sub-header chip-count assertions and snapshots (AC-7, AC-85); the `/s/:code` state specs; the "zero `Authorization`", "zero `ws://`/`text/event-stream`", "zero accent-filled control" and "no `Referer` containing the code" network/DOM assertions (AC-42, AC-45, AC-109, AC-S5) |

### 6.1 Test coverage the QA gate owns

**F4 (`backend/tests/api.rs`):** mint/list/revoke happy path; 401 (missing/malformed/unknown Bearer) × 3
routes; 404-not-403 for a foreign token × 3 routes; the cap 422; the label 422; mint-on-tombstoned → 404
(AC-100); tombstone-revokes-all + every prior URL 404s, **including with the revoke forced to fail**
(AC-S9); **byte-identical 404** for unknown vs revoked vs tombstoned across status line, body **and
headers**, including `HEAD` (AC-36, AC-S14); cross-endpoint detail id → 404 (AC-35); `?limit=999` → 422
for **both** a valid and an invalid code (AC-101); 429 + `Retry-After` past 120/min and past the global
ceiling (AC-113); `no-store` on 200/404/422/429/503 (AC-37); POST/PATCH/PUT/DELETE on both public paths
→ 405 with the five row counts unchanged (AC-39); the public JSON key-set allow-list (AC-102); the
`Set-Cookie` asymmetry test (AC-S1); token-absence after the filter across all eight `served_by` paths
(AC-S2); the row-level masked-value invariant (AC-S4); 100 000 IPs without evicting `rl:<token>`
(AC-S7); no code in any log line or error detail (AC-S6, AC-S16).

**F7 (`backend/tests/api.rs`, `helpers.rs`/`ids.rs` `#[cfg(test)]`):** the eight-row `served_by` capture
matrix (AC-68); the three `NULL` cases (AC-69); truncation (a)–(d) including mid-multibyte no-panic
**with a 2xx and a written trace row** (AC-70, AC-S17); the `0x80 0xFF 0xFE` lossy case with byte-exact
client bytes (AC-71); golden status + sorted-headers + body fixtures for seven paths (AC-72); (d1)/(d2)/
(d3) (AC-73); the `RequestDetail` key-set-unchanged assertion (AC-74); the `new_request` payload has no
body keys (AC-75); the echo-redaction triple assertion (AC-S3); the 5 MB pre-spawn truncation (AC-S19);
the panic-layer test (AC-S18); `gen_share_code` entropy/charset/uniqueness/clamp/non-derivation (AC-31,
AC-32, AC-103).

## 7. New files  *(all [new — to be created])*

**Backend**
- `backend/migrations/0002_share_links.sql` — §5.4 DDL (`id` + `code_hash`, additive; `0001` untouched)
- `backend/src/routes/share.rs` (~320 lines) — **the entire F4 route surface in one auditable file**:
  `share_router()`, the three owner handlers (#19–21), the two public handlers (#22–23), `share_url`,
  `share_not_found`, `is_share_code_shape`, the coalesced `last_used_at` touch, the row→`Public*`
  projection functions, and §5.11's two filter constants. The module doc states the trust boundary, the
  frozen check order, and the "no mutation, no owner secret, no token" invariants

**Frontend**
- `src/screens/share-view.tsx` (~280 lines) — the public `/s/:code` viewer. **Imports neither `AppShell`
  nor `session`** (AC-S13). Ten documented states (AC-44), the AC-105 poller, and the AC-109 visual
  prohibitions. Reuses only read-only presentational primitives (`MethodBadge`, `StatusCode`,
  `ServedByChip`, `KeyValueRows`, `JsonTree`, `CodeBlock`, `SkeletonLines`, `InlineAlert`, `Tabs*`,
  `BrandMark`, `ThemeToggle`) so every value is a text node (AC-67)
- `src/components/hookbox/share-dialog.tsx` (~220 lines) — owner mint/list/revoke on the existing
  `Dialog` primitives, with AC-24's shown-once panel, AC-93's disclosure, AC-94's label field, AC-95's
  inline two-step revoke and AC-96's full state set
- `src/components/hookbox/confirm-dialog.tsx` — extraction of the private helper at
  `src/screens/settings.tsx:659-705`, **plus the missing `catch`** (AC-83). Two existing Settings
  confirms migrate to it
- `src/components/ui/progress.tsx` (~20 lines) — the determinate bar of AC-134, tokens only. Shared by F3
  and F5; no such primitive exists today
- `src/lib/csv.ts` (~70 lines) — **pure**, DOM-free RFC 4180 serializer: `escapeCell` (guard-then-quote),
  `toCsv` (CRLF + trailing CRLF), and the three frozen artifact constants (AC-64's exception). Unit-
  testable without a browser (AC-54, AC-55)
- `src/lib/request-export.ts` (~120 lines) — the F5 orchestrator: `EXPORT_CONCURRENCY = 4`,
  `fetchDetails`, `buildRequestCsv`, `exportFilename`, and the one place a 429 `Retry-After` would be
  handled (AC-119). Keeps `dashboard.tsx` from growing. *(Added per `architecture.md` §8 item 8 — it was
  missing from the draft's §7.)*
- `src/lib/config-bundle.ts` (~120 lines) — `configBundleSchema` + `CONFIG_BUNDLE_VERSION = 1`,
  `buildBundle`, `parseBundle` (returning the first-failure message of AC-16/AC-90),
  `MAX_BUNDLE_BYTES = 5_000_000`, `MAX_BUNDLE_RULES = 200`, the `_assignable` type guard, and the
  BOM-strip
- `src/lib/download.ts` — one `downloadBlob(filename, mime, bytes)`: create object URL → click →
  **revoke** in a `finally`. AC-12 and AC-49 both require it, so a shared helper is what stops the two
  from drifting. *(Added per `ux.md` gap 8.)*
- `src/lib/time.ts` — `relTime` extracted from `src/components/hookbox/feed-row.tsx:23-34`; four new
  surfaces need it (share rows, viewer rows, "Updated {when}", the export strip). *(Added per `ux.md` §6
  item 13.)*
- `public/robots.txt` — `Disallow: /s/` (AC-S27). Vite copies `public/` into `dist/`; there is no
  `public/` directory today, so this creates one

**Docs**
- `docs/features/operator-toolkit/prd.md` — this document (already exists; listed for completeness)

## 8. Risks & assumptions

**R1 — F4 is a new internet-reachable read surface (highest risk).** Any bug in code entropy, ownership
scoping, response projection, or revocation exposes one operator's captured traffic to anyone.
Mitigations are **structural, not intentions**: 192-bit CSPRNG codes stored hashed (§5.4) with a ≥ 16-byte
clamp (AC-103); the projection is built field-by-field from standalone structs so a future owner-shape
field cannot leak by default (AC-102); AC-35 is enforced inside a single `WHERE id = ? AND token = ?`;
all five negative outcomes funnel through one `share_not_found()`; parameter validation precedes code
resolution so `?limit=999` is not an existence oracle (AC-101); revocation is a per-request DB read with
no cache and `no-store` (AC-37); the resolver is per-IP **and** globally rate limited (AC-113); no verb
but GET is routed (AC-39); and the code never appears in an owner-route URL or a log (AC-26, AC-S5,
AC-S6). **`security.md` found this list was still insufficient** — see R11.

**R2 — Shared links expose payloads by design, and F7 widens what "payload" means.** A link exposes
request bodies, request headers (redacted only for `authorization`/`cookie`/`x-owner-id` — a bearer token
in a *custom* header or a JSON body is shared verbatim), response headers (filtered per AC-S1 but not
otherwise redacted), **and** response bodies including anything an operator hard-coded into a rule
`body_template` or that a MITM/tunnel upstream returned. **Mitigated by disclosure, not by hiding:**
AC-S11/AC-93 make the mint step name all of it — including that *other people's* requests and *pre-mint*
requests are included (AC-S12 declined) and that the endpoint's **name** is visible. Assumption: the
operator consciously accepts this at mint time. Partial comfort verified: MITM already strips upstream
`Set-Cookie` before the response exists **[existing — verified at
`backend/src/interceptor/proxy.rs:25-44`]**, so the at-rest exposure is narrower than it looks (§5.11).

**R3 — RESOLVED, not a risk.** §5.4 settles share-code storage as hashed + non-secret `id`, with
`architecture.md` §3.2's five-option trade-off table and `security.md` §4's explicit deferral. The
`id`-based revoke route must survive any future re-litigation, because D10 is independent of the at-rest
question.

**R4 — F7 puts new work on the mock plane's hot path (second-highest risk).**
- *Latency.* Capture is a `size_hint` read, a refcount bump and a `Response::from_parts`; the only copy
  is the ≤ 256 KB truncated `String` the row needed anyway. Gated by AC-73(a)/(b)/(c) plus the rewritten
  (d1)/(d2)/(d3) and AC-S20.
- *Memory.* **No extra full-size retention.** The `Bytes` is shared with the outgoing response, and what
  moves into the spawned task is the already-truncated `Option<String>` (AC-S19), so the 5 MB MITM worst
  case is not held past the response. The one transient extra allocation is `from_utf8_lossy` for a body
  that is *not* valid UTF-8 (valid UTF-8 borrows), bounded by `MITM_MAX_BODY_BYTES` and freed before the
  response returns.
- *Storage.* A trace row can hold up to 2 × `MAX_BODY_BYTES` of body text instead of 1 ×. Bounded by
  `TRACE_CAP = 100` and `TRACE_TTL_HOURS = 24` **[existing — verified at
  `backend/src/config.rs:156-157`]**, enforced both at write time **[existing — verified at
  `backend/src/db.rs:91-103`]** and by the periodic sweep, so worst case per endpoint roughly doubles
  (~25 MB → ~50 MB) and the existing sweep reclaims it with **no change**.
- *Exposure.* Feeds R2 and R11.
- *Silent truncation is ambiguous (accepted).* AC-70 mirrors `request_body`'s existing marker-free
  convention so the two columns cannot behave differently. Symmetric-and-silent beat
  asymmetric-and-explicit, which would have forced a schema change and a §5 shape change for what is
  meant to be a narrow fix. The developer-facing heuristic is a stored length equal to `MAX_BODY_BYTES`;
  **AC-136 surfaces it to users as a hedge** ("may be cut short") rather than leaving it invisible.

**R5 — F3 import is not transactional.** SQLite gives no cross-request transaction and there is no bulk
endpoint, so a mid-import failure leaves a deterministic prefix applied (AC-19). Assumption: "add, never
replace, never roll back, report precisely" is safer than any client-side rollback that could destroy
pre-existing rules. Mitigated further by AC-S21's pre-apply diff (so the operator consents to the change
set before anything is written) and AC-91's persistent report.

**R6 — F5 issues up to 100 authenticated detail fetches per click.** Bounded concurrency of 4 (AC-47)
plus Cancel (AC-48) plus a per-row timeout (AC-119) keeps this from looking like a self-inflicted DoS on
a small self-hosted box. Assumption verified: only `POST /api/session` is rate limited today **[existing
— verified at `backend/src/routes/api.rs:211-214`, `:304-326`]**; if a limiter is ever added to
`GET /api/requests/{id}`, `src/lib/request-export.ts` is the single place that must grow 429 handling.

**R7 — F1 is irreversible.** `clear_requests` hard-deletes rows **[existing — verified at
`backend/src/routes/api.rs:884-887`]**. A plain confirm is proportionate: traces are already ephemeral
(100-row cap, 24 h TTL) and the typed-token confirm is reserved for endpoint deletion **[existing —
verified at `src/screens/settings.tsx:575-654`]**. `security.md` S-18 independently confirms no CSRF
token is needed (AC-S24).

**R8 — F6 changes an endpoint's default answer, and can silently disable four fallbacks.** The draft's
R8 only named "404 becomes 200". The real risk, found by `journey.md`, is that a matched rule
short-circuits Auto-CRUD, tunnel, MITM and `default_mode` entirely (AC-122), and `priority = 1000`
provides **no** protection. Mitigations: opt-in per endpoint (AC-57), never auto-created (AC-62),
priority 1000 so real rules always win (AC-59), an obviously-placeholder body (AC-125), the completed
duplicate guard (AC-123), and — the substantive one — AC-122's conditional confirm plus a backend
regression test proving the shadowing is chosen rather than discovered.

**R9 — F2 removes an affordance some operators may rely on** for local `/e/<token>` testing. Mitigated by
keeping it on Settings → Identity byte-unchanged (AC-8) **and**, newly, by AC-86's tooltip pointer —
because R9 originally accepted the *removal* but not the *discoverability loss* (`journey.md` gap 45).
This was an explicit, already-approved user decision, not a PM inference.

**R10 — Copy is no longer placeholder-quality, and that changes who owns it.** `copy.md` §4 is now the
frozen source for every new string, wired 1:1 into `src/lib/copy.ts` (AC-64). Two **shipped** values
change (`set.confirm.clearHistory.body`, `insp.body.truncated`), so any Playwright/visual assertion on
them must be updated (AC-66, AC-77, AC-136). Up to eleven optional keys stay intentionally unwired, which
is why AC-10 fixes the parity check to one direction.

**R11 — Response headers are persisted un-redacted, and F4 would have published the endpoint token.**
**This was the batch's most serious finding and it is now fixed, not accepted.** `redact()` is applied to
*request* headers only **[existing — verified at `backend/src/interceptor/engine.rs:674`]** while the
response map is collected verbatim **[existing — verified at `:636-644`]** — and **every** persisted
response-header map contains `x-hookbox-endpoint: <token>` and `x-hookbox-rule-id` **[existing — verified
at `:584-588`]**. The draft's AC-44a would have published that to anonymous viewers, handing them the
owner's endpoint token: enough to `POST /e/<token>` and write into the endpoint, flood ≥ 100 requests to
evict the very evidence being shared, mutate CRUD collections when `auto_crud` is on, and make HookBox
issue attacker-chosen requests to the operator's `target_url`. **"Read-only share link" was falsified.**
Resolution: AC-S1's public-projection filter (§5.11), plus AC-S2/AC-S4 as the assertions that keep it
true. **Verbatim response headers remain accepted for the owner Inspector and F5's CSV** —
`security.md`'s S-4 ruling — because that is what an inspector is for and redacting at rest would
silently change already-stored rows. Residual accepted: the DB holds upstream cookies for ≤ 24 h.

**R12 — Today's truncation can panic on a multibyte boundary, and the panic drops the connection.**
`truncate` does `s[..cap]` **[existing — verified at `backend/src/interceptor/engine.rs:646-653`]**,
which panics when byte `cap` falls inside a UTF-8 character. It is reachable **today** for `request_body`
(a > 256 KB body with a multibyte char at exactly the cap), and F7 would make it reachable for
`response_body`, where the bytes come from an upstream or a bound CLI rather than the operator. Worse
than "a lost body": the record is built *before* the `tokio::spawn` and there is no panic layer
**[existing — verified at `backend/src/router.rs:178-193`]**, so the current failure mode is a **dropped
connection**. AC-70 + AC-S17 fix it at the source for both columns and test the panic case explicitly;
AC-S18 removes the failure mode. This incidentally hardens the pre-existing request path.

**R13 — axum's auto-405 is not our envelope.** AC-39's 405s come from `MethodRouter` and carry an empty
body and no `no-store` (`architecture.md` D14). Accepted: consistent with every other route in the app,
and a 405 discloses nothing. A `map_response` layer over the share router is available if a future
security pass wants uniformity.

**R14 — the rate limit is a courtesy, not a guarantee, and a NAT'd audience shares one bucket.**
`effective_client_ip` resolves the first `X-Forwarded-For` hop / `X-Real-IP` **[existing — verified at
`backend/src/routes/api.rs:273-299`]**, so several viewers behind one corporate egress IP share a bucket
— hence 120/min rather than 60 (AC-113). Two honest caveats: the limiter **fails open** on anomaly and
evicts the most-idle bucket past `MAX_BUCKETS = 100_000` **[existing — verified at
`backend/src/limiter.rs:43`, `:88-89`, `:130-142`]**; and **the shipped compose topology has no nginx**
**[existing — verified at `docker-compose.yml`]**, so viewers there may collapse into a single bucket
(`security.md` S-6b). AC-S7 stops that from harming the mock plane; AC-S15's global ceiling bounds
egress; the viewer's 429 state shows the `Retry-After` countdown (AC-44) rather than looking broken.

**R15 — a hand-typed share URL on the mock host would ingest the code as a webhook.**
`<token>.<MOCK_DOMAIN>/s/<code>` resolves to the **mock** plane **[existing — verified at
`backend/src/planes.rs:137-148`]**, so the code would be persisted into `request_logs.path`, displayed in
the feed, exported by F5, and shown to viewers of that same link. **Mitigated by construction, not by
scrubbing:** `share_url` is always built on the app origin (§5.5.3, AC-99), so HookBox never *mints* such
a URL. A hand-typed one is user error. Deliberately **not** mitigated by scrubbing trace paths that match
the share-code shape — that would put a secret-shaped pattern match on the mock plane's hot path, which
is worse than the risk it addresses (`journey.md` gap 12, decided).

**R16 — no CSP, and the viewer is same-origin with the owner secret.** The viewer page is built entirely
from attacker-supplied text and lives on the same origin as `localStorage['hookbox-owner-secret']`
**[existing — verified at `src/api/session.ts:17-19`]**, so an XSS there is owner takeover. There is **no
CSP, no `Referrer-Policy` and no `X-Frame-Options` anywhere today** (zero `add_header`/`Content-Security`
hits across `deploy/`, `index.html`, `backend/src/`). This batch adds the cheap headers (AC-S26) and
relies on AC-67 + AC-S13 for XSS prevention, but **the starter CSP is deferred** (§2) because
`index.html`'s inline theme script needs a nonce/hash strategy. Recorded honestly: until that follow-up
lands, AC-S13 is the *only* mitigation, with no defence-in-depth behind it. This is the single largest
knowingly-accepted residual in the batch.

## 9. Open Questions

**None remaining.** Every question raised across the six discovery/design documents has been decided in
this revision. For the record, and so no reviewer has to re-derive them:

| Origin | Question | Decision | Where |
|---|---|---|---|
| draft OQ-1 | Is `response_body` capture in scope? | **Yes** — F7 | §4.8 |
| draft OQ-2 | Does the public viewer see bodies + headers? | **Yes**, with AC-S1's response-header filter | AC-44a, §5.5.5 |
| `architecture.md` D8 | AC-73(d) is unmeasurable | Replaced by (d1)/(d2)/(d3) | AC-73 |
| `architecture.md` D9/D10/D11 | Share-code storage; revoke by what? | **Hashed + revoke by integer `id`** | §5.4, AC-24/25/26 |
| `architecture.md` D12 | AC-55's quoting parenthetical | Dropped; expected cell is unquoted | AC-55 |
| `architecture.md` D13 | Compose `endpointConfigPatchSchema`? | **No** — separate `.strict()`, all nine required | AC-13, §5.5.6 |
| `architecture.md` D14 | Does *every* public response carry `no-store`? | Narrowed to handler-produced | AC-37 |
| `architecture.md` D15 | `SHARE_RATE_LIMIT_PER_MIN` = 60? | **120**, + a 1200 global ceiling | AC-113, §5.8 |
| `architecture.md` §8.9 | Fix the chaos-dropout trace? | **No** — leave it, file a follow-up | §2 (F7) |
| `architecture.md` §8.10 | `SHARE_LINKS_ENABLED` flag? | **No** — nginx `return 404` is the opt-out | §5.8 |
| `architecture.md` §8.11 | Is `endpoint.name` public? | **Yes**, disclosed at mint; never the `<h1>` | AC-93, AC-107 |
| `security.md` OQ-S1 | Adopt AC-S12 (mint-time window)? | **Declined**, with reasons; pre-mint disclosure becomes mandatory | AC-S12 |
| `security.md` OQ-S2 | `Referrer-Policy` via meta or nginx? | **Both, meta first** | AC-S5, §5.9 |
| `security.md` OQ-S3 | Is 60/min defensible? | **No** — 120/IP + 1200 global | AC-113 |
| `security.md` OQ-S4 | A redaction note inside the CSV? | **No** (RFC 4180 has no comments) — UI-side alert instead | AC-121, §5.6 |
| `security.md` S-17 | Re-open plaintext storage? | **No** — explicitly closed | §5.4 |
| `ux.md` gaps 1–2 | Clear all / Export CSV placement and weight | Overflow `Menu`; ghost + `text-danger-fg` | AC-1, AC-46, AC-84 |
| `ux.md` gap 3 | Count in the F1 confirm? | **No count**; the shipped Settings string loses `{n}` | AC-77 |
| `ux.md` gap 12 | Share placement + count badge? | Right cluster, first; badge **in scope**, degrades gracefully | AC-23, AC-98 |
| `ux.md` gap 15 | Storage blocks the dialog design | Resolved → shown-once panel | AC-24, §5.4 |
| `ux.md` gap 27 | Owner-authored `insp.*` on a public page? | **Yes**, except `insp.headers.redacted.tooltip` | AC-111 |
| `ux.md` gap 30 | Owner mobile nav in scope? | **No** — out of scope, must not get worse | AC-127 |
| `design.md` gap 5 | Fix the dead `bg-*` classes? | **Yes, precisely scoped**; rest is a follow-up | AC-130 |
| `design.md` gap 14 | `insp.body.largeRaw` as caption or alert? | **Quiet caption** | AC-135 |
| `copy.md` §7.1 | Disclosure vs AC-S1 contradiction | Resolved for AC-S1; disclosure states the redaction list | AC-93 |
| `copy.md` §7.4 | Amend §5.5.7's frozen payload? | **Yes** — `copy.md`'s `name`/`body_template` | AC-125, §5.5.7 |
| `copy.md` §7.8 | Unreachable-origin heuristic | Adopted as proposed | AC-99 |
| `copy.md` §7.10 | Where does `feed.export.detailNote` live? | Dismissible `InlineAlert` after a partial export | AC-121 |
| `copy.md` §7.15 | Ship the landing deltas? | **No** — out of scope this batch | §2 |
| `copy.md` §7.16 | `dash.mockUrl.tooltip`? | **Adopted** | AC-86 |
| `copy.md` §7.19 | Copy-parity direction | One direction only | AC-10 |
| `journey.md` gap 12 | Scrub share-code-shaped trace paths? | **No** — mitigate by construction | §8-R15 |
| `journey.md` gap 15 | Row deep-linking on the viewer? | **No** for v1 | AC-112 |
| `journey.md` gap 16 | F7 widens pre-existing links | **Ship F4 and F7 together** | AC-114 |
| `journey.md` gap 34 | Sentinel/body collision | Documented and accepted | AC-121, §5.6 |

**This PRD is lockable.** Once locked, §5 is frozen: both lanes build against it, and neither may change
a shape the other depends on.

## 10. Task graph (beads)

**BUILT. Epic: `hookbox-mun`** (resolved, not re-created). 13 build issues + 5 out-of-scope follow-ups.
`bd dep cycles` reports none. Lane queues: `bd ready -l area:frontend,feature:operator-toolkit`,
`bd ready -l area:backend,feature:operator-toolkit`. **The PRD is frozen — no further edits are expected;
§5 is the only contract both lanes build against.**

### 10.1 The issue → AC index

| Issue | Lane | P | Scope | ACs |
|---|---|---|---|---|
| `hookbox-mun.11` | **BE-1** `area:backend` | 1 | F4 server: migration `0002`, `share.rs`'s 5 routes, standalone public structs, `gen_share_code`, the 4 config vars, `api.rs`'s two edits, `mod.rs`/`router.rs` wiring, §5.11's filter, integration tests | AC-24…AC-30 (server halves), AC-31, AC-32, AC-33, AC-34, AC-35, AC-36, AC-37, AC-38, AC-39, AC-40, AC-99 (server half), AC-100, AC-101, AC-102, AC-103, AC-104, AC-113, AC-S1, AC-S2, AC-S4, AC-S6 (app-logging half), AC-S7, AC-S9, AC-S10, AC-S12 (declined — do not scope), AC-S14, AC-S15, AC-S16, AC-S24 (backend half), AC-66 |
| `hookbox-mun.12` | **BE-2** `area:backend` | 1 | F7 capture: `capture_response_body`, the three `helpers.rs` fns, `spawn_trace`'s new param, the 5 call sites, the echo redaction, the full test matrix — **plus the three test-only F6/engine regression assertions** | AC-68, AC-69, AC-70, AC-71, AC-72, AC-73(a–c, d1–d3), AC-74, AC-75, AC-S3, AC-S17, AC-S19, AC-S20, **AC-62, AC-63, AC-122(e)**, AC-66 |
| `hookbox-mun.13` | **BE-3** `area:backend` | 2 | Deploy + hardening: the two nginx locations, the four headers, `index.html`'s `<meta referrer>`, `public/robots.txt`, the `catch-panic` feature + `CatchPanicLayer` | AC-S5, AC-S6 (nginx half), AC-S18, AC-S26, AC-S27, AC-66 |
| `hookbox-mun.14` | **FE-0** `area:frontend` | 1 | Shared primitives + shared libs + in-scope debt: `confirm-dialog.tsx`, `progress.tsx`, `download.ts`, `time.ts`, `menu.tsx`, `button.tsx`, `copy-button.tsx`, `key-value-rows.tsx`, `json-tree.tsx` | AC-83, AC-84, AC-128, AC-129, AC-130(a)+(b in-scope), AC-131, AC-132, AC-133, AC-134, AC-135, AC-136, AC-77 (shipped-string half), AC-64/65/66 |
| `hookbox-mun.15` | **FE-1** `area:frontend` | 2 | F1 Clear all: `clearRows` in `use-feed.ts`, the overflow `Menu` + confirm in `FeedPane` | AC-1…AC-6, AC-76, AC-77 (feed copy), AC-78, AC-79, AC-80, AC-81, AC-82, AC-126, AC-127 |
| `hookbox-mun.16` | **FE-2** `area:frontend` | 2 | F2 delete the path chip + its test fallout + the mock-URL tooltip | AC-7, AC-8, AC-9, AC-10, AC-85, AC-86, AC-130(b) app-shell sites, AC-127 |
| `hookbox-mun.17` | **FE-3** `area:frontend` | 2 | F3 `config-bundle.ts` + the Settings Configuration section + the pre-apply diff | AC-11…AC-22, AC-87, AC-88, AC-89, AC-90, AC-91, AC-92, AC-S21, AC-S22, AC-S23, AC-134, AC-126, AC-81 |
| `hookbox-mun.18` | **FE-4** `area:frontend` | 1 | F4 UI: schemas + 5 client methods, `share-dialog.tsx`, `share-view.tsx`, the `/s/:code` route, the Share control | AC-23…AC-27 (client halves), AC-41, AC-42, AC-43, AC-44, AC-44a, AC-45, AC-93…AC-99 (client halves), AC-105…AC-112, AC-S8, AC-S11, AC-S12 (ship the un-scoped wording), AC-S13, AC-126, AC-127, AC-128, AC-81 |
| `hookbox-mun.19` | **FE-5** `area:frontend` | 2 | F5 `csv.ts` + `request-export.ts` + the Export CSV item and progress strip | AC-46…AC-56, AC-56a (client half), AC-115…AC-121, AC-S25, AC-64 (+ its CSV-literal exception), AC-82, AC-134, AC-128, AC-81 |
| `hookbox-mun.20` | **FE-6** `area:frontend` | 3 | F6 "Add default rule" + the shadow confirm + the complete duplicate guard | AC-57…AC-61, AC-122(a)–(d), AC-123, AC-124, AC-125 |
| `hookbox-mun.21` | **QA gate** `area:qa` `step:qa` | 1 | Blocked on all ten task issues. Both lenses + the §5 contract on both sides | **all 165**, and it *owns* AC-44a, AC-56a, AC-114 |
| `hookbox-mun.22` | **Security gate** `area:security` `step:security` | 1 | Code-level review after QA passes | AC-S1…AC-S27 re-verified against the shipped code |
| `hookbox-mun.23` | **Sync** `area:sync` `step:sync` | 2 | Reconcile `.beads/issues.jsonl`, close the epic, push | — |

Cross-cutting ACs appear under more than one issue on purpose (each lane owns its own surface):
AC-64/AC-65/AC-66/AC-67 are standing requirements on every issue; AC-81 and AC-126/AC-127/AC-128 are
listed against each control that must implement them. Every one of the 165 ACs appears at least once.

### 10.2 Dependency edges (`bd dep add <blocked> <blocker>`)

- `BE-1 ← BE-3` — both edit `backend/src/router.rs` (`:178-193`); BE-3 settles the layer stack
  (`CatchPanicLayer` outermost, no `TraceLayer`) before BE-1 merges `share_router()`. This is the one
  deviation from `architecture.md` §6's "BE-3 can land before BE-1, depends on nothing": it now also owns
  AC-S18, which touches that file.
- `FE-1, FE-2, FE-3, FE-4, FE-5, FE-6 ← FE-0` — the primitives land once, not six times.
- `FE-4 ← FE-2` (coordination point 2: `app-shell.tsx:98-118`); `FE-5 ← FE-1` (coordination point 1:
  `dashboard.tsx:325-350`).
- `QA ← BE-1, BE-2, BE-3, FE-0, FE-1, FE-2, FE-3, FE-4, FE-5, FE-6` (all ten).
- `Security ← QA`; `Sync ← Security`.
- Initial ready queue, verified: frontend `FE-0`; backend `BE-2`, `BE-3`; qa/security/sync all empty.

### 10.3 Two refinements of `architecture.md` §6, recorded

1. **AC-62, AC-63 and AC-122(e) went to BE-2, not FE-6.** All three are Rust assertions in
   `backend/tests/api.rs` about existing engine/matcher behaviour (`engine.rs:141-145`, `:228-245`,
   `matcher.rs:53`, `:182`) and require **no production code**. Putting them in the engine owner's issue
   keeps the backend lane at three issues and keeps FE-6 purely frontend.
2. **`index.html` and `public/robots.txt` belong to BE-3, not FE-4**, even though PRD §6 lists `index.html`
   under Frontend: AC-S5 needs the `<meta name="referrer">` and the nginx `add_header` to ship together,
   so one issue owns both halves. FE-4's description forbids touching either file.

### 10.4 Follow-ups filed (out of scope, deliberately **not** labelled `feature:operator-toolkit`)

Labelled `followup:operator-toolkit` with no parent, so they cannot hold the sync gate or the epic open:

| id | Item | Source |
|---|---|---|
| `hookbox-59n` | Remaining dead `bg-*` call sites + the `tracking-wide` and alpha-modifier findings | AC-130(c), `design.md` §9.3 |
| `hookbox-19v` | Starter CSP with a nonce/hash strategy for `index.html`'s inline theme script | AC-S26, §8-R16 |
| `hookbox-2sb` | Chaos-dropout trace's `status_code = 0` / `response_headers = {}` low fidelity | §2 (F7), `architecture.md` D5 |
| `hookbox-dcq` | Opt-in "only requests from now on" share window | AC-S12 (declined for v1) |
| `hookbox-ds2` | Owner shell mobile navigation (`shell.mobileNav.*` renders nothing) | AC-127, `ux.md` gap 30 |

### 10.5 Planned shape as approved (retained for the record)

- **Feature epic:** label `feature:operator-toolkit` (resolve the existing epic; do not create a new one).
- **Backend lane (`area:backend`), 3 independent issues.** **BE-1** F4 server (migration 0002,
  `share.rs`'s five routes, the standalone public structs, `gen_share_code`, the four config vars,
  `api.rs`'s two edits, `mod.rs`/`router.rs` wiring, §5.11's filter, integration tests) — AC-23…AC-40,
  AC-99…AC-104, AC-113, AC-S1/S2/S4/S6/S7/S9/S10/S14/S15/S16. **BE-2** F7 capture (`capture_response_body`,
  the three `helpers.rs` functions, `spawn_trace`'s new param, the five call sites, the echo redaction,
  the full test matrix) — AC-68…AC-75, AC-S3/S17/S19/S20. **BE-3** deploy + hardening (the two nginx
  locations, the four headers, `public/robots.txt`, the `catch-panic` feature + `CatchPanicLayer`) —
  AC-S5/S6/S18/S26/S27. BE-1 and BE-2 touch **disjoint** files.
- **Frontend lane (`area:frontend`), 6 independent issues.** **FE-1** F1 · **FE-2** F2 · **FE-3** F3 ·
  **FE-4** F4 UI (owner dialog + public viewer + client/schemas + route) · **FE-5** F5 · **FE-6** F6, plus
  a shared **FE-0** for the cross-cutting primitive fixes (AC-83, AC-129, AC-130, AC-131, AC-132, AC-133,
  AC-134) that FE-1/FE-3/FE-4/FE-5 all consume — sequenced first so the others do not each patch the same
  primitives.
- **Coordination points, and only these four.** (1) FE-1 and FE-5 both edit `FeedPane`'s action group —
  land FE-1 first. (2) FE-2 and FE-4 both edit `app-shell.tsx`'s sub-header — FE-2 is a deletion and
  lands first. (3) FE-4 ⇄ BE-1 share only §5; FE-4 develops against `e2e/mock-backend.ts` stubs. (4)
  **AC-44a, AC-56a and AC-114 are end-to-end assertions owned by the QA gate**, which blocks on
  BE-1 + BE-2 + FE-4 + FE-5 — not on any single lane.
- **Gates.** One `area:qa` issue blocked on every task issue; one `area:security` issue blocked on QA
  (F4's projection gets the deepest review); one `area:sync` issue blocked on security.
- **Follow-up issues to file at BREAKDOWN** (each recorded above as explicitly out of scope): the
  remaining dead-class call sites + the `tracking-wide` and alpha-modifier findings (AC-130(c)); the
  starter CSP with a nonce/hash strategy for `index.html`'s inline theme script (AC-S26); the
  chaos-dropout trace's `status_code = 0` / `response_headers = {}` low fidelity (§2, F7); an opt-in
  "only requests from now on" share window (AC-S12); and the owner shell's mobile navigation (AC-127).
- The `issue-id → AC-#` index is §10.1 above.
