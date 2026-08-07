/** KeyValueRows (design.md §3.8). Two-column hairline-separated rows: key
 * (mono-sm, text-secondary) · value (mono-sm, text-primary, wrap). Per-row
 * CopyButton on hover/focus. A redacted value renders as a neutral-chip pill,
 * not raw text (AC-61 surfacing). Used by inspector Headers/Query/State. Values
 * are plain text nodes (XSS-inert) — never dangerouslySetInnerHTML. */
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { CopyButton } from '@/components/ui/copy-button'

// Backend writes '<redacted>' (backend/src/helpers.rs::redact) — this used to
// compare against '__redacted__' and the pill never rendered (AC-133).
const REDACTED = '<redacted>'

export function KeyValueRows({
  data,
  emptyLabel,
  className,
}: {
  data: Record<string, string>
  emptyLabel?: string
  className?: string
}) {
  const entries = Object.entries(data ?? {})
  if (entries.length === 0) {
    return (
      <p className="px-1 py-3 text-body-sm text-text-tertiary">
        {emptyLabel ?? t('insp.headers.none')}
      </p>
    )
  }
  return (
    <dl className={cn('divide-y divide-border', className)}>
      {entries.map(([k, v]) => {
        const redacted = v === REDACTED
        return (
          <div key={k} className="group grid grid-cols-[minmax(8rem,1fr)_2fr_auto] items-start gap-2 py-1.5">
            <dt className="break-all font-mono text-mono-sm text-text-secondary">{k}</dt>
            <dd className="break-all font-mono text-mono-sm text-text-primary">
              {redacted ? (
                <span className="inline-flex rounded-xs bg-neutral-chip-bg px-1.5 py-0.5 text-caption font-medium text-neutral-chip-fg">
                  {t('insp.headers.redacted')}
                </span>
              ) : (
                v
              )}
            </dd>
            {!redacted && (
              <CopyButton
                value={v}
                className="opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
              />
            )}
          </div>
        )
      })}
    </dl>
  )
}
