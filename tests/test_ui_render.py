"""Regression: the UI routes must render the REAL templates, not the stub fallback.

A Starlette TemplateResponse signature bug (old `(name, context)` form on a modern
Starlette) made every UI page throw deep in Jinja; the route's `except` swallowed
it and returned a bare `<p>HookBox</p>` stub — which still 200s, so the original
status-only QA check passed while the actual UI was blank. These tests assert the
rendered HTML is the real template and that assets are vendored locally (no CDN)."""


def test_landing_renders_real_template_not_stub(client):
    r = client.get("/")
    assert r.status_code == 200
    body = r.text
    assert "Get my endpoint" in body          # the real landing form
    assert "<p>HookBox</p>" not in body         # NOT the stub fallback
    # assets are vendored locally — no <script src="http…"> to an external CDN
    assert "/static/vendor/" in body
    assert 'src="http' not in body
    assert 'src="//' not in body


def test_dashboard_renders_real_template_not_stub(client, owner):
    _, token = owner
    r = client.get(f"/d/{token}")
    assert r.status_code == 200
    body = r.text
    assert ("Live feed" in body) or ("Select a request" in body)   # the split-screen
    assert f"<p>dashboard {token}</p>" not in body                  # NOT the stub


def test_vendored_assets_are_served(client):
    for f in ("tailwind.js", "alpine.min.js", "htmx.min.js"):
        r = client.get(f"/static/vendor/{f}")
        assert r.status_code == 200, f
        assert len(r.content) > 1000, f
