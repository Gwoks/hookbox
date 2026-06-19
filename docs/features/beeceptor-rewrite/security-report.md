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

## Deep audit (pass 1) — one finding, fixed
Confirmed: no eval/exec/SQLi/shell/pickle; **atomic Lua** rate-limiter (CRUD via
MULTI/EXEC); **no ReDoS** (path patterns `re.escape` user input); valid Compose
config. One finding, now **fixed & closed**:

- **DNS-rebinding TOCTOU in the MITM SSRF guard** (`hookbox-zqd`): the guard
  resolved+checked the hostname's IPs but httpx re-resolved at connect, so a
  rebinding record could swap in an internal address between check and connect. Fixed
  in `app/interceptor/proxy.py` by **pinning the connection to the validated IP**
  while preserving the `Host` header + TLS SNI / certificate verification for the
  hostname. Verified 7/7 (real-HTTPS TLS preserved, metadata/loopback still blocked,
  connection pinned to the checked IP, Host/SNI intact).

## Residual / follow-up
- **`hookbox-ej9`** — remaining exhaustive pass: WS/SSE backpressure under slow
  consumers, multipart/upload edges, dependency CVE scan, line-by-line
  crud/conditions/middleware/database. Recommended before any public/multi-tenant
  deployment.
- No critical/high finding is left open from `security.md`'s threat model.
