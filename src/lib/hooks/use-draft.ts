/**
 * useDraft — an editable copy of a server object with dirty tracking and a
 * minimal PATCH diff (replaces settings' nine parallel useStates).
 *
 * - The draft is seeded from `source`; edits never mutate `source`.
 * - `resetKey`: when it changes (e.g. a different endpoint token), the draft
 *   is re-seeded from the current source.
 * - A refreshed `source` under the SAME key (compared by value) rebases: the
 *   user's own edits are kept on top of the new source, and every untouched
 *   field follows the server — so a clean draft stays clean and Save never
 *   sends a stale value back.
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

interface DraftState<T> {
  key: unknown
  /** The source the current edits were made against. */
  base: T
  draft: T
}

/** The state for this render: re-seeded on a key change, rebased on a refresh. */
function syncDraft<T extends object>(state: DraftState<T>, source: T, resetKey: unknown): DraftState<T> {
  if (!Object.is(state.key, resetKey)) return { key: resetKey, base: source, draft: source }
  if (isEqual(state.base, source)) return state
  const edits = shallowDiff(state.base, state.draft)
  return { key: resetKey, base: source, draft: { ...source, ...edits } }
}

export function useDraft<T extends object>(source: T, { resetKey }: { resetKey?: unknown } = {}): Draft<T> {
  const [state, setState] = useState<DraftState<T>>({ key: resetKey, base: source, draft: source })

  // Adjust during render (React's "adjusting state when a prop changes"
  // pattern — no effect, no stale frame committed). Converges in one pass:
  // the next render sees the synced key/base and returns the same state.
  const synced = syncDraft(state, source, resetKey)
  if (synced !== state) setState(synced)
  const draft = synced.draft

  const patch = useCallback((changes: Partial<T>) => {
    setState((prev) => ({ ...prev, draft: { ...prev.draft, ...changes } }))
  }, [])
  const set = useCallback(
    <K extends keyof T>(key: K, value: T[K]) => patch({ [key]: value } as unknown as Partial<T>),
    [patch],
  )
  const reset = useCallback(() => setState({ key: resetKey, base: source, draft: source }), [resetKey, source])

  const diff = useMemo(() => shallowDiff(source, draft), [source, draft])
  return { draft, set, patch, reset, dirty: Object.keys(diff).length > 0, diff }
}
