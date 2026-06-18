# Feature artifacts

Each feature gets a folder `docs/features/<slug>/` that is the **shared memory**
for the agent pipeline (agents have isolated context — they coordinate only
through these files). Created and driven by the `/build-feature` command.

| File | Author | Consumed by |
|------|--------|-------------|
| `prd.md` | product-manager | everyone — the source of truth |
| `journey.md` | user-journey | product-manager (critique of the draft), qa-engineer (user-POV walkthrough) |
| `ux.md` | ui-ux | product-manager (critique of the draft), design-agent (visual handoff) |
| `design.md` | design-agent | product-manager (visual ACs), frontend-engineer (implements the styling) |
| `architecture.md` | system-architect | product-manager (§5 source), frontend/backend engineers, qa-engineer |
| `qa-report.md` | qa-engineer | the human (and the fix loop) |

**The whole workflow is a [beads](https://github.com/gastownhall/beads) (`bd`)
graph, not just the tasks.** `init-feature-graph.sh` creates the epic + the
*discovery* issues (`step:prd-draft` → `journey`/`ux`/`architecture`/`security`,
with `ux`→`design` → `prd-revise` → `approval` → `breakdown`); the PM's breakdown
then adds the *build* issues (`area:frontend|backend` tasks → `area:qa` gate →
`area:security` gate → `step:sync`).
Every issue carries `feature:<slug>` and dependencies encode the order, so
`bd ready -l feature:<slug>` always shows the next step and `bd dep tree <epic>`
shows the whole feature. The QA gate only unblocks once the tasks close; the sync
issue only once QA passes. The PRD's §10 records the epic id and issue→AC index.

## The two rules that make parallel autonomy work

1. **§5 Frozen interface contract** in `prd.md` (endpoint shapes, WebSocket
   message formats, data models) is frozen at PRD lock. Frontend and backend
   both code against it, so they never need to talk to each other.
2. **Lane ownership.** frontend-engineer touches only `templates/` + static
   assets; backend-engineer touches only `app/`, `config.py`,
   `requirements.txt`. Disjoint lanes → no merge conflicts when run in parallel.

## Flow

```
/build-feature "<request>"
        │
   bootstrap bd graph (init-feature-graph.sh)
        │
   Stage A · discovery issues (interactive, human-gated)
        │  prd-draft → journey ∥ ux ∥ architecture ∥ security; ux→design → prd-revise → approval → breakdown
        ▼
   Stage B · build issues (autonomous — implement-feature)
        │  frontend ∥ backend → QA gate (functionality + user POV, looping) → security gate → sync
        ▼
   qa-report.md + closed epic
```
