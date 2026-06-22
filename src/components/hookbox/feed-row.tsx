/** FeedRow (design.md §3.5 / §5 / §6, AC-D13/D18). A focusable role="option"
 * grid: [MethodBadge] [path mono-sm truncate] [StatusCode] [ServedByChip]
 * [latency tnum] [relative time]. Row ~40px, hairline separator, hover bg-hover.
 * SELECTED = bg-active + 3px leading accent rail + aria-selected (a non-color
 * marker, not hue alone — AC-D13). New-row arrival uses the feed-row-in
 * animation; instant under reduced-motion (the `.feed-row` class is targeted by
 * the globals reduced-motion block). */
import { cn } from '@/lib/cn'
import { MethodBadge } from './method-badge'
import { StatusCode } from './status-code'
import { ServedByChip, type ServedBy } from './served-by-chip'

export interface FeedRowData {
  id: number
  method: string
  path: string
  status_code: number
  served_by: ServedBy
  duration_ms: number
  timestamp: string
}

function relTime(iso: string): string {
  const t = Date.parse(iso)
  if (Number.isNaN(t)) return ''
  const diff = Math.max(0, Date.now() - t)
  const s = Math.floor(diff / 1000)
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h`
  return `${Math.floor(h / 24)}d`
}

export function FeedRow({
  row,
  selected,
  isNew,
  onSelect,
  className,
}: {
  row: FeedRowData
  selected?: boolean
  isNew?: boolean
  onSelect?: (id: number) => void
  className?: string
}) {
  return (
    <div
      role="option"
      tabIndex={0}
      aria-selected={!!selected}
      onClick={() => onSelect?.(row.id)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect?.(row.id)
        }
      }}
      className={cn(
        'feed-row grid cursor-pointer items-center gap-2 border-b border-border px-3 py-2 text-body-sm',
        'grid-cols-[3.5rem_minmax(0,1fr)_auto_auto_auto_auto]',
        'hover:bg-hover focus-visible:bg-hover',
        selected && 'bg-active shadow-[inset_3px_0_0_var(--accent)]',
        isNew && 'animate-feed-row-in',
        className,
      )}
    >
      <MethodBadge method={row.method} />
      <span className="truncate font-mono text-mono-sm text-text-primary" title={row.path}>
        {row.path}
      </span>
      <StatusCode code={row.status_code} />
      <ServedByChip servedBy={row.served_by} />
      <span className="tnum text-right font-mono text-mono-sm text-text-secondary">
        {row.duration_ms}ms
      </span>
      <span className="w-9 text-right text-caption text-text-tertiary" title={row.timestamp}>
        {relTime(row.timestamp)}
      </span>
    </div>
  )
}
