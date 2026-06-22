import { cn } from '@/lib/cn'

/**
 * Spinner (design.md §3.0). Under prefers-reduced-motion the spin is suppressed
 * by the global CSS rule; the ring stays visible as a static cue (AC-D16).
 */
export function Spinner({ className }: { className?: string }) {
  return (
    <span
      role="status"
      aria-label="Loading"
      className={cn(
        'inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-r-transparent align-[-0.125em]',
        className,
      )}
    />
  )
}
