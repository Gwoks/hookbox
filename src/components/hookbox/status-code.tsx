/** StatusCode (design.md §3.2, AC-D14/D18). Tabular-figure digits colored by
 * class; a 2px leading underline in the class color is the NON-HUE signal. The
 * digits stay full-contrast text-primary so they're legible even desaturated —
 * never rely on hue alone. */
import { cn } from '@/lib/cn'

function classOf(code: number): { ring: string } {
  if (code >= 200 && code < 300) return { ring: 'border-success-fg' }
  if (code >= 300 && code < 400) return { ring: 'border-info-fg' }
  if (code >= 400 && code < 500) return { ring: 'border-warning-fg' }
  if (code >= 500) return { ring: 'border-danger-fg' }
  return { ring: 'border-text-tertiary' }
}

export function StatusCode({ code, className }: { code: number; className?: string }) {
  const { ring } = classOf(code)
  return (
    <span
      className={cn(
        'tnum inline-block border-b-2 px-0.5 font-mono text-mono font-medium text-text-primary',
        ring,
        className,
      )}
      title={`HTTP ${code}`}
    >
      {code}
    </span>
  )
}
