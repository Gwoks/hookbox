/**
 * The public viewer's frozen poll lifecycle (operator-toolkit AC-105/AC-S8),
 * driven with fake timers against a mocked public client.
 */
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/errors'
import type { PublicShareFeed } from '@/api/schemas'
import { useSharedFeed } from './use-shared-feed'

const getSharedRequests = vi.fn()
vi.mock('@/api/public-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/public-client')>()),
  api: { getSharedRequests: (...args: unknown[]) => getSharedRequests(...args) },
}))

const FEED: PublicShareFeed = {
  endpoint: { name: 'Checkout', created_at: '2026-11-08T00:00:00Z', request_count: 1 },
  requests: [
    { id: 1, method: 'GET', path: '/x', status_code: 200, served_by: 'rule', duration_ms: 3, timestamp: '2026-11-08T00:00:00Z' },
  ],
}

function setVisibility(state: 'visible' | 'hidden') {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state })
  document.dispatchEvent(new Event('visibilitychange'))
}
function setOnline(online: boolean) {
  Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => online })
  window.dispatchEvent(new Event(online ? 'online' : 'offline'))
}

/** Flush the mocked fetch's promise chain inside act. */
const settle = () => act(async () => {})
const advance = (ms: number) => act(async () => void vi.advanceTimersByTime(ms))

beforeEach(() => {
  vi.useFakeTimers()
  getSharedRequests.mockReset()
})
afterEach(() => {
  setVisibility('visible')
  setOnline(true)
  vi.useRealTimers()
})

describe('useSharedFeed', () => {
  it('a code change while a poll is in flight still loads the new code', async () => {
    // The first request never settles before the code changes (back/forward
    // between two share links, or StrictMode's double-invoked effect).
    getSharedRequests.mockImplementationOnce(() => new Promise(() => {}))
    getSharedRequests.mockResolvedValue(FEED)
    const { result, rerender } = renderHook(({ code }) => useSharedFeed(code), {
      initialProps: { code: 'first' },
    })
    rerender({ code: 'second' })
    await settle()
    expect(getSharedRequests).toHaveBeenLastCalledWith('second', expect.anything())
    expect(result.current.status).toBe('ready')
  })

  it('a 404 on one code does not stop polling for the next code', async () => {
    getSharedRequests.mockRejectedValueOnce(new ApiError('not_found', 'gone', 404))
    getSharedRequests.mockResolvedValue(FEED)
    const { result, rerender } = renderHook(({ code }) => useSharedFeed(code), {
      initialProps: { code: 'dead' },
    })
    await settle()
    expect(result.current.status).toBe('unavailable')
    rerender({ code: 'alive' })
    await settle()
    expect(getSharedRequests).toHaveBeenLastCalledWith('alive', expect.anything())
    expect(result.current.status).toBe('ready')
  })

  it('loads, then polls every 5s while visible', async () => {
    getSharedRequests.mockResolvedValue(FEED)
    const { result } = renderHook(() => useSharedFeed('code1'))
    expect(result.current.status).toBe('loading')
    await settle()
    expect(result.current.status).toBe('ready')
    expect(result.current.data?.requests).toHaveLength(1)
    expect(getSharedRequests).toHaveBeenCalledWith('code1', expect.objectContaining({ limit: 100 }))
    await advance(5000)
    expect(getSharedRequests).toHaveBeenCalledTimes(2)
  })

  it('(a) a list 404 is terminal: no more polls, even on refresh or focus', async () => {
    getSharedRequests.mockRejectedValue(new ApiError('not_found', 'gone', 404))
    const { result } = renderHook(() => useSharedFeed('code1'))
    await settle()
    expect(result.current.status).toBe('unavailable')
    act(() => result.current.refresh())
    act(() => setVisibility('visible'))
    await advance(120_000)
    expect(getSharedRequests).toHaveBeenCalledTimes(1)
  })

  it('(b) a 429 waits Retry-After, then retries once', async () => {
    getSharedRequests.mockRejectedValueOnce(new ApiError('rate_limited', 'slow', 429, 7)).mockResolvedValue(FEED)
    const { result } = renderHook(() => useSharedFeed('code1'))
    await settle()
    expect(result.current.status).toBe('rateLimited')
    expect(result.current.retryInSeconds).toBe(7)
    await advance(6999)
    expect(getSharedRequests).toHaveBeenCalledTimes(1)
    await advance(1)
    expect(getSharedRequests).toHaveBeenCalledTimes(2)
    expect(result.current.status).toBe('ready')
    expect(result.current.retryInSeconds).toBeNull()
  })

  it('(c) 5xx backs off 10s, 20s… keeping the last rows; success resets it', async () => {
    getSharedRequests
      .mockResolvedValueOnce(FEED)
      .mockRejectedValueOnce(new ApiError('error', 'x', 503))
      .mockRejectedValueOnce(new ApiError('error', 'x', 503))
      .mockResolvedValue(FEED)
    const { result } = renderHook(() => useSharedFeed('code1'))
    await settle()
    await advance(5000) // 2nd call fails
    expect(result.current.status).toBe('error')
    expect(result.current.data?.requests).toHaveLength(1)
    await advance(9999)
    expect(getSharedRequests).toHaveBeenCalledTimes(2)
    await advance(1) // 3rd call at +10s fails
    expect(getSharedRequests).toHaveBeenCalledTimes(3)
    await advance(19_999)
    expect(getSharedRequests).toHaveBeenCalledTimes(3)
    await advance(1) // 4th call at +20s succeeds
    expect(result.current.status).toBe('ready')
    await advance(5000)
    expect(getSharedRequests).toHaveBeenCalledTimes(5)
  })

  it('(d) never runs two polls at once', async () => {
    let resolve!: (feed: PublicShareFeed) => void
    getSharedRequests.mockImplementation(() => new Promise((res) => (resolve = res)))
    const { result } = renderHook(() => useSharedFeed('code1'))
    act(() => result.current.refresh())
    act(() => result.current.refresh())
    expect(getSharedRequests).toHaveBeenCalledTimes(1)
    await act(async () => resolve(FEED))
    expect(result.current.status).toBe('ready')
  })

  it('(e) pauses while hidden/offline, reports it, and resumes on visible/online', async () => {
    getSharedRequests.mockResolvedValue(FEED)
    const { result } = renderHook(() => useSharedFeed('code1'))
    await settle()
    act(() => setVisibility('hidden'))
    expect(result.current.isHidden).toBe(true)
    expect(result.current.paused).toBe(true)
    await advance(30_000)
    expect(getSharedRequests).toHaveBeenCalledTimes(1)

    act(() => setVisibility('visible'))
    await settle()
    expect(getSharedRequests).toHaveBeenCalledTimes(2)
    expect(result.current.paused).toBe(false)

    act(() => setOnline(false))
    expect(result.current.isOffline).toBe(true)
    act(() => setOnline(true))
    await settle()
    expect(result.current.isOffline).toBe(false)
    expect(getSharedRequests).toHaveBeenCalledTimes(3)
  })

  it('(f) aborts the in-flight fetch on unmount', async () => {
    let signal: AbortSignal | undefined
    getSharedRequests.mockImplementation((_code: string, opts: { signal: AbortSignal }) => {
      signal = opts.signal
      return new Promise(() => {})
    })
    const { unmount } = renderHook(() => useSharedFeed('code1'))
    unmount()
    expect(signal?.aborted).toBe(true)
  })
})
