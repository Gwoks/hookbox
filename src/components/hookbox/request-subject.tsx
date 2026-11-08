/** RequestSubject — the "one subject, one color" strip above a request's
 * detail (design.md §2.3): method, path, status, served-by. Viewer-safe. */
import type { PublicRequestSummary } from '@/api/schemas'
import { cn } from '@/lib/cn'
import { MethodBadge } from './method-badge'
import { ServedByChip } from './served-by-chip'
import { StatusCode } from './status-code'

export type RequestSubjectData = Pick<PublicRequestSummary, 'method' | 'path' | 'status_code' | 'served_by'>

export function RequestSubject({ request, className }: { request: RequestSubjectData; className?: string }) {
  return (
    <div className={cn('flex flex-wrap items-center gap-2', className)}>
      <MethodBadge method={request.method} />
      <span className="min-w-0 truncate font-mono text-mono-sm text-text-primary" title={request.path}>
        {request.path}
      </span>
      <StatusCode code={request.status_code} />
      <ServedByChip servedBy={request.served_by} />
    </div>
  )
}
