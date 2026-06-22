/** CodeBlock (design.md §3.7, AC-D19). radius-md, bg-subtle, mono-lg, 1px
 * border, integrated CopyButton top-right. Mock-URL chips are copy-only and
 * render in text-primary mono — NEVER accent/link color — so users don't expect
 * navigation (AC-D19). */
import { cn } from '@/lib/cn'
import { CopyButton } from '@/components/ui/copy-button'

export function CodeBlock({
  value,
  copy = true,
  className,
  ariaLabel,
}: {
  value: string
  copy?: boolean
  className?: string
  ariaLabel?: string
}) {
  return (
    <div
      className={cn(
        'relative flex items-start gap-2 rounded-md border border-border bg-subtle px-3 py-2',
        className,
      )}
    >
      <code
        aria-label={ariaLabel}
        className="block flex-1 overflow-x-auto whitespace-pre-wrap break-all font-mono text-mono-lg text-text-primary"
      >
        {value}
      </code>
      {copy && <CopyButton value={value} className="shrink-0" />}
    </div>
  )
}

/** Inline mock-URL chip — copy-only, text-primary mono, no navigation (AC-D19). */
export function MockUrlChip({ url, className }: { url: string; className?: string }) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-xs border border-border bg-subtle px-1.5 py-0.5',
        className,
      )}
    >
      <code className="font-mono text-mono-sm text-text-primary">{url}</code>
      <CopyButton value={url} className="h-4 w-4" />
    </span>
  )
}
