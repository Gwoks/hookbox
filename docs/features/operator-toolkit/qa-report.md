# QA report — operator-toolkit (round 6)

**Gate:** `hookbox-mun.21` · **Feature:** operator-toolkit · **PRD:** `docs/features/operator-toolkit/prd.md`
(FROZEN — 165 ACs in §4, contract in §5) · **Date:** 2026-08-08 · **Tree under test:** `0b01a01`,
**clean working tree** (`git status --porcelain` = `?? docs/features/operator-toolkit/qa-report.md` only).

**Verdict: NOT PASSED.** **164 of 165 ACs pass; 1 fails** — **AC-43**. Round 5's defect
`hookbox-mun.34` fixed **two of three** token-disclosure channels; a **third** is still open and is the
same live write capability. Filed as **`hookbox-mun.36` (backend)**. The gate stays open, re-blocked on it.

**Round-5's two defects are re-verified:**

| Round-5 defect | Bug | Status | Round-6 evidence |
|---|---|---|---|
| D1 channel A — a CORS echo of a wildcard `Origin` survived the name-based response-header filter | `hookbox-mun.34` | **FIXED** | Live, anonymous, `Origin: https://<token>.mock.local`: `access-control-allow-origin` now returns `<redacted>` on **2/2** affected rows. `mask_token_in_value` (`backend/src/routes/share.rs:96-101`) + unit test `filter_response_headers_masks_any_value_containing_the_token` |
| D1 channel B — `request_headers.host` structurally carried the token in wildcard mode | `hookbox-mun.34` | **FIXED** | Live: `host`/`origin`/`referer` are **absent** from public `request_headers` on **every** row; other values containing the token are masked. `filter_public_request_headers` (`share.rs:253-273`) + unit test `filter_request_headers_drops_host_origin_referer_and_masks_token_value`. Owner route on the **same row** still shows `host: <token>.mock.local` verbatim (§5.11 asymmetry intact) |
| **D1 channel C — the echo-mode `response_body`** | **NEW: `hookbox-mun.36`** | **STILL OPEN** | See §5 D1 |
| D2 — the AC-66 de-flake was uncommitted | `hookbox-mun.35` | **FIXED** | The de-flake ships on HEAD (`5faea5a`); the tree is clean; `cargo test` **10/10** green (160 tests) |

---

## 1. How round 6 was validated

Every FAIL and every gate-owned AC was re-verified **dynamically this round** against the real server
and the real built SPA. Verdicts are marked **[R6]** where re-run this round, **[R5-carry]** where
round 5's evidence is carried forward on code that is byte-identical (`git diff 7d3685b HEAD` touches
only `backend/src/routes/share.rs` and `backend/tests/api.rs`).

| Harness | Result |
|---|---|
| `pnpm typecheck` | clean (exit 0) |
| `pnpm build` | clean (exit 0) |
| `cargo fmt --check` | clean (exit 0) |
| `cargo clippy --all-targets -- -D warnings` | clean (exit 0) |
| `cargo test` × 10 (160 tests: 109 unit + 51 integration) | **10/10 green**, 0 failures — no flake |
| `pnpm e2e` (Playwright, chromium + reduced-motion) | **118 passed** in 20.0 s |
| **Live token-disclosure sweep** — real `backend/target/debug/hookbox` on `:8099`, fresh DB, `MOCK_DOMAIN=mock.local`, `PUBLIC_BASE_URL=http://localhost:8099` | 19 checks: **3 failed on first run** → 2 were my own harness (wrong row picked), **1 is real (channel C)** |
| **`served_by` matrix through the public projection** (`crud`, `chaos`, `ratelimit`, `cors`, `rule`, `default`/404, `default`/echo) | token absent on **every** path **except `default`/echo** |
| **Channel-C exploit harness** (anonymous, share code only) | **3/3** — token recovered, used to write twice into the endpoint, `request_count` 1 → 3, injected rows visible in the shared feed |
| **Live anonymous viewer walk** — real headless Chromium, real `dist/` SPA, real backend, clean context, every row × every tab | **22 checks, 20 pass / 2 fail** (both are channel C in the DOM) |
| **Live owner journey walk** — real Chromium driving the real SPA against `:8099` (F1 · F2 · F3 export+import+5 malformed bundles · F4 owner mint/list/revoke · F5 real CSV download · F6) | **47 checks, 47 pass** |
| **Live §5 contract sweep** (owner routes #19-#21, public routes #22-#23, models, config, deploy) | **51 checks, 49 pass / 2 "fail"** — both re-adjudicated as harness over-strictness (§2 note) |
| Rate limits on a **fresh** instance (`:8098`, fresh DB) | per-IP first 429 at request **#121**, `retry-after: 1`, `cache-control: no-store`, body `{"detail":"Too many requests. Please slow down.","error":"rate_limited"}` |
| AC-73 latency harness (`:8098`, fresh) | 200 sequential 64 KB-template mock requests: median **1016 µs**, p95 **1390 µs**; `overhead_ms` bucket 0 for **100/100** rows; TRACE_CAP prune holds at 100; `request_count = 200`; stored body length exactly 64 000 |

**Contract integrity was proved on both sides simultaneously.** The SPA was driven against the **real**
server (never `e2e/mock-backend.ts`): the viewer, the Share dialog, the CSV export and the config
export/import all ran against `:8099`, so every `zod` schema in `src/api/schemas.ts` validated live — a
shape drift would have thrown `contract_mismatch` at `parseJsonResponse`. In the same pass the server
side was asserted with exact key-set equality. **No shape deviation was found on either side.** The one
defect is a *value* defect inside a correct shape.

---

## 2. Per-AC verdicts

### 4.1 — F1 Clear all logs

| AC | Verdict | Evidence |
|---|---|---|
| AC-1 | PASS **[R6]** | Live browser: exactly **1** `aria-label="Feed actions"` trigger; menu items `["Export CSV","Clear all"]` in that order, destructive last |
| AC-2 | PASS **[R6]** | Live confirm: title "Clear all requests?", explanatory body, ghost Cancel, `variant="danger"` "Clear all" |
| AC-3 | PASS **[R6]** | Live: open menu → no writes issued; server still holds **2** rows |
| AC-4 | PASS **[R6]** | Live: confirm → server `GET …/requests` = `[]` **and** the pane renders "No requests yet" |
| AC-5 | PASS **[R6]** | e2e "AC-4/AC-5/AC-78/AC-79: confirming clears the feed, empties the paused buffer, and resets selection" green |
| AC-6 | PASS **[R6]** | e2e "AC-6: a server failure keeps the dialog open, shows the detail, and removes no rows" green |
| AC-76 | PASS **[R6]** | e2e "AC-76: paused with an empty visible list but a buffered arrival still enables Clear all" green |
| AC-77 | PASS **[R6]** | Live confirm body verbatim: `Deletes every request captured for "Errors" — not only the ones listed here. The feed starts fresh, and this can't be undone.` — endpoint named, wider blast radius, no count |
| AC-78 | PASS **[R6]** | Live: after the clear the Inspector reads "Select a request" |
| AC-79 | PASS **[R6]** | e2e (same test as AC-5); the screen re-fetches `GET /api/endpoints/{token}` after the DELETE |
| AC-80 | PASS **[R6]** | Live confirm renders "Requests that arrive after this show up as normal." |
| AC-81 | PASS **[R6]** | e2e "410 → distinct 'Endpoint deleted' card (not the 404 copy)" green |
| AC-82 | PASS **[R6]** | e2e "AC-82: Clear all is disabled for the whole export" green |
| AC-83 | PASS **[R6]** | e2e "J9b: settings → clear-history confirm shows a server failure inline and stays open (ConfirmDialog, AC-83)" green |
| AC-84 | PASS **[R5-carry]** | `src/components/ui/menu.tsx` `destructive` prop carries both `focus:` overrides, applied at the feed item **and** the rules Delete item |

### 4.2 — F2 remove the Local path chip

| AC | Verdict | Evidence |
|---|---|---|
| AC-7 | PASS **[R6]** | Live sub-header copy affordances = `["Copy"]`, zero "Local path"; e2e "AC-7" green |
| AC-8 | PASS **[R6]** | Live Settings → Identity still renders **`Mock URL`**, **`Local path`** and Token |
| AC-9 | PASS **[R6]** | e2e "AC-9: path_url is still a required string on endpointDetailSchema" green; live `GET /api/endpoints/{token}` returns `path_url` |
| AC-10 | PASS **[R6]** | `grep -rn dash.pathUrl src/` → **2 hits, both the definitions in `src/lib/copy.ts:69-70`**, zero call sites; `pnpm typecheck` clean |
| AC-85 | PASS **[R6]** | `pnpm e2e` 118/118, no snapshot churn |
| AC-86 | PASS **[R5-carry]** | `app-shell.tsx` passes `tooltip={t("dash.mockUrl.tooltip")}` into `UrlChip` |

### 4.3 — F3 export / import config

| AC | Verdict | Evidence |
|---|---|---|
| AC-11 | PASS **[R6]** | Live Settings renders a "Configuration" section with `Export config` / `Import config…` |
| AC-12 | PASS **[R6]** | Live download `hookbox-config-RB72BxP5DV.json`; `src/lib/download.ts` revokes the object URL in a `finally` |
| AC-13 | PASS **[R6]** | Live bundle: `hookbox_config_version: 1`, `exported_at`, endpoint key set **exactly** `auto_crud,chaos_mode,chaos_pct,cors_enabled,default_mode,latency_ms,name,rate_limit_per_min,target_url` |
| AC-14 | PASS **[R6]** | Live rule key set = `name,priority,enabled,match,response,state_writes,latency_ms,rate_limit_per_min,chaos_mode,webhook_action` — no `id`/`token`/`created_at` |
| AC-15 | PASS **[R6]** | Live round trip into a second endpoint: `name/latency_ms/chaos_pct` = `Journey source/250/7`, rules deep-equal in order |
| AC-16 | PASS **[R6]** | Live, real backend, **zero** writes for each of: malformed JSON · version 2 · unknown top-level key · **nested** unknown key · 0-byte. Each renders its own human message (e.g. `isn't valid JSON. If you edited it by hand, check for a missing brace or a trailing comma.`) |
| AC-17 | PASS **[R6]** | Live write log after confirm: `["PATCH /api/endpoints/{T2}", "POST /api/endpoints/{T2}/rules"]` — one PATCH then one POST per rule, in array order |
| AC-18 | PASS **[R6]** | Live: 1 pre-existing rule + 1 imported = **2**; add-never-replace |
| AC-19 | PASS **[R6]** | e2e "AC-19: a partial failure (rule 2 of 2 fails) creates rule 1, never attempts rule 2, and reports all five facts" green |
| AC-20 | PASS **[R6]** | e2e "AC-S21/17/18/89" green; both controls disabled while applying |
| AC-21 | PASS **[R5-carry]** | Body-template-cap 422 handled through AC-19's path |
| AC-22 | PASS **[R6]** | Asserted on the downloaded bytes: no token, no owner secret, no `code`/`share` key, no `/s/` |
| AC-87 | PASS **[R6]** | e2e "AC-87: export uses freshly fetched server state, not the dirty in-memory form" green |
| AC-88 | PASS **[R5-carry]** | Forced 503 on `GET …/rules`: message renders **inside the section**, **no download** |
| AC-89 | PASS **[R6]** | Live: **(a)** after the import the form shows `Journey source\|250\|7`. **(b)** the very next Save PATCHes `{"name":"Journey source",…,"latency_ms":250,…,"chaos_pct":7,…}` — the **imported** values |
| AC-90 | PASS **[R6]** | e2e "a leading UTF-8 BOM is stripped, not surfaced as a parser error", "malformed JSON is worded for a hand-editing human", "an invalid rule names its 1-based index" all green |
| AC-91 | PASS **[R5-carry]** | Partial-failure report persists past 4 s and carries a Dismiss |
| AC-92 | PASS **[R5-carry]** | `settings.tsx` renders the `peer sr-only` file input **before** the label, with `peer-focus-visible:outline*` |

### 4.4 — F4 share links

| AC | Verdict | Evidence |
|---|---|---|
| AC-23 | PASS **[R6]** | Live: `Share` control (`aria-label="Share this endpoint read-only"`) opens "Share a read-only link"; e2e "AC-23/98: the Share control is first in the action cluster" green |
| AC-24 | PASS **[R6]** | Live: the full `http://localhost:8099/s/<code>` revealed once with the "Shown once" hint; the 201 is the only body carrying `code`/`url` |
| AC-25 | PASS **[R6]** | Live `GET …/shares` key set is **exactly** `{id,label,created_at,last_used_at}`; the reopened dialog shows "Acme vendor" and contains **no `/s/` substring** |
| AC-26 | PASS **[R6]** | Live `DELETE …/shares/{int id}` → **204**; re-revoke → 404; non-integer `{id}` → **400** from extraction; unknown id → 404; no code in any owner URL |
| AC-27 | PASS **[R6]** | Live 11th mint → **422** `This endpoint already has the maximum of 10 active share links. Revoke one first.` |
| AC-28 | PASS **[R6]** | Live POST/GET/DELETE with a second owner's secret → **(404, 404, 404)**, never 403 |
| AC-29 | PASS **[R6]** | Live: no credential → **401** + `www-authenticate: Bearer` on all three; an unknown bearer → 401 |
| AC-30 | PASS **[R6]** | Live: `DELETE /api/endpoints/{token}` → that endpoint's live share code 404s **byte-identically** to unknown |
| AC-31 | PASS **[R6]** | Live code length **32**, charset `[A-Za-z0-9_-]`; `cargo test ids::tests::share_code_*` green (incl. `share_code_10000_are_distinct`) |
| AC-32 | PASS **[R6]** | Live: no ≥ 4-char substring shared with the token; `ids::tests::share_code_shares_no_long_substring_with_a_token` green |
| AC-33 | PASS **[R6]** | Live `GET /api/share/{code}/requests` with **no** credential → 200 `PublicShareFeed` |
| AC-34 | PASS **[R6]** | Live detail key set is **exactly** the 12-key allow-list; `token`/`matched_rule_id`/`overhead_ms`/`trace`/`state_snapshot` absent as **keys**; no `x-hookbox-*` inside `response_headers`. *(AC-34's value half is delegated to AC-S2/AC-43 — see AC-43.)* |
| AC-35 | PASS **[R6]** | Live: endpoint B's request id through A's code → **404**; A's own id → 200. `AND token = ?` present at `share.rs` |
| AC-36 | PASS **[R6]** | Live: unknown / revoked / tombstoned / malformed / length-5 / length-65 all **byte-identical** across status, headers and body — `(404, cache-control: no-store, application/json, {"detail":"This share link is not available.","error":"not_found"})` |
| AC-37 | PASS **[R6]** | Live: revoke → the very next request 404s; `no-store` on 200 / 201 / 404 / 422 / 429 |
| AC-38 | PASS **[R6]** | Fresh instance `:8098`: first 429 at request **#121**, `retry-after: 1`, `cache-control: no-store`; key `share:<ip>`, checked before any DB read |
| AC-39 | PASS **[R6]** | Live: POST/PUT/PATCH/DELETE × both public paths → **405** (8/8); `request_logs, mock_rules, endpoints, endpoint_state, crud_collections` counts identical before/after |
| AC-40 | PASS **[R6]** | Live: `limit=1`→200, `limit=200`→200, `limit=0`→422, `limit=201`→422, `offset=-1`→422, `offset=999`→200. `limit=abc`→**400** on **both** the public and the owner route (parity — see §2 note) |
| AC-41 | PASS **[R6]** | Live `/s/<code>` renders with no session, no redirect, no login prompt; e2e "AC-41: never redirects to / and creates no session" green |
| AC-42 | PASS **[R6]** | Live anonymous Chromium context: **zero** requests carried `Authorization` or `Cookie` |
| **AC-43** | **FAIL [R6]** | **The viewer DOM contains the endpoint token and the mock_url host.** Live anonymous Chromium at `/s/<code>` against the real backend, opening **every row × every tab**: on a `default_mode = "echo"` row captured through the wildcard mock host, the **Response** tab renders `host: "BQKdUE2Xbn.mock.local"`. AC-43 requires the DOM "not contain the endpoint token, `mock_url`, `path_url`, `target_url`, or any rule text" — both the token and the `mock_url` host are present. Owner-affordance half **passes** (all 15 forbidden names absent). **D1 / `hookbox-mun.36`** |
| AC-44 | PASS **[R6]** | Live: loading · list · detail ready · terminal unavailable · `/s/` NotFound. e2e covers empty (both causes) · detail gone · detail 5xx · 429 + `Retry-After` · offline · list-404-terminal — all green |
| **AC-44a** (gate-owned) | PASS **[R6]** | Live `GET /api/share/{code}/requests/{id}`: `request_headers`, `query_params`, `request_body`, `response_headers`, `response_body` are all **present keys** on every row; `response_headers` filtered (`set-cookie` → `<redacted>`, every `x-hookbox-*` dropped, `x-custom: keepme` kept) while the owner route on the **same row** shows `set-cookie: sid=abc` and `x-hookbox-endpoint/-rule-id/-served-by`; the omission list (`token`, `matched_rule_id`, `overhead_ms`, `trace`, `state_snapshot`) holds |
| AC-45 | PASS **[R6]** | Live viewer opened **0** WebSockets and received **0** `text/event-stream`; `POLL_INTERVAL_MS = 5000` (`use-shared-feed.ts:19`); manual Refresh present; e2e "AC-45/AC-S8: zero WebSocket/EventSource connections open" green |
| AC-93 | PASS **[R6]** | Live disclosure names bodies, both header maps and the last 100 requests, including pre-mint and other people's; contains no "exactly as sent" claim; e2e "AC-93/AC-S11" green |
| AC-94 | PASS **[R6]** | Live: 81 chars → client-side `role="alert"` `Labels are 80 characters or fewer.` with Create disabled **before** any request; server 81 chars → 422 `label must be at most 80 characters.`; `"   "` → stored `label: null`; `{}` → 201 |
| AC-95 | PASS **[R6]** | Live two-step arm ("It stops working immediately…can't be brought back") inside the **same** dialog → revoke → row removed, code 404s next request; e2e covers 404-as-success, 5xx row restore and Cancel-disarms |
| AC-96 | PASS **[R6]** | Live empty state + Create; e2e "AC-96: an empty list shows the empty state" and "AC-96: list-load failure shows an alert with Retry" green |
| AC-97 | PASS **[R6]** | Live DB: `share_links.last_used_at` stamped only after a resolve (`2026-08-07 20:11:31`) |
| AC-98 | PASS **[R6]** | Live: after minting, the accessible name becomes `Share this endpoint read-only — 1 active links` |
| AC-99 | PASS **[R6]** | Live hostile `Host: evil.example` **and** `X-Forwarded-Host: evil.example` still mint `http://localhost:8099/s/…`; e2e "AC-99: a share URL pointing at localhost shows the unreachable-origin warning" green |
| AC-100 | PASS **[R6]** | Live: mint on a tombstoned endpoint → 404 |
| AC-101 | PASS **[R6]** | Live: `?limit=999` → **422 with an identical body** for both a live and a dead code — no existence oracle; `?limit=abc` → identical 400 for both |
| AC-102 | PASS **[R6]** | `backend/src/routes/share.rs:282-308` builds standalone `Public*` structs field-by-field (no `flatten`, no `skip`); live key set equals the allow-list exactly |
| AC-103 | PASS **[R6]** | `cargo test config::tests::share_code_bytes_clamped_to_16_floor` + `ids::tests::share_code_clamped_to_16_byte_floor` green |
| AC-104 | PASS **[R6]** | Live DB: every `code_hash` is 64 hex; `select … where code_hash like '%<code>%'` → **0**; the code is absent from `GET …/shares`, from every error `detail`, and from the app log |
| AC-105 | PASS **[R6]** | (a) live: after the terminal 404, **1** further share request in 7 s then none; e2e covers (a) explicitly plus (b) 429/Retry-After, (c)(d)(e)(f) |
| AC-106 | PASS **[R6]** | e2e "AC-44/106: a detail 404 renders 'gone', stays open, and the rest of the list still works" + "a detail 5xx renders a distinct, retryable error (not 'gone')" green |
| AC-107 | PASS **[R6]** | Live viewer: one `<h1>` "Shared requests", `document.title` "Shared requests · HookBox", neither containing the token; standing banner + `Read-only` chip + `<footer>`; caption "Shows the last 100 requests from the past 24 hours. Updates every 5 seconds…"; `request_count` correct (1 for 1 row) |
| AC-108 | PASS **[R6]** | e2e "AC-44/108: the empty state covers both causes" green |
| AC-109 | PASS **[R6]** | Live: `<footer>` present, no `Resize feed and inspector` handle; e2e "AC-109: zero accent-filled controls and no dashboard chrome" green |
| AC-110 | PASS **[R6]** | Live 360 × 780: `scrollWidth > clientWidth` → **false**; row trigger height **69.7 px** (≥ 44) |
| AC-111 | PASS **[R6]** | Live viewer DOM across all rows and tabs: `/\byour\b/i` → **false**; e2e "AC-43: no owner-voiced accessible name renders anywhere on the page" green |
| AC-112 | PASS **[R6]** | Live: codes of length 5/31/65 and a bad charset short-circuit server-side to the identical 404; `/s/` renders "Page not found" with no crash |
| AC-113 | PASS **[R6]** | Fresh instance: per-IP first 429 at **#121**; `cargo test share_rate_limit_429_global_ceiling` + `limiter::tests::*` (8/8, incl. the two namespace-flood tests) green |
| **AC-114** (gate-owned) | PASS **[R6]** | One tree contains BE-1 + BE-2: live mint → `GET /api/share/{code}/requests/{id}` returned `response_body = '{"paid":true,"amount":"1,20","note":"he said \"hi\"\nbye"}'` — **non-null and byte-equal to the bytes the mock client received** — and the real viewer rendered it |

### 4.5 — F5 CSV export

| AC | Verdict | Evidence |
|---|---|---|
| AC-46 | PASS **[R6]** | Live menu items `["Export CSV","Clear all"]`, Export first |
| AC-47 | PASS **[R6]** | e2e "§5.6 fetch mechanism: at most 4 requests in flight, and results stay index-aligned under out-of-order completion" green |
| AC-48 | PASS **[R6]** | e2e "AC-48/AC-53: cancelling the export aborts every in-flight GET /api/requests/{id}" green |
| AC-49 | PASS **[R6]** | Live real-browser download `hookbox-requests-TPiqqKhguZ-20260807T200806Z.csv` |
| AC-50 | PASS **[R6]** | Live file line 1 byte-equals the frozen 10-column header |
| AC-51 | PASS **[R6]** | Live file: 2 data rows for 2 visible feed rows, newest-first (`/preflight` then `/pay`) |
| AC-52 | PASS **[R6]** | e2e "AC-52: a per-row 404 reads 'pending', a per-row 500 reads 'unavailable', never shifting a column" green |
| AC-53 | PASS **[R6]** | `src/api/client.ts` clears the session and bounces on 401; the abort path is proven by AC-48 |
| AC-54 | PASS **[R6]** | Live file: CRLF records, trailing CRLF, no BOM (`csv[0] === 0x74`) |
| AC-55 | PASS **[R6]** | e2e "architecture D12: `'=cmd\|' /c calc'!A1'` guards but does NOT quote" + "guard runs before quoting" green |
| AC-56 | PASS **[R6]** | Live CSV: `response_headers` cell **verbatim** (`sid=abc` **and** `x-hookbox-endpoint` present) while `request_headers` carries `<redacted>` |
| **AC-56a** (gate-owned) | PASS **[R6]** | Live real-browser download. Rule `body_template = {"paid":true,"amount":"1,20","note":"he said \"hi\"\nbye"}` → one mock request → the CSV `response_body` cell **equals the bytes the client received, byte-for-byte** after RFC-4180 unquoting (comma, embedded quotes and a literal newline all survive). The 204 CORS-preflight row exports as an **empty** cell (`status_code = 204`, cell `""`), distinct from `pending`/`unavailable` |
| AC-115 | PASS **[R6]** | e2e "AC-115/116: a mid-export arrival is excluded — the snapshot is fixed at click time" green |
| AC-116 | PASS **[R6]** | same test; `feed.export.note` states the visible count |
| AC-117 | PASS **[R5-carry]** | `dashboard.tsx` swaps to `feed.export.preparing` at 100 % and keeps Cancel mounted but `disabled` |
| AC-118 | PASS **[R5-carry]** | `dashboard.tsx` renders `feed.export.error.file` on a Blob/URL failure, `feed.export.error` otherwise |
| AC-119 | PASS **[R5-carry]** | `src/lib/request-export.ts` `ROW_TIMEOUT_MS = 10_000` → `unavailable`; 429 `Retry-After` hook in the same file |
| AC-120 | PASS **[R6]** | `exporting` disables both menu items (e2e AC-82); no `beforeunload` handler exists |
| AC-121 | PASS **[R6]** | Dismissible `feed.export.detailNote`; the real downloaded file carries **no** comment line |

### 4.6 — F6 default catch-all rule

| AC | Verdict | Evidence |
|---|---|---|
| AC-57 | PASS **[R6]** | Live: the control renders in the Rules empty state (`aria-label="Add a catch-all rule that answers any unmatched request"`) |
| AC-58 | PASS **[R6]** | Live: exactly **one** `POST …/rules`; e2e "AC-58/AC-60: zero active fallbacks → straight to the POST, no confirm, exact §5.5.7 body" green |
| AC-59 | PASS **[R6]** | Live created rule `priority = 1000`; e2e "AC-59: F6's default catch-all payload matches the frozen §5.5.7 bytes exactly" green |
| AC-60 | PASS **[R6]** | Live: the list reloads showing `Catch-all (default)` |
| AC-61 | PASS **[R6]** | Live: the control is `disabled` immediately after creation; e2e "AC-61/AC-123(a): an existing catch-all (even disabled) blocks a second" green |
| AC-62 | PASS **[R6]** | Live: a fresh endpoint's `GET …/rules` = `[]` and an unmatched mock request 404s |
| AC-63 | PASS **[R6]** | Live: `PATCH /anything/at/all` on the mock plane → **200** + `x-hookbox-served-by: rule` |
| AC-122 | PASS **[R6]** | **(e)** `cargo test auto_crud_endpoint_with_catch_all_rule_is_served_by_rule_not_crud` green. (a)(b)(d) e2e "AC-122(a)/(b): exactly one active fallback (auto_crud) → exactly one shadow bullet" and "AC-122(d): cancelling the shadow confirm issues zero requests" green |
| AC-123 | PASS **[R6]** | e2e (a) disabled catch-all blocks a second, (b) in-flight disable, (c) stale-list refresh — all green |
| AC-124 | PASS **[R5-carry]** | `rules-manager.tsx` wraps the disabled button in `<span tabIndex={0} title={reason}>` inside `Tooltip` |
| AC-125 | PASS **[R6]** | Live created rule's `body_template` byte-equals the frozen amended payload |

### 4.7 — Cross-cutting

| AC | Verdict | Evidence |
|---|---|---|
| AC-64 | PASS **[R6]** | New strings resolve through `t()`; the three CSV literals are constants in `src/lib/csv.ts:23,25` |
| AC-65 | PASS **[R6]** | e2e "AC-44/65: rows are disclosure buttons (aria-expanded/aria-controls) inside a role=region well" green; live viewer has one `<h1>`, a skip link and a `<footer>`; every new control has an accessible name *(see A5)* |
| **AC-66** | PASS **[R6]** | `pnpm typecheck` ✓ · `pnpm build` ✓ · `cargo fmt --check` ✓ · `cargo clippy --all-targets -D warnings` ✓ · `cargo test` **10/10 green** (160 tests, zero flakes) · Playwright **118/118** ✓. **This now holds on HEAD** — round 5's D2 is closed |
| AC-67 | PASS **[R6]** | `grep -rn dangerouslySetInnerHTML src/` → 2 hits, **both inside comments stating it is never used** |
| AC-126 | PASS **[R6]** | `dashboard.tsx:87,110,284` disables both menu items offline with `feed.actions.offlineHint`; e2e "AC-44: offline suspends polling and shows the offline banner" green |
| AC-127 | PASS **[R6]** | e2e "AC-127" for the sub-header and the feed header at the 360 px floor green |
| AC-128 | PASS **[R6]** | `e2e/reduced-motion.spec.ts` green under the `reduced-motion` project |
| AC-129 | PASS **[R6]** | `src/components/ui/button.tsx:23` `danger: 'bg-danger-fg text-text-on-accent hover:opacity-90'` |
| AC-130 | PASS **[R6]** | Repo-wide grep: the only remaining `bg-subtle`/`bg-hover` call site is `rule-builder.tsx:518`, which AC-130(c) lists as **out of scope**; the `--bg-*` hits in `globals.css` are CSS custom properties, not Tailwind classes; **zero** in new files |
| AC-131 | PASS **[R6]** | `json-tree.tsx:109-135` / `key-value-rows.tsx:27` route Pretty/Raw/notJson/largeRaw/truncated/"None" through `t()` |
| AC-132 | PASS **[R6]** | `copy-button.tsx:34-43`: `setStatus('copied')` only **after** `await navigator.clipboard.writeText`; rejection → `'failed'` |
| AC-133 | PASS **[R6]** | Live viewer Headers tab never renders the raw `<redacted>` sentinel; e2e "ready → a redacted header renders the neutral 'redacted' pill" green |
| AC-134 | PASS **[R6]** | `progress.tsx:23,27` `role="progressbar"` + `aria-valuetext`, shared by F3's import and F5's export |
| AC-135 | PASS **[R6]** | `json-tree.tsx:17,90` `LARGE_BODY_BYTES = 64 * 1024` → raw default + quiet caption `insp.body.largeRaw` |
| AC-136 | PASS **[R6]** | `json-tree.tsx:91,125-127` `isAtCap` → `insp.body.truncated` |

### 4.8 — F7 response-body capture

| AC | Verdict | Evidence |
|---|---|---|
| AC-68 | PASS **[R6]** | All eight `served_by` paths have a green integration case (`f7_rule_…`, `f7_default_mock_404_…`, `f7_default_echo_…`, `f7_crud_…`, `f7_cors_preflight_…`, `f7_ratelimit_429_…`, `f7_chaos_…`, `f7_mitm_…`), and **seven were re-driven live** this round through the real server and read back through the public projection: `rule`, `default`/404, `default`/echo, `crud`, `cors` (204 → `NULL`), `ratelimit`, `chaos`. Deterministic across 10 `cargo test` runs |
| AC-69 | PASS **[R6]** | Live: the 204 CORS-preflight row's `response_body` is `null` (not `""`) on the owner route, on the public route (present-with-`null`) **and** as an empty CSV cell |
| AC-70 | PASS **[R6]** | Live: a 64 000-byte template stores whole (`length(response_body) = 64000`, cap 256 000); `cargo test f7_response_body_truncation_backs_off_at_multibyte_boundary_no_panic` green |
| AC-71 | PASS **[R6]** | `cargo test f7_lossy_utf8_response_body_client_gets_raw_bytes_stored_is_lossy` green |
| AC-72 | PASS **[R6]** | Live: the mock client's bytes byte-equal the stored owner copy, the CSV cell and the public projection — measured on one and the same request. *Advisory A3: still no separate golden-fixture file* |
| AC-73 | PASS **[R6]** | **(a)** capture is in-memory, the insert stays inside the existing `tokio::spawn` (`engine.rs:771`); **(b)** `insert_trace` is **two** statements — `backend/src/db.rs:78` `INSERT … RETURNING` and `:104` prune `DELETE`, and nothing else (counters ride an AFTER INSERT trigger); live 200 requests → `request_count = 200`, `last_hit` non-null; **(c)** persistence stays fire-and-forget; **(d1)** median **1016 µs** / p95 **1390 µs** over 200 sequential 64 KB-template requests; **(d2)** **100/100** rows in `overhead_ms` bucket 0; **(d3)** `capture_response_body_5mb_is_faster_than_a_full_copy_and_byte_equal` green in all 10 runs |
| AC-74 | PASS **[R6]** | Live owner `RequestDetail` still carries `token`, `matched_rule_id`, `overhead_ms`, `trace`, `state_snapshot` |
| AC-75 | PASS **[R6]** | `cargo test f7_new_request_feed_payload_has_no_body_keys` green; `summary_base` (`engine.rs:764-769`) carries no body key; the live viewer opened no WS at all |

### 4.9 — Security ACs

| AC | Verdict | Evidence |
|---|---|---|
| AC-S1 | PASS **[R6]** | Live asymmetry on the **same row**: public `set-cookie: <redacted>`, every `x-hookbox-*` dropped, `x-custom: keepme` kept — vs owner `set-cookie: sid=abc` with `x-hookbox-endpoint/-rule-id/-served-by` present. `cargo test routes::share::tests::filter_drops_x_hookbox_prefix_and_redacts_credential_headers` green |
| AC-S2 | PASS **[R6]** | Re-verified in AC-S2's **stated scope** (`response_headers` + the summary/identity fields, **after** the filter), on the wildcard mock host, across every `served_by` path: **zero** token-bearing values. Round 5's channel A is closed — `access-control-allow-origin` now returns `<redacted>`. *(The echo `response_body` leak is outside AC-S2's stated scope and is reported under **AC-43**.)* |
| AC-S3 | PASS **[R6]** | Live on a real `default_mode = "echo"` endpoint: the **client's** body carries the raw `Authorization`, while the stored `response_body`, the public detail and the CSV all show `"authorization":"<redacted>"`; `cargo test f7_default_echo_response_body_redacted_at_rest_while_client_gets_raw` green |
| AC-S4 | PASS **[R6]** | Live: `CALLERSECRET` / `CALLERCOOKIE` appear **nowhere** in the public row's bytes. *(See D1 note: `host` is now **dropped** rather than masked from public `request_headers` and does reappear inside the echo body — dropped ≠ masked, so AC-S4 as written still holds, but this is the mechanism behind AC-43's failure.)* |
| AC-S5 | PASS **[R6]** | Live `index.html` served by the real backend carries `<meta name="referrer" content="no-referrer">`; live browser walk: **0** requests carried the code in a `Referer` and **0** off-origin subresources; `deploy/nginx.conf:23` adds the header on `location /s/` |
| AC-S6 | PASS **[R6]** | `deploy/nginx.conf:27,48` `access_log off` on both new locations; the app log after the whole sweep contains **none** of the minted codes; no `TraceLayer` in `backend/src` |
| AC-S7 | PASS **[R6]** | `backend/src/limiter.rs` per-namespace bound + `namespace_of`; `cargo test limiter::tests::*` **8/8 green** including `share_namespace_flood_never_evicts_or_resets_rl_bucket`, `rl_namespace_flood_never_touches_share_bucket`, `eviction_stays_cheap_well_past_the_namespace_cap`. 120 000-IP live flood **[R5-carry]** |
| AC-S8 | PASS **[R6]** | Live terminal-404 walk (≤ 1 further request over 7 s after the flip) + e2e 429/backoff/offline cases |
| AC-S9 | PASS **[R6]** | Live: revoke → soft only (`revoked_at` set, row retained); tombstone kills the code byte-identically; resolver predicate `s.revoked_at IS NULL AND e.gone_at IS NULL` read per request |
| AC-S10 | PASS **[R6]** | Live DB: `last_used_at` stamped after a resolve and **unchanged** by an immediate second resolve (`2026-08-07 20:11:31` → unchanged) — coalesced, written off-path |
| AC-S11 | PASS **[R6]** | Live dialog disclosure names bodies, both header maps, the last 100 requests, pre-mint arrivals and other people's requests |
| AC-S12 | PASS (declined by design) **[R6]** | No mint-time window scoping in `share.rs`; the un-scoped `copy.md` §4.5 wording ships (verified live in the dialog text) |
| AC-S13 | PASS **[R6]** | e2e `viewer-import-graph.spec.ts` (full transitive walk via the TS compiler API) green; live viewer sent zero credentialed requests. *(Residual A2)* |
| AC-S14 | PASS **[R6]** | Live 404 identity across status/headers/body incl. `HEAD`; hostile `Host`/`X-Forwarded-Host` cannot move the minted URL |
| AC-S15 | PASS **[R6]** | `cargo test share_rate_limit_429_global_ceiling` green; the per-IP check is ordered before it, both pre-DB |
| AC-S16 | PASS **[R6]** | Code absent from `GET …/shares`, from every error `detail` (`{"detail":"limit must be between 1 and 200.","error":"validation_error"}`), from the app log and from the F3 bundle; `cargo test share_code_never_appears_in_error_detail` green |
| AC-S17 | PASS **[R6]** | `cargo test f7_response_body_truncation_backs_off_at_multibyte_boundary_no_panic` + `helpers::tests::truncate_utf8_floors_to_char_boundary_no_panic` green |
| AC-S18 | PASS **[R6]** | `backend/src/router.rs:206` `.layer(CatchPanicLayer::new())` outermost; the panic route is `#[cfg(test)]`-only by design (`router.rs:210-229`), and `cargo test router::tests::panic_becomes_500_and_next_request_still_served` is green through the real layer stack |
| AC-S19 | PASS **[R6]** | `engine.rs:703-771` — the `TraceRecord` is built (and truncated at `state.cfg.max_body_bytes`) **before** the `tokio::spawn`; only the ≤ 256 KB `Option<String>` crosses |
| AC-S20 | PASS **[R6]** | `backend/src/db.rs:73-110` `insert_trace` issues `INSERT … RETURNING` + prune `DELETE` and nothing else — the frozen two-statement baseline, documented in the doc comment |
| AC-S21 | PASS **[R6]** | Live: the confirm renders provenance + the old→new diff **before any request** (write log empty at that moment) and states how many rules will be added — `Apply this configuration? … Exported 8/8/2026 from an endpoint named "Journey source". Settings that change (3) name Target original→Journey source latency_ms 0→250 chaos_pct 0→7 … Adds 1 rules to the 1 already on this endpoint.` |
| AC-S22 | PASS **[R6]** | Live: a **nested** unknown key inside `endpoint` is rejected pre-write with zero requests; rule objects stay non-strict (e2e "a stray id/token/created_at inside a rule is stripped, not fatal"); the importer calls only `PATCH …/{token}` and `POST …/rules` |
| AC-S23 | PASS **[R6]** | e2e "countRulesUsingRequestHeaderTag detects the {{request.header.X}} tag (AC-S23)" green; wired at `src/screens/settings.tsx:905` |
| AC-S24 | PASS **[R6]** | `src/api/http.ts` `credentials: 'omit'`; live browser: **zero** cookie-bearing requests from the viewer; live: no `Authorization` → 401 with nothing deleted |
| AC-S25 | PASS **[R6]** | Live CSV: `response_headers` verbatim while `request_headers` is `<redacted>`; e2e "AC-56: request Authorization exports already-redacted; response headers stay verbatim" green |
| AC-S26 | PASS (scoped) **[R6]** | `deploy/nginx.conf:23-26` `Referrer-Policy`, `X-Robots-Tag`, `X-Content-Type-Options`, `X-Frame-Options: DENY` on `location /s/`; CSP deferred per §8-R16 |
| AC-S27 | PASS **[R6]** | Live `GET /robots.txt` → 200 `User-agent: *\nDisallow: /s/\n` |

**Tally: 164 PASS · 1 FAIL** (AC-43).

> **Note on the two §5-sweep "failures" I re-adjudicated.**
> **(1) `?limit=abc` → 400, not 422.** AC-40 requires 422 for *out-of-range* values "matching the
> existing owner list route". Measured: the **owner** route returns 400 for `?limit=abc` and
> `?offset=xyz` and 422 for `?limit=0`/`?limit=201` — the public route is byte-identical on all four.
> A non-numeric value is rejected by axum's `Query` extractor before either handler, and the 400 body
> is **identical for a live and a dead code**, so AC-101's no-oracle property holds. Parity achieved;
> not a defect.
> **(2) `GET /__test_panic` → 200.** That route is `#[cfg(test)]`-only by design (`router.rs:210-229`),
> so it is absent from the running binary and falls through to the SPA. AC-S18's assertion is the unit
> test, which drives the **real** layer stack and is green.

---

## 3. Journey walkthrough (`journey.md`)

### 3.1 Primary and alternate flows

| Flow | Works? | Evidence |
|---|---|---|
| **F1 primary** — clear a noisy feed | YES **[R6]** | Menu → confirm (endpoint named, wider blast radius, arrival note) → server `GET …/requests` = `[]`, "No requests yet" renders, Inspector back to "Select a request" |
| F1-A Settings entry point | YES **[R6]** | e2e "J9b" — the shared `ConfirmDialog` renders confirm failures inline |
| F1-B clear while paused | YES **[R6]** | e2e AC-76 green; `clearRows()` empties the buffer |
| F1-C cancel via Esc / overlay | YES **[R6]** | Live: 0 writes, 2 rows still present |
| F1 arrival race | YES **[R6]** | AC-80's note renders in the confirm |
| **F2 primary** — one chip | YES **[R6]** | Sub-header exposes no "Local path" copy affordance; Settings still lists it |
| F2-A discoverability fallback | YES **[R5-carry]** | The Mock URL chip carries `dash.mockUrl.tooltip` pointing at Settings |
| **F3 export** | YES **[R6]** | Real download `hookbox-config-<token>.json` — version 1, 9 endpoint fields, rules, no token/secret/owner/code/share key |
| **F3 import** (F3-A fresh endpoint) | YES **[R6]** | Confirm-with-diff (zero writes before confirm) → one PATCH → one POST per rule; config **and** rules landed; the form then shows the imported values and the next Save PATCHes them |
| F3-B import onto existing rules | YES **[R6]** | Live: 1 pre-existing + 1 imported = 2 — add-never-replace |
| F3-C hand-edited bundle | YES **[R6]** | Five malformed bundles rejected **pre-write with zero requests**, each with its own human message |
| F3 partial-failure prefix | YES **[R6]** | e2e AC-19 — real 422 mid-import → the five-fact persistent report; rule 2 never attempted |
| F3-D no undo | YES (by design) **[R6]** | AC-S21's pre-apply diff + confirm verified live before any write |
| F3-E export as backup | YES **[R6]** | Round-trip fidelity proven (AC-15) |
| **F4-A owner** mint → paste → revoke | YES **[R6]** | Disclosure → label "Acme vendor" → Create → URL shown once + "Shown once" hint → reopened list row **without a URL** → two-step inline revoke → the code 404s on the next request |
| **F4 viewer** a stranger opens it | **PARTIAL — FAILS the stated end state** **[R6]** | The read path works: real `/s/<code>` in a clean browser context renders the banner, `Read-only` chip, static `<h1>`, correct total, rows as disclosures, Headers/Query/Body/Response tabs, the redaction pill, filtered response headers and the **captured response body verbatim**; revoke-while-open flips to terminal and stops polling. **But `journey.md` F4 VIEWER step 6 — "They never learned the token, the mock URL, the rules, or the owner's identity" — is still false:** on a `default_mode = "echo"` row the Response tab renders `host: "<token>.mock.local"`, and I used that recovered token to write two new requests into the endpoint (`request_count` 1 → 3), which then appeared in the shared feed. **D1** |
| F4 revoke kills the link | YES **[R6]** | Live: the open viewer tab flips to "This link isn't available" and issues ≤ 1 further request over 7 s |
| F4-B revoke-all by deleting the endpoint | YES **[R6]** | Live tombstone → the prior share URL 404s byte-identically to unknown |
| F4-C forwarded link | YES (accepted) **[R6]** | Anyone with the URL reads it — the disclosure says so |
| F4-D phone viewport | YES **[R6]** | 360 × 780: no horizontal scroll; row triggers 69.7 px |
| F4-E overnight tab | YES **[R6]** | e2e: 5 s poll resumes on visibility/online; a stale row's detail 404 renders "gone" and list polling continues |
| F4-F owner opens their own link | YES **[R6]** | Zero `Authorization`/cookie requests from `/s/`; e2e "AC-41: never redirects to / and creates no session" green |
| F4-G crawler | YES **[R6]** | `robots.txt` `Disallow: /s/` served live; nginx adds `X-Robots-Tag`, `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy` + the `<meta>` fallback |
| **F5 primary** — export | YES **[R6]** | Real file: frozen header, CRLF + trailing CRLF, no BOM, real bodies byte-equal to the client's, verbatim response headers, redacted request headers |
| F5-A paused buffer excluded | YES **[R6]** | e2e AC-115/116 green |
| F5-B cancel | YES **[R6]** | e2e: every in-flight detail fetch aborted, no file |
| F5 per-row 404 / 5xx | YES **[R6]** | e2e AC-52 — 404 → `pending`, 500 → `unavailable`, 10 columns intact |
| F5-C double export | YES **[R6]** | `exporting` disables both menu items (e2e AC-82) |
| F5-D 1 row / cap | YES **[R6]** | Live 2-row export; TRACE_CAP prune verified at exactly 100 after 200 requests; e2e "zero data rows still produces a header + trailing CRLF" |
| **F6-A primary** (plain endpoint) | YES **[R6]** | One click from the empty state → **one** POST → `Catch-all (default)` at priority 1000, **no confirm** when nothing is shadowed; the control then disables; `PATCH /anything/at/all` answers `served-by: rule` |
| **F6 on an `auto_crud` endpoint** | YES **[R6]** | `cargo test auto_crud_endpoint_with_catch_all_rule_is_served_by_rule_not_crud` green; e2e AC-122(a)/(b)/(d) green |
| F6-B/C/D | YES **[R6]** | priority 1000 sorts last; e2e AC-123(a)(b)(c) green |
| **F7** Inspector / CSV / viewer | YES **[R6]** | The same captured body verified byte-for-byte in all three surfaces from one and the same request |

### 3.2 Error and failure paths walked

* **F1:** Esc/overlay cancel (0 writes) · 503 on the DELETE (e2e AC-6) · 404 and 410 cards (e2e).
* **F3:** malformed JSON · wrong version · unknown top-level key · **nested** unknown key · 0-byte — all
  rejected with **zero** writes and a human message · BOM, same-file re-select, > 200 rules, a real 422
  mid-import and a 503 on the export's rules fetch (e2e).
* **F4 owner:** 401 on all three routes (+ `WWW-Authenticate: Bearer`) · unknown bearer 401 · 404-not-403
  for a foreign owner on all three · 11th mint 422 · label-81 422 (server) and disabled-Create (client) ·
  blank label → `null` · `{}` → 201 · revoke of an unknown id 404 · re-revoke 404 (idempotent) ·
  non-integer id 400 · mint on a tombstoned endpoint 404 · list 503 + Retry recovers (e2e) ·
  5xx-on-revoke restores the row (e2e) · localhost unreachable-origin warning (e2e).
* **F4 viewer:** unknown / revoked / tombstoned / malformed code byte-identical 404, GET **and** `HEAD` ·
  code lengths 5/31/65 and a bad charset · `/s/` with no code → "Page not found" · `limit`/`offset` out of
  range 422 (**and 422 with an identical body for a dead code — no oracle**) · `limit=abc` → identical 400
  for both · POST/PUT/PATCH/DELETE → 405 × 8 with **zero** mutation across five tables · 429 with
  `Retry-After` on the 121st request · revoked-while-open (terminal, polling stops) · detail 404 → "gone" ·
  detail 5xx → retryable · offline.
* **F5:** per-row 404 → `pending`, per-row 5xx → `unavailable`, cancel aborts, 401 aborts.
* **F6:** confirm on a shadowing endpoint, Cancel issues zero requests, duplicate guard (incl. a
  *disabled* catch-all), stale-list refresh, in-flight disable.

### 3.3 Things that looked like failures but were my harness

1. **Two AC-S1 asymmetry checks** in the first token sweep failed because my script picked `IDS[0]`,
   which was the `default`/404 row (no `set-cookie`, no `x-custom`). Re-run against the **rule** row:
   both halves pass.
2. **`?limit=abc` → 400** — my check asserted 422. Owner-route parity measured; not a defect (§2 note).
3. **`GET /__test_panic` → 200** — the route is `#[cfg(test)]`-only; the unit test is the assertion.
4. **A `default`/echo row never reached one probe** because that endpoint had `auto_crud = true`, which
   swallowed the single-segment path into the CRUD plane. Re-driven on a non-CRUD endpoint — that is
   where channel C surfaced.

---

## 4. Contract findings (§5, verified on BOTH sides)

Confirmed matching, **real server ⇄ real SPA**:

* **§5.1 #19** 201 key set = `{id, code, url, label, created_at, last_used_at}` ⇄ `shareLinkCreatedSchema`
  (`src/api/schemas.ts:196-204`), `last_used_at` present-with-`null`, `Cache-Control: no-store`.
* **§5.1 #20** `[{id, label, created_at, last_used_at}]` ⇄ `shareLinkSchema` (`:188-193`) — no code, no
  url, revoked links absent.
* **§5.1 #21** revoke by **integer id** → 204; unknown / foreign / already-revoked → 404
  indistinguishably; a non-integer id → 400 from extraction before the handler. Soft revoke only.
* **§5.2 #22/#23** the frozen check order is observable: rate limit before any DB read (429 + lowercase
  `retry-after` + `no-store` at request #121); `?limit=999` → 422 for **both** a live and a dead code;
  one `share_not_found()` byte-identical across unknown/revoked/tombstoned/malformed/bad-length, GET
  **and** HEAD; `no-store` on every handler-produced status; 405 for other verbs with no envelope
  (accepted, D14).
* **§5.5.4/§5.5.5** the real JSON key sets are **exactly** `publicShareFeedSchema` /
  `publicRequestSummarySchema` / `publicRequestDetailSchema` (`schemas.ts:209-234`); the five detail
  fields are present-with-`null`; the omission list holds. **Deviation: a *value* inside
  `response_body` violates AC-43 — see D1.** The shape is correct; the redaction is incomplete.
* **§5.5.6** the exported bundle validates against `configBundleSchema`, and the real backend accepts
  `bundle.endpoint` as a `PATCH` body and each rule as a `POST` body (round trip landed, in order).
* **§5.5.7** the created catch-all's payload is byte-identical to the frozen amended bytes, and the
  matcher serves it (`served-by: rule` for `PATCH /anything/at/all`).
* **§5.6** real browser download: frozen 10-column header, CRLF + trailing CRLF, no BOM,
  `response_headers` verbatim, `response_body` byte-exact, `pending`/`unavailable`/empty-cell all three
  distinguishable.
* **§5.7** no WS/SSE change — the viewer opened **0** WebSockets and received **0** `text/event-stream`.
* **§5.8** defaults confirmed at runtime: `SHARE_CODE_BYTES` 24 → 32-char code (clamped ≥ 16),
  `SHARE_MAX_PER_ENDPOINT` 10 (11th mint 422), `SHARE_RATE_LIMIT_PER_MIN` 120 (first 429 at #121),
  global ceiling asserted by `share_rate_limit_429_global_ceiling`.
* **§5.9** the real backend serves `/s/<code>` (SPA fallback) and `/robots.txt`; `deploy/nginx.conf`
  carries both new locations with the four security headers, `access_log off` and `X-Real-IP`.
* **§5.10** `response_body` is a real value on all three surfaces; the owner `RequestDetail` key set is
  unchanged.
* **§5.11** the redaction table holds **for the keys it enumerates**, measured on one and the same
  request: owner verbatim (`sid=abc`, `x-hookbox-*` present, `host: <token>.mock.local` present) / CSV
  verbatim / public filtered (`<redacted>`, `x-hookbox-*` dropped, `x-custom` kept).
  **Two recorded drifts from the frozen table, both consequences of the `hookbox-mun.34` fix:**
  (a) the Public row's `Request headers` cell reads "as stored (already redacted)", but the shipped
  code now additionally **drops** `host`/`origin`/`referer` and masks token-bearing values — required
  by AC-43, no shape change, so I record it rather than fail it (**A6**);
  (b) §5.11 has **no rule for `response_body`** on the public surface, which is exactly the hole D1
  falls through.

**Shape deviations found: none. Behavioural deviations found: one (D1).**

---

## 5. Defects (round 6)

### D1 — `hookbox-mun.36` (backend, **HIGH**). AC-43 · journey F4 VIEWER step 6.

**The endpoint token still reaches anonymous share viewers — through the `default_mode = "echo"`
`response_body` — and the recovered token is a live write capability against the endpoint.**
`hookbox-mun.34` closed the two header channels; this is the third, of the same class and the same
severity, and it is what `prd.md` §0 item 1 says F4 cannot ship with.

**The channel.** `engine.rs:392-404` rebuilds the *persisted* echo body from
`echo_payload(&method, mock_path, &query, &redact(&headers_lower), &body_text)`. `redact()`
(`backend/src/helpers.rs:32-34`) masks only `authorization`/`cookie`/`x-owner-id` — it has no notion of
`host`/`origin`/`referer`, which is exactly the set the new `PUBLIC_REQUEST_HEADER_DROP`
(`share.rs:80-88`) removes from public `request_headers`. The public projection filters both header maps
but passes `response_body` through verbatim (`share.rs:305`), so the same map the filter just dropped
comes straight back inside the body:

```
GET /api/share/<code>/requests/<id>          (anonymous, no credentials)
response_body:
  {"body":"{\"order\": 1}",
   "headers":{"authorization":"<redacted>", ...,
              "host":"dTXgCJYPaG.mock.local",              <-- token + mock_url host
              "origin":"https://dTXgCJYPaG.mock.local"},
   "method":"POST","path":"/hooks/order","query":{}}
```

**Rendered in the viewer.** Live anonymous headless Chromium at `/s/<code>` against the real backend and
the real `dist/` SPA, opening **every row × every tab** — the Response tab renders:

```
host: "BQKdUE2Xbn.mock.local"          visible in the DOM, row 0
```

AC-43: "The DOM must additionally **not contain the endpoint token**, `mock_url`, `path_url`,
`target_url`, or any rule text." Both the token and the `mock_url` host are present.

**Proof the recovered token is a live write capability** (anonymous, share code only — 3/3 green):

1. `GET /api/share/<code>/requests` → pick the echo row
2. `GET /api/share/<code>/requests/<id>` → `response_body.headers.host = "dTXgCJYPaG.mock.local"`
3. token = `dTXgCJYPaG`
4. `POST http://localhost:8099/e/dTXgCJYPaG/attacker-injected` → 200, trace persisted
5. `POST` with `Host: dTXgCJYPaG.mock.local` `/attacker-injected-2` → 200, trace persisted
6. `endpoint.request_count` **1 → 3**; both injected rows now visible in the shared feed

**Why the existing regressions miss it.** `backend/tests/api.rs`'s
`share_public_detail_key_allowlist_and_no_token_substring` — extended by `hookbox-mun.34` with the
wildcard `Origin` case — drives a **rule** row, whose `response_body` is a rendered template.
`f7_default_echo_response_body_redacted_at_rest_while_client_gets_raw` drives an echo row but asserts
only the `authorization`/`cookie` masking, never a token substring. No test fires an echo request with
`Host: <token>.<MOCK_DOMAIN>`.

**Scope note.** `default_mode = "echo"` is a shipped, documented mode and the wildcard host is the form
`mock_url` hands the operator, so this fires with **no attacker setup and no operator mistake** — the
same structural quality that made channel B a defect.

**Suggested fix** (in the bug): mirror AC-S3's existing intervention on the **persist path** — extend
the echo rebuild so the persisted `headers` sub-object also drops/masks `host`/`origin`/`referer` (the
client's echo body keeps them, so AC-72 and §2's non-goal hold). A public-projection mask on
`response_body` also works but is broader. Storage for non-echo rows, the owner Inspector and F5's CSV
must stay verbatim per §5.11.

---

## 6. Advisory notes (no bug filed)

* **A1 — a tombstoned endpoint's *owner* routes still answer 200.** After `DELETE /api/endpoints/{token}`,
  `GET /api/endpoints/{token}`, `GET …/shares` and `DELETE …/requests` still return 200; only mint
  (AC-100) and the public resolver (AC-S9) check `gone_at`. `journey.md`'s F4-owner table expects "every
  subsequent share call 404s", true for **mint** but not list/revoke. Pre-existing; the frozen §5.1 does
  not ask for a `gone_at` check on #20/#21, and there is no exposure (every link is already dead
  publicly). Follow-up outside this feature.
* **A2 — AC-S13's residual, unchanged.** The viewer's *module graph* no longer reaches `session.ts`
  (which is what the AC asserts, green on HEAD), but the app still builds as a single chunk, so
  `session.ts` executes on the viewer page via the router's own graph. The real mitigation remains the
  deferred CSP (§8-R16).
* **A3 — AC-72** still has no separate golden-fixture file; the live client-bytes assertions
  (byte-equality across client / owner / CSV / public, from one request) are the equivalent evidence.
* **A4 — AC-73(d1)/(d2)** numbers were produced by QA, not recorded in a PR body: median **1016 µs** /
  p95 **1390 µs**, **100/100** rows in `overhead_ms` bucket 0. Machine- and load-specific (rounds 4/5
  measured 495/989 µs on the same code), so only the `overhead_ms` bucket-0 property is comparable
  across rounds.
* **A5 — "Add default rule" fails WCAG 2.5.3 (Label in Name).** Visible label "Add default rule",
  accessible name "Add a catch-all rule that answers any unmatched request". The `aria` string is frozen
  by `copy.md` §4.7 and AC-65 only requires "an accessible name", so not an AC violation. Future copy
  pass.
* **A6 — §5.11's Public `Request headers` cell is now stale.** The frozen table says "as stored (already
  redacted)"; the shipped code additionally drops `host`/`origin`/`referer` and masks token-bearing
  values. That is **required** by AC-43 and changes no shape, so it is a documentation drift between two
  frozen texts (§4 AC-43 vs §5.11), not a code defect — but a future PRD revision should update the
  cell, and should add the missing `response_body` rule that D1 falls through.
* **A7 — §5.11 filters by header *name* only** (and now, post-`hookbox-mun.34`, by *value* for the token
  specifically). It still has no general concept of a value-based rule, so the next value-carrying
  header or rule-authored body is a fresh discovery rather than a covered case.
* **A8 —** the remaining dead `bg-subtle`/`bg-hover` site (`rule-builder.tsx:518`) is inside AC-130(c)'s
  out-of-scope list and remains tracked separately (`hookbox-59n`).
* **A9 — `Esc` does not close the Share dialog while its success toast is on screen** (round-5 A2,
  unchanged). The Radix toast becomes the topmost dismissable layer and consumes the key. Transient;
  `Done`, `Close` and overlay-click all work throughout. No AC requires Esc-to-close here.
