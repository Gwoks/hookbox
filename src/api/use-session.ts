/**
 * React bindings over the session status store (hookbox-a73.3).
 *
 * - `useSession()` only reads: status, anonymous owner id, and whether the
 *   browser accepts cookies (the key lives in one, so blocked cookies mean
 *   nothing can stay signed in).
 * - `useSessionGate()` also bootstraps: whenever the status is `unknown`
 *   (first render, or another tab signed in as a different owner) it asks
 *   the server once via `ensureSession()`. A network failure is exposed as
 *   `bootstrapError` with a `retry()` so the screen can render a retry.
 */
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { ensureSession } from './bootstrap'
import { session, type SessionState } from './session'

export interface SessionSnapshot extends SessionState {
  hasSession: boolean
  cookiesEnabled: boolean
}

export interface SessionGate extends SessionSnapshot {
  /** The last bootstrap failure (network / 5xx / 429), else null. */
  bootstrapError: unknown
  retry: () => void
}

function cookiesEnabled(): boolean {
  return typeof navigator === 'undefined' ? true : navigator.cookieEnabled
}

export function useSession(): SessionSnapshot {
  const state = useSyncExternalStore(session.subscribe, session.getSnapshot, session.getSnapshot)
  return useMemo(
    () => ({ ...state, hasSession: state.status === 'signed-in', cookiesEnabled: cookiesEnabled() }),
    [state],
  )
}

export function useSessionGate(): SessionGate {
  const snapshot = useSession()
  const [bootstrapError, setBootstrapError] = useState<unknown>(null)
  const [attempt, setAttempt] = useState(0)
  const isUnknown = snapshot.status === 'unknown'

  useEffect(() => {
    if (!isUnknown) return
    let cancelled = false
    setBootstrapError(null)
    ensureSession().catch((err: unknown) => {
      if (!cancelled) setBootstrapError(err ?? new Error('bootstrap failed'))
    })
    return () => {
      cancelled = true
    }
  }, [isUnknown, attempt])

  const retry = useCallback(() => setAttempt((n) => n + 1), [])

  return useMemo(() => ({ ...snapshot, bootstrapError, retry }), [snapshot, bootstrapError, retry])
}
