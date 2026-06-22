/** MethodBadge (design.md §3.1, AC-D14). CVA variant keyed by HTTP method;
 * radius-xs, overline (uppercase mono, +0.06em), method fg/bg tokens. The TEXT
 * LABEL is the source of truth (grayscale-legible); color reinforces. Fixed
 * min-width so the feed's leading column aligns as rows stream (AC-D18). */
import { cva } from 'class-variance-authority'
import { cn } from '@/lib/cn'

const methodVariants = cva(
  'inline-flex min-w-[3.25rem] items-center justify-center rounded-xs px-1.5 py-0.5 font-mono text-overline uppercase tracking-[0.06em]',
  {
    variants: {
      method: {
        GET: 'text-method-get-fg bg-method-get-bg',
        POST: 'text-method-post-fg bg-method-post-bg',
        PUT: 'text-method-put-fg bg-method-put-bg',
        PATCH: 'text-method-patch-fg bg-method-patch-bg',
        DELETE: 'text-method-delete-fg bg-method-delete-bg',
        HEAD: 'text-method-head-fg bg-method-head-bg',
      },
    },
    defaultVariants: { method: 'HEAD' },
  },
)

type Known = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD'
const KNOWN: Known[] = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD']

export function MethodBadge({ method, className }: { method: string; className?: string }) {
  const upper = method.toUpperCase()
  // OPTIONS / ANY / unknown verbs share the neutral HEAD slot per design.md §2.2.
  const variant = (KNOWN.includes(upper as Known) ? upper : 'HEAD') as Known
  return (
    <span className={cn(methodVariants({ method: variant }), className)} title={upper}>
      {upper}
    </span>
  )
}
