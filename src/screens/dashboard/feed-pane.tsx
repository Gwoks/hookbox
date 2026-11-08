/**
 * The live feed card body: header, the non-modal export progress strip
 * (the feed keeps streaming underneath), AC-121's dismissible "rows without
 * detail" note, then loading · empty · the newest-first listbox of FeedRows
 * (role="option", AC-D13/D18).
 */
import type { RequestSummary } from '@/api'
import { t } from '@/lib/copy'
import { FeedRow } from '@/components/hookbox/feed-row'
import { InlineAlert } from '@/components/hookbox/inline-alert'
import { Button } from '@/components/ui/button'
import { LoadingBlock } from '@/components/ui/loading-block'
import { Progress } from '@/components/ui/progress'
import { FeedEmpty } from './feed-empty'
import { FeedHeader, type FeedHeaderProps } from './feed-header'
import type { CsvExport, ExportPhase } from './use-csv-export'

export interface FeedPaneProps extends Omit<FeedHeaderProps, 'count' | 'onExport'> {
  rows: RequestSummary[]
  mockUrl: string
  isLoaded: boolean
  selectedId: number | null
  onSelect: (id: number) => void
  csv: CsvExport
}

function exportLabel(phase: ExportPhase): string {
  if (phase.kind === 'fetching') return t('feed.export.progress', { done: phase.done, total: phase.total })
  if (phase.kind === 'serialising') return t('feed.export.preparing')
  return ''
}

function ExportStrip({ csv }: { csv: CsvExport }) {
  const { phase } = csv
  if (phase.kind === 'idle') return null
  const label = exportLabel(phase)
  const value = phase.kind === 'fetching' ? phase.done : phase.total
  return (
    <div className="animate-fade-in space-y-1.5 border-b border-border bg-accent-subtle-bg px-4 py-2">
      <div className="flex items-center justify-between gap-2">
        <span className="tnum text-body-sm text-text-secondary">{label}</span>
        <Button
          variant="ghost"
          size="sm"
          onClick={csv.cancel}
          disabled={phase.kind === 'serialising'}
          aria-label={t('feed.export.cancel.aria')}
        >
          {t('feed.export.cancel')}
        </Button>
      </div>
      <Progress value={value} max={phase.total} label={label} />
    </div>
  )
}

export function FeedPane({ rows, mockUrl, isLoaded, selectedId, onSelect, csv, ...header }: FeedPaneProps) {
  const count = isLoaded && rows.length > 0 ? rows.length : null
  return (
    <div className="flex h-full min-h-0 flex-col">
      <FeedHeader {...header} count={count} onExport={() => void csv.start(rows)} />
      <ExportStrip csv={csv} />
      {csv.hasDetailNote && (
        <div className="border-b border-border px-4 py-2">
          <InlineAlert
            variant="info"
            role="status"
            action={
              <Button variant="ghost" size="sm" onClick={csv.dismissDetailNote}>
                {t('common.dismiss')}
              </Button>
            }
          >
            {t('feed.export.detailNote')}
          </InlineAlert>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto">
        {!isLoaded ? (
          <LoadingBlock label={t('feed.loading.aria')} lines={8} className="p-4" />
        ) : rows.length === 0 ? (
          <FeedEmpty mockUrl={mockUrl} />
        ) : (
          <div role="listbox" aria-label={t('feed.header.title')} className="flex flex-col">
            {rows.map((row, i) => (
              <FeedRow key={row.id} row={row} selected={row.id === selectedId} isNew={i === 0} onSelect={onSelect} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
