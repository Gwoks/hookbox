/** SharedRequestRow (design.md §3.7, operator-toolkit F4 viewer, AC-65/106/110).
 * A real `<button type="button" aria-expanded aria-controls>` disclosure
 * inside a `role="region"` well — NOT FeedRow's `role="option"`/listbox
 * semantics, which are wrong for a page whose content updates underneath the
 * reader. No per-row deep link (AC-112): selecting a row never changes the URL.
 */
import { useCallback, useEffect, useState } from 'react'
import { ChevronRight, RotateCw } from 'lucide-react'
import { api, ApiError, type PublicRequestDetail, type PublicRequestSummary } from '@/api'
import { t } from '@/lib/copy'
import { cn } from '@/lib/cn'
import { relTime } from '@/lib/time'
import { MethodBadge } from '@/components/hookbox/method-badge'
import { StatusCode } from '@/components/hookbox/status-code'
import { ServedByChip } from '@/components/hookbox/served-by-chip'
import { KeyValueRows } from '@/components/hookbox/key-value-rows'
import { JsonTree } from '@/components/hookbox/json-tree'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { SkeletonLines } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

export function SharedRequestRow({
  code,
  row,
  isOpen,
  onToggle,
}: {
  code: string
  row: PublicRequestSummary
  isOpen: boolean
  onToggle: () => void
}) {
  const rowId = `share-row-${row.id}`
  const panelId = `share-panel-${row.id}`

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        id={rowId}
        aria-expanded={isOpen}
        aria-controls={panelId}
        onClick={onToggle}
        aria-label={
          isOpen
            ? t('viewer.row.collapse.aria', { method: row.method, path: row.path })
            : t('viewer.row.expand.aria', {
                method: row.method,
                path: row.path,
                status: row.status_code,
              })
        }
        className={cn(
          'grid min-h-11 w-full items-center gap-x-2 gap-y-1 px-3 py-2.5 text-left text-body-sm',
          'grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto] sm:grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto_auto_auto_auto]',
          'hover:bg-surface-hover focus-visible:bg-surface-hover',
          isOpen && 'bg-canvas',
        )}
      >
        <ChevronRight
          className={cn(
            'h-3.5 w-3.5 shrink-0 text-text-tertiary transition-transform duration-fast ease-standard',
            isOpen && 'rotate-90',
          )}
          aria-hidden="true"
        />
        <MethodBadge method={row.method} />
        <span className="truncate font-mono text-mono-sm text-text-primary" title={row.path}>
          {row.path}
        </span>
        <StatusCode code={row.status_code} />
        {/* Never hidden below `sm` — it wraps onto its own line instead
         * (design.md §7): `sm:contents` folds it back into the 7-col grid. */}
        <span className="col-span-4 flex flex-wrap items-center gap-2 pl-6 text-text-tertiary sm:col-span-1 sm:contents sm:pl-0">
          <ServedByChip servedBy={row.served_by} />
          <span className="tnum font-mono text-mono-sm text-text-secondary sm:text-right">
            {row.duration_ms}ms
          </span>
          <span className="text-caption text-text-tertiary sm:text-right" title={row.timestamp}>
            {relTime(row.timestamp)}
          </span>
        </span>
      </button>
      {isOpen && (
        <div
          id={panelId}
          role="region"
          aria-labelledby={rowId}
          className="border-t border-border bg-canvas px-3 py-3"
        >
          <SharedRequestDetail code={code} id={row.id} />
        </div>
      )}
    </div>
  )
}

type DetailState = { kind: 'loading' } | { kind: 'gone' } | { kind: 'error' } | { kind: 'ready'; detail: PublicRequestDetail }

function SharedRequestDetail({ code, id }: { code: string; id: number }) {
  const [state, setState] = useState<DetailState>({ kind: 'loading' })

  const load = useCallback(async () => {
    setState({ kind: 'loading' })
    try {
      const detail = await api.getSharedRequest(code, id)
      setState({ kind: 'ready', detail })
    } catch (err) {
      // AC-106/AC-S8(g): a detail 404 is "this request rolled off", never
      // terminal — the owner Inspector's "pending" state does not apply here
      // (this list came from the DB, not a WS broadcast).
      if (err instanceof ApiError && err.status === 404) {
        setState({ kind: 'gone' })
      } else {
        setState({ kind: 'error' })
      }
    }
  }, [code, id])

  useEffect(() => {
    void load()
  }, [load])

  if (state.kind === 'loading') {
    return (
      <div aria-busy="true">
        <span className="sr-only">{t('viewer.detail.loading.aria')}</span>
        <SkeletonLines lines={6} />
      </div>
    )
  }

  if (state.kind === 'gone') {
    return (
      <InlineAlert variant="info" role="status" title={t('viewer.detail.gone.title')}>
        {t('viewer.detail.gone.body')}
      </InlineAlert>
    )
  }

  if (state.kind === 'error') {
    return (
      <InlineAlert
        variant="danger"
        role="alert"
        title={t('viewer.detail.error.title')}
        action={
          <Button variant="secondary" size="sm" onClick={() => void load()}>
            <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
            {t('common.retry')}
          </Button>
        }
      >
        {t('viewer.detail.error.body')}
      </InlineAlert>
    )
  }

  const { detail } = state
  return (
    <Tabs defaultValue="headers">
      <TabsList>
        <TabsTrigger value="headers">{t('insp.tab.headers')}</TabsTrigger>
        <TabsTrigger value="query">{t('insp.tab.query')}</TabsTrigger>
        <TabsTrigger value="body">{t('insp.tab.body')}</TabsTrigger>
        <TabsTrigger value="response">{t('insp.tab.response')}</TabsTrigger>
      </TabsList>
      <div className="p-3">
        <TabsContent value="headers">
          <KeyValueRows data={detail.request_headers} emptyLabel={t('insp.headers.empty')} />
        </TabsContent>
        <TabsContent value="query">
          <KeyValueRows data={detail.query_params} emptyLabel={t('insp.query.empty')} />
        </TabsContent>
        <TabsContent value="body">
          {detail.request_body ? (
            <JsonTree raw={detail.request_body} />
          ) : (
            <p className="text-body-sm text-text-tertiary">{t('insp.body.empty')}</p>
          )}
        </TabsContent>
        <TabsContent value="response" className="space-y-4">
          <section className="space-y-1.5">
            <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
              {t('insp.response.servedByLabel')}
            </h3>
            <ServedByChip servedBy={detail.served_by} />
          </section>
          <section className="space-y-1.5">
            <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
              {t('insp.response.headers')}
            </h3>
            <KeyValueRows data={detail.response_headers} emptyLabel={t('insp.headers.empty')} />
          </section>
          <section className="space-y-1.5">
            <h3 className="text-overline uppercase tracking-wide text-text-tertiary">
              {t('insp.response.body')}
            </h3>
            {detail.response_body ? (
              <JsonTree raw={detail.response_body} />
            ) : (
              <p className="text-body-sm text-text-tertiary">{t('insp.response.empty')}</p>
            )}
          </section>
        </TabsContent>
      </div>
    </Tabs>
  )
}
