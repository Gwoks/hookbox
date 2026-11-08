/**
 * Exponential backoff — pure, with the random source injected so tests (and
 * the two feed hooks' documented policies) are deterministic.
 *
 *   delay(n) = min(max, base · factor^min(n, maxExponent))
 *              + up to `jitter` · that, still capped at max
 *
 * The live feed uses {base 250, max 8000, jitter 0.3, maxExponent 5}; the
 * public viewer's poll uses {base 5000, max 60000} (no jitter).
 */
export interface BackoffPolicy {
  base: number
  max: number
  factor?: number
  /** Extra random delay as a fraction of the computed delay (0 = none). */
  jitter?: number
  maxExponent?: number
}

export function backoffDelay(
  attempt: number,
  { base, max, factor = 2, jitter = 0, maxExponent = Number.POSITIVE_INFINITY }: BackoffPolicy,
  random: () => number = Math.random,
): number {
  const delay = Math.min(max, base * factor ** Math.min(attempt, maxExponent))
  if (jitter === 0) return delay
  return Math.min(max, delay + random() * delay * jitter)
}
