/**
 * Cloudflare Turnstile for the landing's first session mint (action
 * `session_mint`). Managed mode with `appearance: interaction-only`: most
 * visitors never see it; the rare interactive challenge renders in place.
 *
 * Landing-only on purpose: it loads a third-party script, so it must stay
 * out of the viewer-safe component folders and the /s/:code module graph.
 * Tokens are single-use — the owner calls `reset()` after every attempt.
 */
import { forwardRef, useImperativeHandle, useMemo, useRef } from 'react'
import { Turnstile, type TurnstileInstance } from '@marsidev/react-turnstile'

export const TURNSTILE_ACTION = 'session_mint'

const WIDGET_OPTIONS = {
  action: TURNSTILE_ACTION,
  appearance: 'interaction-only',
  theme: 'auto',
} as const

export interface TurnstileChallengeHandle {
  /** Discard the current (single-use) token and run a fresh challenge. */
  reset: () => void
}

interface TurnstileChallengeProps {
  siteKey: string
  onToken: (token: string) => void
  onExpire: () => void
  /** The widget, its script or the browser failed; no token will arrive. */
  onError: () => void
}

export const TurnstileChallenge = forwardRef<TurnstileChallengeHandle, TurnstileChallengeProps>(
  function TurnstileChallenge({ siteKey, onToken, onExpire, onError }, ref) {
    const widget = useRef<TurnstileInstance | undefined>(undefined)
    useImperativeHandle(ref, () => ({ reset: () => widget.current?.reset() }), [])
    // Stable identity: the library re-runs its script-injection effect when
    // this object changes.
    const scriptOptions = useMemo(() => ({ onError }), [onError])

    return (
      <Turnstile
        ref={widget}
        siteKey={siteKey}
        options={WIDGET_OPTIONS}
        scriptOptions={scriptOptions}
        onSuccess={onToken}
        onExpire={onExpire}
        onTimeout={onExpire}
        onError={onError}
        onUnsupported={onError}
        className="flex justify-center"
      />
    )
  },
)
