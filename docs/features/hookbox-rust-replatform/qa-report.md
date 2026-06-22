# QA report — HookBox Rust/Axum + SQLite re-platform (round 1)

- **Gate:** `hookbox-sks.34` (area:qa, feature:hookbox-rust-replatform)
- **Date:** 2026-06-21
- **Verdict:** NOT PASSED — 1 P0 frontend defect blocks the primary journey; 1 low backend test-flake defect. Gate re-blocked on both bugs.
- **Method:** dynamic. Backend built + `cargo test` (80/81 unit + 12/12 integration). Live `hookbox` binary on :8099 over a seeded SQLite DB, exercised with curl (P1 mock plane, P2 management API, P3 SPA + SSE feed). Tunnel CLI binary run against the live server. Frontend built (`pnpm build`), typechecked (`tsc --noEmit`, clean), Playwright e2e run (22 passed / 1 skipped). FE zod schemas cross-checked against live BE JSON.

---

## A. Functionality POV — acceptance criteria

### Lens summary
- Backend `/api/**` contract (§5.2), error envelope (§5.1), data shapes (§5.3), feed (§5.4), resolution order + X-HookBox headers + 404/410 + caps (§5.5), templating sandbox (§5.7), tunnel protocol (§5.8): **all verified PASS dynamically**.
- Frontend: typecheck clean, build clean, zod schemas match BE byte-for-byte, all `/d/:token` screens wired and e2e-green — **except the `/` landing route is not wired** (P0).

### Per-AC table (representative + every dynamically tested AC)

| AC | Verdict | Evidence |
|----|---------|----------|
| AC-1 session shape/anti-enum | PASS | `POST /api/session` new+existing email return identical keys `[endpoints, owner_id, owner_secret, primary]`; 200; invalid email → `422 {error:validation_error}` |
| AC-2 / S6 cap rotation | PASS | 2nd session → old secret `401`, new secret `200` (live) |
| AC-3 auto-provision primary | PASS | new email returns one `primary` endpoint (live) |
| AC-4 / S7 owner_id not a credential | PASS | owner_id as Bearer → `401` (live) |
| AC-5 session rate-limit | PASS (static) | `SESSION_RATE_LIMIT_PER_MIN=30`; limiter wired at `routes/api.rs` |
| AC-6/7/8/S16 plane isolation | PASS | `/api` under mock Host → P1 `no_match` 404, never management; `planes::tests` (9 tests) green |
| AC-9 token alphabet/case | PASS | `ids::tests::token_alphabet_strips_ambiguous_chars`, `planes::subdomain_label_case_preserved` |
| AC-10 path-fallback mode | PASS | `path_url=/e/<token>` present in SessionResponse; `blank_or_dotless_mock_domain_degrades_to_path_fallback` |
| AC-11 priority,id order | PASS | `matcher::order_is_priority_then_id`; live `/api/.../rules` ORDER BY |
| AC-12 match criteria | PASS | `matcher::path_exact_param_and_wildcard`, `body_and_state_conditions` |
| AC-13 rule response | PASS | live `/e/<tok>/hello` → 201, body templated, `X-HookBox-Served-By: rule`, `X-HookBox-Rule-Id` |
| AC-14 5-tab rule builder | PASS | `src/screens/rule-builder.tsx` Matching·Response·Templating·Actions·Throttling; e2e J10 builds a rule end-to-end |
| AC-15 per-rule overrides | PASS | `limiter::keys_isolate_endpoint_and_rule`; schema carries per-rule latency/rate/chaos |
| AC-16 templating tags | PASS | live: `{{random 'uuid'}}→uuid`, `{{random 'int' 1 5}}→4`, `{{now 'unix'}}`, `{{request.method}}→GET` |
| AC-17 / S5 SSTI verbatim | PASS | live `{{ 7*7 }} {{config}}` returned literal; `templating::ssti_probes_returned_verbatim` |
| AC-18 unknown/malformed literal | PASS | `{{random "uuid"}}` (double-quoted, non-spec form) left literal; `templating::size_and_tag_caps` |
| AC-19 state_write before body | PASS | live `/login` rule body `set:{{state.auth}}` → `set:true` (write applied before render) |
| AC-20 stateful multi-step | PASS | live: pre-login `/dashboard` → 404 (fail-closed); post-login → 200 `welcome` |
| AC-21 lazy state read | PASS (static) | `state_store` + `any_rule_gates_on_state` gating |
| AC-22 state get/clear | PASS | live `GET /state → {state:{}}`; owner-gated |
| AC-23 / S17 safe key charset | PASS | `state_store::write_read_roundtrip_and_unsafe_skipped` |
| AC-24 Auto-CRUD | PASS | live POST `/items`→201 uuid id, `X-HookBox-Served-By: crud`; GET array; DELETE→204 |
| AC-25 CRUD caps/charset | PASS | live non-object body→400; unsafe collection name→422; `crud_store::max_items_cap` |
| AC-26 CRUD atomicity | PASS | `crud_store::concurrent_posts_no_collision` (BEGIN IMMEDIATE) |
| AC-27 collection peek/clear | PASS | live `GET /collections/items → {items:[]}`; bad name → 422 |
| AC-28 MITM forward | PASS (test) | `proxy::builds_target_url_with_path_and_query`; rule wins over forward |
| AC-29/30/31 / S1-S4 SSRF + strip | PASS | live target `169.254.169.254` → `502`; `ssrf` tests (block resolved IP set, IPv4-mapped); `proxy::ssrf_blocked_target_is_unreachable` |
| AC-32 target_url validate | PASS | live `ftp://x` PATCH → 422 |
| AC-33/34/36 P1 CORS | PASS | live OPTIONS → 204 + reflected origin + methods + max-age 600 + Vary; no Allow-Credentials; `cors` tests |
| AC-35 / S11 no CORS on P2 | PASS | live `/api` with Origin → zero `Access-Control-*` headers |
| AC-37/38 conditions order/latency | PASS | `conditions::latency_applies_clamped`, `clamps`; live overhead_ms recorded |
| AC-39 rate-limit | PASS | live `rate_limit_per_min=2` → hit3/4 `429` + `Retry-After:30` + `X-RateLimit-Limit/Remaining` |
| AC-40 chaos | PASS | live `chaos_pct=100,error` → `502` + `X-HookBox-Served-By: chaos`; `conditions::chaos_edges_deterministic` |
| AC-41 feed publish | PASS | live SSE streamed `new_request` for `/sse-probe` |
| AC-42 / S9 feed owner-gate | PASS | live SSE anon → 401, wrong cap → 401, valid cap → stream; `feed` channel-isolation test |
| AC-43 / S19 conn caps | PASS (static) | `WS_MAX_CONN_PER_ENDPOINT`, bounded broadcast + per-send timeout in feed.rs |
| AC-44 hello + RequestDetail | PASS | live SSE `hello {server_time, token}`; `GET /api/requests/{id}` keys exactly match `requestDetailSchema` |
| AC-45 SPA over §5 | PASS* | dashboard/rules/settings/cli screens wired + e2e-green; **landing screen NOT wired — see DEFECT-1** |
| AC-46/47 retention | PASS | `tasks::sweep::caps_traces_per_token`, `reaps_expired_state_crud_and_tombstones` |
| AC-48 list/clear requests | PASS | live `?limit=5` returns RequestSummary[]; `limit=0`→422, `limit=500`→422 |
| AC-49/50/51 tunnel protocol | PASS | live tunnel CLI binds, forwards (`GET /tunnelprobe → localhost:3000 (502)`), labeled tunnel; `tunnels::last_bind_wins` |
| AC-52 tunnel CLI stdout | PASS | live bad-secret → "Authentication failed … Stopping." + clean exit (no loop); good → "Tunnel up. Forwarding …" |
| AC-53 single-binary serve | PASS | from repo-root cwd: `/`, `/d/<tok>`, `/cli` → 200 index.html, `/assets/index-*.js` → 200 text/javascript; seed bin plants demo |
| AC-54 Docker optional | PASS (static) | single-container `Dockerfile`/`docker-compose.yml` re-authored (no Redis) |
| AC-55 build gates | FAIL | `cargo test` (default parallel) → **1 failure** from a config-test env-var race — see DEFECT-2. Playwright: 22 pass / 1 skipped (landing, blocked by DEFECT-1) |
| AC-56 X-HookBox headers | PASS | live rule hit carries Endpoint/Served-By/Rule-Id; crud/chaos/default served-by all observed |
| AC-57 404 vs 410 | PASS | live unknown token → `404 unknown_endpoint`; deleted token → `410 endpoint_gone`; `unknown_404_and_gone_410_not_traced` |
| AC-58 / S15 body caps | PASS (static) | `MAX_INGEST_BODY_BYTES` 413 guard; `MAX_BODY_BYTES` trace truncation |
| AC-59 off-path trace | PASS | `served_requests_are_traced_with_redaction`; SSE delivered after response returned |
| AC-60 / S8 auth envelope | PASS | live missing auth → `401` + `WWW-Authenticate: Bearer`; non-owner token → `404`; flat `{error,detail}` |
| AC-61 / S13 redaction | PASS | live trace: `authorization:<redacted>`, `cookie:<redacted>`, secret absent from full payload; secret never in server log (?cap= redacted) |
| AC-S14 cap not in cookie | PASS | `src/api/session.ts` holds cap in memory+localStorage; no `document.cookie` writes |
| AC-S17 no SQL string-interp | PASS (static) | sqlx parameterized binds; charset enforced (tests) |
| AC-S18 limiter bounded/fail-open | PASS | `limiter::idle_eviction_bounds_map` |

**Visual ACs:** AC-D11 (0 raw hex in `src/components/**` — verified by grep + e2e no-hex.spec), AC-D13/D14/D15/D16/D18/D19/D22/D24 all PASS via Playwright `visual.spec.ts` + `reduced-motion.spec.ts`. AC-D12/D20/D21/D23 wired via semantic tokens (static, plausible; not pixel-measured this round).

### Contract integrity (FE ↔ BE) — highest-value check
Live BE JSON cross-checked against `src/api/schemas.ts` zod types — **no mismatch found**:
- `SessionResponse`, `EndpointSummary`, `EndpointDetail` (incl `tunnel_active`, `chaos_mode`), `MockRule` (incl per-rule `chaos_mode`, `webhook_action`), `RequestSummary`, `RequestDetail` — live key sets are byte-identical to the schemas.
- Error envelope flat `{error, detail}` on every error path (401/404/410/422/429) — matches `errorEnvelopeSchema`.
- Feed envelope: BE WS `{"type":kind,"data":...}` (feed.rs:32) ↔ FE `decodeWsFrame`; BE SSE `event:`+`data:` ↔ FE `decodeSseEvent`. Event names `hello`/`new_request`/`endpoint_updated` emitted by BE and consumed by FE match.

---

## B. User POV — journey walkthrough

| Flow | Works? | Evidence |
|------|--------|----------|
| Primary 1-3: land → submit email → persist+route | **NO** | `/` renders the `Placeholder` "Landing / email gate", not the `Landing` screen. `src/screens/landing.tsx` exists but is imported nowhere; `src/router.tsx:54` mounts `<Placeholder>`. A new user cannot enter an email or reach `/d/:token`. Playwright landing happy-path self-skips ("/ not yet wired to the Landing screen"). **DEFECT-1.** |
| Primary 4-11: dashboard boot → copy URL → build rule → hit mock → watch live → inspect → tune | YES | e2e journeys J2 (split-screen + inspect), J10 (5-tab rule build), J9 (settings delete); live feed SSE streaming + RequestDetail verified |
| Auto-resume (stored owner → /d/:token) | PARTIAL | dashboard auto-resume logic present in `dashboard.tsx`; entry via `/` blocked by DEFECT-1 (a user with stored creds is redirected by the dashboard, but the gate screen itself is unreachable) |
| Re-enter email to rotate | BACKEND OK / FE blocked | rotation verified live (AC-2); FE rotation handling in `api/session.ts`, but the email form is unreachable (DEFECT-1) |
| Multi-endpoint switcher / +New | YES (static) | switcher + create wired in dashboard; `POST /api/endpoints`→201 live |
| Auto-CRUD path | YES | live full lifecycle (POST/GET/DELETE) |
| Stateful multi-step | YES | live login→dashboard gated flow |
| MITM proxy / SSRF | YES | live SSRF block 502 |
| Auto-CORS preflight | YES | live OPTIONS 204 |
| Tunnel CLI (connect/bound/forward/4401-stop) | YES | live CLI run: bound, forwarded, bad-auth stop |
| Pause feed + N-new pill | YES | e2e journeys J2 pause-buffers test |
| Error paths: 422 email / unknown 404 / gone 410 / inspector pending / feed unauthorized / conn-cap busy | YES | e2e states.spec (404/410 cards, inspector pending+retry, feed empty/streaming); live 410/404/422/429 |
| Inspector body modes / empty sub-states | YES | e2e states inspector ready (5 tabs) + empty |

---

## Defects (filed as bd bugs, gate re-blocked)

1. **[P0 · frontend] `/` route mounts the Placeholder, not the Landing screen.**
   `src/router.tsx:54` → `{ path: "/", element: <Placeholder name="Landing / email gate" /> }`. `src/screens/landing.tsx` exports a complete `Landing()` but is imported nowhere (`grep` → 0 imports). Breaks journey.md Primary steps 1-3 and AC-J1 / AC-1 (FE) / AC-D22 / AC-D24 landing. The Playwright landing happy-path self-skips for this reason. Fix: import `Landing` and mount it at `/`.

2. **[low · backend] `cargo test` is flaky (env-var race) — AC-55 build gate is non-deterministic.**
   `config::tests::defaults_apply_without_env` and `config::tests::blank_or_dotless_mock_domain_degrades_to_path_fallback` both mutate the process-global `MOCK_DOMAIN` env var without serialization; under default parallel `cargo test` the former intermittently fails (`assert !cfg.path_fallback_only` at config.rs:197). Passes with `--test-threads=1`. AC-55 requires `cargo test` to gate the build, so a flaky gate is a defect. Fix: serialize the two tests (a shared mutex / `serial_test`) or have each set+restore its own env.

No other open findings. Product behavior is otherwise at full parity on both lenses.

---

## Round 2 — re-verification addendum (2026-06-22)

- **Verdict: PASSED.** Both round-1 defects are fixed and re-verified by the orchestrator; all other lenses were already passing in round 1.
- **DEFECT-1 (P0, frontend) — RESOLVED.** `src/router.tsx:13` now `import { Landing } from "@/screens/landing"` and line 36 mounts `{ path: "/", element: <Landing /> }` (no longer the Placeholder). `tsc --noEmit` clean; `pnpm build` succeeds (1695 modules → `dist/`, 541 kB / 163 kB gzip). The landing/email gate is reachable and AC-J1 states render; new-vs-existing email remains indistinguishable through success (no "welcome back" string). AC-1 (FE) / AC-D22 / AC-J1 now satisfied.
- **DEFECT-2 (low, backend) — RESOLVED.** Both `config::tests` (`defaults_apply_without_env`, `blank_or_dotless_mock_domain_degrades_to_path_fallback`) now serialize via `crate::testutil::env_lock()`. Full suite at **default parallelism** is green and deterministic: **81 unit + 12 integration tests pass, 0 failed** (re-run confirmed, no flake). AC-55 build gate is now deterministic.
- **Live smoke (binary, :8099, seeded SQLite):** `/healthz` → 200; `GET /` (P3 SPA) → 200; `POST /api/session` → 200 with the frozen §5.2 shape (`owner_id`, `owner_secret`, `endpoints[]`, `primary`); seed-on-first-run logged the demo owner + primary endpoint.
- **allPassed: true.** Gate `hookbox-sks.34` closed.
