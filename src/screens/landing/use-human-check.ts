/**
 * The landing's Turnstile state: whether a check is required (a site key was
 * built in), the current single-use token, and whether the widget failed.
 * Without a site key the check is off: `isReady` is always true and no token
 * is ever sent (the backend then skips verification too).
 */
import { useCallback, useRef, useState, type RefObject } from 'react'
import type { TurnstileChallengeHandle } from './turnstile-challenge'
import { getTurnstileSiteKey } from './turnstile-config'

export interface HumanCheck {
  /** Null when the check is disabled (no widget rendered). */
  siteKey: string | null
  /** The token to send with the next mint, if any. */
  token: string | undefined
  /** The mint may be attempted: the check is off, or a token is held. */
  isReady: boolean
  /** The widget errored (script blocked, unsupported browser, …). */
  hasFailed: boolean
  widgetRef: RefObject<TurnstileChallengeHandle>
  onToken: (token: string) => void
  onExpire: () => void
  onError: () => void
  /** Drop the spent token and start a fresh challenge. Call after EVERY
   * mint attempt — Turnstile tokens are single-use. */
  consume: () => void
}

export function useHumanCheck(): HumanCheck {
  const [siteKey] = useState(getTurnstileSiteKey)
  const [token, setToken] = useState<string | undefined>(undefined)
  const [hasFailed, setHasFailed] = useState(false)
  const widgetRef = useRef<TurnstileChallengeHandle>(null)

  const onToken = useCallback((next: string) => {
    setToken(next)
    setHasFailed(false)
  }, [])
  const onExpire = useCallback(() => setToken(undefined), [])
  const onError = useCallback(() => {
    setToken(undefined)
    setHasFailed(true)
  }, [])
  const consume = useCallback(() => {
    setToken(undefined)
    widgetRef.current?.reset()
  }, [])

  return {
    siteKey,
    token,
    isReady: siteKey === null || token !== undefined,
    hasFailed,
    widgetRef,
    onToken,
    onExpire,
    onError,
    consume,
  }
}
