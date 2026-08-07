# Security report (code-level, post-QA): Operator Toolkit (slug: `operator-toolkit`)

**Gate:** `hookbox-mun.22` · **Verdict: FAIL — 5 findings filed, gate left open and re-blocked.**
Reviewed at commit `c17a0d5` (QA round 7 PASS) against `prd.md` §4.9 (AC-S1..AC-S27) + §5.1–§5.11,
`architecture.md` §3.2/§7 and this feature's DESIGN-mode `security.md` (S-1..S-19). Method: read the
shipped diffs (`5e15f73`, `c50c578`, `ad43798`, `3cb3db5`, `afd8152`), then **ran the real backend**
(`backend/target/debug/hookbox`, fresh DB, `MOCK_DOMAIN=mock.local`, `PUBLIC_BASE_URL=http://localhost:8123`,
`STATIC_DIR=dist`) and probed it anonymously. Every finding below is backed by a live response, not by
inspection alone.

**Headline.** QA's round-7 conclusion that *"no channel D exists"* is **wrong**. There is a fourth
variant of the D1 token-disclosure class, it is reachable with **no attacker setup and no operator
misconfiguration**, and I recovered the endpoint token from a share link and used it to write into the
operator's endpoint anonymously. Separately, the AC-S5/AC-S6/AC-S26 header + access-log hardening is
**specified correctly and delivered by no deployment** — QA verified those three ACs by reading
`deploy/nginx.conf` line numbers, and the directives sit in an nginx location that never handles the
response.

---

## 1. Findings

| id | Sev | Finding | Location | Exploitability | bd |
|----|-----|---------|----------|----------------|----|
| **F-1** | **HIGH** | **Channel D** — echo-mode `response_body` republishes token-bearing request headers that the public projection itself masks. Violates **AC-S4** mechanically. | `backend/src/interceptor/engine.rs:604-630` (`ECHO_PERSIST_HEADER_DROP`, `redact_echo_persisted_headers`), applied at `:396-408`; consumed unfiltered at `backend/src/routes/share.rs:313` | **Proven end-to-end.** Anonymous viewer → endpoint token → 2 anonymous writes into the endpoint; `request_count` 7→9; injected rows visible in the shared feed. | `hookbox-mun.37` |
| **F-2** | **MEDIUM** | `mask_token_in_value` is **case-sensitive**; nginx's `$host` is lowercase, so `proxy_set_header X-Forwarded-Host $host` delivers a case-folded token that survives the public filter. | `backend/src/routes/share.rs:103-108` (`s.contains(token)`) | Live: `x-forwarded-host: ixau3viom4.mock.local` published unmasked for token `ixaU3viom4`. Residual guess space = letter case only (≤2⁹ for a 10-char token). | `hookbox-mun.38` |
| **F-3** | **MEDIUM** | nginx `location /s/` is a **no-op**: `try_files … /index.html` performs an *internal redirect*, which re-runs location matching into `location /`. Both the log phase and the headers filter read the final `loc_conf`, so `access_log off` and all four `add_header`s never apply → **the share code is written to the nginx access log** (`$request` = `GET /s/<CODE>`). | `deploy/nginx.conf:21-28` | Same leak class as commit `47a267c` (`?cap=` on `/ws/`, `/sse/`). Anyone with access-log read (incl. log shippers, backups) holds a working, non-expiring share link. | `hookbox-mun.39` |
| **F-4** | LOW | `/s/{code}` served by the backend (the **shipped** nginx-less `docker-compose.yml`, and `cargo run`) carries **no** `X-Robots-Tag`, `nosniff`, `X-Frame-Options` or `Referrer-Policy` header. AC-S5's `X-Robots-Tag` MUST and AC-S26's subset are unmet. | `backend/src/routes/spa.rs:62-68` | Live `curl -D-` output shows only `content-type`, `x-hookbox-plane`, `content-length`, `date`. Viewer is framable; §2's "no embed support" unenforced. With F-3, **no** deployment sends these headers today. | `hookbox-mun.40` |
| **F-5** | LOW | Public resolver rate limit is **bypassed by a malformed query string** — axum's `Query` extractor rejects before the handler, so `check_share_rate_limit` never runs. | `backend/src/routes/share.rs:502`, `:574` (limiter inside the handler) vs `:556` (extractor) | Live: 200× `?limit=abc` → 200× `400`, **zero** `429`. No oracle and no amplification, but it falsifies AC-S15's stated 1200/min instance ceiling. | `hookbox-mun.41` |

### F-1 in detail — the fourth channel

`c50c578` closed channel C by rebuilding the persisted echo body from `redact_echo_persisted_headers()`,
which applies `redact()` plus a **3-name** drop list `["host","origin","referer"]`. That ports only
**half** of the `hookbox-mun.34` fix. `share.rs` closes the same class with **two** mechanisms — the name
drop list (`PUBLIC_REQUEST_HEADER_DROP`, `share.rs:93`) **and** a value-based mask
(`mask_token_in_value`, `share.rs:103`) applied to every surviving header value. Only the first was
ported to the persist path, and `share.rs:313` passes `response_body` straight through.

Result: any request header whose **value** contains the token but whose **name** is not
`host`/`origin`/`referer` is masked in the public `request_headers` map and published verbatim inside
the echo `response_body` of the **same row**:

```
row  16 POST /ingress   leak_in_response_body=['x-envoy-original-path','x-forwarded-uri','x-original-uri']
                        masked_in_request_headers=['x-envoy-original-path','x-forwarded-uri','x-original-uri']
row  15 POST /rfc7239   leak_in_response_body=['forwarded']          masked_in_request_headers=['forwarded']
row  14 POST /apache    leak_in_response_body=['x-forwarded-host','x-forwarded-server']
                        masked_in_request_headers=['x-forwarded-host','x-forwarded-server']
```

```
public request_headers['x-forwarded-host']       = "<redacted>"            <- HookBox masks it
public response_body.headers['x-forwarded-host'] = "oPp8tASu3i.mock.local" <- and publishes it
```

**AC-S4 (MUST, `prd.md:1312`) — "nothing masked in `request_headers` appears verbatim anywhere else in
the same public row" — is violated observably.** AC-S4 exists precisely to catch this class
*generically*; the shipped regression tests assert the three known channels one at a time instead, which
is why the fourth got through both `5e15f73`/`c50c578` and QA's 20-surface sweep.

**Reachability — no attacker setup, no operator misconfiguration.** `X-Forwarded-Host` and
`X-Forwarded-Server` are added by Apache `mod_proxy`'s `ProxyPass` **by default**; `X-Forwarded-Host` by
Caddy `reverse_proxy` and by Traefik; `X-Original-URI` by ingress-nginx auth flows; `X-Envoy-Original-Path`
by Envoy. Reproduced in **both** topologies — wildcard mock host *and* the default path-fallback
(`/e/<token>/…`) deployment. That is the same "fires by itself" bar QA applied in round 6 to classify
channel C as a defect rather than an advisory.

**Exploit chain (attacker holds only the share code — no account, no owner secret):**

```
attacker sees endpoint request_count = 7
  recovered token 'oPp8tASu3i' from row 16 response_body.headers['x-envoy-original-path'] = '/e/oPp8tASu3i/ingress'
  anonymous write -> 200
  anonymous write -> 200
request_count after anonymous writes = 9
injected rows now visible in the SHARED feed: ['/attacker-injected', '/attacker-injected']
```

With the token an anonymous viewer can additionally evict the shared evidence past `TRACE_CAP = 100`,
mutate CRUD collections when `auto_crud` is on, and force one upstream call to the operator's
`target_url` per hit. "Read-only share link" (§0 item 1) is false again.

**Fix (both F-1 and F-2 in one change):** give `redact_echo_persisted_headers` the token (`handle_mock`
already holds it at `engine.rs:41`) and mask any header **value** containing it, mirroring
`share.rs:103-108`; make that comparison **case-insensitive** in all three call sites. Do **not** extend
the name list — that leaves the next proxy header open. Add the AC-S4 generic invariant test (walk every
JSON leaf of a public row and assert no value masked in `request_headers` appears verbatim elsewhere).

---

## 2. Threat-surface checklist

| Surface | Status | Evidence |
|---|---|---|
| **AuthN — owner routes** | **covered** | All three share routes take `OwnerId`; live `GET /api/endpoints` without Bearer → 401 `unauthorized`. `POST /api/session` + the two `/api/share/**` routes are the only unauthenticated routes (`share.rs:620-629`, `router.rs:186-200`). |
| **AuthZ / IDOR — ownership** | **covered** | `assert_owns_endpoint` on all three owner routes (`share.rs:363,440,468`) → 404-not-403. Revoke is by integer `id` **and** `AND token = ?` (`share.rs:472-478`). |
| **IDOR — cross-endpoint trace ids (AC-35)** | **covered** | `SELECT * FROM request_logs WHERE id = ? AND token = ?` (`share.rs:590`). Live: another endpoint's trace id `19` via a valid share code → **404**. |
| **Tombstone liveness (AC-S9)** | **covered** | Revoke runs *before* `gone_at` (`api.rs:526-531`), and the resolver joins `s.revoked_at IS NULL AND e.gone_at IS NULL` per request (`share.rs:333-343`). Live: `DELETE /api/endpoints/{token}` → share list **404** on the next request. |
| **404 identity (AC-36/AC-S14)** | **covered** | Single `share_not_found()` (`share.rs:155`). Live: unknown / malformed / suffixed codes are byte-identical across status line, headers (`content-type`, `cache-control: no-store`, `x-hookbox-plane`) and body. `HEAD` → 200 and is rate-limited; unrouted verbs → 405. |
| **Existence oracle (AC-101)** | **covered** | Param validation precedes resolution (`share.rs:504-520`). Live: `?limit=999` → **422 for both** a live and a dead code. |
| **Public projection shape (AC-34/AC-102)** | **covered** | `PublicRequestSummary`/`PublicRequestDetail`/`PublicShareFeed` are standalone `#[derive(Serialize)]` structs (`models.rs:319-350`), built field-by-field; no `flatten`, no `skip`. `token`, `matched_rule_id`, `overhead_ms`, `trace`, `state_snapshot` absent — confirmed on live JSON. |
| **Response-header filter (AC-S1)** | **covered** | `x-hookbox-*` dropped by **prefix**; five credential headers masked (`share.rs:237-255`). Live: `access-control-allow-origin` echoing the wildcard `Origin` → `<redacted>` (channel A closed). |
| **Request-header filter (channel B)** | **covered** | `host`/`origin`/`referer` absent from every public row (`share.rs:263-277`). Live-confirmed. |
| **Echo persist redaction (AC-S3)** | **covered** | `authorization`/`cookie`/`x-owner-id` → `<redacted>` in the persisted echo body; client body untouched. Live-confirmed. |
| **Token absence, server-generated fields (AC-S2/AC-S4)** | **VULN** | **F-1** (`response_body`) and **F-2** (case-folded values). AC-S4's row-level invariant fails. |
| **SQL injection** | **covered** | Every statement in `share.rs` is a parameterised `sqlx::query*`; zero `format!`-built SQL. The one dynamic builder (`api.rs:468`) joins **fixed allow-listed column fragments** and binds every value (`bind_value`, `:485-502`). |
| **XSS / viewer (AC-67, AC-S13)** | **covered** | Zero `dangerouslySetInnerHTML` in the viewer graph; no user value reaches `href`/`src`/`srcdoc`/`on*`. The only `style` props are computed from a numeric `depth` (`json-tree.tsx:42,56,78`). `title={row.path}` / `title={endpoint.name}` are React-escaped attributes. `document.title` and the `<h1>` are static (`share-view.tsx:39,105`). `public-client.ts` roots the viewer graph at `http.ts` and never reaches `session.ts`; `e2e/viewer-import-graph.spec.ts` asserts it. |
| **CSRF (AC-S24)** | **covered** | No cookies anywhere; `doFetch` sends `credentials: 'omit'` (`src/api/http.ts:31`). Header-only capability ⇒ no CSRF token needed; regression holds. |
| **SSRF** | **covered** | `webhook_action` remains **inert** — parsed/stored/compiled only, **no dispatch site** in `backend/src`. F3 import calls exactly two routes (`settings.tsx:286` PATCH endpoint, `:721` POST rules). MITM keeps the pinned-IP SSRF guard (`proxy.rs`), and `location /api/share/` adds no new egress. |
| **Secrets — share code** | **covered** | Hashed at rest (`code_hash = sha256`, `share.rs:408`; migration `0002`); plaintext appears in exactly one response; never in the owner list, an error `detail`, a URL, or the F3 bundle. `grep -c <code> server.log` = **0**. No `TraceLayer` on the app router. |
| **Secrets — code entropy** | **covered** | `gen_share_code` = 24 CSPRNG bytes → base64url, **192 bits**, with a hard `n_bytes.max(16)` floor in `ids.rs:70` *and* a `Config` clamp (`config.rs:201`). Independent of token/owner/clock. |
| **Rate limiting (AC-S7/AC-S15)** | **covered, one gap** | `ad43798` is a real structural fix: per-**namespace** `DashMap`s (`limiter.rs:119-130`), each bounded at `MAX_BUCKETS_PER_NAMESPACE`, batch-trimmed to 90% so the O(n) scan amortises (`limiter.rs:85-100`). `share:<ip>` **cannot** evict `rl:<token>` — they are different maps. Both directions covered by unit tests. Live: 125 requests → 123×200 + 2×429 with `Retry-After: 1` and `cache-control: no-store`. Gap = **F-5** (malformed query bypasses the check). |
| **`last_used_at` write (AC-S10)** | **covered** | `tokio::spawn` after the 200 value is built, with the ≥60 s coalescing predicate in the `WHERE` clause (`share.rs:215-227`). Touches `share_links` only. |
| **DoS / body caps (AC-S17/S19/S20)** | **covered** | Truncation happens at row build on the **request** task before the spawn (`engine.rs:770-790` → `:803`), so only the ≤256 KB `Option<String>` crosses in; the 5 MB MITM worst case is not retained. `truncate_utf8` is boundary-safe (`helpers.rs:129-138`). No new awaited I/O on the mock path; `insert_trace` is still two statements. |
| **Panic handling (AC-S18)** | **covered** | `CatchPanicLayer` outermost (`router.rs:206`), with a panicking route mounted through the **real** layer stack in test (`router.rs:220-228`, `:661-680`). |
| **Host-header injection (AC-99/AC-S14)** | **covered** | `share_url` uses `public_base_url` only (`share.rs:147-149`); never `mock_url`'s wildcard form, never `Host`/`X-Forwarded-Host`. |
| **F3 import validation (AC-S21/S22)** | **covered** | `configBundleSchema` is `.strict()` at the top level **and** on `endpoint` (`config-bundle.ts:26-50`), so a nested unknown key is rejected; `MAX_BUNDLE_BYTES`/`MAX_BUNDLE_RULES` enforced before any write; explicit pre-apply diff + confirm dialog; rule objects intentionally reuse the non-strict `mockRuleCreateSchema` per AC-S22. |
| **Templating sandbox** | **covered** | Closed-grammar scanner; `t_request` exposes only `method`/`path`/`body`/`query.*`/`path.*`/`header.*`/`body.*` — **no endpoint/token tag** (`templating.rs:126-147`). Unknown tags returned verbatim. |
| **Viewer polling lifecycle (AC-S8)** | **covered** | List 404 terminal, 429 backs off per `Retry-After`, aborts in flight on unmount (`use-shared-feed.ts:52,91-100,140`). |
| **Access log / referrer (AC-S5/S6/S26/S27)** | **VULN** | **F-3** and **F-4**. `robots.txt` itself is correct and live-served (`Disallow: /s/`). `location /api/share/`'s `access_log off` **is** effective (proxy_pass, no try_files). |
| **CSP** | N/A — accepted | Deferred residual, `prd.md` §8-R16. Recorded, not a new finding. |

---

## 3. Advisories (recorded, not filed as defects)

* **A12 — the public projection discloses the webhook sender's source IP.** `x-real-ip` and
  `x-forwarded-for` are added by the operator's own reverse proxy, not by the caller, and are published
  verbatim to anonymous viewers (live: `"x-real-ip": "203.0.113.9"`). This is inside the frozen §5.11
  contract ("request headers: as stored") and generically disclosed by AC-S11, but `copy.md`'s framing —
  *"Anything a caller **sends** in a header"* — misdescribes it, and these are structurally identical to
  the `host`/`origin`/`referer` set that AC-43 already drops. **Recommendation for the next PRD
  revision:** add `x-real-ip` / `x-forwarded-for` to `PUBLIC_REQUEST_HEADER_DROP`, or amend the
  disclosure copy. Not blocking — it is a contract question, not an implementation defect.
* **A13 — the global ceiling is shared-fate.** Ten IPs at 120/min exhaust `share:__global__` (1200/min)
  and deny every share viewer instance-wide. Explicitly accepted as a "courtesy limit, not a guarantee"
  in §8-R14; recorded only.
* **A10 / A11 (carried from QA §7.3) — confirmed as advisories.** An operator-authored
  `{{request.header.host}}` template, and an MITM/tunnel upstream that echoes request headers in its own
  body. Both require operator or upstream action and A11's bytes are the upstream's own (AC-72). I agree
  with QA's classification. Note that A10 becomes cheaper to hit once F-1 is fixed only on the echo path
  — the fix should be reviewed for whether the same value mask belongs on the rule-render path too.
* **Timing.** A malformed code short-circuits before any DB read while a well-formed unknown code costs
  one indexed probe — a measurable difference, but not an existence oracle (both are "not found"), and
  192-bit codes behind 120/min make enumeration irrelevant. No finding.
* **Error disclosure.** Every public failure returns the flat `{error, detail}` envelope with fixed
  strings; `sqlx::Error` maps to 503 `store_unavailable` with no driver text. No path, schema or version
  disclosure observed. No finding.

---

## 4. Verdict

**FAIL.** `hookbox-mun.22` stays **open**, blocked by `hookbox-mun.37`, `.38`, `.39`, `.40`, `.41`.
F-1 alone is blocking: AC-S1/AC-S2/AC-S4 are named in `prd.md` §0 item 1 as *blocking prerequisites for
shipping F4*, and AC-S4 is currently false.

Re-review after the fixes should re-run: (a) the AC-S4 generic row-walk against every `served_by` path
in **both** host topologies with a proxy-header set (`x-forwarded-host`, `x-forwarded-server`,
`forwarded`, `x-original-uri`, `x-forwarded-uri`, `x-envoy-original-path`) and in mixed **and** folded
case; (b) `GET /s/<CODE>` header + access-log assertions against a **running** nginx *and* against the
bare backend; (c) the malformed-query limiter counter.
