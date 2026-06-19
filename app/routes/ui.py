"""Server-rendered UI routes (P3, arch §2 / routes/ui.py).

Only the dashboard shell pages are served here: the landing page (``/``) and the
split-screen dashboard (``/d/{token}``). All data is fetched client-side from the
``/api/*`` plane (the FE lane owns the templates + JS). These routes are reached
only when ``request.state.plane == "ui"`` (the plane middleware guarantees this on
the app host; mock-host traffic never lands here — AC-6).
"""

from __future__ import annotations

import logging
from pathlib import Path

from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse, JSONResponse
from fastapi.templating import Jinja2Templates

import config

logger = logging.getLogger("hookbox.ui")

router = APIRouter(tags=["UI"])

_BASE_DIR = Path(__file__).resolve().parent.parent.parent
templates = Jinja2Templates(directory=str(_BASE_DIR / "templates"))


def _ctx(request: Request, **extra) -> dict:
    base = {
        "request": request,
        "mock_domain": config.MOCK_DOMAIN,
        "path_fallback_only": config.PATH_FALLBACK_ONLY,
        "app_host": config.APP_HOST,
    }
    base.update(extra)
    return base


# NOTE: Starlette ≥0.29 uses TemplateResponse(request, name, context). The old
# (name, context) form silently mis-binds `name` to the context dict and raises
# deep in Jinja ("unhashable type: 'dict'"); the except blocks below must LOG that
# (not swallow it to a stub), or a render bug ships invisibly behind a 200.
@router.get("/", response_class=HTMLResponse)
async def landing(request: Request):
    """Landing / email entry. FE: POST /api/session → localStorage → /d/<token>."""
    try:
        return templates.TemplateResponse(request, "index.html", _ctx(request))
    except Exception:  # noqa: BLE001
        logger.exception("failed to render index.html")
        return HTMLResponse("<!doctype html><title>HookBox</title><p>HookBox</p>")


@router.get("/d/{token}", response_class=HTMLResponse)
async def dashboard(request: Request, token: str):
    """Dashboard split-screen for an endpoint. Ownership is enforced client-side
    via the stored capability hitting /api/* (the page itself is not secret)."""
    try:
        return templates.TemplateResponse(request, "dashboard.html", _ctx(request, token=token))
    except Exception:  # noqa: BLE001
        logger.exception("failed to render dashboard.html")
        return HTMLResponse(f"<!doctype html><title>HookBox</title><p>dashboard {token}</p>")
