/**
 * RequestCells — the cells of one request row, as a Fragment so the caller
 * owns the grid container (FeedRow's role="option", the viewer's disclosure
 * <button>, the landing mockup). The FIRST child is always the MethodBadge:
 * e2e AC-D18 measures `row.firstElementChild` to prove the leading column
 * aligns. Types come from @/api/schemas only (viewer-safe).
 *
 * `wrapMeta` folds served-by / duration / time into one cell that wraps onto
 * its own line below `sm` and dissolves back into the grid (`sm:contents`)
 * above it — use it with REQUEST_GRID_DISCLOSURE's leading chevron track.
 */
import type { PublicRequestSummary } from '@/api/schemas'
import { cn } from '@/lib/cn'
import { relTime } from '@/lib/time'
import { MethodBadge } from './method-badge'
import { ServedByChip } from './served-by-chip'
import { StatusCode } from './status-code'

/** [method][path][status][served-by][duration][time] */
export const REQUEST_GRID = 'grid-cols-[3.5rem_minmax(0,1fr)_auto_auto_auto_auto]'
/** A leading disclosure-chevron track, then REQUEST_GRID's columns. */
export const REQUEST_GRID_DISCLOSURE =
  'grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto] sm:grid-cols-[1rem_3.5rem_minmax(0,1fr)_auto_auto_auto_auto]'

export type RequestCellsData = Pick<
  PublicRequestSummary,
  'method' | 'path' | 'status_code' | 'served_by' | 'duration_ms' | 'timestamp'
>

export function RequestCells({ request, wrapMeta = false }: { request: RequestCellsData; wrapMeta?: boolean }) {
  const meta = (
    <>
      <ServedByChip servedBy={request.served_by} />
      <span
        className={cn(
          'tnum font-mono text-mono-sm text-text-secondary',
          wrapMeta ? 'sm:text-right' : 'text-right',
        )}
      >
        {request.duration_ms}ms
      </span>
      <span
        className={cn('text-caption text-text-tertiary', wrapMeta ? 'sm:text-right' : 'w-9 text-right')}
        title={request.timestamp}
      >
        {relTime(request.timestamp)}
      </span>
    </>
  )
  return (
    <>
      <MethodBadge method={request.method} />
      <span className="truncate font-mono text-mono-sm text-text-primary" title={request.path}>
        {request.path}
      </span>
      <StatusCode code={request.status_code} />
      {wrapMeta ? (
        <span className="col-span-4 flex flex-wrap items-center gap-2 pl-6 sm:col-span-1 sm:contents sm:pl-0">
          {meta}
        </span>
      ) : (
        meta
      )}
    </>
  )
}
