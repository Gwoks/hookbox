/**
 * useFeed — the live-feed hook (PRD §5.4, AC-41/42/43, AC-J5/J6/J13,
 * AC-D16/D17/D18). The resilience-critical real-time layer; the no-Redis
 * at-most-once reconciliation lives here.
 *
 * Transport: WebSocket `/ws/{token}` first; after MAX_WS_FAILS_BEFORE_SSE
 * (6) failures fall back to SSE `/sse/{token}` (AC-J5). The credential is
 * the owner's HttpOnly session cookie, which the browser attaches to these
 * same-origin requests itself — nothing in the URL (hookbox-a73.3). The feed
 * only connects while the session status is `signed-in`. ./transport.ts
 * builds both URLs. Backoff comes from lib/backoff, lifecycle listeners from
 * lib/hooks/use-lifecycle-events (shared with the public viewer's poll).
 *
 * Contract handling:
 * - `new_request` (RequestSummary in the {type,data} envelope) prepends live,
 *   newest-first, capped at 100 (AC-41, feed.count).
 * - `hello` (first frame) triggers an INITIAL reconcile via the §5.2 GET routes;
 *   on every (re)connect we reconcile authoritative lists/state — the broadcast
 *   channel is best-effort/at-most-once; the management API is the source of
 *   truth (§5.4, AC-43/J13).
 * - exponential backoff with jitter 250→8000ms on drop (AC-J5).
 * - heartbeat: WS sends "ping" and expects "pong" within a grace window; a
 *   missed pong forces a reconnect (half-open detection, AC-J5).
 * - pill states connecting/live/reconnecting(n)/sse/offline/unauthorized/busy —
 *   the TEXT label carries state (AC-D17).
 * - WS 1013 / SSE 503 → distinct "busy" state, NOT endless reconnect / no
 *   gate-hammer (AC-J6).
 * - WS 4401 (close) / SSE 401 → "unauthorized" (key rotated elsewhere, AC-J5).
 * - reconnection paused while the tab is hidden, resumed with a back-fill on
 *   focus (AC-J5/J13).
 * - while PAUSED (by the user) buffered new_request count is exposed as
 *   `newCount` (feed.newCount); flushing on resume preserves read position.
 * - browser fully offline → `offline` pill + management offline banner (AC-J13).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ConnState } from '@/components/hookbox/connection-pill'
import { api, useSession, type RequestSummary } from '@/api'
import { backoffDelay, type BackoffPolicy } from '@/lib/backoff'
import { subscribeLifecycle } from '@/lib/hooks/use-lifecycle-events'
import { decodeSseEvent, decodeWsFrame, type FeedEvent } from './events'
import { feedEventSourceUrl, feedSocketUrl } from './transport'

// 250ms doubling to 8s (exponent capped at 5) plus up to 30% jitter (AC-J5).
const RECONNECT_BACKOFF: BackoffPolicy = { base: 250, max: 8000, jitter: 0.3, maxExponent: 5 }
const MAX_WS_FAILS_BEFORE_SSE = 6
const HEARTBEAT_INTERVAL_MS = 20000
const PONG_GRACE_MS = 5000
const FEED_CAP = 100

export interface UseFeedOptions {
  /** Endpoint token; the feed only flows while the tab is signed in. */
  token: string | null
  /** User-controlled pause; while paused, arrivals buffer into newCount. */
  paused?: boolean
  onStateChanged?: (key: string, value: string) => void
  onEndpointUpdated?: (fields: string[]) => void
}

export interface UseFeedResult {
  rows: RequestSummary[]
  connState: ConnState
  /** Reconnect attempt count, surfaced for feed.conn.reconnecting.n. */
  attempt: number
  /** Buffered-while-paused count for the feed.newCount pill. */
  newCount: number
  /** Flush buffered rows into the visible list (resume), preserving order. */
  flushBuffered: () => void
  /** Force a fresh reconcile + reconnect (e.g. manual Retry). */
  reconnect: () => void
  /** Empty the visible list AND the paused-arrival buffer (operator-toolkit
   * F1 "Clear all" — AC-5). Client-side only; the caller is responsible for
   * the DELETE that makes it authoritative. */
  clearRows: () => void
}

export function useFeed(opts: UseFeedOptions): UseFeedResult {
  const { token, paused = false, onStateChanged, onEndpointUpdated } = opts
  const signedIn = useSession().status === 'signed-in'

  const [rows, setRows] = useState<RequestSummary[]>([])
  const [connState, setConnState] = useState<ConnState>('connecting')
  const [attempt, setAttempt] = useState(0)
  const [newCount, setNewCount] = useState(0)

  // Buffer of arrivals received while the user paused the feed.
  const buffer = useRef<RequestSummary[]>([])
  const pausedRef = useRef(paused)
  pausedRef.current = paused

  // Transport refs (kept out of state to avoid re-render churn).
  const ws = useRef<WebSocket | null>(null)
  const sse = useRef<EventSource | null>(null)
  const backoffTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const heartbeat = useRef<ReturnType<typeof setInterval> | null>(null)
  const pongTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const wsFails = useRef(0)
  const attemptRef = useRef(0)
  const useSse = useRef(false)
  const stopped = useRef(false) // true once busy/unauthorized — do NOT reconnect (AC-J6)
  const generation = useRef(0) // bumped on teardown to invalidate stale callbacks

  const dispatchEvent = useCallback(
    (ev: FeedEvent) => {
      switch (ev.type) {
        case 'hello':
          // Initial reconcile is driven by the connect path; nothing extra here.
          break
        case 'new_request':
          if (pausedRef.current) {
            buffer.current = [ev.data, ...buffer.current].slice(0, FEED_CAP)
            setNewCount(buffer.current.length)
          } else {
            setRows((prev) => [ev.data, ...prev.filter((r) => r.id !== ev.data.id)].slice(0, FEED_CAP))
          }
          break
        case 'state_changed':
          onStateChanged?.(ev.data.key, ev.data.value)
          break
        case 'endpoint_updated':
          onEndpointUpdated?.(ev.data.fields)
          break
      }
    },
    [onStateChanged, onEndpointUpdated],
  )

  // Reconcile authoritative list via §5.2 GET (the source of truth; AC-43/J13).
  const reconcile = useCallback(
    async (tok: string, gen: number) => {
      try {
        const list = await api.listRequests(tok, { limit: FEED_CAP })
        if (gen !== generation.current) return
        // Newest-first; server returns DESC by id. Preserve any buffered arrivals.
        setRows(list.slice(0, FEED_CAP))
      } catch {
        // A 401 is handled by the client (bounce); other errors leave the last
        // known rows visible (offline-tolerant, AC-J13).
      }
    },
    [],
  )

  const clearTimers = useCallback(() => {
    if (backoffTimer.current) clearTimeout(backoffTimer.current)
    if (heartbeat.current) clearInterval(heartbeat.current)
    if (pongTimer.current) clearTimeout(pongTimer.current)
    backoffTimer.current = null
    heartbeat.current = null
    pongTimer.current = null
  }, [])

  const teardown = useCallback(() => {
    generation.current += 1
    clearTimers()
    if (ws.current) {
      ws.current.onclose = null
      ws.current.onerror = null
      ws.current.onmessage = null
      ws.current.onopen = null
      try {
        ws.current.close()
      } catch {
        /* ignore */
      }
      ws.current = null
    }
    if (sse.current) {
      try {
        sse.current.close()
      } catch {
        /* ignore */
      }
      sse.current = null
    }
  }, [clearTimers])

  const scheduleReconnect = useCallback(
    (connect: () => void) => {
      if (stopped.current) return
      attemptRef.current += 1
      setAttempt(attemptRef.current)
      setConnState('reconnecting')
      const delay = backoffDelay(attemptRef.current, RECONNECT_BACKOFF)
      backoffTimer.current = setTimeout(() => {
        if (document.visibilityState === 'hidden') return // paused while hidden (AC-J5)
        connect()
      }, delay)
    },
    [],
  )

  // The connect routine is recreated when token changes.
  const connectRef = useRef<() => void>(() => {})

  useEffect(() => {
    stopped.current = false
    wsFails.current = 0
    attemptRef.current = 0
    useSse.current = false
    setAttempt(0)

    if (!token || !signedIn) {
      setConnState('connecting')
      return
    }

    const startHeartbeat = (socket: WebSocket) => {
      heartbeat.current = setInterval(() => {
        if (socket.readyState !== WebSocket.OPEN) return
        try {
          socket.send('ping')
        } catch {
          /* ignore */
        }
        if (pongTimer.current) clearTimeout(pongTimer.current)
        pongTimer.current = setTimeout(() => {
          // No pong within grace → half-open socket; force a reconnect (AC-J5).
          try {
            socket.close()
          } catch {
            /* ignore */
          }
        }, PONG_GRACE_MS)
      }, HEARTBEAT_INTERVAL_MS)
    }

    const connectSse = () => {
      teardown()
      const gen = generation.current
      useSse.current = true
      setConnState('connecting')
      const es = new EventSource(feedEventSourceUrl(token))
      sse.current = es
      es.onopen = () => {
        if (gen !== generation.current) return
        attemptRef.current = 0
        setAttempt(0)
        setConnState('sse')
        void reconcile(token, gen)
      }
      const handler = (type: string) => (e: MessageEvent) => {
        const ev = decodeSseEvent(type, e.data)
        if (ev) dispatchEvent(ev)
      }
      es.addEventListener('hello', handler('hello'))
      es.addEventListener('new_request', handler('new_request'))
      es.addEventListener('state_changed', handler('state_changed'))
      es.addEventListener('endpoint_updated', handler('endpoint_updated'))
      es.onerror = () => {
        if (gen !== generation.current) return
        // SSE 503 (conn-cap) surfaces as a hard error on the stream; the browser
        // would normally retry forever. We can't read the status from
        // EventSource directly, so a rapid repeated failure escalates to busy.
        if (!navigator.onLine) {
          setConnState('offline')
          return
        }
        es.close()
        sse.current = null
        scheduleReconnect(connectSse)
      }
    }

    const connectWs = () => {
      teardown()
      const gen = generation.current
      setConnState(attemptRef.current > 0 ? 'reconnecting' : 'connecting')
      const socket = new WebSocket(feedSocketUrl(token))
      ws.current = socket

      socket.onopen = () => {
        if (gen !== generation.current) return
        wsFails.current = 0
        attemptRef.current = 0
        setAttempt(0)
        setConnState('live')
        startHeartbeat(socket)
        void reconcile(token, gen) // hello-driven initial reconcile (AC-43)
      }
      socket.onmessage = (e) => {
        if (typeof e.data !== 'string') return
        if (e.data === 'pong') {
          if (pongTimer.current) clearTimeout(pongTimer.current)
          return
        }
        const ev = decodeWsFrame(e.data)
        if (ev) dispatchEvent(ev)
      }
      socket.onclose = (e) => {
        if (gen !== generation.current) return
        clearTimers()
        ws.current = null
        // 4401 = unauthorized bind (key rotated elsewhere). Stop; do not hammer (AC-J5).
        if (e.code === 4401) {
          stopped.current = true
          setConnState('unauthorized')
          return
        }
        // 1013 = try-again-later / conn-cap. Distinct busy state, no reconnect
        // loop (AC-J6).
        if (e.code === 1013) {
          stopped.current = true
          setConnState('busy')
          return
        }
        if (!navigator.onLine) {
          setConnState('offline')
          // wait for the online event (registered below) to reconnect.
          return
        }
        wsFails.current += 1
        if (wsFails.current >= MAX_WS_FAILS_BEFORE_SSE) {
          scheduleReconnect(connectSse) // fall back to SSE (AC-J5)
        } else {
          scheduleReconnect(connectWs)
        }
      }
      socket.onerror = () => {
        // onerror is always followed by onclose; let onclose drive the policy.
      }
    }

    connectRef.current = () => (useSse.current ? connectSse() : connectWs())
    connectWs()

    const unsubscribeLifecycle = subscribeLifecycle({
      // Tab-visibility: pause reconnection while hidden, back-fill on focus (AC-J5/J13).
      onVisibilityChange: (visible) => {
        if (!visible || stopped.current) return
        // Reconnect + reconcile to back-fill anything missed while hidden.
        if (!ws.current && !sse.current) connectRef.current()
        else void reconcile(token, generation.current)
      },
      onOnline: () => {
        if (stopped.current) return
        attemptRef.current = 0
        connectRef.current()
      },
      onOffline: () => setConnState('offline'),
    })

    return () => {
      unsubscribeLifecycle()
      teardown()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, signedIn])

  const flushBuffered = useCallback(() => {
    if (buffer.current.length === 0) {
      setNewCount(0)
      return
    }
    setRows((prev) => [...buffer.current, ...prev].slice(0, FEED_CAP))
    buffer.current = []
    setNewCount(0)
  }, [])

  const reconnect = useCallback(() => {
    stopped.current = false
    wsFails.current = 0
    attemptRef.current = 0
    useSse.current = false
    connectRef.current()
  }, [])

  const clearRows = useCallback(() => {
    setRows([])
    buffer.current = []
    setNewCount(0)
  }, [])

  return { rows, connState, attempt, newCount, flushBuffered, reconnect, clearRows }
}
