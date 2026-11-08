/**
 * useResource — the one loading/error/ready state machine for a fetched
 * value (replaces the hand-rolled copies across screens).
 *
 * - Runs `fetcher(signal)` on mount and whenever `deps` change; a deps change
 *   clears the old data (it belonged to another key) and aborts the old call.
 * - Every call gets an AbortController and a generation number; a response
 *   from an aborted or superseded call is ignored, and nothing updates after
 *   unmount.
 * - `reload()` → back to `loading`; `reload({ silent: true })` keeps the
 *   current status and data on screen and flags `refreshing` instead. A
 *   failed SILENT reload over ready data keeps that data (status stays
 *   `ready`) and only exposes `error`.
 * - `setData` applies a local update (e.g. after a mutation).
 */
import { useCallback, useEffect, useRef, useState, type DependencyList } from 'react'

export type ResourceStatus = 'loading' | 'error' | 'ready'

export interface ResourceState<T> {
  status: ResourceStatus
  data: T | undefined
  error: unknown
  refreshing: boolean
  reload: (opts?: { silent?: boolean }) => Promise<void>
  setData: (next: T | ((prev: T | undefined) => T)) => void
}

interface Core<T> {
  status: ResourceStatus
  data: T | undefined
  error: unknown
}

type RunMode = 'initial' | 'reload' | 'silent'

const INITIAL: Core<never> = { status: 'loading', data: undefined, error: null }

export function useResource<T>(
  fetcher: (signal: AbortSignal) => Promise<T>,
  deps: DependencyList,
): ResourceState<T> {
  const [core, setCore] = useState<Core<T>>(INITIAL)
  const [refreshing, setRefreshing] = useState(false)
  const fetcherRef = useRef(fetcher)
  const controllerRef = useRef<AbortController | null>(null)
  const generationRef = useRef(0)

  // Keep the latest fetcher for reload() without making it a dependency.
  useEffect(() => {
    fetcherRef.current = fetcher
  })

  const run = useCallback(async (mode: RunMode) => {
    controllerRef.current?.abort()
    const controller = new AbortController()
    controllerRef.current = controller
    const generation = ++generationRef.current
    const isCurrent = () => generation === generationRef.current && !controller.signal.aborted

    if (mode === 'silent') setRefreshing(true)
    else if (mode === 'reload') setCore((prev) => ({ ...prev, status: 'loading', error: null }))
    else setCore((prev) => (prev === INITIAL ? prev : INITIAL))

    try {
      const data = await fetcherRef.current(controller.signal)
      if (isCurrent()) setCore({ status: 'ready', data, error: null })
    } catch (error) {
      if (!isCurrent()) return
      setCore((prev) =>
        mode === 'silent' && prev.status === 'ready' ? { ...prev, error } : { ...prev, status: 'error', error },
      )
    } finally {
      if (generation === generationRef.current) setRefreshing(false)
    }
  }, [])

  useEffect(() => {
    void run('initial')
    return () => {
      generationRef.current += 1
      controllerRef.current?.abort()
    }
    // `deps` is the caller's key list, exactly like useEffect's.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)

  const reload = useCallback((opts?: { silent?: boolean }) => run(opts?.silent ? 'silent' : 'reload'), [run])

  const setData = useCallback((next: T | ((prev: T | undefined) => T)) => {
    // A locally applied value (e.g. a save response) is newer than any reload
    // still in flight: invalidate it so a late, stale response can't win.
    generationRef.current += 1
    controllerRef.current?.abort()
    setRefreshing(false)
    setCore((prev) => ({
      ...prev,
      data: typeof next === 'function' ? (next as (p: T | undefined) => T)(prev.data) : next,
    }))
  }, [])

  return { ...core, refreshing, reload, setData }
}
