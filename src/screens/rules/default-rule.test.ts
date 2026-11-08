import { describe, expect, it } from 'vitest'
import { mockRuleCreateSchema, type EndpointDetail, type MockRule } from '@/api/schemas'
import { activeShadowedFallbacks, DEFAULT_CATCH_ALL_RULE, findCatchAllRule, hasCatchAllRule } from './default-rule'

const ENDPOINT: EndpointDetail = {
  token: 'toka1111',
  name: null,
  mock_url: 'https://toka1111.hookbox.test',
  path_url: 'http://localhost/e/toka1111',
  auto_crud: false,
  target_url: null,
  default_mode: 'mock_404',
  latency_ms: 0,
  rate_limit_per_min: 0,
  chaos_pct: 0,
  chaos_mode: 'error',
  cors_enabled: false,
  tunnel_active: false,
  created_at: '2026-11-08T00:00:00Z',
  last_hit: null,
  request_count: 0,
}

function rule(id: number, method: string, path: string, enabled = true): MockRule {
  return {
    ...mockRuleCreateSchema.parse({ ...DEFAULT_CATCH_ALL_RULE, enabled, match: { ...DEFAULT_CATCH_ALL_RULE.match, method, path } }),
    id,
    token: 'toka1111',
    created_at: '2026-11-08T00:00:00Z',
  } as MockRule
}

describe('DEFAULT_CATCH_ALL_RULE', () => {
  it('is a valid create payload that matches anything and sorts last', () => {
    expect(mockRuleCreateSchema.safeParse(DEFAULT_CATCH_ALL_RULE).success).toBe(true)
    expect(DEFAULT_CATCH_ALL_RULE.match).toMatchObject({ method: 'ANY', path: '/*' })
    expect(DEFAULT_CATCH_ALL_RULE.priority).toBe(1000)
    expect(DEFAULT_CATCH_ALL_RULE.name).toBe('Catch-all (default)')
  })
})

describe('catch-all detection', () => {
  it('finds a catch-all whether or not it is enabled (AC-61)', () => {
    const disabled = rule(1, 'ANY', '/*', false)
    expect(hasCatchAllRule([rule(2, 'GET', '/*'), disabled])).toBe(true)
    expect(findCatchAllRule([disabled])).toBe(disabled)
  })

  it('needs both ANY and /*', () => {
    expect(hasCatchAllRule([rule(1, 'GET', '/*'), rule(2, 'ANY', '/orders')])).toBe(false)
    expect(findCatchAllRule([])).toBeUndefined()
  })
})

describe('activeShadowedFallbacks', () => {
  it('lists only the fallbacks that are actually on, in resolution order (AC-122)', () => {
    expect(activeShadowedFallbacks(ENDPOINT)).toEqual([])
    expect(
      activeShadowedFallbacks({
        ...ENDPOINT,
        auto_crud: true,
        tunnel_active: true,
        target_url: 'https://api.example.com',
        default_mode: 'echo',
      }),
    ).toEqual([
      'Auto-CRUD stops serving this endpoint.',
      'Your tunnel stops receiving requests.',
      'Requests stop being proxied to your target URL.',
      'The Echo default response stops being used.',
    ])
  })
})
