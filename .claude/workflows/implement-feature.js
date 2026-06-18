export const meta = {
  name: 'implement-feature',
  description: 'Autonomously implement a locked HookBox PRD whose tasks live in beads: frontend ∥ backend drain their lanes, then QA (functionality + user POV), then a code-level security review gate — each with a looping fix cycle — then sync beads.',
  phases: [
    { title: 'Implement' },
    { title: 'QA' },
    { title: 'Fix' },
    { title: 'Security' },
    { title: 'Sync' },
  ],
}

// args.slug → the beads label feature:<slug>. Tasks were created by the PM in bd.
const slug = args && args.slug
if (!slug) throw new Error('implement-feature requires args.slug (the locked feature slug)')
const dir = `docs/features/${slug}`
const label = `feature:${slug}`
const MAX_QA_ROUNDS = 6        // safety cap; the loop normally ends when QA passes
const MAX_SECURITY_ROUNDS = 4  // safety cap; the loop normally ends when the gate is clean

const QA_SCHEMA = {
  type: 'object',
  required: ['allPassed', 'criteria'],
  additionalProperties: false,
  properties: {
    // true ONLY when functionality (ACs + contract) AND user journeys all pass
    allPassed: { type: 'boolean' },
    criteria: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'pass', 'evidence'],
        additionalProperties: false,
        properties: {
          id: { type: 'string' },
          pass: { type: 'boolean' },
          evidence: { type: 'string' },
        },
      },
    },
    userFlows: {
      type: 'array',
      items: {
        type: 'object',
        required: ['flow', 'works'],
        additionalProperties: false,
        properties: {
          flow: { type: 'string' },
          works: { type: 'boolean' },
          note: { type: 'string' },
        },
      },
    },
    bugsFiled: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'area'],
        additionalProperties: false,
        properties: {
          id: { type: 'string' },
          area: { type: 'string', enum: ['frontend', 'backend'] },
          description: { type: 'string' },
        },
      },
    },
  },
}

const SECURITY_SCHEMA = {
  type: 'object',
  required: ['passed', 'findings'],
  additionalProperties: false,
  properties: {
    // true ONLY when no unresolved finding remains (accepted info/low may be noted in the report)
    passed: { type: 'boolean' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'severity', 'area', 'title', 'evidence'],
        additionalProperties: false,
        properties: {
          id: { type: 'string' },
          severity: { type: 'string', enum: ['critical', 'high', 'medium', 'low', 'info'] },
          area: { type: 'string', enum: ['frontend', 'backend'] },
          title: { type: 'string' },
          evidence: { type: 'string' },   // file:line + why it is exploitable
        },
      },
    },
    bugsFiled: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'area'],
        additionalProperties: false,
        properties: {
          id: { type: 'string' },
          area: { type: 'string', enum: ['frontend', 'backend'] },
          severity: { type: 'string' },
          description: { type: 'string' },
        },
      },
    },
  },
}

const lanePrompt = (lane, dirPath, fLabel, what) =>
  `You are the ${lane}-engineer. ${what} for feature "${slug}". ` +
  `Loop: \`bd ready -l area:${lane},${fLabel} --claim --json\` → implement in your lane only, ` +
  `honoring ${dirPath}/prd.md §5 and ${dirPath}/architecture.md → \`bd close <id> -r "..."\`. ` +
  `Stop when the ready queue is empty.`

// Stage 1 — frontend ∥ backend drain their task lanes against the frozen contract + design.
phase('Implement')
await parallel([
  () => agent(lanePrompt('frontend', dir, label, 'Drain your beads task lane'), { label: 'frontend', phase: 'Implement', agentType: 'frontend-engineer' }),
  () => agent(lanePrompt('backend', dir, label, 'Drain your beads task lane'), { label: 'backend', phase: 'Implement', agentType: 'backend-engineer' }),
])

// Stage 2 — QA (functionality + user POV); engineers fix filed bugs; loop until pass, capped + stall-guarded.
let qa = null
let prevProgress = -1
let stall = 0
for (let round = 1; round <= MAX_QA_ROUNDS; round++) {
  phase('QA')
  qa = await agent(
    `You are the qa-engineer. Claim and validate the QA gate for feature "${slug}" (label ${label}) on BOTH lenses: ` +
    `functionality (${dir}/prd.md §4 ACs + §5 contract, verifying FE↔BE on both sides) AND user POV ` +
    `(walk every flow in ${dir}/journey.md, including error/empty states). File any defects as bd bugs routed to ` +
    `the owning lane (area:frontend/area:backend) and re-block the gate. Write ${dir}/qa-report.md. (QA round ${round}.)`,
    { label: `qa-round-${round}`, phase: 'QA', agentType: 'qa-engineer', schema: QA_SCHEMA },
  )

  if (!qa || qa.allPassed) break

  // progress = ACs passing + journeys working; stall-guard stops un-fixable spins.
  const acPass = (qa.criteria || []).filter((c) => c.pass).length
  const flowPass = (qa.userFlows || []).filter((f) => f.works).length
  const progress = acPass + flowPass
  if (progress <= prevProgress) {
    stall += 1
    if (stall >= 2) { log(`QA stalled at ${progress} passing for 2 rounds — stopping with open defects`); break }
  } else {
    stall = 0
  }
  prevProgress = progress
  log(`QA round ${round}: ${acPass} ACs + ${flowPass} flows pass; ${(qa.bugsFiled || []).length} defect(s) filed — engineers fixing`)

  // Engineers drain the newly filed bug issues from their lanes (same claim loop).
  phase('Fix')
  await parallel([
    () => agent(lanePrompt('frontend', dir, label, 'Drain newly filed bug issues'), { label: `fe-fix-${round}`, phase: 'Fix', agentType: 'frontend-engineer' }),
    () => agent(lanePrompt('backend', dir, label, 'Drain newly filed bug issues'), { label: `be-fix-${round}`, phase: 'Fix', agentType: 'backend-engineer' }),
  ])
}

// Stage 3 — Security gate: code-level review AFTER QA passes; engineers fix filed security
// bugs; loop until clean, capped + stall-guarded. Skipped when QA didn't fully pass — the
// security gate stays blocked behind the open QA gate, so sync would just report it open.
let sec = null
if (qa && qa.allPassed) {
  let prevOpen = Infinity
  let secStall = 0
  for (let round = 1; round <= MAX_SECURITY_ROUNDS; round++) {
    phase('Security')
    sec = await agent(
      `You are the security-engineer in REVIEW mode. Claim and validate the security gate for feature "${slug}" ` +
      `(label area:security,${label}) by reviewing the IMPLEMENTED code against ${dir}/prd.md §5 and the threats in ` +
      `${dir}/security.md (authz/IDOR, injection, SSRF, secrets, CSRF, WebSocket auth, DoS). File each finding as a bd ` +
      `bug routed to the owning lane (area:frontend/area:backend) and re-block the gate. Write ${dir}/security-report.md. ` +
      `(Security round ${round}.)`,
      { label: `security-round-${round}`, phase: 'Security', agentType: 'security-engineer', schema: SECURITY_SCHEMA },
    )

    if (!sec || sec.passed) break

    // progress = unresolved findings going DOWN; stall-guard stops un-fixable spins.
    const open = (sec.findings || []).length
    if (open >= prevOpen) {
      secStall += 1
      if (secStall >= 2) { log(`Security stalled at ${open} open finding(s) for 2 rounds — stopping with open defects`); break }
    } else {
      secStall = 0
    }
    prevOpen = open
    log(`Security round ${round}: ${open} open finding(s); ${(sec.bugsFiled || []).length} bug(s) filed — engineers fixing`)

    // Engineers drain the newly filed security bug issues from their lanes (same claim loop).
    phase('Fix')
    await parallel([
      () => agent(lanePrompt('frontend', dir, label, 'Drain newly filed security bug issues'), { label: `fe-sec-fix-${round}`, phase: 'Fix', agentType: 'frontend-engineer' }),
      () => agent(lanePrompt('backend', dir, label, 'Drain newly filed security bug issues'), { label: `be-sec-fix-${round}`, phase: 'Fix', agentType: 'backend-engineer' }),
    ])
  }
} else {
  log('QA did not fully pass — security gate stays blocked; skipping security review and proceeding to sync (which will report open issues)')
}

// Stage 4 — sync beads: claim the sync issue, reconcile JSONL, close epic + sync, push.
phase('Sync')
const sync = await agent(
  `Sync the beads graph for feature "${slug}" (label ${label}). Steps: ` +
  `(0) Claim the sync issue if it is ready: \`bd ready -l step:sync,${label} --claim --json\` (note its id). ` +
  `(1) \`bd export -o .beads/issues.jsonl\` to reconcile the interchange file with the DB. ` +
  `(2) If every "${label}" issue is closed except the epic and the sync issue, close the epic ` +
  `(\`bd close <epic-id> -r "feature complete"\`) and then the sync issue (\`bd close <sync-id> -r "synced"\`). ` +
  `(3) If a Dolt remote is configured, \`bd dolt push\`; if there is no remote, skip it silently — do not fail. ` +
  `Report: any issues still open under "${label}", and what you synced/pushed.`,
  { label: 'bd-sync', phase: 'Sync' },
)

return { slug, qa, qaReport: `${dir}/qa-report.md`, security: sec, securityReport: `${dir}/security-report.md`, sync }
