import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api as publicApi } from './public-client'
import { api, setUnauthorizedHandler } from './client'
import { session } from './session'

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

describe('owner credentials (cookie + X-HookBox)', () => {
  it('sends the cookie same-origin and the X-HookBox header on every owner call', async () => {
    fetchMock.mockResolvedValue(jsonResponse([]))
    await api.listEndpoints()
    const [, init] = fetchMock.mock.calls[0]
    expect(init.credentials).toBe('same-origin')
    expect(init.headers['X-HookBox']).toBe('1')
    expect(init.headers['Authorization']).toBeUndefined()
  })

  it('the public viewer client sends no cookie and no X-HookBox header', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ endpoint: { name: null, created_at: 'x', request_count: 0 }, requests: [] }),
    )
    await publicApi.getSharedRequests('code')
    const [, init] = fetchMock.mock.calls[0]
    expect(init.credentials).toBe('omit')
    expect(init.headers?.['X-HookBox']).toBeUndefined()
  })
})

describe('session routes', () => {
  const ENDPOINT = {
    token: 'tok',
    name: null,
    mock_url: 'https://tok.hookbox.test',
    path_url: 'http://localhost/e/tok',
    created_at: 'x',
    last_hit: null,
    request_count: 0,
  }
  const SUMMARY = { owner_id: 'owner_a', endpoints: [ENDPOINT], primary: ENDPOINT }

  it('maps each session method to its route, method and body', async () => {
    fetchMock.mockImplementation(async (url: string, init: RequestInit) => {
      if (url === '/api/session' && init.method === 'DELETE') return new Response(null, { status: 204 })
      if (url === '/api/session/key' || url === '/api/session/rotate') return jsonResponse({ key: 'k' })
      return jsonResponse(SUMMARY, init.method === 'POST' && url === '/api/session' ? 201 : 200)
    })

    await expect(api.getSession()).resolves.toEqual(SUMMARY)
    await expect(api.createSession()).resolves.toEqual(SUMMARY)
    await expect(api.restoreSession('the-key')).resolves.toEqual(SUMMARY)
    await expect(api.revealKey()).resolves.toEqual({ key: 'k' })
    await expect(api.rotateKey()).resolves.toEqual({ key: 'k' })
    await expect(api.signOut()).resolves.toBeUndefined()

    const seen = fetchMock.mock.calls.map(([url, init]) => [url, init.method, init.body ?? null])
    expect(seen).toEqual([
      ['/api/session', 'GET', null],
      ['/api/session', 'POST', null],
      ['/api/session/restore', 'POST', JSON.stringify({ key: 'the-key' })],
      ['/api/session/key', 'POST', null],
      ['/api/session/rotate', 'POST', null],
      ['/api/session', 'DELETE', null],
    ])
  })

  it('accepts a null primary in the session summary', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ owner_id: 'o', endpoints: [], primary: null }))
    await expect(api.getSession()).resolves.toEqual({ owner_id: 'o', endpoints: [], primary: null })
  })

  it('rejects a key response without a key', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ secret: 'k' }))
    await expect(api.revealKey()).rejects.toMatchObject({ code: 'contract_mismatch' })
  })
})

describe('401 handling', () => {
  afterEach(() => setUnauthorizedHandler(() => {}))

  it('a 401 on an owner route signs the tab out and bounces', async () => {
    const bounce = vi.fn()
    setUnauthorizedHandler(bounce)
    session.markSignedIn('owner_a')
    fetchMock.mockResolvedValue(jsonResponse({ error: 'unauthorized', detail: 'no' }, 401))

    await expect(api.listEndpoints()).rejects.toMatchObject({ status: 401 })

    expect(session.getSnapshot().status).toBe('signed-out')
    expect(bounce).toHaveBeenCalledTimes(1)
  })

  it('a 401 from GET /api/session or a restore never bounces', async () => {
    const bounce = vi.fn()
    setUnauthorizedHandler(bounce)
    session.invalidate()
    fetchMock.mockImplementation(async () => jsonResponse({ error: 'invalid_key', detail: 'bad key' }, 401))

    await expect(api.getSession()).rejects.toMatchObject({ status: 401 })
    await expect(api.restoreSession('nope')).rejects.toMatchObject({ code: 'invalid_key', status: 401 })

    expect(bounce).not.toHaveBeenCalled()
    expect(session.getSnapshot().status).toBe('unknown')
  })
})
