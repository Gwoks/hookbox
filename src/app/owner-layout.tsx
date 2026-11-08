/**
 * OwnerLayout — the `/d/:token` route element (inside RequireSession): one
 * EndpointProvider per token (keyed, so switching endpoints starts every
 * screen fresh), the AppShell chrome and the matched child screen.
 *
 * Until the endpoint loads it renders a whole-page frame instead of the
 * shell: loading, 404 "Endpoint not found", 410 "Endpoint deleted", or any
 * other failure (5xx, network) with Retry — previously a 5xx left the
 * dashboard on its loading skeleton forever. A 401 keeps the loading frame:
 * the API client is already bouncing the tab to the landing gate.
 */
import { RotateCw } from 'lucide-react'
import { Link, Outlet, useParams } from 'react-router-dom'
import { ApiError } from '@/api/errors'
import { t } from '@/lib/copy'
import { AppShell } from '@/components/owner/app-shell'
import { EndpointProvider, useEndpointState } from '@/components/owner/endpoint-context'
import { Button } from '@/components/ui/button'
import { LoadingBlock } from '@/components/ui/loading-block'
import { MessagePage } from './message-page'

export function OwnerLayout() {
  const { token = '' } = useParams()
  return (
    <EndpointProvider key={token} token={token}>
      <EndpointGate />
    </EndpointProvider>
  )
}

function EndpointGate() {
  const state = useEndpointState()
  if (state.status === 'ready') {
    return (
      <AppShell>
        <Outlet />
      </AppShell>
    )
  }
  const status = state.status === 'error' && state.error instanceof ApiError ? state.error.status : null
  if (state.status === 'loading' || status === 401) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas">
        <LoadingBlock label={t('dash.state.loading.aria')} className="w-64" />
      </div>
    )
  }
  if (status === 404 || status === 410) return <EndpointGonePage isDeleted={status === 410} />
  return (
    <MessagePage
      title={t('owner.error.title')}
      description={t('owner.error.body')}
      actions={
        <Button variant="outline" onClick={state.retry}>
          <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
          {t('common.retry')}
        </Button>
      }
    />
  )
}

function EndpointGonePage({ isDeleted }: { isDeleted: boolean }) {
  return (
    <MessagePage
      title={isDeleted ? t('dash.state.gone.title') : t('dash.state.notFound.title')}
      description={isDeleted ? t('dash.state.gone.body') : t('dash.state.notFound.body')}
      actions={
        <Button variant="tint" asChild>
          <Link to="/">{t('dash.state.backToStart')}</Link>
        </Button>
      }
    />
  )
}
