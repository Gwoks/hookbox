# §9 OPEN-QUESTION RESOLUTIONS — human-approved (fold into prd.md, then §9 = empty)

The human reviewed the locked PRD and resolved all 5 blocking open questions on
2026-06-18. Fold these into `prd.md` verbatim (as ACs and/or §5 contract notes),
clear §9 Open Questions (move the originals to the §9 resolution log), and lock.

## OQ-4 — Live-feed capability gate  **[RESOLVES the architect-vs-security conflict in favor of SECURITY]**
**Decision: GATE THE FEED.** Subscribing to an endpoint's real-time feed
(WebSocket **and** SSE) REQUIRES the owner capability, verified server-side
**before** `accept()` / channel-join. Update **§5.4 of the frozen contract**:
- The feed client (`static/js/request-stream.js`) presents the `owner_secret`
  on connect (query param `?cap=<owner_secret>` or first-message handshake — the
  architect/§5.4 picks one and states it). The server resolves the endpoint,
  verifies the capability owns it, and only then accepts; otherwise it closes the
  socket with an auth error (e.g. WS close code 4401 / SSE 401) **before** sending
  any frame.
- The **mock interception plane stays fully public/unauthenticated** (clients must
  be able to hit the mock URL). Only the **observability feed** is owner-gated.
- This supersedes the prior "opportunistic no-secret WS auth" wording. Reflect it
  in AC-29/AC-30 (or add an AC) and SEC-AC-12 so QA + the security gate verify a
  non-owner cannot subscribe.

## OQ-1 — Expired/pruned endpoint status
**Decision:** Return **`410 Gone`** for a known-but-expired/pruned endpoint;
**`404`** (`unknown_endpoint`) for one that never existed. Lets the owner
dashboard distinguish "expired — re-create" from "typo/never-existed." Add to
§5.5 and an AC.

## OQ-2 — Chaos "dropout" semantics
**Decision:** Default chaos injection returns a random **5xx status** (502/503/504).
A raw **connection-drop** mode is **opt-in** per rule/endpoint. **Both** are
bounded by the same global rate/size caps as every other path (no unbounded abuse
vector). Honors prompt.txt §1.6 ("dropouts **or** random HTTP errors").

## OQ-3 — Redis-down degradation table  (per-feature fail-open vs fail-closed)
**Decision (authoritative table — add as an AC / §5.10 note):**
| Feature | Redis down behavior |
|---|---|
| Static mock matching | **Survives** — served from the in-process rule cache (no Redis on the match read path). |
| State-gated rules (read/require/mutate) | **Fail-CLOSED** — the state condition does not match (rule is skipped); never silently "match" a state-gated rule without state. |
| Auto-CRUD (data in Redis) | **503** — do not fabricate/lose data; surface a clear degraded error. |
| Rate limiter (token bucket) | **Fail-OPEN** — allow the request, but the global body/size caps (in-process, not Redis-backed) still apply, so it is not unbounded. |
| Real-time feed (pub/sub) | Mock serving + SQLite trace logging **unaffected**; the dashboard shows a **"degraded" pill** and may fall back to polling. |

## OQ-5 — Tunnel bind transport + slug contention
**Decision:** The `mock-tunnel` CLI authenticates with the **owner capability
(`owner_secret`) over the WebSocket control channel** (same capability model as
the rest of the app). **Slug contention: last authenticated bind wins** — a
second *correctly-authenticated* (owner) CLI binding an already-bound slug
**takes over**; the prior tunnel connection is closed with a clear "rebound
elsewhere" message. Because binding is capability-gated, cross-owner hijack is
impossible. Add to §5 (tunnel protocol) + an AC + SEC-AC for tunnel auth.

---
**After folding:** §9 Open Questions MUST be empty. The §5 contract is then FROZEN
(with the §5.4 feed-auth change above). Proceed to BREAKDOWN.
