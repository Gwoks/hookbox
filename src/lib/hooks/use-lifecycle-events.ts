/**
 * Page-lifecycle events shared by the live feed (useFeed) and the public
 * viewer's poll (useSharedFeed): tab visibility and browser online/offline.
 *
 * `subscribeLifecycle` is the plain DOM subscription (use it inside an
 * effect that already owns its other resources); `useLifecycleEvents` is the
 * hook form, always calling the latest handlers.
 */
import { useEffect, useRef } from 'react'

export interface LifecycleHandlers {
  /** Fired on every `visibilitychange`, with the new visibility. */
  onVisibilityChange?: (visible: boolean) => void
  onOnline?: () => void
  onOffline?: () => void
}

export const isDocumentVisible = (): boolean => document.visibilityState !== 'hidden'

export function subscribeLifecycle(handlers: LifecycleHandlers): () => void {
  const onVisibility = () => handlers.onVisibilityChange?.(isDocumentVisible())
  const onOnline = () => handlers.onOnline?.()
  const onOffline = () => handlers.onOffline?.()
  document.addEventListener('visibilitychange', onVisibility)
  window.addEventListener('online', onOnline)
  window.addEventListener('offline', onOffline)
  return () => {
    document.removeEventListener('visibilitychange', onVisibility)
    window.removeEventListener('online', onOnline)
    window.removeEventListener('offline', onOffline)
  }
}

export function useLifecycleEvents(handlers: LifecycleHandlers): void {
  const latest = useRef(handlers)
  useEffect(() => {
    latest.current = handlers
  })
  useEffect(
    () =>
      subscribeLifecycle({
        onVisibilityChange: (visible) => latest.current.onVisibilityChange?.(visible),
        onOnline: () => latest.current.onOnline?.(),
        onOffline: () => latest.current.onOffline?.(),
      }),
    [],
  )
}
