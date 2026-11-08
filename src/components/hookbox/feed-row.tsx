/** FeedRow (design.md §3.5 / §5 / §6, AC-D13/D18). A focusable role="option"
 * grid: [MethodBadge] [path mono-sm truncate] [StatusCode] [ServedByChip]
 * [latency tnum] [relative time]. Row ~40px, hairline separator, hover
 * bg-surface-hover. SELECTED = bg-surface-active + 3px leading accent rail +
 * aria-selected (a non-color marker, not hue alone — AC-D13). New-row arrival uses the feed-row-in
 * animation; instant under reduced-motion (the `.feed-row` class is targeted by
 * the globals reduced-motion block). */
import { cn } from '@/lib/cn'
import { REQUEST_GRID, RequestCells } from './request-cells'
import type { ServedBy } from './served-by-chip'

export interface FeedRowData {
  id: number
  method: string
  path: string
  status_code: number
  served_by: ServedBy
  duration_ms: number
  timestamp: string
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
        REQUEST_GRID,
        'hover:bg-surface-hover focus-visible:bg-surface-hover',
        selected && 'bg-surface-active shadow-[inset_3px_0_0_var(--accent)]',
        isNew && 'animate-feed-row-in',
        className,
      )}
    >
      <RequestCells request={row} />
    </div>
  )
}
