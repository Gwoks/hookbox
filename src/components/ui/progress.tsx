/** Progress (design.md §2.4/§3.3/§9.1). Determinate 6px bar, tokens only.
 * Shared by the F5 CSV export strip and the F3 config import. The visible
 * label is NOT itself an aria-live region — callers pair this with a
 * throttled sr-only announcement (design.md §3.3). Width transition is
 * zeroed by the global prefers-reduced-motion block (globals.css); never
 * indeterminate — both callers know `total` up front. */
import { cn } from '@/lib/cn'

export function Progress({
  value,
  max,
  label,
  className,
}: {
  value: number
  max: number
  label: string
  className?: string
}) {
  const pct = max > 0 ? Math.min(100, Math.max(0, Math.round((value / max) * 100))) : 0
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={label}
      aria-label={label}
      className={cn('h-1.5 w-full overflow-hidden rounded-pill bg-surface-active', className)}
    >
      <div
        className="h-full rounded-pill bg-accent-fill transition-[width] duration-base ease-standard"
        style={{ width: `${pct}%` }}
      />
    </div>
  )
}
