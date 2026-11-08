/**
 * useAsyncAction — `pending` / `error` bookkeeping for a user-triggered async
 * operation (save, delete, revoke…), replacing the busy/try/finally blocks.
 * `run` never rejects: it resolves to `{ ok: true, value }` or
 * `{ ok: false, error }` so a forgotten `catch` can't become an unhandled
 * rejection. Overlapping runs: only the latest settles the state; nothing
 * updates after unmount.
 */
import { useCallback, useEffect, useRef, useState } from 'react'

export type AsyncResult<R> = { ok: true; value: R } | { ok: false; error: unknown }

export interface AsyncAction<A extends unknown[], R> {
  run: (...args: A) => Promise<AsyncResult<R>>
  pending: boolean
  error: unknown
  reset: () => void
}

export function useAsyncAction<A extends unknown[], R>(action: (...args: A) => Promise<R>): AsyncAction<A, R> {
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const actionRef = useRef(action)
  const latestCall = useRef(0)
  const mounted = useRef(true)

  useEffect(() => {
    actionRef.current = action
  })
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const run = useCallback(async (...args: A): Promise<AsyncResult<R>> => {
    const call = ++latestCall.current
    const settles = () => mounted.current && call === latestCall.current
    setPending(true)
    setError(null)
    try {
      const value = await actionRef.current(...args)
      if (settles()) setPending(false)
      return { ok: true, value }
    } catch (err) {
      if (settles()) {
        setPending(false)
        setError(err)
      }
      return { ok: false, error: err }
    }
  }, [])

  const reset = useCallback(() => setError(null), [])

  return { run, pending, error, reset }
}
