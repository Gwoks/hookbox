/**
 * The rule builder's working copy — pure, no React. A Draft holds every
 * §5.3 field in an editable shape: key/value maps become `{uid,key,value}`
 * rows (stable keys for the list editors), the webhook is split into two
 * strings. `draftToPayload` is the exact MockRuleCreate body Save sends
 * (uids stripped, blank rows dropped) and round-trips `draftFromRule`.
 */
import type { BodyCondition, MockRule, MockRuleCreate, StateRequirement, StateWrite } from '@/api/schemas'
import { t } from '@/lib/copy'

/** §5.3 / architecture: body_template ≤ 256000 bytes. */
export const BODY_CAP_BYTES = 256000
/** The counter turns to a warning at 90% of the cap. */
export const BODY_WARN_BYTES = Math.floor(BODY_CAP_BYTES * 0.9)
const STATUS_MIN = 100
const STATUS_MAX = 599

export const METHODS = ['ANY', 'GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

export interface PairRow {
  uid: string
  key: string
  value: string
}
export type BodyConditionRow = BodyCondition & { uid: string }
export type StateRequirementRow = StateRequirement & { uid: string }
export type StateWriteRow = StateWrite & { uid: string }
export type ChaosOverride = 'error' | 'dropout' | null

export interface Draft {
  name: string
  priority: number
  enabled: boolean
  method: string
  path: string
  headers: PairRow[]
  query: PairRow[]
  body_conditions: BodyConditionRow[]
  state_requirements: StateRequirementRow[]
  status_code: number
  content_type: string
  resp_headers: PairRow[]
  body_template: string
  state_writes: StateWriteRow[]
  webhook_url: string
  webhook_body: string
  latency_ms: number | null
  rate_limit_per_min: number | null
  chaos_mode: ChaosOverride
}

export type RuleTab = 'matching' | 'response' | 'templating' | 'actions' | 'throttling'
export const RULE_TABS: readonly RuleTab[] = ['matching', 'response', 'templating', 'actions', 'throttling']

export interface DraftErrors {
  path?: string
  status_code?: string
  body_template?: string
}

const FIELD_TAB: Record<keyof DraftErrors, RuleTab> = {
  path: 'matching',
  status_code: 'response',
  body_template: 'response',
}

let uidCounter = 0
export function newUid(): string {
  uidCounter += 1
  return `row-${uidCounter}`
}

export const withUid = <T extends object>(row: T): T & { uid: string } => ({ ...row, uid: newUid() })

const toPairs = (record: Record<string, string>): PairRow[] =>
  Object.entries(record).map(([key, value]) => ({ uid: newUid(), key, value }))

const toRecord = (rows: readonly PairRow[]): Record<string, string> =>
  Object.fromEntries(rows.filter((row) => row.key.trim()).map((row) => [row.key, row.value]))

function stripUid<T extends { uid: string }>({ uid: _uid, ...rest }: T): Omit<T, 'uid'> {
  return rest
}

export function draftFromRule(rule: MockRule | null): Draft {
  return {
    name: rule?.name ?? '',
    priority: rule?.priority ?? 100,
    enabled: rule?.enabled ?? true,
    method: rule?.match.method ?? 'ANY',
    path: rule?.match.path ?? '/*',
    headers: toPairs(rule?.match.headers ?? {}),
    query: toPairs(rule?.match.query ?? {}),
    body_conditions: (rule?.match.body_conditions ?? []).map(withUid),
    state_requirements: (rule?.match.state_requirements ?? []).map(withUid),
    status_code: rule?.response.status_code ?? 200,
    content_type: rule?.response.content_type ?? 'application/json',
    resp_headers: toPairs(rule?.response.headers ?? {}),
    body_template: rule?.response.body_template ?? '',
    state_writes: (rule?.state_writes ?? []).map(withUid),
    webhook_url: rule?.webhook_action?.url ?? '',
    webhook_body: rule?.webhook_action?.body_template ?? '',
    latency_ms: rule?.latency_ms ?? null,
    rate_limit_per_min: rule?.rate_limit_per_min ?? null,
    chaos_mode: rule?.chaos_mode ?? null,
  }
}

export function draftToPayload(draft: Draft): MockRuleCreate {
  const webhookUrl = draft.webhook_url.trim()
  return {
    name: draft.name.trim() || null,
    priority: draft.priority,
    enabled: draft.enabled,
    match: {
      method: draft.method,
      path: draft.path,
      headers: toRecord(draft.headers),
      query: toRecord(draft.query),
      body_conditions: draft.body_conditions.filter((c) => c.path.trim()).map(stripUid),
      state_requirements: draft.state_requirements.filter((s) => s.key.trim()).map(stripUid),
    },
    response: {
      status_code: draft.status_code,
      content_type: draft.content_type,
      headers: toRecord(draft.resp_headers),
      body_template: draft.body_template,
    },
    state_writes: draft.state_writes.filter((s) => s.key.trim()).map(stripUid),
    latency_ms: draft.latency_ms,
    rate_limit_per_min: draft.rate_limit_per_min,
    chaos_mode: draft.chaos_mode,
    // Sent whenever a URL is present so the shape round-trips (AC-J10 —
    // stored, not yet sent).
    webhook_action: webhookUrl ? { url: webhookUrl, body_template: draft.webhook_body } : null,
  }
}

export function utf8Bytes(text: string): number {
  return new TextEncoder().encode(text).length
}

/** Mirrors the §5.3 clamps; the server stays authoritative. */
export function validateDraft(draft: Draft): DraftErrors {
  const errors: DraftErrors = {}
  if (!draft.path.startsWith('/')) errors.path = t('rule.error.path.invalid')
  if (!Number.isInteger(draft.status_code) || draft.status_code < STATUS_MIN || draft.status_code > STATUS_MAX) {
    errors.status_code = t('rule.error.status.range')
  }
  if (utf8Bytes(draft.body_template) > BODY_CAP_BYTES) errors.body_template = t('rule.error.body.tooLarge')
  return errors
}

export function errorCountByTab(errors: DraftErrors): Record<RuleTab, number> {
  const counts: Record<RuleTab, number> = { matching: 0, response: 0, templating: 0, actions: 0, throttling: 0 }
  for (const field of Object.keys(errors) as (keyof DraftErrors)[]) counts[FIELD_TAB[field]] += 1
  return counts
}

export function firstInvalidTab(errors: DraftErrors): RuleTab | null {
  const counts = errorCountByTab(errors)
  return RULE_TABS.find((tab) => counts[tab] > 0) ?? null
}

export interface TextRange {
  start: number
  end: number
}

/** Insert `insert` over `range` (or at the end), returning the new text and
 * the caret position just after the insert. */
export function insertAt(text: string, insert: string, range: TextRange | null): { text: string; caret: number } {
  const start = range?.start ?? text.length
  const end = range?.end ?? text.length
  return { text: text.slice(0, start) + insert + text.slice(end), caret: start + insert.length }
}
