import { describe, expect, it } from 'vitest'
import { apiPath, buildUrl } from './http'

describe('apiPath', () => {
  it('prefixes /api and encodes every segment on its own', () => {
    expect(apiPath('endpoints', 'ab12', 'rules', 7)).toBe('/api/endpoints/ab12/rules/7')
    expect(apiPath('endpoints', 'a b/c?d', 'collections', 'x#y')).toBe(
      '/api/endpoints/a%20b%2Fc%3Fd/collections/x%23y',
    )
  })

  it('matches the previous encodeURIComponent-per-segment output', () => {
    const token = 'tok/../en'
    expect(apiPath('endpoints', token)).toBe(`/api/endpoints/${encodeURIComponent(token)}`)
  })

  it('composes with buildUrl query strings', () => {
    expect(buildUrl(apiPath('endpoints', 't', 'requests'), { limit: 50, offset: undefined })).toBe(
      '/api/endpoints/t/requests?limit=50',
    )
  })
})
