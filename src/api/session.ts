/**
 * Owner-capability session store (PRD §5.1, AC-S14, AC-J7, AC-60).
 *
 * The capability ("your secret") is held in memory + localStorage, NEVER a
 * cookie — this closes CSRF without tokens (AC-S14): the cap is attached as an
 * `Authorization: Bearer <owner_secret>` header by the API client, and is the
 * `?cap=` query param for the WS/SSE feed (handled by the feed hook). A request
 * forged cross-site can't read localStorage, so it cannot attach the header.
 *
 * Rotation (AC-J7): re-submitting an email overwrites the stored secret. Tabs
 * subscribe to changes; the ACTIVE tab that performed the rotation keeps its
 * secret and survives. A STALE tab still holding the OLD secret bounces on its
 * next /api 401 and only retries once the STORED secret actually changes (a
 * `storage` event from another tab, or an explicit re-auth in this tab).
 */
const KEY_SECRET = 'hookbox-owner-secret'
const KEY_OWNER = 'hookbox-owner-id'
const KEY_EMAIL = 'hookbox-owner-email'

type Listener = () => void

interface SessionState {
  ownerSecret: string | null
  ownerId: string | null
  email: string | null
  /** True when localStorage is unavailable (private mode / blocked). The cap
   * then lives in memory only for this tab (landing.warn.storage). */
  storageAvailable: boolean
}

function readStorage(): { secret: string | null; owner: string | null; email: string | null; ok: boolean } {
  try {
    return {
      secret: localStorage.getItem(KEY_SECRET),
      owner: localStorage.getItem(KEY_OWNER),
      email: localStorage.getItem(KEY_EMAIL),
      ok: true,
    }
  } catch {
    return { secret: null, owner: null, email: null, ok: false }
  }
}

const init = readStorage()
const state: SessionState = {
  ownerSecret: init.secret,
  ownerId: init.owner,
  email: init.email,
  storageAvailable: init.ok,
}

const listeners = new Set<Listener>()
function emit() {
  for (const l of listeners) l()
}

/** Subscribe to session changes (rotation, sign-out, cross-tab storage event). */
export function subscribe(l: Listener): () => void {
  listeners.add(l)
  return () => listeners.delete(l)
}

export const session = {
  getSecret(): string | null {
    return state.ownerSecret
  },
  getOwnerId(): string | null {
    return state.ownerId
  },
  getEmail(): string | null {
    return state.email
  },
  isStorageAvailable(): boolean {
    return state.storageAvailable
  },
  hasSession(): boolean {
    return !!state.ownerSecret
  },

  /** Persist a freshly issued capability (rotate-overwrite, AC-J7). The active
   * tab that calls this keeps the new secret and continues. */
  set(secret: string, ownerId: string, email?: string) {
    state.ownerSecret = secret
    state.ownerId = ownerId
    if (email !== undefined) state.email = email
    try {
      localStorage.setItem(KEY_SECRET, secret)
      localStorage.setItem(KEY_OWNER, ownerId)
      if (email !== undefined) localStorage.setItem(KEY_EMAIL, email)
      state.storageAvailable = true
    } catch {
      state.storageAvailable = false
    }
    emit()
  },

  /** Clear the capability from this browser (sign-out). */
  clear() {
    state.ownerSecret = null
    state.ownerId = null
    state.email = null
    try {
      localStorage.removeItem(KEY_SECRET)
      localStorage.removeItem(KEY_OWNER)
      localStorage.removeItem(KEY_EMAIL)
    } catch {
      /* ignore */
    }
    emit()
  },
}

// Cross-tab sync: when another tab rotates/clears the secret, mirror it here so
// a stale tab can pick up the NEW secret and stop bouncing (AC-J7).
if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === KEY_SECRET) {
      state.ownerSecret = e.newValue
      try {
        state.ownerId = localStorage.getItem(KEY_OWNER)
        state.email = localStorage.getItem(KEY_EMAIL)
      } catch {
        /* ignore */
      }
      emit()
    }
  })
}
