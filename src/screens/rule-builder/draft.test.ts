import { describe, expect, it } from 'vitest'
import type { MockRule } from '@/api/schemas'
import {
  BODY_CAP_BYTES,
  draftFromRule,
  draftToPayload,
  errorCountByTab,
  firstInvalidTab,
  insertAt,
  validateDraft,
} from './draft'

const RULE: MockRule = {
  id: 7,
  token: 'toka1111',
  name: 'Login success',
  priority: 10,
  enabled: false,
  match: {
    method: 'POST',
    path: '/login',
    headers: { 'x-env': 'test' },
    query: { debug: '1' },
    body_conditions: [{ path: '$.user.role', op: 'eq', value: 'admin' }],
    state_requirements: [{ key: 'logged_in', op: 'absent', value: null }],
  },
  response: {
    status_code: 201,
    headers: { 'x-mock': 'yes' },
    body_template: '{"id":"{{random \'uuid\'}}"}',
    content_type: 'application/json',
  },
  state_writes: [{ key: 'logged_in', value: 'true' }],
  latency_ms: 250,
  rate_limit_per_min: null,
  chaos_mode: 'dropout',
  webhook_action: { url: 'https://example.com/hook', body_template: '{}' },
  created_at: '2026-11-08T00:00:00Z',
}

describe('draftFromRule', () => {
  it('starts a new rule from the documented defaults', () => {
    const draft = draftFromRule(null)
    expect(draft).toMatchObject({
      name: '',
      priority: 100,
      enabled: true,
      method: 'ANY',
      path: '/*',
      status_code: 200,
      content_type: 'application/json',
      body_template: '',
      latency_ms: null,
      rate_limit_per_min: null,
      chaos_mode: null,
      webhook_url: '',
    })
    expect(draft.headers).toEqual([])
  })

  it('gives every row a unique uid', () => {
    const draft = draftFromRule(RULE)
    const uids = [...draft.headers, ...draft.query, ...draft.resp_headers, ...draft.body_conditions, ...draft.state_writes].map(
      (row) => row.uid,
    )
    expect(new Set(uids).size).toBe(uids.length)
  })
})

describe('draftToPayload', () => {
  it('round-trips a rule losslessly (uids stripped)', () => {
    const { id: _id, token: _token, created_at: _createdAt, ...create } = RULE
    expect(draftToPayload(draftFromRule(RULE))).toEqual(create)
  })

  it('drops blank rows, nulls a blank name and the webhook without a URL', () => {
    const draft = {
      ...draftFromRule(null),
      name: '   ',
      headers: [{ uid: 'a', key: ' ', value: 'x' }],
      body_conditions: [{ uid: 'b', path: '', op: 'eq' as const, value: '' }],
      state_writes: [{ uid: 'c', key: '', value: 'v' }],
      webhook_url: '  ',
      webhook_body: 'ignored',
    }
    const payload = draftToPayload(draft)
    expect(payload.name).toBeNull()
    expect(payload.match.headers).toEqual({})
    expect(payload.match.body_conditions).toEqual([])
    expect(payload.state_writes).toEqual([])
    expect(payload.webhook_action).toBeNull()
  })
})

describe('validateDraft', () => {
  it('accepts a valid draft', () => {
    expect(validateDraft(draftFromRule(RULE))).toEqual({})
    expect(firstInvalidTab({})).toBeNull()
  })

  it('flags the path, status and body cap, counted per tab', () => {
    const errors = validateDraft({
      ...draftFromRule(null),
      path: 'login',
      status_code: 600,
      body_template: 'x'.repeat(BODY_CAP_BYTES + 1),
    })
    expect(Object.keys(errors).sort()).toEqual(['body_template', 'path', 'status_code'])
    expect(errorCountByTab(errors)).toEqual({ matching: 1, response: 2, templating: 0, actions: 0, throttling: 0 })
    expect(firstInvalidTab(errors)).toBe('matching')
  })

  it('counts UTF-8 bytes, not characters, against the cap', () => {
    const multiByte = 'é'.repeat(BODY_CAP_BYTES / 2 + 1)
    expect(validateDraft({ ...draftFromRule(null), body_template: multiByte }).body_template).toBeDefined()
  })
})

describe('insertAt', () => {
  it('replaces the selection and reports the caret after the insert', () => {
    expect(insertAt('ab', '{{x}}', { start: 1, end: 1 })).toEqual({ text: 'a{{x}}b', caret: 6 })
    expect(insertAt('abc', 'Z', { start: 0, end: 2 })).toEqual({ text: 'Zc', caret: 1 })
  })

  it('appends without a known caret', () => {
    expect(insertAt('ab', 'Z', null)).toEqual({ text: 'abZ', caret: 3 })
  })
})
