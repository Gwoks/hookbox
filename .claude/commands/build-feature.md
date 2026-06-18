---
description: Single entry point — interactive PRD refinement, then autonomous build + QA.
argument-hint: <feature description>
allowed-tools: Agent, Workflow, Read, Write, Bash, Grep, Glob
---

You are the **orchestrator** for HookBox feature delivery. The feature request is:

> $ARGUMENTS

Run the two stages below. Subagents share state ONLY through files in
`docs/features/<slug>/` — pass each one the slug and the exact paths to read.

## Stage A — PRD (INTERACTIVE: the human is the final gate)

The whole workflow is a beads graph. Each step below is a tracked issue: claim
it (→ in_progress) before its agent runs, close it after. The dependency graph
keeps the order — `bd ready -l feature:<slug>` always shows the next step.

0. **Prereqs.** `bd stats` works (else tell the human to run `bd init`); the
   working tree has source (else `git restore .`).
1. Derive a kebab-case `<slug>`. Create `docs/features/<slug>/`.
2. **Bootstrap the graph.**
   `bash .claude/scripts/init-feature-graph.sh <slug> "<feature title>"`
   → creates the epic + discovery issues: `prd-draft` → `journey`/`ux`/`architecture`/`security`
   → `prd-revise` → `approval` (human gate) → `breakdown`.
3. **Draft.** Claim `bd ready -l step:prd-draft,feature:<slug> --claim --json`
   (note the id) → run agent `product-manager` (DRAFT) → `prd.md` →
   `bd close <id> -r "prd.md drafted"`.
4. **Critique + design (parallel).** `journey`/`ux`/`architecture`/`security` are
   now ready. For each: claim its step (`step:journey` / `step:ux` /
   `step:architecture` / `step:security`), run its agent (`user-journey`→`journey.md`,
   `ui-ux`→`ux.md`, `system-architect`→`architecture.md`, `security-engineer`
   (DESIGN)→`security.md`), then close it. Run the four in parallel.
4b. **Visual design (handoff from ui-ux).** `step:design` becomes ready once
   `step:ux` closes (it depends only on ux). Claim it (`step:design,feature:<slug>`),
   run agent `design-agent` (reads `ux.md` + templates → `design.md`, the
   visual/aesthetic layer built on top of the UX), then close it. It can run
   concurrently with `architecture`/`security` if those are still finishing.
5. **Revise.** Claim `step:prd-revise` → run `product-manager` (REVISE; folds the
   critiques + `design.md`'s visual direction & required visual ACs +
   `architecture.md` + `security.md`'s required security ACs / §5 notes into
   `prd.md`, lifting §5 verbatim) → close it.
6. **Human review (approval gate).** `step:approval` is now ready. Show the human
   prd.md's goal, ACs (§4), frozen contract (§5), and Open Questions (§9).
   - Feedback → re-run `product-manager` (REVISE) to update `prd.md`; repeat.
   - Never close the gate while §9 Open Questions is non-empty.
   - On explicit approval → `bd close <approval-id> -r "approved by <user>"`.
7. **Breakdown.** `step:breakdown` is now ready. Claim it → run `product-manager`
   (BREAKDOWN; resolves the existing epic and creates the build sub-graph —
   `area:<lane>` task issues, a QA gate depending on them, a sync issue depending
   on the gate) → close it. Show the human `bd dep tree <epic>`. The PRD is frozen.

## Stage B — build (AUTONOMOUS)

8. Call the **Workflow** tool: `{ name: "implement-feature", args: { slug: "<slug>" } }`.
   It runs frontend ∥ backend (drain bd lanes) → QA on two lenses (functionality
   *and* user POV) looping until both pass → a code-level **security review gate**
   looping with the engineers until clean → then syncs beads.
9. When it returns, report the outcome: acceptance criteria + user journeys that
   passed, the **security verdict + any findings**, any remaining defects (open bd
   bugs), the paths to `docs/features/<slug>/qa-report.md` and `security-report.md`,
   and what the sync pushed. Do not declare success unless QA's `allPassed` **and**
   security's `passed` are true — relay failures (and stalls) plainly.
