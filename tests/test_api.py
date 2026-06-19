"""Integration tests for the §5 contract + core features (against a live server)."""
from tests.helpers import auth


def test_session_returns_secret_and_token(owner):
    secret, token = owner
    assert secret and token


def test_auth_required(client):
    assert client.get("/api/endpoints").status_code == 401


def test_auth_works_and_lists_own_endpoint(client, owner):
    secret, token = owner
    r = client.get("/api/endpoints", headers=auth(secret))
    assert r.status_code == 200 and any(e["token"] == token for e in r.json())


def test_idor_cross_owner_is_404(client, owner):
    secret_a, token_a = owner
    mallory = client.post("/api/session", json={"email": "mallory-idor@example.com"}).json()["owner_secret"]
    assert client.get(f"/api/endpoints/{token_a}", headers=auth(mallory)).status_code == 404
    assert client.patch(f"/api/endpoints/{token_a}", headers=auth(mallory), json={"name": "x"}).status_code == 404


def test_mock_serve_and_templating(client, owner):
    secret, token = owner
    rule = {
        "name": "h", "priority": 10, "enabled": True,
        "match": {"method": "GET", "path": "/hello"},
        "response": {"status_code": 201, "content_type": "application/json",
                     "body_template": "{\"q\":\"{{request.query.name}}\",\"u\":\"{{random 'uuid'}}\"}"},
    }
    assert client.post(f"/api/endpoints/{token}/rules", headers=auth(secret), json=rule).status_code in (200, 201)
    r = client.get(f"/e/{token}/hello?name=bob")
    assert r.status_code == 201
    body = r.json()
    assert body["q"] == "bob" and len(body["u"]) == 36


def test_auto_cors_preflight_never_credentialed(client, owner):
    _, token = owner
    r = client.request("OPTIONS", f"/e/{token}/x",
                       headers={"Origin": "https://a.test", "Access-Control-Request-Method": "GET"})
    assert r.status_code in (200, 204)
    assert r.headers.get("access-control-allow-origin") in ("*", "https://a.test")
    assert "access-control-allow-credentials" not in {k.lower() for k in r.headers}


def test_stateful_multi_step(client, owner):
    secret, _ = owner
    t = client.post("/api/endpoints", headers=auth(secret), json={"name": "s"}).json()["token"]
    client.post(f"/api/endpoints/{t}/rules", headers=auth(secret), json={
        "name": "login", "priority": 10, "enabled": True,
        "match": {"method": "POST", "path": "/login"}, "state_writes": [{"key": "auth", "value": "true"}],
        "response": {"status_code": 200, "content_type": "text/plain", "body_template": "in"}})
    client.post(f"/api/endpoints/{t}/rules", headers=auth(secret), json={
        "name": "secret", "priority": 10, "enabled": True,
        "match": {"method": "GET", "path": "/secret",
                  "state_requirements": [{"key": "auth", "op": "eq", "value": "true"}]},
        "response": {"status_code": 200, "content_type": "text/plain", "body_template": "ok-secret"}})
    assert "ok-secret" not in client.get(f"/e/{t}/secret").text   # gated before login
    client.post(f"/e/{t}/login")
    assert client.get(f"/e/{t}/secret").text == "ok-secret"        # gated open after login


def test_auto_crud_lifecycle(client, owner):
    secret, _ = owner
    t = client.post("/api/endpoints", headers=auth(secret), json={"name": "c"}).json()["token"]
    client.patch(f"/api/endpoints/{t}", headers=auth(secret), json={"auto_crud": True})
    bid = client.post(f"/e/{t}/books", json={"title": "Dune"}).json()["id"]
    assert client.get(f"/e/{t}/books").json()[0]["title"] == "Dune"
    assert client.get(f"/e/{t}/books/{bid}").status_code == 200
    assert client.delete(f"/e/{t}/books/{bid}").status_code == 204
    assert client.get(f"/e/{t}/books/{bid}").status_code == 404


def test_chaos_injects_5xx(client, owner):
    secret, token = owner
    client.patch(f"/api/endpoints/{token}", headers=auth(secret), json={"chaos_pct": 100})
    assert client.get(f"/e/{token}/anything").status_code in (502, 503, 504)


def test_unknown_404_and_deleted_410(client, owner):
    secret, _ = owner
    assert client.get("/e/zzznotokenxx/x").status_code == 404
    t = client.post("/api/endpoints", headers=auth(secret), json={"name": "g"}).json()["token"]
    client.delete(f"/api/endpoints/{t}", headers=auth(secret))
    assert client.get(f"/e/{t}/x").status_code == 410
