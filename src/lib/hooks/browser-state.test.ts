import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useCountdown } from './use-countdown'
import { useDocumentVisible } from './use-document-visible'
import { subscribeLifecycle, useLifecycleEvents } from './use-lifecycle-events'
import { useMediaQuery } from './use-media-query'
import { useOnline } from './use-online'

function setVisibility(state: 'visible' | 'hidden') {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state })
  document.dispatchEvent(new Event('visibilitychange'))
}

function setOnline(online: boolean) {
  Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => online })
  window.dispatchEvent(new Event(online ? 'online' : 'offline'))
}

afterEach(() => {
  setVisibility('visible')
  setOnline(true)
})

describe('useDocumentVisible / useOnline', () => {
  it('follow visibilitychange and online/offline', () => {
    const visible = renderHook(() => useDocumentVisible())
    const online = renderHook(() => useOnline())
    expect(visible.result.current).toBe(true)
    expect(online.result.current).toBe(true)
    act(() => setVisibility('hidden'))
    act(() => setOnline(false))
    expect(visible.result.current).toBe(false)
    expect(online.result.current).toBe(false)
  })
})

describe('subscribeLifecycle / useLifecycleEvents', () => {
  it('routes the three browser events to handlers and unsubscribes', () => {
    const onVisibilityChange = vi.fn()
    const onOnline = vi.fn()
    const onOffline = vi.fn()
    const unsubscribe = subscribeLifecycle({ onVisibilityChange, onOnline, onOffline })
    setVisibility('hidden')
    setOnline(false)
    setOnline(true)
    expect(onVisibilityChange).toHaveBeenCalledWith(false)
    expect(onOffline).toHaveBeenCalledTimes(1)
    expect(onOnline).toHaveBeenCalledTimes(1)
    unsubscribe()
    setVisibility('visible')
    expect(onVisibilityChange).toHaveBeenCalledTimes(1)
  })

  it('the hook always calls the latest handlers', () => {
    const first = vi.fn()
    const second = vi.fn()
    const { rerender, unmount } = renderHook(({ handler }) => useLifecycleEvents({ onOnline: handler }), {
      initialProps: { handler: first },
    })
    rerender({ handler: second })
    setOnline(true)
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledTimes(1)
    unmount()
    setOnline(true)
    expect(second).toHaveBeenCalledTimes(1)
  })
})

describe('useCountdown', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('counts down once a second to zero, and is null when off', () => {
    const { result, rerender } = renderHook(({ s }) => useCountdown(s), {
      initialProps: { s: null as number | null },
    })
    expect(result.current).toBeNull()
    rerender({ s: 3 })
    expect(result.current).toBe(3)
    act(() => vi.advanceTimersByTime(1000))
    expect(result.current).toBe(2)
    act(() => vi.advanceTimersByTime(5000))
    expect(result.current).toBe(0)
    rerender({ s: null })
    expect(result.current).toBeNull()
  })

  it('restarts when the starting value changes', () => {
    const { result, rerender } = renderHook(({ s }) => useCountdown(s), { initialProps: { s: 5 as number | null } })
    act(() => vi.advanceTimersByTime(2000))
    expect(result.current).toBe(3)
    rerender({ s: 10 })
    expect(result.current).toBe(10)
  })
})

describe('useMediaQuery', () => {
  it('reads matchMedia and follows change events', () => {
    const listeners = new Set<() => void>()
    let matches = false
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (query: string) =>
        ({
          get matches() {
            return matches
          },
          media: query,
          addEventListener: (_: string, cb: () => void) => listeners.add(cb),
          removeEventListener: (_: string, cb: () => void) => listeners.delete(cb),
        }) as unknown as MediaQueryList,
    )
    const { result, unmount } = renderHook(() => useMediaQuery('(min-width: 768px)'))
    expect(result.current).toBe(false)
    act(() => {
      matches = true
      listeners.forEach((cb) => cb())
    })
    expect(result.current).toBe(true)
    unmount()
    expect(listeners.size).toBe(0)
  })
})
