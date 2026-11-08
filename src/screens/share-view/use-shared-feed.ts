/** useSharedFeed — the public /s/:code viewer's polling hook (operator-toolkit
 * F4, AC-45/AC-105/AC-S8). Deliberately NOT the owner's WS/SSE `useFeed`: the
 * viewer opens no WebSocket and no EventSource, ever — it polls
 * `GET /api/share/{code}/requests` every 5s while the document is visible,
 * with the frozen lifecycle:
 *
 *   (a) a LIST 404 is TERMINAL — stop polling permanently, forever.
 *   (b) 429 pauses for >= Retry-After, then retries once.
 *   (c) 5xx / network / contract_mismatch back off exponentially from 5s,
 *       ceiling 60s, keeping the last-known rows visible.
 *   (d) no two polls ever run concurrently.
 *   (e) resumes on `visibilitychange -> visible` and on `online`.
 *   (f) every in-flight fetch is aborted on unmount.
 *   (g) (NOT this hook — a DETAIL 404 is handled per-row, never terminal.)
 *
 * Backoff comes from lib/backoff and the visibility/online listeners from
 * lib/hooks/use-lifecycle-events — the same helpers the owner feed uses.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { api, ApiError, type PublicRequestSummary, type PublicShareFeed } from '@/api/public-client'
import { backoffDelay, type BackoffPolicy } from '@/lib/backoff'
import { useDocumentVisible } from '@/lib/hooks/use-document-visible'
import { isDocumentVisible, subscribeLifecycle } from '@/lib/hooks/use-lifecycle-events'
import { useOnline } from '@/lib/hooks/use-online'

const POLL_INTERVAL_MS = 5000
const SHARE_FEED_LIMIT = 100
// (c) doubles from the poll interval: 10s, 20s, 40s, then the 60s ceiling.
const ERROR_BACKOFF: BackoffPolicy = { base: POLL_INTERVAL_MS, max: 60_000 }

export type SharedFeedStatus = 'loading' | 'unavailable' | 'ready' | 'error' | 'rateLimited'

export interface SharedFeedData {
  endpoint: PublicShareFeed['endpoint']
  requests: PublicRequestSummary[]
  updatedAt: number
}

export interface UseSharedFeedResult {
  /** The CURRENT transient status — `data` may still be populated (and kept
   * on screen at full opacity) even when status is 'error'/'rateLimited'. */
  status: SharedFeedStatus
  data: SharedFeedData | null
  /** Only meaningful while status === 'rateLimited'. */
  retryInSeconds: number | null
  /** True once the tab is hidden or the browser is offline — polling is
   * suspended, not stopped; it resumes on its own. */
  paused: boolean
  isHidden: boolean
  isOffline: boolean
  refresh: () => void
}

export function useSharedFeed(code: string): UseSharedFeedResult {
  const [status, setStatus] = useState<SharedFeedStatus>('loading')
  const [data, setData] = useState<SharedFeedData | null>(null)
  const [retryInSeconds, setRetryInSeconds] = useState<number | null>(null)
  const isHidden = !useDocumentVisible()
  const isOffline = !useOnline()

  const stoppedRef = useRef(false) // (a) terminal — never poll again
  const inFlightRef = useRef(false) // (d)
  const failuresRef = useRef(0) // consecutive (c) failures
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const controllerRef = useRef<AbortController | null>(null)
  const generationRef = useRef(0) // bumped on unmount to invalidate late callbacks

  const poll = useCallback(async () => {
    if (stoppedRef.current || inFlightRef.current || !isDocumentVisible() || !navigator.onLine) {
      return
    }
    inFlightRef.current = true
    const gen = generationRef.current
    const controller = new AbortController()
    controllerRef.current = controller

    const scheduleNext = (ms: number) => {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => void poll(), ms)
    }

    try {
      const feed = await api.getSharedRequests(code, {
        limit: SHARE_FEED_LIMIT,
        signal: controller.signal,
      })
      if (gen !== generationRef.current) return
      failuresRef.current = 0
      setRetryInSeconds(null)
      setData({ endpoint: feed.endpoint, requests: feed.requests, updatedAt: Date.now() })
      setStatus('ready')
      scheduleNext(POLL_INTERVAL_MS)
    } catch (err) {
      if (gen !== generationRef.current || controller.signal.aborted) return
      if (err instanceof ApiError && err.status === 404) {
        stoppedRef.current = true // (a) terminal, no further scheduling
        setStatus('unavailable')
        return
      }
      if (err instanceof ApiError && err.status === 429) {
        const seconds = err.retryAfter ?? 5
        setRetryInSeconds(seconds)
        setStatus('rateLimited')
        scheduleNext(seconds * 1000) // (b) exactly one retry after Retry-After
        return
      }
      // (c) 5xx / network / contract_mismatch — exponential backoff, ceiling 60s.
      failuresRef.current += 1
      setStatus('error')
      scheduleNext(backoffDelay(failuresRef.current, ERROR_BACKOFF))
    } finally {
      inFlightRef.current = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [code])

  const refresh = useCallback(() => {
    if (stoppedRef.current) return
    if (timerRef.current) clearTimeout(timerRef.current)
    failuresRef.current = 0
    void poll()
  }, [poll])

  useEffect(() => {
    void poll()

    // (e) resume on visible / online; isHidden/isOffline above re-render.
    const unsubscribeLifecycle = subscribeLifecycle({
      onVisibilityChange: (visible) => {
        if (visible && navigator.onLine) refresh()
      },
      onOnline: refresh,
    })

    return () => {
      generationRef.current += 1 // (f) invalidate any in-flight callback
      controllerRef.current?.abort()
      if (timerRef.current) clearTimeout(timerRef.current)
      unsubscribeLifecycle()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [code])

  return { status, data, retryInSeconds, paused: isHidden || isOffline, isHidden, isOffline, refresh }
}
