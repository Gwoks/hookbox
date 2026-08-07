# User journey: Operator Toolkit (F1–F7)

**Input:** `docs/features/operator-toolkit/prd.md` (DRAFT, frozen-pending).
**Method:** every flow below is mapped against the screens that exist today. Verification markers
follow the PRD's convention — `[exists at path:line]` for a surface/behaviour read in this repo,
`[NEW]` for a surface this feature must create.

**Screen inventory (what the operator actually has today)** — `src/router.tsx:35-49`:
`/` Landing/email gate · `/d/:token` Dashboard (split pane: FeedPane | Inspector) ·
`/d/:token/rules` Rules Manager (+ RuleBuilder dialog) · `/d/:token/settings` Settings ·
`/cli` public tunnel page · `*` NotFound. There is **no** public data route today; `/s/:code` is the
first one **[NEW]**.

**Two personas.**
- **Operator** — holds `owner_secret` in `localStorage` **[exists at `src/api/session.ts:16-50`]**,
  attached as `Authorization: Bearer` on every `/api/**` call except `POST /api/session`
  **[exists at `src/api/client.ts:78-81`]**. Owns F1, F2, F3, F5, F6 and the owner half of F4.
- **Viewer (new, F4)** — anonymous, no account, no session, arrives by URL only. Untrusted, possibly
  a crawler or a scanner. Owns the second half of F4 and never sees anything else.

F7 has no persona of its own: it changes what the Operator sees in the Inspector's Response tab
**[exists at `src/screens/dashboard/inspector.tsx:242-253`]**, what lands in F5's CSV, and what the
Viewer reads in F4's detail pane.

---

## Primary (happy) flow

### F1 — Clear all logs (Live Feed header)

1. Operator is on `/d/:token`, feed streaming, N rows visible (cap 100
   **[exists at `src/feed/use-feed.ts:42`]**), connection pill reads "Live".
2. Operator reads the feed header action group — today `[N new?] [Pause]`
   **[exists at `src/screens/dashboard.tsx:325-350`]** — and finds the new **Clear all** control to
   the left of Pause **[NEW]**. It is enabled because rows exist.
3. Operator activates Clear all → a confirm dialog opens on the existing `Dialog` primitive
   (Radix: focus trapped, `Esc`/overlay close, focus restored)
   **[exists at `src/components/ui/dialog.tsx`, pattern at `src/screens/rules-manager.tsx:317-336`]**,
   naming the endpoint, with ghost **Cancel** and `variant="danger"` **Clear all**.
4. Operator confirms → exactly one `DELETE /api/endpoints/{token}/requests`
   **[exists at `backend/src/routes/api.rs:878-889`, client `src/api/client.ts:221-226`]**.
5. 200 → dialog closes; success toast **[exists at `src/components/ui/toast.tsx:25`]**; the feed pane
   flips to its empty state `feed.empty.*` **[exists at `src/screens/dashboard.tsx:384-413`]**
   client-side (no waiting for a poll/WS frame); the "N new" pill is gone and `newCount === 0`.
6. Operator sends a new webhook; the first `new_request` repopulates the feed with no reload
   **[exists at `src/feed/use-feed.ts:99-106`]**. End state: a clean feed the operator trusts.

### F2 — Remove the "Local path" chip (sub-header)

1. Operator loads `/d/:token`. The sub-header renders **one** URL chip (Mock URL) instead of two
   **[today two, at `src/components/hookbox/app-shell.tsx:100-101`]**.
2. Operator copies the mock URL as before; the removed chip's information is still available two
   clicks away at Settings → Identity → "Local path"
   **[exists at `src/screens/settings.tsx:255-261`]**. `path_url` is unchanged in the API
   **[exists at `backend/src/routes/api.rs:83`, `src/api/schemas.ts:63`]**.
3. End state: a less crowded sub-header, with room for F4's Share control.

### F3 — Export / import config JSON (Settings)

**Export**
1. Operator opens `/d/:token/settings`; the screen fetches `GET /api/endpoints/{token}`
   **[exists at `src/screens/settings.tsx:66-75`]** and renders the form.
2. Operator scrolls to a new **Configuration** section **[NEW]** and activates **Export config**.
3. The client fetches the rules list (`GET /api/endpoints/{token}/rules`
   **[exists at `backend/src/routes/api.rs:542`]**) — a call this screen does **not** make today —
   builds the `ConfigBundle` (§5.5.6) and triggers a download of
   `hookbox-config-<token>.json`.
4. End state: a portable file with the 9 config fields + rules, no token, no secret.

**Import**
1. Operator opens Settings on the *target* endpoint and activates **Import config** **[NEW]**.
2. Operator picks a `.json` file. The client parses + validates the whole bundle against
   `configBundleSchema` **before any network write**.
3. Valid → one `PATCH /api/endpoints/{token}`
   **[exists at `backend/src/routes/api.rs:380-481`]**, then one
   `POST /api/endpoints/{token}/rules` per rule in array order
   **[exists at `backend/src/routes/api.rs:584-628`]**, showing "Creating rule {i} of {n}…" while
   both controls are disabled.
4. All succeed → success message with counts; the screen re-fetches
   `GET /api/endpoints/{token}`; the operator navigates to Rules and sees the imported rules
   *appended* to whatever was already there (AC-18).

### F4 — Share links, OWNER side (mint / list / revoke)

1. Operator is on `/d/:token` with traffic they want to show someone.
2. Operator activates **Share** in the sub-header, immediately after the Mock URL chip **[NEW]**.
3. The Share dialog opens and fetches `GET /api/endpoints/{token}/shares` **[NEW]**; a first-time
   operator sees the dialog's *empty* state ("no active links") plus **Create share link**.
4. Operator (optionally) types a label ≤ 80 chars, reads the exposure disclosure — *this link shows
   request bodies, request headers, response headers and response bodies to anyone who has it* —
   and activates **Create share link**.
5. `POST /api/endpoints/{token}/shares` → 201 `ShareLinkCreated`. The full URL is displayed,
   selectable, with a copy affordance, and appears at the top of the active list.
6. Operator copies the URL, pastes it into Slack/a ticket, and closes the dialog.
7. Later: Operator reopens the dialog, sees the row (created-at, label, URL, last used), and
   activates **Revoke** → `DELETE /api/endpoints/{token}/shares/{code}` → 204 → the row leaves the
   list. The link is dead permanently (soft revoke; a revoked code can never be re-minted, §5.1).

### F4 — Share links, VIEWER side (anonymous)

1. Viewer clicks a URL in Slack. The browser loads `/s/<code>`. nginx `try_files` serves
   `index.html` **[exists at `deploy/nginx.conf:9-11`]**; the plane resolver classifies the unknown
   app-host path as UI **[exists at `backend/src/planes.rs:178-179`]**; the SPA mounts the new
   `/s/:code` route **[NEW]**.
2. The page renders a **loading** state. It does **not** check for a session, does **not** redirect
   to `/` (unlike `/d/:token` **[exists at `src/screens/dashboard.tsx:136-140`]**), does **not**
   create one, and sends **no** `Authorization` header (the `noAuth` path
   **[exists at `src/api/client.ts:78-81`]**).
3. `GET /api/share/{code}/requests` **[NEW]** → 200 `PublicShareFeed`. The page renders a
   read-only header (endpoint name, created-at, request count — no token, no mock URL, no config)
   and a newest-first list of request summaries. No AppShell, no switcher, no account menu, no
   Rules/Settings/New rule, no pause/clear/export.
4. Viewer clicks a row → `GET /api/share/{code}/requests/{id}` **[NEW]** → 200
   `PublicRequestDetail` → a read-only inspector shows method, path, status, served-by, request
   headers, query, request body, response headers and **response body** (real content thanks to F7).
   Every value renders as a text node through `KeyValueRows` / `JsonTree` / `CodeBlock`
   **[exists at `src/components/hookbox/json-tree.tsx:68-103`]**.
5. While the tab is visible the list re-polls every 5 s; new hits appear near-live. On tab hide
   polling stops; on tab show it resumes.
6. Viewer copies a body into their bug report and closes the tab. They never learned the token, the
   mock URL, the rules, or the owner's identity.

### F5 — Export request log as CSV (Live Feed header)

1. Operator has rows in the feed and activates **Export CSV** **[NEW]**, in the same action group as
   Clear all and Pause.
2. The client snapshots the currently visible rows (≤ 100) and fetches
   `GET /api/requests/{id}` **[exists at `backend/src/routes/api.rs:850-874`]** for each, 4 in
   flight, showing "Exporting {done} of {total}…" with **Cancel**.
3. All details resolve → the client serialises RFC-4180 CSV (CRLF, UTF-8 no BOM, formula guard) and
   downloads `hookbox-requests-<token>-<YYYYMMDDTHHMMSSZ>.csv`, then revokes the object URL.
4. A completion toast reports "Exported {n} rows ({m} without detail)".
5. Operator opens the file in a spreadsheet and sees 10 columns including a populated
   `response_body` (F7).

### F6 — Add default catch-all rule (Rules Manager)

1. Operator opens `/d/:token/rules`. With zero rules they see the empty state
   **[exists at `src/screens/rules-manager.tsx:218-227`]**; with rules they see the toolbar
   **[exists at `src/screens/rules-manager.tsx:191-194`]**.
2. Operator activates **Add default rule** **[NEW]** (present in both places).
3. One `POST /api/endpoints/{token}/rules` with the frozen §5.5.7 payload
   (`ANY` `/*`, 200, priority 1000, placeholder JSON body).
4. 201 → the list reloads, the new rule shows `ANY` `/*` in the Match column, success toast; the
   control becomes disabled with an explanatory tooltip.
5. Operator sends `PATCH /anything/at/all` at the mock surface and gets 200 with
   `X-HookBox-Served-By: rule` **[matcher already supports this, verified at
   `backend/src/interceptor/matcher.rs:53` and `:182`]**.

### F7 — Response-body capture (no new screen)

1. Operator adds a rule whose `body_template` renders `{"hello":"world"}` and fires one request.
2. Operator selects the row in the feed; the Inspector's **Response** tab → "Response body" now
   shows the rendered body instead of `insp.response.empty`
   **[exists at `src/screens/dashboard/inspector.tsx:246-253`]** — zero frontend change.
3. The same content appears in F5's CSV `response_body` cell and in F4's public detail pane.
4. The client that made the mock request received byte-identical bytes (AC-72); the feed/WS payload
   is still body-free (AC-75).

---

## Alternate flows

**F1-A** Operator clears from Settings instead ("Clear request history"
**[exists at `src/screens/settings.tsx:445-455`]**) — the same route, a different confirm, a
different toast. Both entry points must leave the dashboard feed and the endpoint's
`request_count` consistent.
**F1-B** Operator clears while **paused**: the visible list may be empty while the buffer holds
arrivals (`buffer` / `newCount` **[exists at `src/feed/use-feed.ts:77`, `:74`]**).
**F1-C** Operator cancels via `Esc` or overlay click rather than the Cancel button — no request.

**F2-A** Operator who relied on the chip goes looking for it, finds nothing in the sub-header, and
must discover Settings → Identity unaided.

**F3-A** Export → import into a **fresh** endpoint (the intended round-trip, AC-15).
**F3-B** Export → import into an endpoint that **already has rules** → 5 rules from 2 + 3 (AC-18).
**F3-C** Import a bundle a colleague hand-edited (extra key, wrong version, 300 rules).
**F3-D** Operator imports, dislikes the result, and wants to undo — there is no undo; they must
delete rules one at a time and re-enter 9 config fields by hand.
**F3-E** Operator uses Export purely as a backup ("copy this before I break it") and never imports.

**F4-A** Operator mints several links, one per recipient, using labels to tell them apart, and
revokes them individually as each conversation ends.
**F4-B** Operator revokes *everything* by deleting the endpoint — the tombstone handler revokes all
share links in the same transaction (AC-30, `backend/src/routes/api.rs:506-532`).
**F4-C** Viewer is forwarded the link by the original recipient (expected and unpreventable).
**F4-D** Viewer opens the link on a phone in a narrow viewport.
**F4-E** Viewer leaves the tab open overnight and returns; the poll must recover, and the data they
see may have rotated out (100-row cap / 24 h TTL
**[exists at `backend/src/config.rs:156-157`]**).
**F4-F** Operator opens their **own** share link in the same browser where they hold a session — the
page must stay anonymous and must not let a share 401/404 clear the real session
**[the `noAuth` branch skips `session.clear()`, `src/api/client.ts:99-105`]**.
**F4-G** A crawler fetches `/s/<code>` because the link was pasted somewhere public.

**F5-A** Operator exports while **paused** — buffered rows are not "visible", so they are excluded.
**F5-B** Operator cancels mid-export → no file.
**F5-C** Operator exports twice in a row (or double-clicks) → two concurrent exports?
**F5-D** Operator exports a feed of exactly 1 row, and a feed at the 100-row cap.

**F6-A** Operator adds the catch-all from the **empty state** on a brand-new endpoint (the main
"make it answer something" path).
**F6-B** Operator adds it on an endpoint that already has real rules — priority 1000 keeps it last
(AC-59).
**F6-C** Operator edits the catch-all afterwards in the RuleBuilder to return their own body.
**F6-D** Operator disables the catch-all instead of deleting it, then clicks Add default rule again.

**F7-A** Operator inspects a `mitm` trace whose upstream returned HTML, or a `tunnel` trace whose
CLI returned binary-ish bytes — the response body is no longer JSON.
**F7-B** Operator inspects a trace whose response body was truncated at `MAX_BODY_BYTES`
(256 000 bytes, no marker per AC-70).

---

## Error & failure paths

### F1
| Path | Expected |
|---|---|
| `DELETE` returns 5xx / 503 `store_unavailable` | dialog stays open, danger toast, **no** rows removed (AC-6) |
| Network down (`ApiError('network', …, 0)` **[exists at `src/api/client.ts:93-95`]**) | same as above; the offline banner is already showing **[exists at `src/screens/dashboard.tsx:264-274`]** |
| 401 (secret rotated elsewhere) | the client clears the session and bounces to `/` **[exists at `src/api/client.ts:99-105`]**; the dialog unmounts with the screen |
| 404 (endpoint deleted in another tab) / 410 gone | **unspecified in the PRD** — the shell only handles 404/410 on the *initial* load |
| Confirm double-clicked | exactly one DELETE; the confirm button must be disabled while in flight |
| A `new_request` lands between confirm and 200 | end state must be defined (see gaps) |

### F3
| Path | Expected |
|---|---|
| Malformed JSON / wrong `hookbox_config_version` / unknown top-level key / bad rule / > 5 MB / > 200 rules | reject before any write, naming the first failing field/index; **zero** `PATCH`/`POST` (AC-16) |
| File dialog cancelled | no-op, no error |
| Same file re-selected after a failure | **must still fire** — a native file input emits no `change` for an identical value unless it is reset |
| Non-JSON file with a `.json` name, 0-byte file, UTF-8 BOM prefix | all are `JSON.parse` failures; the BOM case comes from files round-tripped through editors and needs a human message, not "Unexpected token" |
| Config `PATCH` fails (422 invalid `target_url`, 503, network) | zero rules created; error names the config step (AC-19) |
| Rule *k* fails (422 body-template cap **[exists at `backend/src/routes/api.rs:571-573`]**, 503, network) | rules `1..k-1` stay, `k+1..n` not attempted, nothing rolled back, message states config-applied + `k-1`/`n` + the failing index/name + server `detail` (AC-19) |
| 401 mid-import | client bounces to `/`; the partial prefix is stranded with **no record shown to the operator** |
| Tab closed / navigated away mid-import | partial prefix stranded silently |
| Export's rules fetch fails / 401 | **unspecified** — no loading or error state for Export |
| Clipboard unavailable when copying anything | `CopyButton` shows "Copied" even when `navigator.clipboard.writeText` throws **[exists at `src/components/ui/copy-button.tsx:22-31`]** — a silent lie |

### F4 — owner side
| Path | Expected |
|---|---|
| 401 on any of the three routes | `WWW-Authenticate: Bearer` (AC-29); client clears + bounces |
| 404 for a token you don't own (never 403) | AC-28 **[helper at `backend/src/auth.rs:55-69`]** |
| 11th active link → 422 `validation_error` | surfaced in the dialog (AC-27) — but the **Create** control should already be disabled at 10 with a reason |
| Label > 80 chars → 422 | needs an inline field error, not a toast — and **no label input is specified anywhere in §4.4** |
| List fetch fails (5xx/503/network) | **unspecified** — the dialog needs an error + Retry state |
| Revoke returns 404 (already revoked in another tab) | the user's intent is satisfied; must be treated as success + refresh, not rendered as a failure |
| Revoke returns 5xx | row stays, error surfaced, list not silently desynced |
| Endpoint deleted in another tab while the dialog is open | every subsequent share call 404s |
| Copy of the share URL fails silently | operator pastes stale clipboard content into a ticket and believes they shared a link (see `copy-button.tsx:22-31`) |
| `PUBLIC_BASE_URL` unset | `url` is `/s/{code}` (§5.5.2) and the SPA absolutizes with `window.location.origin` **[exists at `src/lib/url.ts:8-11`]** → on localhost / a LAN host the copied link is **unreachable for the recipient**, with no warning |

### F4 — viewer side (the surface with the most unhappy paths)
| Path | Expected |
|---|---|
| Unknown code | 404 `not_found`, byte-identical to revoked and to tombstoned (AC-36) → one "unavailable" page |
| Revoked code | same 404, immediately, uncacheable, `Cache-Control: no-store` (AC-37) |
| Endpoint deleted | same 404 |
| Code failing `^[A-Za-z0-9_-]{32,64}$` | short-circuit 404 without a DB read (§5.2) → same page |
| `/s/` with no code | **unspecified** — must render the same unavailable page (or NotFound), never a crash |
| Link revoked **while the viewer's tab is open** | the next poll 404s → terminal "unavailable" state **and polling must stop**; the currently-open detail must also stop being fetched |
| Open detail whose row was cleared by the owner (F1) or aged out (TRACE_CAP/TTL) | a **404 on the detail route only** — and it is byte-identical to a dead-code 404 (AC-35/AC-36), so the viewer page cannot tell "this one row is gone" from "the whole link is dead" |
| 429 rate limited | `Retry-After` (AC-38, `backend/src/error.rs:66-69`) → the page must show a rate-limited state **and stop polling until `Retry-After` elapses** — a fixed 5 s poll (AC-45) would hammer forever |
| Several viewers behind one NAT/office IP | one shared per-IP bucket: 60/min default, 12/min consumed per polling viewer → ~5 concurrent viewers exhaust it and everyone 429s |
| 503 `store_unavailable`, 502 from nginx, non-JSON body | generic error state + Retry; must not loop |
| Network drop / laptop sleep / offline | error or offline state; recover on `online` + `visibilitychange`, not a tight retry loop |
| Contract mismatch (`ApiError('contract_mismatch', …)` **[exists at `src/api/client.ts:128-133`]**) after a server upgrade | must render the error state, not a blank page |
| Viewer hits a `POST`/`DELETE` on the share path | 405, no mutation (AC-39) |
| `limit`/`offset` out of range (hand-edited URL) | 422 → error state, not a crash |
| Share URL opened on the **mock host** (`<token>.<MOCK_DOMAIN>/s/<code>`) | resolves to the **Mock plane** **[exists at `backend/src/planes.rs:137-148`]** → it is ingested as a webhook, and the share code is persisted into `request_logs.path`, shown in the feed, exported by F5, and shown to viewers |
| Crawler fetch | there is no `robots.txt` (nginx falls back to `index.html` for it, `deploy/nginx.conf:9-11`; no `public/` dir in the repo), no per-route `<meta name="robots">` (single `dist/index.html`), no `Referrer-Policy`, no `X-Frame-Options`/CSP → indexable and framable |

### F5
| Path | Expected |
|---|---|
| Per-row detail 404 (documented "pending" for a just-streamed trace **[exists at `src/screens/dashboard/inspector.tsx:63-66`]**) | row still emitted; the four detail cells read `pending` (AC-52) |
| Per-row 5xx / network / contract mismatch | cells read `unavailable`; export continues (AC-52) |
| 401 mid-export | the only aborting case; no file (AC-53) |
| Cancel | in-flight fetches aborted, no file (AC-48) |
| A row's body literally equals `pending` | indistinguishable from the sentinel |
| Blob/`createObjectURL` fails, or the browser blocks the download (100 rows × up to 2 × 256 KB ≈ tens of MB) | **unspecified** — needs a failure state |
| Feed rows change mid-export (arrival, cap eviction, Clear all in another tab) | **unspecified** — is the row set snapshotted at click time? |
| Operator navigates away mid-export | abort + no file |
| 429 on `GET /api/requests/{id}` if a limiter is ever added there (R6) | must honour `Retry-After` — no handling specified |

### F6
| Path | Expected |
|---|---|
| `POST` 422 / 5xx / network | danger toast, list unchanged (AC-60) |
| A catch-all already exists and is **enabled** | control disabled + tooltip (AC-61) |
| A catch-all exists but is **disabled** | AC-61's predicate says `enabled`, so the control stays live → a second catch-all is created |
| Another tab created a catch-all a second ago | the client's list is stale → the guard passes → duplicate created, no server-side guard |
| 401 | bounce |

### F7
| Path | Expected |
|---|---|
| Non-UTF-8 upstream/CLI bytes | lossy decode to U+FFFD, row still readable, client still gets raw bytes (AC-71/72) |
| Body truncated at exactly `MAX_BODY_BYTES` mid-JSON | stored without a marker (AC-70) → the Inspector's `JsonTree` fails `JSON.parse` and silently falls back to raw `<pre>` **[exists at `src/components/hookbox/json-tree.tsx:71-99`]**, with **no** "truncated"/"not JSON" affordance |
| 413 ingest-cap rejection | writes no trace at all **[exists at `backend/src/interceptor/engine.rs:65-72`]** — pre-existing, not a regression |
| Chaos dropout / 204 preflight / empty CRUD | `response_body IS NULL`, rendered as the existing empty state (AC-69) |
| A 256 KB JSON body in "Pretty" mode | `JsonTree` renders a node per key with `depth < 2` auto-expanded — now reachable for response bodies too |

---

## Edge cases

**Empty / first run**
- E1 A brand-new endpoint: feed empty → Clear all and Export CSV both disabled; Share dialog empty
  list; Rules empty state offering "Add default rule"; Settings Configuration section exporting a
  bundle with `rules: []`.
- E2 A share link minted on an endpoint that has never been hit → the viewer's **valid-but-empty**
  state, which must read differently from "unavailable" and must not look like a broken link.
- E3 A share link opened *after* the history rotated out (100-row cap / 24 h TTL / owner ran Clear
  all) → the viewer sees the same empty state as E2 and concludes the operator sent them nothing.
- E4 First-ever viewer has no idea what HookBox is: no product context, no "read-only" framing, no
  "who sent me this" affordance is specified beyond "no owner affordances" (AC-43).

**Concurrency / multi-tab / multi-actor**
- E5 Two operator tabs: tab A clears logs, tab B still shows 100 rows and an `endpoint.request_count`
  from before; tab B's Export CSV then produces 100 rows of `pending`/`unavailable`.
- E6 Tab A revokes a share link; tab B's open Share dialog still lists it; tab B's Revoke returns 404.
- E7 Tab A imports a config; tab B has unsaved Settings edits and clicks Save → the import is
  overwritten. **Worse, in the same tab:** `SettingsForm` seeds its fields with `useState(endpoint…)`
  once and is mounted without a `key`
  **[exists at `src/screens/settings.tsx:137-143` and `:162-170`]**, so the post-import re-fetch does
  **not** refresh the visible fields, and the next Save `PATCH`es the pre-import values — silently
  reverting the import.
- E8 Owner clears logs while a viewer is mid-poll → viewer list empties, open detail 404s.
- E9 Owner deletes the endpoint while a viewer has the tab open → all share calls 404 forever.
- E10 A rule is created by F6 in one tab while the RuleBuilder is open in another.
- E11 Two simultaneous mints racing against `SHARE_MAX_PER_ENDPOINT` (10).

**Volume / limits**
- E12 CSV export of 100 rows each with a 256 KB request body and a 256 KB response body (F7) →
  ~50 MB of strings plus the Blob, on a phone.
- E13 Import of 200 rules → 201 sequential requests, no cancel, no progress persistence.
- E14 A viewer paging with `limit=200` (max) against a 100-row `TRACE_CAP` → `offset` beyond the data
  returns an empty list, not a 404; the pager must handle it.
- E15 A scanner sprays `/s/<random>`: each unique client IP creates a limiter bucket; the map evicts
  the most-idle entry past 100 000 **[exists at `backend/src/limiter.rs:88-89`, `:130-142`]** and the
  limiter **fails open** on anomaly (`limiter.rs:9-10`), so a legitimate viewer's bucket can be
  evicted and reset.
- E16 A `mitm` response of up to `MITM_MAX_BODY_BYTES` = 5 MB **[exists at
  `backend/src/config.rs:177`]** becomes an extra transient in-memory copy per concurrent request
  (R4) and up to 256 KB persisted per row.
- E17 Feed at exactly 100 rows: the oldest row is evicted client-side mid-export.

**Narrow viewport / a11y**
- E18 The feed header would carry: title, count, "N new" pill, Clear all, Export CSV, Pause — plus
  the sub-header gaining Share next to the Mock URL chip, the Auto-CRUD chip, the tunnel badge, the
  connection pill, Rules, Settings and New rule. No overflow/responsive rule is specified.
- E19 Keyboard-only operator: the confirm dialog, the Share dialog, the progress dialog and the file
  input all need reachable, labelled, focus-trapped behaviour (AC-65) — including focus return to
  the triggering control after each dialog closes.
- E20 Screen-reader operator during a long export/import: progress must be announced politely
  (`role="status"`/`aria-live`), not silently.

**Cross-feature interactions the PRD never names**
- E21 **F6 shadows Auto-CRUD, tunnel and MITM.** A matched rule short-circuits the engine
  **[exists at `backend/src/interceptor/engine.rs:141-145`]**; Auto-CRUD, tunnel forwarding, MITM
  and the `default_mode` answer are all only reached in the `else` branch via `resolve_unmatched`
  **[exists at `backend/src/interceptor/engine.rs:228-245`, `:391-404`]**. Priority orders rules
  against *each other* only. So "Add default rule" on an endpoint with `auto_crud = true`, a
  `target_url`, or an active tunnel **silently turns all of that off**.
- E22 **F6 shadows `default_mode: "echo"`** for the same reason — echo becomes unreachable.
- E23 **F1 vs F5** — clearing during an export turns every remaining row into a sentinel.
- E24 **F1 vs the Inspector** — the selected row is deleted; because its id may be in `liveIds`
  **[exists at `src/screens/dashboard.tsx:203`, `:219-227`]**, the 404 renders as
  "pending… Retry" **[exists at `src/screens/dashboard/inspector.tsx:63-66`]** forever instead of
  "gone".
- E25 **F1 vs Settings' counter** — `endpoint.request_count` feeds the clear-history confirm copy
  **[exists at `src/screens/settings.tsx:449`]**; after a dashboard Clear all it lies until re-fetch.
- E26 **F3 vs F4** — importing a bundle does **not** import share links (correct), but importing a
  `target_url` re-points MITM, so an existing share link starts publishing a *different upstream's*
  response bodies and headers (un-redacted, R11) to whoever already holds the link.
- E27 **F7 vs F4** — every existing share link's exposure widens the moment F7 ships: response
  bodies that were `null` start carrying content, including anything an operator hard-coded in a
  `body_template` and any upstream `Set-Cookie`/token in a response header (R11). Links minted
  *before* F7 were consented to under the narrower projection.
- E28 **F2 vs the test suite** — the sub-header's loading skeleton renders one placeholder chip per
  `UrlChip` **[exists at `src/components/hookbox/app-shell.tsx:161-166`]**; removing one changes
  visual snapshots and any chip-count assertion in `e2e/visual.spec.ts` / `e2e/states.spec.ts`.

---

## Required states

For each screen: loading · empty · error · success (plus the extra states the surface needs).

### `/d/:token` — Live Feed header (F1, F5)
| State | Requirement |
|---|---|
| loading | existing skeleton **[`src/screens/dashboard.tsx:355-359`]**; Clear all + Export CSV **disabled** (nothing to act on, and `rows` is not yet authoritative) |
| empty | existing `feed.empty.*`; both controls disabled |
| success | both enabled; enable predicate must account for the paused buffer and for server-side rows the client hasn't loaded |
| confirm (F1) | dialog open; confirm button `loading` and disabled while the DELETE is in flight |
| error (F1) | dialog stays open + danger toast + rows intact |
| exporting (F5) | determinate progress "Exporting {done} of {total}…", Cancel, both controls + Clear all disabled, live-arrival policy stated |
| export partial | completion toast "Exported {n} rows ({m} without detail)" |
| export failed | a real failure state for "no file was produced" (Blob/download failure, 401 abort) |
| offline | both controls disabled or clearly failing; the existing offline banner is already shown |

### `/d/:token` — Inspector (F7 consumer)
| State | Requirement |
|---|---|
| existing | empty · loading · pending · unauthorized · error · ready **[`src/screens/dashboard/inspector.tsx:36-42`]** |
| response body populated | new default for the Response tab |
| response body `NULL` | keep `insp.response.empty` |
| response body not JSON | `JsonTree` falls back to raw `<pre>`; needs a visible "not JSON" hint |
| response body truncated | needs a "truncated at 256 KB" hint (AC-70 stores no marker, so this must be UI-side or accepted as invisible) |
| row deleted under the selection | must resolve to a "gone" state, not perpetual "pending" |

### `/d/:token/settings` — Configuration section (F3)
| State | Requirement |
|---|---|
| loading | the section is inside the existing screen-level loading skeleton **[`src/screens/settings.tsx:110-117`]** |
| export idle / in flight | Export must have its own busy state (it fetches rules) |
| export error | rules fetch failed / 401 |
| import idle | file picker; both controls enabled |
| import validating | pre-flight validation of the file (can be large: 5 MB cap) |
| import invalid | inline, specific message naming the first failing field/index |
| import in flight | "Creating rule {i} of {n}…", both controls disabled, a cancel or a "don't close this tab" warning |
| import partial failure | persistent (not toast-only) report: config applied? `k-1`/`n` rules created, which index/name failed, server `detail`, "nothing after it was attempted" |
| import success | success message + counts + a form that actually reflects the new server state |
| unsaved-edits conflict | explicit guard before an import overwrites live config while the form is dirty |

### Share dialog (F4 owner) **[NEW]**
| State | Requirement |
|---|---|
| loading | list fetch in flight |
| empty | "no active links" + Create + exposure disclosure |
| list | rows: created-at, label, URL (selectable), copy, **last used**, Revoke |
| creating | Create disabled + busy |
| created | the new URL highlighted, copy affordance, copy-failure fallback |
| at cap | Create disabled with "10 of 10 active links — revoke one first" (not just the 422 after the fact) |
| label invalid | inline field error (≤ 80 chars) |
| error | list/create/revoke failure + Retry; a 404 on revoke treated as success |
| revoking | row busy; a confirm step, because revoke is permanent and breaks an already-distributed URL |

### `/s/:code` — public viewer (F4 viewer) **[NEW]**
| State | Requirement |
|---|---|
| loading | first fetch; no session check, no redirect, no `Authorization` header |
| empty (valid link, no traffic yet) | must be visibly different from "unavailable" and must explain that only the **last 100 requests / 24 h** are ever shown |
| list | read-only summaries, newest first, no owner affordances |
| detail loading / detail ready | read-only inspector, all values as text nodes |
| detail gone | the selected row no longer exists (cleared/aged out) — **distinct from** link-unavailable |
| unavailable (404) | one message covering unknown/revoked/deleted; **terminal — polling stops** |
| rate limited (429) | show `Retry-After` seconds; **polling pauses for at least that long** |
| error (other) | message + Retry; bounded backoff, no hammering |
| offline / hidden tab | polling suspended; resumed on `online` / `visibilitychange` |
| unreachable-origin warning (owner-side) | when the minted URL is origin-relative and the instance has no `PUBLIC_BASE_URL`, tell the *operator* at mint time, not the viewer at open time |

### `/d/:token/rules` (F6)
| State | Requirement |
|---|---|
| existing | loading · empty · error+Retry · list **[`src/screens/rules-manager.tsx:54-57`]** |
| add-default idle | enabled in both toolbar and empty state |
| add-default busy | disabled while the POST is in flight (no double-create) |
| add-default disabled | tooltip explaining an enabled catch-all already exists |
| add-default warning | a warning (or a confirm) when `auto_crud`, `target_url` or `tunnel_active` means the catch-all will shadow them (E21) |
| add-default error | danger toast, list unchanged |

---

## PRD gaps

Numbered, actionable, severity-tagged. **BLOCKER** = the implementation cannot be built correctly
from the current text; **HIGH** = a real user will hit a broken/undefined path; **MEDIUM** = a state
or decision is missing and will be improvised.

### F4 — public share links (highest risk)

1. **BLOCKER — the viewer cannot distinguish "this row is gone" from "this link is dead."**
   AC-36 requires unknown/revoked/tombstoned codes to return a *byte-identical* 404, and AC-35
   requires an unknown or foreign request id to return **404** as well. A single stale detail click
   (row cleared by F1, or aged out by `TRACE_CAP`/`TRACE_TTL_HOURS`
   **[`backend/src/config.rs:156-157`]**) therefore looks exactly like a revoked link, so the viewer
   page will render the terminal "unavailable" state and stop. Add an AC that fixes the client rule
   (a detail 404 is *never* terminal; only a **list** 404 is), and state the required UX for
   "detail gone". If a distinguishable signal is wanted instead, that is a §5.2 contract change and
   must be made explicitly (it weakens AC-36).

2. **BLOCKER — the viewer's polling lifecycle is specified only for the happy path.** AC-45 gives a
   fixed 5 s interval gated on document visibility, and nothing else. Add ACs for: stop permanently
   on a list 404; pause for at least `Retry-After` on 429 (a fixed 5 s poll against a 60/min bucket
   never recovers); exponential backoff with a ceiling on 5xx/network; resume on `online` and on
   `visibilitychange` to visible; no overlapping in-flight polls; abort in-flight fetches on unmount.

3. **HIGH — no mint-time exposure disclosure, and no consent moment.** R2 says ux.md "should make
   that consequence explicit", which leaves it optional. Given F7, minting now publishes request
   bodies, request headers, **un-redacted response headers** (R11) and response bodies to anyone with
   the URL. Add an AC: the mint step names exactly what becomes public, before the link exists.

4. **HIGH — the label has no journey.** `ShareLinkCreate` carries `label`, §5.1 returns 422 for
   `label > 80 chars`, and AC-25 renders "an optional label" — but AC-23/AC-24 describe no input and
   no validation path. Add: where the label is entered, its inline error state, and what an empty
   label renders as in the list.

5. **HIGH — revoke has no confirm, no undo, no toast, and no 404-is-success rule.** A revoke is
   permanent (§5.1: soft revoke, never re-mintable) and instantly breaks a URL already pasted into a
   ticket. AC-26 only asserts 204 + row removal. Add: a confirm naming the label/URL, a success
   toast, an explicit "a 404 means it is already revoked — treat as success and refresh" rule, and
   the 5xx path (row must not silently disappear).

6. **HIGH — `last_used_at` is stored but never shown, and its write path is unspecified.** §5.4 and
   §5.5.2 both carry it; AC-25's row spec omits it. It is the operator's only "is anyone actually
   reading this?" signal. Also decide explicitly whether an unauthenticated request triggers a DB
   write on every resolve (that hands an anonymous caller a write primitive and contradicts the
   no-new-I/O posture applied to F7 in AC-73) — e.g. throttle to once per minute per code, or drop
   the column. Add both ACs.

7. **HIGH — nothing stops a share page from being indexed, framed, or leaking its code via
   `Referer`.** There is no `robots.txt` in the repo (nginx's `try_files` serves `index.html` for it,
   `deploy/nginx.conf:9-11`), `dist/index.html` is a single document with no per-route meta and a
   constant `<title>HookBox</title>`, and `deploy/nginx.conf` sets no `Referrer-Policy`,
   `X-Frame-Options` or CSP. §5.9.3 only covers access logs. Add ACs for: `noindex, nofollow` on
   `/s/:code` (meta and/or `X-Robots-Tag`), a real `robots.txt` disallowing `/s/`,
   `Referrer-Policy: no-referrer` on the share page, and a framing decision (the non-goal says no
   embed support — make that enforced, not just unimplemented).

8. **HIGH — the copy affordance can lie.** `CopyButton` calls `setCopied(true)` **outside** the
   try/catch **[`src/components/ui/copy-button.tsx:22-31`]**, so when `navigator.clipboard` is
   unavailable it still says "Copied". The shipped nginx listens on port 80 only
   **[`deploy/nginx.conf:1-2`]**, i.e. a non-secure context where the Clipboard API is unavailable —
   exactly the deployment where the operator copies a share URL. Add an AC: the share URL is always
   rendered as selectable text, and a copy failure is surfaced (this also fixes F3's export/paste
   flows and the Mock URL chip).

9. **HIGH — "the link I copied doesn't work for the recipient" has no state.** With
   `PUBLIC_BASE_URL` unset, §5.5.2 returns `/s/{code}` and the SPA absolutizes with
   `window.location.origin` **[`src/lib/url.ts:8-11`]**, producing `http://localhost:5173/s/…` — a
   link nobody else can open. Add an AC: when the resolved share URL is not externally reachable
   (no `PUBLIC_BASE_URL`), the mint step says so.

10. **HIGH — no owner-side states for empty / error / at-cap.** AC-23–AC-27 describe the list and the
    11th-POST 422 only. Add: dialog loading, empty ("no active links"), list-fetch error + Retry,
    Create disabled at `SHARE_MAX_PER_ENDPOINT` with a reason, and create/revoke busy states.

11. **MEDIUM — the rate limit's key, its interaction with the poll interval, and NAT are
    unspecified.** AC-38 says "per client IP" at 60/min. A single polling viewer consumes 12/min, so
    ~5 viewers behind one office IP exhaust the bucket and all see 429s. State the bucket key
    (IP only, or IP+code), justify 60/min against the 5 s poll, and note that the limiter fails open
    and evicts idle buckets past 100 000 **[`backend/src/limiter.rs:9-10`, `:88-89`, `:130-142`]**,
    so it is a courtesy limit, not a guarantee.

12. **MEDIUM — a share URL opened on the mock host becomes a webhook.** `<token>.<MOCK_DOMAIN>/s/<code>`
    resolves to the Mock plane **[`backend/src/planes.rs:137-148`]**, so the share **code** is
    ingested and persisted into `request_logs.path` — then displayed in the feed, exported by F5, and
    shown to viewers of that same link. Add an AC covering the expected behaviour and whether trace
    paths matching the share-code shape must be scrubbed.

13. **MEDIUM — the viewer's empty state carries a second meaning nobody has written.** AC-44's
    "empty (share valid, no requests yet)" also fires when the history rotated out (100-row cap,
    24 h TTL) or the owner ran F1. Add: copy that states the window ("the last 100 requests, up to
    24 hours"), so a recipient opening the link tomorrow does not conclude the link is broken. Also
    add the owner-side counterpart at mint time.

14. **MEDIUM — no first-run orientation for the viewer.** AC-43 defines the viewer page by *absence*.
    A cold recipient needs: what this page is, that it is read-only, that it is a snapshot of one
    endpoint's recent traffic, and that they cannot act on it. Add ACs for the page's own explanatory
    header and `<title>`/heading (the current `dist/index.html` title is a constant "HookBox").

15. **MEDIUM — viewer navigation/deep-linking is undefined.** Does selecting a row change the URL
    (`/s/:code?r=123`)? Is there a back-button contract? What renders for `/s/` with no code, or a
    code of the wrong shape (the server 404s without a DB read, §5.2)? Add ACs.

16. **MEDIUM — F7 silently widens every link minted before it ships.** Links consented to under
    "response_body is always null" start publishing real response bodies the moment F7 lands (E27).
    Decide and record: ship F7 first and accept it, or revoke pre-existing links, or notify the
    operator. This is a PM decision, not an architecture one.

### F1 — clear all logs

17. **HIGH — the disabled predicate is wrong in three real states.** AC-1 disables on
    `rows.length === 0`. But (a) while paused, `rows` can be empty while `buffer`/`newCount` hold
    arrivals **[`src/feed/use-feed.ts:74`, `:77`]**; (b) if the reconcile failed or the tab is
    offline, `rows` is empty while the server holds rows; (c) `request_count > 0` with an evicted
    view. Define the predicate against server state (or `request_count`), not the visible list.

18. **HIGH — the confirm copy must say the action is not scoped to the visible 100.** The route
    deletes **all** rows for the endpoint **[`backend/src/routes/api.rs:884-887`]** while the feed
    shows at most 100 of them. Add an AC on the confirm wording.

19. **MEDIUM — the selected row and `liveIds` are not cleared.** After Clear all, the open Inspector
    detail 404s; if the id is in `liveIds` **[`src/screens/dashboard.tsx:203`, `:219-227`]** it
    renders "pending… Retry" forever **[`src/screens/dashboard/inspector.tsx:63-66`]**. Add an AC:
    Clear all resets the selection and the live-id set.

20. **MEDIUM — `request_count` goes stale.** It feeds the Settings clear-history confirm copy
    **[`src/screens/settings.tsx:449`]** and any future counter. Add an AC: re-fetch
    `GET /api/endpoints/{token}` (or decrement locally) after a successful clear.

21. **MEDIUM — the arrival race has no defined end state.** A `new_request` can land between the
    confirm click and the 200. Specify whether the post-clear feed is asserted empty or
    "empty except arrivals after the DELETE", so AC-4's "immediately renders its empty state" is not
    flaky in the e2e suite.

22. **MEDIUM — 404/410 mid-session is unhandled.** The shell handles 404/410 only on the initial load
    **[`src/screens/dashboard.tsx:73-84`]**. Define what Clear all (and Export CSV, and Share) do
    when the endpoint was deleted in another tab.

### F3 — export / import config

23. **BLOCKER — import does not update the form the operator is looking at, and the next Save
    reverts it.** `SettingsForm` seeds all nine fields with `useState(endpoint…)` and is mounted
    without a `key` **[`src/screens/settings.tsx:137-143`, `:162-170`]**, so AC-20's "the Settings
    form … reflect the server state" is not achievable by re-fetching alone. Add an explicit AC (remount
    via `key`, or lift/reset the field state) **and** an assertion that a Save immediately after an
    import does not revert the imported values.

24. **HIGH — import overwrites nine live config values with no confirm, no preview, no undo.**
    `target_url` re-points MITM and `auto_crud` changes the mock plane's behaviour immediately, on a
    screen where every other mutation requires an explicit Save and every destructive op has a
    confirm **[`src/screens/settings.tsx:444-479`]**. Add: a confirm/preview step listing what will
    change (and how many rules will be *added*), plus a dirty-form guard (E7).

25. **HIGH — Export has no states.** It must fetch the rules list, which the Settings screen does not
    fetch today. Add ACs for export busy, export error (5xx/network), 401, and the 0-rules case.

26. **MEDIUM — file-input mechanics are unspecified.** Re-selecting the *same* file after a rejection
    fires no `change` event unless the input value is reset; a cancelled dialog must be a no-op; a
    0-byte file, a mislabelled non-JSON file, and a leading UTF-8 BOM (common after a round-trip
    through an editor) all need human messages rather than a raw parser error. Add ACs.

27. **MEDIUM — a 200-rule import has no cancel and no durable report.** AC-19's precise error report
    is transient UI; a closed tab or a 401 mid-run strands a prefix with nothing to tell the operator
    what landed. Add: a cancel affordance (stop before rule *k*, report the prefix), an
    unload/navigation guard, and a report the operator can re-read (e.g. keep it until dismissed).

28. **MEDIUM — repeated import silently duplicates.** AC-18 is deliberate, but a double-click or a
    re-import doubles the rules with no warning. Add: idempotent-click protection and a warning when
    the bundle's rules look already present.

29. **MEDIUM — cross-screen reflection.** After an import the operator is on Settings; the rules
    result lives on another screen. Add an AC: the success message states "n rules added" and links
    to the Rules Manager.

### F5 — CSV export

30. **HIGH — the exported row set is not defined as a snapshot.** AC-47 says "every row currently
    visible", but rows change during the export (arrivals prepend, the 100-cap evicts, another tab or
    F1 clears). Add an AC: the row set and its order are snapshotted at activation, `total` is fixed,
    and later arrivals are excluded.

31. **HIGH — F5 and F1 must be mutually exclusive.** Clearing during an export turns every
    outstanding row into `pending`/`unavailable` and produces a file that looks corrupt (E23). Add an
    AC: Clear all is disabled while an export is in flight (and vice versa).

32. **MEDIUM — the paused buffer's exclusion is a surprise.** State explicitly that buffered
    ("N new") rows are not exported, or flush them first.

33. **MEDIUM — no failure state for "no file was produced" other than 401.** Blob/`createObjectURL`
    failure, a browser-blocked download, or an out-of-memory serialise (E12: ~50 MB of body text once
    F7 lands) are all unspecified. Add an error state and consider a row/byte budget.

34. **MEDIUM — the `pending` / `unavailable` sentinels can collide with real body content.** A
    request body of exactly `pending` is indistinguishable. Either document the ambiguity in §5.6 or
    use a shape that cannot occur naturally.

35. **MEDIUM — no per-row timeout and no 429 handling.** A hung detail fetch leaves the progress
    dialog stuck with only Cancel; R6 assumes no limiter on `GET /api/requests/{id}` today. Add a
    per-row timeout that maps to `unavailable`, and a `Retry-After` path.

### F6 — default catch-all rule

36. **BLOCKER — the catch-all silently disables Auto-CRUD, the tunnel, MITM and `default_mode`.**
    A matched rule returns before any of them: `resolve_unmatched` (Auto-CRUD → tunnel → MITM →
    default) is only reached in the `else` branch
    **[`backend/src/interceptor/engine.rs:141-145` vs `:228-245`, `:391-404`]**. `priority = 1000`
    orders rules against *each other* and gives no protection here. R8 only mentions "404 becomes
    200". Add ACs: the control warns (or confirms) when `auto_crud` is true, `target_url` is set, or
    `tunnel_active` is true; the tooltip/copy names the consequence; and a regression test asserts an
    endpoint with `auto_crud` + a catch-all is served by `rule`, not `crud` — so the behaviour is
    chosen rather than discovered in production.

37. **MEDIUM — the duplicate guard is incomplete.** AC-61 keys on an **enabled** `ANY` `/*` rule, so a
    *disabled* catch-all lets a second one be created, and the check runs against a possibly-stale
    client list (two tabs) with no server-side guard. Add: the predicate for disabled catch-alls, the
    stale-list path (refresh + a "you already have one" message instead of a silent duplicate), and a
    busy state so a double-click cannot POST twice.

38. **MEDIUM — `default_mode: "echo"` becomes unreachable.** Same mechanism as gap 36. Add a warning
    for endpoints configured with `echo`.

### F7 — response-body capture

39. **HIGH — "zero frontend changes" is true for rendering but not for meaning.** F7 makes
    non-JSON, lossy-UTF-8 and silently truncated bodies reachable in the Inspector, the CSV and the
    public viewer for the first time. `JsonTree` degrades to a raw `<pre>`
    **[`src/components/hookbox/json-tree.tsx:71-99`]** with no "not JSON" and no "truncated at
    256 KB" affordance, and "Pretty" mode renders a DOM node per key for a 256 KB body. Add ACs for a
    not-JSON hint, a truncation hint (or an explicit accepted-invisible decision beyond R4's
    developer-facing note), and a render budget for very large bodies.

40. **MEDIUM — the truncation heuristic is developer-only.** R4 offers `len == MAX_BODY_BYTES` as a
    heuristic; nothing surfaces it to an operator or a viewer. Either surface it or state that the
    ambiguity is accepted user-facing behaviour in three places (Inspector, CSV, share viewer).

### Cross-cutting

41. **HIGH — no offline/degraded contract for the new controls.** The dashboard already renders an
    offline banner **[`src/screens/dashboard.tsx:264-274`]**. Specify what Clear all, Export CSV,
    Share (mint/list/revoke) and Import do while `navigator.onLine === false` — disabled, or attempted
    and failed with the network message **[`src/api/client.ts:93-95`]**.

42. **MEDIUM — no responsive/overflow rule for two now-crowded headers.** The feed header gains two
    controls and the sub-header gains Share (E18). Specify the narrow-viewport behaviour (overflow
    menu? icon-only with `sr-only` labels, as the Settings link already does
    **[`src/components/hookbox/app-shell.tsx:124-136`]**?).

43. **MEDIUM — announcement/focus behaviour for long-running operations is unstated.** AC-65 covers
    names, keyboard reach and focus trapping; it does not cover `aria-live` progress announcements
    for import/export, nor focus restoration after the progress dialog auto-closes (E19/E20).

44. **MEDIUM — F2's fallout on tests and copy parity is only half-covered.** AC-10 covers unused copy
    keys and `pnpm typecheck`; add the visual/state snapshot updates for the one-chip sub-header and
    its loading skeleton **[`src/components/hookbox/app-shell.tsx:161-166`]**, and an AC that no
    copy-table parity check fails on two intentionally unreferenced keys (E28).

45. **MEDIUM — no discovery path replaces the removed chip.** AC-8 keeps "Local path" on Settings but
    nothing points an operator there. Either accept it explicitly as a journey regression (R9 already
    accepts the removal, not the discoverability) or add a pointer in the Mock URL chip's tooltip.
