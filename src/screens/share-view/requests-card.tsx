/** The viewer's request list: one flush card with a toolbar (count, the
 * Read-only pill, "Updated …", an outline Refresh), the column header
 * (sm+), then loading · empty · the disclosure rows. */
import { RotateCw } from 'lucide-react'
import { useState } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { relTime } from '@/lib/time'
import { REQUEST_GRID_DISCLOSURE } from '@/components/hookbox/request-cells'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { EmptyState } from '@/components/ui/empty-state'
import { LoadingBlock } from '@/components/ui/loading-block'
import { Overline } from '@/components/ui/overline'
import { Pill } from '@/components/ui/pill'
import { SharedRequestRow } from './row'
import type { SharedFeedData, SharedFeedStatus } from './use-shared-feed'

const COLUMNS = [
  'viewer.col.method',
  'viewer.col.path',
  'viewer.col.status',
  'viewer.col.served',
  'viewer.col.duration',
  'viewer.col.when',
] as const

export interface RequestsCardProps {
  code: string
  data: SharedFeedData | null
  status: SharedFeedStatus
  onRefresh: () => void
}

function Toolbar({ data, status, onRefresh }: Omit<RequestsCardProps, 'code'>) {
  const updatedAt = data ? new Date(data.updatedAt).toISOString() : null
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-3">
      <div className="flex items-center gap-2">
        {data && <span className="text-body-sm text-text-secondary">{t('viewer.count', { n: data.requests.length })}</span>}
        <Pill tone="neutral">{t('viewer.readOnlyChip')}</Pill>
      </div>
      <div className="flex items-center gap-2">
        {updatedAt && (
          <span
            className={cn('text-caption', status === 'rateLimited' ? 'text-warning-fg' : 'text-text-tertiary')}
            title={updatedAt}
          >
            {t('viewer.updated', { when: relTime(updatedAt) })}
          </span>
        )}
        <Button variant="outline" size="sm" onClick={onRefresh} aria-label={t('viewer.refresh.aria')}>
          <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
          {t('viewer.refresh')}
        </Button>
      </div>
    </div>
  )
}

export function RequestsCard({ code, data, status, onRefresh }: RequestsCardProps) {
  const [openRowId, setOpenRowId] = useState<number | null>(null)
  const requests = data?.requests ?? []

  return (
    <Card padding="none" className="overflow-hidden">
      <Toolbar data={data} status={status} onRefresh={onRefresh} />
      {requests.length > 0 && (
        <div className={cn('hidden gap-2 border-b border-border px-4 py-2 sm:grid', REQUEST_GRID_DISCLOSURE)}>
          <span aria-hidden="true" />
          {COLUMNS.map((key) => (
            <Overline as="span" key={key}>
              {t(key)}
            </Overline>
          ))}
        </div>
      )}
      {!data && status === 'loading' && <LoadingBlock label={t('viewer.loading.aria')} lines={8} className="p-4" />}
      {data && requests.length === 0 && <EmptyState title={t('viewer.empty.title')} description={t('viewer.empty.body')} />}
      {requests.length > 0 && (
        <div>
          {requests.map((row) => (
            <SharedRequestRow
              key={row.id}
              code={code}
              row={row}
              isOpen={openRowId === row.id}
              onToggle={() => setOpenRowId((id) => (id === row.id ? null : row.id))}
            />
          ))}
        </div>
      )}
    </Card>
  )
}
