/**
 * Owner-route session gate (hookbox-a73.3). Wraps every owner screen
 * (dashboard, rules, settings):
 *
 *   unknown     → bootstrap via useSessionGate; a loading frame meanwhile,
 *                 or a retry frame when the server could not be reached
 *   signed-out  → <Navigate> to / with the common.error.401 reason (a
 *                 render-time redirect, never navigate() during render)
 *   signed-in   → the screen; its data effects only ever run signed in
 *
 * A cross-tab sign-in as a different owner flips the status back to
 * unknown, which unmounts the screen until the gate has re-asked the server.
 */
import type { ReactNode } from 'react'
import { Navigate } from 'react-router-dom'
import { RotateCw } from 'lucide-react'
import { useSessionGate } from '@/api'
import { errorMessage } from '@/lib/api-errors'
import { t } from '@/lib/copy'
import { FullPageMessage } from '@/components/layout/full-page-message'
import { Button } from '@/components/ui/button'
import { LoadingBlock } from '@/components/ui/loading-block'

interface SessionPendingProps {
  error: unknown
  onRetry: () => void
}

/** The full-page frame shown while the session status is still unknown. */
export function SessionPending({ error, onRetry }: SessionPendingProps) {
  if (error) {
    return (
      <div className="flex min-h-screen flex-col bg-canvas">
        <FullPageMessage
          title={t('session.error.title')}
          description={errorMessage(error, t('session.error.body'))}
          actions={
            <Button variant="outline" onClick={onRetry}>
              <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
              {t('common.retry')}
            </Button>
          }
        />
      </div>
    )
  }
  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas">
      <LoadingBlock label={t('session.loading.aria')} className="w-64" />
    </div>
  )
}

export function RequireSession({ children }: { children: ReactNode }) {
  const gate = useSessionGate()
  if (gate.status === 'signed-out') {
    return <Navigate to="/" replace state={{ reason: t('common.error.401') }} />
  }
  if (gate.status === 'unknown') {
    return <SessionPending error={gate.bootstrapError} onRetry={gate.retry} />
  }
  return <>{children}</>
}
