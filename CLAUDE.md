# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->


## Build & Test

_Add your build and test commands here_

```bash
# Example:
# npm install
# npm test
```

## Architecture Overview

_Add a brief overview of your project architecture_

## Conventions & Patterns

### Design system

`DESIGN.md` (repo root) is the canonical visual style reference for the
frontend (`src/`) — a "Notion — warm paper notebook" identity: warm off-white
canvas, near-monochrome ink-black text hierarchy, a single blue accent
(`#0075de`) as the only filled-button color, hairline borders instead of
shadows on content cards, 12px card / 8px button / 4px small / pill (9999px)
border-radius, and Inter typography with tightened tracking on display/heading
sizes. Consult it before any new UI work — colors, spacing, radii, and
component treatments should match its Colors/Typography/Components/Do's-and-
Don'ts sections rather than being improvised per-screen.

The design is implemented as a **semantic-token system**, not per-component
styling:

- `src/globals.css` — the ONLY file where raw hex/rgba may appear. Defines
  the DESIGN.md primitive palette, then maps it to theme-aware semantic CSS
  custom properties (`--bg-canvas`, `--text-primary`, `--accent`, etc.) for
  `:root` (light) and `.dark` (dark — DESIGN.md is light-only; dark is an
  original complement using the same warm undertone and blue accent).
- `tailwind.config.ts` — surfaces those CSS vars as Tailwind utilities
  (`bg-canvas`, `text-text-primary`, `rounded-md`, …) plus the type scale,
  radius scale, and motion tokens.
- Components and screens consume semantic Tailwind classes ONLY — never a
  raw hex, inline color, or one-off shadow/radius value. `e2e/no-hex.spec.ts`
  enforces this for `src/components/**`.
- Functional status/semantic hues (success/info/warning/danger) and the
  method/served-by color-coding (GET/POST/…, rule/crud/mitm/…) are
  intentionally unchanged from DESIGN.md — that doc doesn't address them, and
  they carry previously-verified AA contrast that a repaint would need to
  re-verify.

When adding or restyling UI: change tokens in `src/globals.css` /
`tailwind.config.ts`, not component-local styles; re-run `pnpm e2e` (covers
`no-hex` and `reduced-motion`) and eyeball `/_gallery` (dev-only primitives
page) in both themes before calling a visual change done.
