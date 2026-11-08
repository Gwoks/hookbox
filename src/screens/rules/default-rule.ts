/**
 * F6 "Add default rule" — the pure parts (no React, no client), so the
 * contract spec can import the frozen payload in Node.
 */
import type { EndpointDetail, MockRule, MockRuleCreate } from '@/api/schemas'
import { t } from '@/lib/copy'

/** The frozen §5.5.7 payload (as amended by copy.md/AC-125). `name` and the
 * body come from copy.ts so a copy edit never drifts from the request the
 * client actually sends (AC-58/AC-59). */
export const DEFAULT_CATCH_ALL_RULE: MockRuleCreate = {
  name: t('rules.default.ruleName'),
  priority: 1000,
  enabled: true,
  match: {
    method: 'ANY',
    path: '/*',
    headers: {},
    query: {},
    body_conditions: [],
    state_requirements: [],
  },
  response: {
    status_code: 200,
    headers: {},
    content_type: 'application/json',
    body_template: t('rules.default.bodyTemplate'),
  },
  state_writes: [],
  latency_ms: null,
  rate_limit_per_min: null,
  chaos_mode: null,
  webhook_action: null,
}

type RuleMatch = Pick<MockRule, 'match'>

/** AC-61: a catch-all is `ANY` + `/*` — enabled or not, so a DISABLED
 * catch-all also blocks adding a second one. */
export function findCatchAllRule<R extends RuleMatch>(rules: readonly R[]): R | undefined {
  return rules.find((r) => r.match.method === 'ANY' && r.match.path === '/*')
}

export function hasCatchAllRule(rules: readonly RuleMatch[]): boolean {
  return findCatchAllRule(rules) !== undefined
}

/** AC-122: the fallbacks a catch-all would silently shadow (the engine's
 * matched-rule short-circuit), one bullet each — and only those actually
 * switched on, in resolution order. */
export function activeShadowedFallbacks(endpoint: EndpointDetail | null): string[] {
  if (!endpoint) return []
  return [
    endpoint.auto_crud && t('rules.default.shadow.crud'),
    endpoint.tunnel_active && t('rules.default.shadow.tunnel'),
    Boolean(endpoint.target_url) && t('rules.default.shadow.proxy'),
    endpoint.default_mode === 'echo' && t('rules.default.shadow.echo'),
  ].filter((bullet): bullet is string => typeof bullet === 'string')
}
