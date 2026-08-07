/** F3 config export/import — the ConfigBundle FILE contract (operator-toolkit
 * prd.md §5.5.6). This is a client/file format: it is never a request body
 * and has no Rust counterpart. Pure and DOM-free (no fetch, no File API) so
 * it is unit-testable without a browser. */
import { z } from 'zod'
import {
  chaosModeSchema,
  defaultModeSchema,
  mockRuleCreateSchema,
  type EndpointConfigPatch,
  type EndpointDetail,
  type MockRule,
  type MockRuleCreate,
} from '@/api'
import { t } from './copy'

export const CONFIG_BUNDLE_VERSION = 1
export const MAX_BUNDLE_BYTES = 5_000_000
export const MAX_BUNDLE_RULES = 200

/** The nine portable EndpointConfigPatch fields, ALL required (nullable
 * where the API is nullable) — architecture D13. `.strict()` so `token`,
 * `mock_url`, `path_url`, `created_at`, `last_hit`, `request_count` and
 * `tunnel_active` are rejected rather than silently accepted (AC-13). */
export const configBundleEndpointSchema = z
  .object({
    name: z.string().nullable(),
    auto_crud: z.boolean(),
    target_url: z.string().nullable(),
    default_mode: defaultModeSchema,
    latency_ms: z.number().int(),
    rate_limit_per_min: z.number().int(),
    chaos_pct: z.number().int(),
    chaos_mode: chaosModeSchema,
    cors_enabled: z.boolean(),
  })
  .strict()

/** `.strict()` at the top level too — rejects any key other than the three
 * below, AND (because `configBundleEndpointSchema` is itself `.strict()`) a
 * nested unknown key inside `endpoint` is rejected as well (AC-S22). Rule
 * objects reuse the non-strict `mockRuleCreateSchema` [existing] so a stray
 * `id`/`token`/`created_at` inside a rule is stripped rather than fatal. */
export const configBundleSchema = z
  .object({
    hookbox_config_version: z.literal(CONFIG_BUNDLE_VERSION),
    exported_at: z.string(),
    endpoint: configBundleEndpointSchema,
    rules: z.array(mockRuleCreateSchema).max(MAX_BUNDLE_RULES),
  })
  .strict()

export type ConfigBundle = z.infer<typeof configBundleSchema>
export type ConfigBundleEndpoint = ConfigBundle['endpoint']

// Compile-time guard (AC-13): keeps the bundle's endpoint type in lockstep
// with EndpointConfigPatch. All nine bundle fields are required, and
// EndpointConfigPatch's are all `.optional()`, so a required value always
// satisfies an optional one — this fails to typecheck only if a field is
// renamed, retyped incompatibly, or dropped from either side.
const _assignable: EndpointConfigPatch = {} as ConfigBundleEndpoint
void _assignable

const BUNDLE_ENDPOINT_FIELDS = [
  'name',
  'auto_crud',
  'target_url',
  'default_mode',
  'latency_ms',
  'rate_limit_per_min',
  'chaos_pct',
  'chaos_mode',
  'cors_enabled',
] as const satisfies readonly (keyof ConfigBundleEndpoint)[]

/** Project an EndpointDetail down to the nine portable fields, in the exact
 * shape both `buildBundle` and `computeConfigDiff` share. */
export function toBundleEndpoint(endpoint: EndpointDetail): ConfigBundleEndpoint {
  return {
    name: endpoint.name,
    auto_crud: endpoint.auto_crud,
    target_url: endpoint.target_url,
    default_mode: endpoint.default_mode,
    latency_ms: endpoint.latency_ms,
    rate_limit_per_min: endpoint.rate_limit_per_min,
    chaos_pct: endpoint.chaos_pct,
    chaos_mode: endpoint.chaos_mode,
    cors_enabled: endpoint.cors_enabled,
  }
}

/** Project a MockRule down to the MockRuleCreate shape, omitting `id`,
 * `token`, `created_at` (AC-14). */
export function toBundleRule(rule: MockRule): MockRuleCreate {
  return {
    name: rule.name,
    priority: rule.priority,
    enabled: rule.enabled,
    match: rule.match,
    response: rule.response,
    state_writes: rule.state_writes,
    latency_ms: rule.latency_ms,
    rate_limit_per_min: rule.rate_limit_per_min,
    chaos_mode: rule.chaos_mode,
    webhook_action: rule.webhook_action,
  }
}

/** Build the exported bundle from FRESHLY FETCHED server state (AC-87) —
 * callers must not pass in-memory/dirty form values. `rules` must already be
 * in the server's list order (`ORDER BY priority, id`). */
export function buildBundle(endpoint: EndpointDetail, rules: readonly MockRule[]): ConfigBundle {
  return {
    hookbox_config_version: CONFIG_BUNDLE_VERSION,
    exported_at: new Date().toISOString(),
    endpoint: toBundleEndpoint(endpoint),
    rules: rules.map(toBundleRule),
  }
}

export type ParseBundleResult =
  | { success: true; bundle: ConfigBundle }
  | { success: false; message: string }

function messageForIssue(issue: z.ZodIssue): string {
  const [head, sub] = issue.path
  if (head === 'rules' && typeof sub === 'number') {
    return t('set.config.import.invalid.rule', { index: sub + 1, reason: issue.message })
  }
  if (head === 'endpoint') {
    const field = sub !== undefined ? String(sub) : 'endpoint'
    return t('set.config.import.invalid.field', { field, reason: issue.message })
  }
  return t('set.config.import.invalid.shape', { reason: issue.message })
}

/** Parse + validate a config file's text, entirely offline — no network
 * write happens until this returns success (AC-16). Strips a leading UTF-8
 * BOM rather than surfacing it as a parser error (journey.md gap 26). Checks
 * are ordered cheapest-and-most-specific first so the reported message names
 * the real problem rather than a downstream schema complaint. */
export function parseBundle(rawText: string): ParseBundleResult {
  const text = rawText.charCodeAt(0) === 0xfeff ? rawText.slice(1) : rawText

  if (text.trim().length === 0) {
    return { success: false, message: t('set.config.import.invalid.empty') }
  }

  let json: unknown
  try {
    json = JSON.parse(text)
  } catch {
    return { success: false, message: t('set.config.import.invalid.json') }
  }

  if (typeof json !== 'object' || json === null || Array.isArray(json)) {
    return {
      success: false,
      message: t('set.config.import.invalid.shape', { reason: 'the file is not a JSON object' }),
    }
  }
  const obj = json as Record<string, unknown>

  if (obj.hookbox_config_version !== CONFIG_BUNDLE_VERSION) {
    return {
      success: false,
      message: t('set.config.import.wrongVersion', {
        version: String(obj.hookbox_config_version ?? 'missing'),
      }),
    }
  }

  if (Array.isArray(obj.rules) && obj.rules.length > MAX_BUNDLE_RULES) {
    return {
      success: false,
      message: t('set.config.import.tooManyRules', { n: obj.rules.length }),
    }
  }

  const parsed = configBundleSchema.safeParse(json)
  if (!parsed.success) {
    return { success: false, message: messageForIssue(parsed.error.issues[0]) }
  }
  return { success: true, bundle: parsed.data }
}

export interface ConfigDiffRow {
  field: keyof ConfigBundleEndpoint
  from: string | null
  to: string | null
}

function displayValue(v: string | number | boolean | null): string | null {
  return v === null ? null : String(v)
}

/** Only the fields that actually change, in the frozen field order —
 * "settings that stay the same aren't listed" (AC-S21). */
export function computeConfigDiff(
  current: ConfigBundleEndpoint,
  next: ConfigBundleEndpoint,
): ConfigDiffRow[] {
  return BUNDLE_ENDPOINT_FIELDS.filter((f) => current[f] !== next[f]).map((f) => ({
    field: f,
    from: displayValue(current[f]),
    to: displayValue(next[f]),
  }))
}

const REQUEST_HEADER_TAG_RE = /\{\{\s*request\.header\./

/** AC-S23: how many imported rules copy a request header into their
 * response body — a SHOULD-warn, not a block (the templating grammar is a
 * closed sandbox and `webhook_action` has no dispatch site, so this is not
 * SSTI/SSRF, just a "credentials might echo back" heads-up). */
export function countRulesUsingRequestHeaderTag(rules: readonly MockRuleCreate[]): number {
  return rules.filter((r) => REQUEST_HEADER_TAG_RE.test(r.response.body_template)).length
}
