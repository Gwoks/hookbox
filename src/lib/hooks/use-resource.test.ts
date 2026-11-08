import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useResource } from './use-resource'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('useResource', () => {
  it('setData invalidates an in-flight reload so a late response cannot overwrite it', async () => {
    let call = 0
    const late = deferred<string>()
    const { result } = renderHook(() =>
      useResource(() => (++call === 1 ? Promise.resolve('server-v1') : late.promise), []),
    )
    await waitFor(() => expect(result.current.data).toBe('server-v1'))
    act(() => void result.current.reload({ silent: true }))
    act(() => result.current.setData('saved-v2'))
    await act(async () => late.resolve('stale-v1'))
    expect(result.current.data).toBe('saved-v2')
    expect(result.current.refreshing).toBe(false)
  })

  it('goes loading → ready with the fetched data', async () => {
    const { result } = renderHook(() => useResource(async () => ['a'], []))
    expect(result.current.status).toBe('loading')
    await waitFor(() => expect(result.current.status).toBe('ready'))
    expect(result.current.data).toEqual(['a'])
    expect(result.current.error).toBeNull()
  })

  it('goes loading → error and keeps the error', async () => {
    const boom = new Error('boom')
    const { result } = renderHook(() =>
      useResource(async () => {
        throw boom
      }, []),
    )
    await waitFor(() => expect(result.current.status).toBe('error'))
    expect(result.current.error).toBe(boom)
  })

  it('aborts and ignores a stale request when deps change', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const signals: AbortSignal[] = []
    const { result, rerender } = renderHook(({ key }) =>
      useResource((signal) => {
        signals.push(signal)
        return key === 1 ? first.promise : second.promise
      }, [key]),
    { initialProps: { key: 1 } })

    rerender({ key: 2 })
    expect(signals[0].aborted).toBe(true)
    await act(async () => second.resolve('two'))
    await act(async () => first.resolve('one'))
    expect(result.current.data).toBe('two')
    expect(result.current.status).toBe('ready')
  })

  it('clears data when the key changes', async () => {
    const { result, rerender } = renderHook(({ key }) => useResource(async () => `v${key}`, [key]), {
      initialProps: { key: 1 },
    })
    await waitFor(() => expect(result.current.data).toBe('v1'))
    const gate = deferred<void>()
    rerender({ key: 2 })
    expect(result.current.data).toBeUndefined()
    expect(result.current.status).toBe('loading')
    gate.resolve()
    await waitFor(() => expect(result.current.data).toBe('v2'))
  })

  it('silent reload keeps data visible, flags refreshing, then swaps data', async () => {
    let call = 0
    const next = deferred<string>()
    const { result } = renderHook(() => useResource(() => (call++ === 0 ? Promise.resolve('old') : next.promise), []))
    await waitFor(() => expect(result.current.data).toBe('old'))

    act(() => {
      void result.current.reload({ silent: true })
    })
    expect(result.current.status).toBe('ready')
    expect(result.current.refreshing).toBe(true)
    expect(result.current.data).toBe('old')

    await act(async () => next.resolve('new'))
    expect(result.current.data).toBe('new')
    expect(result.current.refreshing).toBe(false)
  })

  it('a failed silent reload keeps the data and status, but exposes the error', async () => {
    let call = 0
    const boom = new Error('later')
    const { result } = renderHook(() =>
      useResource(() => (call++ === 0 ? Promise.resolve('kept') : Promise.reject(boom)), []),
    )
    await waitFor(() => expect(result.current.data).toBe('kept'))
    await act(async () => result.current.reload({ silent: true }))
    expect(result.current.status).toBe('ready')
    expect(result.current.data).toBe('kept')
    expect(result.current.error).toBe(boom)
  })

  it('a loud reload after an error goes back to loading, then ready', async () => {
    let call = 0
    const { result } = renderHook(() =>
      useResource(() => (call++ === 0 ? Promise.reject(new Error('x')) : Promise.resolve('ok')), []),
    )
    await waitFor(() => expect(result.current.status).toBe('error'))
    act(() => {
      void result.current.reload()
    })
    expect(result.current.status).toBe('loading')
    await waitFor(() => expect(result.current.status).toBe('ready'))
    expect(result.current.data).toBe('ok')
  })

  it('aborts on unmount and never updates afterwards', async () => {
    const pending = deferred<string>()
    let signal: AbortSignal | undefined
    const { result, unmount } = renderHook(() =>
      useResource((s) => {
        signal = s
        return pending.promise
      }, []),
    )
    unmount()
    expect(signal?.aborted).toBe(true)
    await act(async () => pending.resolve('late'))
    expect(result.current.status).toBe('loading')
  })

  it('setData replaces or updates the data in place', async () => {
    const { result } = renderHook(() => useResource(async () => [1, 2], []))
    await waitFor(() => expect(result.current.status).toBe('ready'))
    act(() => result.current.setData((prev) => [...(prev ?? []), 3]))
    expect(result.current.data).toEqual([1, 2, 3])
    act(() => result.current.setData([9]))
    expect(result.current.data).toEqual([9])
  })

  it('always calls the latest fetcher on reload', async () => {
    const a = vi.fn(async () => 'a')
    const b = vi.fn(async () => 'b')
    const { result, rerender } = renderHook(({ fetcher }) => useResource(fetcher, []), {
      initialProps: { fetcher: a },
    })
    await waitFor(() => expect(result.current.data).toBe('a'))
    rerender({ fetcher: b })
    await act(async () => result.current.reload())
    expect(b).toHaveBeenCalledTimes(1)
    expect(result.current.data).toBe('b')
  })
})
