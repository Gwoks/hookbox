/** CopyButton (design.md §3.7/§6, AC-132). Button → check + "Copied" for ~1.6s,
 * plus an sr-only aria-live announcement. `setCopied`-equivalent state only
 * flips to "copied" AFTER `navigator.clipboard.writeText` resolves — on
 * rejection (the shipped nginx listens on plain HTTP:80, a non-secure context
 * where the Clipboard API throws) it enters a distinct failure state instead
 * of silently claiming success. Every call site (CodeBlock, MockUrlChip,
 * KeyValueRows, JsonTree) already renders the value as visible, selectable
 * text right next to this button, so the failure state's fallback is just
 * "select it yourself" — no duplicate value control needed here. Used by
 * CodeBlock and KeyValueRows. */
import { Check, Copy, X } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { cn } from '@/lib/cn'
import { t } from '@/lib/copy'
import { Button } from './button'

type Status = 'idle' | 'copied' | 'failed'

export function CopyButton({
  value,
  label,
  className,
  size = 'icon-sm',
}: {
  value: string
  label?: string
  className?: string
  size?: 'icon' | 'icon-sm' | 'sm'
}) {
  const [status, setStatus] = useState<Status>('idle')
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const idleLabel = label ?? t('common.copy')

  const onCopy = useCallback(async () => {
    if (timer.current) clearTimeout(timer.current)
    try {
      await navigator.clipboard.writeText(value)
      setStatus('copied')
    } catch {
      setStatus('failed')
    }
    timer.current = setTimeout(() => setStatus('idle'), 1600)
  }, [value])

  const isCopied = status === 'copied'
  const isFailed = status === 'failed'
  const visibleLabel = isCopied ? t('common.copied') : isFailed ? t('common.copy.failed') : idleLabel

  return (
    <Button
      variant="ghost"
      size={size}
      onClick={onCopy}
      aria-label={visibleLabel}
      className={cn(
        'text-text-tertiary hover:text-text-primary',
        isFailed && 'text-danger-fg hover:text-danger-fg',
        className,
      )}
    >
      {isCopied && <Check className="h-4 w-4 text-success-fg" aria-hidden="true" />}
      {isFailed && <X className="h-4 w-4" aria-hidden="true" />}
      {!isCopied && !isFailed && <Copy className="h-4 w-4" aria-hidden="true" />}
      {size === 'sm' && <span>{visibleLabel}</span>}
      <span className="sr-only" role="status" aria-live="polite">
        {isCopied ? t('common.copy.announce') : isFailed ? t('common.copy.failed') : ''}
      </span>
    </Button>
  )
}
