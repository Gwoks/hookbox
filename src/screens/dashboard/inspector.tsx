/**
 * Inspector — the deep detail for the selected feed row (PRD §3, AC-44/45,
 * copy.md §5.5): GET /api/requests/{id} through useResource, then
 *
 *   empty        no row selected
 *   loading      fetch in flight
 *   pending      a JUST-streamed row whose detail 404s because the trace is
 *                not persisted yet (`isLive`) → info + Retry, NOT a hard 404
 *                (OQ-2 / AC-45)
 *   unauthorized 401 (the client is already bouncing the tab)
 *   error        anything else → RetryAlert
 *   ready        RequestSubject + the shared RequestDetailTabs, with the
 *                owner-only State & Tracing tab appended
 */
import { api, ApiError, type RequestDetail } from '@/api'
import { t } from '@/lib/copy'
import { useResource } from '@/lib/hooks/use-resource'
import { RequestDetailTabs } from '@/components/hookbox/request-detail-tabs'
import { RequestSubject } from '@/components/hookbox/request-subject'
import { RetryAlert } from '@/components/hookbox/retry-alert'
import { EmptyState } from '@/components/ui/empty-state'
import { LoadingBlock } from '@/components/ui/loading-block'
import { TracePanel } from './trace-panel'

export function Inspector({ id, isLive }: { id: number | null; isLive: boolean }) {
  if (id === null) {
    return (
      <EmptyState title={t('insp.empty.title')} description={t('insp.empty.body')} className="h-full justify-center" />
    )
  }
  return <InspectorDetail key={id} id={id} isLive={isLive} />
}

function InspectorDetail({ id, isLive }: { id: number; isLive: boolean }) {
  const detail = useResource((signal) => api.getRequest(id, { signal }), [id])
  if (detail.status === 'loading') return <LoadingBlock label={t('insp.loading.aria')} lines={6} className="p-4" />
  if (detail.status === 'error' || !detail.data) {
    return (
      <div className="p-4">
        <InspectorError error={detail.error} isLive={isLive} onRetry={() => void detail.reload()} />
      </div>
    )
  }
  return <Ready detail={detail.data} />
}

function InspectorError({ error, isLive, onRetry }: { error: unknown; isLive: boolean; onRetry: () => void }) {
  const status = error instanceof ApiError ? error.status : null
  if (status === 404 && isLive) {
    return (
      <RetryAlert
        variant="info"
        role="status"
        title={t('insp.pending.title')}
        retryLabel={t('insp.pending.retry')}
        onRetry={onRetry}
      >
        {t('insp.pending.body')}
      </RetryAlert>
    )
  }
  if (status === 401) {
    return <RetryAlert title={t('insp.unauthorized.title')}>{t('insp.unauthorized.body')}</RetryAlert>
  }
  return (
    <RetryAlert title={t('insp.error.title')} retryLabel={t('insp.error.retry')} onRetry={onRetry}>
      {t('insp.error.body')}
    </RetryAlert>
  )
}

function Ready({ detail }: { detail: RequestDetail }) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <RequestSubject request={detail} className="border-b border-border px-4 py-3" />
      <RequestDetailTabs
        detail={detail}
        extraTabs={[{ value: 'trace', label: t('insp.tab.trace'), content: <TracePanel detail={detail} /> }]}
        className="flex min-h-0 flex-1 flex-col"
        panelClassName="min-h-0 flex-1 overflow-auto p-4"
      />
    </div>
  )
}
