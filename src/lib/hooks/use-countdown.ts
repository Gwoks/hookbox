/** useCountdown — whole seconds remaining from `seconds` down to 0, ticking
 * once a second; `null` while off. Restarts whenever `seconds` changes
 * (e.g. a fresh Retry-After). */
import { useEffect, useState } from 'react'

const TICK_MS = 1000

export function useCountdown(seconds: number | null): number | null {
  const [state, setState] = useState<{ from: number | null; left: number | null }>({ from: seconds, left: seconds })

  // Restart during render when the start value changes (no stale frame).
  const isStale = state.from !== seconds
  if (isStale) setState({ from: seconds, left: seconds })

  useEffect(() => {
    if (seconds == null) return
    const interval = setInterval(() => {
      setState((prev) => ({ ...prev, left: prev.left && prev.left > 0 ? prev.left - 1 : 0 }))
    }, TICK_MS)
    return () => clearInterval(interval)
  }, [seconds])

  return isStale ? seconds : state.left
}
