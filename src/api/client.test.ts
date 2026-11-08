import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api as publicApi } from './public-client'
import { api } from './client'

const fetchMock = vi.fn()

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

beforeEach(() => {
  fetchMock.mockReset()
  vi.stubGlobal('fetch', fetchMock)
})
afterEach(() => vi.unstubAllGlobals())

describe('owner client paths and abort signals', () => {
  it('encodes path segments and forwards an AbortSignal', async () => {
    fetchMock.mockResolvedValue(jsonResponse([]))
    const controller = new AbortController()
    await api.listRules('a/b', { signal: controller.signal })
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/endpoints/a%2Fb/rules')
    expect(init.signal).toBe(controller.signal)
  })

  it('keeps query params and the signal on listRequests', async () => {
    fetchMock.mockResolvedValue(jsonResponse([]))
    const controller = new AbortController()
    await api.listRequests('tok', { limit: 100 }, { signal: controller.signal })
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/endpoints/tok/requests?limit=100')
    expect(init.signal).toBe(controller.signal)
  })

  it('encodes the collection name too', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ items: [] }))
    await api.peekCollection('tok', 'my coll')
    expect(fetchMock.mock.calls[0][0]).toBe('/api/endpoints/tok/collections/my%20coll')
  })

  it('builds DELETE/PATCH rule paths with numeric ids', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }))
    await api.deleteRule('tok', 12)
    expect(fetchMock.mock.calls[0][0]).toBe('/api/endpoints/tok/rules/12')
    expect(fetchMock.mock.calls[0][1].method).toBe('DELETE')
  })
})

describe('public client paths', () => {
  it('encodes the share code', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ endpoint: { name: null, created_at: 'x', request_count: 0 }, requests: [] }),
    )
    await publicApi.getSharedRequests('c/d', { limit: 100 })
    expect(fetchMock.mock.calls[0][0]).toBe('/api/share/c%2Fd/requests?limit=100')
  })
})
