/** Segmented control (design.md §3.0/§3.6). Pretty/Raw on JsonTree, etc.
 * Selected = bg-surface + text-primary; the active segment is a filled chip on a
 * subtle track (non-color: the fill itself). */
import { cn } from '@/lib/cn'

export interface SegmentedProps<T extends string> {
  options: { value: T; label: string }[]
  value: T
  onChange: (v: T) => void
  className?: string
  'aria-label'?: string
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  className,
  ...rest
}: SegmentedProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={rest['aria-label']}
      className={cn('inline-flex items-center gap-0.5 rounded-sm bg-subtle p-0.5', className)}
    >
      {options.map((opt) => {
        const active = opt.value === value
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(opt.value)}
            className={cn(
              'rounded-[4px] px-2.5 py-1 text-caption font-medium transition-colors',
              active
                ? 'bg-surface text-text-primary shadow-xs'
                : 'text-text-tertiary hover:text-text-secondary',
            )}
          >
            {opt.label}
          </button>
        )
      })}
    </div>
  )
}
