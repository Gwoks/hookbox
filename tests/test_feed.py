"""Integration tests for the owner-gated real-time feed (§5.4, OQ-4)."""
import asyncio
import json

import httpx
import websockets


def _owner(base, email):
    with httpx.Client(base_url=base) as c:
        s = c.post("/api/session", json={"email": email}).json()
    return s["owner_secret"], (s.get("primary") or {}).get("token")


async def test_ws_bad_cap_refused_with_4401(live_server):
    secret, token = _owner(live_server, "ws-bad@example.com")
    ws_base = live_server.replace("http://", "ws://")
    refused = False
    try:
        async with websockets.connect(f"{ws_base}/ws/{token}?cap=wrong", open_timeout=5) as ws:
            try:
                await asyncio.wait_for(ws.recv(), timeout=4)
            except Exception as e:  # noqa: BLE001
                refused = _is_4401(e)
            if not refused:
                refused = getattr(ws, "close_code", None) == 4401
    except Exception as e:  # noqa: BLE001
        refused = _is_4401(e)
    assert refused


async def test_ws_good_cap_hello_and_delivery(live_server):
    secret, token = _owner(live_server, "ws-good@example.com")
    ws_base = live_server.replace("http://", "ws://")
    async with websockets.connect(f"{ws_base}/ws/{token}?cap={secret}", open_timeout=5) as ws:
        hello = json.loads(await asyncio.wait_for(ws.recv(), timeout=4))
        assert hello["type"] == "hello"
        async with httpx.AsyncClient(base_url=live_server) as c:
            await c.get(f"/e/{token}/ping?x=1")
        got = None
        for _ in range(6):
            m = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
            if m.get("type") == "new_request":
                got = m
                break
        assert got and got["data"]["path"] == "/ping"


def test_sse_gate(live_server):
    secret, token = _owner(live_server, "sse-gate@example.com")
    with httpx.Client(base_url=live_server) as c:
        with c.stream("GET", f"/sse/{token}", timeout=5) as r:
            assert r.status_code == 401
        with c.stream("GET", f"/sse/{token}?cap=wrong", timeout=5) as r:
            assert r.status_code == 401
        with c.stream("GET", f"/sse/{token}?cap={secret}", timeout=5) as r:
            assert r.status_code == 200 and "event-stream" in r.headers.get("content-type", "")


def _is_4401(e) -> bool:
    if getattr(e, "code", None) == 4401:
        return True
    rcvd = getattr(e, "rcvd", None)
    if rcvd is not None and getattr(rcvd, "code", None) == 4401:
        return True
    return "4401" in str(e)
