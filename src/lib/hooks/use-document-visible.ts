/** useDocumentVisible — true while the tab is visible (Page Visibility API),
 * via useSyncExternalStore so it is tear-free under concurrent rendering. */
import { useSyncExternalStore } from 'react'
import { isDocumentVisible } from './use-lifecycle-events'

function subscribe(onChange: () => void): () => void {
  document.addEventListener('visibilitychange', onChange)
  return () => document.removeEventListener('visibilitychange', onChange)
}

export function useDocumentVisible(): boolean {
  return useSyncExternalStore(subscribe, isDocumentVisible, () => true)
}
