import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useAsyncAction } from './use-async-action'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('useAsyncAction', () => {
  it('tracks pending and returns {ok, value} on success', async () => {
    const gate = deferred<number>()
    const { result } = renderHook(() => useAsyncAction((n: number) => gate.promise.then((v) => v + n)))
    let outcome: Awaited<ReturnType<typeof result.current.run>> | undefined
    let running!: Promise<void>
    act(() => {
      running = result.current.run(1).then((r) => {
        outcome = r
      })
    })
    expect(result.current.pending).toBe(true)
    await act(async () => {
      gate.resolve(41)
      await running
    })
    expect(outcome).toEqual({ ok: true, value: 42 })
    expect(result.current.pending).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('captures a failure as {ok:false} and in `error` — never an unhandled rejection', async () => {
    const boom = new Error('nope')
    const { result } = renderHook(() =>
      useAsyncAction(async () => {
        throw boom
      }),
    )
    let outcome: unknown
    await act(async () => {
      outcome = await result.current.run()
    })
    expect(outcome).toEqual({ ok: false, error: boom })
    expect(result.current.error).toBe(boom)
    expect(result.current.pending).toBe(false)
  })

  it('reset clears the error; a new run clears it too', async () => {
    let fail = true
    const { result } = renderHook(() =>
      useAsyncAction(async () => {
        if (fail) throw new Error('x')
        return 'ok'
      }),
    )
    await act(async () => void (await result.current.run()))
    expect(result.current.error).not.toBeNull()
    act(() => result.current.reset())
    expect(result.current.error).toBeNull()
    await act(async () => void (await result.current.run()))
    fail = false
    await act(async () => void (await result.current.run()))
    expect(result.current.error).toBeNull()
  })

  it('only the latest of overlapping runs settles the state', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const calls = [first, second]
    const { result } = renderHook(() => useAsyncAction(() => calls.shift()!.promise))
    let a!: Promise<unknown>
    let b!: Promise<unknown>
    act(() => {
      a = result.current.run()
      b = result.current.run()
    })
    await act(async () => {
      first.reject(new Error('stale'))
      await a
    })
    expect(result.current.pending).toBe(true)
    expect(result.current.error).toBeNull()
    await act(async () => {
      second.resolve('fresh')
      await b
    })
    expect(result.current.pending).toBe(false)
  })

  it('does not touch state after unmount', async () => {
    const gate = deferred<void>()
    const { result, unmount } = renderHook(() => useAsyncAction(() => gate.promise))
    let running!: Promise<unknown>
    act(() => {
      running = result.current.run()
    })
    unmount()
    gate.resolve()
    await expect(running).resolves.toEqual({ ok: true, value: undefined })
  })
})
