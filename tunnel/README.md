# `mock-tunnel` — HookBox local tunnel CLI

Reverse-tunnels public traffic hitting your HookBox endpoint
(`https://<slug>.<MOCK_DOMAIN>/…`) down to a server running on your **localhost**,
over a single multiplexed WebSocket control channel to the HookBox server.

This is the spec's **blueprint + reference CLI** (`_decisions.md` §8). A Go binary
would be the production-grade choice; this Python reference implements the frozen
control-channel protocol (§5.12 of the interface contract) exactly, so a Go
rewrite is a drop-in on the same wire format.

## How it fits the resolution order

A public request to your endpoint is resolved by HookBox as
(`§5.5` / `architecture.md` §4.6):

```
OPTIONS preflight → matching rule → Auto-CRUD → TUNNEL (if a CLI is bound) → MITM → default
```

So the tunnel only serves requests that **no local rule and no Auto-CRUD** already
handle. Tunnelled traffic still appears in your live dashboard feed labelled
`served_by: "tunnel"` and is subject to the same ingest-size and rate-limit caps as
every other request.

## Install

The CLI needs the `websockets` package (already pinned in the repo's
`requirements.txt`); the local HTTP replay uses only the Python standard library.

```bash
pip install -r requirements.txt        # from the repo root
# or just:  pip install websockets
```

## Usage

```bash
# Run as a module from the repo root:
python -m tunnel \
  --port 3000 \
  --endpoint <slug> \
  --server ws://localhost:8000 \
  --secret <owner_secret>
```

| Flag | Required | Default | Meaning |
| --- | --- | --- | --- |
| `--port`, `-p` | yes | — | Local port to forward public requests to (e.g. `3000`). |
| `--endpoint`, `-e` | yes | — | The HookBox endpoint slug/token to bind. **Must be owned by `--secret`.** |
| `--server`, `-s` | no | `ws://localhost:8000` | HookBox server WS URL (`ws://host[:port]` or `wss://host`). `http(s)://` is accepted and mapped to `ws(s)://`. |
| `--secret` | yes | — | Your **owner capability** (`owner_secret`). Authenticates the bind. |
| `--host` | no | `127.0.0.1` | Local host to forward to. |

Get your `owner_secret` by entering your email on the HookBox landing page; it is
stored in the dashboard's `localStorage` under `hookbox_owner.owner_secret`.

### Example

```bash
# Terminal 1 — your app under development:
python -m http.server 3000

# Terminal 2 — the tunnel:
python -m tunnel -p 3000 -e ab3cd5efgh -s ws://localhost:8000 --secret <owner_secret>
# → [mock-tunnel] Connecting to ws://localhost:8000/ws/tunnel/ab3cd5efgh …
# → [mock-tunnel] Bound to endpoint 'ab3cd5efgh'.
# → [mock-tunnel] Tunnel live: ws://localhost:8000/ws/tunnel/ab3cd5efgh -> http://127.0.0.1:3000

# Now: curl http://ab3cd5efgh.127.0.0.1.nip.io:8000/anything  → served by your local :3000
```

## Authentication & ownership (frozen — §5.12 / AC-S27)

The CLI presents `Authorization: Bearer <owner_secret>` on the WebSocket
handshake. The server verifies the capability **owns the slug before** registering
the tunnel:

- **Unauthenticated / wrong owner / slug not owned** → the server closes with WS
  code **`4401`**. The CLI prints an auth-failure message and **exits 2** — it does
  **not** retry an auth failure.
- **Cross-owner hijack is impossible**: because binding is capability-gated, only
  the slug's owner can bind it.

## Slug contention — last authenticated bind wins (takeover)

If a **second** correctly-authenticated owner CLI binds an already-bound slug, it
**takes over**: the server registers the new tunnel and closes the prior one with a
`{ "t": "err", "message": "rebound elsewhere" }` frame, then WS close `4409`. The
displaced CLI prints "rebound by another tunnel" and **exits cleanly (0)**.

## Reconnect & resilience (AC-41)

- Any **non-fatal** control-channel drop triggers exponential-backoff reconnect
  (`250ms → 500 → 1000 → 2000 → 4000 → 8000ms`, with jitter), then forwarding
  resumes automatically.
- An in-flight public request **during** a gap is failed **server-side** with a
  deterministic `504 {"error":"no_tunnel"}` (bounded by the server's
  `TUNNEL_REQUEST_TIMEOUT_S`) rather than hanging forever.
- When **no** tunnel is connected at all, public callers immediately get
  `504 {"error":"no_tunnel"}`.

## CLI status output (states)

| Line | Meaning |
| --- | --- |
| `Connecting to … (endpoint '<slug>') …` | dialling the control channel |
| `Bound to endpoint '<slug>'.` | bind accepted (capability owns the slug) |
| `Tunnel live: … -> http://<host>:<port>` | ready; forwarding public traffic |
| `<METHOD> <path> -> <status> (<n>B)` | one request forwarded + local response |
| `<METHOD> <path> -> LOCAL ERROR: …` | local dev server refused/failed (caller gets 504) |
| `Auth failed / not your endpoint (WS 4401).` | fatal: bad `--secret`/`--endpoint` (exit 2) |
| `… rebound by another tunnel … Exiting.` | fatal: a newer owner CLI took over (exit 0) |
| `Reconnecting in <s>s (attempt N) …` | transient drop; backing off |

## Control-channel protocol (frozen — §5.12)

JSON text frames over one WebSocket, multiplexed by request `id`:

```
→ client   {"t":"req", "id":N, "method":..., "path":..., "query":{}, "headers":{}, "body_b64":"…"}
← client   {"t":"res", "id":N, "status":200, "headers":{}, "body_b64":"…"}
← client   {"t":"err", "id":N, "message":"…"}        # local replay failed
↔          {"t":"ping"} / {"t":"pong"}               # keepalive
← server   {"t":"bound", "slug":"<slug>"}            # bind confirmation
← server   {"t":"err", "message":"rebound elsewhere"} # takeover (no id)
```

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | clean shutdown (Ctrl-C) or displaced by a takeover |
| `2` | authentication failed / bad arguments |
| `3` | the `websockets` package is not installed |
