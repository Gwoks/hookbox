import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { session } from './session'
import { useSession, useSessionGate } from './use-session'

const fetchMock = vi.fn()
const SUMMARY = { owner_id: 'owner_a', endpoints: [], primary: null }

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

beforeEach(() => {
  fetchMock.mockReset()
  vi.stubGlobal('fetch', fetchMock)
  localStorage.clear()
  session.invalidate()
})
afterEach(() => vi.unstubAllGlobals())

describe('useSession', () => {
  it('reflects the store and reports cookie support', () => {
    const { result } = renderHook(() => useSession())
    expect(result.current).toMatchObject({ status: 'unknown', ownerId: null, hasSession: false })
    expect(result.current.cookiesEnabled).toBe(navigator.cookieEnabled)

    act(() => session.markSignedIn('owner_a'))
    expect(result.current).toMatchObject({ status: 'signed-in', ownerId: 'owner_a', hasSession: true })
  })

  it('does not bootstrap on its own', () => {
    renderHook(() => useSession())
    expect(fetchMock).not.toHaveBeenCalled()
  })
})

describe('useSessionGate', () => {
  it('bootstraps an unknown session', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(SUMMARY))
    const { result } = renderHook(() => useSessionGate())
    expect(result.current.status).toBe('unknown')
    await waitFor(() => expect(result.current.status).toBe('signed-in'))
    expect(result.current.ownerId).toBe('owner_a')
    expect(result.current.bootstrapError).toBeNull()
  })

  it('does not call the server when the status is already known', () => {
    session.markSignedOut()
    const { result } = renderHook(() => useSessionGate())
    expect(result.current.status).toBe('signed-out')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('exposes a network failure and retries on demand', async () => {
    fetchMock.mockRejectedValueOnce(new TypeError('offline')).mockResolvedValueOnce(jsonResponse(SUMMARY))
    const { result } = renderHook(() => useSessionGate())
    await waitFor(() => expect(result.current.bootstrapError).not.toBeNull())
    expect(result.current.status).toBe('unknown')

    act(() => result.current.retry())

    await waitFor(() => expect(result.current.status).toBe('signed-in'))
    expect(result.current.bootstrapError).toBeNull()
  })

  it('re-bootstraps when another tab invalidates the session', async () => {
    fetchMock.mockImplementation(async () => jsonResponse(SUMMARY))
    const { result } = renderHook(() => useSessionGate())
    await waitFor(() => expect(result.current.status).toBe('signed-in'))

    act(() => session.invalidate())

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(result.current.status).toBe('signed-in'))
  })
})
