# Security Review Report — beeceptor-rewrite

**Date:** 2026-06-19 · **Scope of THIS report:** runtime + targeted-code
verification of every threat `security.md` prioritized (critical/high). The
autonomous `security-engineer` REVIEW gate did not complete (the agent stalled on a
streaming read), so the orchestrator performed this focused review against the
running app and the relevant modules. **An exhaustive line-by-line audit of all
~9,400 LOC remains a tracked follow-up: `hookbox-ej9`.**

## Verdict: the prioritized threats are mitigated

| # | Threat (security.md) | Mitigation verified | Evidence |
|---|----------------------|---------------------|----------|
| F1 | `/api/*` authz / cross-tenant IDOR | bearer = 256-bit secret, **sha256-hashed at rest**; `owner_id` (public) never accepted as credential; non-owner → **404** (never confirms existence) | `app/auth.py` (`require_owner`, `assert_owns_endpoint`); runtime: owner-B GET/PATCH owner-A → 404 |
| F2 | Email enumeration | new-vs-existing email return identical shape/status; secret rotates | `app/routes/api.py:157` (AC-S5) |
| F3 | SSRF via MITM `target_url` | http(s)-only; **resolved-IP block** of private/link-local/loopback/metadata; timeout + size caps; redirects off by default; owner cap never forwarded | `app/interceptor/proxy.py`, `config.py` (`MITM_*`); runtime: `169.254.169.254` & `127.0.0.1:6379` → blocked (≥400, 0 upstream hit) |
| F4 | SSTI / RCE in templating | hand-written single-pass scanner over a closed allow-list; **no** eval/exec/`str.format`/Jinja over user text; unknown tags left literal | `app/interceptor/templating.py`; runtime: `{{7*7}}`,`{{''.__class__}}`,`{{config}}` → inert literals |
| F5 | WS/SSE feed authz (OQ-4) | subscribing REQUIRES `?cap=<owner_secret>`, verified **before** `accept()`/first frame; SSE no/bad cap → **401**, WS bad cap → close **4401**; cross-owner → zero events | `app/websocket.py`, `app/auth.py:verify_cap_owns_token`; runtime confirmed |
| F6 | Stored XSS in inspector | captured request data bound via Alpine `x-text` (text nodes); **no** `x-html`/`innerHTML`/`\|safe` on captured data | `templates/partials/inspector*.html` (AC-S14/S15) |
| F7 | Secret leakage into traces | sensitive headers (`authorization`, `cookie`, `x-*-id`, `x-hookbox-cap`) scrubbed before persistence | `app/utils/helpers.py:28` |
| F8 | Credentialed CORS wildcard | `Access-Control-Allow-Credentials` **never** emitted; origin reflected (or `*`); P1-only (no wildcard CORS on `/api/*`) | `app/interceptor/cors.py` (AC-S16/S17); runtime confirmed absent |
| F9 | DoS / resource abuse | ingest body cap (413), per-endpoint rate limit (429), bounded latency/chaos, WS conn cap (50), template size/tag caps | `config.py` bounds; round-1 QA verified 413/429/conn-cap |
| F10 | Tunnel slug hijack | bind capability-gated (owner_secret over WS); cross-owner bind refused (close 4401); last-authed-bind-wins takeover | `app/routes/tunnel.py` (AC-S27) |
| F11 | Legacy insecure surface | `X-User-Id` header-trust, `/status` crypto route, GitHub auto-deploy webhook, SMTP backup all **removed** | `git` deletions; grep shows only doc-comment references |

## Deep audit (2 passes) — findings & fixes

Covered the `hookbox-ej9` scope. **Confirmed clean:** no eval/exec/SQLi/shell/pickle/
unsafe-yaml; **atomic Lua** rate-limiter (CRUD via MULTI/EXEC); **no ReDoS** (path
patterns `re.escape` user input); pure-ASGI plane middleware (the `Content-Length`
desync class is structurally impossible); CRUD path segments charset-validated (no
Redis-namespace injection); **`pip-audit`: no known vulnerabilities** in the
dependency tree; valid Compose config; `__import__` loader uses hardcoded internal
paths only.

**Two issues found and fixed:**
- **DNS-rebinding TOCTOU in the MITM SSRF guard** (`hookbox-zqd`, fixed): the guard
  resolved+checked the hostname's IPs but httpx re-resolved at connect, so a rebinding
  record could swap in an internal address. Fixed in `app/interceptor/proxy.py` by
  **pinning the connection to the validated IP** while preserving `Host` + TLS SNI /
  certificate verification. Verified 7/7 (real-HTTPS TLS preserved, metadata/loopback
  still blocked, connection pinned, Host/SNI intact).
- **Feed relay stalled by slow clients** (fixed): the pub/sub relay awaits
  `broadcast()` inline and `broadcast()` sent to WS clients **sequentially**, so one
  endpoint's slow clients (≤ cap × `WS_SEND_TIMEOUT_S`) could delay the **global**
  live feed. WS sends are now **concurrent** (`asyncio.gather`, per-client timeout) →
  bounded by a single timeout regardless of client count. Feed delivery re-verified
  over WS + SSE.

**One accepted limitation (tracked, low priority):**
- **CRUD write atomicity** — PUT/PATCH/DELETE read-modify-write the Redis list, so
  concurrent writes to one collection can lose an update. Acceptable for an ephemeral
  single-tenant mock store; revisit (WATCH/MULTI or Lua CAS) for high-concurrency use.

## Residual
No critical/high finding from `security.md` remains open; the `hookbox-ej9` audit
scope is covered. Multipart/upload bodies are captured as **raw bytes bounded by
`MAX_INGEST_BODY_BYTES` (413)** — no multipart parser is exposed to attacker input.
