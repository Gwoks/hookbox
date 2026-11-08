/**
 * useDraft — an editable copy of a server object with dirty tracking and a
 * minimal PATCH diff (replaces settings' nine parallel useStates).
 *
 * - The draft is seeded from `source`; edits never mutate `source`.
 * - `resetKey`: when it changes (e.g. a different endpoint token), the draft
 *   is re-seeded from the current source. A refreshed `source` under the
 *   SAME key keeps the user's edits, and `diff` is taken against it.
 * - `diff` holds only the top-level keys whose values differ (nested
 *   arrays/objects compared by value), ready to send as a PATCH body.
 */
import { useCallback, useMemo, useState } from 'react'

function isEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false
  if (Array.isArray(a) !== Array.isArray(b)) return false
  const aKeys = Object.keys(a)
  const bKeys = Object.keys(b)
  if (aKeys.length !== bKeys.length) return false
  return aKeys.every((key) => isEqual((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]))
}

/** The top-level entries of `draft` that differ from `base`. Pure. */
export function shallowDiff<T extends object>(base: T, draft: T): Partial<T> {
  const keys = new Set([...Object.keys(base), ...Object.keys(draft)]) as Set<keyof T & string>
  return [...keys].reduce<Partial<T>>(
    (diff, key) => (isEqual(base[key], draft[key]) ? diff : { ...diff, [key]: draft[key] }),
    {},
  )
}

export interface Draft<T extends object> {
  draft: T
  set: <K extends keyof T>(key: K, value: T[K]) => void
  patch: (changes: Partial<T>) => void
  reset: () => void
  dirty: boolean
  diff: Partial<T>
}

export function useDraft<T extends object>(source: T, { resetKey }: { resetKey?: unknown } = {}): Draft<T> {
  const [state, setState] = useState({ key: resetKey, draft: source })

  // Re-seed during render when the key changes (React's "adjusting state
  // when a prop changes" pattern — no effect, no stale frame committed).
  const isStale = !Object.is(state.key, resetKey)
  if (isStale) setState({ key: resetKey, draft: source })
  const draft = isStale ? source : state.draft

  const patch = useCallback((changes: Partial<T>) => {
    setState((prev) => ({ ...prev, draft: { ...prev.draft, ...changes } }))
  }, [])
  const set = useCallback(
    <K extends keyof T>(key: K, value: T[K]) => patch({ [key]: value } as unknown as Partial<T>),
    [patch],
  )
  const reset = useCallback(() => setState({ key: resetKey, draft: source }), [resetKey, source])

  const diff = useMemo(() => shallowDiff(source, draft), [source, draft])
  return { draft, set, patch, reset, dirty: Object.keys(diff).length > 0, diff }
}
