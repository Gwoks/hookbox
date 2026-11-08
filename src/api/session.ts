/**
 * Owner session STATUS store (hookbox-a73.3, S2 identity).
 *
 * The credential itself is an anonymous recovery key held in an HttpOnly,
 * SameSite=Strict cookie that this script can never read. This module holds
 * only what the UI needs to route: whether the tab is signed in, and as which
 * anonymous owner. It stores no key, touches no localStorage and makes no
 * network calls — `bootstrap.ts` asks the server and reports back here.
 *
 * Cross-tab sync rides `BroadcastChannel('hookbox-session')`, and only for
 * explicit user actions (`{ broadcast: true }`): a sign-out in one tab signs
 * the others out; a sign-in as a DIFFERENT owner (restore / mint) makes the
 * others `invalidate()` so their gate re-asks the server which owner the
 * shared cookie now belongs to. Incoming messages are applied locally and
 * never re-broadcast, so tabs cannot ping-pong.
 */

export type SessionStatus = 'unknown' | 'signed-in' | 'signed-out'

export interface SessionState {
  readonly status: SessionStatus
  readonly ownerId: string | null
}

export interface BroadcastOptions {
  /** Tell the other tabs of this browser (an explicit user action). */
  broadcast?: boolean
}

/** The slice of `BroadcastChannel` the store uses (injectable for tests). */
export interface SessionChannel {
  postMessage(message: unknown): void
  onmessage: ((event: MessageEvent) => void) | null
}

export interface SessionStore {
  getSnapshot(): SessionState
  subscribe(listener: () => void): () => void
  markSignedIn(ownerId: string, options?: BroadcastOptions): void
  markSignedOut(options?: BroadcastOptions): void
  /** Forget what we know; the next gate render re-bootstraps. */
  invalidate(): void
}

export const SESSION_CHANNEL_NAME = 'hookbox-session'

type SessionMessage = { type: 'signed-out' } | { type: 'signed-in'; ownerId: string }

const UNKNOWN: SessionState = Object.freeze({ status: 'unknown', ownerId: null })
const SIGNED_OUT: SessionState = Object.freeze({ status: 'signed-out', ownerId: null })

function parseMessage(data: unknown): SessionMessage | null {
  if (typeof data !== 'object' || data === null) return null
  const message = data as { type?: unknown; ownerId?: unknown }
  if (message.type === 'signed-out') return { type: 'signed-out' }
  if (message.type === 'signed-in' && typeof message.ownerId === 'string' && message.ownerId) {
    return { type: 'signed-in', ownerId: message.ownerId }
  }
  return null
}

export function createSessionStore(channel: SessionChannel | null): SessionStore {
  let state: SessionState = UNKNOWN
  const listeners = new Set<() => void>()

  function setState(next: SessionState): void {
    if (next.status === state.status && next.ownerId === state.ownerId) return
    state = next
    for (const listener of [...listeners]) listener()
  }

  function post(message: SessionMessage, options?: BroadcastOptions): void {
    if (!options?.broadcast || !channel) return
    try {
      channel.postMessage(message)
    } catch {
      /* a closed channel only costs cross-tab sync; this tab is still right */
    }
  }

  const store: SessionStore = {
    getSnapshot: () => state,
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    markSignedIn(ownerId, options) {
      setState(Object.freeze({ status: 'signed-in', ownerId }))
      post({ type: 'signed-in', ownerId }, options)
    },
    markSignedOut(options) {
      setState(SIGNED_OUT)
      post({ type: 'signed-out' }, options)
    },
    invalidate() {
      setState(UNKNOWN)
    },
  }

  if (channel) {
    channel.onmessage = (event) => {
      const message = parseMessage(event.data)
      if (!message) return
      if (message.type === 'signed-out') store.markSignedOut()
      else if (message.ownerId !== state.ownerId) store.invalidate()
    }
  }

  return store
}

function openChannel(): SessionChannel | null {
  if (typeof BroadcastChannel === 'undefined') return null
  try {
    const channel = new BroadcastChannel(SESSION_CHANNEL_NAME)
    // Node (vitest) keeps the event loop alive for an open channel; browsers
    // have no `unref`, so this is a no-op there.
    ;(channel as BroadcastChannel & { unref?: () => void }).unref?.()
    return channel
  } catch {
    return null
  }
}

/** The app-wide store every owner screen shares. */
export const session: SessionStore = createSessionStore(openChannel())
