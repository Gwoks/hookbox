import { describe, expect, it } from 'vitest'
import type { EndpointDetail } from '@/api/schemas'
import { formFromEndpoint, isFormDirty, toPatch, validateSettings } from './form'

const ENDPOINT: EndpointDetail = {
  token: 'toka1111',
  name: 'Checkout API',
  mock_url: 'https://toka1111.hookbox.test',
  path_url: 'http://localhost/e/toka1111',
  auto_crud: true,
  target_url: null,
  default_mode: 'mock_404',
  latency_ms: 25,
  rate_limit_per_min: 0,
  chaos_pct: 10,
  chaos_mode: 'error',
  cors_enabled: false,
  tunnel_active: false,
  created_at: '2026-11-08T00:00:00Z',
  last_hit: null,
  request_count: 3,
}

describe('settings form', () => {
  it('seeds every editable field from the endpoint (nulls as empty text)', () => {
    expect(formFromEndpoint(ENDPOINT)).toEqual({
      name: 'Checkout API',
      target_url: '',
      default_mode: 'mock_404',
      auto_crud: true,
      cors_enabled: false,
      latency_ms: 25,
      rate_limit_per_min: 0,
      chaos_pct: 10,
      chaos_mode: 'error',
    })
  })

  it('always builds the full nine-field PATCH, trimming text and nulling blanks', () => {
    const form = { ...formFromEndpoint(ENDPOINT), name: '   ', target_url: ' https://api.example.com ' }
    expect(toPatch(form)).toEqual({
      name: null,
      target_url: 'https://api.example.com',
      default_mode: 'mock_404',
      auto_crud: true,
      cors_enabled: false,
      latency_ms: 25,
      rate_limit_per_min: 0,
      chaos_pct: 10,
      chaos_mode: 'error',
    })
  })

  it('is dirty only when the PATCH would change something', () => {
    const base = formFromEndpoint(ENDPOINT)
    expect(isFormDirty(base, base)).toBe(false)
    expect(isFormDirty(base, { ...base, name: 'Checkout API  ' })).toBe(false)
    expect(isFormDirty(base, { ...base, latency_ms: 30 })).toBe(true)
  })

  it('accepts an empty or http(s) target URL and rejects anything else', () => {
    const base = formFromEndpoint(ENDPOINT)
    expect(validateSettings(base)).toEqual({})
    expect(validateSettings({ ...base, target_url: 'https://api.example.com' })).toEqual({})
    expect(validateSettings({ ...base, target_url: 'ftp://files.example.com' }).target_url).toBe(
      'Enter an http or https URL with a host.',
    )
    expect(validateSettings({ ...base, target_url: 'not a url' }).target_url).toBeDefined()
  })
})
