"""Regression test for the Auto-CRUD atomicity fix (hookbox-65m): concurrent
writes to one collection must not lose updates."""
import asyncio

import httpx

from tests.helpers import auth


async def test_concurrent_patch_no_lost_update(live_server):
    async with httpx.AsyncClient(base_url=live_server, timeout=15) as c:
        s = (await c.post("/api/session", json={"email": "crud-cc@example.com"})).json()
        h = auth(s["owner_secret"])
        t = (await c.post("/api/endpoints", headers=h, json={"name": "c"})).json()["token"]
        await c.patch(f"/api/endpoints/{t}", headers=h, json={"auto_crud": True})
        iid = (await c.post(f"/e/{t}/items", json={"base": 1})).json()["id"]

        n = 12
        results = await asyncio.gather(
            *[c.patch(f"/e/{t}/items/{iid}", json={f"f{i}": i}) for i in range(n)]
        )
        assert all(r.status_code == 200 for r in results), sorted({r.status_code for r in results})

        got = (await c.get(f"/e/{t}/items/{iid}")).json()
        assert got["base"] == 1                                  # original field survives
        assert all(got.get(f"f{i}") == i for i in range(n)), \
            f"lost update: missing {[i for i in range(n) if got.get(f'f{i}') != i]}"
