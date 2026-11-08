/**
 * EndpointProvider — the one fetch of the current endpoint (detail) and its
 * owner's sibling list (the switcher), shared by the owner chrome and every
 * owner screen. Replaces the three per-screen `getEndpoint`/`listEndpoints`
 * fetches and their silent `.catch(() => {})`s.
 *
 * - `useEndpointState()` is status-aware (OwnerLayout renders the loading,
 *   404/410 and 5xx frames from it).
 * - `useEndpoints()` is the ready value and throws outside a ready provider.
 * - `refresh()` re-fetches both silently (e.g. on `endpoint_updated`).
 * - `replace(e)` applies a server response locally (Save, import) and bumps
 *   `revision`, which forms key their drafts on.
 *
 * A failing sibling list never blocks the screen: the switcher just shows
 * the current endpoint.
 */
import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react'
import { api, type EndpointDetail, type EndpointSummary } from '@/api'
import { useResource } from '@/lib/hooks/use-resource'

export interface EndpointContextValue {
  token: string
  endpoint: EndpointDetail
  endpoints: EndpointSummary[]
  /** Bumped by every `replace()`; drafts re-seed when it changes. */
  revision: number
  refresh: () => Promise<void>
  replace: (endpoint: EndpointDetail) => void
}

export type EndpointState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown; retry: () => void }
  | { status: 'ready'; value: EndpointContextValue }

const EndpointStateContext = createContext<EndpointState | null>(null)

const EMPTY_LIST: EndpointSummary[] = []

function renameInList(list: EndpointSummary[] | undefined, endpoint: EndpointDetail): EndpointSummary[] {
  return (list ?? EMPTY_LIST).map((entry) =>
    entry.token === endpoint.token ? { ...entry, name: endpoint.name } : entry,
  )
}

export function EndpointProvider({ token, children }: { token: string; children: ReactNode }) {
  const detail = useResource((signal) => api.getEndpoint(token, { signal }), [token])
  const list = useResource((signal) => api.listEndpoints({ signal }), [token])
  const [revision, setRevision] = useState(0)
  const { reload: reloadDetail, setData: setDetail } = detail
  const { reload: reloadList, setData: setList } = list

  const refresh = useCallback(async () => {
    await Promise.all([reloadDetail({ silent: true }), reloadList({ silent: true })])
  }, [reloadDetail, reloadList])

  const replace = useCallback(
    (endpoint: EndpointDetail) => {
      setDetail(endpoint)
      setList((prev) => renameInList(prev, endpoint))
      setRevision((r) => r + 1)
    },
    [setDetail, setList],
  )

  const retry = useCallback(() => {
    void reloadDetail()
    void reloadList()
  }, [reloadDetail, reloadList])

  const endpoint = detail.data
  const endpoints = list.data ?? EMPTY_LIST
  const state = useMemo<EndpointState>(() => {
    if (detail.status === 'error') return { status: 'error', error: detail.error, retry }
    if (detail.status === 'loading' || !endpoint) return { status: 'loading' }
    return { status: 'ready', value: { token, endpoint, endpoints, revision, refresh, replace } }
  }, [detail.status, detail.error, endpoint, endpoints, revision, refresh, replace, retry, token])

  return <EndpointStateContext.Provider value={state}>{children}</EndpointStateContext.Provider>
}

export function useEndpointState(): EndpointState {
  const state = useContext(EndpointStateContext)
  if (!state) throw new Error('useEndpointState must be used inside an EndpointProvider')
  return state
}

export function useEndpoints(): EndpointContextValue {
  const state = useContext(EndpointStateContext)
  if (state?.status !== 'ready') throw new Error('useEndpoints must be used inside a ready EndpointProvider')
  return state.value
}
