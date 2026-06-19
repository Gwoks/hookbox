# Dashboard design-elevation — BRIEF (read first)

## Context
HookBox is a fully-built, working Beeceptor-class platform. The landing (`/`) and
dashboard (`/d/<token>`) **now render correctly** — a render bug (wrong Starlette
`TemplateResponse` signature → stub fallback) + a CDN-asset dependency were just
fixed. **All functionality already exists and works** and must be preserved:
- **Landing:** email entry → `POST /api/session` → localStorage → `/d/<token>`.
- **Dashboard:** endpoint switcher + "New endpoint"; Auto-CRUD toggle; **Rules
  manager + create/edit modal**; **Settings overlay** (MITM target URL, chaos %,
  latency, rate-limit, CORS); **live feed** (color-coded method badges, served-by
  chips, status colors, WS health pill, pause / "N new", mock-URL chips, keyboard
  nav, skeleton/empty states); **deep inspector** (Headers / Query / Body JSON-tree
  / Response Served / State & Tracing tabs, lazy-loaded).

## Goal (user request)
**Elevate the UI/UX and visual design** of the dashboard + landing into a polished,
**high-density, developer-centric** product where every functionality is clearly
presented and a pleasure to use. This is a **DESIGN + POLISH pass on a working UI**,
not a rebuild.

## LOCKED CONSTRAINTS — do not violate
1. **Stack:** server-rendered **Jinja2 + HTMX + Alpine.js + Tailwind**. No React, no
   build step.
2. **Assets are VENDORED LOCALLY** at `/static/vendor/` (`tailwind.js`,
   `alpine.min.js`, `htmx.min.js`). **NEVER add an external CDN / `src="http…"`** —
   it must render offline/airgapped. (That CDN dependency was the original bug.)
3. **Preserve ALL functionality and the frozen §5 API contract**
   (`docs/features/beeceptor-rewrite/prd.md` §5). Do not change request/response
   shapes, Alpine store APIs, or element IDs/hooks the JS relies on unless you
   update both sides.
4. **Frontend lane ONLY:** edit `templates/` + `static/`. **Never touch `app/`** (esp.
   `app/routes/ui.py` — the render fix lives there).
5. **Build on the existing design:** `docs/features/beeceptor-rewrite/ux.md` +
   `design.md` are the prior spec — elevate them, don't discard.
6. Keep **accessibility** (ARIA roles, `aria-live`, focus management, `prefers-
   reduced-motion`, ≥4.5:1 contrast) and the **XSS-safe `x-text`** rendering of
   captured request data (never `x-html`/`innerHTML`/`|safe` on captured data).

## Verification (orchestrator does this after implementation)
- `pytest` stays green (esp. `tests/test_ui_render.py` — real templates + no CDN).
- Headless-Chrome render of `/` and `/d/<token>` shows the elevated UI.
