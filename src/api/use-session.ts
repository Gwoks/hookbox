/** React binding over the session store — re-renders on rotation / sign-out /
 * cross-tab storage events (AC-J7). useSyncExternalStore keeps it consistent. */
import { useSyncExternalStore } from 'react'
import { session, subscribe } from './session'

export interface SessionSnapshot {
  ownerSecret: string | null
  ownerId: string | null
  email: string | null
  hasSession: boolean
  storageAvailable: boolean
}

let cache: SessionSnapshot | null = null
function snapshot(): SessionSnapshot {
  const next: SessionSnapshot = {
    ownerSecret: session.getSecret(),
    ownerId: session.getOwnerId(),
    email: session.getEmail(),
    hasSession: session.hasSession(),
    storageAvailable: session.isStorageAvailable(),
  }
  // Stable identity unless a field changed (useSyncExternalStore requirement).
  if (
    cache &&
    cache.ownerSecret === next.ownerSecret &&
    cache.ownerId === next.ownerId &&
    cache.email === next.email &&
    cache.storageAvailable === next.storageAvailable
  ) {
    return cache
  }
  cache = next
  return next
}

export function useSession(): SessionSnapshot {
  return useSyncExternalStore(subscribe, snapshot, snapshot)
}
