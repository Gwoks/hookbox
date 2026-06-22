import { cn } from '@/lib/cn'

/**
 * Skeleton (design.md §5). Shimmer sweep on a muted block; under
 * prefers-reduced-motion the global CSS turns it static (AC-D16). Shapes should
 * mirror final content.
 */
export function Skeleton({ className }: { className?: string }) {
  return <div className={cn('skeleton h-4 w-full', className)} aria-hidden="true" />
}

export function SkeletonLines({ lines = 3, className }: { lines?: number; className?: string }) {
  return (
    <div className={cn('space-y-2', className)} aria-hidden="true">
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton key={i} className={i === lines - 1 ? 'w-2/3' : 'w-full'} />
      ))}
    </div>
  )
}
