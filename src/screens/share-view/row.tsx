/** SharedRequestRow (design.md §3.7, operator-toolkit F4 viewer,
 * AC-65/106/110). A real `<button aria-expanded aria-controls>` disclosure
 * whose panel is a `role="region"` — NOT the owner feed's option/listbox
 * semantics, which are wrong for a page whose content updates underneath
 * the reader. The cells are the shared RequestCells (meta wraps below
 * `sm`), the detail the shared RequestDetailTabs. No per-row deep link
 * (AC-112): opening a row never changes the URL. */
import { ChevronRight } from 'lucide-react'
import { api, ApiError, type PublicRequestSummary } from '@/api/public-client'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { useResource } from '@/lib/hooks/use-resource'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { REQUEST_GRID_DISCLOSURE, RequestCells } from '@/components/hookbox/request-cells'
import { RequestDetailTabs } from '@/components/hookbox/request-detail-tabs'
import { RetryAlert } from '@/components/hookbox/retry-alert'
import { LoadingBlock } from '@/components/ui/loading-block'

export interface SharedRequestRowProps {
  code: string
  row: PublicRequestSummary
  isOpen: boolean
  onToggle: () => void
}

export function SharedRequestRow({ code, row, isOpen, onToggle }: SharedRequestRowProps) {
  const rowId = `share-row-${row.id}`
  const panelId = `share-panel-${row.id}`
  const label = isOpen
    ? t('viewer.row.collapse.aria', { method: row.method, path: row.path })
    : t('viewer.row.expand.aria', { method: row.method, path: row.path, status: row.status_code })

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        id={rowId}
        aria-expanded={isOpen}
        aria-controls={panelId}
        aria-label={label}
        onClick={onToggle}
        className={cn(
          'grid min-h-11 w-full items-center gap-x-2 gap-y-1 px-4 py-2.5 text-left text-body-sm',
          REQUEST_GRID_DISCLOSURE,
          'transition-colors duration-base hover:bg-surface-hover focus-visible:bg-surface-hover',
          isOpen && 'bg-surface-hover',
        )}
      >
        <ChevronRight
          className={cn(
            'h-3.5 w-3.5 shrink-0 text-text-tertiary transition-transform duration-fast ease-standard',
            isOpen && 'rotate-90',
          )}
          aria-hidden="true"
        />
        <RequestCells request={row} wrapMeta />
      </button>
      {isOpen && (
        <div id={panelId} role="region" aria-labelledby={rowId} className="border-t border-border bg-canvas">
          <SharedRequestDetail code={code} id={row.id} />
        </div>
      )}
    </div>
  )
}

function SharedRequestDetail({ code, id }: { code: string; id: number }) {
  const detail = useResource((signal) => api.getSharedRequest(code, id, { signal }), [code, id])

  if (detail.status === 'loading') {
    return <LoadingBlock label={t('viewer.detail.loading.aria')} lines={6} className="p-4" />
  }
  // AC-106 / AC-S8(g): a detail 404 means "this request rolled off" — never
  // terminal, and never the owner inspector's "pending" (this list came
  // from the database, not a live broadcast).
  if (detail.error instanceof ApiError && detail.error.status === 404) {
    return (
      <div className="p-4">
        <InlineAlert variant="info" role="status" title={t('viewer.detail.gone.title')}>
          {t('viewer.detail.gone.body')}
        </InlineAlert>
      </div>
    )
  }
  if (detail.status === 'error' || !detail.data) {
    return (
      <div className="p-4">
        <RetryAlert title={t('viewer.detail.error.title')} onRetry={() => void detail.reload()}>
          {t('viewer.detail.error.body')}
        </RetryAlert>
      </div>
    )
  }
  return <RequestDetailTabs detail={detail.data} panelClassName="p-4" />
}
