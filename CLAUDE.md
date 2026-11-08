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

```bash
# Frontend (repo root) — Vite + React SPA in src/
pnpm install
pnpm typecheck            # tsc --noEmit
pnpm test                 # vitest (unit + component), jsdom
pnpm coverage             # vitest with v8 coverage
pnpm build                # dist/ (served by the Rust binary)
pnpm e2e                  # Playwright over `vite preview` + e2e/mock-backend.ts
E2E_PORT=4591 pnpm e2e    # parallel worktrees: give each its own preview port

# Backend — Rust crate in backend/
cd backend
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --release      # unit + tests/api + tests/wire_contract (golden bytes)
UPDATE_GOLDEN=1 cargo test --test wire_contract   # ONLY for an intentional wire change
```

`tests/wire_contract.rs` pins exact status/header/body bytes in
`backend/tests/golden/*.txt`; a refactor must pass it with zero golden edits.

## Architecture Overview

- **backend/** — one Rust (Axum + SQLite/sqlx) binary: the mock plane
  (`<token>.<MOCK_DOMAIN>/…` and `/e/<token>/…`, `src/interceptor/`), the
  management API (`/api/**`, route table in `src/routes/mod.rs`), the live feed
  (`/ws`, `/sse`) and the tunnel control channel. Data access lives in
  `src/repo/`, shared validation in `src/validate.rs`, header/WS helpers in
  `src/net/`, identity in `src/auth/` + `src/session/`.
- **Identity** — anonymous owner key in an HttpOnly `SameSite=Strict` cookie
  (Bearer still accepted for scripts/CLI); cookie calls need `X-HookBox: 1` +
  same-origin. See `docs/features/anonymous-cookie-identity/decision.md`.
- **src/** — Vite + React SPA. `src/api/` (owner client, public client,
  zod schemas, session store/bootstrap), `src/feed/` (live feed), `src/app/`
  (layouts), `src/screens/<feature>/` (one folder per screen).

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
  custom properties (`--bg-canvas`, `--text-primary`, `--accent`,
  `--card-<tone>`, `--shadow-nav`, …) for `:root` (light) and `.dark` (dark —
  DESIGN.md is light-only; dark is an original complement using the same warm
  undertone and blue accent). `src/design/contrast.test.ts` asserts WCAG AA
  for the key pairs in both themes.
- `src/design/tokens.ts` — the single source for the type scale, radii,
  shadows, widths, spacing and motion. `tailwind.config.ts` AND `src/lib/cn.ts`
  (tailwind-merge config) both import it, so custom classes like
  `text-body-sm` merge correctly and the two can't drift.
- Components and screens consume semantic Tailwind classes ONLY — never a
  raw hex, inline color, alpha-modified color var, arbitrary radius, or
  gradient. `e2e/no-hex.spec.ts` and `e2e/token-classes.spec.ts` enforce this.
- Functional status/semantic hues (success/info/warning/danger) and the
  method/served-by color-coding (GET/POST/…, rule/crud/mitm/…) are
  intentionally unchanged from DESIGN.md — that doc doesn't address them, and
  they carry previously-verified AA contrast that a repaint would need to
  re-verify.

Use the primitives instead of re-styling per screen:

| Need | Use |
|---|---|
| Buttons | `Button` variants: `primary` (**one per screen**), `tint` (DESIGN "ghost CTA"), `ghost`, `outline` (compact), `danger` (soft red — never filled), `danger-ghost`, `nav` (muted, `aria-current`), `link` |
| Content surfaces | `Card` (`tone`: surface / marigold / coral / sky / tint / peach / midnight; `padding`), `CardHeader` |
| Labels & status | `Pill` (tones, `sm`/`md`/`hero`), `Overline` |
| Page structure | `PageHeader`, `SettingsSection` / `MarketingSection`, `PublicPage` + `PublicHeader`, `FullPageMessage`, `SkipLink` |
| Forms | `Field` / `TextField` / `SelectField` / `SwitchField` / `Fieldset` (FieldContext wires id, description, invalid), `Select`, `NumberSlider`, `ListEditor` |
| States | `EmptyState`, `LoadingBlock`, `RetryAlert`, `InlineAlert` |
| Requests | `RequestCells` (feed rows, viewer rows, landing mockup), `RequestDetailTabs`, `RequestSubject` |
| Data & effects | `useResource`, `useAsyncAction`, `useDraft`, `useLifecycleEvents`, `useMediaQuery`, `apiPath()` |

Import boundaries (`e2e/import-boundaries.spec.ts`): `components/ui`,
`components/hookbox` and `components/layout` must stay safe for the
credential-free `/s/:code` viewer — no `@/api` barrel, owner client, session
store or feed imports (types come from `@/api/schemas`). Owner-only chrome
lives in `components/owner/`.

When adding or restyling UI: change tokens in `src/globals.css` /
`src/design/tokens.ts`, not component-local styles; add or extend a primitive
rather than copying class strings; re-run `pnpm test` and `pnpm e2e` (covers
`no-hex`, `token-classes`, `import-boundaries`, `reduced-motion`) and eyeball
`/_gallery` (dev-only primitives page) in both themes and at 360px before
calling a visual change done.
