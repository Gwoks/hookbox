import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ensureSession, LEGACY_STORAGE_KEYS, resolvePrimaryToken } from './bootstrap'
import { ApiError } from './errors'
import { session } from './session'
import type { SessionSummary } from './schemas'

const LEGACY_SECRET = 'legacy_secret_value_0123456789abcdef'
const fetchMock = vi.fn()

const ENDPOINT = {
  token: 'ab12cd34',
  name: null,
  mock_url: 'https://ab12cd34.hookbox.test',
  path_url: 'http://localhost/e/ab12cd34',
  created_at: '2026-11-08T00:00:00Z',
  last_hit: null,
  request_count: 0,
}

function summary(ownerId = 'owner_a'): SessionSummary {
  return { owner_id: ownerId, endpoints: [ENDPOINT], primary: ENDPOINT }
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function errorResponse(status: number, code: string): Response {
  return jsonResponse({ error: code, detail: code }, status)
}

function seedLegacy(): void {
  localStorage.setItem('hookbox-owner-secret', LEGACY_SECRET)
  localStorage.setItem('hookbox-owner-id', 'legacy_owner')
  localStorage.setItem('hookbox-owner-email', 'dev@example.com')
}

function legacyValues(): (string | null)[] {
  return LEGACY_STORAGE_KEYS.map((key) => localStorage.getItem(key))
}

function calls(): { url: string; method: string; body: unknown }[] {
  return fetchMock.mock.calls.map(([url, init]) => ({
    url: String(url),
    method: (init?.method as string) ?? 'GET',
    body: init?.body ? JSON.parse(init.body as string) : undefined,
  }))
}

beforeEach(() => {
  fetchMock.mockReset()
  vi.stubGlobal('fetch', fetchMock)
  localStorage.clear()
  session.invalidate()
})
afterEach(() => vi.unstubAllGlobals())

describe('ensureSession', () => {
  it('marks the tab signed in from GET /api/session and drops the legacy keys', async () => {
    seedLegacy()
    fetchMock.mockResolvedValueOnce(jsonResponse(summary('owner_a')))

    await expect(ensureSession()).resolves.toEqual(summary('owner_a'))

    expect(calls()).toEqual([{ url: '/api/session', method: 'GET', body: undefined }])
    expect(session.getSnapshot()).toEqual({ status: 'signed-in', ownerId: 'owner_a' })
    expect(legacyValues()).toEqual([null, null, null])
  })

  it('marks the tab signed out on a 401 when there is no legacy secret', async () => {
    fetchMock.mockResolvedValueOnce(errorResponse(401, 'unauthorized'))

    await expect(ensureSession()).resolves.toBeNull()

    expect(calls()).toHaveLength(1)
    expect(session.getSnapshot()).toEqual({ status: 'signed-out', ownerId: null })
  })

  it('swaps a legacy localStorage secret for the cookie, then deletes all three keys', async () => {
    seedLegacy()
    fetchMock
      .mockResolvedValueOnce(errorResponse(401, 'unauthorized'))
      .mockResolvedValueOnce(jsonResponse(summary('owner_legacy')))

    await expect(ensureSession()).resolves.toEqual(summary('owner_legacy'))

    expect(calls()).toEqual([
      { url: '/api/session', method: 'GET', body: undefined },
      { url: '/api/session/restore', method: 'POST', body: { key: LEGACY_SECRET } },
    ])
    expect(session.getSnapshot()).toEqual({ status: 'signed-in', ownerId: 'owner_legacy' })
    expect(legacyValues()).toEqual([null, null, null])
  })

  it('deletes the legacy keys when the server rejects the legacy secret', async () => {
    seedLegacy()
    fetchMock
      .mockResolvedValueOnce(errorResponse(401, 'unauthorized'))
      .mockResolvedValueOnce(errorResponse(401, 'invalid_key'))

    await expect(ensureSession()).resolves.toBeNull()

    expect(session.getSnapshot().status).toBe('signed-out')
    expect(legacyValues()).toEqual([null, null, null])
  })

  it('keeps the legacy keys and rejects when the restore hits a network error', async () => {
    seedLegacy()
    fetchMock.mockResolvedValueOnce(errorResponse(401, 'unauthorized')).mockRejectedValueOnce(new TypeError('offline'))

    await expect(ensureSession()).rejects.toBeInstanceOf(ApiError)

    expect(session.getSnapshot().status).toBe('unknown')
    expect(legacyValues()).toEqual([LEGACY_SECRET, 'legacy_owner', 'dev@example.com'])
  })

  it('keeps the legacy keys when the restore is rate limited', async () => {
    seedLegacy()
    fetchMock
      .mockResolvedValueOnce(errorResponse(401, 'unauthorized'))
      .mockResolvedValueOnce(errorResponse(429, 'rate_limited'))

    await expect(ensureSession()).rejects.toMatchObject({ status: 429 })

    expect(session.getSnapshot().status).toBe('unknown')
    expect(legacyValues()[0]).toBe(LEGACY_SECRET)
  })

  it('rejects and stays unknown when GET /api/session fails on the network', async () => {
    fetchMock.mockRejectedValueOnce(new TypeError('offline'))

    await expect(ensureSession()).rejects.toMatchObject({ code: 'network' })

    expect(session.getSnapshot().status).toBe('unknown')
  })

  it('shares one request between concurrent callers', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(summary('owner_a')))

    const [first, second] = await Promise.all([ensureSession(), ensureSession()])

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(first).toBe(second)
  })

  it('starts a fresh request once the previous one settled', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(summary('owner_a'))).mockResolvedValueOnce(jsonResponse(summary('owner_b')))

    await ensureSession()
    await ensureSession()

    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(session.getSnapshot().ownerId).toBe('owner_b')
  })

  it('still signs in when localStorage throws', async () => {
    const getItem = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    fetchMock.mockResolvedValueOnce(errorResponse(401, 'unauthorized'))

    await expect(ensureSession()).resolves.toBeNull()

    expect(getItem).toHaveBeenCalled()
    expect(session.getSnapshot().status).toBe('signed-out')
  })
})

describe('resolvePrimaryToken', () => {
  it('returns the primary token without a request', async () => {
    await expect(resolvePrimaryToken(summary())).resolves.toBe('ab12cd34')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('creates an endpoint when the owner has no primary', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        ...ENDPOINT,
        token: 'newtok99',
        auto_crud: false,
        target_url: null,
        default_mode: 'mock_404',
        latency_ms: 0,
        rate_limit_per_min: 0,
        chaos_pct: 0,
        chaos_mode: 'error',
        cors_enabled: false,
        tunnel_active: false,
      }),
    )

    await expect(resolvePrimaryToken({ owner_id: 'o', endpoints: [], primary: null })).resolves.toBe('newtok99')
    expect(calls()).toEqual([{ url: '/api/endpoints', method: 'POST', body: {} }])
  })
})
